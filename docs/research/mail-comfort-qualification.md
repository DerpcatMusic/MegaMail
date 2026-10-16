# Mail comfort qualification · 2026-10-16

Qualified native Rust revision: `e6c6ec6f98d72c0a455cb203cd1b1d1f6b6f15ae` (native executable reports this revision). The final acceptance commit also qualifies a separately compiled HTML-helper stability fix; native Rust source is unchanged. PR: [MegaMail #10](https://github.com/DerpcatMusic/MegaMail/pull/10), incorporating the baseline from #9.

## Tests and build

| Gate | Evidence |
| --- | --- |
| Portable core | 590 unit + 4 OAuth callback + 21 provider-auth integration tests pass (615 active); 22 prerequisite-dependent cases ignored explicitly. Includes real SQLite pre-limit sorting, tied sender keys, reverse order and saved unread-rank paging. |
| Native | 317 active tests pass; private Thunderbird account probe ignored. Includes role-based draft/sent/work inference, 1,500 participant identities in each theme against reader/selected/hover/avatar surfaces, durable lifecycle merge/reopening and existing send/recovery tests. |
| Formatting/build | Native `cargo fmt --check` and locked optimized Linux build pass. |
| Reader contracts | 15 source checks, 82 installed API symbols, 35 compiled C policy assertions pass. |
| Actual WebKit | Isolated responsive CSS, hidden preheader, bounded clipping, hostile/parser-confusion links and missing-policy fail-closed fixtures pass. The long-message clipping fixture repeats four independent engine launches. |
| Supporting contracts | Thunderbird bridge, installer syntax, activation packaging (6), reader focus wiring (6), queued cancellation (5), QA import (2) and verifier contracts (10) pass. |

Every local Cargo invocation used the machine-wide `kurv-cargo.lock`; final filesystem-sensitive test runs are serial. Scratch/output live on the development drive. GTK callers of shared structures were preserved in source; the legacy GTK application was not separately qualified.

## Native interaction evidence

Both Light and Dark were captured from the qualified optimized binary using the repository-owned `bwrap`/Xvfb harness, a fresh synthetic profile, no host credentials/bus/network and explicit fictional sending-disabled demo mode. Required viewports: 1280×800, 1060×640 and 1520×900. Captures visibly confirm Inbox/Sent/Drafts navigation, sender sorting, long mixed-direction message wrapping and composition.

Supplemental **actual mouse clicks** enter a fictional subject/body, enable Markdown and preview, browse Sent while retaining the composer, and resume the same text/preview. This proves the isolated UI path, not provider draft saving. Capture names alone and delivered keys were not accepted as evidence: earlier unchanged/clipped captures were rejected, focus restoration and owned-window placement were corrected, and the entire required matrix was recaptured.

- [Light thread](../preview/mail-comfort-light.png)
- [Dark thread](../preview/mail-comfort-dark.png)
- [Resumed formatted draft](../preview/mail-comfort-compose.png)

Independent finish reviewer: **ship at the five-fix scope**. Authorship/delivery, persistence wording, participant contrast, relevant documentation and compose-format footer were each scored resolved. The previously failing correspondent label measured approximately 5.74:1 in Light and 8.72:1 in Dark. This verdict does not claim a whole-product audit. Fresh documenter updated PRODUCT.md, DESIGN.md and the relevant token sidecar, then rechecked after the last source edit; unrelated preexisting schema drift was left intact.

The installed-package check initially caught an intermittent long-message clipping failure. A provisional positive `document-loaded` height could start a snapshot before explicit post-load layout, and the renderer accepted a stale allocation. The helper now always measures after load, waits for the target widget allocation, and publishes only the exact intended frame height, with bounded retries/fail-closed behavior. The complete engine suite and repeated long-message fixture pass on the rebuilt and installed package. Normal wide/narrow fixtures remain pixel-identical to the prior qualified helper.

## Installation and artifacts

The user installer installed the exact qualified binary at `~/.local/bin/megamail`, plus the isolated reader, runtime and license bundle. The installed binary hash matches the retained qualified build. The running prior instance was retained; reopening the app loads the replacement.

Machine-local receipts and backups remain under `/mnt/Windows11/DEV_WORKSPACE/PluginArtifacts/MegaMail/`: `qualified-e6c6ec6/receipt.json` (source digest, hashes and installed paths), `previous-20261016/`, `comfort-ship-light/`, `comfort-ship-dark/`, `comfort-reader-qa/` and `comfort-installed-reader-qa/`. Build/test/install logs live under `/mnt/Windows11/DEV_WORKSPACE/Research/megamail-assessment-2026-10-16/`. The bundled WebKit runtime came from signature-verified Arch/CachyOS packages; archive hashes and verification records are retained in its `receipt.json`. It is a qualified runtime for this Linux host, not a portable cross-distribution bundle claim.

## Boundaries

No real-provider test messages, flag changes or account edits were performed. Local lifecycle tests and immediate reply-card logic do not establish end-to-end delivery across providers. Session-local cards reconcile exact account/Message-ID; durable Outbox tickets and recovered drafts retain restart/retry authority. Waiting/Done reopening uses observed cached inbound dates; thread lookup and work filters retain explicit coverage bounds.

Provider-global search/history, every historical thread member, WYSIWYG editing, contact/calendar synchronization, cloud work-state sync, other operating systems and broader real-provider/reconnect/accessibility qualification remain explicit backlog in the [checklist](../MAIL_COMFORT_CHECKLIST.md). All local mail excludes Trash/Junk/Templates; body search covers downloaded unencrypted bodies. Server acceptance is not delivery, and uncertainty never grants automatic resend permission.
