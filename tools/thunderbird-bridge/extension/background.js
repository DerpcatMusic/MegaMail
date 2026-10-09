"use strict";

const HOST_NAME = "__HOST_NAME__";
const MAX_MESSAGE_BYTES = 100 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES = 50 * 1024 * 1024;
const MAX_UPLOADS = 20;
const MAX_CHUNK_BYTES = 512 * 1024;
const MAX_BODY_BYTES = 2 * 1024 * 1024;
const MAX_BODY_SOURCE_BYTES = 20 * 1024 * 1024;
const MAX_CONVERSATION_IDS = 256;
const MAX_CONVERSATION_SEEDS = 100;
const MAX_CONVERSATION_PARENT_IDS = 24;
const MAX_CONVERSATION_FOLDERS = 8;
const MAX_CONVERSATION_MESSAGES = 100;
// ponytail: reply hints inspect one bounded Sent header window per batch; widen only if older replies need coverage.
const MAX_CONVERSATION_CANDIDATES = 128;
const MAX_CONVERSATION_HEADERS_PER_ID = 8;
const CONVERSATION_HEADER_CACHE_LIMIT = 8192;
const lists = new Map();
const downloads = new Map();
const uploads = new Map();
const conversationHeaders = new Map();
let nextToken = 1;
let port;

function expireTransfers() {
  const now = Date.now();
  for (const [key, item] of downloads) if (item.expires < now) downloads.delete(key);
  for (const [key, item] of uploads) if (item.expires < now) uploads.delete(key);
}

function expireLists() {
  const now = Date.now();
  for (const [key, item] of lists) {
    if (item.expires < now) {
      lists.delete(key);
      messenger.messages.abortList(item.id).catch(() => {});
    }
  }
}

function token() {
  return `${Date.now().toString(36)}-${(nextToken++).toString(36)}`;
}

function safeText(value) {
  return typeof value === "string" ? value : "";
}

function folderJson(folder) {
  return {
    id: String(folder.id),
    name: safeText(folder.name),
    type: safeText(folder.type),
    path: safeText(folder.path),
    subFolders: Array.isArray(folder.subFolders) ? folder.subFolders.map(folderJson) : []
  };
}

function accountJson(account) {
  return {
    ...accountSummary(account),
    rootFolder: account.rootFolder ? folderJson(account.rootFolder) : null
  };
}

function accountSummary(account) {
  return {
    id: String(account.id),
    name: safeText(account.name),
    type: safeText(account.type),
    identities: (account.identities || []).map(identity => ({
      id: String(identity.id),
      name: safeText(identity.name),
      email: safeText(identity.email)
    }))
  };
}

async function allAccounts() {
  const accounts = (await messenger.accounts.list(false)).filter(account => account.type === "imap");
  return accounts.map(accountSummary);
}

function hasCachedInbox(folders) {
  if (!Array.isArray(folders)) return false;
  return folders.some(folder =>
    safeText(folder.type).toLowerCase() === "inbox" ||
    safeText(folder.name).toLowerCase() === "inbox" ||
    hasCachedInbox(Array.isArray(folder.subFolders) ? folder.subFolders : []));
}

async function findFolder(id) {
  let folder;
  try {
    folder = await messenger.folders.get(String(id), false);
  } catch (_) {
    throw new Error("Thunderbird could not find the selected folder. Refresh folders and try again.");
  }
  const accountId = safeText(folder.accountId);
  const account = accountId ? await messenger.accounts.get(accountId, false) : null;
  if (!account || account.type !== "imap") {
    throw new Error("MegaMail currently supports IMAP folders only.");
  }
  return folder;
}

async function loadAccount(account, refresh = false) {
  const result = accountJson(account);
  try {
    const readLocalFolders = async () => {
      const fresh = await messenger.accounts.get(String(account.id), false);
      const discovered = await messenger.folders.query({ accountId: String(account.id) });
      const subFolders = await messenger.folders.getSubFolders(fresh, true);
      return {
        fresh,
        discovered,
        subFolders,
        rootFolder: fresh.rootFolder ? folderJson({ ...fresh.rootFolder, subFolders }) : null
      };
    };
    if (refresh) await messenger.megamailSync.discoverFolders(String(account.id), true);
    let snapshot = await readLocalFolders();
    if (!refresh && !hasCachedInbox(snapshot.subFolders)) {
      await messenger.megamailSync.discoverFolders(String(account.id), true);
      snapshot = await readLocalFolders();
    }
    result.rootFolder = snapshot.rootFolder;
    if (!snapshot.discovered.length) {
      result.folderError = "Thunderbird connected but did not report any remote folders for this account.";
    }
  } catch (error) {
    result.folderError = error instanceof Error ? error.message : "Thunderbird could not discover this account's folders.";
  }
  return result;
}

