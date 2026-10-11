//! Provider authorization on a blocking worker, never the UI thread.
//!
//! Gmail uses IMAP/SMTP; Microsoft uses Graph (not Exchange IMAP). Production
//! and synthetic callers use the same transaction, HTTP, browser, probe and
//! persistence contracts. Nothing is persisted before identity and probe checks.
//! See `docs/research/batch2-provider-auth.md` for integration and release gates.

use std::fmt;
use std::io::Read;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::{AccountConfig, OAuthSettings, Protocol, ServerSecurity};
use crate::oauth::Authorization;

const MAX_RESPONSE: usize = 64 * 1024;
const MAX_ATTEMPT: Duration = Duration::from_secs(300);
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
const GOOGLE_IDENTITY: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const MICROSOFT_IDENTITY: &str =
    "https://graph.microsoft.com/v1.0/me?$select=id,displayName,mail,userPrincipalName";
const GRAPH_PROBE: &str = "https://graph.microsoft.com/v1.0/me/mailFolders/inbox?$select=id";
const GOOGLE_SCOPES: &str = "openid email https://mail.google.com/";
const GRAPH_SCOPES: &str = "https://graph.microsoft.com/User.Read https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/MailboxSettings.ReadWrite offline_access";

/// Microsoft authority/audience selection, enforced by the provider endpoint.
/// This is not a verified tenant-ID claim or shared-mailbox authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicrosoftAudience {
    Personal,
    WorkOrSchool,
    PersonalAndWork,
}

impl MicrosoftAudience {
    fn authority(self) -> &'static str {
        match self {
            Self::Personal => "consumers",
            Self::WorkOrSchool => "organizations",
            Self::PersonalAndWork => "common",
        }
    }
}

/// Explicit protocol/resource selection. No Graph token is handed to IMAP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    GoogleImapSmtp,
    MicrosoftGraph(MicrosoftAudience),
}

impl Provider {
    fn key(self) -> &'static str {
        match self {
            Self::GoogleImapSmtp => "google",
            Self::MicrosoftGraph(_) => "microsoft",
        }
    }

    fn settings(self, client_id: String, client_secret: String) -> OAuthSettings {
        let (auth_url, token_url, scopes) = match self {
            Self::GoogleImapSmtp => (
                "https://accounts.google.com/o/oauth2/v2/auth".into(),
                "https://oauth2.googleapis.com/token".into(),
                GOOGLE_SCOPES,
            ),
            Self::MicrosoftGraph(audience) => (
                format!(
                    "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
                    audience.authority()
                ),
                format!(
                    "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
                    audience.authority()
                ),
                GRAPH_SCOPES,
            ),
        };
        OAuthSettings {
            auth_url,
            token_url,
            client_id,
            client_secret: if self == Self::GoogleImapSmtp {
                client_secret
            } else {
                String::new()
            },
            scopes: scopes.into(),
            redirect_uri: String::new(),
        }
    }
}

