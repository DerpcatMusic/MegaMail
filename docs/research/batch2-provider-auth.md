# Batch 2: provider authorization transaction (F3/F4)

## Evidence and status

This batch adds a reusable, executable worker-side authorization transaction,
not a shipped native sign-in button. **No real Google/Microsoft login, provider
probe, send, keyring operation, private-profile access, Cargo command, compiler,
helper build, commit or push was performed.** The parent owns module declaration,
native integration and the single batch build window after all owners freeze.

Read the repository instructions/product/design/architecture, feature matrix,
provider-auth and account-lifecycle foundation reports, and the engine/reader/
latency foundation summaries. Graft was queried first; its empty graph supplied
no nodes. Used installed Rust guidance, existing dependency source and current
Microsoft identity documentation through Context7. No credentials or borrowed
Thunderbird registration values were used.

### Actual owned changes

- `crates/mail-core/src/provider_auth.rs` (new): app-owned registration policy,
  explicit Google IMAP/SMTP versus Microsoft Graph selection, attempt generation
  and cancellation, one-use browser transaction, fixed-HTTPS authenticated
  identity queries, bounded injected HTTP, scope/token validation, pending-token
  production probes, add-only persistence adapter and pending-grant renewal.
- `crates/mail-core/src/oauth.rs`: additive worker-local borrowed code-exchange
  form and remaining-budget callback wait; fallible browser-open helper while
  preserving `open_uri`'s signature. The strict callback parser/wait implementation
  and existing GTK-facing OAuth result/function signatures are unchanged.
- `crates/mail-core/src/onboarding.rs`: callable native transaction constructor;
  missing-registration copy explicitly names the owned-client requirement.
  Existing password/manual/app-password paths and Graph-flavored legacy preset
  are retained.
- `crates/mail-core/tests/provider_auth_contract.rs` (new): fictional HTTP,
  browser, pending-probe and persistence adapters exercise the actual transaction
  and its strict loopback callback path. They are not a second implementation of
  the authorization state machine.
- This document.

No edits to `config.rs`, crate roots, manifests, worker, Thunderbird, cache,
outbox, native UI or account-session ownership code were made by this owner.

## Protocol and identity choices

| Route | Mail protocol / permissions | Identity evidence | Limits |
| --- | --- | --- | --- |
| Google | Gmail IMAP 993 and SMTP 465, implicit TLS, XOAUTH2; `https://mail.google.com/` plus `openid email` | Bearer GET to fixed `https://openidconnect.googleapis.com/v1/userinfo`; nonempty `sub`, `email_verified=true`, valid email; email must match requested mailbox | No JWT payload decoding or unverified OIDC identity. Workspace admin policy and restricted-scope release verification remain external. Aliases are not accepted as a substitute for the authenticated primary email. |
| Microsoft | **Graph**, not Exchange IMAP/SMTP; User.Read, Mail.ReadWrite, Mail.Send, MailboxSettings.ReadWrite, offline_access | Bearer GET to fixed Graph `/v1.0/me?$select=id,displayName,mail,userPrincipalName`; nonempty Graph object id and valid nonempty primary `mail` matching requested mailbox | Personal=`consumers`, work/school=`organizations`, mixed=`common`; selected authority enforces audience at the provider. No claim of verified `tid`/issuer from a JWT. No pinned-tenant allowlist, shared mailbox, UPN-only identity, or managed-device broker flow. |

Graph permissions are chosen for the existing Graph backend, including category
settings; they are not described as least-privilege read-only permissions. A
Graph Inbox-folder GET verifies mailbox accessibility but does **not** prove that
Mail.Send/Mail.ReadWrite operations will succeed under every tenant policy. No
mutating probe is used to prove that. Google incoming and outgoing auth must both
pass; this slice does not add a read-only partial-capability account.

Microsoft Exchange IMAP/SMTP would require a separate route with
`https://outlook.office.com/IMAP.AccessAsUser.All` and
`https://outlook.office.com/SMTP.Send` and a trustworthy identity strategy for that
resource. **Do not put the new Graph scopes or the legacy Microsoft preset into
an IMAP account.** This batch deliberately does not implement Exchange IMAP.