function messageJson(message) {
  return {
    id: String(message.id),
    subject: safeText(message.subject),
    author: safeText(message.author),
    recipients: message.recipients || [],
    cc: message.ccList || [],
    date: message.date instanceof Date ? message.date.getTime() : Number(message.date || 0),
    read: Boolean(message.read),
    flagged: Boolean(message.flagged),
    hasAttachment: typeof message.hasAttachments === "boolean" ? message.hasAttachments : null,
    size: Number(message.size || 0),
    tags: Array.isArray(message.tags) ? message.tags : [],
    headerMessageId: safeText(message.headerMessageId)
  };
}

function headerValue(headers, name) {
  return headerValueInfo(headers, name).value;
}

function headerValueInfo(headers, name) {
  const key = Object.keys(headers || {}).find(item => item.toLowerCase() === name.toLowerCase());
  if (!key) return { value: "", truncated: false };
  const values = Array.isArray(headers[key]) ? headers[key] : [headers[key]];
  const text = values.map(safeText).filter(Boolean).join(" ");
  return { value: text.slice(0, 8192), truncated: text.length > 8192 };
}

function normalizedMessageId(value) {
  const id = safeText(value).trim();
  if (!id || id.length > 998) return "";
  return id.replace(/^</, "").replace(/>$/, "").toLowerCase();
}

function normalizedSubject(value) {
  return safeText(value).trim()
    .replace(/^(?:(?:re|fw|fwd)\s*(?:\[\d+\])?\s*:\s*)+/i, "")
    .replace(/\s+/g, " ").trim().slice(0, 512).toLowerCase();
}

function conversationIdList(values) {
  const ids = [];
  const seen = new Set();
  let partial = !Array.isArray(values) || values.length > MAX_CONVERSATION_IDS;
  for (const value of (Array.isArray(values) ? values : []).slice(0, MAX_CONVERSATION_IDS)) {
    const id = safeText(value).trim();
    const normalized = normalizedMessageId(id);
    if (!normalized) {
      if (id) partial = true;
      continue;
    }
    if (!seen.has(normalized)) {
      seen.add(normalized);
      ids.push(id);
    }
  }
  return { ids, partial };
}

function rememberConversationHeaders(accountId, messageId, headers) {
  const key = String(messageId);
  let cache = conversationHeaders.get(accountId);
  if (!cache) {
    cache = new Map();
    conversationHeaders.set(accountId, cache);
  }
  cache.delete(key);
  cache.set(key, headers);
  if (cache.size > CONVERSATION_HEADER_CACHE_LIMIT) cache.delete(cache.keys().next().value);
}

async function conversationHeaderFields(accountId, messageId) {
  const key = String(messageId);
  let cache = conversationHeaders.get(accountId);
  if (cache && cache.has(key)) {
    const value = cache.get(key);
    cache.delete(key);
    cache.set(key, value);
    return value;
  }
  const headers = await messenger.messages.getHeaders(Number(messageId), { decodeHeaders: true });
  const references = headerValueInfo(headers, "references");
  const inReplyTo = headerValueInfo(headers, "in-reply-to");
  const fields = {
    references: references.value,
    inReplyTo: inReplyTo.value,
    replyTo: headerValue(headers, "reply-to"),
    truncated: references.truncated || inReplyTo.truncated
  };
  rememberConversationHeaders(accountId, key, fields);
  return fields;
}

async function mapLimited(items, limit, fn) {
  const output = [];
  for (let offset = 0; offset < items.length; offset += limit) {
    output.push(...await Promise.all(items.slice(offset, offset + limit).map(fn)));
  }
  return output;
}

async function limitedConversationQuery(query, limit) {
  let list = await messenger.messages.query({
    ...query,
    messagesPerPage: limit + 1,
    autoPaginationTimeout: 0
  });
  if (typeof list === "string") {
    await messenger.messages.abortList(list).catch(() => {});
    return { messages: [], partial: true };
  }
  if (!list || !Array.isArray(list.messages)) {
    throw new Error("Thunderbird returned an invalid conversation query.");
  }
  const partial = Boolean(list.id) || list.messages.length > limit;
  if (list.id) await messenger.messages.abortList(list.id).catch(() => {});
  return { messages: list.messages.slice(0, limit), partial };
}

async function conversationMessageJson(message, accountId, allowedFolderIds, fallbackFolderId = "") {
  const folder = message && message.folder;
  const folderId = String((folder && folder.id) || fallbackFolderId);
  if (!message || !Number.isSafeInteger(Number(message.id)) || !folderId ||
      (folder && String(folder.accountId) !== accountId) || !allowedFolderIds.has(folderId)) {
    throw new Error("Thunderbird returned a conversation message outside the selected account or folders.");
  }
  const headers = await conversationHeaderFields(accountId, message.id);
  return {
    ...messageJson(message),
    folderId,
    references: headers.references,
    inReplyTo: headers.inReplyTo,
    replyTo: headers.replyTo,
    headersTruncated: headers.truncated
  };
}