/// Safe UI/worker diagnostic vocabulary. Never carries raw provider text,
/// URLs, codes, token values, identity responses or storage errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    RegistrationRequired,
    InvalidRegistration,
    InvalidEmail,
    Cancelled,
    StaleAttempt,
    TimedOut,
    BrowserUnavailable,
    CallbackUnavailable,
    ConsentDenied,
    TenantPolicyDenied,
    NeedsReauthorization,
    TemporarilyUnavailable,
    InvalidResponse,
    MissingScope,
    IdentityUnavailable,
    EmailMismatch,
    IdentityChanged,
    IncomingProbeFailed,
    OutgoingProbeFailed,
    MailboxProbeFailed,
    Storage,
    AccountAlreadyExists,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RegistrationRequired => "Browser sign-in requires a MegaMail-owned registered desktop client ID. No provider registration is bundled. Use an approved app-password/manual path where permitted, or the separate Thunderbird runtime.",
            Self::InvalidRegistration => "The configured desktop registration is invalid.",
            Self::InvalidEmail => "Enter a valid mailbox email address.",
            Self::Cancelled => "Sign-in was cancelled. No account was added.",
            Self::StaleAttempt => "A newer sign-in replaced this attempt. No account was added.",
            Self::TimedOut => "Sign-in timed out. Start a new attempt.",
            Self::BrowserUnavailable => "The system browser could not be opened.",
            Self::CallbackUnavailable => "The local sign-in callback listener is unavailable.",
            Self::ConsentDenied => "The provider denied sign-in or consent.",
            Self::TenantPolicyDenied => "Your organization requires administrator approval or a policy this desktop client cannot satisfy.",
            Self::NeedsReauthorization => "The authorization expired or was revoked. Sign in again explicitly.",
            Self::TemporarilyUnavailable => "The provider or network is temporarily unavailable. This is not proof of revoked consent.",
            Self::InvalidResponse => "The provider returned an invalid or unsupported authorization response.",
            Self::MissingScope => "The provider did not grant all permissions required by the selected mail protocol.",
            Self::IdentityUnavailable => "The provider did not return a trustworthy mailbox identity.",
            Self::EmailMismatch => "The signed-in mailbox does not match the requested email. Start a separate attempt for the intended mailbox.",
            Self::IdentityChanged => "The authenticated provider identity changed. The existing grant was not replaced.",
            Self::IncomingProbeFailed => "Gmail IMAP authentication failed. Check account policy and IMAP access.",
            Self::OutgoingProbeFailed => "Gmail SMTP authentication failed. Sending may be disabled by account policy; no mail was sent.",
            Self::MailboxProbeFailed => "Microsoft Graph could not access the mailbox. Check tenant consent and mailbox availability.",
            Self::Storage => "Credentials or account metadata could not be saved. The previous account configuration remains active; the pending grant can be retried in this worker.",
            Self::AccountAlreadyExists => "This mailbox is already configured. This add-only transaction must not replace an existing account.",
        })
    }
}
impl std::error::Error for AuthError {}

/// Registration metadata. Values must be from MegaMail's registration, never
/// Thunderbird or another client. Presence cannot prove provider approval.
/// Not Clone/Serialize; Debug redacts the optional Google desktop secret.
pub struct Registration {
    provider: Provider,
    settings: OAuthSettings,
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registration")
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl Registration {
    /// Resolve existing MEGAMAIL_* / MegaMail oauth.toml / build configuration.
    /// Call off-thread; this may read registration metadata, not account tokens.
    pub fn configured(provider: Provider) -> Result<Self, AuthError> {
        let (client, secret) = crate::oauth::provider_credentials(provider.key());
        Self::from_owned_client(provider, client, secret)
    }

    /// Inject app-owned registration metadata, also used by synthetic tests.
    /// Caller attests ownership; this does not verify branding/consent/release.
    pub fn from_owned_client(
        provider: Provider,
        client_id: String,
        client_secret: String,
    ) -> Result<Self, AuthError> {
        if client_id.trim().is_empty() {
            return Err(AuthError::RegistrationRequired);
        }
        if client_id.len() > 1024
            || client_id
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            || client_secret.len() > 4096
            || client_secret.chars().any(char::is_control)
            || (matches!(provider, Provider::MicrosoftGraph(_)) && !client_secret.is_empty())
        {
            return Err(AuthError::InvalidRegistration);
        }
        Ok(Self {
            provider,
            settings: provider.settings(client_id, client_secret),
        })
    }
}

struct CurrentAttempt {
    generation: u64,
    cancelled: Arc<AtomicBool>,
}

/// One controller per setup surface. Beginning again cancels its older attempt.
/// Begin/check/commit serialize against each other. Call those on the worker,
/// not in rendering or input handling. Attempt::cancel is a nonblocking signal.
#[derive(Clone)]
pub struct AuthController(Arc<Mutex<CurrentAttempt>>);

impl Default for AuthController {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(CurrentAttempt {
            generation: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
        })))
    }
}

/// Opaque correlation/cancellation handle, safe to send to a worker/UI.
#[derive(Clone)]
pub struct Attempt {
    controller: AuthController,
    generation: u64,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}

