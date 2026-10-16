//! Native OAuth2 (XOAUTH2) sign-in for MegaMail accounts.
//!
//! Runs the authorization-code flow with PKCE against the provider: opens the
//! system browser, captures the redirect on a loopback socket, and exchanges the
//! code for a refresh token. The refresh token is kept in the keyring; a fresh
//! access token is minted from it at connect time.
//!
//! Client IDs come from a MegaMail-owned registration at build time, from the
//! user's environment, or from `oauth.toml` in MegaMail's configuration
//! directory. The source tree deliberately contains no provider client IDs.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::config::OAuthSettings;

/// Accept only credential endpoints that send OAuth data over HTTPS. Return a
/// field-specific error without reflecting the configured URL into the UI/log.
fn validate_https_endpoint(url: &str, label: &str) -> Result<(), String> {
    let valid = glib::Uri::parse(url, glib::UriFlags::NONE).is_ok_and(|uri| {
        uri.scheme().as_str().eq_ignore_ascii_case("https")
            && uri
                .host()
                .is_some_and(|host| !host.as_str().trim().is_empty())
            && uri.userinfo().is_none()
            && uri.fragment().is_none()
    });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "OAuth {label} must be a valid HTTPS URL with a hostname and no user information or fragment."
        ))
    }
}

fn validate_oauth_endpoints(settings: &OAuthSettings) -> Result<(), String> {
    validate_https_endpoint(&settings.auth_url, "authorization endpoint")?;
    validate_https_endpoint(&settings.token_url, "token endpoint")
}

// Provider-specific redirect/consent behavior must not be selected by a
// hostname lookalike or by text in a custom endpoint's path/query.
fn token_endpoint_has_host(url: &str, expected_host: &str) -> bool {
    glib::Uri::parse(url, glib::UriFlags::NONE).is_ok_and(|uri| {
        uri.scheme().as_str().eq_ignore_ascii_case("https")
            && uri
                .host()
                .is_some_and(|host| host.as_str().eq_ignore_ascii_case(expected_host))
            && uri.userinfo().is_none()
            && uri.fragment().is_none()
    })
}

fn validate_client_id(settings: &OAuthSettings) -> Result<(), String> {
    if settings.client_id.trim().is_empty() {
        Err("OAuth client ID is not configured. Set a MegaMail-owned client registration or provide your own client in oauth.toml.".into())
    } else {
        Ok(())
    }
}

fn token_request_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(status, _) => {
            format!("OAuth token request failed with HTTP status {status}.")
        }
        ureq::Error::Transport(error) => {
            format!("OAuth token request failed: {}.", error.kind())
        }
    }
}

fn oauth_agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| ureq::AgentBuilder::new().redirects(0).build())
}

// MegaMail owns no provider registration yet. Official builds leave these
// empty; build-time and runtime overrides allow a registration to be supplied
// without committing its credentials to the source tree.
// Google falls back to Thunderbird's desktop registration, which is also what
// accounts migrated from Thunderbird were issued under.
const GOOGLE_CLIENT_ID: &str = match option_env!("MEGAMAIL_GOOGLE_CLIENT_ID") {
    Some(v) => v,
    None => crate::thunderbird_migrate::THUNDERBIRD_GOOGLE_CLIENT_ID,
};
const GOOGLE_CLIENT_SECRET: &str = match option_env!("MEGAMAIL_GOOGLE_CLIENT_SECRET") {
    Some(v) => v,
    None => crate::thunderbird_migrate::THUNDERBIRD_GOOGLE_CLIENT_SECRET,
};
// Microsoft is a public client (PKCE, no secret). Keep the default empty until
// MegaMail has its own registered client; never borrow another mail client's ID.
const MICROSOFT_CLIENT_ID: &str = match option_env!("MEGAMAIL_MICROSOFT_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
const MICROSOFT_CLIENT_SECRET: &str = "";
// Dropbox's public client uses PKCE too. A MegaMail-owned key may be supplied
// at build/runtime or in oauth.toml.
const DROPBOX_CLIENT_ID: &str = match option_env!("MEGAMAIL_DROPBOX_CLIENT_ID") {
    Some(v) => v,
    None => "",
};

/// Dropbox matches loopback redirect URIs exactly, port included, so the
/// listener for its sign-in is on this fixed port and the app's registered
/// redirect URI is `http://localhost:41597/`.
pub const DROPBOX_REDIRECT_PORT: u16 = 41597;

// A callback confirms neither token exchange nor durable account setup.
const CALLBACK_RECEIVED_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
    <title>MegaMail — Finishing sign-in</title><h1>Authorization received</h1>\
    <p>Return to MegaMail while it finishes sign-in. Your account is not connected yet.</p></html>";
const CALLBACK_REFUSED_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
    <title>MegaMail</title><p>The sign-in did not finish.\
    You can close this tab and return to MegaMail.</p></html>";
const CALLBACK_INVALID_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
    <title>MegaMail</title><p>This request is not a valid sign-in callback.\
    MegaMail is still waiting for authorization.</p></html>";

/// Standard base64, for inline `data:` URIs.
pub fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Decode standard base64, skipping whitespace and `=` wherever they fall (a
/// MIME body wraps its lines) and a lone last character that cannot make a
/// byte. `None` if the input isn't valid (which for a truncated fetch is a
/// real possibility, not a bug).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
    use base64::Engine;
    const LENIENT: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::RequireNone)
            .with_decode_allow_trailing_bits(true),
    );
    let mut clean: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    LENIENT.decode(clean).ok()
}

/// Server presets for a known provider (endpoints + IMAP/SMTP hosts). Client
/// credentials come from [`provider_credentials`], not here.
pub struct Preset {
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static str,
    pub imap_host: &'static str,
    pub imap_port: u16,
    pub smtp_host: &'static str,
    pub smtp_port: u16,
}

/// Preset for a provider key ("google" / "microsoft"), or `None` for custom.
pub fn preset(provider: &str) -> Option<Preset> {
    match provider {
        "google" => Some(Preset {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            scopes: "https://mail.google.com/",
            imap_host: "imap.gmail.com",
            imap_port: 993,
            smtp_host: "smtp.gmail.com",
            smtp_port: 465,
        }),
        "microsoft" => Some(Preset {
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            // Graph, not IMAP: many work tenants turn IMAP off, and Graph is
            // what Microsoft accounts from GNOME Online Accounts use. The
            // mailbox settings carry the categories tags map to (#71).
            scopes: "https://graph.microsoft.com/Mail.ReadWrite \
                     https://graph.microsoft.com/Mail.Send \
                     https://graph.microsoft.com/MailboxSettings.ReadWrite \
                     https://graph.microsoft.com/User.Read offline_access",
            imap_host: "outlook.office365.com",
            imap_port: 993,
            smtp_host: "smtp.office365.com",
            smtp_port: 587,
        }),
        _ => None,
    }
}

