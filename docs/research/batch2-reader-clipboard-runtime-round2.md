# Reader clipboard runtime, round 2: correction 5

## Verdict

**Startup journey FAIL; post-Search control BLOCKED. No confirmed native Copy-handler defect and no acceptance PASS.** The missing-utility blocker is removed: an actual Xlib consumer queried the owned private display. It found **CLIPBOARD owner=0**, empty text, and two body-oracle misses after actual FocusReader → SelectAll → Copy keys, with the RTL body visibly loaded.

| Stage | Result | Evidence |
|---|---|---|
| Owned production demo | PASS, narrow | Pinned normal binary `--demo`; PID 5, XID 2097153; 1280×800; fictional title and visible Leila RTL body. |
| Startup FocusReader → SelectAll → Copy | FAIL, exact journey | `startup-clipboard.json`: owner 0, empty text; both expected body paragraphs absent. Selection screenshot shows no whole-body highlight. Delivery does not prove dispatch. |
| Ctrl+Alt+L → Down → Up before click | FAIL, observable expectation | Leila remains selected; Priya UID 1090 not achieved. |
| Permitted Search click/type/clear | BLOCKED | Visual-review deadline expired before click. **No click or sentinel typing executed.** |
| Post-click navigation and reader Copy | BLOCKED | Never reached; cannot compare before/after internal Search focus. |
| Clipboard text conversion | BLOCKED, narrower | XOpenDisplay/hidden requestor/XGetSelectionOwner ran successfully. With no producer owner, TARGETS/UTF8_STRING/STRING conversion branches were not exercised. |

## Loop and limitations

Read repository AGENTS, architecture, DESIGN, QA plan and diagnosing-bugs. Graft query returned no indexed context; no rebuild. Consumer ABI comes directly from installed `/usr/include/X11/Xlib.h` and `X.h`: XSelectionEvent, XEvent's `long pad[24]`, XConvertSelection and XGetWindowProperty. Python ctypes uses public installed `libX11.so.6`; no installs or compilation.

External wrapper mounts only the exact consumer file read-only. Consumer requires private net/PID/IPC namespaces, matching owned app/Xvfb namespaces/executables, fixed fake HOME and allocated private DISPLAY. Requestor is never mapped/focused; never writes or claims CLIPBOARD. SelectionNotify conversion has a two-second deadline, 64-KiB cap, UTF8/STRING fallback, and refuses INCR/oversize properties. Each GUI capture/key checks PID and actual focused XID. No clipboard seed, fake Rust handler, mock focus or application-state injection.

Two phases only, each inner budget 70 seconds; parent wait 85; external timeout 86 plus three-second kill grace. Phase 1's stale initial loading screenshot was not approved: readiness BLOCKED, no keys. Phase 2 refreshed owned captures while awaiting review, obtained body-ready approval, then reproduced startup Copy failure. Its second review gate consumed the remaining budget before Search approval. A late approval recreated only an external control directory after teardown; that exact directory was removed. **Review timing is a harness limitation, not application latency.** This is a red-capable observed journey, not yet an unattended deterministic regression test.

## Diagnosis and next boundary

Frozen source has context-scoped mailbox/list bindings (`desktop_actions.rs:30–43`), tracked message focus (`main.rs:4409`), and guarded identity-matched actual TextView focus (`main.rs:7294–7312`). Startup constructs handles but that alone does not establish GPUI internal focus. X11 focus is not GPUI context; parent shell input is not an application positive control.

Rank remains: initial GPUI context unset; XTEST translation/processing failure; reader identity/readiness/Copy guard. Visible body removes initial-loading confound only. Search typing must be observed before attributing failure specifically to ReaderCopy. Parent needs a newly authorized bounded phase with faster review coordination; no third phase or Rust instrumentation was attempted. Stale-generation acceptance remains separate. Original HTML gate stays closed; Zeron unchanged; no accessibility/physical-latency claim.

## Integrity, command, hashes

All 575 correction-5 entries match before/after; source digest and binary unchanged. Both app/Xvfb launchers reaped; outer launchers absent; no matching runtime processes or working HOME/XDG roots remain. Only new repository write is this report.

Executed phase 2 (visual approval files required at named gates):

```sh
B=/tmp/megamail-reader-clipboard-round2
/usr/bin/timeout --signal=TERM --kill-after=3s 86s /usr/bin/python3 -B "$B/qa-run.py" --execute --binary /mnt/Windows11/DEV_WORKSPACE/Toolchains/User/cargo-target/debug/megamail --reviewed-source-sha256 9f6f87c592cbb9c0777c77322606f548dcfbc9e69ebe4c2cec9b6aac1def7f0d --timeout 70 --settle 0.3 --output-dir "$B/evidence-phase2"
```

SHA-256, artifacts relative to `$B`:

```text
binary b60904077a2e0d2dadb7b7e76f9c6a8b6d55ee0d4e4d521de6a54ba9e3cd3636
qa-run.py 0bd9249a18d4ce1f00790887becc6413bb8e0d8eb194b482d5652c29227757cf
clipboard_read.py b996751b4b5265caa53b48d2743a182aa49f4c24a61bf0ed382de2cfe39b3e21
mechanical.patch 0a6abf304005c4b9d787257683b8ed15e2abfac2d18d02e16b1710a5561be3a1
SHA256SUMS ad499f0ef10dd09558b4df4c56b5f047197085ce2daba7fc06646fd4d70c79fd
```

Manifest pins both phases, exact reconstructed `qa-phase1.py`/`phase1.patch`, screenshots, consumer output, observations and integrity receipts. No forbidden host display/clipboard/profile/provider/browser/device/mail operations, builds or source/test-script edits.
