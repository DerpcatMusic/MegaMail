#!/usr/bin/env python3
"""Injected-stall test of the REAL Thunderbird extension dispatch seam.

No Thunderbird, profile, provider, network or Cargo access. Node runs the checked-in
background.js in a VM with fictional messenger methods. This is NOT a native
adapter, Gecko or click-to-paint benchmark. stdout is JSON; exit 1 means the exact
selected body missed the deadline. --dispatch serial is a negative control,
not a claim about the production Rust actor. Python/Node standard libraries only.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

JS = r"""
const fs = require("node:fs");
const vm = require("node:vm");
const crypto = require("node:crypto");
const {performance} = require("node:perf_hooks");
const cfg = JSON.parse(process.argv[1]);
const source = fs.readFileSync(cfg.source, "utf8");
const hash = crypto.createHash("sha256").update(source).digest("hex");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const percentile = (values, q) => [...values].sort((a,b)=>a-b)[Math.ceil(values.length*q)-1];
const results = [];
(async () => {
  for (let sample = 0; sample < cfg.samples; sample++) {
    let listener;
    let selectedReply;
    let calls = [];
    let events = [];
    let selectedStarted;
    const t0 = performance.now();
    let startedSlow;
    const slowStarted = new Promise(resolve => { startedSlow = resolve; });
    const record = (event, id) => events.push({event, id, ms: +(performance.now()-t0).toFixed(3)});
    async function operation(name, id, value) {
      const call = {name, id, started_ms: +(performance.now()-t0).toFixed(3)};
      calls.push(call);
      const operationStarted = performance.now();
      const slow = (cfg.scenario === "unrelated" && name === "getFull" && id === 1)
        || (cfg.scenario === "headers" && name === "getHeaders" && id === 42)
        || (cfg.scenario === "attachments" && name === "listAttachments" && id === 42);
      if (slow) {
        record("stall_started", id);
        startedSlow();
        await sleep(cfg.stall_ms);
        record("stall_released", id);
      } else if (name === "getFull") await sleep(5);
      call.finished_ms = +(performance.now()-t0).toFixed(3);
      call.duration_ms = +(performance.now()-operationStarted).toFixed(3);
      return value;
    }
    const port = {
      onMessage: {addListener: fn => {listener = fn;}},
      onDisconnect: {addListener: () => {}},
      postMessage: reply => {
        if (reply.id === 42) {
          selectedReply = reply;
          record("selected_reply", 42);
        }
      }
    };
    // TB 153 getFull returns a virtual root. Non-content headers live there,
    // not on its multipart child. Values below are already RFC 2047 decoded by TB.
    const headers = {
      "reply-to": ['"Hörst, Kenny" <reply@example.test>'],
      "references": ["<root@example.test> <parent@example.test>", "<ancestor@example.test>"],
      "in-reply-to": ["<parent@example.test>"]
    };
    const selectedHtml = '<html><head><style>.mail{color:#203050}</style></head><body><table role="presentation"><tr><td><h1>Selected synthetic body 42</h1><img src="cid:logo" alt="Logo"><a href="https://tracking.example.test/42?opaque=abc">Read article</a></td></tr></table></body></html>';
    const messenger = {
      runtime: {connectNative: () => port, getManifest: () => ({version: "fixture"})},
      messages: {
        get: id => operation("get", id, {id, size: 1024}),
        getFull: (id, options) => {
          if (options?.decodeContent !== true) throw Error("Body decoding not requested");
          return operation("getFull", id, {
            contentType: "message/rfc822", partName: "", decryptionStatus: "none", headers,
            parts: [{contentType: "multipart/alternative", partName: "1",
              headers: {"content-type": ["multipart/alternative"]}, parts: [
                {contentType: "text/plain", partName: "1.1", body: "Selected synthetic body " + id},
                {contentType: "text/html", partName: "1.2", body: selectedHtml.replaceAll("42", String(id))}
              ]}]
          });
        },
        getHeaders: id => operation("getHeaders", id, headers),
        listAttachments: id => operation("listAttachments", id, [])
      }
    };
    // No require, fetch, sockets, filesystem or process is exposed to the extension.
    vm.runInNewContext(source, {messenger, TextEncoder, TextDecoder, console},
      {filename: cfg.source, timeout: 1000});
    if (typeof listener !== "function") throw Error("Extension listener missing");
    let unrelated;
    if (cfg.scenario === "unrelated") {
      unrelated = listener({id:1, method:"body", params:{messageId:1}});
      await slowStarted; // Selected request starts while unrelated work is already stalled.
    }
    selectedStarted = performance.now();
    record("selected_submit", 42);
    if (cfg.dispatch === "serial" && unrelated) await unrelated;
    const selected = listener({id:42, method:"body", params:{messageId:42}});
    await selected;
    const elapsed = performance.now()-selectedStarted;
    if (unrelated) await unrelated;
    const exact = selectedReply && !selectedReply.error
      && selectedReply.result.plainText === "Selected synthetic body 42"
      && selectedReply.result.html === selectedHtml;
    const replyMetadata = selectedReply && !selectedReply.error
      && selectedReply.result.replyTo === headers["reply-to"][0]
      && selectedReply.result.references === headers.references.join(" ")
      && selectedReply.result.inReplyTo === headers["in-reply-to"][0]
      && selectedReply.result.hasAttachment === false && selectedReply.result.truncated === false;
    const selectedCalls = calls.filter(c=>c.id===42);
    const lastDependency = Math.max(...selectedCalls.map(c=>c.finished_ms));
    const replyTime = events.find(e=>e.event==="selected_reply")?.ms;
    const unrelatedRelease = events.find(e=>e.event==="stall_released" && e.id===1)?.ms;
    const independent = cfg.scenario !== "unrelated" || replyTime < unrelatedRelease;
    results.push({
      selected_ms: +elapsed.toFixed(3), passed: !!exact && !!replyMetadata && independent && elapsed < cfg.deadline_ms,
      exact_selected_body: !!exact, reply_metadata_correct: !!replyMetadata,
      before_unrelated_release: cfg.scenario === "unrelated" ? independent : null, calls, events,
      selected_stage_ms: Object.fromEntries(selectedCalls.map(c=>[c.name, c.duration_ms]).concat([
        ["after_dependencies_to_reply", replyTime == null ? null : +(replyTime-lastDependency).toFixed(3)]
      ])),
      selected_getFull_calls: selectedCalls.filter(c=>c.name==="getFull").length,
      selected_getHeaders_calls: selectedCalls.filter(c=>c.name==="getHeaders").length,
      selected_payload_utf8_bytes: selectedReply ? Buffer.byteLength(JSON.stringify(selectedReply)) : 0,
      error: selectedReply?.error || null
    });
  }
  const values = results.map(r=>r.selected_ms);
  console.log(JSON.stringify({
    seam: "production background.js with injected messenger stalls; not native/TB network",
    source_sha256: hash, scenario: cfg.scenario, dispatch: cfg.dispatch,
    deadline_ms: cfg.deadline_ms, stall_ms: cfg.stall_ms, samples: cfg.samples,
    p50_ms: percentile(values, .50), p95_ms: percentile(values, .95),
    passed: results.every(r=>r.passed), network_bytes: 0, network_round_trips: 0,
    results
  }));
  process.exitCode = results.every(r=>r.passed) ? 0 : 1;
})().catch(error => { console.error(error.stack); process.exitCode = 2; });
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", choices=("unrelated", "headers", "attachments"), default="unrelated")
    parser.add_argument("--dispatch", choices=("parallel", "serial"), default="parallel")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--stall-ms", type=int, default=300)
    parser.add_argument("--deadline-ms", type=int, default=100)
    args = parser.parse_args()
    if not 1 <= args.samples <= 100:
        parser.error("samples must be 1..100")
    if not 0 < args.deadline_ms < args.stall_ms <= 5000:
        parser.error("require 0 < deadline-ms < stall-ms <= 5000")
    source = Path(__file__).resolve().parent / "thunderbird-bridge/extension/background.js"
    config = dict(vars(args), source=str(source))
    try:
        child = subprocess.run(["node", "-e", JS, json.dumps(config)],
                               timeout=args.samples * (args.stall_ms / 1000 + 2) + 5,
                               check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        return 2
    return child.returncode


if __name__ == "__main__":
    sys.exit(main())
