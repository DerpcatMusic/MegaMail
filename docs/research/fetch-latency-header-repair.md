# Selected-body header repair: bounded measurement

Date: 2026-10-10. Base HEAD: `d4dbd2bba14f474a33870269cc15bd98376ff360`.
The working tree contains other owners' changes. This package changes only:

- `tools/thunderbird-bridge/extension/background.js`
- `tools/thunderbird-bridge/extension.test.cjs`
- `tools/megamail-latency-fixture.py`
- This new note

## Result and scope

The production extension no longer requests `getHeaders` when reading the selected body.
It reads decoded message headers from the `getFull` root instead.
A synthetic 300 ms stall in the removed operation no longer delays the exact selected body.
Five trials changed from a 300.855 ms median to a 5.463 ms median at this JavaScript boundary.
This is not a product latency percentile, a network benchmark, or a native first-paint measurement.

The body still waits for `listAttachments`. Its 300 ms stall remains red.
The injected serial-dispatch control also remains red.
The actual extension listener completes selected work before an unrelated stalled body request finishes.
This package does not establish that the whole user-visible latency problem is fixed.

No Rust, GPUI, direct mail engine, profile, credential, provider-mail, permissions, or rendering policy was changed.
No Cargo command, Thunderbird launch, real-mail access, commit, or push was performed.
Public Thunderbird documentation and tagged source were retrieved over HTTPS.
All executable mail inputs were fictional objects supplied to Node.

## Header contract and compatibility evidence