function referencesAny(message, ids) {
  const refs = `${safeText(message.references)} ${safeText(message.inReplyTo)}`;
  return refs.split(/\s+/).some(id => ids.has(normalizedMessageId(id)));
}

function referenceIds(message) {
  return `${safeText(message.references)} ${safeText(message.inReplyTo)}`
    .split(/\s+/)
    .map(id => ({ id, normalized: normalizedMessageId(id) }))
    .filter(item => item.normalized);
}

function headerIdentity(message) {
  const author = safeText(message.author).trim();
  const address = author.match(/<([^<>]+)>/)?.[1] || author;
  return `${address.trim().toLowerCase()}\u0000${Number(message.date || 0)}`;
}

function unambiguousHeaderIds(messages) {
  const identities = new Map();
  const ambiguous = new Set();
  for (const message of messages) {
    const id = normalizedMessageId(message.headerMessageId);
    if (!id) continue;
    const identity = headerIdentity(message);
    if (identities.has(id) && identities.get(id) !== identity) ambiguous.add(id);
    else identities.set(id, identity);
  }
  return new Set([...identities.keys()].filter(id => !ambiguous.has(id)));
}

function uniquePhysicalMessages(messages) {
  const unique = new Map();
  for (const message of messages) unique.set(String(message.id), message);
  return [...unique.values()];
}

async function exactConversationMessages(accountId, lookups) {
  let partial = false;
  const results = await mapLimited(lookups, 4, async ({ headerMessageId, folderIds }) => {
    const result = await limitedConversationQuery({ accountId, folderId: folderIds, headerMessageId }, MAX_CONVERSATION_HEADERS_PER_ID);
    partial ||= result.partial;
    return result.messages;
  });
  return { messages: uniquePhysicalMessages(results.flat()), partial };
}

async function expandParentHeaders(accountId, folders, seedMessages, allowedFolderIds, priorPartial = false) {
  let messages = uniquePhysicalMessages(seedMessages);
  let partial = priorPartial;
  let remaining = MAX_CONVERSATION_PARENT_IDS;
  const folderIds = folders.map(folder => String(folder.id));
  const queried = new Set(messages.map(message => normalizedMessageId(message.headerMessageId)).filter(Boolean));
  while (remaining > 0) {
    const mapped = await mapLimited(messages, 8, message => conversationMessageJson(message, accountId, allowedFolderIds));
    if (mapped.some(message => message.headersTruncated)) partial = true;
    const uniqueIds = unambiguousHeaderIds(mapped);
    const parents = [];
    for (const message of mapped) {
      if (!uniqueIds.has(normalizedMessageId(message.headerMessageId))) continue;
      for (const item of referenceIds(message)) {
        if (queried.has(item.normalized)) continue;
        queried.add(item.normalized);
        parents.push(item.id);
      }
    }
    if (!parents.length) break;
    if (parents.length > remaining) partial = true;
    const lookups = parents.slice(0, remaining).map(headerMessageId => ({ headerMessageId, folderIds }));
    remaining -= lookups.length;
    const result = await exactConversationMessages(accountId, lookups);
    partial ||= result.partial;
    const previousCount = messages.length;
    messages = uniquePhysicalMessages([...messages, ...result.messages]);
    if (messages.length === previousCount && parents.length <= lookups.length) break;
  }
  const headers = await mapLimited(messages, 8, message => conversationMessageJson(message, accountId, allowedFolderIds));
  if (headers.some(message => message.headersTruncated)) partial = true;
  return { messages, headers, uniqueIds: unambiguousHeaderIds(headers), partial };
}

async function relatedCandidateMessages(accountId, exactHeaders, candidateRaw, allowedFolderIds) {
  const candidates = await mapLimited(candidateRaw, 8, ({ message, folderId }) => conversationMessageJson(message, accountId, allowedFolderIds, folderId));
  const allUniqueIds = unambiguousHeaderIds([...exactHeaders, ...candidates]);
  const known = new Set(unambiguousHeaderIds(exactHeaders));
  const included = new Set();
  let changed = true;
  while (changed) {
    changed = false;
    for (let index = 0; index < candidates.length; index++) {
      const ownId = normalizedMessageId(candidates[index].headerMessageId);
      if (included.has(index) || !allUniqueIds.has(ownId) || (!known.has(ownId) && !referencesAny(candidates[index], known))) continue;
      included.add(index);
      known.add(ownId);
      changed = true;
    }
  }
  return {
    messages: candidateRaw.filter((_, index) => included.has(index)).map(item => item.message),
    partial: candidates.some(message => message.headersTruncated)
  };
}

