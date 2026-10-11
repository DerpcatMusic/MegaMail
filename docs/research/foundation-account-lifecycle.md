# Foundation audit: account identity, startup, restore, and reconnect

## Scope and evidence limits

Static research against checkout `d4dbd2bba14f474a33870269cc15bd98376ff360`.
Read `AGENTS.md`, `PRODUCT.md`, `docs/MEGAMAIL_ARCHITECTURE.md`, native startup/live/Thunderbird code, and relevant design, diagnosis, Rust, and documentation-lookup guidance. Graft was queried first; it reported an empty/missing graph, so source inspection used `rg` and bounded file reads. No nested `CONTEXT.md` or ADR document was found in the inspected scope.

Only this research document was written. No Cargo invocation, app launch, personal profile/credential access, provider mail operation, implementation, or commit was performed. Public first-party documentation was fetched read-only. The parent owns the serial Cargo slot.

**The exact cause of the user's repeated-import experience is not established.** The diagnosis skill's executed-red-loop gate cannot be met under this research-only assignment. The defects below are source-confirmed control-flow/invariant failures, not an observed reproduction on the user's machine. Section 6 specifies a deterministic, synthetic regression that the implementation owner can make red. Do not describe it as already run or passed.

This is the prioritized foundation slice of the larger Thunderbird/Spark/Superhuman gap audit, not a claim of exhaustive competitor parity research. Existing validation reports are historical evidence, not new verification by this agent.

Status vocabulary:

- **Confirmed defect (source):** a concrete input/state violates an invariant through the inspected production call chain.
- **Suspected defect:** a realistic failure mode still needs a runtime trace or synthetic reproduction.
- **Already implemented, unverified here:** the code exists; this audit did not execute it.
- **New work:** absent capability or an intentional limitation, not necessarily a regression.

## 1. Executive result

Persistence is real, but it persists **successful Thunderbird source references**, not a complete durable account-session model. Startup, setup routing, account inventory, identity allocation, connection status, and retry policy are split between `main.rs`, `live.rs`, and `thunderbird_adapter.rs`.

The highest-value correction is to distinguish **configured account** from **currently usable runtime**. A temporary network/login/source/storage failure must not make a saved account look like a first-run import. Keep the Thunderbird backend available. Put startup/reconnect policy behind a deep account-session module; keep mail transport and UI presentation outside it.

Five source-proven foundation problems deserve attention before feature fanout:

1. Direct-account metadata failure prevents all Thunderbird restore, even when its separate manifest is valid.
2. A manifest-read failure becomes `false` for the startup screen decision: storage trouble is conflated with no saved account.
3. Direct and Thunderbird account IDs come from independent allocators that can collide.
4. Reconnect iterates all saved references and reports healthy active Thunderbird accounts as ID conflicts.
5. Newly selected Thunderbird accounts are persisted only after initial folders/list startup succeeds; unsuccessful selections have no durable account entry.

The first slice recommended below fixes startup inventory/recovery semantics and reconnect idempotence. Cross-backend ID allocation is a separate urgent dependency for enabling mixed-account additions safely; do not conceal it by renumbering cache identities.

## 2. What the code actually does

### 2.1 Native entry point and ownership