The browser flow uses existing secure randomness and S256 PKCE. It binds a random
127.0.0.1 listener before browser open. Google redirect is literal loopback with
root slash; Microsoft uses registered localhost form without trailing slash.
Both success and denial pass the existing state/method/path/unique-parameter
validator. Consuming Transaction closes the listener and prevents code reuse.
The callback page remains “authorization received / not connected yet.”

Token parsing requires Bearer, nonempty bounded tokens, explicit granted-scope
evidence and a 60–86,400-second expiry. Omitted scope is blocked rather than
assumed; OAuth technically permits some omissions, so this conservative rule can
reject otherwise valid provider responses. A refresh token is mandatory for new
accounts; pending renewal retains the old one only when the response omits a
replacement. No code, token or raw provider/storage diagnostic appears in typed
errors or pending-grant Debug. Secret strings are not guaranteed memory-zeroized.

## Public native integration contract

**Parent must add `pub mod provider_auth;` to `crates/mail-core/src/lib.rs` before
its batch build.** No dependency addition is needed. New onboarding references
that root module, so this owner intentionally does not claim the currently
undeclared module compiles before parent integration.

Use one AuthController for the setup surface. Call begin, constructor,
authorize, renew, account-store creation and commit on a blocking worker. The UI
may hold/clone Attempt and call its nonblocking cancel method. UI receives only
fixed AuthError, safe metadata, and SavedAccount; keep ReadyAccount and injected
HTTP/browser/probe/store objects in the worker.

```rust
// Worker-local outline, not a GUI-thread action or a separate fake flow.
let attempt = controller.begin(std::time::Duration::from_secs(300));
let pending = onboarding::provider_authorization(
    provider_auth::Provider::GoogleImapSmtp, &requested_email, attempt.clone(),
)?;
let ready = pending.authorize(
    &mut provider_auth::ProviderHttp,
    &mut provider_auth::SystemBrowser,
    &mut provider_auth::ProviderProbe,
)?;
// Before the next call, native lifecycle owner holds its write lock, loads a
// CHECKED current roster, checks both direct/TB duplicates, and reserves stable
// account/cache IDs against the complete cross-backend inventory.
let mut store = provider_auth::ConfigAccountStore::new(checked_current_roster);
let saved = ready.commit(&mut store); // success = SAVED, not CONNECTED
```

For Microsoft choose `Provider::MicrosoftGraph(MicrosoftAudience::Personal)` or
WorkOrSchool/PersonalAndWork explicitly. Do not infer provider solely from a
custom email domain. Setup can show a generic “Finishing sign-in” state; this
contract does not currently emit per-stage progress callbacks.

The real ProviderProbe uses **pending access tokens**, not
`worker::test_connection_blocking` or `config::store_oauth_refresh` as a staging
workaround. Google uses async-native-tls and async-imap auth/logout, then lettre
XOAUTH2 connection/auth/NOOP (no SMTP envelope/DATA). Graph uses a bounded Inbox
folder GET, never the old probe's IMAP fallthrough. Protocol work runs in a
current-thread Tokio runtime on the blocking worker. No mail bodies are fetched
or cached by setup. TLS hostname/certificate validation stays enabled.

HTTP has no redirects, a 64 KiB response cap, a 20-second operation limit and the
remaining transaction budget. Browser portal wait is capped at 10 seconds before
xdg-open fallback. Callback reads/writes use the unchanged strict bounded wait.
Google probe has a 40-second whole-probe cap and polls cancel/staleness every
20 ms. The transaction has a five-minute cap checked between stages. Cancellation
does not interrupt an in-flight blocking HTTP request; late results are checked
and rejected before probing or saving. OS DNS/runtime behavior and real portal/
Flatpak behavior still need acceptance testing.

Commit serializes against a new attempt's begin. Nonblocking cancel after the
last pre-save check cannot undo an atomic save. **Accept and display SavedAccount
in that race; do not silently ignore success because the UI cancel flag is set.**
A new attempt starts only after the earlier commit returns. Credential-store
calls are not forcibly interrupted by the transaction timer once commit starts.
The UI may need a “Finishing save” state until the actual result arrives.