async function batchConversationHeaders(accountId, folders, seedIds, allowedFolderIds) {
  const excluded = new Set(["sent", "drafts", "templates", "trash", "junk"]);
  let partial = !Array.isArray(seedIds) || seedIds.length === 0 || seedIds.length > MAX_CONVERSATION_SEEDS;
  const seeds = (Array.isArray(seedIds) ? seedIds : []).slice(0, MAX_CONVERSATION_SEEDS);
  if (!seeds.length) return { messages: [], partial: true };
  const exact = await exactConversationMessages(accountId, seeds.map(headerMessageId => ({
    headerMessageId,
    folderIds: folders.map(folder => String(folder.id))
  })));
  partial ||= exact.partial;
  const foundSeeds = new Set(exact.messages.map(message => normalizedMessageId(message.headerMessageId)));
  if (seeds.some(id => !foundSeeds.has(normalizedMessageId(id)))) partial = true;
  const expanded = await expandParentHeaders(accountId, folders, exact.messages, allowedFolderIds, partial);
  partial = expanded.partial;
  const otherFolders = folders.filter(folder => {
    const type = safeText(folder.type).toLowerCase();
    return type !== "sent" && !excluded.has(type);
  });
  const sentFolders = folders.filter(folder => safeText(folder.type).toLowerCase() === "sent");
  const jobs = [];
  if (otherFolders.length) jobs.push({ folders: otherFolders });
  if (sentFolders.length) jobs.push({ folders: sentFolders, fromMe: true });
  if (!jobs.length) return { messages: expanded.messages, partial: true };

  let extra = MAX_CONVERSATION_CANDIDATES % jobs.length;
  const results = await mapLimited(jobs, 2, async job => {
    const limit = Math.floor(MAX_CONVERSATION_CANDIDATES / jobs.length) + (extra-- > 0 ? 1 : 0);
    return limitedConversationQuery({
      accountId,
      folderId: job.folders.map(folder => String(folder.id)),
      ...(job.fromMe ? { fromMe: true } : {})
    }, limit);
  });
  const unique = new Map();
  for (const result of results) {
    partial ||= result.partial;
    for (const message of result.messages) unique.set(String(message.id), message);
  }
  const candidates = await mapLimited([...unique.values()], 8, message => conversationMessageJson(message, accountId, allowedFolderIds));
  if (candidates.some(message => message.headersTruncated)) partial = true;

  const allUniqueIds = unambiguousHeaderIds([...expanded.headers, ...candidates]);
  const known = new Set(expanded.uniqueIds);
  const included = new Set();
  let changed = true;
  while (changed) {
    changed = false;
    for (let index = 0; index < candidates.length; index++) {
      const ownId = normalizedMessageId(candidates[index].headerMessageId);
      if (included.has(index) || !allUniqueIds.has(ownId) || (!known.has(ownId) && !referencesAny(candidates[index], known))) continue;
      included.add(index);
      if (ownId) known.add(ownId);
      changed = true;
    }
  }
  return { messages: uniquePhysicalMessages([
    ...expanded.messages,
    ...[...unique.values()].filter((_, index) => included.has(index))
  ]), partial };
}

