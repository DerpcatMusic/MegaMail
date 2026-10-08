# Account onboarding research: Thunderbird discovery and OAuth

Research checked 2026-10-08 against the Hylki checkout and current Thunderbird/Mozilla source and documentation.

## Recommendation

Reuse Thunderbird’s *autoconfiguration format and ISPDB as a source of server settings*. Do not embed Thunderbird as MegaMail’s mail runtime or depend on Thunderbird’s OAuth client registrations. Thunderbird’s account wizard and mail account manager run inside Gecko and its privileged JavaScript/XPCOM environment; its configuration XML is the portable piece. OAuth is a second, provider-specific integration: XML can say `OAuth2`, but a client still needs a provider mapping, an app registration, scopes, browser redirect handling, and secure token storage.

The practical flow is: accept an email address, discover and validate a server-settings candidate, show the account and security details, then run a MegaMail-owned sign-in flow for a provider MegaMail supports or offer the provider’s IMAP/app-password/manual path. Discovery can cover many domains; it cannot turn every domain into a one-click OAuth login.

## What Hylki already does

Hylki’s account editor uses a fixed `ProviderKind` model for manual IMAP/POP, known server presets, Google OAuth, Microsoft OAuth, custom OAuth and JMAP ([`src/ui/accounts.rs:19-40`](../../src/ui/accounts.rs#L19)). Its [`PROVIDERS` table](../../src/ui/accounts.rs#L145) includes direct Google and Microsoft sign-in choices, a range of IMAP presets, and special entries such as Proton Bridge at localhost ([`src/ui/accounts.rs:145-162`](../../src/ui/accounts.rs#L145)). I found no ISPDB/autoconfig lookup implementation in Hylki’s `src`; unknown domains are handled through manual server fields/custom OAuth rather than domain discovery.

The existing OAuth implementation is useful as a *behavioral reference*: Hylki opens the system browser, listens on loopback, runs authorization-code + PKCE S256, checks the returned `state`, exchanges the code, and keeps the refresh token in the system keyring ([`src/oauth.rs:403-455`](../../src/oauth.rs#L403), [`src/oauth.rs:467-489`](../../src/oauth.rs#L467), [`src/config.rs:422-433`](../../src/config.rs#L422)). Hylki’s keyring service is specifically `co.hyprlab.Hylki` ([`src/config.rs:270-271`](../../src/config.rs#L270)); MegaMail should use its own stable service/application identity and account-key namespace.

Hylki’s Google endpoints and Gmail IMAP host/scope are in [`src/oauth.rs:188-199`](../../src/oauth.rs#L188). Its public source intentionally has no built-in Google client ID/secret unless supplied at build time, and its documentation says official builds use GNOME Online Accounts or the user’s own client ([`src/oauth.rs:27-42`](../../src/oauth.rs#L27), [`docs/DOCUMENTATION.md:320-343`](../DOCUMENTATION.md#L320)). This matches the core product constraint: MegaMail needs its own provider registration and must not copy Thunderbird’s or Hylki’s client credentials.

## Reusable Thunderbird surface

The Thunderbird ISPDB repository is the database of mail-provider configuration files and is marked MPL-2.0 ([repository](https://github.com/thunderbird/autoconfig), [license](https://github.com/thunderbird/autoconfig/blob/master/LICENSE)). Its current wiki lists several independent clients that use autoconfig/ISPDB, including Evolution, Geary, FairEmail and Thunderbird ([Thunderbird autoconfig wiki](https://github.com/thunderbird/autoconfig/wiki)). The XML format describes provider domains, incoming IMAP/POP settings, outgoing SMTP settings, port, socket security, authentication kind, and username placeholders. Its specification explicitly describes the format as stable and usable by other mail clients ([Mozilla autoconfig XML specification](https://wiki.mozilla.org/Thunderbird:Autoconfiguration:ConfigFileFormat)). This is a configuration-data protocol, not Thunderbird’s mail engine.

Current Thunderbird source constructs provider-domain lookups at `https://autoconfig.<domain>/mail/config-v1.1.xml` and `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`; it also fetches a configured central ISPDB URL with the sanitized domain ([`FetchConfig.sys.mjs`](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/accountcreation/modules/FetchConfig.sys.mjs#L89-L180)). Thunderbird can additionally use MX-derived domains, but its own source describes that as an approximation because an MX host is not necessarily the IMAP host ([`FetchConfig.sys.mjs:183-207`](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/accountcreation/modules/FetchConfig.sys.mjs#L183)). Use ISPDB and HTTPS provider-hosted configuration as candidates; treat MX-derived guesses as weaker hints, not authoritative settings.

The XML can advertise `OAuth2`, but that does not carry a complete login integration. Thunderbird’s parser accepts that authentication value only when its separate `OAuth2Providers` registry recognizes the server hostname and protocol ([`readFromXML.sys.mjs:350-379`](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/accountcreation/modules/readFromXML.sys.mjs#L350)). That registry maps server hostnames to issuers and scopes, and the source states provider client details are hard-coded because dynamic client registration is unsupported; it explicitly tells downstream applications to register their own client ([`OAuth2Providers.sys.mjs`](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/base/src/OAuth2Providers.sys.mjs#L2142-L2159)). Therefore ISPDB helps MegaMail find IMAP/SMTP settings, but it cannot supply MegaMail’s Google OAuth registration or authorize MegaMail to use Thunderbird’s.

The rest is not a small embeddable library. Thunderbird’s parser imports `resource:///` modules, `ChromeUtils`, `Services`, `Ci` and `nsMsgAuthMethod`; Thunderbird’s account manager is exposed through Gecko’s `MailServices.accounts` and `nsIMsgAccountManager` APIs ([Thunderbird account-configuration architecture](https://github.com/thunderbird/developer-docs/blob/master/thunderbird-development/codebase-overview/account-configuration.md)). The practical inference from these source dependencies is that porting the account-creation modules into Rust/GPUI means reimplementing their narrow data conversion and trust checks. Running or embedding all of Thunderbird would bring the Gecko application/profile/mail stack with it and would make MegaMail a Thunderbird shell rather than a GPUI mail client.

## OAuth requirements and Gmail limits

Gmail supports OAuth 2.0 over standard IMAP/POP/SMTP using SASL XOAUTH2; its documentation says the scope is `https://mail.google.com/` and describes that scope as full email access, including read, compose, send and permanent delete ([Google Gmail IMAP/SMTP](https://developers.google.com/workspace/gmail/imap/imap-smtp), [Google XOAUTH2 scope and mechanism](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)). Public apps using Google data scopes must go through Google’s OAuth verification process; restricted-scope access also has additional requirements and may require a security assessment ([Google app verification](https://support.google.com/cloud/answer/13463073), [verification requirements](https://support.google.com/cloud/answer/13464321)). This is a real launch dependency for one-click Gmail IMAP, not a benefit inherited from Thunderbird.

For a native desktop client, use the user’s external/system browser, authorization-code flow with PKCE, and a loopback redirect. RFC 8252 requires PKCE for public native clients, recommends the external browser and documents loopback redirects for desktop apps; it prohibits embedded user-agents for OAuth authorization ([RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)). Google continues to support loopback redirects for OAuth clients registered as desktop apps ([Google loopback-flow migration guide](https://developers.google.com/identity/protocols/oauth2/resources/loopback-migration)). Register MegaMail’s own Google OAuth client, request only scopes the product actually needs, and keep refresh tokens in MegaMail’s OS credential store. A bundled native-app client ID is public metadata; a distributed app cannot keep a client secret confidential.

For broad coverage, make OAuth provider integrations explicit (initially Gmail and whichever other providers have approved MegaMail registrations), while treating discovered IMAP settings and provider passwords/app-passwords as a separate path. Some providers disable IMAP until enabled by the user, require app passwords, or need a local bridge; no universal authorization page exists for arbitrary email domains. Thunderbird’s own help says accounts absent from its database need manual configuration ([Automatic Account Configuration](https://support.mozilla.org/en-US/kb/automatic-account-configuration), [Manual Account Configuration](https://support.mozilla.org/en-US/kb/manual-account-configuration)).

## Optional read-only Thunderbird profile import

The local **Import Thunderbird settings** action can prefill server and
identity fields without using Thunderbird as a login provider. On Linux the
documented default profile root is `~/.thunderbird`; the Flatpak package stores
its persistent profile under
`~/.var/app/org.mozilla.Thunderbird/.thunderbird` ([Mozilla profile guide](https://support.mozilla.org/en-US/kb/profiles-where-thunderbird-stores-user-data),
[Flatpak packaging](https://github.com/flathub/org.mozilla.Thunderbird/blob/master/org.mozilla.Thunderbird.yaml)).
Thunderbird's account manager records the account list in
`mail.accountmanager.accounts`, maps each `mail.account.<id>` to a server, and
associates identity IDs with each account ([account-manager source](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/base/src/nsMsgAccountManager.cpp#L2328-L2347),
[account interface](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/base/public/nsIMsgAccount.idl)).
Incoming `hostname`, `port`, `username`, `authMethod`, and `socketType` are
settings on the incoming-server object ([interface](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/base/public/nsIMsgIncomingServer.idl));
identities provide `useremail`, `fullName`, and the selected SMTP server
([identity interface](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/base/public/nsIMsgIdentity.idl),
[identity preference mapping](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/base/src/nsMsgIdentity.cpp)).
SMTP host/port are maintained by Thunderbird's SMTP server settings, with
`try_ssl` and `authMethod` values in the account preferences
([SMTP interface](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/compose/public/nsISmtpServer.idl),
[current socket/auth enum definitions](https://github.com/thunderbird/thunderbird-desktop/blob/main/mailnews/base/public/MailNewsTypes2.idl)).

MegaMail's importer reads only `profiles.ini` and the selected profile's
`prefs.js`, with file-size and item-count bounds. Its small parser accepts
only `user_pref("key", string-or-number-or-boolean);` statements and never
evaluates JavaScript. It ignores non-IMAP account types and returns profile,
IMAP/SMTP host, port, TLS/auth mode, username, and identity candidates. A user
must explicitly open the import flow, choose a profile/account, review the
server values, and complete MegaMail's own onboarding. No Thunderbird password
store, OAuth refresh token, browser session, or local mail cache is imported;
OAuth2 in an imported setting is only an auth-method hint, not authorization
for MegaMail to use Thunderbird's token.

This import is a convenience for migrating settings, not for authenticating a
new MegaMail account. The user enters a provider-approved app password or uses
a MegaMail-registered OAuth flow; Thunderbird's client IDs and stored OAuth
identity are not portable credentials.

## Trust boundaries for MegaMail discovery

Thunderbird’s current source contains an explicit warning: direct provider autoconfig may rely on insecure DNS and HTTP. The implementation can append `http://` candidates when its `sslOnly` setting is false ([`FetchConfig.sys.mjs:49-74`](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/accountcreation/modules/FetchConfig.sys.mjs#L49), [`FetchConfig.sys.mjs:89-145`](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/accountcreation/modules/FetchConfig.sys.mjs#L89)). MegaMail should keep discovery HTTPS-only with normal certificate validation. If HTTPS discovery fails, show manual setup or provider help; do not retry the same configuration fetch over plaintext HTTP.

Fetched XML and all advertised mail hosts are untrusted input. Thunderbird’s parser itself says it can validate syntax but cannot know whether the advertised hostname is a good server ([`readFromXML.sys.mjs:17-30`](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/accountcreation/modules/readFromXML.sys.mjs#L17)). MegaMail should parse a bounded XML subset into typed values, reject unknown/invalid protocol, authentication, port and TLS values, and never execute XML, expand external entities, or fetch arbitrary URLs named in the document. Construct discovery URLs only from a normalized email domain and fixed HTTPS paths; restrict redirects and response size. Avoid sending the full email address in the URL unless a provider-specific flow requires it.

Before attempting any credentialed connection, show the candidate incoming/outgoing hosts, ports and TLS mode and require the user to accept them. In this desktop-client context, the SSRF-like risk is that remote settings can make MegaMail probe a local/private service or send credentials to an unexpected server. Treat a remote document that points to loopback, link-local, or private-network addresses as suspicious; require an explicit user choice before connecting, while preserving deliberate local setups such as Proton Bridge through a bundled provider preset or manual configuration. Preserve certificate validation and STARTTLS requirements; never silently turn off TLS or accept a certificate/name mismatch. If the provider advertises plaintext authentication or cannot establish TLS, stop and explain the risk rather than sending credentials.

## Suggested implementation boundary

Keep the discovery adapter in the GPUI app’s Rust domain layer and return a value such as `AccountCandidate { source, protocol, incoming, outgoing, auth_hint, username_template }`. It should not own credentials or tokens. The onboarding UI can display candidate provenance and security details; a separate provider-auth layer chooses MegaMail’s registered OAuth integration or password/app-password/manual setup; the account repository persists metadata, while the OS keyring stores secrets under a MegaMail-specific namespace. This preserves a simple one-address-first experience without coupling GPUI to Gecko or treating untrusted ISPDB XML as executable configuration.
