"use strict";

const HOST_NAME = "__HOST_NAME__";
const MAX_MESSAGE_BYTES = 100 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES = 50 * 1024 * 1024;
const MAX_UPLOADS = 20;
const MAX_CHUNK_BYTES = 512 * 1024;
const MAX_BODY_BYTES = 2 * 1024 * 1024;
const MAX_BODY_SOURCE_BYTES = 20 * 1024 * 1024;
const lists = new Map();
const downloads = new Map();
const uploads = new Map();
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
    id: String(account.id),
    name: safeText(account.name),
    type: safeText(account.type),
    identities: (account.identities || []).map(identity => ({
      id: String(identity.id),
      name: safeText(identity.name),
      email: safeText(identity.email)
    })),
    rootFolder: account.rootFolder ? folderJson(account.rootFolder) : null
  };
}

async function allAccounts() {
  const accounts = (await messenger.accounts.list(false)).filter(account => account.type === "imap");
  return Promise.all(accounts.map(loadAccount));
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

async function loadAccount(account) {
  const result = accountJson(account);
  try {
    await messenger.megamailSync.discoverFolders(String(account.id));
    const fresh = await messenger.accounts.get(String(account.id), false);
    const discovered = await messenger.folders.query({ accountId: String(account.id) });
    const subFolders = await messenger.folders.getSubFolders(fresh, true);
    result.rootFolder = fresh.rootFolder ? folderJson({
      ...fresh.rootFolder,
      subFolders
    }) : null;
    if (!discovered.length) {
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
  const key = Object.keys(headers || {}).find(item => item.toLowerCase() === name.toLowerCase());
  if (!key) return "";
  const values = Array.isArray(headers[key]) ? headers[key] : [headers[key]];
  return values.map(safeText).filter(Boolean).join(" ").slice(0, 8192);
}

async function messageList(params) {
  if (!params.cursor) {
    const folder = await findFolder(params.folderId);
    if (!messenger.megamailSync || !messenger.megamailSync.getNewMessages) {
      throw new Error("This Thunderbird version cannot synchronize mail through MegaMail.");
    }
    const status = await messenger.megamailSync.getNewMessages(String(folder.id));
    if (!status || status.synced !== true) {
      throw new Error("Thunderbird did not synchronize the selected folder.");
    }
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
  lists.delete(key);
  const list = await messenger.messages.continueList(entry.id);
  return listJson(list, entry.folderId);
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
      const loaded = await loadAccount(account);
      if (loaded.folderError) throw new Error(loaded.folderError);
      return loaded.rootFolder;
    }
    case "list": return messageList(params);
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