async function conversationMessages(params) {
  const accountId = safeText(params.accountId);
  if (!accountId) throw new Error("Thunderbird could not identify the selected account for this conversation.");
  const account = await messenger.accounts.get(accountId, false).catch(() => null);
  if (!account || account.type !== "imap") throw new Error("Thunderbird could not find the selected IMAP account.");

  const requestedFolders = Array.isArray(params.folderIds) ? params.folderIds.map(String) : [];
  if (!requestedFolders.length || requestedFolders.length > MAX_CONVERSATION_FOLDERS) {
    throw new Error("Thunderbird conversation lookup needs between 1 and 8 selected folders.");
  }
  const folders = [];
  const seenFolders = new Set();
  for (const id of requestedFolders) {
    if (!id || seenFolders.has(id)) continue;
    const folder = await messenger.folders.get(id, false).catch(() => null);
    if (!folder || String(folder.accountId) !== accountId) {
      throw new Error("Thunderbird conversation lookup received a folder outside the selected account.");
    }
    seenFolders.add(id);
    folders.push(folder);
  }
  if (!folders.length) throw new Error("Thunderbird could not identify any conversation folders.");

  const { ids, partial: idsPartial } = conversationIdList(params.ids);
  if (!ids.length) return { messages: [], partial: idsPartial };
  const allowedFolderIds = new Set(folders.map(folder => String(folder.id)));
  let partial = idsPartial;
  let rawMessages;
  if (params.includeBatchRelated === true) {
    const { ids: seedIds, partial: seedPartial } = conversationIdList(params.seedIds);
    const batch = await batchConversationHeaders(accountId, folders, seedIds, allowedFolderIds);
    rawMessages = batch.messages;
    partial ||= seedPartial || batch.partial;
  } else {
    const folderIds = folders.map(folder => String(folder.id));
    const exact = await exactConversationMessages(accountId, ids.map(headerMessageId => ({ headerMessageId, folderIds })));
    partial ||= exact.partial;
    rawMessages = exact.messages;
    if (rawMessages.length > MAX_CONVERSATION_MESSAGES) {
      rawMessages = rawMessages.slice(0, MAX_CONVERSATION_MESSAGES);
      partial = true;
    }
    if (params.includeReplies === true && rawMessages.length) {
      const expanded = await expandParentHeaders(accountId, folders, rawMessages, allowedFolderIds, partial);
      rawMessages = expanded.messages;
      partial = expanded.partial;
      const allSubjects = [...new Set(expanded.headers
        .filter(message => expanded.uniqueIds.has(normalizedMessageId(message.headerMessageId)))
        .map(message => normalizedSubject(message.subject))
        .filter(subject => subject.length >= 3))];
      const subjects = allSubjects.slice(0, 2);
      if (subjects.length === 0) partial = true;
      if (allSubjects.length > subjects.length) partial = true;
      const excluded = new Set(["drafts", "templates", "trash", "junk"]);
      const candidateFolders = folders.filter(folder => {
        const type = safeText(folder.type).toLowerCase();
        return !excluded.has(type);
      });
      if (!candidateFolders.length) partial = true;
      const slots = Math.max(1, subjects.length * candidateFolders.length);
      const baseLimit = Math.floor(MAX_CONVERSATION_CANDIDATES / slots);
      let extra = MAX_CONVERSATION_CANDIDATES % slots;
      const candidateResults = await mapLimited(subjects.flatMap(subject => candidateFolders.map(folder => ({ subject, folder }))), 4, async ({ subject, folder }) => {
        const limit = baseLimit + (extra-- > 0 ? 1 : 0);
        const type = safeText(folder.type).toLowerCase();
        const result = await limitedConversationQuery({
          accountId,
          folderId: [String(folder.id)],
          subject,
          ...(type === "sent" ? { fromMe: true } : {})
        }, limit);
        return { ...result, folderId: String(folder.id) };
      });
      const candidateRaw = candidateResults.flatMap(result => {
        partial ||= result.partial;
        return result.messages.map(message => ({ message, folderId: result.folderId }));
      });
      const related = await relatedCandidateMessages(accountId, expanded.headers, candidateRaw, allowedFolderIds);
      rawMessages.push(...related.messages);
      partial ||= related.partial;
    }
  }

  const uniqueRaw = [];
  const seenMessageIds = new Set();
  for (const message of rawMessages) {
    const key = String(message.id);
    if (seenMessageIds.has(key)) continue;
    seenMessageIds.add(key);
    uniqueRaw.push(message);
  }
  if (uniqueRaw.length > MAX_CONVERSATION_MESSAGES) {
    uniqueRaw.length = MAX_CONVERSATION_MESSAGES;
    partial = true;
  }
  const mapped = await mapLimited(uniqueRaw, 8, async message => conversationMessageJson(message, accountId, allowedFolderIds));
  if (mapped.some(message => message.headersTruncated)) partial = true;
  return {
    messages: mapped,
    partial,
    warning: partial ? "More messages may exist outside the indexed/query window" : null
  };
}

async function messageList(params) {
  if (!params.cursor) {
    const folder = await findFolder(params.folderId);
    const limit = Math.max(1, Math.min(Number(params.limit) || 100, 100));
    let list;
    if (safeText(params.search).trim()) {
      list = await messenger.messages.query({
        folderId: [String(folder.id)],
        fullText: safeText(params.search).trim().slice(0, 512),
        messagesPerPage: limit,
        autoPaginationTimeout: 250
      });
    } else {
      list = await messenger.messages.list(folder, { sortType: "date", sortOrder: "descending" });
    }
    if (typeof list === "string") list = await messenger.messages.continueList(list);
    const unreadMessageCount = await readUnreadMessageCount(folder);
    return listJson(list, String(folder.id), unreadMessageCount);
  }
  expireLists();
  const key = String(params.cursor);
  const entry = lists.get(key);
  if (!entry) throw new Error("This message list expired. Refresh the folder.");
  const list = await messenger.messages.continueList(entry.id);
  lists.delete(key);
  return listJson(list, entry.folderId);
}

async function refreshFolder(params) {
  const folder = await findFolder(params.folderId);
  if (!messenger.megamailSync || !messenger.megamailSync.discoverFolders || !messenger.megamailSync.getNewMessages) {
    throw new Error("This Thunderbird version cannot synchronize mail through MegaMail.");
  }
  await messenger.megamailSync.discoverFolders(String(folder.accountId));
  const status = await messenger.megamailSync.getNewMessages(String(folder.id));
  if (!status || status.synced !== true) {
    throw new Error("Thunderbird did not synchronize the selected folder.");
  }
  return status;
}

async function readUnreadMessageCount(folder) {
  try {
    const info = await messenger.folders.getFolderInfo(String(folder.id));
    const count = info?.unreadMessageCount;
    return Number.isSafeInteger(count) && count >= 0 ? Math.min(count, 0xffffffff) : null;
  } catch (_) {
    return null;
  }
}