impl fmt::Debug for Attempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attempt")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl AuthController {
    /// Allocate a fresh attempt. Timeout is capped at five minutes, including
    /// exchange, identity and probe work, not restarted at each stage.
    pub fn begin(&self, timeout: Duration) -> Attempt {
        let mut current = self.0.lock().unwrap_or_else(|e| e.into_inner());
        current.cancelled.store(true, Ordering::Relaxed);
        current.generation = current.generation.wrapping_add(1);
        current.cancelled = Arc::new(AtomicBool::new(false));
        Attempt {
            controller: self.clone(),
            generation: current.generation,
            cancelled: Arc::clone(&current.cancelled),
            deadline: Instant::now() + timeout.min(MAX_ATTEMPT),
        }
    }
}

impl Attempt {
    /// Nonblocking cooperative cancellation. Once commit has passed its final
    /// check under the controller lock, commit wins; cancellation cannot undo a
    /// durable write. UI must accept a SavedAccount result in this race instead
    /// of discarding it and pretending that nothing was saved.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Reject late results before applying them to the setup surface.
    pub fn check(&self) -> Result<(), AuthError> {
        let current = self.controller.0.lock().unwrap_or_else(|e| e.into_inner());
        self.check_locked(&current)
    }

    fn check_locked(&self, current: &CurrentAttempt) -> Result<(), AuthError> {
        if current.generation != self.generation {
            return Err(AuthError::StaleAttempt);
        }
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(AuthError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(AuthError::TimedOut);
        }
        Ok(())
    }

    /// Budget for each injected blocking operation. Adapters must obey it.
    pub fn remaining(&self) -> Result<Duration, AuthError> {
        self.check()?;
        Ok(self.deadline.saturating_duration_since(Instant::now()))
    }
}

/// Bounded, non-logging HTTP response. Raw data is deliberately not Debug.
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Worker-local HTTP seam. Never log request forms or bearer headers. No
/// redirects, trusted TLS, bounded time and body are required for adapters.
pub trait Http {
    fn post_form(
        &mut self,
        endpoint: &str,
        fields: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<HttpResponse, AuthError>;
    fn get_bearer(
        &mut self,
        endpoint: &str,
        token: &str,
        timeout: Duration,
    ) -> Result<HttpResponse, AuthError>;
}

/// Production HTTPS adapter; endpoints are chosen only by this catalogue.
pub struct ProviderHttp;

impl ProviderHttp {
    fn agent(timeout: Duration) -> ureq::Agent {
        let budget = timeout.min(HTTP_TIMEOUT);
        ureq::AgentBuilder::new()
            .redirects(0)
            // ureq 2.12.1's default 30s connect timeout overrides the overall
            // deadline. Cap it explicitly to this operation's remaining budget.
            .timeout_connect(budget)
            .timeout_read(budget)
            .timeout_write(budget)
            .timeout(budget)
            .build()
    }

    fn response(result: Result<ureq::Response, ureq::Error>) -> Result<HttpResponse, AuthError> {
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(_)) => return Err(AuthError::TemporarilyUnavailable),
        };
        let status = response.status();
        let mut body = Vec::new();
        response
            .into_reader()
            .take((MAX_RESPONSE + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| AuthError::TemporarilyUnavailable)?;
        if body.len() > MAX_RESPONSE {
            return Err(AuthError::InvalidResponse);
        }
        Ok(HttpResponse { status, body })
    }
}

impl Http for ProviderHttp {
    fn post_form(
        &mut self,
        endpoint: &str,
        fields: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<HttpResponse, AuthError> {
        Self::response(Self::agent(timeout).post(endpoint).send_form(fields))
    }
    fn get_bearer(
        &mut self,
        endpoint: &str,
        token: &str,
        timeout: Duration,
    ) -> Result<HttpResponse, AuthError> {
        Self::response(
            Self::agent(timeout)
                .get(endpoint)
                .set("Authorization", &format!("Bearer {token}"))
                .call(),
        )
    }
}

/// Browser-launch seam; the transaction binds its listener first and owns all
/// callback parsing. Returning success from open does not supply a code.
pub trait Browser {
    fn open(&mut self, request: &Authorization, attempt: &Attempt) -> Result<(), AuthError>;
}

/// System browser via existing portal/xdg-open code, never an embedded login.
pub struct SystemBrowser;
impl Browser for SystemBrowser {
    fn open(&mut self, request: &Authorization, attempt: &Attempt) -> Result<(), AuthError> {
        let timeout = attempt.remaining()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| AuthError::BrowserUnavailable)?;
        let portal = runtime.block_on(async {
            tokio::time::timeout(timeout.min(Duration::from_secs(10)), async {
                let connection = zbus::Connection::session().await?;
                let options: std::collections::HashMap<&str, zbus::zvariant::Value> =
                    std::collections::HashMap::new();
                connection
                    .call_method(
                        Some("org.freedesktop.portal.Desktop"),
                        "/org/freedesktop/portal/desktop",
                        Some("org.freedesktop.portal.OpenURI"),
                        "OpenURI",
                        &("", request.url.as_str(), options),
                    )
                    .await
            })
            .await
        });
        attempt.check()?;
        if matches!(portal, Ok(Ok(_))) {
            return Ok(());
        }
        std::process::Command::new("xdg-open")
            .arg(&request.url)
            .spawn()
            .map(|_| ())
            .map_err(|_| AuthError::BrowserUnavailable)
    }
}

