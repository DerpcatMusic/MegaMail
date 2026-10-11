# Actual reader clipboard: parent-controlled search test

## Named results

Correction5 normal binary b60904077a2e0d2dadb7b7e76f9c6a8b6d55ee0d4e4d521de6a54ba9e3cd3636, source575 unchanged during these tests. No compiler/source/private profile/provider/keyring/mail/install/host-display/host-clipboard operation.

- Startup (body visible): **FAIL**, actual FocusReader/SelectAll/Copy yields CLIPBOARD owner0, no text/oracles.
- Startup Down/Up: no selection change.
- Search positive control: **PASS observed**, permitted click within fixed owned Search field300,108; typed mm-clip-probe-260 visibly appears with 0 shown/No matches. Clearing restores list. This rules out blanket XTEST/input translation failure for this configuration.
- After Search receives internal focus, CtrlAltL/Down selects Priya (UID1090), updated conversation/last sender header observed.
- Then actual CtrlAltR/CtrlA/CtrlC: **PASS**, private Xlib reader returns UTF8_STRING, clipboardowner4194304,185bytes, both independent body-only Priya paragraphs match.
- CtrlAltL/Up then actual reader SelectAll/Copy: **PASS**,1021UTF8bytes with both RTL body paragraphs matching. No search sentinel/header-only data satisfies either oracle.
- This demonstrates actual retained TextView consumer+Copy after internal focus and across selection changes, NOT startup keyboard acceptance, source-generation acceptance, screen-reader/physical latency or helper clipboard.

## Controlled phases and timing

Round2 child review gates consumed bounded phases before input control. Parent phase3 ran identical external observer with110s manual-review allowance (< original QA120cap), preserving all safety guards. Body/search capture approved; Search typed and Priya selected. Last body was below viewport fold; manual readiness gate could not establish full body pixels. Review delay expired; parent did not count this as copy failure. An initial CLI110 timeout argument was refused before app launch; that setup is excluded. Late control-only directory was removed.

Parent phase4 eliminated human/agent review waits, taking owned captures as **not_run observations** for post-run review. Fixed8s cold-start pause is not readiness/performance proof; actual clipboard oracles decide functional result. Geometry/PID/XID/namespace/source/binary checks and known Search-only click remain. No application state substitution, clipboard seeding, mock focus or handler replacement. No arbitrary pixels/URLs/actions. This gives an unattended red-capable comparison between startup and post-input-focus in the same actual process.

## Tight command / artifacts

```sh
/usr/bin/timeout --signal=TERM --kill-after=4s 85s /usr/bin/python3 -B \
 /tmp/megamail-parent-clipboard-phase4/qa-run.py --execute \
 --binary /mnt/Windows11/DEV_WORKSPACE/Toolchains/User/cargo-target/debug/megamail \
 --reviewed-source-sha256 9f6f87c592cbb9c0777c77322606f548dcfbc9e69ebe4c2cec9b6aac1def7f0d \
 --timeout 70 --settle 0.3 --output-dir NEW_ABSOLUTE_EVIDENCE_PATH
```

The wrapper returns observation completion0 even with baseline failure; evaluate startup_clipboard/priya_clipboard/return-rtl_clipboard individually in observations.json. All latter oracle comparisons are real clipboard readback, not inferred from exit0.

Artifacts /tmp/megamail-parent-clipboard-phase{3,4}/ include exact scripts, mechanical change, receipts, captures and JSON clipboard records. Phase4 own timeout launcher reaped; appPID5 and XvfbPID3 reaped; private namespace/HOME removed. Source report originals retained. Production startup still needs source repair to establish appropriate initial mailbox focus without stealing existing reader/input/composer/modal focus. No such fix compiled yet.