function listJson(list, folderId, unreadMessageCount = null) {
  expireLists();
  const cursor = list.id ? token() : null;
  if (cursor) {
    if (lists.size >= 64) {
      const [oldest] = lists.entries().next().value;
      const old = lists.get(oldest);
      lists.delete(oldest);
      messenger.messages.abortList(old.id).catch(() => {});
    }
    lists.set(cursor, { id: list.id, folderId, expires: Date.now() + 120000 });
  }
  const result = { messages: (list.messages || []).map(messageJson), cursor };
  if (Number.isSafeInteger(unreadMessageCount) && unreadMessageCount >= 0) {
    result.unreadMessageCount = Math.min(unreadMessageCount, 0xffffffff);
  }
  return result;
}

function collectBody(parts, output) {
  if (!parts) return;
  for (const part of parts) {
    const contentType = safeText(part.contentType).toLowerCase();
    if (typeof part.body === "string") {
      if (contentType.startsWith("text/plain") && output.plainText === null) output.plainText = part.body;
      if (contentType.startsWith("text/html") && output.html === null) output.html = part.body;
    }
    collectBody(part.parts, output);
  }
}

async function messageBody(params) {
  const messageId = Number(params.messageId);
  const header = await messenger.messages.get(messageId);
  const advertisedSize = Number(header.size || 0);
  if (!Number.isSafeInteger(advertisedSize) || advertisedSize <= 0 || advertisedSize > MAX_BODY_SOURCE_BYTES) {
    throw new Error("This message is too large to display safely in the reader. Download the raw message or its attachments instead.");
  }
  const [full, headers, attachments] = await Promise.all([
    messenger.messages.getFull(messageId, { decodeContent: true }),
    messenger.messages.getHeaders(messageId, { decodeHeaders: true }),
    messenger.messages.listAttachments(messageId)
  ]);
  const output = { plainText: null, html: null };
  collectBody(full.parts || [full], output);
  if (output.plainText === null && output.html === null) {
    throw new Error("Thunderbird has no readable body for this message.");
  }
  const enc = new TextEncoder();
  let used = 0;
  let truncated = false;
  for (const field of ["plainText", "html"]) {
    if (output[field] === null) continue;
    const bytes = enc.encode(output[field]);
    const available = Math.max(0, MAX_BODY_BYTES - used);
    if (bytes.byteLength > available) {
      output[field] = new TextDecoder().decode(bytes.subarray(0, available));
      truncated = true;
    }
    used += enc.encode(output[field]).byteLength;
  }
  const previewSource = output.plainText || output.html || "";
  const preview = previewSource.replace(/<[^>]*>/g, " ").replace(/\s+/g, " ").trim().slice(0, 240);
  return {
    ...output,
    preview,
    hasAttachment: attachments.length > 0,
    replyTo: headerValue(headers, "reply-to"),
    inReplyTo: headerValue(headers, "in-reply-to"),
    references: headerValue(headers, "references"),
    truncated
  };
}

function collectAttachments(parts, output) {
  for (const part of parts || []) {
    if (part.name && part.partName) {
      output.push({
        name: safeText(part.name),
        partName: String(part.partName),
        mimeType: safeText(part.contentType) || "application/octet-stream",
        size: Number(part.size || 0)
      });
    }
    collectAttachments(part.parts, output);
  }
}

async function attachmentList(params) {
  const attachments = await messenger.messages.listAttachments(Number(params.messageId));
  return attachments.map(attachment => ({
    name: safeText(attachment.name),
    partName: String(attachment.partName),
    contentType: safeText(attachment.contentType) || "application/octet-stream",
    size: Number(attachment.size || 0)
  }));
}

async function startDownload(params) {
  let file;
  let limit = MAX_MESSAGE_BYTES;
  let name = "message.eml";
  let mimeType = "message/rfc822";
  if (params.kind === "attachment") {
    const messageId = Number(params.messageId);
    const partName = String(params.partName || "");
    const attachments = await messenger.messages.listAttachments(messageId);
    const attachment = attachments.find(item => String(item.partName) === partName);
    if (!attachment) throw new Error("Thunderbird could not find this attachment.");
    const advertisedSize = Number(attachment.size || 0);
    if (!Number.isSafeInteger(advertisedSize) || advertisedSize <= 0 || advertisedSize > MAX_ATTACHMENT_BYTES) {
      throw new Error("Attachment exceeds MegaMail's 50 MiB transfer limit or has no reliable size metadata.");
    }
    file = await messenger.messages.getAttachmentFile(messageId, partName);
    limit = MAX_ATTACHMENT_BYTES;
    name = safeText(attachment.name) || "attachment.bin";
    mimeType = safeText(attachment.contentType) || "application/octet-stream";
  } else {
    const header = await messenger.messages.get(Number(params.messageId));
    const advertisedSize = Number(header.size || 0);
    if (!Number.isSafeInteger(advertisedSize) || advertisedSize <= 0 || advertisedSize > MAX_MESSAGE_BYTES) {
      throw new Error("Message exceeds MegaMail's 100 MiB raw-message limit or has no reliable size metadata.");
    }
    file = await messenger.messages.getRaw(Number(params.messageId), { data_format: "File" });
  }
  if (file.size > limit) throw new Error(`Message exceeds MegaMail's ${limit / (1024 * 1024)} MiB transfer limit.`);
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (bytes.byteLength > limit) throw new Error("Thunderbird returned more data than the advertised transfer limit.");
  expireTransfers();
  if (downloads.size >= 2) throw new Error("Finish or cancel an existing message download first.");
  const outstandingBytes = [...downloads.values()].reduce((total, item) => total + item.bytes.byteLength, 0);
  if (outstandingBytes + file.size > MAX_MESSAGE_BYTES) throw new Error("Finish an existing download before starting another large transfer.");
  const transferId = token();
  downloads.set(transferId, { bytes, name, mimeType, expires: Date.now() + 120000 });
  return { transferId, size: bytes.byteLength, name, mimeType };
}