/// A Microsoft sign-in made with another app registration:
/// its client ID, its tenant (`common`, `organizations`, `consumers`, a
/// directory ID or domain), its scopes and its redirect. Empty values keep
/// what `settings` has. Scopes are Graph's short names, as Evolution lists
/// them; `offline_access` is added when missing, or no refresh token comes
/// back.
pub fn microsoft_advanced(
    settings: &mut OAuthSettings,
    client_id: &str,
    tenant: &str,
    scopes: &str,
    redirect: &str,
) {
    if !client_id.is_empty() {
        settings.client_id = client_id.to_string();
    }
    let tenant = tenant.trim().trim_matches('/');
    if !tenant.is_empty() && !tenant.contains(['/', '?', '#', ' ']) {
        settings.auth_url = settings
            .auth_url
            .replacen("/common/", &format!("/{tenant}/"), 1);
        settings.token_url = settings
            .token_url
            .replacen("/common/", &format!("/{tenant}/"), 1);
    }
    if !scopes.trim().is_empty() {
        let mut list: Vec<&str> = scopes.split_whitespace().collect();
        if !list
            .iter()
            .any(|s| s.eq_ignore_ascii_case("offline_access"))
        {
            list.push("offline_access");
        }
        settings.scopes = list.join(" ");
    }
    settings.redirect_uri = redirect.trim().to_string();
}

/// [`microsoft_advanced`] read back from a saved account's settings, for
/// the form: `(client_id, tenant, scopes, redirect)`, with the client ID
/// blank when it matches MegaMail's configured default.
pub fn microsoft_advanced_of(settings: &OAuthSettings) -> (String, String, String, String) {
    let Some(p) = preset("microsoft") else {
        return Default::default();
    };
    if !settings.token_url.contains("microsoftonline") {
        return Default::default();
    }
    let client = if settings.client_id == MICROSOFT_CLIENT_ID {
        String::new()
    } else {
        settings.client_id.clone()
    };
    let tenant = settings
        .auth_url
        .strip_prefix("https://login.microsoftonline.com/")
        .and_then(|rest| rest.split('/').next())
        .filter(|t| *t != "common")
        .unwrap_or_default()
        .to_string();
    let scopes = if settings.scopes == p.scopes {
        String::new()
    } else {
        settings.scopes.clone()
    };
    (client, tenant, scopes, settings.redirect_uri.clone())
}

/// Resolve a provider's OAuth client credentials `(client_id, client_secret)`.
/// Runtime environment variables take precedence over `[google]`, `[microsoft]`
/// or `[dropbox]` entries in MegaMail's `oauth.toml`; compile-time MegaMail
/// credentials are the final fallback. Empty defaults keep sign-in unavailable
/// until MegaMail has an app-owned registration or the user supplies their own.
/// Google Desktop registrations may supply a client ID alone. A configured
/// secret is retained and sent, but is not proof of client confidentiality.
/// See https://developers.google.com/identity/protocols/oauth2/native-app.
pub fn provider_credentials(provider: &str) -> (String, String) {
    let (env_id, env_secret, default_id, default_secret) = match provider {
        "google" => (
            "MEGAMAIL_GOOGLE_CLIENT_ID",
            "MEGAMAIL_GOOGLE_CLIENT_SECRET",
            GOOGLE_CLIENT_ID,
            GOOGLE_CLIENT_SECRET,
        ),
        "microsoft" => (
            "MEGAMAIL_MICROSOFT_CLIENT_ID",
            "MEGAMAIL_MICROSOFT_CLIENT_SECRET",
            MICROSOFT_CLIENT_ID,
            MICROSOFT_CLIENT_SECRET,
        ),
        "dropbox" => (
            "MEGAMAIL_DROPBOX_CLIENT_ID",
            "MEGAMAIL_DROPBOX_CLIENT_SECRET",
            DROPBOX_CLIENT_ID,
            "",
        ),
        _ => return (String::new(), String::new()),
    };
    resolve_provider_credentials(
        provider,
        (
            std::env::var(env_id).unwrap_or_default(),
            std::env::var(env_secret).unwrap_or_default(),
        ),
        || creds_from_file(provider),
        (default_id, default_secret),
    )
}

// The production resolver's source seam keeps tests away from process-wide
// environment changes and private credential files. The file stays lazy:
// a usable higher-priority registration must not read oauth.toml at all.
fn resolve_provider_credentials(
    provider: &str,
    environment: (String, String),
    file: impl FnOnce() -> Option<(String, String)>,
    defaults: (&str, &str),
) -> (String, String) {
    if !matches!(provider, "google" | "microsoft" | "dropbox") {
        return (String::new(), String::new());
    }
    if !environment.0.trim().is_empty() {
        return environment;
    }
    if let Some(credentials) = file().filter(|credentials| !credentials.0.trim().is_empty()) {
        return credentials;
    }
    (defaults.0.to_string(), defaults.1.to_string())
}

#[derive(Deserialize, Default)]
struct OAuthFile {
    #[serde(default)]
    google: Option<FileCreds>,
    #[serde(default)]
    microsoft: Option<FileCreds>,
    #[serde(default)]
    dropbox: Option<FileCreds>,
}

#[derive(Deserialize, Default)]
struct FileCreds {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
}

fn creds_from_file(provider: &str) -> Option<(String, String)> {
    let path = crate::config::config_base()?.join("oauth.toml");
    let text = std::fs::read_to_string(path).ok()?;
    let file: OAuthFile = toml::from_str(&text).ok()?;
    let creds = match provider {
        "google" => file.google,
        "microsoft" => file.microsoft,
        "dropbox" => file.dropbox,
        _ => None,
    }?;
    Some((creds.client_id, creds.client_secret))
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// What a refresh gives back.
pub struct Refreshed {
    pub access_token: String,
    /// A replacement for the refresh token that was spent, when the provider
    /// rotates them (Microsoft does, on every refresh). Keeping the old one
    /// works only until it expires, about 90 days on, and then the account
    /// has to sign in again.
    pub refresh_token: Option<String>,
    /// Seconds the access token is good for, when the provider says.
    pub expires_in: Option<u64>,
}

/// Result of a completed sign-in (the refresh token to persist).
pub struct FlowResult {
    pub refresh_token: String,
}

/// Open a URI in the user's default handler (a browser, for http/https).
///
/// Uses the XDG **OpenURI portal** over D-Bus so it works inside a Flatpak
/// sandbox as well as natively, and is safe to call from any thread (the OAuth
/// flow opens the browser from a worker thread). Falls back to `xdg-open` if the
/// portal isn't available.
pub fn open_uri(uri: &str) {
    let _ = try_open_uri(uri);
}

/// Launch the system browser, reporting launch failure without reflecting its
/// authorization URL. A successful launch does not establish authorization.
pub fn try_open_uri(uri: &str) -> Result<(), String> {
    if open_uri_portal(uri).is_ok() {
        return Ok(());
    }
    std::process::Command::new("xdg-open")
        .arg(uri)
        .spawn()
        .map(|_| ())
        .map_err(|_| "Could not open the system browser.".into())
}

fn open_uri_portal(uri: &str) -> Result<(), Box<dyn std::error::Error>> {
    let conn = zbus::blocking::Connection::session()?;
    // OpenURI(parent_window: s, uri: s, options: a{sv}) — fire and forget.
    let options: std::collections::HashMap<&str, zbus::zvariant::Value> =
        std::collections::HashMap::new();
    conn.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.OpenURI"),
        "OpenURI",
        &("", uri, options),
    )?;
    Ok(())
}

