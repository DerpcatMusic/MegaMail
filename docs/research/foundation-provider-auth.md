# Foundation audit: native provider authentication and account ownership

## Scope and evidence

Research-only audit of MegaMail at `d4dbd2bba14f474a33870269cc15bd98376ff360`, retrieved on **2026-10-10**. Line references describe that snapshot. Read `AGENTS.md`, `PRODUCT.md`, `docs/MEGAMAIL_ARCHITECTURE.md`, the Rust best-practices, codebase-design, find-docs, and diagnosing-bugs skills, and relevant source. Public documentation was retrieved directly; Context7 was used to resolve/query Google OAuth documentation, then primary pages were checked because the indexed answer overemphasized web-server examples.

No Cargo build/check/test, native app execution, private profile inspection, credential reads, mail-provider login, mail mutation, or commit was performed. The parent retains the serial Cargo slot. This report is the only repository file written by this audit. Public-document downloads used `/tmp`, not repository fixtures or user profiles.

**Evidence levels:**

- **Confirmed source defect/gap:** the inspected code establishes the behavior or missing path. This is not a live runtime reproduction.
- **Suspected bug:** plausible causal path needing a deterministic regression test or authorized runtime evidence.
- **Already implemented, unverified here:** do not rebuild it or mistake existing helpers for an end-to-end feature.
- **New work:** absent capability or deliberate product extension, not necessarily a regression.

The diagnosing-bugs runtime loop was explicitly out of scope: the requested no-Cargo/no-private-profile constraints prevent a red-capable native reproduction. A read-only Python source assertion checked six facts: native flow callers absent; password builder rejects OAuth; saved Thunderbird restore wired; Google secret gate present; worker does not consume `account.oauth_refresh`; Graph probe falls through to IMAP. All assertions returned true. These are static evidence checks, not security or mail integration tests.

## Executive decision

**Keep Thunderbird available. Add a MegaMail-owned direct Google OAuth slice without replacing the working backend.** Do not start a faster-backend rewrite before reliable account restoration, credential ownership, secure reauthorization, and representative measurements exist.

The native application does **not** have working Google/Microsoft browser sign-in merely because the core has PKCE and token-refresh helpers. Its setup path constructs password accounts and rejects OAuth candidates. Microsoft has an additional trap: the core OAuth preset requests **Graph** permissions, not IMAP/SMTP permissions; assigning those tokens to an IMAP account will not work.

Repeated import is **not proven to be a current unconditional startup bug**. There is a saved manifest, automatic restore, stable IDs, reconnect UI, and copy saying no import is needed. Preserve these paths. Investigate corrupt/missing manifests, unavailable source profiles, clone refresh locks, unsupported interactive unlock, and the startup error branch instead of prescribing another import on every launch.

Three release gates are separate:

1. Secure native flow and credential lifecycle in code, tested locally with synthetic adapters.
2. MegaMail-owned registrations and provider/tenant approval.
3. Explicitly authorized real-account retrieval/reconnect tests, then separately authorized sending tests.

Passing gate 1 does not imply gates 2 or 3.

## 1. Existing native setup → core → worker trace

| Stage | Evidence | Consequence |
| --- | --- | --- |
| First-run/startup | `apps/megamail/src/main.rs:900–958` loads `config::load_profiles()` and `thunderbird_adapter::has_saved_accounts()` off-thread. Direct accounts open Mailbox; empty direct accounts trigger Thunderbird restore. | Do not equate an empty direct `accounts.toml` with no accounts. Thunderbird references are separate. |
| Provider discovery | `crates/mail-core/src/discovery.rs:189–261`, `known_preset:463–511` | Gmail/Googlemail get a **password** preset with app-password help; Outlook/Hotmail/Live get an OAuth2 preset. Other domains use ISPDB and provider HTTPS autoconfig. |
| Discovery safety | `discovery.rs:264–369`, `fetch_limited:722`, `email_domain:513` | Bounded XML, DTD disabled, domain match required, redirects disabled, encrypted settings/review warnings. Does not establish provider policy or OAuth registration. |
| Native form | `apps/megamail/src/onboarding.rs:14–185`; `main.rs:2293–2358` (`collect_account`) | `AccountForm` owns password strings and always calls `set_password`. There is no provider sign-in branch. |
| Core conversion | `crates/mail-core/src/onboarding.rs:191–253` (`build_imap_account`) | Incoming auth must be `Password`; outgoing auth must be `Password` or `None`. OAuth2 candidates are rejected even if a client ID is configured. |
| Native test/add | `main.rs:2358–2440` (`connect_account`) | Tests incoming and SMTP before `config::save`, reloads profiles, adds one live worker. Does not send mail. |
| OAuth plumbing | `crates/mail-core/src/onboarding.rs:68–88`; `oauth.rs:458–617`, `Authorization::new:528`, `refresh_access_token_as:661` | Registration resolver, external browser, loopback, PKCE S256, state, exchange, refresh exist but are not called by native setup. |
| Credential persistence | `crates/mail-core/src/config.rs:199–281`, `457–480`, `1027–1126`, `1345–1366` | Runtime secrets skip serialization and redact Debug; keyring uses `com.megamail.MegaMail`; generation is committed with atomic metadata. |
| Runtime tokens | shared `src/worker.rs:5618–5706`, `connect:6666`, SMTP auth `6762–6766` | Worker loads refresh token by email from MegaMail's configured keyring, caches access tokens, stores replacements. An unsaved `account.oauth_refresh` is **not** the probe credential. |
| Shared source | `crates/mail-core/src/lib.rs:15–52` | Core exports MegaMail `config`/`oauth` but reuses root worker/Graph/JMAP source; changes must preserve the GTK crate's `crate::config`/`crate::oauth` callers. |

### A setup trap that must be addressed in the first slice

The password setup can test before saving because it passes a transient password to the worker. Simply setting `account.oauth = true` and `account.oauth_refresh = result.refresh_token` cannot reuse that sequence: `fetch_oauth_token` loads from the keyring by email and ignores the transient field (`src/worker.rs:5629–5637`). Persisting the token early to make the test work would violate test-before-commit ownership and make cancellation/rollback unsafe.

