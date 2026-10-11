# Batch 2 reader clipboard: correction 5 runtime

## Verdict

**Copy acceptance remains BLOCKED. No native Copy pass or confirmed native-handler defect.**

| Journey | Result | Actual evidence |
|---|---|---|
| Isolated production demo launch | PASS, narrow | Normal pinned binary, `--demo`; unique window 2097153, namespace PID 5, Local demo title. Fictional RTL body subsequently visible. |
| Ctrl+Alt+L → Down → Up | FAIL, observation expectation only | Leila highlight/subject remained unchanged. Different-message journey was not achieved; this does not identify a production defect. |
| FocusReader → SelectAll | BLOCKED | XTEST keys delivered, but no visible whole-body selection or production-dispatch witness. |
| Copy at startup, after Down, after Up | BLOCKED for each | `/usr/bin/xclip` and `/usr/bin/xsel` are absent. No clipboard consumer ran; nonempty output, producer execution, and oracle matches are unverified. |

Read diagnosing-bugs, repository AGENTS, DESIGN, architecture, QA harness/plan, production startup, `refresh_reader_text`, FocusReader, `reader_session`, and real demo fixtures. Graft query returned no indexed context; no graph rebuild. Demo skips inventory/provider/recovery initialization and activation; workers receive no real account configuration.

## Mechanical harness change

Only an external private copy was instrumented: `/tmp/megamail-reader-clipboard-20261010-2006/qa-run.py`. Exact unified diff: `mechanical.patch` in that directory.

Changes: explicit repository ROOT on the parent side and `/` inside; PID/liveness verification, `windowfocus --sync`, then exact focused-XID verification before every key/capture; `xdotool key --clearmodifiers KEY` **without `--window`**; three reader-key sequences with intervening list Down/Up; remove unrelated resize/composer actions; record owned-child reaping; tighten inner maximum to 55 seconds. No Rust instrumentation, handler replacement, focus mock, clipboard seed, helper launch, or state substitution.

All binary trust/source-digest gates, namespace/mount/environment isolation, PID-owned window-only capture, bounded commands and original cleanup guards remain. Parent wait is 70 seconds; external timeout is 80 seconds with four-second kill grace, below 90 seconds.

First attempt stopped before keys because bare Xvfb had no initial focused window. Both children were reaped. Explicitly focusing the verified window before capture resolved that harness precondition. Second attempt exited zero in 10.4 seconds, produced 13 captures and delivered 13 keys. Combined runtime was approximately 12 seconds. Exit zero means observation completed, not feature acceptance.

## Diagnosis and oracle

Ranked unresolved explanations: (1) missing GPUI internal focus/dispatch context despite X11 focus; (2) input processing/translation failure; (3) reader readiness/action guard. Source has context-scoped bindings; X11 focus alone does not prove the GPUI focus handle. Initial captures were still loading, so the first FocusReader attempt has a readiness confound. Later navigation also failed after body population. XTEST alone did not resolve the dead-key observation. No confirmed clipboard-producer diagnosis is possible without handler/readback evidence.

Body-only oracles from `live.rs:4806` include “MegaMail leads with Latin, then עברית and العربية, while this paragraph stays right-to-left.” and “Latin-only RTL paragraph with inline_code keeps one paragraph context.” Intended Down target: Priya, UID 1090, `demo_long_thread_reply`; its body begins “One last pass: keyboard focus now stays with the conversation as you move between messages.” No claimed clipboard comparison occurred. Original-source completeness/calendar metadata remain unknown; original HTML gate remains closed.

## Integrity and cleanup

`sourceSnapshotUnchanged=true`: all 575 correction-5 receipt entries match before/after, as does the harness digest. Binary unchanged. App/Xvfb launchers reaped in both attempts; parent sandbox reaped; no matching runtime processes remain. Private working HOME/XDG directories removed. Only new repository deliverable is this report. No builds/tests, real profiles, host clipboard/display, URLs, pixel clicks, sends, provider probes, or style changes. Zeron preserved. No accessibility or physical-latency claim.

## Command and artifact hashes

```sh
B=/tmp/megamail-reader-clipboard-20261010-2006
/usr/bin/timeout --signal=TERM --kill-after=4s 80s /usr/bin/python3 -B "$B/qa-run.py" \
  --execute --binary /mnt/Windows11/DEV_WORKSPACE/Toolchains/User/cargo-target/debug/megamail \
  --reviewed-source-sha256 9f6f87c592cbb9c0777c77322606f548dcfbc9e69ebe4c2cec9b6aac1def7f0d \
  --timeout 55 --settle 0.5 --output-dir "$B/evidence-focused" > "$B/execution-focused.json"
```

SHA-256:

```text
binary       b60904077a2e0d2dadb7b7e76f9c6a8b6d55ee0d4e4d521de6a54ba9e3cd3636
source       9f6f87c592cbb9c0777c77322606f548dcfbc9e69ebe4c2cec9b6aac1def7f0d
freeze JSON  5114b47b2b80e4717eb6d9276d9efb2f7000b94c908c90fbde6e469e4539beb3
qa-run.py    2b773b1e723bd9c885a634312c9b37b86a77adccbccad3c73661ed2de23f9898
patch        eafc2ab87cbd88ba47a14bd644733d71dea3dce73093deb3deb52ce95de9f629
observations f850f817198838d58597bc1ec106c9da2813308d6d865a5405d2fd1690aa60a3
SHA256SUMS   58af081f17b5bebbbb3b6ec378252edc7292c8069cdbaff021d575ff6e6c95cf
```

Artifact paths are relative to `$B`: `evidence-focused/artifacts/*.png`, `evidence-focused/observations.json`, `postflight.json`, `SHA256SUMS`. The manifest pins every focused-run capture/log/receipt.