/// Run the interactive authorization-code + PKCE flow (blocking — call off the
/// UI thread). Opens the browser and waits at most five minutes for the loopback
/// callback, including reads and writes to local peers.
pub fn run_flow(settings: &OAuthSettings) -> Result<FlowResult, String> {
    run_flow_with_cancel(settings, &AtomicBool::new(false))
}

/// Like [`run_flow`], with cooperative cancellation of the callback wait.
/// Set `cancelled` to true from another thread; do not reset it during a flow.
/// Cancellation is checked before browser launch and before code exchange.
/// It does not interrupt the system-browser portal or an in-flight HTTPS
/// token request. This function does not persist credentials or accounts.
pub fn run_flow_with_cancel(
    settings: &OAuthSettings,
    cancelled: &AtomicBool,
) -> Result<FlowResult, String> {
    check_cancelled(cancelled)?;
    validate_oauth_endpoints(settings)?;
    validate_client_id(settings)?;
    // Dropbox matches the redirect URI exactly, port and all, so its
    // listener sits on a fixed port the app registers; the others accept
    // any loopback port.
    let dropbox = token_endpoint_has_host(&settings.token_url, "api.dropboxapi.com");
    let listener = if dropbox {
        bind_fixed_port(DROPBOX_REDIRECT_PORT)?
    } else {
        TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?
    };
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    // Microsoft (Entra) only ignores the port when matching *localhost* loopback
    // redirects — a random-port 127.0.0.1 URI would need the exact port registered,
    // which we can't do. Google (and others) accept the 127.0.0.1 literal. The
    // listener is on 127.0.0.1 either way; browsers resolve localhost to it.
    let microsoft = token_endpoint_has_host(&settings.token_url, "login.microsoftonline.com");
    let host = if microsoft || dropbox {
        "localhost"
    } else {
        "127.0.0.1"
    };
    // Microsoft compares the redirect with the registered `http://localhost`
    // to the letter, the port aside, so it gets no trailing slash.
    let redirect = if microsoft {
        format!("http://{host}:{port}")
    } else {
        format!("http://{host}:{port}/")
    };
    // What asks for a refresh token: Dropbox has its own parameter for it,
    // and Microsoft's `offline_access` scope is the request. Microsoft gets
    // its account picker rather than a forced consent screen: in a tenant
    // where users may not consent, `prompt=consent` asks an admin every time
    // even after the app was approved.
    let offline = if dropbox {
        "&token_access_type=offline"
    } else if microsoft {
        "&prompt=select_account"
    } else {
        "&access_type=offline&prompt=consent"
    };

    let request = Authorization::new(settings, &redirect, offline)?;

    // Open the system browser (via the OpenURI portal, so it works in a Flatpak).
    check_cancelled(cancelled)?;
    open_uri(&request.url);

    let code = wait_for_code(&listener, &request.state, cancelled).map_err(|e| {
        if microsoft {
            explain_microsoft_refusal(settings, e)
        } else {
            e
        }
    })?;
    check_cancelled(cancelled)?;
    exchange_code(settings, &request, &code)
}

/// One authorization request: the address to open, and what checks and
/// redeems its answer.
pub struct Authorization {
    pub url: String,
    pub redirect: String,
    pub state: String,
    verifier: String,
}

impl Authorization {
    /// `extra` is appended to the query as it is ("&prompt=…").
    pub fn new(settings: &OAuthSettings, redirect: &str, extra: &str) -> Result<Self, String> {
        validate_client_id(settings)?;
        validate_https_endpoint(&settings.auth_url, "authorization endpoint")?;
        // PKCE S256 (RFC 7636 §4.2). With `plain` the challenge *is* the verifier, so
        // anyone who gets to read the authorization URL — browser history, an
        // extension, another local process — can redeem a stolen code, which is the
        // one thing PKCE exists to prevent. RFC 8252 requires S256 of any client that
        // can compute SHA-256.
        let entropy = |e| format!("no secure randomness available: {e}");
        let verifier = crate::rng::token(64).map_err(entropy)?;
        let state = crate::rng::token(24).map_err(entropy)?;
        let challenge = pkce_challenge(&verifier);
        let url = format!(
            "{base}?response_type=code&client_id={cid}&redirect_uri={redir}&scope={scope}\
             &code_challenge={chal}&code_challenge_method=S256&state={st}{extra}",
            base = settings.auth_url,
            cid = crate::percent::encode(&settings.client_id),
            redir = crate::percent::encode(redirect),
            scope = crate::percent::encode(&settings.scopes),
            chal = crate::percent::encode(&challenge),
            st = crate::percent::encode(&state),
        );
        Ok(Self {
            url,
            redirect: redirect.to_string(),
            state,
            verifier,
        })
    }

    /// Borrowed token-exchange fields for the provider transaction. Keep these
    /// worker-local: the verifier and code must not enter UI snapshots or logs.
    pub(crate) fn code_form<'a>(
        &'a self,
        settings: &'a OAuthSettings,
        code: &'a str,
    ) -> Vec<(&'static str, &'a str)> {
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.redirect.as_str()),
            ("client_id", settings.client_id.as_str()),
            ("code_verifier", self.verifier.as_str()),
        ];
        if !settings.client_secret.is_empty() {
            form.push(("client_secret", settings.client_secret.as_str()));
        }
        form
    }

    /// For a sign-in in MegaMail's own window: a validated code or correlated
    /// provider refusal. Unrelated, malformed and wrong-state navigations return
    /// `None` so they cannot end the pending transaction. The caller consumes
    /// this request after an accepted answer; this borrowed method is not a
    /// single-use token or an identity verifier.
    pub fn answer(&self, uri: &str, settings: &OAuthSettings) -> Option<Result<String, String>> {
        if uri.len() > MAX_CALLBACK_REQUEST_BYTES {
            return None;
        }
        // The exact registered origin/path must precede the query; accepting
        // fragments or a prefix path would allow unrelated navigations.
        let query = uri
            .strip_prefix(self.redirect.as_str())?
            .strip_prefix('?')?;
        let answer = validate_callback_query(query, &self.state).ok()?;
        Some(match answer {
            CallbackAnswer::Code(code) => Ok(code),
            CallbackAnswer::Refused(reason) => Err(
                if token_endpoint_has_host(&settings.token_url, "login.microsoftonline.com") {
                    explain_microsoft_refusal(settings, reason)
                } else {
                    reason
                },
            ),
        })
    }
}