Introduce an ephemeral grant/session credential for the setup probe, separate from the committed account worker. Commit the refresh credential only after identity and connection checks succeed. Do not temporarily overwrite an existing email-keyed token as a probe workaround.

### Thunderbird startup and restoration already exist

- `apps/megamail/src/thunderbird_adapter.rs:70–108`: manifest account references contain source ID + persistent MegaMail app ID; `has_saved_accounts` reads it.
- `thunderbird_adapter.rs:543–563`: successfully started accounts are written to the source manifest; only successful new imports are persisted.
- `thunderbird_adapter.rs:586–738` (`restore_saved`): rediscovers source profiles, resolves stored IDs, checks duplicate IDs/emails, produces outcomes for missing sources/accounts, and starts valid ones.
- `apps/megamail/src/live.rs:736–780` (`restore_thunderbird`): background restore, reserved IDs, duplicate-mailbox protection; completion resets restore/loading guards at `1676–1677`.
- `main.rs:1092–1103`: restores can leave Setup for Mailbox; `3108–3138` renders “Reconnecting saved accounts…” / “Your imported accounts are saved. No import is needed.”; `3460–3472` has “Reconnect saved accounts.”
- `crates/mail-core/src/thunderbird_bridge.rs:539–651`: clone generations follow source credential stamp; refuse changed snapshots while source Thunderbird is open; private profile lock and encrypted store copy.
- `main.rs:6598–6600` explains interactive login/primary-password limits. `thunderbird_adapter.rs:3704–3749` classifies errors by string fragments, not an authoritative OAuth error type.

**Product evidence, not a new test:** `PRODUCT.md` and architecture record six scoped authenticated Thunderbird retrievals plus Thunderbird 157 loopback new/reply send and draft-save checks. They explicitly do not claim real-provider sending, all configurations, remote Drafts, or native direct-provider verification.

## 2. Findings and classification

| ID / priority | Classification | Finding and evidence | Required response |
| --- | --- | --- | --- |
| F1 / P0 | Confirmed native capability gap | No native caller of `oauth::run_flow`/`exchange_code`; password-only builder at `onboarding.rs:212–219`; UI warns OAuth is not configured (`main.rs:4964–4975`). | A real native provider sign-in path, conditional on registration; no fake success. |
| F2 / P0 | Confirmed integration defect **if existing preset is used for native IMAP** | Microsoft `oauth::preset` uses Graph Mail.ReadWrite/Mail.Send/MailboxSettings.ReadWrite/User.Read (`oauth.rs:231–244`); discovery proposes IMAP/SMTP (`discovery.rs:474–483`). | Explicit resource/protocol selection. Never use Graph tokens for XOAUTH2 IMAP/SMTP. |
| F3 / P0 | Confirmed probe coverage defect | `src/worker.rs:6707–6720` handles POP/JMAP, otherwise tests IMAP and SMTP; `Protocol::Graph` is not handled. | A Graph-specific read-only probe before any Microsoft Graph setup is wired to Test and add. |
| F4 / P0 | Confirmed flow robustness defect by source; no runtime replay | `wait_for_code`, `oauth.rs:735–800`, accepts a blocking stream and calls `read` without read deadline. Its five-minute deadline is checked only when accept returns WouldBlock. | Bound each accepted socket read/write and total operation; support cancellation; one local stalled peer must not trap the flow. |
| F5 / P0 | Confirmed callback validation/UX defect by source | `oauth.rs:755–777` accepts provider error before checking state; `780–797` sends success page before state/code validation. `Authorization::answer:563–581` also accepts errors without state validation. | Validate method, exact callback path, unique params, transaction state and expected provider for **success and error**. Invalid/favicons must not complete the transaction. Show “return to app; finishing” until exchange/commit has succeeded. |
| F6 / P1 | Confirmed discovery loss of valid auth alternatives | `parse_server:392–400` uses first `authentication` child; `child_text:457` returns one child. Public Yahoo ISPDB retrieved this audit contains OAuth2 **then** password-cleartext for the same endpoints. The core rejects the selected OAuth2 candidate, losing the legitimate app-password alternative. | Preserve auth alternatives separately from endpoint selection. User chooses approved method; never silently downgrade OAuth. Add Yahoo XML regression fixture. |
| F7 / P1 | Confirmed mismatch with current published Google contract; token behavior untested | `onboarding.rs:75–79` / `oauth::provider_credentials:323–359` require a Google client secret and claim the endpoint demands it. Current Google native docs label `client_secret` **Optional** for both exchange and refresh. | Do not classify a valid public desktop client ID alone as “unregistered.” Registration type/required fields belong to provider policy; test the actual owned registration separately. A bundled desktop secret is not a confidential-client proof. |
| F8 / P1 | Confirmed credential-retention gap | `config::save:1114–1116` deliberately defers orphan-generation cleanup; deletion (`1431–1485`) targets only the active generation and has no inventory of historical ones. | Generation inventory/journal + safe GC and account-deletion retry. Do not claim remove account erases all old refresh/password grants. Recovery retention is a tradeoff, not “harmless” from a security perspective. |
| F9 / P1 | Suspected refresh concurrency/stale-completion bug | `fetch_oauth_token:5629–5660` has cached read then network refresh then store; mutex only guards cache map (`5666–5693`), not the whole refresh. `store_oauth_refresh` locks storage, but has no grant-version compare-and-swap. | Single-flight by grant/resource and generation check. Concurrent SMTP/IMAP/Graph and reauth/save/delete tests must reproduce or rule it out. Microsoft's old token is not immediately revoked on refresh, but other providers rotate strictly. |
| F10 / P1 | Confirmed error-information loss; runtime consequence suspected | Token HTTP status is mapped to a string without OAuth error body (`oauth.rs:53–61`); worker collapses refresh failures to `Option` (`src/worker.rs:5637–5639`); keyring loads also map failure to None (`config.rs:1325–1333`). | Typed `invalid_grant`, denied consent, policy block, transient network, and keyring-locked states. Do not prompt reauth for timeout/keyring outage. |
| F11 / P1 | Suspected explanation for repeated import, **not confirmed recurrence** | Startup discards `has_saved_accounts()` errors with `unwrap_or(false)` (`main.rs:940`), opens Setup on direct-config load error (`947–950`), and saved restore depends on source rediscovery (`thunderbird_adapter.rs:601–635`). | Restore a persistent account roster even when unavailable; show repair/reconnect, not import. Synthetic restart/missing/corrupt manifest tests; request redacted symptom evidence only if needed. |
| F12 / P1 | Confirmed identity-model limitation / new migration work | `profile_identity` includes email/protocol/host/port/username (`config.rs:720–736`), whereas credentials are unique by normalized email (`598–615`, `1290–1322`). Provider issuer/subject/client/resource are not canonical identity. | Stable account/grant IDs independent of editable connection values; preserve current cache IDs on migration. |
| F13 / P1 | Confirmed native lifecycle capability gap | Native source has one `config::save` call in connect_account, no call to `delete_account_secrets`, and no account removal/reauth editor path in `apps/megamail/src`. Existing account list/add/reconnect is not full lifecycle management. | Add reauthorize/disconnect/remove with explicit cache policy, not reimport. Do not reinterpret PRODUCT's “account management” as complete ownership controls. |
| F14 / P1 | New provider work, not a safe checkbox | Proton manual setup allows private hosts only after review and requires normal TLS trust (`onboarding.rs:300–309`, `security:245–247`). Proton Bridge uses generated self-signed local certs. | Dedicated loopback Bridge method with explicit scoped trust/pin; do not enable global invalid-cert acceptance. |