function base64FromBytes(bytes) {
  let binary = "";
  for (let index = 0; index < bytes.length; index += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(index, Math.min(index + 0x8000, bytes.length)));
  }
  return btoa(binary);
}

function bytesFromBase64(value) {
  const binary = atob(String(value));
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

function downloadChunk(params) {
  const item = downloads.get(String(params.transferId));
  if (!item || item.expires < Date.now()) {
    downloads.delete(String(params.transferId));
    throw new Error("This download expired. Start it again.");
  }
  const offset = Number(params.offset);
  const length = Math.min(Number(params.length || MAX_CHUNK_BYTES), MAX_CHUNK_BYTES);
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > item.bytes.byteLength || !Number.isSafeInteger(length) || length < 0) {
    throw new Error("Invalid attachment download range.");
  }
  const bytes = item.bytes.subarray(offset, Math.min(offset + length, item.bytes.byteLength));
  return { offset, data: base64FromBytes(bytes), done: offset + bytes.byteLength >= item.bytes.byteLength };
}

function beginUpload(params) {
  expireTransfers();
  const size = Number(params.size);
  const limit = params.kind === "attachment" ? MAX_ATTACHMENT_BYTES : 0;
  if (!Number.isSafeInteger(size) || size < 0 || size > limit) throw new Error("Upload exceeds MegaMail's transfer limit.");
  if (uploads.size >= MAX_UPLOADS || [...uploads.values()].reduce((total, item) => total + item.size, 0) + size > MAX_ATTACHMENT_BYTES) {
    throw new Error("Finish an existing upload before starting another attachment.");
  }
  const transferId = token();
  uploads.set(transferId, { size, received: 0, bytes: new Uint8Array(size), name: safeText(params.name), mimeType: safeText(params.mimeType), complete: false, expires: Date.now() + 120000 });
  return { transferId, size };
}

function uploadChunk(params) {
  const item = uploads.get(String(params.transferId));
  if (!item) throw new Error("Upload expired. Start it again.");
  const offset = Number(params.offset);
  const bytes = bytesFromBase64(params.data);
  if (offset !== item.received || bytes.byteLength > MAX_CHUNK_BYTES || offset + bytes.byteLength > item.size) {
    throw new Error("Invalid upload chunk sequence.");
  }
  item.bytes.set(bytes, offset);
  item.received += bytes.byteLength;
  item.expires = Date.now() + 120000;
  return { received: item.received, size: item.size };
}

function finishUpload(params) {
  const key = String(params.transferId);
  const item = uploads.get(key);
  if (!item || item.received !== item.size) throw new Error("Upload is incomplete.");
  item.complete = true;
  item.expires = Date.now() + 120000;
  return { transferId: key, name: item.name, mimeType: item.mimeType, size: item.size };
}

function consumeUploads(attachments) {
  return (attachments || []).map(attachment => {
    const key = String(attachment.transferId || "");
    const item = uploads.get(key);
    if (!item || !item.complete || item.received !== item.size) throw new Error("An attachment upload is incomplete.");
    uploads.delete(key);
    const name = safeText(attachment.name || item.name) || "attachment.bin";
    const type = safeText(attachment.mimeType || item.mimeType) || "application/octet-stream";
    return { file: new File([item.bytes], name, { type }), name };
  });
}

function composeDetails(params) {
  return {
    identityId: params.identityId ? String(params.identityId) : undefined,
    to: params.to || [],
    cc: params.cc || [],
    bcc: params.bcc || [],
    replyTo: params.replyTo ? (Array.isArray(params.replyTo) ? params.replyTo : [params.replyTo]) : [],
    subject: safeText(params.subject),
    body: safeText(params.body),
    plainTextBody: safeText(params.plainTextBody),
    isPlainText: Boolean(params.isPlainText),
    attachments: consumeUploads(params.attachments)
  };
}