/// Authenticated principal from a fixed provider HTTPS API. No ID-token payload
/// is decoded or treated as verified. Graph subject is its /me id, not OIDC sub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderIdentity {
    pub provider: Provider,
    pub subject: String,
    pub email: String,
}

struct Tokens {
    access: String,
    refresh: String,
    expires_at: Instant,
}

fn json(response: HttpResponse) -> Result<serde_json::Value, AuthError> {
    if response.body.len() > MAX_RESPONSE {
        return Err(AuthError::InvalidResponse);
    }
    if response.status == 429 || response.status >= 500 {
        return Err(AuthError::TemporarilyUnavailable);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&response.body).map_err(|_| AuthError::InvalidResponse)?;
    if !(200..300).contains(&response.status) {
        let error = value["error"].as_str().unwrap_or_default();
        let codes = value["error_codes"].as_array();
        let policy = codes.is_some_and(|codes| {
            codes.iter().any(|c| {
                matches!(
                    c.as_u64(),
                    Some(65001 | 65004 | 90094 | 90095 | 53000 | 53003)
                )
            })
        });
        return Err(if policy {
            AuthError::TenantPolicyDenied
        } else {
            match error {
                "invalid_grant" | "interaction_required" | "login_required" => {
                    AuthError::NeedsReauthorization
                }
                "consent_required" | "access_denied" => AuthError::ConsentDenied,
                _ if response.status == 429 || response.status >= 500 => {
                    AuthError::TemporarilyUnavailable
                }
                _ => AuthError::InvalidResponse,
            }
        });
    }
    Ok(value)
}

fn token_text(value: &serde_json::Value, key: &str) -> Result<String, AuthError> {
    let text = value[key].as_str().ok_or(AuthError::InvalidResponse)?;
    if text.is_empty()
        || text.len() > 16384
        || text.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(AuthError::InvalidResponse);
    }
    Ok(text.into())
}

fn tokens(
    response: HttpResponse,
    provider: Provider,
    previous: Option<&str>,
) -> Result<Tokens, AuthError> {
    let value = json(response)?;
    if !value["token_type"]
        .as_str()
        .is_some_and(|t| t.eq_ignore_ascii_case("bearer"))
    {
        return Err(AuthError::InvalidResponse);
    }
    // Require actual granted-scope evidence. Do not equate requested scopes with
    // granted scopes. Providers omitting this field are blocked, not guessed.
    let scope = value["scope"].as_str().ok_or(AuthError::MissingScope)?;
    let granted: Vec<&str> = scope.split_whitespace().collect();
    let required: &[&str] = match provider {
        Provider::GoogleImapSmtp => &["https://mail.google.com/"],
        Provider::MicrosoftGraph(_) => &[
            "User.Read",
            "Mail.ReadWrite",
            "Mail.Send",
            "MailboxSettings.ReadWrite",
        ],
    };
    let has = |required: &str| {
        granted.iter().any(|s| {
            *s == required
                || (matches!(provider, Provider::MicrosoftGraph(_))
                    && s.strip_prefix("https://graph.microsoft.com/") == Some(required))
        })
    };
    if !required.iter().all(|s| has(s)) {
        return Err(AuthError::MissingScope);
    }
    let seconds = value["expires_in"]
        .as_u64()
        .filter(|s| (60..=86400).contains(s))
        .ok_or(AuthError::InvalidResponse)?;
    let refresh = match value.get("refresh_token") {
        Some(_) => token_text(&value, "refresh_token")?,
        None => previous
            .map(str::to_owned)
            .ok_or(AuthError::InvalidResponse)?,
    };
    Ok(Tokens {
        access: token_text(&value, "access_token")?,
        refresh,
        expires_at: Instant::now() + Duration::from_secs(seconds - 30),
    })
}