| Owner | Evidence | Responsibility today |
|---|---|---|
| `main()` | [`main.rs:7113–7173`](../../apps/megamail/src/main.rs#L7113) | Handles `--thunderbird-host` before GUI startup; normal mode takes `instance.lock`, then opens GPUI. Demo skips normal-account startup. |
| `MailApp` | [`main.rs:694–750`](../../apps/megamail/src/main.rs#L694), [`753–826`](../../apps/megamail/src/main.rs#L753) | Owns `Entity<LiveMailbox>`, subscriptions, direct `profiles`, setup form, current view, and multiple Thunderbird auto-open flags. Starts with an empty live mailbox and Setup view. |
| `LiveMailbox` | [`live.rs:372–438`](../../apps/megamail/src/live.rs#L372), [`442–569`](../../apps/megamail/src/live.rs#L442) | Owns account rows, request/page senders, event receiver task, query thread handle, connection/error sets, and mailbox presentation state. Direct saved profiles immediately get rows/workers. Thunderbird rows are added only from `StartedAccount`. |
| Thunderbird adapter | [`thunderbird_adapter.rs:68–109`](../../apps/megamail/src/thunderbird_adapter.rs#L68), [`849–1132`](../../apps/megamail/src/thunderbird_adapter.rs#L849) | Owns source manifest and process-global weak profile-actor registry. One actor owns each profile runtime and bindings. Request/page closures hold strong command senders. |
| Core bridge runtime | [`thunderbird_bridge.rs:94–278`](../../crates/mail-core/src/thunderbird_bridge.rs#L94), [`346–381`](../../crates/mail-core/src/thunderbird_bridge.rs#L346) | Owns cloned profile lock, child process, socket, reply routing, host registration and temporary directory. Shared `Arc` runtime; last-owner drop performs bounded shutdown/kill and removes temporary artifacts, not the persistent clone. |

An important already-fixed ownership pattern: `replace_live` is used for the direct-profile startup replacement, not for every account addition ([`main.rs:919–925`](../../apps/megamail/src/main.rs#L919), [`1000–1021`](../../apps/megamail/src/main.rs#L1000)). Successful later direct setup calls `live.add_profile`, preserving existing workers, pending sends and the retained event loop ([`main.rs:2438–2464`](../../apps/megamail/src/main.rs#L2438), [`live.rs:855–902`](../../apps/megamail/src/live.rs#L855)). Do not regress this into reconstructing the whole mailbox on Add Account.

### 2.2 Startup chain, including screen routing

```text
main() -> MailApp::new -> empty LiveMailbox, loading_profiles=true
    background: prune stale composer attachment dirs
                config::load_profiles()
                thunderbird_adapter::has_saved_accounts()
    UI continuation:
        direct profiles nonempty:
            replace_live(direct profiles)
            live.restore_thunderbird()
            clear Setup; show Mailbox
        direct profiles empty:
            live.restore_thunderbird()
            show Mailbox if saved_thunderbird.unwrap_or(false), otherwise Setup
        direct load error:
            startup_error; show Setup; DO NOT restore Thunderbird
```

Evidence: [`main.rs:904–952`](../../apps/megamail/src/main.rs#L904). The initial Setup is not necessarily displayed: render shows “Loading MegaMail profiles…” while `loading_profiles` is true ([`main.rs:6144–6159`](../../apps/megamail/src/main.rs#L6144)). Do not report a guaranteed first-run-form flash.

`load_profiles` reads MegaMail `accounts.toml`, validates/loads its separate `profile_ids.toml`, assigns missing stable direct IDs, and can write the ID mapping ([`config.rs:619–631`](../../crates/mail-core/src/config.rs#L619), [`745–825`](../../crates/mail-core/src/config.rs#L745)). It is not merely a read of direct profiles and it is not a Thunderbird importer.

`has_saved_accounts` reads and validates the complete Thunderbird manifest just to return a bool. `restore_saved` reads it again ([`thunderbird_adapter.rs:101–110`](../../apps/megamail/src/thunderbird_adapter.rs#L101), [`599–603`](../../apps/megamail/src/thunderbird_adapter.rs#L599)). `read_store` distinguishes NotFound from malformed/oversize/unsupported-version data; the UI loses that distinction with `unwrap_or(false)` ([`759–792`](../../apps/megamail/src/thunderbird_adapter.rs#L759)).

`restore_thunderbird` is explicitly outside `LiveMailbox::new`, to avoid connecting the initial mailbox that startup later replaces. It collects occupied IDs/emails, excluding IDs marked reconnectable, and spawns a named background restore thread ([`live.rs:733–779`](../../apps/megamail/src/live.rs#L733)). Its boolean means approximately “attempt requested,” not “saved accounts exist” or “spawn succeeded.” It returns true even after spawn error; loading flags are cleared in that error branch.

The restore completion updates loading/error/result fields, then installs only returned `StartedAccount`s; failed outcomes do not create account rows. Existing reconnectable accounts can replace their senders and reset pending runtime requests ([`live.rs:1666–1813`](../../apps/megamail/src/live.rs#L1666), [`1816–1864`](../../apps/megamail/src/live.rs#L1816)). Startup's auto-open observer leaves Setup only when there are live account rows ([`main.rs:1082–1105`](../../apps/megamail/src/main.rs#L1082)).

**Normal saved-only failure behavior matters:** with a valid nonempty manifest and no direct metadata error, startup selects Mailbox before restore succeeds. Source unavailability does not by itself force Setup in that branch. Mailbox has a “Reconnect saved accounts” button when `thunderbird_error` is present ([`main.rs:3458–3473`](../../apps/megamail/src/main.rs#L3458)). Thus “there is no persistence” and “every failed restore asks for import” are both too broad.

### 2.3 Explicit discovery, import and persistence

1. Setup's “Find mail accounts” calls `import_thunderbird_settings`, which runs the scanner on a background executor, and preselects usable addresses not already present in live rows ([`main.rs:2121–2177`](../../apps/megamail/src/main.rs#L2121), [`6577–6592`](../../apps/megamail/src/main.rs#L6577)).
2. “Import selected” groups selections by profile, invokes `live.import_thunderbird`, and sets UI auto-open flags ([`main.rs:2244–2290`](../../apps/megamail/src/main.rs#L2244)).
3. The live module rejects concurrent import/restore and supplies occupied IDs/emails plus active/reconnectable Thunderbird IDs ([`live.rs:783–843`](../../apps/megamail/src/live.rs#L783)).
4. `import_selected` reads the manifest, canonicalizes paths, reuses saved IDs or allocates above the maximum of current and saved Thunderbird IDs. It rejects duplicate addresses and active-ID conflicts ([`thunderbird_adapter.rs:376–506`](../../apps/megamail/src/thunderbird_adapter.rs#L376)).
5. It starts each profile. Only ready accounts are accumulated for persistence; source references are written atomically after startup ([`511–563`](../../apps/megamail/src/thunderbird_adapter.rs#L511)).
6. `write_store` delegates to `config::write_private_file`: same-directory temporary file, private mode, file sync and atomic rename; parent-directory sync is best-effort ([`829–846`](../../apps/megamail/src/thunderbird_adapter.rs#L829), [`config.rs:68–124`](../../crates/mail-core/src/config.rs#L68)).

**Actual manifest v1 fields:** profile `path`; account `source_id`, `app_id`. It has no persisted account email, display name, endpoint fingerprint, chosen clone generation, or login/recovery state ([`thunderbird_adapter.rs:68–85`](../../apps/megamail/src/thunderbird_adapter.rs#L68)). The architecture text's claim that both selected source profile and *email* are recorded is inaccurate ([architecture, Account setup](../MEGAMAIL_ARCHITECTURE.md#account-setup-and-current-mail-flow)). Stable IDs are present; an offline display inventory is not.

### 2.4 Restore is rediscovery, not independent clone reopen

`restore_saved` invokes `discover_profiles()` over both standard Linux roots, canonicalizes discoveries and matches each saved path/account key back to the live source ([`thunderbird_adapter.rs:604–688`](../../apps/megamail/src/thunderbird_adapter.rs#L604)). It does not open a saved clone by descriptor independently of the source. A saved path missing from `profiles.ini`, a missing/rejected `prefs.js`, or a removed/unsupported account can fail restore even if its private clone still exists.

The scanner reads every discovered profile, not just saved ones. Bounds are 32 profiles, 1 MiB prefs, 64 KiB profiles.ini. Missing/unresolved profiles can be skipped; several I/O/size/UTF-8 errors propagate and fail the entire discovery call ([`thunderbird.rs:88–168`](../../crates/mail-core/src/thunderbird.rs#L88), [`179–191`](../../crates/mail-core/src/thunderbird.rs#L179)). Public `discover_profiles_in(home)` is an existing synthetic-fixture seam. Its comment says explicit import only, but restore also calls the scanner; this is an intentional saved-reference read, not a fresh consent prompt.

Profile startup reuses an actor if present, otherwise starts a new isolated runtime, queries accounts, and prefetches each selected account's folders and first page concurrently. One profile's results are returned as a batch; profiles are processed serially ([`thunderbird_adapter.rs:849–949`](../../apps/megamail/src/thunderbird_adapter.rs#L849), [`1134–1183`](../../apps/megamail/src/thunderbird_adapter.rs#L1134), [`1256–1542`](../../apps/megamail/src/thunderbird_adapter.rs#L1256)). Only after folders and a valid page succeed is an account reported Connected. Slow/erroring accounts can delay the visible return of successful siblings in that profile. This is startup latency, not proof of missing saved metadata.

### 2.5 Private clone lifecycle

- Each source path gets a hashed root; metadata for `key4.db`, `logins.json`, `cert9.db` determines a credential-generation subdirectory. Same source stamp reuses a persistent clone ([`thunderbird_bridge.rs:534–577`](../../crates/mail-core/src/thunderbird_bridge.rs#L534), [`920–957`](../../crates/mail-core/src/thunderbird_bridge.rs#L920)). This is a metadata stamp, not a content hash or provider-token validity check.
- Startup validates Thunderbird 153+, acquires a per-source MegaMail lock, computes the source stamp and prepares the selected generation ([`104–145`](../../crates/mail-core/src/thunderbird_bridge.rs#L104), [`579–615`](../../crates/mail-core/src/thunderbird_bridge.rs#L579)).
- A missing/mismatched generation stamp requires the source to be unlocked and encrypted-store journals to be safe. Credential files are staged, bounded, checked against before/after stamps and installed; no Rust-side credential decryption ([`617–646`](../../crates/mail-core/src/thunderbird_bridge.rs#L617), [`792–957`](../../crates/mail-core/src/thunderbird_bridge.rs#L792)).
- Account preferences are still reread from the source on every start; allowed preferences go into `user.js`, with private mail paths, disabled background auto-send and a separate persistent Local Folders directory ([`647–747`](../../crates/mail-core/src/thunderbird_bridge.rs#L647), [`thunderbird.rs:584–660`](../../crates/mail-core/src/thunderbird.rs#L584)). The cloned configuration includes the discovered profile's IMAP accounts, not just the UI-selected subset; the adapter exposes the selected bindings.
- Socket, wrapper and native-host manifest are recreated for each process session. Headless Thunderbird starts against the private generation. Bridge startup has a 60-second accept limit; the handshake read has a 90-second timeout ([`147–260`](../../crates/mail-core/src/thunderbird_bridge.rs#L147)). Operation deadlines are separate: metadata 15 seconds, folders 90 seconds, refresh 165 seconds ([`40–43`](../../crates/mail-core/src/thunderbird_bridge.rs#L40), [`384–401`](../../crates/mail-core/src/thunderbird_bridge.rs#L384)).
- Shutdown removes temporary host/socket artifacts and releases the child/lock. It deliberately retains the clone. No old-generation pruning appears in this lifecycle ([`346–379`](../../crates/mail-core/src/thunderbird_bridge.rs#L346)). Never prune a generation's Outbox/drafts blindly.

“Thunderbird is open” is not universally a restore blocker: unchanged generation stamps avoid recopy; updated snapshots require closing the source. “Copy every launch” is also inaccurate.

## 3. Confirmed defects and realistic triggering states

These are source-confirmed, not runtime-reproduced here.

| ID / priority | Trigger and actual behavior | Required behavior / evidence |
|---|---|---|
| F1 / P0 | Malformed/unreadable direct `accounts.toml` or invalid direct ID map, plus valid saved Thunderbird manifest. `main` takes direct-load Err and never calls restore. View is Setup. | Independent backend load failures must not suppress good saved accounts. [`main.rs:919–951`](../../apps/megamail/src/main.rs#L919), [`config.rs:745–825`](../../crates/mail-core/src/config.rs#L745). This can reproduce the literal import-oriented screen despite saved Thunderbird references. |
| F2 / P0 | Direct inventory empty, Thunderbird manifest parse/version/permission error. `has_saved_accounts` is Err; `unwrap_or(false)` chooses Setup. Restore separately produces an error. | Distinguish Missing/Empty from Unreadable; show recovery, never silently classify as first run. [`main.rs:930–944`](../../apps/megamail/src/main.rs#L930), [`thunderbird_adapter.rs:759–792`](../../apps/megamail/src/thunderbird_adapter.rs#L759). Reimport itself still fails `read_store`; repeated import is not a repair. |
| F3 / P0 | Import Thunderbird first into otherwise empty app: TB ID 1. Add first direct account: direct ID allocator also assigns 1. Saving succeeds, then `add_profile` rejects the ID. On restart direct ID 1 is loaded and TB restore rejects its saved ID. The same collision can occur after later additions beyond the direct allocator's prior maximum. | One stable identity authority across backends, preserving existing cache IDs. [`config.rs:776–799`](../../crates/mail-core/src/config.rs#L776), [`thunderbird_adapter.rs:393–424`](../../apps/megamail/src/thunderbird_adapter.rs#L393), [`main.rs:2422–2469`](../../apps/megamail/src/main.rs#L2422), [`live.rs:863–874`](../../apps/megamail/src/live.rs#L863), [`thunderbird_adapter.rs:651–660`](../../apps/megamail/src/thunderbird_adapter.rs#L651). Existing duplicate-ID rejection is correct defense, not an allocator fix. |
| F4 / P1 | One saved TB account is healthy; another failed/reconnectable account causes user to click reconnect. Live reserves healthy IDs; restore loops over **all** saved references and rejects those healthy IDs as conflicts. All-healthy reconnect also reports conflicts. | Idempotently skip the same healthy saved identity, retry only failed/not-started entries; still reject genuinely different owners. [`live.rs:745–760`](../../apps/megamail/src/live.rs#L745), [`thunderbird_adapter.rs:619–660`](../../apps/megamail/src/thunderbird_adapter.rs#L619). Explicit import already has an “already connected” case at [`437–446`](../../apps/megamail/src/thunderbird_adapter.rs#L437); restore does not. |
| F5 / P1 | User selects a new TB account; initial folders/list fails (offline/login/timeout). No `StartedAccount`, therefore no manifest entry, even if other selected accounts succeed. Next launch knows nothing about the failed selection. | Durable selected-account intent must survive transient connection failure; show saved/unavailable, not unadded. [`thunderbird_adapter.rs:511–563`](../../apps/megamail/src/thunderbird_adapter.rs#L511), [`1460–1496`](../../apps/megamail/src/thunderbird_adapter.rs#L1460). Current behavior is narrower than a durable account model; stage this as a later schema/persistence change. |
| F6 / P1 | Saved account never successfully starts this process. Live only installs successful `StartedAccount`s; reconnectable set is intersected with installed TB account IDs. Generic Refresh cannot trigger its restore and toolbar Refresh is hidden when no live rows exist. | Saved inventory must remain visible with a targeted Retry. Mailbox's separate reconnect button exists; do not claim there is no recovery at all. [`live.rs:1678–1717`](../../apps/megamail/src/live.rs#L1678), [`1134–1145`](../../apps/megamail/src/live.rs#L1134), [`4425–4432`](../../apps/megamail/src/live.rs#L4425), [`main.rs:6080–6096`](../../apps/megamail/src/main.rs#L6080), [`3458–3473`](../../apps/megamail/src/main.rs#L3458). |
| F7 / P1 | An unrelated discovered source profile has oversize/invalid UTF-8 prefs or a propagated I/O error. Global discovery fails before any saved-profile startup. | Restore saved profiles independently and limit scanning to saved sources; one broken unrelated profile must not block the others. [`thunderbird.rs:110–160`](../../crates/mail-core/src/thunderbird.rs#L110), [`179–191`](../../crates/mail-core/src/thunderbird.rs#L179), [`thunderbird_adapter.rs:604–606`](../../apps/megamail/src/thunderbird_adapter.rs#L604). |
| F8 / P2 | Source credential update hits the “updated encrypted credentials”/“credential store changed” safety error. Substring classification sees “credentials,” maps it to NeedsLogin and replaces the actionable close/retry reason with “Sign in …”. | Preserve typed source-busy/snapshot-changing vs provider auth/unlock failures. [`thunderbird_bridge.rs:625–640`](../../crates/mail-core/src/thunderbird_bridge.rs#L625), [`thunderbird_adapter.rs:3704–3754`](../../apps/megamail/src/thunderbird_adapter.rs#L3704). The UI's NeedsLogin message says “was not imported” even for restore ([`main.rs:6595–6601`](../../apps/megamail/src/main.rs#L6595)). |

Other concrete limitation: `import_selected` can finish runtime startup, then fail `write_store`; wrapper emits empty accounts/results plus an error. It does not report “usable but unsaved” as a distinct state ([`thunderbird_adapter.rs:313–338`](../../apps/megamail/src/thunderbird_adapter.rs#L313), [`560–562`](../../apps/megamail/src/thunderbird_adapter.rs#L560)). The next launch cannot recover a reference never durably saved. Do not label this a confirmed persistence failure on this machine without the write-result evidence.

## 4. Suspected causes requiring a probe

| Hypothesis | Falsifiable prediction / safe probe |
|---|---|
| Different installed binary or profile namespace on each launch | Startup trace shows different build identity/config-root fingerprint, or a successful import commit in a different namespace than next startup. Record build/git hash, executable fingerprint, and root fingerprints—not personal paths/content. The current entry point derives storage via XDG/dirs ([`config.rs:46–64`](../../crates/mail-core/src/config.rs#L46)); repository docs alone cannot prove what binary the user launches. |
| Saved source moved, removed from profiles.ini, or stale source account key | Synthetic same manifest with changed source discovery returns profile-unavailable/Unsupported. A stable clone does not bypass the source lookup ([`thunderbird_adapter.rs:630–688`](../../apps/megamail/src/thunderbird_adapter.rs#L630)). Need a saved descriptor and explicit relink, not repeated import. |
| Clone generation changes because source credential-file metadata changes, discarding a useful clone-side refreshed token | Source stamp A reuses generation A; metadata touch produces generation B even with equivalent contents. Inspect only synthetic stamps and generation-selection results. Runtime's refreshed private store is not copied back; the generation function does not validate token freshness ([`thunderbird_bridge.rs:534–555`](../../crates/mail-core/src/thunderbird_bridge.rs#L534), [`920–957`](../../crates/mail-core/src/thunderbird_bridge.rs#L920)). Do not “fix” by decoding private tokens or overwriting the source. |
| Account says Connected based on cached clone data before real remote auth succeeds | Existing cached Inbox lets `loadAccount` skip folder discovery; first `messageList` reads local messages; later refresh is the explicit network-sync path. Fake a cached folder/page and failing sync, assert Cached/Offline rather than online-authenticated. [`background.js:108–139`](../../tools/thunderbird-bridge/extension/background.js#L108), [`544–561`](../../tools/thunderbird-bridge/extension/background.js#L544), [`572–583`](../../tools/thunderbird-bridge/extension/background.js#L572). Standard accounts.list promises enumeration, not login ([official API](https://webextension-api.thunderbird.net/en/stable/accounts.html#list-includesubfolders)). |
| Late events from an old profile runtime cross reconnect | `WorkerEnvelope` identifies only account ID; page results have generations, but worker/account/folder/error events lack session epoch. Replay held old Error/Folders after a new Ready; assert new session remains healthy. [`live.rs:221–235`](../../apps/megamail/src/live.rs#L221), [`1816–1843`](../../apps/megamail/src/live.rs#L1816), [`2544–2561`](../../apps/megamail/src/live.rs#L2544). Retirement drops actor state but in-flight tasks can retain Runtime/command clones; likelihood needs execution, not a source-only assertion. |

Do not rank these above F1–F7 without runtime evidence. No personal-profile trace is necessary to build their synthetic tests.

## 5. Deep account-session module

### Interface and usage (design, not implementation)

Place the module at the native application's account-lifecycle seam: proposed `apps/megamail/src/account_session.rs`. It should not import GPUI. Its implementation can privately compose the existing core configuration, source scanner, direct workers and Thunderbird profile actors. The backends already genuinely vary; this is not a speculative pass-through abstraction.

```rust
// Illustrative types; settle exact ownership during implementation.
AccountSessions::start(paths, events) -> SessionHandle
SessionHandle::snapshot() -> Arc<AccountInventory>
SessionHandle::request(SessionCommand) -> Result<(), SubmitError>

// Commands, not five caller-owned sets and lifecycle flags:
SessionCommand::{Add(AccountSetup), Retry(AccountId), Remove(AccountId)}
```

`AccountInventory` distinguishes Loading, Empty, Ready and StorageRecovery. Each configured identity has a backend descriptor and connection phase: Dormant/Connecting/Cached/Online/Offline/NeedsLogin/SourceUnavailable/Failed. A storage problem is not Empty. “Online” requires backend evidence beyond enumerating an account; the cached state stays useful without claiming authentication.

Common use: main starts once, observes inventory, and chooses mailbox vs first-run/recovery from that inventory. Live receives session-tagged ready/error transitions and owns mailbox/reader state. UI Retry gives one stable account ID; callers need not read sources, reserve IDs, know clone generations, or infer restartability from English errors. Setup is an explicit Add action, not a fallback for every failed connection.

**Hidden complexity earning depth:** independent store reads/errors; versioned descriptors; stable ID reservation; source/clone reconciliation; single flight per profile; shared profile failure fanout; retry/backoff; worker ownership; session epochs; bounded cancellation/shutdown; credential snapshot policy; and durable Add intent/rollback. A future direct Google login completes an Add transaction through this module rather than constructing a second lifecycle in the form.

Keep protocol parsing, query paging, MIME handling, mail mutations and UI layout outside. Do not put all of `LiveMailbox` into the new module. The account-session interface is not a generic replacement for `MailRequest`.

### Common-startup optimization

1. Load durable descriptors once off the UI thread. Publish known accounts immediately, even if dormant/offline.
2. Attempt saved accounts without showing import or enumerating every unrelated Thunderbird profile. Separate explicit discovery from reconnect.
3. Start one Thunderbird runtime per required source; reuse unchanged generations. Return ready account transitions incrementally instead of waiting for every sibling's first page.
4. Defer message-body work, unrelated folder scans and all-folder search. Preserve existing bounded header/body caching and shared runtime multiplexing.
5. Retry by profile internally when a shared bridge dies; keep per-account display/results. Do not restart healthy profiles because another profile failed.
6. Measure inventory-ready, process-ready, first-header and first-body latency separately. Existing six-account measurements are not a direct-vs-Thunderbird benchmark ([validation](../MEGAMAIL_VALIDATION.md#remaining-verification-limits)).

### Tradeoffs and invariants

- Local account inventory belongs to the app; provider sessions are remote/external dependencies. Filesystem tests use explicit temporary paths; backend adapters use deterministic fakes. Inject dependencies privately—do not expose credential/clone internals to GPUI merely for tests.
- Keep source-descriptor storage separate from credentials. Thunderbird encrypted stores remain Thunderbird-owned; direct secrets remain in MegaMail Secret Service. Never make credentials transferable by treating two backends as interchangeable auth adapters.
- Existing direct cache IDs cannot be casually changed: a future common allocator must reserve historical direct IDs and saved TB IDs, define collision recovery, and avoid cross-account cache mixing. Detect collisions now; migrate deliberately later.
- Per-profile actors reduce process count but couple failures; the module must represent that coupling explicitly. One runtime per account would simplify isolation but increase process/memory/startup costs.
- Holding dormant descriptors adds state but removes caller ambiguity. Use typed phases, not more `*_started`, `*_seen_loading`, and `*_auto_open` booleans.
- Removal must reconcile runtime ownership, descriptor persistence, private clone retention and queued sends. It is not “delete account row.” Defer destructive removal until its data-retention contract is specified.
- Avoid adding another parallel source of truth. Move existing lifecycle decisions into the module; replace helper-only tests with behavioral tests through its interface as coverage becomes available.

## 6. One implementable first slice: saved startup is never first-run setup

**Outcome:** with a saved Thunderbird reference, a direct-config error, unavailable source or failed session leads to a retained account/recovery surface with Retry—not an invitation to import the same mailbox. Healthy accounts remain healthy when a sibling is retried. The existing Thunderbird transport remains unchanged.

### Bounded implementation

1. Introduce the GPUI-independent account-session module for startup inventory and reconnect reconciliation only. Reuse manifest v1; do not add OAuth, migrate IDs/cache, rework compose or change mail operations in this slice.
2. Replace `has_saved_accounts` boolean/read-twice routing with a single loaded Thunderbird inventory passed to restore. Missing is Empty; invalid/unreadable is StorageRecovery. Read direct and Thunderbird inputs independently and retain their errors independently.
3. Represent known v1 `(source path, source_id, app_id)` entries as configured identities before runtime success. When source is absent, use a generic display label plus app ID; do not invent an email. Resolved metadata may enrich the in-memory descriptor. A later schema can persist display metadata.
4. Keep the initial live entity and initialize direct workers after inventory load instead of replacing it, or prove replacement happens before any session work and is exclusive. In either case, retain one lifecycle owner/event channel for startup and later additions.
5. Reconcile Retry against backend-qualified identity: same already-active identity is a no-op/success; a different owner with the same numeric ID is an explicit conflict. Retry failed/not-started saved identities without rereading every healthy source.
6. Main routes Loading -> Empty first-run, Ready configured mailbox, or StorageRecovery. Recovery offers Retry and explicit Add/Relink; Add is not the primary action for an existing reference. Live's connection success must not erase configured inventory on failure.

**Owned files:**

- New `apps/megamail/src/account_session.rs`: inventory load/state, startup/retry policy, internal injectable adapters, behavioral tests.
- `apps/megamail/src/main.rs`: module wiring, remove bool-based startup routing and redundant auto-open startup flags; preserve explicit Add flow and existing layout.
- `apps/megamail/src/live.rs`: bootstrap/install transitions, account inventory display integration and targeted retry; preserve retained mailbox and pending send ownership.
- `apps/megamail/src/thunderbird_adapter.rs`: manifest inventory extraction/single-load restore and already-active reconciliation; do not rewrite actor mail operations.
- `docs/MEGAMAIL_ARCHITECTURE.md`, `docs/MEGAMAIL_VALIDATION.md`: exact behavior/verification corrections after tests run.

**Do not claim this slice fixes F3 or F5.** Common stable-ID allocation/collision migration and persist-before-connect Add intent need subsequent owned work. If mixed-account additions are enabled during this slice, block a conflicting Add before durable save and report the conflict; fixing it only at `live.add_profile` is too late. Do not fan out edits to these shared files concurrently.

### Deterministic red-capable synthetic regression

No test/harness was created or run here. Current restore dependencies are hard-coded (`read_store`, global home scanner, concrete runtime), and main owns the decisive routing in a GPUI continuation. Existing helper tests cannot lock down the full symptom. First extract the production startup/reconcile implementation unchanged behind internal test dependencies; route the real main continuation through it. **Do not replace it with a toy copy of the branching logic.**

Suggested test: `saved_thunderbird_survives_direct_metadata_error_without_import`.

- Use a unique temporary directory; pass explicit config/data/home roots into test adapters. No environment-variable mutation and no discovery of the user's real home.
- Store synthetic manifest v1 with a canonical temporary source path, `source_id="account1"`, `app_id=7`; use `synthetic@example.test` in synthetic prefs. Credential files are absent or meaningless synthetic bytes and never decrypted.
- Direct metadata loader returns InvalidData (or read an intentionally malformed synthetic TOML). Thunderbird inventory uses the real bounded manifest reader with explicit path and the real scanner's `discover_profiles_in` on the fixture root.
- Backend start adapter records calls and returns a deterministic typed Offline failure first; no Thunderbird process, socket to a provider, or mail mutation is allowed.
- Drive **the production startup path** to completion. Assert: inventory retains account 7; view decision is configured/recovery, not first-run; restore was attempted despite direct error; primary recovery action is Retry; no import-selection command was requested.
- Drive Retry(7) with fake startup now returning Ready. Assert same ID, one account, zero Add/import operations, and direct storage error still independently visible.
- Restart the session with the same manifest and a fresh fake backend; assert configured identity again appears without import.

Current main's direct Err branch cannot attempt restore and chooses Setup, so the startup part is red-capable before the behavior change. A pure module test covers the ownership/policy seam; add a GPUI smoke/assertion for actual screen mapping after it goes green. A test that asserts only manifest parsing or matching booleans is insufficient.

Parent-only command after adding that test (not executed here):

```sh
cargo test --manifest-path apps/megamail/Cargo.toml --locked saved_thunderbird_survives_direct_metadata_error_without_import -- --nocapture
```

Additional acceptance cases in the same narrow test group:

| Case | Observable assertion |
|---|---|
| No direct or TB records | Empty, first-run Setup; zero backend calls. |
| Valid saved TB, direct inventory valid/empty | Configured account before fake backend completes; no import required. |
| Malformed TB manifest, direct empty | StorageRecovery, not Empty; bytes untouched; import cannot silently overwrite it. |
| TB source absent | Account ID retained, SourceUnavailable, Retry/Relink offered; no fabricated email. |
| Healthy TB 7 + failed TB 8, then Retry(8) | 7 neither restarted nor reported as conflicting; exactly one retry of failed profile where possible. |
| Repeated Retry or startup request while pending | Single flight; stale completion cannot win over current inventory. |
| ID 7 owned by direct and distinct TB reference | Explicit conflict; no sender/cache alias and no automatic renumbering. |
| Source list contains unrelated bad profile | Restore of saved good source unaffected when targeted read is implemented. |

Existing tests are useful but incomplete: `source_manifest_rejects_duplicate_app_and_source_account_ids` checks validation, `failed_empty_startup_retires_actor_so_retry_gets_a_new_channel` checks registry/helper behavior, and `broken_startup_prefetch_never_returns_ready_accounts` checks a result transformer, not full restart/UI behavior ([`thunderbird_adapter.rs:4159`](../../apps/megamail/src/thunderbird_adapter.rs#L4159), [`4425–4480`](../../apps/megamail/src/thunderbird_adapter.rs#L4425)). Direct-ID stability tests exercise only the direct allocator ([`config.rs:2038–2093`](../../crates/mail-core/src/config.rs#L2038)); they do not disprove F3.

### Minimal instrumentation if the synthetic test does not match the user's case

Add narrowly scoped, opt-in structured `account_lifecycle` events: startup run ID, build identity, storage-root fingerprints, store status/version/counts, backend-qualified app ID, source lookup outcome, clone generation reused/new, source snapshot safe/busy, spawn/handshake status, connection phase, retry target, persistence commit result and view decision. Include durations and a reason code, not raw provider errors containing account details.

Never emit profile paths, emails, credential bytes/stamps, OAuth URLs/codes/tokens, mail subjects/bodies or encrypted-store contents. Hashing an email is not anonymization. App IDs and runtime epochs suffice for correlation. Tests should assert transitions through the public module interface; logs explain a field report but are not the regression oracle.

## 7. Login, backend strategy, and broader app gaps

### Keep Thunderbird available; improve the direct backend independently

The current independent direct Hylki worker and Thunderbird runtime are genuine implementations ([architecture runtime shape](../MEGAMAIL_ARCHITECTURE.md#runtime-shape)). Preserve the latter as a supported backend for existing saved sessions; do not silently replace it with direct auth or treat imported encrypted tokens as portable.

A faster direct backend is plausible, not demonstrated here. Compare on synthetic and explicitly authorized representative accounts: cold/warm inventory-to-first-header, authenticated reconnect, first-body latency, idle CPU/memory, network calls and error recovery. Thunderbird-inspired discovery/settings parsing and durable profile/session practice can be reused conceptually without adopting its registrations or privileged Gecko modules. Existing account-onboarding research's recommendation against embedding Thunderbird predates the implemented bridge and must not override the user's current keep-it-available requirement ([older research](account-onboarding.md#recommendation), [current product](../../PRODUCT.md#capabilities-and-constraints)).

### Real Google/other-provider login: new native integration, existing core primitives

- Native setup currently offers app-password/manual/discovery paths and explicitly tells users direct Google OAuth is not configured ([`main.rs:4750–4755`](../../apps/megamail/src/main.rs#L4750), [`4964–4975`](../../apps/megamail/src/main.rs#L4964)). Changing a label to “Sign in with Google” would not implement login.
- The core already has HTTPS endpoint validation, external-browser loopback authorization, random state, PKCE S256 and code exchange. App-owned Google/Microsoft credentials are resolved from MegaMail settings/environment; registration absence is a typed setup failure ([`oauth.rs:458–590`](../../crates/mail-core/src/oauth.rs#L458), [`onboarding.rs:65–91`](../../crates/mail-core/src/onboarding.rs#L65)). These are implemented but not proof of a usable native provider flow or provisioned registration.
- Google documents installed-app PKCE and loopback flows; Gmail IMAP/SMTP scope is `https://mail.google.com/` and public user-data access requires verification. Google desktop token requests can require a client-secret parameter, but a distributed desktop application cannot keep that value confidential. Do not remove it on the assumption all public native token endpoints omit it. [Google installed-app OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [Gmail XOAUTH2](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol).
- Microsoft requires a MegaMail Entra registration and delegated protocol scopes including IMAP access, SMTP send and offline access as needed; tenant/account policy can still prevent access. [Microsoft protocol OAuth](https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth).
- Headless Thunderbird cannot show primary-password/new-provider prompts; current generic failure classification cannot reliably identify every native status as NeedsLogin ([`api/implementation.js:12–37`](../../tools/thunderbird-bridge/extension/api/implementation.js#L12), [`main.rs:6595–6601`](../../apps/megamail/src/main.rs#L6595)). Reauthentication needs an explicit supported interaction, not an infinite reimport loop. Thunderbird auth must stay Thunderbird-owned.

Next login slice: one approved provider registration, explicit browser-based native setup/cancel/retry, account-session Add transaction, keyring refresh-token persistence, verified restart/refresh/revoke behavior. Broader discovery is not universal OAuth; provider adapters and policy coverage remain explicit.

### Gap categories to keep honest

| Category | Status and evidence |
|---|---|
| Saved TB references/private clone/real backend | Already implemented; not retested here. Historical six-account and persisted-clone probes exist, but do not prove the current installed startup route or all providers ([validation](../MEGAMAIL_VALIDATION.md#remaining-verification-limits)). |
| Direct IMAP/SMTP worker, private cache and durable Outbox | Already implemented; native real-provider/keyring-unlock coverage remains limited ([architecture](../MEGAMAIL_ARCHITECTURE.md#current-boundaries), [product](../../PRODUCT.md#capabilities-and-constraints)). |
| Unified Inbox, reference-based conversations, scoped actions, bounded reader | Already implemented, with explicit bounded/partial semantics; not a new stub needing replacement ([product](../../PRODUCT.md#capabilities-and-constraints)). |
| Full-history/body/all-folder search, rich composition, inline reply, bulk triage | New work explicitly not ported; do not mistake inherited Hylki functions for native UI coverage ([product Open Decisions](../../PRODUCT.md#open-decisions)). |
| TB outbox retry/discard UI; remote draft coverage; delivery assurance | New capability/verification work. TB queued sends belong to its private runtime; direct Outbox has separate controls. Loopback draft/send tests do not prove remote draft sync or real-provider delivery ([architecture mail flow](../MEGAMAIL_ARCHITECTURE.md#account-setup-and-current-mail-flow)). |
| Real Google OAuth and provider breadth | New native setup/registration/policy work using existing core primitives, not an ISPDB toggle. See first-party sources above. |
| Contacts/calendar, snooze/scheduled-send UX, advanced rules, account edit/remove, encryption UI, accessibility | Audit next through native call sites and competitor first-party documentation. This focused inspection does not establish absence or parity; shared core symbols or UI settings labels alone are insufficient evidence. |
| Startup/performance, physical-display UX, Windows/macOS | Measurement/verification plus platform work; Linux is current target ([product Platform](../../PRODUCT.md#platform), [validation limits](../MEGAMAIL_VALIDATION.md#remaining-verification-limits)). |

Before implementation fanout: one owner for account-session/main/live/adapter edits, one owner for ID-allocation migration, and a serial parent Cargo gate. Parallel agents can research provider requirements or write isolated fixture plans, but must not independently mutate overlapping lifecycle files or launch real-mail tests.

## 8. Acceptance and handoff

Deliver the first slice only after the synthetic regression goes red on the current startup behavior and green on the production-wired module, with restart and partial-retry cases included. Report exact commands/results and the installed build identity separately. Do not claim a real-user fix from helper tests or documents alone.

This audit establishes code paths and deterministic failure inputs; it does not establish the user's active manifest, installed binary, OAuth registration, current credentials, or provider health. If synthetic coverage passes while the report remains reproducible, request an opt-in redacted lifecycle trace—not access to personal mail—and choose the next change from that evidence.