### Already implemented: preserve and verify, do not duplicate

- PKCE S256 and secure RNG (`oauth.rs:532–545`), RFC vector test (`1039`), HTTPS/no-userinfo/no-fragment endpoint checks (`22–49`), zero HTTP redirects for token requests (`64–67`). **These do not by themselves bind a malicious custom HTTPS host to a trusted provider.**
- Password/TLS/manual validation, distinct SMTP username/password and no-login outgoing mode (`onboarding.rs:191–309`, native form conversion). Gmail app-password route remains useful where permitted.
- Private directories/files, atomic metadata, keyring-first generations, redacted Debug/secret serialization. Existing tests at `config.rs:1910–2033` cover serialization/staging; they were not run here.
- Stable direct profile ID mapping surviving reorder/removal/readd (`config.rs:712–826`, tests `2038–2088`). Do not reset that mapping in a new schema.
- Access-token expiry margin, rejection invalidation, refresh replacement persistence (`src/worker.rs:5662–5706`). Caching is not single-flight or ownership isolation.
- Persistent Thunderbird references, restart/reconnect, scoped identity routing and limited body cache. Do not delete this adapter during OAuth work.

## 3. Provider inventory and restrictions

Provider support means an approved authentication route **and** tested protocol behavior for the relevant account policy. Domain suffixes, a preset, or one successful Thunderbird account are not a compatibility guarantee.

| Provider | Native target and current state | Restrictions / externally blocked parts |
| --- | --- | --- |
| **Google Gmail / Google Workspace** | First slice: browser authorization-code + S256 PKCE with a MegaMail Desktop registration; Gmail IMAP 993 implicit TLS, SMTP 465 implicit TLS or 587 STARTTLS, XOAUTH2. Current native route is app password only. | Gmail IMAP/POP/SMTP uses `https://mail.google.com/` ([G2]); this is broad/restricted, not replaced by `gmail.readonly`. Public production verification, branding/privacy/limited-use compliance; security assessment is conditional on restricted data passing through a server, not universally required for local-only clients ([G4]). External Testing refresh tokens expire in 7 days for mail scopes ([G3]). Workspace admin policy may deny access. App passwords require 2-Step Verification and can be unavailable under security/admin restrictions; password change revokes them ([G5]). Workspace custom domains must be explicitly recognized, not inferred as Gmail from any Google-like MX. |
| **Microsoft Outlook.com / Microsoft 365** | Decide between direct Exchange IMAP/SMTP OAuth and existing Graph worker. `common` must match registration supported account types; tenant/consumer/organization choice is explicit. Current discovery OAuth cannot reach native setup. | IMAP uses `https://outlook.office.com/IMAP.AccessAsUser.All`, SMTP uses `https://outlook.office.com/SMTP.Send`, plus `offline_access` ([M1]); Graph scopes are different. Register a **public desktop/mobile** application, PKCE, no client secret ([M4]); tenant consent/CA/publisher rules may block. Exchange Online IMAP Basic auth is removed; app password is not a general fix ([M5]). IMAP/SMTP AUTH may be disabled even with OAuth. Do not hardcode an SMTP Basic retirement date: [M5] links the changing timeline. Shared mailbox grant principal differs from XOAUTH2 mailbox username; model these separately. |
| **Yahoo / AOL** | IMAP/SMTP app-password fallback with truthful provider guidance; future approved Yahoo OAuth. Current Yahoo public ISPDB first-auth parsing blocks the discovered password alternative. | Yahoo's current developer-access page requires application/access review for consumer mail interfaces ([Y2]); a generic OAuth app/tutorial is not proof of approved mail scopes. The old tutorial is explicitly server-side and contains OOB examples; **do not copy those into a native flow**. Yahoo app-password creation eligibility may be denied and support cannot override it; changing main password does not revoke app passwords ([Y1]). Thunderbird's Yahoo integration does not license its client ID to MegaMail. |
| **iCloud Mail (icloud.com / me.com / mac.com, including hosted custom domains)** | IMAP `imap.mail.me.com:993`; SMTP `smtp.mail.me.com:587` STARTTLS. App-specific password route fits current core. IMAP may use local part; SMTP full email ([A1]); retain independent usernames. | Apple Account 2FA needed; up to 25 app-specific passwords; changing/resetting primary password revokes them ([A2]). Apple's current docs also acknowledge Apple Account authorization for **supported** third-party apps. Do not claim “Apple never supports OAuth,” and do not mistake general Sign in with Apple for iCloud mail authorization. Availability/registration for MegaMail is unverified. |
| **Fastmail / custom domains** | Existing direct IMAP/SMTP app-password path; eventual OAuth or JMAP. IMAP 993, SMTP 465 or 587 STARTTLS ([F1]). Root shared JMAP code exists but native setup only builds IMAP. | Normal Fastmail password is not valid for protocol access. Basic plans exclude IMAP/SMTP/CalDAV/CardDAV ([F1]). Official developer docs support OAuth for protocols and require manual registration/contact with Fastmail ([F2]); S256, provider-specific redirect path, strict refresh-token replacement/reuse protection. Do not turn JMAP tokens or protocol-specific OAuth grants into generic password strings. |
| **Zoho / custom domains** | IMAP/SMTP discovered/manual with correct datacenter and account-type hosts; app-specific password when 2FA is enabled. IMAP access must be enabled ([Z1]). | Personal and organization hosts differ (`imap.zoho.com` versus `imappro.zoho.com`; outgoing similarly); use account-specific server configuration, not a universal .com preset. Free-plan protocol access varies: current pricing says Forever Free excludes IMAP/POP/ActiveSync ([Z2]), while setup docs describe some Free Organization configurations. Treat entitlement/region/legacy status explicitly. Native IMAP OAuth client requirements were not verified; do not advertise Zoho REST OAuth as mail-protocol support. |
| **Custom-domain/self-hosted IMAP** | HTTPS ISPDB/provider autoconfig and explicit manual TLS settings; separate SMTP credentials and no-auth mode already exist. | Custom email domain is not the authentication issuer. Endpoint discovery must not confer trust on token endpoints or authorize sending credentials to arbitrary hosts. IDNA domains currently rejected (`discovery.rs:513–547`); enterprise Autodiscover/MX/SRV fallback and SASL methods beyond existing password/XOAUTH2 are separate work. No silent plaintext/invalid-cert downgrade. Public-DNS-to-private-IP/rebinding needs a policy/test; lexical host checks alone are not DNS validation. |
| **Proton Mail Bridge** | Keep Thunderbird path; new explicit direct Bridge method using user-provided loopback endpoints/Bridge password. No direct Proton cloud IMAP promise. | Bridge is paid-plan-only, decrypts locally, exposes local IMAP/SMTP, and uses a unique password different from Proton login ([P1]); generated self-signed cert and 127.0.0.1 transport are documented ([P2]). User authenticates to Proton in Bridge, not MegaMail OAuth. Respect actual generated ports/cert; never assume 993/587. Cache contains decrypted data outside Bridge's own storage guarantees: explain and enforce MegaMail cache/privacy policy. |

