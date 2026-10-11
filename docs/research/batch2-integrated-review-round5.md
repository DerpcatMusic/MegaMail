## Standards

**0 new confirmed hard breaches in the bounded R5 correction.**

HEAD remains the pinned baseline. Reviewed the working-tree changes and relevant untracked reader modules/checks—not the empty committed diff. All four supplied source hashes match.

`main.rs::retire_reader_session` refreshes retained native state outside rendering. Helper cancellation remains nonblocking (`reader_host.rs::ReaderHost::cancel`); helper validation/start remains background work. No new synchronous filesystem/network operation was found in the corrected lifecycle paths.

`tools/test-megamail-reader-focus-wiring.py` accurately labels its six checks as source-only. The corresponding verifier entry preserves that limitation; it neither claims GUI acceptance nor launches an app. **I inspected these checks but did not execute them.**

## Spec

**R4 P2 native-focus lifecycle regression: SOURCE-RESOLVED, not runtime-accepted. No new concrete defect found in the assigned scope.**

Source proof:

- **Immediate opening and asynchronous refusal:** `main.rs::open_original_formatting` calls `retire_reader_session` at both retirement sites. That method retires the helper, obtains the actual current `LiveMailbox` snapshot, and calls `refresh_reader_text` before notification.
- **Hidden view:** the event pump uses the same synchronizing retirement method.
- **Closed/Fallback/disconnected helper:** the pump records generation before `Session::poll`, then refreshes the actual current native target when generation changes (`main.rs:1147–1156`).
- **Live selection/source/epoch changes:** the remaining direct retirement in `on_live_change` precedes body-cache projection and the existing final refresh (`main.rs:1709,1840`). No body-version or source-generation restamping was introduced.
- **Consumer agreement:** selected rendering uses `TextView::new(&target.state)` after identity matching; `FocusReader` obtains that same state’s actual focus handle (`main.rs:5262–5270,7294–7310`). Plain/quote/collapse transitions retain explicit refreshes.
- **Generation exhaustion:** `Session::retire` uses checked addition and clears active scope. Native identity no longer rejects `exhausted`. At `u64::MAX`, retirement cannot change generation, so the polling comparison need not refresh an unchanged native identity; helper acceptance remains denied. Stale asynchronous starts are cancelled, and stale scoped events remain rejected.

**Remaining execution gate:** after the parent’s grouped build window, exercise actual default helper refusal, helper closure/fallback, and hidden-view return with a fictional loaded body; verify managed rendering, FocusReader, SelectAll and Copy. Source markers do not prove these interactions.

Previously declared provider, epoch, recovery, engine/CID, accessibility and performance gaps remain separate—not newly discovered bugs or accepted capabilities.

**Read-only review released. No edits, tests, builds, launches, provider operations or credential reads performed.**