fn identity(
    http: &mut impl Http,
    provider: Provider,
    access: &str,
    attempt: &Attempt,
) -> Result<ProviderIdentity, AuthError> {
    let endpoint = match provider {
        Provider::GoogleImapSmtp => GOOGLE_IDENTITY,
        Provider::MicrosoftGraph(_) => MICROSOFT_IDENTITY,
    };
    let value = json(http.get_bearer(endpoint, access, attempt.remaining()?)?)?;
    attempt.check()?;
    let (subject, email) = match provider {
        Provider::GoogleImapSmtp => {
            if value["email_verified"].as_bool() != Some(true) {
                return Err(AuthError::IdentityUnavailable);
            }
            (value["sub"].as_str(), value["email"].as_str())
        }
        Provider::MicrosoftGraph(_) => (
            value["id"].as_str(),
            // A UPN can be a login name rather than the primary mailbox address.
            // Do not silently turn it into mailbox identity when mail is absent.
            value["mail"].as_str().filter(|s| !s.is_empty()),
        ),
    };
    let subject = subject
        .filter(|s| !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control))
        .ok_or(AuthError::IdentityUnavailable)?;
    let email = email.ok_or(AuthError::IdentityUnavailable)?;
    crate::discovery::validate_email_address(email).map_err(|_| AuthError::IdentityUnavailable)?;
    Ok(ProviderIdentity {
        provider,
        subject: subject.into(),
        email: email.into(),
    })
}

/// Read-only/auth-only pending-grant probe. Never load the keyring, save an
/// account, issue SMTP DATA or mutate mail/flags. Adapters must obey attempt
/// cancellation/deadline. This is separate from the committed worker API.
pub trait Probe {
    fn validate(
        &mut self,
        account: &AccountConfig,
        access: &str,
        attempt: &Attempt,
    ) -> Result<(), AuthError>;
}