After SavedAccount, reload through the native stable-ID/profile owner, start the
existing matching worker, and wait for its account-scoped successful mail events
before showing connected. Returned metadata contains no pending refresh token
or optional Google client secret; actual values are persisted by config::save in
MegaMail's private generation/keyring mechanism. SavedAccount is not permission
to send mail or proof of provider-wide protocol compatibility.

## Credential lifecycle / config-owner requests

This is an **add-only transaction**. ConfigAccountStore rejects an existing
normalized email; it must not be used as reauthorization or a backend migration.
A duplicate Thunderbird mailbox must also be rejected by the native roster owner
before commit. Stable issuer/subject metadata is returned, but AccountConfig has
no field to persist it in this batch. Do not pretend the legacy email key is a
new canonical grant ID.

ConfigAccountStore calls existing config::save with a checked roster provided by
the caller. It deliberately does not call config::load().unwrap_or_default(),
which can overwrite an unreadable roster. config::save stages a credential
generation before atomically renaming metadata; on reported failure its prior
active metadata is retained. CommitFailure returns the pending grant for explicit
retry until its attempt/token expires. Dropping it discards the in-memory grant;
it does not revoke remote consent. Failed staged generations may remain in the
keyring; their inventory/cleanup belongs to the config owner, not this module.

ReadyAccount::renew consumes the pending object, refreshes through the same HTTP
seam, validates scopes and stable authenticated identity, and re-probes. Errors
consume/drop the pending grant so an unvalidated rotated token cannot be saved
or the old token silently retried. On success the rotated refresh is retained for
later config::save. Store failure likewise retains it in CommitFailure. New
access tokens are not written to account metadata.

**After commit the existing worker's private keyring load/refresh/replacement
path remains in use.** This batch does not fix its cross-worker single-flight,
raw-refresh-token cache key, generation-CAS, typed keyring-load error or stale
refresh-after-reauth/remove concerns. Do not claim runtime token lifecycle is now
fully hardened just because pending renewal is tested in source.

Requests to parent/config/lifecycle owners before enabling native release:

1. Serialize roster load + duplicate check + stable-ID reservation + save with
   ALL lifecycle writes. The existing public save is atomic for one write, but
   not an atomic checked-add against a concurrently changing roster. Prefer an
   injected checked `add_account_if_roster_revision_matches` operation inside
   config, or enforce the native lifecycle lock across the whole sequence.
2. Persist provider/client/resource/authenticated subject metadata separately
   from tokens. A future reauthorization must match principal + account/grant
   generation, retain stable IDs/cache/outbox, and never replace by email alone.
3. Provide checked credential read and compare-and-swap rotation APIs before
   centralizing committed-worker refresh. Coordinate IMAP/SMTP/Graph concurrent
   refresh and remove/reauth completion; don't reintroduce Thunderbird tokens.
4. Journal/clean failed staged generations safely, including deletion retries.
   This packet does not promise complete local erasure or remote revocation.

These are integration/release obligations, not edits made to another owner's
file and not reasons to hold this implementation waiting on UI work.

## Independent external/manual release gates

- **Registration gate:** no client ID is bundled. MEGAMAIL_* / MegaMail oauth.toml
  / build metadata must contain **MegaMail's** actual Desktop/public client.
  `from_owned_client` is an ownership-attested injection boundary, not a provider
  ownership/approval verifier. Google desktop secret is optional; when supplied
  it is sent only to the token endpoint and saved privately. Microsoft public
  clients have no secret. Never copy Thunderbird IDs, grants or private keys.
- **Google approval:** owned Desktop registration, branding/consent/privacy and
  restricted Gmail mail scope review; testing-mode grants/policy restrictions
  remain provider-specific. There is no authorized Google login evidence here.
- **Microsoft approval:** owned public desktop/mobile registration, supported
  personal/work audience, exact localhost redirect/platform and tenant admin/
  conditional-access approval. A decoded token or an email suffix cannot prove
  tenant identity; pinned-tenant and broker support are not implemented.