/// Redeem an authorization code for the refresh token (blocking).
pub fn exchange_code(
    settings: &OAuthSettings,
    request: &Authorization,
    code: &str,
) -> Result<FlowResult, String> {
    validate_client_id(settings)?;
    validate_https_endpoint(&settings.token_url, "token endpoint")?;
    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &request.redirect),
        ("client_id", &settings.client_id),
        ("code_verifier", &request.verifier),
    ];
    if !settings.client_secret.is_empty() {
        form.push(("client_secret", &settings.client_secret));
    }
    let token: TokenResponse = oauth_agent()
        .post(&settings.token_url)
        .send_form(&form)
        .map_err(token_request_error)?
        .into_json()
        .map_err(|e| e.to_string())?;

    if token.refresh_token.is_empty() {
        return Err("the provider did not return a refresh token".into());
    }
    Ok(FlowResult {
        refresh_token: token.refresh_token,
    })
}

/// Who signed in to a Microsoft account (blocking): the display name and
/// address Graph reports, and the refresh token to keep, rotated by the
/// refresh this takes. Fills the account in after a sign-in, so it is named
/// by the mailbox actually reached rather than by what was typed.
pub fn microsoft_whoami(
    settings: &OAuthSettings,
    refresh: &str,
    user: Option<&str>,
) -> Result<(String, String, String), String> {
    let fresh = refresh_access_token_as(settings, refresh, user)?;
    let me: serde_json::Value = oauth_agent()
        .get("https://graph.microsoft.com/v1.0/me?$select=displayName,mail,userPrincipalName")
        .set("Authorization", &format!("Bearer {}", fresh.access_token))
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let name = me["displayName"].as_str().unwrap_or_default().to_string();
    let address = me["mail"]
        .as_str()
        .filter(|m| !m.is_empty())
        .or_else(|| me["userPrincipalName"].as_str())
        .unwrap_or_default()
        .to_string();
    Ok((
        name,
        address,
        fresh.refresh_token.unwrap_or_else(|| refresh.to_string()),
    ))
}

/// Mint a fresh access token from a stored refresh token (blocking).
pub fn refresh_access_token(
    settings: &OAuthSettings,
    refresh_token: &str,
) -> Result<Refreshed, String> {
    refresh_access_token_as(settings, refresh_token, None)
}

/// [`refresh_access_token`] for the account `user` signs in as. Behind
/// Conditional Access that wants a managed device, Entra checks every
/// refresh again: the identity broker's device cookie goes with it, as a
/// header of its own name (#329, see `ms_broker`).
pub fn refresh_access_token_as(
    settings: &OAuthSettings,
    refresh_token: &str,
    user: Option<&str>,
) -> Result<Refreshed, String> {
    validate_client_id(settings)?;
    validate_https_endpoint(&settings.token_url, "token endpoint")?;
    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", &settings.client_id),
    ];
    if !settings.client_secret.is_empty() {
        form.push(("client_secret", &settings.client_secret));
    }
    let mut post = oauth_agent().post(&settings.token_url);
    if let Some(cookie) = crate::ms_broker::sso_cookie(settings, user) {
        post = post.set(&cookie.name, &cookie.value);
    }
    let token: TokenResponse = post
        .send_form(&form)
        .map_err(token_request_error)?
        .into_json()
        .map_err(|e| e.to_string())?;
    if token.access_token.is_empty() {
        return Err("no access token in refresh response".into());
    }
    let rotated = Some(token.refresh_token).filter(|t| !t.is_empty() && t != refresh_token);
    Ok(Refreshed {
        access_token: token.access_token,
        refresh_token: rotated,
        expires_in: token.expires_in,
    })
}

/// Fixed-port providers must not let an unauthenticated local HTTP request
/// cancel another transaction. A conflicting listener is now an explicit error;
/// callers using cooperative cancellation can close their own earlier wait.
fn bind_fixed_port(port: u16) -> Result<TcpListener, String> {
    TcpListener::bind(("127.0.0.1", port)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            format!("could not listen on localhost port {port} for the sign-in redirect. Another sign-in or program is using it; finish or cancel it first.")
        } else {
            format!("could not listen on localhost port {port} for the sign-in redirect: {error}")
        }
    })
}

const MAX_CALLBACK_REQUEST_BYTES: usize = 8192;
const CALLBACK_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy)]
struct CallbackLimits {
    total_wait: Duration,
    peer_io: Duration,
}

const CALLBACK_LIMITS: CallbackLimits = CallbackLimits {
    total_wait: Duration::from_secs(300),
    peer_io: Duration::from_secs(2),
};

struct CallbackBudget<'a> {
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        Err("sign-in cancelled".into())
    } else {
        Ok(())
    }
}

impl CallbackBudget<'_> {
    fn check(&self) -> Result<(), String> {
        check_cancelled(self.cancelled)?;
        if Instant::now() >= self.deadline {
            return Err("sign-in timed out".into());
        }
        Ok(())
    }

    fn pause(&self, peer_deadline: Instant) {
        std::thread::sleep(
            CALLBACK_POLL_INTERVAL
                .min(self.deadline.saturating_duration_since(Instant::now()))
                .min(peer_deadline.saturating_duration_since(Instant::now())),
        );
    }
}

