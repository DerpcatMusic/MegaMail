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
  console.log('Thunderbird bridge protocol checks passed');
})().catch(error => { console.error(error.message); process.exitCode = 1; });