/// Production probe: Gmail TLS IMAP auth/logout + SMTP auth-only connection;
/// Graph Inbox-folder GET. Uses pending access token, not a stored refresh.
pub struct ProviderProbe;
impl Probe for ProviderProbe {
    fn validate(
        &mut self,
        account: &AccountConfig,
        access: &str,
        attempt: &Attempt,
    ) -> Result<(), AuthError> {
        if account.protocol == Protocol::Graph {
            let value =
                json(ProviderHttp.get_bearer(GRAPH_PROBE, access, attempt.remaining()?)?)?;
            attempt.check()?;
            if value["id"].as_str().is_none_or(str::is_empty) {
                return Err(AuthError::MailboxProbeFailed);
            }
            return Ok(());
        }
        // This adapter is not generic autoconfig. Reject accidental/custom
        // endpoints before a pending bearer token could leave the catalogue.
        if account.protocol != Protocol::Imap
            || account.imap_host != "imap.gmail.com"
            || account.imap_port != 993
            || account.smtp_host != "smtp.gmail.com"
            || account.smtp_port != 465
            || account.tls_accept_hostname_mismatch
            || account.security.as_ref().is_none_or(|tls| {
                tls.imap_starttls
                    || tls.smtp_starttls
                    || tls.imap_accept_invalid_certs
                    || tls.smtp_accept_invalid_certs
                    || tls.smtp_no_auth
            })
        {
            return Err(AuthError::InvalidRegistration);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| AuthError::IncomingProbeFailed)?;
        runtime.block_on(async {
            let work = async {
                let tcp =
                    tokio::net::TcpStream::connect((account.imap_host.as_str(), account.imap_port))
                        .await
                        .map_err(|_| AuthError::IncomingProbeFailed)?;
                let stream = async_native_tls::TlsConnector::new()
                    .connect(account.imap_host.as_str(), tcp)
                    .await
                    .map_err(|_| AuthError::IncomingProbeFailed)?;
                let mut client = async_imap::Client::new(stream);
                client
                    .read_response()
                    .await
                    .ok_or(AuthError::IncomingProbeFailed)?
                    .map_err(|_| AuthError::IncomingProbeFailed)?;
                let auth = PendingXoauth {
                    email: &account.email,
                    access,
                    first: true,
                };
                let mut session = client
                    .authenticate("XOAUTH2", auth)
                    .await
                    .map_err(|_| AuthError::IncomingProbeFailed)?;
                session
                    .logout()
                    .await
                    .map_err(|_| AuthError::IncomingProbeFailed)?;
                attempt.check()?;
                let smtp: lettre::AsyncSmtpTransport<lettre::Tokio1Executor> =
                    lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::relay(&account.smtp_host)
                        .map_err(|_| AuthError::OutgoingProbeFailed)?
                        .port(account.smtp_port)
                        .timeout(Some(Duration::from_secs(20)))
                        .credentials(lettre::transport::smtp::authentication::Credentials::new(
                            account.email.clone(),
                            access.into(),
                        ))
                        .authentication(vec![
                            lettre::transport::smtp::authentication::Mechanism::Xoauth2,
                        ])
                        .build();
                if !smtp
                    .test_connection()
                    .await
                    .map_err(|_| AuthError::OutgoingProbeFailed)?
                {
                    return Err(AuthError::OutgoingProbeFailed);
                }
                Ok(())
            };
            tokio::pin!(work);
            let deadline = tokio::time::sleep(attempt.remaining()?.min(Duration::from_secs(40)));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    result = &mut work => return result,
                    _ = &mut deadline => return Err(AuthError::TimedOut),
                    _ = tokio::time::sleep(Duration::from_millis(20)) => attempt.check()?,
                }
            }
        })?;
        attempt.check()
    }
}

struct PendingXoauth<'a> {
    email: &'a str,
    access: &'a str,
    first: bool,
}
impl async_imap::Authenticator for PendingXoauth<'_> {
    type Response = Vec<u8>;
    fn process(&mut self, _challenge: &[u8]) -> Vec<u8> {
        if std::mem::take(&mut self.first) {
            format!("user={}\x01auth=Bearer {}\x01\x01", self.email, self.access).into_bytes()
        } else {
            Vec::new()
        }
    }
}

/// Single-use browser transaction. Construct/run only in a blocking worker.
/// Listener and PKCE/code/state remain worker-local and drop on every exit.
pub struct Transaction {
    registration: Registration,
    expected_email: String,
    attempt: Attempt,
}

impl Transaction {
    pub fn new(
        registration: Registration,
        expected_email: &str,
        attempt: Attempt,
    ) -> Result<Self, AuthError> {
        crate::discovery::validate_email_address(expected_email)
            .map_err(|_| AuthError::InvalidEmail)?;
        attempt.check()?;
        Ok(Self {
            registration,
            expected_email: expected_email.trim().into(),
            attempt,
        })
    }