### Correct current documentation interpretations

- **Google loopback is still supported for Desktop registrations.** The migration restriction is for Android/iOS/Chrome app types, not Linux desktop ([G6]). Use system browser, not embedded credentials/WebView.
- **Google desktop client secret is optional in current docs.** Existing code's unconditional requirement is stale. Even if a registration distributes a client-secret value, an installed client cannot keep it confidential ([G1], [R1 §8.5]); no server-style secrecy claim.
- **Microsoft does not revoke the old refresh token immediately upon normal refresh.** [M3] says retain new, securely delete old. Current comments about a token being “spent” are not generally true for Microsoft; Fastmail explicitly has strict reuse protections ([F2]). Implement provider policy, not one universal rotation assumption.
- **Microsoft redirect matching is provider-specific.** Current code uses `localhost` because Microsoft ignores localhost port differences; [M2] documents that exception and recommends literal 127.0.0.1 where registered/configured. Do not blindly switch host/slash/path or mix Web/SPA/native redirect platform types.
- **Read/write permission and sending capability are distinct.** A successful login need not imply SMTP AUTH enabled. Support typed read-only/policy-disabled capability instead of interpreting all such failures as bad credentials.

## 4. App registrations versus reusable Thunderbird authentication

A public client ID identifies an application registration; it is not an account password. A refresh grant belongs to the issuing client/issuer/resource/account relationship. A desktop-bundled “secret” is extractable and must not be represented as confidential-client security.

**Allowed existing Thunderbird route:** with user consent, launch installed Thunderbird with the private clone and let Thunderbird handle its own encrypted store and OAuth registration. MegaMail receives mailbox operations/results, not raw tokens. The source profile is not modified. Preserve primary-password failures, source-change locks, limited copies, isolation, and explicit import.

**Not allowed:** copy Thunderbird/Thunderbird-for-Android client IDs into MegaMail direct auth, decrypt/export tokens from `logins.json`/`key4.db`, use a Thunderbird refresh grant with MegaMail's registration, or silently swap registrations for an existing account. Public Mozilla source is inspiration for provider mappings/capability handling, **not** registration reuse permission ([T3]). This audit did not copy registration values into the report or source.

Reauth for a Thunderbird account remains with Thunderbird/its clone owner until a separately designed authorized interactive-clone flow exists. “Sign in to source Thunderbird, close it, reconnect saved account” is an interim repair, not “import all again.” A direct-backend upgrade is a **new MegaMail authorization**, with explicit account matching/cache policy, not token migration. Keep the original working Thunderbird account available until the direct account passes user-approved verification; do not run two mutating workers against the same account by accident.

The encrypted store copy is profile-wide, whereas import selection is account-specific (`thunderbird_bridge.rs:814–825`, bounded file copy). Do not claim selected-account-only secret minimization. This is an inherited isolation tradeoff to document and revisit without inspecting/decrypting stores.

## 5. Canonical metadata and credential lifecycle (proposed, not implemented)

### Separate mailbox, identity, authentication owner, and execution adapter

Use non-secret, versioned metadata with these invariants:

```text
AccountRecord v2
  account_id: immutable MegaMail ID
  cache_id: existing u32 worker/cache mapping (preserved)
  display_name, primary_address, aliases, enabled
  backend: DirectImapSmtp | MicrosoftGraph | DirectJmap | Thunderbird
  connection: typed per-backend encrypted endpoint settings + capabilities
  auth: PasswordGrantRef | OAuthGrantRef | ThunderbirdSourceRef
  source/provenance: user/manual/preset/ISPDB; selected method; review stamp
  lifecycle: configured/disabled/removing; last safe diagnostic (no secrets)

OAuthGrant metadata
  grant_id, account_id, credential_generation
  provider_key, issuer, subject, tenant (when relevant)
  registration_id + client_id + registration_type + redirect policy version
  requested_scopes, granted_scopes, resource/audience
  mailbox_login_identity (not necessarily OAuth subject/email)
  status: Ready | NeedsConsent | NeedsReauth | PolicyBlocked |
          KeyringLocked | TemporarilyUnavailable | Revoked

ThunderbirdSourceRef
  source_profile_locator, source_account_id, private clone owner reference
  credential owner = Thunderbird; no direct token/password reference
```

- Use tagged enums instead of independent `oauth`, `goa_id`, `smtp_separate`, protocol booleans permitting impossible combinations. Keep the legacy `AccountConfig` as a translation adapter initially; do not make root GTK adopt an incompatible schema during native feature work.
- Email is an editable mailbox/display attribute, not a unique credential key or stable principal. Google identity is issuer + stable `sub`, not an unverified typed email ([G7]); Microsoft needs issuer/tenant + provider subject and explicit mailbox selection. Aliases/shared mailboxes may differ from grant owner.
- Preserve `profile_ids.toml` / Thunderbird `app_id` mappings and historical cache routing. Changing server, username, address spelling, reauth, or registration must not accidentally create a new cache namespace or make old results act on another mailbox.
- Store `grant_id/generation` keyring references in metadata, not tokens. Keyring namespace includes app + account/grant + purpose + generation. Separate IMAP password, SMTP override, alias credentials, refresh token, and any provider-required client credential.
- Do not put secret strings in `Clone`able public account/view snapshots. Short-lived lease types are redacted, non-serializable, minimize copying, and zeroize owned buffers where practical. Token values must not be cache map keys.
- Cap token-response bytes/lifetime, bind endpoints to a trusted provider catalog, reject redirects, redact OAuth descriptions, and distinguish an advanced user-approved custom issuer from known-provider settings. A valid HTTPS URL alone is not trust.
- Treat requested scopes as intent, granted scopes as evidence, and available mailbox capabilities as protocol/policy results. Never report a successful partial grant as fully connected.

### Small deep-module interface

Conceptual Rust-like shape; this is a design sketch, not source code or a request to implement every method immediately:

```rust
// AccountAuth owns transactions, secret leases, errors and grant persistence.
fn status(account: AccountId) -> AccountAuthStatus;
fn begin(intent: ConnectIntent) -> Result<AuthOperation, AuthError>;
fn credential(account: AccountId, usage: MailUsage)
    -> Result<CredentialLease, AuthError>;
fn disconnect(account: AccountId, policy: DisconnectPolicy)
    -> Result<DisconnectOutcome, AuthError>;
```

`ConnectIntent` is either Add(provider/backend/mailbox) or Reauthorize(existing AccountId). `AuthOperation` exposes only progress/result/cancel and the system-browser handoff; it consumes one setup transaction. The native UI never receives state/verifier/refresh-token strings. `credential` is a worker-internal interface, not a UI method, and can use the already supplied pending grant for a setup probe. Password/SMTP-no-auth/Thunderbird delegation are typed outcomes rather than pretending everything is a bearer token. Thunderbird returns a runtime-owner handle, **never** a `CredentialLease::OAuth`.

**Usage:** UI starts Add Google → receives progress → browser authorization → verified provider identity → read-only incoming and auth-only outgoing probes → keyring staging + atomic record commit → returns AccountId → native starts the existing direct worker. UI cancels: pending flow/probes close, no committed account changes. Worker requests credential for IMAP/SMTP or Graph; module handles cache/expiry/refresh/reauth events. UI starts Reauthorize(existing ID): current account remains in the roster; only a verified, matching identity and generation can replace its grant.

**Hidden complexity earning depth:** PKCE entropy, callback parser/deadlines, provider/issuer/resource checks, identity verification, keyring lock/error behavior, refresh single-flight, generation CAS, scope checking, expiry skew, restart-safe commit, deletion tombstones, and redaction concentrate in one module. Removing this module would scatter those invariants across onboarding, SMTP, IMAP, Graph, Settings, and reconnect.

**Test seams/dependencies:** pure state/parser logic is in-process; filesystem/keyring are local-substitutable adapters; provider/browser/clock/randomness are external dependency adapters injected internally. Use production HTTP/Secret Service adapters and deterministic synthetic adapters. Test through the module's interface rather than duplicating tests of private implementation state.

**Tradeoffs:**

- A lifecycle module is worthwhile because both direct and Thunderbird execution already vary; a speculative third replacement engine is not needed.
- Full-schema migration is riskier than a first Google slice. Add versioned auth metadata through a compatibility adapter first; parent owns the final migration/manifest wiring and should avoid two competing account authorities.
- Local-only direct auth avoids a token custody server and unnecessary centralization. OAuth registration approval is still external work; do not add a server merely to hide a desktop client secret.
- Preserve recoverability without indefinite credential retention: journal generations, keep active + in-flight versions, garbage-collect only after a proven commit and no leases, retain failed cleanup as a retryable tombstone.
- No send/probe framework should attempt exactly-once mail claims; account readiness and SMTP send ambiguity are separate domains.

### Credential state transitions and ownership