The bundled manifest requires Thunderbird 153.0 or newer.
The runtime requirement was not lowered.
The current first-party [messages API documentation](https://webextension-api.thunderbird.net/en/latest/messages.html#getfull-messageid-options) states:

- `getFull` includes message headers and MIME parts.
- `decodeHeaders` requests RFC 2047 decoded headers and defaults to true. The option was added in TB 133.
- [`MessagePart.headers`](https://webextension-api.thunderbird.net/en/latest/messages.html#messagepart) contains decoded header arrays when requested.
- [`getHeaders`](https://webextension-api.thunderbird.net/en/latest/messages.html#getheaders-messageid-options) is faster when only headers are needed. It was added in TB 147.

The implementation explicitly requests `{decodeContent: true, decodeHeaders: true}`.
It retains the existing default decryption policy.
Decoded headers were already the default, so this does not introduce a separate decoding pass.

Minimum-version evidence comes from the first-party `THUNDERBIRD_153_0_RELEASE` tag, not only moving latest documentation:

- [`mail/config/version.txt`](https://hg.mozilla.org/releases/comm-release/file/THUNDERBIRD_153_0_RELEASE/mail/config/version.txt) reports `153.0`.
- [`ext-messages.js::convertMessagePart`](https://hg.mozilla.org/releases/comm-release/file/THUNDERBIRD_153_0_RELEASE/mail/components/extensions/parent/ext-messages.js#l615) creates an outer `message/rfc822` root. It moves non-`content-*` headers to that root.
- [`convertHeaders`](https://hg.mozilla.org/releases/comm-release/file/THUNDERBIRD_153_0_RELEASE/mail/components/extensions/parent/ext-messages.js#l476) supplies both `getFull` and `getHeaders`. Mailbox headers use the address-aware parser, including quoting decoded names with commas. Other headers use the MIME decoder.
- [`getFull` and `getHeaders`](https://hg.mozilla.org/releases/comm-release/file/THUNDERBIRD_153_0_RELEASE/mail/components/extensions/parent/ext-messages.js#l1246) both request continuation stripping when decoded headers are requested.
- [`test_ext_messages_get.js`](https://hg.mozilla.org/releases/comm-release/file/THUNDERBIRD_153_0_RELEASE/mail/components/extensions/test/xpcshell/test_ext_messages_get.js#l429) expects decoded header arrays on the virtual root and content headers on its child. The upstream test includes an encoded mailbox name with a comma.

These upstream tests were inspected, not executed here.
The consumed fields are `reply-to`, `in-reply-to`, and `references`, not MIME content headers.
The existing `headerValue` projection remains unchanged: case-insensitive names, safe string filtering, space-joined values, and an 8192-character limit.
There is no homemade address or RFC 2047 decoder.

A valid empty root dictionary means that the three fields are absent and returns empty strings.
An absent, null, array, or non-object root dictionary now returns an explicit error:

```text
Thunderbird did not return decoded message headers. This runtime cannot safely provide reply metadata for the reader.
```

There is no fallback to `rawHeaders`, nested MIME headers, message-list metadata, or another header lookup.
This keeps an unsupported runtime from silently producing incomplete reply metadata.
Conversation-only `getHeaders` and its cache are unchanged.

This is equivalence of the consumed decoded fields, not equivalence of every header in both API responses.
`getHeaders` reads the original tree, while default `getFull` can read a decrypted tree.
No live encrypted-message or protected-header reproduction was executed.
The repair uses the decoded top-level headers of the same message tree as the displayed body.
Decryption defaults and encrypted-cache policy are not changed or newly certified by this package.

## Production-wired regression coverage

`node tools/thunderbird-bridge/extension.test.cjs` passed.
New body checks enter the shipped native-host listener and inspect its reply frame.
The size-budget checks also enter production `dispatch` to distinguish the body budget from the JSON transport budget.
The regression was executed before the repair and failed. The same test passed after the repair.

The tests cover:

- Exact plain text, sender HTML including CSS/table/CID/link markup, preview, and the existing response keys.
- Quoted decoded Unicode mailbox names, raw encoded values that must not replace decoded values, and explicit decoder configuration.
- Folded reference whitespace, multiple values, case-insensitive names, absent fields, safe non-string filtering, and the header cap.
- Virtual-root metadata, conflicting child headers, and missing decoded-root failure without silent fallback.
- Plain-only and HTML-only bodies, a missing readable body, the shared 2 MiB body budget, and truncation.
- Independent JSON response-size rejection. A body at its 2 MiB cap can still exceed the JSON transport cap.
- Unsafe source sizes rejected before content operations, and acceptance of the inclusive 20 MiB source cap.
- Attachment enumeration overriding stale `hasAttachments`, no MIME guessing, and attachment-list failures remaining failures.
- One `getFull`, no selected `getHeaders`, and unchanged conversation-only header tests.

The fixture now models Thunderbird's virtual root and decoded header arrays.
Every timed trial asserts exact plain text, byte-for-byte synthetic HTML, `replyTo`, `references`, `inReplyTo`, attachment state, and truncation state.
It also records operation start, finish, and duration.
For unrelated work, it asserts that the selected reply precedes the unrelated stall release.

The tests do not execute Thunderbird's own MIME parser or address decoder.
Decoder behavior is supported by tagged first-party source, with its API output represented by synthetic decoded values.
No HTML document is loaded by these tests, and no URL in the fictional HTML is fetched.

## Executed measurements

Environment: Node `v26.11.1`, Python `3.14.7`, Linux agent shell.
Measurements were taken around 12:36–12:38 UTC on 2026-10-10.
Each cohort uses five trials, a 300 ms injected stall, and a 100 ms completion gate.
`getFull` for the selected message otherwise sleeps for 5 ms.
All other API results are in-memory objects.

Both versions were executed with the same enhanced fixture.
The baseline source was frozen from HEAD and its hash matched the original audit.

```text
Before SHA-256: 24156e8077649bb7156464c12ce9c2c0f01f33653b73871a8ca67f9683b3c4eb
After SHA-256:  3193b9278074e24213b439fcec445fa92198ac8b06a7b173d110042eff60ec05
```

Selected completion times in milliseconds:

| Cohort | Five before trials | Before median / max | Five after trials | After median / max | Before → after |
| --- | --- | --- | --- | --- | --- |
| Selected extra-header stall | 301.452, 299.975, 300.855, 301.066, 300.615 | 300.855 / 301.452 | 5.989, 5.387, 6.642, 5.207, 5.463 | 5.463 / 6.642 | Red → green |
| Unrelated `getFull(1)` stall | 5.640, 6.004, 5.913, 5.707, 5.303 | 5.707 / 6.004 | 5.594, 5.053, 5.553, 5.834, 5.251 | 5.553 / 5.834 | Green → green |
| Selected attachment-list stall | 301.201, 301.325, 300.923, 300.307, 300.771 | 300.923 / 301.325 | 301.290, 300.695, 300.892, 301.133, 300.955 | 300.955 / 301.290 | Red → red |
| Injected serial host, unrelated stall | 306.372, 305.838, 306.004, 306.138, 306.379 | 306.138 / 306.379 | 306.566, 307.428, 306.072, 305.472, 306.081 | 306.081 / 307.428 | Red → red |

The fixture reports nearest-rank `p95_ms`. With five trials it is the maximum above, not a representative product tail percentile.
All 40 trials returned the correct selected content and reply metadata, including deliberately red latency controls.
After repair, every selected trial called `getFull` once and `getHeaders` zero times.
Before repair, each called both once.
Both versions returned 589 UTF-8 bytes of selected JSON in this enhanced fixture.

Selected API stages in milliseconds, shown as median / maximum:

| Cohort / version | `get` | `getFull` | `getHeaders` | `listAttachments` | Last dependency → reply |
| --- | --- | --- | --- | --- | --- |
| Headers / before | .001 / .003 | 5.568 / 6.001 | 300.497 / 300.847 | .001 / .003 | .142 / .353 |
| Headers / after | .001 / .005 | 5.256 / 6.466 | Not called | .001 / .005 | .092 / .265 |
| Unrelated / before | .001 / .001 | 5.583 / 5.872 | .001 / .004 | .001 / .001 | .104 / .311 |
| Unrelated / after | .001 / .001 | 5.270 / 5.727 | Not called | .001 / .003 | .095 / .294 |
| Attachments / before | .001 / .004 | 5.301 / 5.698 | .001 / .003 | 300.551 / 301.102 | .137 / .379 |
| Attachments / after | .001 / .005 | 5.675 / 6.339 | Not called | 300.662 / 300.908 | .143 / .366 |
| Serial / before | .001 / .002 | 5.419 / 5.462 | .001 / .001 | .000 / .001 | .076 / .094 |
| Serial / after | .001 / .004 | 5.368 / 6.066 | Not called | .001 / .003 | .071 / .083 |

API stages overlap inside `Promise.all`. Do not add their percentiles to form a total.
The serial control includes time before selected dispatch, which is outside those selected API durations.
The last stage includes production body collection, metadata projection, preview, and JSON reply preparation for this tiny body.
Sub-millisecond values are instrumentation observations, not CPU microbenchmark claims.
Network bytes and network round trips are zero.
API calls do not establish Thunderbird downloads or IMAP command counts.

## Commands and replay

From repository root, these commands execute the current production source:

```sh
node tools/thunderbird-bridge/extension.test.cjs
python3 tools/megamail-latency-fixture.py --scenario headers --samples 5
python3 tools/megamail-latency-fixture.py --scenario unrelated --samples 5
python3 tools/megamail-latency-fixture.py --scenario attachments --samples 5
python3 tools/megamail-latency-fixture.py --dispatch serial --samples 5
```

The first three commands succeed. The final two intentionally exit 1.
The measurement runner used the fixture's embedded JavaScript against both frozen sources:

```sh
git show HEAD:tools/thunderbird-bridge/extension/background.js > /tmp/megamail-header-baseline.js
python3 - <<'PY'
import hashlib, json, pathlib, runpy, subprocess
fixture = runpy.run_path('tools/megamail-latency-fixture.py')
baseline = pathlib.Path('/tmp/megamail-header-baseline.js')
assert hashlib.sha256(baseline.read_bytes()).hexdigest() == '24156e8077649bb7156464c12ce9c2c0f01f33653b73871a8ca67f9683b3c4eb'
for version, source in [('before', baseline), ('after', pathlib.Path('tools/thunderbird-bridge/extension/background.js'))]:
    for scenario, dispatch in [('headers', 'parallel'), ('unrelated', 'parallel'), ('attachments', 'parallel'), ('unrelated', 'serial')]:
        cfg = dict(source=str(source.resolve()), scenario=scenario, dispatch=dispatch,
                   samples=5, stall_ms=300, deadline_ms=100)
        child = subprocess.run(['node', '-e', fixture['JS'], json.dumps(cfg)],
                               capture_output=True, text=True, timeout=20)
        data = json.loads(child.stdout)
        green = dispatch == 'parallel' and (scenario == 'unrelated' or version == 'after' and scenario == 'headers')
        assert child.returncode == (0 if green else 1)
        assert all(r['exact_selected_body'] and r['reply_metadata_correct'] for r in data['results'])
        if version == 'after':
            assert all(r['selected_getFull_calls'] == 1 and r['selected_getHeaders_calls'] == 0 for r in data['results'])
        print(version, scenario, dispatch, child.returncode, child.stdout.strip())
PY
```

This replay assumes HEAD still contains the baseline. Its hash assertion detects a changed HEAD.
The full local measurement output was saved at `/tmp/megamail-header-repair-measurements.json`.
The tables preserve the measured trials and stage summaries without depending on that temporary file.

Documentation lookup used the find-docs workflow:

```sh
npx ctx7@latest library Thunderbird "messages.getFull decodeHeaders MessagePart.headers version guarantees"
npx ctx7@latest docs /thunderbird/webext-docs "messages.getFull options decodeHeaders defaults true version MessagePart headers top-level rawHeaders getHeaders equivalence"
```

Latest documentation and TB 153 tagged source were then read directly from first-party URLs above.

## Residual limits

Attachment listing needs a separate metadata-completeness policy before it can leave the selected-body path.
This package does not return false no-attachment data to gain speed.
The exact body also still waits for `messages.get` and `getFull`.
Slow selected downloads, MIME parsing, decryption, and large body processing remain possible.

The native actor, request admission, account/session generations, stale suppression, cache writes, sanitization, and UI painting are outside this measurement.
The fixture's serial mode is an injected negative control, not an observation of the production actor.
The repair preserves sender HTML at the bridge boundary but does not implement a faithful native HTML/CSS renderer or CID byte delivery.
Remote-resource and document-isolation privileges remain unchanged.
Both direct and Thunderbird engines remain in place.

A complete product claim needs instrumented native/direct/TB runs with defined cold and warm cohorts, actual network counters, and first-usable-paint timestamps.
The earlier parent test totals, 471 core passes with 19 ignored and 103 native passes with a private probe ignored, are not revalidated here.
