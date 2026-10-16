# Stable native reader qualification · 2026-10-16

PR: [MegaMail #11](https://github.com/DerpcatMusic/MegaMail/pull/11). Native implementation revision: `42cc12534e43f37e530e8feb94172c6f8972ee95`; subsequent commits contain QA and documentation only. This receipt supersedes PR #10's automatic bitmap reader behavior, while preserving that release's other mail functionality.

## Verified behavior

The default reader remains selectable GPUI text. It retains bounded parsed native states, separates authored replies from disclosed history, removes explicitly hidden inline preheaders, and never automatically launches WebKit or swaps in a PNG. Original formatting is an explicit separate-window action. Both pane dividers are draggable; expanded and compact layouts retain separate widths during the session. Sender names/addresses are selectable outside message disclosure buttons. Details show actual authentication evidence or unavailability. Photos use explicitly chosen local files or configured own-account avatars, bounded to two background loads, with initials fallback.

The corrected isolated fictional Light/Dark packs contain 28 captures at 1280×800, 1060×640 and 1520×900. Actual clipboard conversion verifies multilingual body text, the full sender address and a newsletter sentence. Visible captures verify nested history expansion/restoration, divider movement, sidebar layout retention, local photo loading, honest absent authentication, and explicit original-window opening/closing. A fresh full independent review returned **SHIP**, with no material fixes.

The first test script queried the clipboard before the application processed Copy and clicked below the quote control. Those results were rejected. The corrected harness waits for the expected text and verifies the expanded/collapsed pixels; capture names alone are not acceptance evidence.

## Tests and build

- 319 native tests pass locally and in [Linux CI](https://github.com/DerpcatMusic/MegaMail/actions/runs/38896447638).
- The same CI passes 591 portable-core unit tests, 4 OAuth callback tests and 21 provider-auth tests (616 active; 22 prerequisite-dependent cases explicitly ignored), plus 25 theme tests.
- Locked native build, formatting, helper policy/ABI checks, Thunderbird bridge and installer syntax pass in CI. Local reader focus wiring (6) and QA import (2) contracts pass.
- Locked optimized release build passes (`38be7825a12e5ab3cce0f71bf9eb10a752e86278`). The source-identical release executable passes the actual body/address/newsletter clipboard expectations, explicit original-window and helper-shutdown assertions again without another screenshot pass.
- Installed atomically at `~/.local/bin/megamail`; verified SHA-256 `83ad2fbf6cc14429a54ebb172cc6558c9cc9cf6588e7208cea7774a6f3d55161` matches the retained release artifact. The previous executable is backed up, and the running instance was not restarted. Reopening the app loads the replacement.

Release artifact and machine-local install receipt: `/mnt/Windows11/DEV_WORKSPACE/PluginArtifacts/MegaMail/reader-release-38be782/receipt.json`. The existing qualified helper, policy extension, runtime and license bundle are unchanged; their installed hashes were verified against the previous qualification. This is a qualification for the current Linux host, not a cross-platform bundle claim.

Machine-local evidence is retained under `/mnt/Windows11/DEV_WORKSPACE/PluginArtifacts/MegaMail/reader-reviewed-{dark,light}/`. Each contains the fictional captures, observed clipboard values, executable/source hashes and isolated execution receipt. Source-identical development candidate: `native-reader-42cc125/megamail`. The full independent review and final executable/install receipt are retained with the release artifact.

## Qualification limits

No real messages were sent, provider flags changed, credentials copied or live account/draft altered for these checks. Xvfb software captures do not prove physical GPU latency, FPS or a numerical parser-speed improvement. Removing automatic WebKit work and reusing native states address the demonstrated regression; the HTML parser itself was not replaced. The photo-picker workflow and real-provider authentication were not independently exercised.

Original mode requires complete eligible source. Direct-worker MIME completeness is currently unattested, so those messages report unavailability rather than reconstructing an original. Broader MIME/CID/HTML fidelity is not established by fictional fixtures. Remote resources remain blocked; local photos do not enable a remote directory or contact synchronization. Pane widths are not persisted across application restart. The broader feature and provider-qualification backlog remains in the [comfort checklist](../MAIL_COMFORT_CHECKLIST.md).