1. **Start:** allocate opaque operation ID/nonce/PKCE state, bind loopback before browser open, record expected provider/client/resource/target account. Pending transaction is memory-only and expires.
2. **Authorize:** browser may reuse a provider login session, but MegaMail must still obtain its own consent/grant. User can pick another account; typed input/login_hint is not proof of identity.
3. **Validate:** success/error callback must match transaction; consume code once. Validate token type/scopes, trusted issuer and identity; if using OIDC validate signature/JWKS, issuer, audience, expiry and nonce, not merely base64-decode. Compare reauth principal to existing issuer/subject/tenant; different account requires explicit separate add, not replacement.
4. **Probe pending grant:** authenticate/read-only incoming checks; SMTP connect/auth/quit only, no message/flag mutation. Missing sending policy can return a read-only capability. Do not persist simply to make the probe work.
5. **Commit:** stage secret(s), verify staged reads, atomically switch metadata generation/record, then report success/start worker. Crash recovery distinguishes abandoned stages from active grants. Metadata write failure leaves previous account usable.
6. **Use/refresh:** token leases keyed by grant + version + resource/scope, never raw refresh string. Single-flight with bounded retries; expiry skew; invalidate rejected token; persist strict rotation before assuming future refreshability. Late completion cannot overwrite reauth/remove/new-registration generation. Storage failure is a distinct degraded state, not silent success forever.
7. **Needs reauth:** invalid/revoked grant creates one actionable account state, stops retry storms, retains roster/cache/ID. Network/429/5xx errors back off; keyring-locked asks unlock, not consent. Administrator/CA denial has a policy-specific explanation. Reauth is explicit user action; no recurring automatic browser launches.
8. **Disconnect/remove:** cancel operations and stop workers/leases first; tombstone account/grant, delete all owned generations, invalidate token cache, retry keyring cleanup failures; explicitly choose keep/delete local cache. Remote revoke is a separate user-approved action and can fail offline. Never revoke Thunderbird's grant using MegaMail, delete the source profile, or imply local removal removed provider authorization.

## 6. Implementable first slice: direct native Google OAuth

**Deliverable:** Add/reauthorize one Google account using the system browser and a MegaMail-owned Desktop registration; authenticate direct IMAP/SMTP with XOAUTH2; restart from MegaMail's keyring without another import; report truthful typed reauth/policy/keyring states. Keep password setup and all Thunderbird accounts intact.

### Ordered implementation plan

1. **Harden and type the flow before wiring UI.** Preserve `Authorization` entropy/S256; implement bounded cancellable callback handler; validate success/error state, unique params and exact method/path/provider; bound/no-redirect token HTTP with sanitized typed errors. Replace URL substring provider detection (`oauth.rs:464,477`) with catalog policy. For stable Google principal binding, request `openid email` alongside `https://mail.google.com/`, generate a transaction nonce, retain and validate the returned ID token against Google's trusted issuer/JWKS, audience, expiry and nonce, and use verified issuer + `sub`; requesting only the existing mail scope does not produce the required identity evidence. Parse granted scopes/identity/access expiry instead of returning only refresh token. Fix Google's unconditional secret gate using current desktop registration contract. Do not force new consent on every reconnect.
2. **Add pending-grant connection probing and commit.** Do not call current `test_connection_blocking` with an unsaved OAuth record and expect `oauth_refresh` to work. Add a narrow probe path accepting a secret access-token lease with existing TLS/connect machinery; reuse read-only/auth-only operations. Keep token out of configuration until validated commit. Preserve keyring-first generation ordering and legacy cache IDs.
3. **Runtime refresh hardening.** Extract shared refresh coordination into a focused worker submodule, with production + synthetic dependency adapters; keep GTK signatures working. Single-flight and grant-version ownership checks are mandatory before extending strict-rotation providers. Google first may translate through current AccountConfig; explicitly record follow-up migration rather than silently making email the permanent grant identity.
4. **Native UI operation controller.** Add Google button only when registration is available; else show “Google sign-in requires a MegaMail registration” and truthful app-password/Thunderbird alternatives. Progress/cancel, browser-open failure, wrong identity, provider denial, keyring unlock, success after commit. Add “Sign in again” on the saved account without resetting account/cache ID. Do not connect this button to Thunderbird import.
5. **Startup account roster and repair.** Existing TB auto-restore remains. Show persisted unavailable accounts and typed reasons; manifest corruption is an error, not an empty profile. New direct account reload retrieves auth settings from its committed grant, not current env registration; switching client IDs requires fresh authorization.
6. **Acceptance gate owned by parent.** Run deterministic synthetic tests and serial Cargo checks; then, only with separate authorization, one live Google retrieval/restart/revocation test. Sending is a separately authorized verification, not part of “Test and add.”

**Do not include in the Google slice:** wholesale backend replacement, Graph migration, Yahoo/Fastmail OAuth release claims, embedded webview login, GOA autoimport, Thunderbird token export, Proton cloud login, broad UI parity work, remote revoke automation, or a distributed OAuth custody service.

### Microsoft is the next separate slice, not a generic preset switch

Choose one route explicitly:

- **Graph route:** use existing Graph worker, a Graph-specific probe, approved minimal Graph scopes, public-client registration/identity validation, tenant policy errors. Do not test its token against IMAP/SMTP.
- **Exchange IMAP/SMTP route:** use Exchange resource scopes from [M1], per-account capability checks, mailbox-versus-grant identity, and existing XOAUTH2 worker. It may fail legitimately when tenant protocols are disabled; do not silently fall back to Basic auth.

The current core preset is Graph-flavored; prefer keeping it labeled as such over silently changing its semantics for legacy callers.

### File ownership for safe later fanout

These are proposed ownership assignments, not changes made by this audit. **One owner per shared file.** No delegated worker may run Cargo concurrently with the parent.

