# Batch 2: standalone helper fixture pixels

**PASS:** actual compiled WebKit helper pixels at 900×700 and 500×700. **OBSERVED:** narrower text wrapping. **BLOCKED:** the authored ≤440px stacking breakpoint. The native original-formatting gate remains CLOSED. This is not inline GPUI, real-mail, or full sender-CSS/MIME-CID acceptance.

## Method and unchanged inputs

Read AGENTS, architecture/design context, helper README/main/policy/fixture_request/engine_probe, and `batch2-helper-engine-runtime.md`. Graft returned no matching nodes; Impeccable context ran. The red-capable outside-repository observer asserts exact fixture colors, table geometry, CID pixels, window ownership, and protocol tokens. No source, CLI, manifest, lockfile, or repository test-script edit/build/install occurred. All **575 Correction 5 receipt inputs** matched before and after both phases; additional README/design/package hashes were unchanged.

Used exact `fixture_request.request('authored')`: 1,184-byte fictional v1 JSON, original 69-byte static 1×1 PNG, displayed at authored 32×32 dimensions. No adaptation or additional raster was necessary. HTML hash: `39d35d34942bac6bbf94504cdaa908f63bfcf666fd02d4b0a67b09b4a6482939`; wire hash: `a56a83148e52ace1908ba0b41088ead5539cf0f6e5c96c4627b6fe6778f13f2f`.

Compiled `--policy-test` output preserves this fixture's style text, presentation table/classes, and image dimensions; CID becomes the registered `/part/0` capability. Source-root class/id/style/dir/lang preservation is source-inspected only: this fixture has no root attributes. The faithful contract is a **subset**: tree/attribute reconstruction removes active content; CSS is retained, not fully token-sanitized. Production CSP, extension, and containment remain unchanged. CSS CID/data images and full HTML5/sender fidelity are not established.

## Actual pixels and responsive observations

| Owned window | Exact panel rectangles, exclusive end coordinates | Visible behavior |
| --- | --- | --- |
| 900×700 | Left `(0,104)-(450,306)`; right `(450,104)-(900,306)` | Two equal-width panels; left paragraph two lines, right paragraph one |
| 500×700 | Left `(0,104)-(250,326)`; right `(250,104)-(500,326)` | Still two panels; left paragraph three lines, right paragraph two |

Pixel assertions find authored backgrounds `#e9f0ed` / `#f5f1e8`, foreground `#23332c`, and exactly **1,024 CID pixels** of decoded `#309070`, rectangle `(24,128)-(56,160)`, in both captures. Headings, fictional copy, and link text remain visible, without flattening the table. White backing remains below it. Confirmation captures are byte-identical to the initial captures.

At 430px requested width, GTK clamped the actual window to **455px**. Confirmation recorded `WM_NORMAL_HINTS` minimum **455×150**. Thus the fixture's `max-width:440px` block-layout rule survives policy transformation but its visual activation is BLOCKED by this helper's minimum width. No hints, source controls, or sandbox were weakened; that width was not retried.

## Ownership, load, and teardown

Fresh dedicated Xvfb displays `:36941` / `:36942`, TCP disabled, supplied only their owned sockets. No host display/Xauthority/profile/provider/GPU/bus grant was used. Compiled containment retained ephemeral homes, read-only package/runtime, private empty read-only `/sys`, dropped capabilities, and nested WebKit sandbox.

Window `2097168` reported inner PID **2**, mapped through owned NSpid chains `[2286183,2]` / `[2291111,2]`, with live PIDfds/start times verified before resize/capture. Namespace and mount receipts record outer/helper/nested WebProcess separation. READY plus matching heartbeats bracketed observations; pixel content confirms original view, not plain fallback.

Initial breakpoint assertion failure used pinned owned cancellation and reaped everything. Confirmation sent only WM_DELETE_WINDOW to the verified window: matching-token `closed`, exit zero, no helper descendants before forced cleanup. Both Xvfb processes were reaped and sockets/locks removed. Phases lasted 9.53/8.55 seconds, below the 60-second bound; these are not latency measurements.

## Commands and artifacts

```sh
PYTHONDONTWRITEBYTECODE=1 python3 /tmp/megamail-fixture-visual.yDfjoG/observe.py
PYTHONDONTWRITEBYTECODE=1 python3 /tmp/megamail-fixture-visual-confirm.mW9k1F/confirm.py
# Inside observers, on their own DISPLAY and verified window only:
xdotool windowsize --sync 2097168 900 700
xdotool windowsize --sync 2097168 500 700
import -display :36942 -window 2097168 <owned-capture.png>
```

Artifacts remain in those two private directories: scripts, fictional wire/PNG, policy output, captures, ownership/geometry/process/mount receipts, stdout, summaries, and `artifact-sha256.json`. Confirmation also contains the 575-file `source-receipt.json`.

```text
SHA-256 production helper
 a14120503370abe86b962d66ff88bc48a18fc32b8a59ed234f640d0b54d18db0
SHA-256 extension
 1c21b3d81abdfac75361f87fe3df5825eba32f7dfd254d996a3897fbfbff411f
SHA-256 wide-900.png
 ca5eea322cace1781545d8a9d987679d150446dfba01ef3d9d22b32be43aa0df
SHA-256 narrow-500.png
 8314a29a692c08b90beaca9aae8bdb9b2cfbb3fdab1c89224cddb4bb28b3d443
SHA-256 confirmation summary.json
 fa8546a880e0f37a84fa1b916339cdc9787379e68aec4145607e364596bc004e
SHA-256 confirmation artifact-sha256.json
 19c7bd7e7cba22cb5d6270c13a85502508c83fcaec514899d9473ed465e6b5d1
```

No links/buttons, printing, mailbox actions, clipboard, accessibility, or real MIME transport were exercised. No new network-trace claim is made; prior hostile-hook/no-AF_INET evidence remains in the separate engine report. Native release enablement is unchanged.