    /// Run the real flow through injected dependencies. Scope and identity are
    /// checked before constructing/probing an account. Returns ready, NOT saved
    /// or connected. UI must not receive its secret-bearing internals.
    pub fn authorize(
        self,
        http: &mut impl Http,
        browser: &mut impl Browser,
        probe: &mut impl Probe,
    ) -> Result<ReadyAccount, AuthError> {
        self.attempt.check()?;
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|_| AuthError::CallbackUnavailable)?;
        let port = listener
            .local_addr()
            .map_err(|_| AuthError::CallbackUnavailable)?
            .port();
        let google = self.registration.provider == Provider::GoogleImapSmtp;
        let redirect = if google {
            format!("http://127.0.0.1:{port}/")
        } else {
            format!("http://localhost:{port}")
        };
        let extra = if google {
            "&access_type=offline&prompt=consent"
        } else {
            "&prompt=select_account"
        };
        let request = Authorization::new(&self.registration.settings, &redirect, extra)
            .map_err(|_| AuthError::InvalidRegistration)?;
        browser.open(&request, &self.attempt)?;
        self.attempt.check()?;
        let code = crate::oauth::wait_for_provider_code(
            &listener,
            &request.state,
            &self.attempt.cancelled,
            self.attempt.remaining()?,
        )
        .map_err(|reason| {
            self.attempt.check().err().unwrap_or_else(|| {
                if reason.contains("AADSTS") {
                    AuthError::TenantPolicyDenied
                } else if reason.contains("timed out") {
                    AuthError::TimedOut
                } else {
                    AuthError::ConsentDenied
                }
            })
        })?;
        // Closing the listener and consuming this transaction prevents replay.
        drop(listener);
        self.attempt.check()?;
        let response = http.post_form(
            &self.registration.settings.token_url,
            &request.code_form(&self.registration.settings, &code),
            self.attempt.remaining()?,
        )?;
        self.attempt.check()?;
        let tokens = tokens(response, self.registration.provider, None)?;
        let identity = identity(
            http,
            self.registration.provider,
            &tokens.access,
            &self.attempt,
        )?;
        if !identity.email.eq_ignore_ascii_case(&self.expected_email) {
            return Err(AuthError::EmailMismatch);
        }
        let mut account = AccountConfig::new(identity.email.clone());
        account.oauth = true;
        account.oauth_settings = Some(self.registration.settings);
        account.security = Some(ServerSecurity::default());
        match self.registration.provider {
            Provider::GoogleImapSmtp => {
                account.protocol = Protocol::Imap;
                account.imap_host = "imap.gmail.com".into();
                account.imap_port = 993;
                account.smtp_host = "smtp.gmail.com".into();
                account.smtp_port = 465;
            }
            Provider::MicrosoftGraph(_) => {
                account.protocol = Protocol::Graph;
            }
        }
        probe.validate(&account, &tokens.access, &self.attempt)?;
        self.attempt.check()?;
        Ok(ReadyAccount {
            account,
            identity,
            tokens,
            attempt: self.attempt,
        })
    }
}

/// Persistence seam for add-only commit. Implementations must stage secrets
/// then atomically commit metadata and preserve the prior active configuration
/// on failure. They must reject an existing mailbox, not overwrite its grant.
pub trait AccountStore {
    fn save_new(&mut self, account: &AccountConfig) -> Result<(), AuthError>;
}

/// Existing private MegaMail config/keyring APIs, not a second secret store.
/// Parent must serialize ALL native account/lifecycle writes, including this
/// adapter: existing config APIs cannot atomically compare a roster revision.
pub struct ConfigAccountStore {
    accounts: Vec<AccountConfig>,
}
impl ConfigAccountStore {
    /// Parent supplies a checked current roster while holding its native
    /// lifecycle write lock. Never use config::load().unwrap_or_default(): it
    /// conflates an unreadable roster with an empty roster. Reserve stable IDs
    /// against Thunderbird's inventory before committing through this adapter.
    pub fn new(checked_current_accounts: Vec<AccountConfig>) -> Self {
        Self {
            accounts: checked_current_accounts,
        }
    }
}
impl AccountStore for ConfigAccountStore {
    fn save_new(&mut self, account: &AccountConfig) -> Result<(), AuthError> {
        if self
            .accounts
            .iter()
            .any(|a| a.email.trim().eq_ignore_ascii_case(account.email.trim()))
        {
            return Err(AuthError::AccountAlreadyExists);
        }
        self.accounts.push(account.clone());
        if crate::config::save(&self.accounts).is_err() {
            self.accounts.pop();
            return Err(AuthError::Storage);
        }
        // Do not retain a second pending plaintext refresh/client secret in the
        // adapter after persistence. Unrelated roster secrets remain caller-owned.
        if let Some(saved) = self.accounts.last_mut() {
            saved.oauth_refresh.clear();
            if let Some(settings) = &mut saved.oauth_settings {
                settings.client_secret.clear();
            }
        }
        Ok(())
    }
}