| Work packet | Owned files | Integration reservation / exit criterion |
| --- | --- | --- |
| Flow/security | `crates/mail-core/src/oauth.rs`; new `crates/mail-core/src/auth_flow.rs`; tests colocated/new synthetic tests | No edits to UI/worker/config. Export typed cancellable outcome with no UI secrets. Parent wires `lib.rs` and dependencies. |
| Provider/domain/auth metadata | new `crates/mail-core/src/account_auth.rs`; `crates/mail-core/src/onboarding.rs`; `crates/mail-core/src/discovery.rs` | Canonical enums + explicit auth alternatives + Google secret contract; no competing config writer. Schema/compatibility reviewed before integration. |
| Durable credentials | `crates/mail-core/src/config.rs`; new `crates/mail-core/src/credential_store.rs` if extraction is justified | Pending→commit/rotation generation ownership, injected local stand-ins, safe cleanup inventory. Preserve public GTK-compatible functions used by shared worker. |
| Worker/probe/refresh | `src/worker.rs`; new `src/worker/oauth_token.rs`; future `src/worker/graph.rs` only if Microsoft slice starts | Shared root edits reviewed for GTK callers; ephemeral OAuth probe + single-flight + typed errors. Do not concurrently let a generic backend refactor own `worker.rs`. |
| Native UI | new `apps/megamail/src/auth_controller.rs`; `apps/megamail/src/onboarding.rs` | Operation/state integration skeleton against agreed interface; no persistence/token parsing in GPUI rendering. Parent reserves `main.rs`/`live.rs` edits or assigns them to this owner exclusively. |
| Parent-only integration | `crates/mail-core/src/lib.rs`, package manifests/locks, `apps/megamail/src/main.rs`, `apps/megamail/src/live.rs`, product/architecture docs | Serial Cargo, account roster/reconnect integration, public registration availability truth, root/native compatibility. Do not create independent schema/module-wiring edits in multiple tasks. |

Thunderbird adapter/runtime files should remain untouched in the Google slice except parent-owned roster/error integration. If Thunderbird repair is a separate task, assign `apps/megamail/src/thunderbird_adapter.rs`, `crates/mail-core/src/thunderbird_bridge.rs`, and extension files to a single explicit owner, not the OAuth implementer.

### Security/regression tests to write and have parent execute

No tests below were executed in this audit. Tests use synthetic addresses, keys and local stand-ins, never private accounts.

**Flow/callback:** RFC S256 vector and secure-random failure; loopback-only bind; listener exists before browser; exact path/method; wrong/missing/duplicate state/code; denial with wrong state; unsolicited favicon; oversized/partial requests; stalled peer; total timeout/cancel; replayed code; canceled operation late callback; no provider mixing; no success page before valid flow completion. Ensure authorization-code flow requests OIDC identity scopes/nonce when the identity validator expects an ID token; never silently accept a mail-only response as verified identity. Assert no token exchange/commit for rejected response.

**Provider/HTTP:** Google client ID alone obeys current public registration policy; missing ID disabled honestly; HTTPS/userinfo/fragment rejection; exact trusted catalog endpoints; no token-bearing HTTP redirect; bounded body; malformed JSON/empty tokens/wrong token type/partial scope; wrong issuer/audience/nonce/expiry/identity; sanitized errors do not include code/token/password/client credential; transient versus invalid-grant versus policy classification.

**Pending grant/ownership:** OAuth probe uses pending lease without keyring/account save; canceled/failed probe does not overwrite existing grant; reauth matching stable subject preserves AccountId/cacheId; wrong-account reauth cannot replace mailbox; changed registration must authorize again; SMTP policy-denied can be read-only rather than credential-corrupt; no SMTP DATA, IMAP STORE, draft or send during probe.

**Persistence:** keyring unavailable/locked/staging failure/readback mismatch; metadata fsync/rename failure; crash before/after commit; generation inventory recovery; CAS prevents stale refresh after reauth/remove; bounded orphan cleanup retries; deletion covers all owned generations/aliases, not unrelated accounts or Hylki service; metadata/Debug/logs contain no secrets. Failed metadata commit preserves old working generation.

**Runtime:** many concurrent IMAP/SMTP/Graph credential requests produce one refresh per grant/resource; expiry skew and clock advance; rotated refresh durably retained; strict old-token-reuse prevention; storage failure has typed outcome; invalid-grant generates one reauth event; no retry/browser storm for 429/5xx/offline/keyring lock; token cache evicts deleted/canceled grant and does not retain raw refresh-token keys.

**Discovery/startup:** Yahoo multiple `<authentication>` elements preserve OAuth and password methods; no automatic downgrade; custom-domain provider explicitly selected; stale review stamp cannot authorize changed endpoints; original ID maps preserved after reauth/server edits. Restart with direct-only/TB-only/mixed accounts, TB source missing/locked/changed, corrupt manifest, failed restore and partial success: keep saved roster and show reconnect/repair rather than mandatory reimport. Verify no source-profile mutation through synthetic clone fixtures.

**Platform:** native Linux and Flatpak system-browser portal success/failure/cancel + loopback permissions; no UI-thread I/O; cancellation does not leave listeners or subprocesses. Windows/macOS remain future targets, not inferred support.

## 7. Externally blocked versus code-local work

| Code-local now | External/manual gate |
| --- | --- |
| Flow hardening, typed state, callback/parser tests, secret lifecycle, first-slice UI/probe, registration-disabled state | Owned Google Cloud Desktop registration and consent config; public verified branding/privacy/required restricted-scope review; real test-account authorization. This audit read no actual registration or credentials. |
| Microsoft explicit resource/protocol model and synthetic flow | Owned Entra public client, supported audience/redirects, tenant/admin policy approval, identity/CA requirements. No guarantee a generic desktop client can satisfy managed-device policy. |
| Yahoo auth-alternative parsing + app-password guidance | Yahoo-approved consumer mail access/scopes/native redirect contract; account eligibility for app-password creation. |
| Fastmail password guidance and OAuth integration scaffolding | Manual Fastmail registration/contact, agreed protocol scopes, account entitlement. |
| iCloud app-password route and scoped reauth UX | Supported Apple Account mail-authorization onboarding for MegaMail is unverified; user 2FA/app password/entitlement. |
| Zoho server/plan-aware setup | Actual region/account plan/admin IMAP policy; IMAP OAuth contract not established here. |
| Proton loopback trust/profile mode | Installed/unlocked Bridge, paid plan, user-provided Bridge endpoints/password/certificate; no cloud credential extraction. |
| Preserve TB restore, source-manifest diagnostics | Installed supported Thunderbird 153+, source profile available; interactive Primary Password/provider login is not supported headlessly. |

