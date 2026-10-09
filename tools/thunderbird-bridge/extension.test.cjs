// Exercise the shipped bridge protocol without a profile, credentials, or network.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
class TestFile {
  constructor(parts, name, options = {}) {
    this.bytes = Buffer.concat(parts.map(part => Buffer.from(part)));
    this.name = name; this.type = options.type; this.size = this.bytes.length;
  }
  async arrayBuffer() { return Uint8Array.from(this.bytes).buffer; }
}
const frames = [];
let rawReads = 0;
const context = vm.createContext({
  File: TestFile, TextEncoder, TextDecoder, Uint8Array, Date, console,
  btoa: value => Buffer.from(value, 'binary').toString('base64'),
  atob: value => Buffer.from(value, 'base64').toString('binary'),
  setTimeout, clearTimeout, setInterval: () => 0,
  messenger: {
    runtime: {
      getManifest: () => ({ version: 'test' }),
      connectNative: () => ({
        onMessage: { addListener() {} }, onDisconnect: { addListener() {} },
        postMessage: frame => frames.push(frame),
      }),
    },
    messages: {
      get: async () => ({ size: 8 }),
      getRaw: async (_id, options) => {
        rawReads++;
        assert.equal(options?.data_format, 'File');
        return new TestFile([Buffer.from([0, 255, 10, 13, 65])], 'sample.eml');
      },
    },
  },
});
vm.runInContext(fs.readFileSync(path.join(__dirname, 'extension/background.js'), 'utf8'), context);
const call = (method, params = {}) => { context.request = { method, params }; return vm.runInContext('dispatch(request.method, request.params)', context); };
(async () => {
  assert.equal(frames[0].result.ready, true);
  const bytes = Buffer.from([0, 255, 1, 13, 10, 65]);
  const upload = await call('upload_begin', { kind: 'attachment', size: bytes.length, name: 'sample.bin' });
  await assert.rejects(call('upload_chunk', { transferId: upload.transferId, offset: 1, data: bytes.toString('base64') }), /sequence/);
  await call('upload_chunk', { transferId: upload.transferId, offset: 0, data: bytes.toString('base64') });
  await call('upload_finish', { transferId: upload.transferId });
  context.composeParams = { to: 'friend@example.invalid', cc: 'copy@example.invalid', bcc: 'hidden@example.invalid', replyTo: 'reply@example.invalid', attachments: [{ transferId: upload.transferId, name: 'sample.bin' }] };
  const details = vm.runInContext('composeDetails(composeParams)', context);
  assert.equal(details.bcc, context.composeParams.bcc);
  assert.equal(Array.isArray(details.replyTo) ? details.replyTo[0] : details.replyTo, context.composeParams.replyTo);
  assert.deepEqual(details.attachments[0].file.bytes, bytes);
  await assert.rejects(call('upload_begin', { kind: 'attachment', size: 50 * 1024 * 1024 + 1 }), /limit/);
  const attachments = [];
  for (let index = 0; index < 20; index++) {
    const item = await call('upload_begin', { kind: 'attachment', size: bytes.length, name: `file-${index}.bin` });
    await call('upload_chunk', { transferId: item.transferId, offset: 0, data: bytes.toString('base64') });
    await call('upload_finish', { transferId: item.transferId });
    attachments.push({ transferId: item.transferId });
  }
  await assert.rejects(call('upload_begin', { kind: 'attachment', size: 0 }), /existing upload/);
  context.composeParams = { attachments };
  const multi = vm.runInContext('composeDetails(composeParams)', context);
  assert.equal(multi.attachments.length, 20);
  for (const attachment of multi.attachments) assert.deepEqual(attachment.file.bytes, bytes);

  const closedTabs = [];
  let nextTab = 20;
  context.messenger.tabs = { remove: async id => closedTabs.push(id) };
  context.messenger.compose = {
    beginNew: async (messageId, details) => { assert.equal(messageId, null); assert.equal(details.plainTextBody, 'Synthetic body'); return { id: nextTab++ }; },
    beginReply: async (messageId, type) => { assert.equal(messageId, 9); assert.equal(type, 'replyToSender'); return { id: nextTab++ }; },
    sendMessage: async (_id, options) => { assert.equal(options.mode, 'sendNow'); return { mode: 'sendNow' }; },
  };
  assert.equal((await call('send', { plainTextBody: 'Synthetic body', isPlainText: true })).mode, 'sendNow');
  assert.equal((await call('send', { replyMessageId: 9, plainTextBody: 'Synthetic body', isPlainText: true })).mode, 'sendNow');
  context.messenger.compose.sendMessage = async () => { throw new Error('Synthetic SMTP failure'); };
  await assert.rejects(call('send', { plainTextBody: 'Synthetic body', isPlainText: true }), /SMTP failure/);
  assert.deepEqual(closedTabs, [20, 21, 22], 'Close temporary compose tabs after both success and failure');
  const download = await call('raw_start', { messageId: 9 });
  const chunk = await call('download_chunk', { transferId: download.transferId, offset: 0, length: 512 * 1024 });
  assert.deepEqual(Buffer.from(chunk.data, 'base64'), Buffer.from([0, 255, 10, 13, 65]));
  assert.equal(chunk.done, true);
  await call('download_end', { transferId: download.transferId });
  await assert.rejects(call('download_chunk', { transferId: download.transferId, offset: 0 }), /expired/);
  const readsBefore = rawReads;
  context.messenger.messages.get = async () => ({ size: 100 * 1024 * 1024 + 1 });
  await assert.rejects(call('raw_start', { messageId: 9 }), /limit/);
  assert.equal(rawReads, readsBefore, 'Reject oversized raw messages before fetching their content');

  const folders = new Map([
    ['archive-id', { id: 'archive-id', accountId: 'synthetic-account', type: 'archive' }],
    ['archive-extra', { id: 'archive-extra', accountId: 'synthetic-account', type: 'archive' }],
    ['inbox-id', { id: 'inbox-id', accountId: 'synthetic-account', type: 'inbox' }],
    ['sent-id', { id: 'sent-id', accountId: 'synthetic-account', type: 'sent' }],
    ['other-account-folder', { id: 'other-account-folder', accountId: 'other-account', type: 'inbox' }],
  ]);
  const makeConversationMessage = message => ({
    ...message,
    folder: folders.get(message.folderId),
    recipients: [], ccList: [], date: new Date(1700000000000 + message.id),
    read: true, flagged: false, hasAttachments: false, size: 100, tags: [],
  });
  const messages = [
    { id: 100, folderId: 'archive-id', headerMessageId: '<parent@example.invalid>', subject: 'Roadmap project', author: 'A Person <a@example.invalid>' },
    { id: 101, folderId: 'archive-id', headerMessageId: '<root@example.invalid>', subject: 'Roadmap project', author: 'A Person <a@example.invalid>' },
    { id: 102, folderId: 'sent-id', headerMessageId: '<reply@example.invalid>', subject: 'Re: Roadmap project', author: 'Me <me@example.invalid>' },
    { id: 103, folderId: 'inbox-id', headerMessageId: '<followup@example.invalid>', subject: 'Re: Roadmap project', author: 'A Person <a@example.invalid>' },
    { id: 104, folderId: 'inbox-id', headerMessageId: '<unrelated@example.invalid>', subject: 'Roadmap project', author: 'Someone <someone@example.invalid>' },
  ].map(makeConversationMessage);
  const byHeaderId = new Map();
  for (const message of messages) {
    byHeaderId.set(message.headerMessageId, [...(byHeaderId.get(message.headerMessageId) || []), message]);
  }
  const addConversationMessage = source => {
    const message = makeConversationMessage(source);
    messages.push(message);
    byHeaderId.set(message.headerMessageId, [...(byHeaderId.get(message.headerMessageId) || []), message]);
    return message;
  };
  addConversationMessage({ id: 105, folderId: 'archive-extra', headerMessageId: '<collision@example.invalid>', subject: 'Collision thread', author: 'First <first@example.invalid>' });
  addConversationMessage({ id: 106, folderId: 'archive-extra', headerMessageId: '<collision@example.invalid>', subject: 'Collision thread', author: 'Second <second@example.invalid>' });
  for (let index = 0; index < 65; index++) {
    addConversationMessage({
      id: 200 + index,
      folderId: 'archive-extra',
      headerMessageId: `<seed-${index}@example.invalid>`,
      subject: 'Older archive conversation',
      author: `Sender ${index} <sender${index}@example.invalid>`,
    });
  }
  addConversationMessage({ id: 267, folderId: 'sent-id', headerMessageId: '<bulk-reply@example.invalid>', subject: 'Re: Older archive conversation', author: 'Me <me@example.invalid>' });
  addConversationMessage({ id: 107, folderId: 'inbox-id', headerMessageId: '<collision-reply@example.invalid>', subject: 'Re: Collision thread', author: 'Third <third@example.invalid>' });
  const conversationQueries = [];
  const headerReads = new Map();
  const abortedLists = [];
  let bodyReads = 0;
  let forcePartialSubjectQuery = false;
  let batchSeedMode = false;
  let includeAmbiguousCandidate = false;
  context.messenger.accounts = {
    get: async id => id === 'synthetic-account'
      ? { id, type: 'imap', identities: [{ email: 'me@example.invalid' }] }
      : null,
  };
  context.messenger.folders = { get: async id => folders.get(id) || null };
  context.messenger.messages.abortList = async id => abortedLists.push(id);
  context.messenger.messages.getFull = async () => { bodyReads++; throw new Error('Conversation lookup must not read bodies'); };
  context.messenger.messages.getHeaders = async id => {
    headerReads.set(id, (headerReads.get(id) || 0) + 1);
    const message = messages.find(item => item.id === id);
    if (message?.id === 101) return { References: ['<parent@example.invalid>'], 'In-Reply-To': ['<parent@example.invalid>'] };
    if (message?.id === 102) return { References: ['<parent@example.invalid> <root@example.invalid>'], 'In-Reply-To': ['<root@example.invalid>'] };
    if (message?.id === 103) return { References: ['<root@example.invalid> <reply@example.invalid>'], 'In-Reply-To': ['<reply@example.invalid>'] };
    if (message?.id === 267) return { References: ['<seed-64@example.invalid>'], 'In-Reply-To': ['<seed-64@example.invalid>'] };
    if (message?.id === 107) return { References: ['<collision@example.invalid>'], 'In-Reply-To': ['<collision@example.invalid>'] };
    return {};
  };
  context.messenger.messages.query = async query => {
    conversationQueries.push(query);
    assert.equal(query.accountId, 'synthetic-account');
    assert.ok(Array.isArray(query.folderId), 'Every conversation search is limited to explicit folders');
    assert.ok(query.folderId.length > 0 && query.folderId.length <= 8);
    assert.ok(query.messagesPerPage <= 129);
    assert.ok(!('fullText' in query), 'Conversation lookup must stay header-only');
    if (query.headerMessageId) {
      assert.deepEqual(Array.from(query.folderId), ['archive-id', 'inbox-id', 'sent-id', 'archive-extra']);
      return { messages: (byHeaderId.get(query.headerMessageId) || []).filter(message => query.folderId.includes(message.folder.id)) };
    }
    if (query.fromMe) return forcePartialSubjectQuery
      ? { id: 'synthetic-list', messages: [messages[2]] }
      : { messages: [batchSeedMode ? messages.find(message => message.id === 267) : messages[2]].filter(Boolean) };
    if (query.subject && query.folderId.includes('inbox-id')) return {
      messages: [messages[3], messages[4], ...(includeAmbiguousCandidate ? messages.filter(message => message.id === 107) : [])],
    };
    if (!query.subject) return { messages: [messages[0], messages[3], messages[4]] };
    return { messages: [] };
  };

  const conversationParams = {
    accountId: 'synthetic-account',
    folderIds: ['archive-id', 'inbox-id', 'sent-id', 'archive-extra'],
    ids: ['<root@example.invalid>'],
    includeReplies: true,
  };
  const conversation = await call('conversation', conversationParams);
  assert.deepEqual(Array.from(conversation.messages, message => message.headerMessageId).sort(), [
    '<followup@example.invalid>', '<parent@example.invalid>', '<reply@example.invalid>', '<root@example.invalid>',
  ]);
  assert.equal(conversation.partial, false);
  assert.equal(conversation.warning, null);
  assert.ok(conversationQueries.some(query => query.fromMe === true && query.folderId[0] === 'sent-id'));
  assert.equal(conversationQueries.filter(query => query.headerMessageId).length, 2);
  assert.equal(bodyReads, 0);
  const batchQueryOffset = conversationQueries.length;
  const batchConversation = await call('conversation', {
    ...conversationParams, seedIds: ['<root@example.invalid>'], includeReplies: false, includeBatchRelated: true,
  });
  assert.deepEqual(Array.from(batchConversation.messages, message => message.headerMessageId).sort(), [
    '<followup@example.invalid>', '<parent@example.invalid>', '<reply@example.invalid>', '<root@example.invalid>',
  ]);
  const batchQueries = conversationQueries.slice(batchQueryOffset);
  assert.equal(batchQueries.length, 4, 'A summary batch exact-looks-up one seed, then uses two scoped candidate queries and its parent');
  assert.equal(batchQueries.filter(query => query.headerMessageId).length, 2);
  assert.ok(batchQueries.filter(query => !query.headerMessageId).every(query => !query.subject));
  assert.ok(batchQueries.some(query => query.fromMe === true));
  assert.ok(batchQueries.every(query => query.messagesPerPage <= 65));
  assert.equal(bodyReads, 0);
  const readsAfterFirstLookup = [...headerReads.values()].reduce((sum, count) => sum + count, 0);
  await call('conversation', conversationParams);
  assert.equal([...headerReads.values()].reduce((sum, count) => sum + count, 0), readsAfterFirstLookup,
    'Parsed headers are reused from the bounded per-account cache');

  batchSeedMode = true;
  const manySeedIds = Array.from({ length: 65 }, (_, index) => `<seed-${index}@example.invalid>`);
  const manySeedConversation = await call('conversation', {
    ...conversationParams,
    ids: manySeedIds,
    seedIds: manySeedIds,
    includeReplies: false,
    includeBatchRelated: true,
  });
  assert.ok(manySeedConversation.messages.some(message => message.headerMessageId === '<seed-64@example.invalid>'),
    'The batch path exact-looks-up all seeds above the old 64-seed limit');
  assert.ok(manySeedConversation.messages.some(message => message.headerMessageId === '<bulk-reply@example.invalid>'),
    'The batched Sent scan can associate a reply to the oldest seed in a large batch');
  assert.equal(manySeedConversation.partial, false);
  const manySeedQueries = conversationQueries.slice(-67);
  assert.equal(manySeedQueries.filter(query => query.headerMessageId).length, 65);
  assert.equal(manySeedQueries.filter(query => !query.headerMessageId).length, 2);
  assert.ok(manySeedQueries.every(query => query.messagesPerPage <= 129));
  batchSeedMode = false;

  includeAmbiguousCandidate = true;
  const ambiguousConversation = await call('conversation', {
    ...conversationParams,
    ids: ['<collision@example.invalid>', '<root@example.invalid>'],
  });
  assert.equal(ambiguousConversation.messages.filter(message => message.headerMessageId === '<collision@example.invalid>').length, 2,
    'Distinct messages with a duplicated Message-ID remain available for core ambiguity handling');
  assert.ok(!ambiguousConversation.messages.some(message => message.headerMessageId === '<collision-reply@example.invalid>'),
    'A reply is not attached through an ambiguous Message-ID');
  includeAmbiguousCandidate = false;

  await assert.rejects(call('conversation', {
    ...conversationParams, folderIds: ['archive-id', 'other-account-folder'],
  }), /outside the selected account/);
  assert.equal(bodyReads, 0);

  forcePartialSubjectQuery = true;
  const partialConversation = await call('conversation', conversationParams);
  assert.equal(partialConversation.partial, true);
  assert.match(partialConversation.warning, /More messages may exist/);
  assert.ok(abortedLists.includes('synthetic-list'), 'Bounded results abort any continuation list');
  assert.equal(bodyReads, 0);

  folders.set('page-folder', { id: 'page-folder', accountId: 'synthetic-account', type: 'inbox' });
  context.messenger.megamailSync = { getNewMessages: async () => ({ synced: true }) };
  context.messenger.messages.list = async () => ({ id: 'retry-page-token-1', messages: [messages[0]] });
  let continueAttempts = 0;
  context.messenger.messages.continueList = async id => {
    assert.equal(id, 'retry-page-token-1');
    continueAttempts++;
    if (continueAttempts === 1) throw new Error('Synthetic transient continuation failure');
    return { messages: [messages[1]] };
  };
  const firstPage = await call('list', { folderId: 'page-folder', limit: 1 });
  await assert.rejects(call('list', { cursor: firstPage.cursor }), /transient continuation failure/);
  const retryPage = await call('list', { cursor: firstPage.cursor });
  assert.equal(continueAttempts, 2, 'A transient continuation failure leaves the same cursor retryable');
  assert.equal(retryPage.messages.length, 1);
  await assert.rejects(call('list', { cursor: firstPage.cursor }), /expired/,
    'A successful continuation consumes its old cursor');
  console.log('Thunderbird bridge protocol checks passed');
})().catch(error => { console.error(error.stack || error.message); process.exitCode = 1; });