/// Accept only complete, bounded request headers. Nonblocking IO plus an
/// absolute peer deadline prevents slow trickles from extending their slot.
/// All peer deadlines are subordinate to the one transaction deadline.
fn read_callback_request(
    stream: &mut TcpStream,
    budget: &CallbackBudget<'_>,
    peer_deadline: Instant,
) -> Option<String> {
    let mut bytes = [0u8; MAX_CALLBACK_REQUEST_BYTES];
    let mut filled = 0;
    loop {
        budget.check().ok()?;
        if Instant::now() >= peer_deadline || filled == bytes.len() {
            return None;
        }
        match stream.read(&mut bytes[filled..]) {
            Ok(0) => return None,
            Ok(count) => {
                filled += count;
                if let Some(end) = bytes[..filled]
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                {
                    return String::from_utf8(bytes[..end + 4].to_vec()).ok();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                budget.pause(peer_deadline);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

/// Responses never include codes, state or provider-supplied text, and may not
/// keep a transaction alive when a browser stops reading.
fn write_callback_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
    budget: &CallbackBudget<'_>,
    peer_deadline: Instant,
) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    );
    let mut remaining = response.as_bytes();
    while !remaining.is_empty() {
        if budget.check().is_err() || Instant::now() >= peer_deadline {
            return;
        }
        match stream.write(remaining) {
            Ok(0) => return,
            Ok(count) => remaining = &remaining[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                budget.pause(peer_deadline);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

fn wait_for_code(
    listener: &TcpListener,
    expected_state: &str,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    wait_for_code_with_limits(listener, expected_state, cancelled, CALLBACK_LIMITS)
}

/// Worker-local wait for a provider transaction with its remaining total
/// budget. This delegates to the unchanged strict callback validator.
pub(crate) fn wait_for_provider_code(
    listener: &TcpListener,
    expected_state: &str,
    cancelled: &AtomicBool,
    remaining: Duration,
) -> Result<String, String> {
    wait_for_code_with_limits(
        listener,
        expected_state,
        cancelled,
        CallbackLimits {
            total_wait: remaining.min(CALLBACK_LIMITS.total_wait),
            peer_io: CALLBACK_LIMITS.peer_io,
        },
    )
}

// A private limits seam allows synthetic sockets to exercise the same wait
// loop used by run_flow without waiting five minutes or opening a browser.
fn wait_for_code_with_limits(
    listener: &TcpListener,
    expected_state: &str,
    cancelled: &AtomicBool,
    limits: CallbackLimits,
) -> Result<String, String> {
    let budget = CallbackBudget {
        deadline: Instant::now() + limits.total_wait,
        cancelled,
    };
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    loop {
        budget.check()?;
        match listener.accept() {
            Ok((mut stream, _)) => {
                if stream.set_nonblocking(true).is_err() {
                    continue;
                }
                // Read and write share one fixed peer slot, not a fresh timeout
                // per packet. Closing an invalid peer does not consume the flow.
                let peer_deadline = (Instant::now() + limits.peer_io).min(budget.deadline);
                let request = read_callback_request(&mut stream, &budget, peer_deadline);
                budget.check()?;
                let answer = request
                    .as_deref()
                    .and_then(|request| validate_callback_request(request, expected_state).ok());
                match answer {
                    Some(CallbackAnswer::Code(code)) => {
                        write_callback_response(
                            &mut stream,
                            "200 OK",
                            CALLBACK_RECEIVED_PAGE,
                            &budget,
                            peer_deadline,
                        );
                        budget.check()?;
                        return Ok(code);
                    }
                    Some(CallbackAnswer::Refused(reason)) => {
                        write_callback_response(
                            &mut stream,
                            "200 OK",
                            CALLBACK_REFUSED_PAGE,
                            &budget,
                            peer_deadline,
                        );
                        budget.check()?;
                        return Err(reason);
                    }
                    None => {
                        write_callback_response(
                            &mut stream,
                            "400 Bad Request",
                            CALLBACK_INVALID_PAGE,
                            &budget,
                            peer_deadline,
                        );
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                budget.pause(budget.deadline);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

// Do not derive Debug: this enum can contain an authorization code.
enum CallbackAnswer {
    Code(String),
    Refused(String),
}

fn validate_callback_request(request: &str, expected_state: &str) -> Result<CallbackAnswer, ()> {
    let (line, _) = request.split_once("\r\n").ok_or(())?;
    let mut parts = line.split(' ');
    let method = parts.next().ok_or(())?;
    let target = parts.next().ok_or(())?;
    let version = parts.next().ok_or(())?;
    if method != "GET" || !matches!(version, "HTTP/1.0" | "HTTP/1.1") || parts.next().is_some() {
        return Err(());
    }
    // run_flow binds only a root loopback redirect (including Microsoft's
    // registered no-trailing-slash URI, whose HTTP request path is still "/").
    let (path, query) = target.split_once('?').ok_or(())?;
    if path != "/" {
        return Err(());
    }
    validate_callback_query(query, expected_state)
}

fn validate_callback_query(query: &str, expected_state: &str) -> Result<CallbackAnswer, ()> {
    if query.len() > MAX_CALLBACK_REQUEST_BYTES || query.contains('#') {
        return Err(());
    }
    let mut names = std::collections::HashSet::new();
    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut description = None;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').ok_or(())?;
        let name = decode_callback_parameter(name)?;
        let value = decode_callback_parameter(value)?;
        if name.trim().is_empty()
            || value.trim().is_empty()
            || name.chars().any(char::is_control)
            || !names.insert(name.clone())
            || (name != "error_description" && value.chars().any(char::is_control))
        {
            return Err(());
        }
        match name.as_str() {
            "code" => code = Some(value),
            "state" => state = Some(value),
            "error" => error = Some(value),
            "error_description" => description = Some(value),
            _ => {}
        }
    }
    // Correlate both outcomes before attributing any refusal to a provider.
    if expected_state.is_empty() || state.as_deref() != Some(expected_state) {
        return Err(());
    }
    match (code, error, description) {
        (Some(code), None, None) => Ok(CallbackAnswer::Code(code)),
        (None, Some(error), description) => Ok(CallbackAnswer::Refused(sanitized_refusal(
            &error,
            description.as_deref(),
        ))),
        _ => Err(()),
    }
}

// OAuth input must not inherit the mail-body decoder's forgiving malformed
// percent escapes and lossy UTF-8. Decode keys too, so encoded duplicate keys
// cannot bypass the uniqueness check.
fn decode_callback_parameter(input: &str) -> Result<String, ()> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'%' => {
                let high = bytes
                    .next()
                    .and_then(|b| (b as char).to_digit(16))
                    .ok_or(())?;
                let low = bytes
                    .next()
                    .and_then(|b| (b as char).to_digit(16))
                    .ok_or(())?;
                decoded.push((high * 16 + low) as u8);
            }
            b'+' => decoded.push(b' '),
            byte => decoded.push(byte),
        }
    }
    String::from_utf8(decoded).map_err(|_| ())
}

// Even a correctly correlated error description can echo tokens, addresses or
// provider diagnostics. Expose only a small fixed vocabulary, never raw text.
fn sanitized_refusal(error: &str, description: Option<&str>) -> String {
    let consent_code = ["AADSTS65001", "AADSTS65004", "AADSTS90094", "AADSTS90095"]
        .into_iter()
        .find(|code| description.is_some_and(|text| text.contains(code)));
    if let Some(code) = consent_code {
        return format!("{code}: The provider requires administrator consent.");
    }
    match error {
        "access_denied" => "access_denied: Sign-in was denied.",
        "consent_required" => "consent_required: The provider requires consent.",
        "login_required" => "login_required: Sign in to the provider and try again.",
        "interaction_required" => {
            "interaction_required: The provider requires interactive sign-in."
        }
        "temporarily_unavailable" => {
            "temporarily_unavailable: The provider is temporarily unavailable."
        }
        "server_error" => "server_error: The provider could not finish sign-in.",
        _ => "The provider refused sign-in.",
    }
    .into()
}

/// A Microsoft sign-in that failed for want of consent, with where an
/// organisation's administrator approves MegaMail for all its users. Users of
/// work and school accounts elsewhere cannot approve an app from a publisher
/// Microsoft has not verified themselves (#329).
fn explain_microsoft_refusal(settings: &OAuthSettings, reason: String) -> String {
    let consent = [
        "AADSTS65001",
        "AADSTS65004",
        "AADSTS90094",
        "AADSTS90095",
        "consent_required",
        "access_denied",
    ]
    .iter()
    .any(|c| reason.contains(c));
    if !consent {
        return reason;
    }
    let link = format!(
        "https://login.microsoftonline.com/organizations/adminconsent?client_id={}",
        settings.client_id
    );
    crate::i18n::i18n_f(
        "{reason} If your organization manages your account, an administrator can approve MegaMail for everyone at {link}",
        &[("reason", &reason), ("link", &link)],
    )
}

/// The PKCE S256 code challenge for a verifier: base64url(SHA-256(verifier)),
/// without padding (RFC 7636 §4.2).
fn pkce_challenge(verifier: &str) -> String {
    use sha2::Digest;
    base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        sha2::Sha256::digest(verifier.as_bytes()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LIMITS: CallbackLimits = CallbackLimits {
        total_wait: Duration::from_secs(5),
        peer_io: Duration::from_millis(150),
    };

    fn loopback_request(address: std::net::SocketAddr, line: &str) -> String {
        let mut peer = std::net::TcpStream::connect(address).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        peer.write_all(format!("{line}\r\nHost: localhost\r\n\r\n").as_bytes())
            .unwrap();
        let mut response = String::new();
        peer.read_to_string(&mut response).unwrap();
        response
    }

    fn assert_invalid_callbacks_stay_pending(lines: &[&str]) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let waiter = std::thread::spawn(move || {
            wait_for_code_with_limits(
                &listener,
                "synthetic-state",
                &AtomicBool::new(false),
                TEST_LIMITS,
            )
        });
        for line in lines {
            let response = loopback_request(address, line);
            assert!(response.starts_with("HTTP/1.1 400 "), "{response}");
            assert!(!response.contains("Authorization received"));
            assert!(!response.contains("UNTRUSTED"));
        }
        let response = loopback_request(
            address,
            "GET /?code=synthetic-code&state=synthetic-state HTTP/1.1",
        );
        assert!(response.starts_with("HTTP/1.1 200 "), "{response}");
        assert!(response.contains("Authorization received"));
        assert!(response.contains("not connected yet"));
        assert!(!response.contains("Signed in"));
        assert_eq!(waiter.join().unwrap(), Ok("synthetic-code".into()));
    }

    #[test]
    fn loopback_wrong_state_success_and_denial_stay_pending() {
        assert_invalid_callbacks_stay_pending(&[
            "GET /?code=UNTRUSTED_CODE&state=wrong HTTP/1.1",
            "GET /?error=access_denied&error_description=UNTRUSTED_REFUSAL&state=wrong HTTP/1.1",
            "GET /?error=access_denied&error_description=UNTRUSTED_REFUSAL HTTP/1.1",
        ]);
    }

    #[test]
    fn loopback_duplicate_missing_and_empty_parameters_stay_pending() {
        assert_invalid_callbacks_stay_pending(&[
            "GET /?code=one&code=two&state=synthetic-state HTTP/1.1",
            "GET /?code=one&state=wrong&state=synthetic-state HTTP/1.1",
            "GET /?code=one&state=synthetic-state&state=wrong HTTP/1.1",
            "GET /?code=one&st%61te=synthetic-state&state=synthetic-state HTTP/1.1",
            "GET /?error=access_denied&error=server_error&state=synthetic-state HTTP/1.1",
            "GET /?error=access_denied&error_description=one&error_description=two&state=synthetic-state HTTP/1.1",
            "GET /?code=one&error=access_denied&state=synthetic-state HTTP/1.1",
            "GET /?code=one HTTP/1.1",
            "GET /?state=synthetic-state HTTP/1.1",
            "GET /?code=&state=synthetic-state HTTP/1.1",
            "GET /?code=one&state= HTTP/1.1",
            "GET /?error=&state=synthetic-state HTTP/1.1",
            "GET /?error=access_denied&state=synthetic-state&error_description= HTTP/1.1",
            "GET /?code=%ZZ&state=synthetic-state HTTP/1.1",
            "GET /?code=%FF&state=synthetic-state HTTP/1.1",
        ]);
    }

    #[test]
    fn loopback_exact_method_path_and_request_line_are_required() {
        assert_invalid_callbacks_stay_pending(&[
            "GET /favicon.ico HTTP/1.1",
            "GET /other?code=one&state=synthetic-state HTTP/1.1",
            "GET //?code=one&state=synthetic-state HTTP/1.1",
            "POST /?code=one&state=synthetic-state HTTP/1.1",
            "HEAD /?code=one&state=synthetic-state HTTP/1.1",
            "GET http://localhost/?code=one&state=synthetic-state HTTP/1.1",
            "GET /?code=one&state=synthetic-state#fragment HTTP/1.1",
            "GET /?code=one&state=synthetic-state HTTP/1.1 extra",
            "GET /?cancel=1 HTTP/1.1",
        ]);
    }

    #[test]
    fn loopback_valid_error_is_correlated_and_redacted() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let waiter = std::thread::spawn(move || {
            wait_for_code_with_limits(
                &listener,
                "synthetic-state",
                &AtomicBool::new(false),
                TEST_LIMITS,
            )
        });
        let response = loopback_request(
            address,
            "GET /?error=access_denied&error_description=UNTRUSTED_TOKEN&state=synthetic-state HTTP/1.1",
        );
        assert!(response.contains("The sign-in did not finish"));
        assert!(!response.contains("UNTRUSTED_TOKEN"));
        let reason = waiter.join().unwrap().unwrap_err();
        assert!(reason.contains("access_denied"));
        assert!(!reason.contains("UNTRUSTED_TOKEN"));
    }

    #[test]
    fn loopback_partial_headers_are_read_before_validation() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let waiter = std::thread::spawn(move || {
            wait_for_code_with_limits(
                &listener,
                "synthetic-state",
                &AtomicBool::new(false),
                TEST_LIMITS,
            )
        });
        let mut peer = std::net::TcpStream::connect(address).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        peer.write_all(b"GET /?code=synthetic-").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        peer.write_all(b"code&state=synthetic-state HTTP/1.1\r\nHost: localhost\r\n")
            .unwrap();
        std::thread::sleep(Duration::from_millis(30));
        peer.write_all(b"\r\n").unwrap();
        let mut response = String::new();
        peer.read_to_string(&mut response).unwrap();
        assert!(response.contains("Authorization received"));
        assert_eq!(waiter.join().unwrap(), Ok("synthetic-code".into()));
    }

    #[test]
    fn loopback_oversized_headers_do_not_complete_transaction() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let waiter = std::thread::spawn(move || {
            wait_for_code_with_limits(
                &listener,
                "synthetic-state",
                &AtomicBool::new(false),
                TEST_LIMITS,
            )
        });
        let mut peer = std::net::TcpStream::connect(address).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let oversized = format!(
            "GET /?code=UNTRUSTED_CODE&state=synthetic-state HTTP/1.1\r\nX-Padding: {}\r\n\r\n",
            "x".repeat(MAX_CALLBACK_REQUEST_BYTES),
        );
        // A reset is also an acceptable rejection of an oversized request.
        let _ = peer.write_all(oversized.as_bytes());
        drop(peer);
        loopback_request(
            address,
            "GET /?code=synthetic-code&state=synthetic-state HTTP/1.1",
        );
        assert_eq!(waiter.join().unwrap(), Ok("synthetic-code".into()));
    }

    #[test]
    fn loopback_stalled_peer_is_evicted_and_valid_callback_can_follow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stalled = std::net::TcpStream::connect(address).unwrap();
        let waiter = std::thread::spawn(move || {
            wait_for_code_with_limits(
                &listener,
                "synthetic-state",
                &AtomicBool::new(false),
                TEST_LIMITS,
            )
        });
        loopback_request(
            address,
            "GET /?code=synthetic-code&state=synthetic-state HTTP/1.1",
        );
        drop(stalled);
        assert_eq!(waiter.join().unwrap(), Ok("synthetic-code".into()));
    }

    #[test]
    fn loopback_total_deadline_caps_a_single_stalled_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stalled = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let start = Instant::now();
        let result = wait_for_code_with_limits(
            &listener,
            "synthetic-state",
            &AtomicBool::new(false),
            CallbackLimits {
                total_wait: Duration::from_millis(100),
                peer_io: Duration::from_secs(10),
            },
        );
        drop(stalled);
        assert_eq!(result, Err("sign-in timed out".into()));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn loopback_total_deadline_is_not_reset_by_invalid_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let wait = Duration::from_millis(250);
        let waiter = std::thread::spawn(move || {
            let start = Instant::now();
            let result = wait_for_code_with_limits(
                &listener,
                "synthetic-state",
                &AtomicBool::new(false),
                CallbackLimits {
                    total_wait: wait,
                    peer_io: Duration::from_secs(10),
                },
            );
            (result, start.elapsed())
        });
        loopback_request(address, "GET /favicon.ico HTTP/1.1");
        let stalled = std::net::TcpStream::connect(address).unwrap();
        let (result, elapsed) = waiter.join().unwrap();
        drop(stalled);
        assert_eq!(result, Err("sign-in timed out".into()));
        assert!(elapsed < Duration::from_secs(2));
    }

    #[test]
    fn loopback_cancellation_interrupts_a_stalled_peer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stalled = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let signal = std::sync::Arc::clone(&cancelled);
        let setter = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            signal.store(true, Ordering::Relaxed);
        });
        let start = Instant::now();
        let result = wait_for_code_with_limits(
            &listener,
            "synthetic-state",
            &cancelled,
            CallbackLimits {
                total_wait: Duration::from_secs(5),
                peer_io: Duration::from_secs(10),
            },
        );
        setter.join().unwrap();
        drop(stalled);
        assert_eq!(result, Err("sign-in cancelled".into()));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn authorization_answer_ignores_invalid_callbacks_and_redacts_errors() {
        let settings = OAuthSettings {
            client_id: "synthetic-client".into(),
            auth_url: "https://login.example.test/authorize".into(),
            token_url: "https://login.example.test/token".into(),
            ..Default::default()
        };
        let request =
            Authorization::new(&settings, "https://login.example.test/callback", "").unwrap();
        for suffix in [
            "?error=access_denied&state=wrong",
            "?error=access_denied",
            "?code=one",
            "?code=&state=",
            "?code=one&code=two",
            "/extra?code=one",
            "#code=one",
        ] {
            assert!(request
                .answer(&format!("{}{suffix}", request.redirect), &settings)
                .is_none());
        }
        let wrong_origin = format!(
            "https://other.example.test/callback?code=one&state={}",
            request.state
        );
        assert!(request.answer(&wrong_origin, &settings).is_none());
        let denied = format!(
            "{}?error=UNTRUSTED_ERROR&error_description=UNTRUSTED_DESCRIPTION&state={}",
            request.redirect, request.state
        );
        let reason = request.answer(&denied, &settings).unwrap().unwrap_err();
        assert_eq!(reason, "The provider refused sign-in.");
    }

    #[test]
    fn provider_registration_resolution_accepts_google_id_without_secret() {
        // Exercise the production resolver's internal source seam without
        // changing environment variables or reading a real oauth.toml.
        let mut file_read = false;
        let resolved = resolve_provider_credentials(
            "google",
            ("synthetic-desktop-client".into(), String::new()),
            || {
                file_read = true;
                Some(("lower-priority-client".into(), "unused-secret".into()))
            },
            ("default-client", "default-secret"),
        );
        assert_eq!(resolved, ("synthetic-desktop-client".into(), String::new()));
        assert!(
            !file_read,
            "a usable environment override must not read a credential file"
        );
    }

    #[test]
    fn provider_registration_resolution_preserves_file_secret_and_default_priority() {
        assert_eq!(
            resolve_provider_credentials(
                "google",
                (String::new(), String::new()),
                || Some((
                    "synthetic-file-client".into(),
                    "synthetic-file-secret".into()
                )),
                ("default-client", "default-secret")
            ),
            (
                "synthetic-file-client".into(),
                "synthetic-file-secret".into()
            ),
        );
        assert_eq!(
            resolve_provider_credentials(
                "google",
                (String::new(), String::new()),
                || Some(("synthetic-file-client".into(), String::new())),
                ("default-client", "default-secret")
            ),
            ("synthetic-file-client".into(), String::new()),
        );
        assert_eq!(
            resolve_provider_credentials(
                "google",
                (String::new(), String::new()),
                || None,
                ("default-client", "default-secret")
            ),
            ("default-client".into(), "default-secret".into()),
        );
        assert_eq!(
            resolve_provider_credentials(
                "unknown",
                ("synthetic-id".into(), "synthetic-secret".into()),
                || Some(("synthetic-id".into(), "synthetic-secret".into())),
                ("default-client", "default-secret")
            ),
            (String::new(), String::new()),
        );
    }

    #[test]
    fn cancellation_before_flow_start_has_no_browser_or_provider_side_effects() {
        assert!(matches!(
            run_flow_with_cancel(&OAuthSettings::default(), &AtomicBool::new(true)),
            Err(reason) if reason == "sign-in cancelled",
        ));
    }

    #[test]
    fn provider_behavior_is_selected_by_exact_token_authority_not_substrings() {
        let host = "login.microsoftonline.com";
        assert!(token_endpoint_has_host(
            "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            host
        ));
        for endpoint in [
            "https://login.microsoftonline.com.example.test/token",
            "https://example.test/microsoftonline/token",
            "https://example.test/token?provider=microsoftonline",
            "https://login.microsoftonline.com@example.test/token",
            "http://login.microsoftonline.com/token",
        ] {
            assert!(!token_endpoint_has_host(endpoint, host));
        }
    }

    #[test]
    fn credential_endpoints_must_be_https_without_authority_secrets_or_fragments() {
        assert!(validate_https_endpoint(
            "https://accounts.google.com/o/oauth2/v2/auth",
            "authorization endpoint"
        )
        .is_ok());

        for bad in [
            "http://accounts.google.com/o/oauth2/v2/auth",
            "not a URI",
            "https:///oauth/token",
            "https://user:secret@example.com/oauth/token",
            "https://example.com/oauth/token#fragment",
        ] {
            let error = validate_https_endpoint(bad, "token endpoint")
                .expect_err("invalid credential endpoint must be rejected");
            assert!(error.contains("token endpoint"));
            assert!(!error.contains(bad), "error reflected the configured URL");
        }
    }

    #[test]
    fn loopback_http_redirect_does_not_relax_credential_endpoints() {
        let settings = OAuthSettings {
            auth_url: "https://login.example.com/oauth/authorize".into(),
            token_url: "https://login.example.com/oauth/token".into(),
            redirect_uri: "http://localhost:41597/".into(),
            ..Default::default()
        };
        assert!(validate_oauth_endpoints(&settings).is_ok());
    }

    #[test]
    fn microsoft_advanced_rows_make_the_sign_in() {
        let p = preset("microsoft").unwrap();
        let mut s = OAuthSettings {
            auth_url: p.auth_url.into(),
            token_url: p.token_url.into(),
            client_id: MICROSOFT_CLIENT_ID.into(),
            scopes: p.scopes.into(),
            ..Default::default()
        };
        assert_eq!(microsoft_advanced_of(&s), Default::default());
        microsoft_advanced(
            &mut s,
            "test-client-id",
            "organizations",
            "Mail.ReadWrite Mail.Send User.Read",
            "https://login.microsoftonline.com/common/oauth2/nativeclient",
        );
        assert_eq!(s.client_id, "test-client-id");
        assert_eq!(
            s.auth_url,
            "https://login.microsoftonline.com/organizations/oauth2/v2.0/authorize"
        );
        assert_eq!(
            s.token_url,
            "https://login.microsoftonline.com/organizations/oauth2/v2.0/token"
        );
        assert_eq!(
            s.scopes,
            "Mail.ReadWrite Mail.Send User.Read offline_access"
        );
        let back = microsoft_advanced_of(&s);
        assert_eq!(back.1, "organizations");
        assert_eq!(
            back.3,
            "https://login.microsoftonline.com/common/oauth2/nativeclient"
        );
        // The redirect is caught, with its code and the request's state.
        let req = Authorization::new(&s, &s.redirect_uri, "").unwrap();
        assert!(
            req.url
                .contains("redirect_uri=https%3A%2F%2Flogin.microsoftonline.com"),
            "{}",
            req.url
        );
        let ok = format!("{}?code=abc&state={}", s.redirect_uri, req.state);
        assert_eq!(req.answer(&ok, &s), Some(Ok("abc".into())));
        assert!(req
            .answer(
                "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?x",
                &s
            )
            .is_none());
        let refused = format!(
            "{}?error=access_denied&error_description=AADSTS65004%3A+declined&state={}",
            s.redirect_uri, req.state
        );
        assert!(matches!(req.answer(&refused, &s), Some(Err(_))));
    }

    #[test]
    fn a_correlated_microsoft_refusal_keeps_only_safe_consent_guidance() {
        let preset = preset("microsoft").unwrap();
        let settings = OAuthSettings {
            client_id: "synthetic-client".into(),
            auth_url: preset.auth_url.into(),
            token_url: preset.token_url.into(),
            ..Default::default()
        };
        let request = Authorization::new(
            &settings,
            "https://login.microsoftonline.com/common/oauth2/nativeclient",
            "",
        )
        .unwrap();
        let uri = format!(
            "{}?error=access_denied&error_description=AADSTS65004%3a+UNTRUSTED_TOKEN%0d%0aTrace+ID%3a+UNTRUSTED_ID&state={}",
            request.redirect, request.state,
        );
        let told = request.answer(&uri, &settings).unwrap().unwrap_err();
        assert!(told.contains("AADSTS65004"), "{told}");
        assert!(
            told.contains("adminconsent?client_id=synthetic-client"),
            "{told}"
        );
        assert!(!told.contains("UNTRUSTED"), "{told}");
        assert_eq!(
            explain_microsoft_refusal(&settings, "sign-in timed out".into()),
            "sign-in timed out",
        );
    }

    #[test]
    fn base64_is_as_forgiving_as_a_mail_body_needs() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&bytes)).unwrap(), bytes);
        assert_eq!(base64_encode(b"hi!?"), "aGkhPw==");
        assert_eq!(base64_decode("aGkh\r\nPw==").unwrap(), b"hi!?");
        assert_eq!(
            base64_decode("aGkhPw").unwrap(),
            b"hi!?",
            "padding is optional"
        );
        assert_eq!(
            base64_decode("aGkhP").unwrap(),
            b"hi!",
            "a lone last character is dropped"
        );
        assert_eq!(base64_decode("aG*h"), None);
    }

    #[test]
    fn a_fixed_port_conflict_does_not_cancel_another_waiter() {
        let old = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = old.local_addr().unwrap().port();
        let error = bind_fixed_port(port).unwrap_err();
        assert!(error.contains("finish or cancel"));
        old.set_nonblocking(true).unwrap();
        assert_eq!(
            old.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        drop(old);
        assert_eq!(
            bind_fixed_port(port).unwrap().local_addr().unwrap().port(),
            port
        );
    }

    #[test]
    fn the_pkce_challenge_matches_the_rfc_7636_vector() {
        // RFC 7636 appendix B. With `plain` the challenge *was* the verifier, so
        // reading the authorization URL was enough to redeem a stolen code.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = pkce_challenge(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_ne!(challenge, verifier);
        // base64url without padding: no `+`, `/` or `=` to percent-encode.
        assert!(!challenge.contains(['+', '/', '=']), "{challenge}");
    }
}