Do not let absence of registration block secure parser/state/ownership work. Conversely, do not describe code completion as provider-ready login without the external gate.

## 8. Broader app gaps: scope checkpoint, not a new exhaustive parity audit

This task is the authentication/foundation packet, not the full Thunderbird/Spark/Superhuman product comparison. Existing product/architecture evidence already identifies rich composition, inline reply, bulk triage, full-history/all-folder/body search, broader provider policy coverage, and native account lifecycle as separate work. Preserve working Unified Inbox, scoped message keys, conversation references, privacy controls, direct durable Outbox, and both worker paths. Thunderbird runtime currently has no native Outbox retry/discard controls and loopback draft save does not establish remote Drafts synchronization.

Priority order: account/auth restoration and ownership → operational error/reconnect correctness → representative performance measurements → wider provider capability testing → remaining workflow features. Do not spend another fanout replacing foundational code already present, or promise a faster direct engine before measuring cold start, first headers/body, incremental sync, memory and reconnect against the retained Thunderbird path using authorized/representative workloads.

## Primary sources (retrieved during this audit)

### Google

- **[G1]** Desktop/iOS native OAuth (PKCE, loopback, token exchange/refresh, optional client secret): https://developers.google.com/identity/protocols/oauth2/native-app
- **[G2]** Gmail XOAUTH2 and full-mail scope: https://developers.google.com/workspace/gmail/imap/xoauth2-protocol
- **[G3]** OAuth token expiry/revocation/testing limits: https://developers.google.com/identity/protocols/oauth2
- **[G4]** Restricted-scope verification and conditional assessment/exceptions: https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification
- **[G5]** App-password eligibility/revocation: https://support.google.com/accounts/answer/185833
- **[G6]** Desktop loopback remains supported: https://developers.google.com/identity/protocols/oauth2/resources/loopback-migration
- **[G7]** OpenID Connect identity/ID-token verification: https://developers.google.com/identity/openid-connect/openid-connect

### Microsoft

- **[M1]** Exchange IMAP/POP/SMTP OAuth scopes and shared mailbox auth: https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth
- **[M2]** Redirect URI rules/localhost exceptions: https://learn.microsoft.com/en-us/entra/identity-platform/reply-url
- **[M3]** Refresh lifetime/replacement/revocation: https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens
- **[M4]** Authorization-code/PKCE, public versus confidential clients, resource scopes: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow
- **[M5]** Exchange Online Basic authentication removal and changing SMTP timeline: https://learn.microsoft.com/en-us/exchange/clients-and-mobile-in-exchange-online/deprecation-of-basic-authentication-exchange-online

### Other providers

- **[Y1]** Yahoo app passwords and creation eligibility: https://help.yahoo.com/kb/SLN15241.html
- **[Y2]** Current Yahoo mail developer-access approval/policies: https://senders.yahooinc.com/developer/developer-access/
- **[Y3]** Yahoo server-side code flow (legacy OOB examples are not native recommendations): https://developer.yahoo.com/oauth2/guide/flows_authcode/
- **[A1]** Current iCloud server settings (published 2026-02-03): https://support.apple.com/en-us/102525
- **[A2]** App-specific passwords and supported Apple Account authorization (published 2025-10-08): https://support.apple.com/en-us/102654
- **[F1]** Fastmail protocol servers/app passwords/Basic restrictions: https://www.fastmail.help/hc/en-us/articles/1500000278342-Server-names-and-ports
- **[F2]** Fastmail OAuth, manual registration, strict rotation: https://www.fastmail.com/dev/
- **[Z1]** Zoho IMAP configuration, datacenter/account distinctions and 2FA: https://www.zoho.com/mail/help/imap-access.html
- **[Z2]** Zoho current pricing/Forever Free exclusions: https://www.zoho.com/mail/zohomail-pricing.html (retrieved localized page; IMAP/POP/ActiveSync exclusion was present)
- **[P1]** Bridge paid plan/local IMAP/SMTP/separate password: https://proton.me/mail/bridge
- **[P2]** Bridge loopback/self-signed TLS: https://proton.me/support/bridge-ssl-connection-issue

### Mozilla and standards

- **[T1]** Mozilla autoconfiguration workflow (historical design reference, not proof all current TB behavior): https://wiki.mozilla.org/Thunderbird:Autoconfiguration
- **[T2]** Mozilla configuration format, auth alternatives: https://wiki.mozilla.org/Thunderbird:Autoconfiguration:ConfigFileFormat
- **[T3]** Current Mozilla provider mapping implementation (moving master reference; conceptual comparison only, no client-ID reuse): https://github.com/mozilla/releases-comm-central/blob/master/mailnews/base/src/OAuth2Providers.sys.mjs
- **[T4]** Current public Yahoo configuration with repeated authentication elements: https://autoconfig.thunderbird.net/v1.1/yahoo.com
- **[T5]** Current public iCloud configuration with separate usernames: https://autoconfig.thunderbird.net/v1.1/icloud.com
- **[R1]** RFC 8252, native external user-agent/loopback/PKCE and public client treatment: https://www.rfc-editor.org/rfc/rfc8252.html
- **[R2]** RFC 9700, OAuth Security BCP (CSRF/mix-up/PKCE/refresh protection): https://www.rfc-editor.org/rfc/rfc9700.html

### Retrieval limitations

Mozilla Primary Password support URL returned a challenge page, so no primary-password documentation claim relies on that fetch; the headless limitation is established by MegaMail source/product evidence. The old Proton `/support/bridge` index did not provide the required setup facts; [P1]/[P2] supplied usable first-party content. Fastmail ISPDB lookup returned 404; do not assume it discovers automatically. No app registrations, policy enforcement, Thunderbird runtime version, or live mail behavior was inspected or tested. Provider support must remain qualified until separately verified.