/// Identity + connection validated grant. Secret-bearing, not Clone/Serialize.
/// Exported account metadata has no pending refresh/access/client-secret value. Failed save
/// returns this object for bounded explicit retry, not a disconnected success.
pub struct ReadyAccount {
    account: AccountConfig,
    identity: ProviderIdentity,
    tokens: Tokens,
    attempt: Attempt,
}

impl fmt::Debug for ReadyAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadyAccount([REDACTED])")
    }
}

/// Fixed diagnostic plus retained pending grant; never format secret contents.
#[derive(Debug)]
pub struct CommitFailure {
    pub error: AuthError,
    pub pending: ReadyAccount,
}

/// Successful persistence, not a live-worker connected event. Starting/reloading
/// the worker and accepting scoped runtime events remains the native owner's job.
#[derive(Debug)]
pub struct SavedAccount {
    pub account: AccountConfig,
    pub identity: ProviderIdentity,
}

impl ReadyAccount {
    /// UI-safe metadata snapshot. Neither token nor optional client secret is
    /// exposed. Do not pass this to the existing stored-token setup probe.
    pub fn account(&self) -> AccountConfig {
        let mut account = self.account.clone();
        if let Some(settings) = &mut account.oauth_settings {
            settings.client_secret.clear();
        }
        account
    }
    pub fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    /// Refresh an expiring pending grant before commit/retry, retaining rotated
    /// refresh tokens in memory. Authenticated identity must remain unchanged;
    /// the renewed access token is probed again. No keyring access occurs here.
    /// Callers drop this grant on any identity/consent/validation error.
    pub fn renew(
        mut self,
        http: &mut impl Http,
        probe: &mut impl Probe,
    ) -> Result<Self, AuthError> {
        self.attempt.check()?;
        let settings = self
            .account
            .oauth_settings
            .as_ref()
            .ok_or(AuthError::InvalidRegistration)?;
        let mut fields = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", self.tokens.refresh.as_str()),
            ("client_id", settings.client_id.as_str()),
            ("scope", settings.scopes.as_str()),
        ];
        if !settings.client_secret.is_empty() {
            fields.push(("client_secret", settings.client_secret.as_str()));
        }
        let response = http.post_form(&settings.token_url, &fields, self.attempt.remaining()?)?;
        self.attempt.check()?;
        let fresh = tokens(response, self.identity.provider, Some(&self.tokens.refresh))?;
        // Retain rotation immediately, even if a later check/probe fails. Never
        // silently retry the old refresh token after the issuer rotated it.
        self.tokens = fresh;
        let verified = identity(
            http,
            self.identity.provider,
            &self.tokens.access,
            &self.attempt,
        )?;
        if verified != self.identity {
            return Err(AuthError::IdentityChanged);
        }
        probe.validate(&self.account, &self.tokens.access, &self.attempt)?;
        self.attempt.check()?;
        Ok(self)
    }

    /// Commit only while this attempt is current and its probed token valid.
    /// Start cannot race the storage call through this controller. Cancellation
    /// after the final check does not undo a commit; success must be reported.
    /// A failed save retains the pending grant; existing config::save stages a
    /// generation and leaves the previous metadata active. Orphan GC is separate.
    pub fn commit(mut self, store: &mut impl AccountStore) -> Result<SavedAccount, CommitFailure> {
        let controller = self.attempt.controller.clone();
        let current = controller.0.lock().unwrap_or_else(|e| e.into_inner());
        let valid = self.attempt.check_locked(&current).and_then(|_| {
            if Instant::now() >= self.tokens.expires_at {
                Err(AuthError::TimedOut)
            } else {
                Ok(())
            }
        });
        if let Err(error) = valid {
            return Err(CommitFailure {
                error,
                pending: self,
            });
        }
        self.account.oauth_refresh = std::mem::take(&mut self.tokens.refresh);
        if let Err(error) = store.save_new(&self.account) {
            self.tokens.refresh = std::mem::take(&mut self.account.oauth_refresh);
            return Err(CommitFailure {
                error,
                pending: self,
            });
        }
        self.account.oauth_refresh.clear();
        // Client secret stays in the private keyring, not returned UI metadata.
        if let Some(settings) = &mut self.account.oauth_settings {
            settings.client_secret.clear();
        }
        Ok(SavedAccount {
            account: self.account,
            identity: self.identity,
        })
    }
}