async function dispatch(method, params) {
  expireTransfers();
  switch (method) {
    case "accounts": return allAccounts();
    case "folders": {
      const account = await messenger.accounts.get(String(params.accountId), false).catch(() => null);
      if (!account || account.type !== "imap") throw new Error("Thunderbird could not find the selected IMAP account.");
      const loaded = await loadAccount(account, params.refresh === true);
      if (loaded.folderError) throw new Error(loaded.folderError);
      return loaded.rootFolder;
    }
    case "list": return messageList(params);
    case "refresh": return refreshFolder(params);
    case "conversation": return conversationMessages(params);
    case "body": return messageBody(params);
    case "attachments": return attachmentList(params);
    case "raw_start": return startDownload({ ...params, kind: "raw" });
    case "attachment_start": return startDownload({ ...params, kind: "attachment" });
    case "download_chunk": return downloadChunk(params);
    case "download_end": downloads.delete(String(params.transferId)); return { ok: true };
    case "upload_begin": return beginUpload(params);
    case "upload_chunk": return uploadChunk(params);
    case "upload_finish": return finishUpload(params);
    case "upload_cancel": uploads.delete(String(params.transferId)); return { cancelled: true };
    case "shutdown":
      if (!messenger.megamailSync || !messenger.megamailSync.shutdown) throw new Error("Thunderbird shutdown API is unavailable.");
      setTimeout(() => messenger.megamailSync.shutdown().catch(() => {}), 0);
      return { stopping: true };
    case "update": {
      const update = {};
      if (typeof params.read === "boolean") update.read = params.read;
      if (typeof params.flagged === "boolean") update.flagged = params.flagged;
      await messenger.messages.update(Number(params.messageId), update);
      return { updated: true };
    }
    case "move": {
      const folder = await findFolder(params.folderId);
      await messenger.messages.move([Number(params.messageId)], folder);
      return { moved: true };
    }
    case "delete":
      await messenger.messages.delete([Number(params.messageId)]);
      return { deleted: true };
    case "send": {
      const details = composeDetails(params);
      let tab;
      if (params.replyMessageId) {
        const replyType = ["replyToSender", "replyToAll", "replyToList"].includes(params.replyType) ? params.replyType : "replyToSender";
        tab = await messenger.compose.beginReply(Number(params.replyMessageId), replyType, details);
      } else {
        // Thunderbird 157 does not expose messages.sendMessage at runtime.
        tab = await messenger.compose.beginNew(null, details);
      }
      let result;
      try {
        result = await messenger.compose.sendMessage(tab.id, { mode: "sendNow" });
      } finally {
        if (tab && Number.isInteger(tab.id)) await messenger.tabs.remove(tab.id).catch(() => {});
      }
      if (!result || !["sendNow", "sendLater"].includes(result.mode)) {
        throw new Error("Thunderbird returned an unclear send result. Check Sent and Outbox before retrying.");
      }
      return { mode: result.mode, headerMessageId: result.headerMessageId || (result.messages && result.messages[0] ? result.messages[0].headerMessageId : null) };
    }
    case "save": {
      const details = composeDetails(params);
      let result;
      if (params.replyMessageId) {
        const replyType = ["replyToSender", "replyToAll", "replyToList"].includes(params.replyType) ? params.replyType : "replyToSender";
        const tab = await messenger.compose.beginReply(Number(params.replyMessageId), replyType, details);
        try {
          result = await messenger.compose.saveMessage(tab.id, { mode: "draft" });
        } finally {
          if (tab && Number.isInteger(tab.id)) await messenger.tabs.remove(tab.id).catch(() => {});
        }
      } else {
        result = await messenger.messages.saveMessage(details, { mode: "draft" });
      }
      return { saved: true, messages: (result && result.messages || []).map(messageJson) };
    }
    default: throw new Error("Unsupported Thunderbird bridge operation.");
  }
}

function connectHost() {
  port = messenger.runtime.connectNative(HOST_NAME);
  port.onMessage.addListener(async request => {
    if (request && request.id === 0) return;
    try {
      const result = await dispatch(String(request.method), request.params || {});
      if (JSON.stringify(result).length > 2 * 1024 * 1024) throw new Error("Thunderbird response exceeds MegaMail's safe transfer limit.");
      port.postMessage({ id: request.id, result });
    } catch (error) {
      port.postMessage({ id: request && request.id, error: error instanceof Error ? error.message : "Thunderbird operation failed." });
    }
  });
  port.onDisconnect.addListener(() => {
    port = null;
    if (messenger.megamailSync && messenger.megamailSync.shutdown) {
      messenger.megamailSync.shutdown().catch(() => {});
    }
  });
  port.postMessage({ id: 0, result: { ready: true, version: messenger.runtime.getManifest().version } });
}

connectHost();