- **Provider certification:** after code/build gates, separately authorize real
  account retrieval, restart/refresh and revoked-consent/locked-keyring checks.
  SMTP delivery/Graph send tests require separate explicit send approval.
- **Manual alternatives:** retain Gmail app-password where account policy permits
  and custom TLS IMAP/manual/separate SMTP/no-auth paths. iCloud can use an
  app-specific password with Apple Account 2FA (IMAP imap.mail.me.com:993; SMTP
  smtp.mail.me.com:587 STARTTLS, independently editable usernames). MegaMail's
  supported Apple mail-authorization registration is unverified; general Sign
  in with Apple is not iCloud IMAP authorization. Microsoft work-account Basic
  auth/app-password fallback is not a general supported workaround.

## Validation and honest assessment

Executed: direct rustfmt of all four owned Rust files, rustfmt checks and owned-file
`git diff --check`. A Python static check also passed owned-file whitespace and
seven source-contract assertions (18 test declarations, existing callback/form
seam, config::save use, no premature refresh-keyring read/write, parent module
still undeclared). These are parse/format/whitespace/source checks, **not
compilation, Rust test execution, provider simulation results or runtime
verification**.

18 Rust contract tests were written (not executed): registration missing/secret
rules; optional-secret redaction; real strict callback wrong-state then valid;
identity/connection before persistence; Graph authority/protocol separation;
correlated tenant denial and typed HTTP policy denial; mismatched/unverified
identity; missing/wrong-resource scopes; pre-browser/in-flight cancellation;
stale identity and cancelled probe; stale/cancelled ready save refusal; storage
failure + prior-active retention + rotated pending retry; renewed identity change;
duplicate add refusal without config/keyring access; failed SMTP probe; bounded/
malformed/non-Bearer token response; production-probe rejection of out-of-catalog
hosts before any network/token disclosure; zero-budget expiry before launch.

Parent's single batch build should execute the integration test and existing
OAuth/onboarding tests after module declaration. No claim is made that these new
Rust tests are green. They cross the actual Transaction/ReadyAccount interfaces,
with bounded loopback sockets and injected external adapters, not real provider
or TLS/SMTP servers. The production adapters' TLS certificates, IMAP/SMTP server
handshakes, portal/browser, actual config save failures and real keyring behavior
still require separate synthetic adapter acceptance fixtures or authorized tests.

**Good:** coherent executable provider transaction; identity and scope gate;
explicit Graph choice; pending probes avoid credential overwrite; single-use code,
stale/cancel checks, secret-redacted diagnostics and retryable storage failure;
existing GTK/password/Thunderbird/cache/outbox/privacy boundaries untouched.

**Bad/incomplete:** module/UI not yet integrated; Rust compilation/tests unexecuted;
add-only, no persisted subject/tenant schema or reauthorization; existing committed
worker refresh defects remain; no read-only partial-capability onboarding; strict
scope/primary-mail rules can reject otherwise valid accounts.

**Risk:** caller-supplied roster must be serialized; staged keyring orphans;
commit/cancel finalization race must be presented honestly; provider approvals,
real registered redirect behavior and policy/portal/TLS behavior are unverified.
No performance improvement or full provider compatibility is claimed.

**Blockers:** parent module declaration and build acceptance; native lifecycle/
roster/ID integration; actual MegaMail registrations/approvals; explicit authorized
provider certification. These gates are independent.

## Sources

- https://developers.google.com/identity/protocols/oauth2/native-app
- https://developers.google.com/identity/openid-connect/openid-connect
- https://developers.google.com/workspace/gmail/imap/xoauth2-protocol
- https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow
- https://learn.microsoft.com/en-us/entra/identity-platform/userinfo
- https://learn.microsoft.com/en-us/graph/api/user-get
- https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens
- https://learn.microsoft.com/en-us/entra/identity-platform/reply-url
- https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth
- https://support.apple.com/en-us/102525
- https://support.apple.com/en-us/102654

Provider restrictions/Google and Apple references are also documented in the
source-linked foundation-provider-auth report; this batch independently queried
Microsoft identity docs and inspected pinned local ureq/zbus/async-imap/lettre
sources for the APIs used. Linked sources are not evidence of a tested registration.
