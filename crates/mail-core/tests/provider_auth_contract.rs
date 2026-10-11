//! Fictional providers + real transaction/PKCE/strict loopback callback path.
//! No provider network, private profiles, Secret Service or actual browser.
use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

use megamail_core::config::{AccountConfig, Protocol};
use megamail_core::oauth::Authorization;
use megamail_core::provider_auth::*;

const ACCESS: &str = "SYNTHETIC_ACCESS_DO_NOT_LOG";
const REFRESH: &str = "SYNTHETIC_REFRESH_DO_NOT_LOG";
const ROTATED: &str = "SYNTHETIC_ROTATED_DO_NOT_LOG";
const SECRET: &str = "SYNTHETIC_DESKTOP_SECRET_DO_NOT_LOG";
const EMAIL: &str = "person@example.test";

fn response(status: u16, value: serde_json::Value) -> HttpResponse {
    HttpResponse {
        status,
        body: serde_json::to_vec(&value).unwrap(),
    }
}
fn token(provider: Provider, refresh: &str) -> HttpResponse {
    let scopes = match provider {
        Provider::GoogleImapSmtp => "openid email https://mail.google.com/",
        Provider::MicrosoftGraph(_) => {
            "User.Read Mail.ReadWrite Mail.Send MailboxSettings.ReadWrite"
        }
    };
    response(
        200,
        serde_json::json!({"access_token": ACCESS, "refresh_token": refresh,
        "expires_in": 3600, "token_type": "Bearer", "scope": scopes}),
    )
}
fn who(provider: Provider, email: &str, subject: &str) -> HttpResponse {
    response(
        200,
        match provider {
            Provider::GoogleImapSmtp => {
                serde_json::json!({"sub": subject, "email": email, "email_verified": true})
            }
            Provider::MicrosoftGraph(_) => {
                serde_json::json!({"id": subject, "mail": email, "userPrincipalName": email})
            }
        },
    )
}

struct FakeHttp {
    replies: VecDeque<HttpResponse>,
    posts: usize,
    gets: usize,
    posted_scopes: Vec<String>,
    posted_refreshes: Vec<String>,
    saw_secret: bool,
    cancel_on_post: Option<Attempt>,
    stale_on_get: Option<AuthController>,
}
impl FakeHttp {
    fn new(replies: impl IntoIterator<Item = HttpResponse>) -> Self {
        Self {
            replies: replies.into_iter().collect(),
            posts: 0,
            gets: 0,
            posted_scopes: Vec::new(),
            posted_refreshes: Vec::new(),
            saw_secret: false,
            cancel_on_post: None,
            stale_on_get: None,
        }
    }
}
impl Http for FakeHttp {
    fn post_form(
        &mut self,
        endpoint: &str,
        fields: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<HttpResponse, AuthError> {
        assert!(
            endpoint == "https://oauth2.googleapis.com/token"
                || endpoint.starts_with("https://login.microsoftonline.com/")
        );
        assert!(timeout > Duration::ZERO && timeout <= Duration::from_secs(300));
        assert_eq!(
            fields.iter().find(|f| f.0 == "client_id").unwrap().1,
            "synthetic-owned-client"
        );
        let grant = fields.iter().find(|f| f.0 == "grant_type").unwrap().1;
        if grant == "authorization_code" {
            assert_eq!(
                fields.iter().find(|f| f.0 == "code").unwrap().1,
                "synthetic-code"
            );
            assert!(
                fields
                    .iter()
                    .find(|f| f.0 == "code_verifier")
                    .unwrap()
                    .1
                    .len()
                    >= 43
            );
        } else {
            assert_eq!(grant, "refresh_token");
            self.posted_refreshes.push(
                fields
                    .iter()
                    .find(|f| f.0 == "refresh_token")
                    .unwrap()
                    .1
                    .into(),
            );
        }
        if let Some((_, scope)) = fields.iter().find(|f| f.0 == "scope") {
            self.posted_scopes.push((*scope).into());
        }
        self.saw_secret |= fields
            .iter()
            .any(|f| f.0 == "client_secret" && f.1 == SECRET);
        self.posts += 1;
        if let Some(attempt) = self.cancel_on_post.take() {
            attempt.cancel();
        }
        Ok(self
            .replies
            .pop_front()
            .expect("unexpected token HTTP request"))
    }
    fn get_bearer(
        &mut self,
        endpoint: &str,
        access: &str,
        timeout: Duration,
    ) -> Result<HttpResponse, AuthError> {
        assert!(endpoint == "https://openidconnect.googleapis.com/v1/userinfo"
            || endpoint == "https://graph.microsoft.com/v1.0/me?$select=id,displayName,mail,userPrincipalName");
        assert_eq!(access, ACCESS);
        assert!(timeout > Duration::ZERO && timeout <= Duration::from_secs(300));
        self.gets += 1;
        if let Some(controller) = self.stale_on_get.take() {
            controller.begin(Duration::from_secs(5));
        }
        Ok(self
            .replies
            .pop_front()
            .expect("unexpected identity HTTP request"))
    }
}

#[derive(Default)]
struct FakeBrowser {
    refusal: bool,
    wrong_then_correct: bool,
    cancel: Option<Attempt>,
    opens: usize,
}
impl Browser for FakeBrowser {
    fn open(&mut self, request: &Authorization, attempt: &Attempt) -> Result<(), AuthError> {
        self.opens += 1;
        assert!(attempt.remaining()? <= Duration::from_secs(300));
        assert!(request.url.contains("code_challenge_method=S256"));
        assert!(!request.url.contains(SECRET));
        // Connecting inside open establishes that the listener exists BEFORE
        // browser launch. Send bounded synthetic HTTP, never access a provider.
        let port = request
            .redirect
            .split(':')
            .next_back()
            .unwrap()
            .trim_end_matches('/')
            .parse::<u16>()
            .unwrap();
        if let Some(attempt) = self.cancel.take() {
            attempt.cancel();
            return Ok(());
        }
        let send = |suffix: &str| {
            let mut peer = TcpStream::connect(("127.0.0.1", port)).unwrap();
            peer.set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            peer.write_all(
                format!("GET /?{suffix} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
            )
            .unwrap();
        };
        if self.wrong_then_correct {
            send("code=untrusted&state=stale");
        }
        if self.refusal {
            send(&format!(
                "error=access_denied&error_description=AADSTS65001%20{SECRET}&state={}",
                request.state
            ));
        } else {
            send(&format!("code=synthetic-code&state={}", request.state));
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeProbe {
    calls: usize,
    failure: Option<AuthError>,
    cancel: Option<Attempt>,
}
impl Probe for FakeProbe {
    fn validate(
        &mut self,
        account: &AccountConfig,
        access: &str,
        attempt: &Attempt,
    ) -> Result<(), AuthError> {
        self.calls += 1;
        assert_eq!(access, ACCESS);
        assert!(account.oauth_refresh.is_empty()); // no premature persistence/probe keyring workaround
        assert!(account.password.is_empty());
        let tls = account.security.as_ref().unwrap();
        assert!(!tls.imap_accept_invalid_certs && !tls.smtp_accept_invalid_certs);
        assert!(!account.tls_accept_hostname_mismatch);
        if account.protocol == Protocol::Graph {
            assert!(account.imap_host.is_empty() && account.smtp_host.is_empty());
            assert!(!account
                .oauth_settings
                .as_ref()
                .unwrap()
                .scopes
                .contains("IMAP.AccessAsUser.All"));
        } else {
            assert_eq!(account.imap_host, "imap.gmail.com");
            assert_eq!(account.imap_port, 993);
            assert_eq!(account.smtp_host, "smtp.gmail.com");
            assert_eq!(account.smtp_port, 465);
        }
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        attempt.check()?;
        self.failure.map_or(Ok(()), Err)
    }
}

#[derive(Default)]
struct FakeStore {
    calls: usize,
    fail: bool,
    active: Option<String>,
    staged: Option<String>,
}
impl AccountStore for FakeStore {
    fn save_new(&mut self, account: &AccountConfig) -> Result<(), AuthError> {
        self.calls += 1;
        assert!(!account.oauth_refresh.is_empty());
        let serialized = toml::to_string(account).unwrap();
        for secret in [ACCESS, REFRESH, ROTATED, SECRET] {
            assert!(!serialized.contains(secret));
        }
        self.staged = Some(account.oauth_refresh.clone());
        if self.fail {
            return Err(AuthError::Storage);
        }
        self.active = self.staged.take();
        Ok(())
    }
}

fn transaction(provider: Provider, attempt: Attempt, secret: &str) -> Transaction {
    let registration =
        Registration::from_owned_client(provider, "synthetic-owned-client".into(), secret.into())
            .unwrap();
    Transaction::new(registration, EMAIL, attempt).unwrap()
}
fn google_ready(controller: &AuthController) -> ReadyAccount {
    let p = Provider::GoogleImapSmtp;
    transaction(p, controller.begin(Duration::from_secs(5)), "")
        .authorize(
            &mut FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]),
            &mut FakeBrowser::default(),
            &mut FakeProbe::default(),
        )
        .unwrap()
}

#[test]
fn missing_registered_client_is_blocked_before_browser_network_or_storage() {
    for p in [
        Provider::GoogleImapSmtp,
        Provider::MicrosoftGraph(MicrosoftAudience::Personal),
    ] {
        let error = Registration::from_owned_client(p, " ".into(), "".into()).unwrap_err();
        assert_eq!(error, AuthError::RegistrationRequired);
        assert!(error.to_string().contains("MegaMail-owned"));
    }
    assert_eq!(
        Registration::from_owned_client(
            Provider::MicrosoftGraph(MicrosoftAudience::WorkOrSchool),
            "synthetic-owned-client".into(),
            SECRET.into()
        )
        .unwrap_err(),
        AuthError::InvalidRegistration
    );
}

#[test]
fn google_public_desktop_no_secret_identity_probe_then_commit_not_code_receipt() {
    let p = Provider::GoogleImapSmtp;
    let mut http = FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]);
    let mut browser = FakeBrowser {
        wrong_then_correct: true,
        ..Default::default()
    };
    let mut probe = FakeProbe::default();
    let mut store = FakeStore::default();
    let ready = transaction(
        p,
        AuthController::default().begin(Duration::from_secs(5)),
        "",
    )
    .authorize(&mut http, &mut browser, &mut probe)
    .unwrap();
    assert_eq!(store.calls, 0);
    assert_eq!(probe.calls, 1);
    assert!(!http.saw_secret);
    assert_eq!(ready.identity().email, EMAIL);
    let saved = ready.commit(&mut store).unwrap();
    assert_eq!(store.active.as_deref(), Some(REFRESH));
    assert_eq!(saved.account.protocol, Protocol::Imap);
    assert!(saved.account.oauth_refresh.is_empty());
}

#[test]
fn google_optional_secret_sent_only_to_token_endpoint_and_redacted_from_metadata() {
    let p = Provider::GoogleImapSmtp;
    let mut http = FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]);
    let ready = transaction(
        p,
        AuthController::default().begin(Duration::from_secs(5)),
        SECRET,
    )
    .authorize(
        &mut http,
        &mut FakeBrowser::default(),
        &mut FakeProbe::default(),
    )
    .unwrap();
    assert!(http.saw_secret);
    assert!(ready
        .account()
        .oauth_settings
        .unwrap()
        .client_secret
        .is_empty());
    for secret in [ACCESS, REFRESH, SECRET] {
        assert!(!format!("{ready:?}").contains(secret));
    }
    let mut store = FakeStore {
        fail: true,
        ..Default::default()
    };
    let failure = ready.commit(&mut store).unwrap_err();
    for secret in [ACCESS, REFRESH, SECRET] {
        assert!(!format!("{failure:?}").contains(secret));
    }
    store.fail = false;
    let saved = failure.pending.commit(&mut store).unwrap();
    assert!(saved
        .account
        .oauth_settings
        .unwrap()
        .client_secret
        .is_empty());
}

#[test]
fn microsoft_personal_work_and_common_are_explicit_graph_authorities() {
    for (audience, authority) in [
        (MicrosoftAudience::Personal, "consumers"),
        (MicrosoftAudience::WorkOrSchool, "organizations"),
        (MicrosoftAudience::PersonalAndWork, "common"),
    ] {
        let p = Provider::MicrosoftGraph(audience);
        let ready = transaction(
            p,
            AuthController::default().begin(Duration::from_secs(5)),
            "",
        )
        .authorize(
            &mut FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "graph-object-id")]),
            &mut FakeBrowser::default(),
            &mut FakeProbe::default(),
        )
        .unwrap();
        let account = ready.account();
        assert_eq!(account.protocol, Protocol::Graph);
        let settings = account.oauth_settings.unwrap();
        assert!(settings.token_url.contains(&format!("/{authority}/")));
        assert!(!settings.scopes.contains("outlook.office.com"));
    }
}

#[test]
fn correlated_tenant_denial_is_sanitized_and_never_exchanges_code() {
    let p = Provider::MicrosoftGraph(MicrosoftAudience::WorkOrSchool);
    let mut http = FakeHttp::new([]);
    let error = transaction(
        p,
        AuthController::default().begin(Duration::from_secs(5)),
        "",
    )
    .authorize(
        &mut http,
        &mut FakeBrowser {
            refusal: true,
            ..Default::default()
        },
        &mut FakeProbe::default(),
    )
    .unwrap_err();
    assert_eq!(error, AuthError::TenantPolicyDenied);
    assert_eq!(http.posts, 0);
    assert!(!error.to_string().contains(SECRET));
}

#[test]
fn provider_token_policy_error_is_typed_and_redacted() {
    let p = Provider::MicrosoftGraph(MicrosoftAudience::WorkOrSchool);
    let reply = response(
        400,
        serde_json::json!({"error":"invalid_grant", "error_codes":[53003], "error_description":SECRET}),
    );
    let error = transaction(
        p,
        AuthController::default().begin(Duration::from_secs(5)),
        "",
    )
    .authorize(
        &mut FakeHttp::new([reply]),
        &mut FakeBrowser::default(),
        &mut FakeProbe::default(),
    )
    .unwrap_err();
    assert_eq!(error, AuthError::TenantPolicyDenied);
    assert!(!format!("{error:?}: {error}").contains(SECRET));
}

#[test]
fn mismatched_email_and_unverified_google_identity_do_not_probe_or_commit() {
    let p = Provider::GoogleImapSmtp;
    for (reply, expected) in [
        (
            who(p, "other@example.test", "stable-subject"),
            AuthError::EmailMismatch,
        ),
        (
            response(
                200,
                serde_json::json!({"sub":"s", "email":EMAIL, "email_verified":false}),
            ),
            AuthError::IdentityUnavailable,
        ),
    ] {
        let mut probe = FakeProbe::default();
        let result = transaction(
            p,
            AuthController::default().begin(Duration::from_secs(5)),
            "",
        )
        .authorize(
            &mut FakeHttp::new([token(p, REFRESH), reply]),
            &mut FakeBrowser::default(),
            &mut probe,
        );
        assert_eq!(result.unwrap_err(), expected);
        assert_eq!(probe.calls, 0);
    }
}

#[test]
fn missing_mail_scope_and_graph_scopes_for_google_are_blocked_before_identity() {
    let p = Provider::GoogleImapSmtp;
    for scope in [
        "openid email",
        "User.Read Mail.ReadWrite Mail.Send MailboxSettings.ReadWrite",
    ] {
        let reply = response(
            200,
            serde_json::json!({"access_token": ACCESS, "refresh_token": REFRESH,
            "expires_in":3600, "token_type":"Bearer", "scope":scope}),
        );
        let mut http = FakeHttp::new([reply]);
        let error = transaction(
            p,
            AuthController::default().begin(Duration::from_secs(5)),
            "",
        )
        .authorize(
            &mut http,
            &mut FakeBrowser::default(),
            &mut FakeProbe::default(),
        )
        .unwrap_err();
        assert_eq!(error, AuthError::MissingScope);
        assert_eq!(http.gets, 0);
    }
}

#[test]
fn cancelled_before_browser_and_during_exchange_cannot_produce_ready() {
    let p = Provider::GoogleImapSmtp;
    let controller = AuthController::default();
    let attempt = controller.begin(Duration::from_secs(5));
    let pending = transaction(p, attempt.clone(), "");
    attempt.cancel();
    let mut browser = FakeBrowser::default();
    assert_eq!(
        pending
            .authorize(
                &mut FakeHttp::new([]),
                &mut browser,
                &mut FakeProbe::default()
            )
            .unwrap_err(),
        AuthError::Cancelled
    );
    assert_eq!(browser.opens, 0);
    let attempt = controller.begin(Duration::from_secs(5));
    let mut http = FakeHttp::new([token(p, REFRESH)]);
    http.cancel_on_post = Some(attempt.clone());
    assert_eq!(
        transaction(p, attempt, "")
            .authorize(&mut http, &mut browser, &mut FakeProbe::default())
            .unwrap_err(),
        AuthError::Cancelled
    );
    assert_eq!(http.gets, 0);
}

#[test]
fn stale_identity_completion_and_cancelled_probe_do_not_produce_ready() {
    let p = Provider::GoogleImapSmtp;
    let controller = AuthController::default();
    let mut http = FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]);
    http.stale_on_get = Some(controller.clone());
    let mut probe = FakeProbe::default();
    assert_eq!(
        transaction(p, controller.begin(Duration::from_secs(5)), "")
            .authorize(&mut http, &mut FakeBrowser::default(), &mut probe)
            .unwrap_err(),
        AuthError::StaleAttempt
    );
    assert_eq!(probe.calls, 0);
    let attempt = controller.begin(Duration::from_secs(5));
    probe.cancel = Some(attempt.clone());
    assert_eq!(
        transaction(p, attempt, "")
            .authorize(
                &mut FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]),
                &mut FakeBrowser::default(),
                &mut probe
            )
            .unwrap_err(),
        AuthError::Cancelled
    );
}

#[test]
fn stale_ready_and_cancelled_ready_cannot_touch_secret_store() {
    let controller = AuthController::default();
    let ready = google_ready(&controller);
    controller.begin(Duration::from_secs(5));
    let mut store = FakeStore::default();
    assert_eq!(
        ready.commit(&mut store).unwrap_err().error,
        AuthError::StaleAttempt
    );
    assert_eq!(store.calls, 0);
    let attempt = controller.begin(Duration::from_secs(5));
    let p = Provider::GoogleImapSmtp;
    let ready = transaction(p, attempt.clone(), "")
        .authorize(
            &mut FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]),
            &mut FakeBrowser::default(),
            &mut FakeProbe::default(),
        )
        .unwrap();
    attempt.cancel();
    assert_eq!(
        ready.commit(&mut store).unwrap_err().error,
        AuthError::Cancelled
    );
    assert_eq!(store.calls, 0);
}

#[test]
fn storage_failure_preserves_active_grant_and_retains_pending_rotation_for_retry() {
    let controller = AuthController::default();
    let ready = google_ready(&controller);
    let mut store = FakeStore {
        fail: true,
        active: Some("previous-owned-grant".into()),
        ..Default::default()
    };
    let failure = ready.commit(&mut store).unwrap_err();
    assert_eq!(failure.error, AuthError::Storage);
    assert_eq!(store.active.as_deref(), Some("previous-owned-grant"));
    let p = Provider::GoogleImapSmtp;
    let mut http = FakeHttp::new([token(p, ROTATED), who(p, EMAIL, "stable-subject")]);
    let renewed = failure
        .pending
        .renew(&mut http, &mut FakeProbe::default())
        .unwrap();
    assert_eq!(http.posted_refreshes, [REFRESH]);
    store.fail = false;
    renewed.commit(&mut store).unwrap();
    assert_eq!(store.active.as_deref(), Some(ROTATED));
}

#[test]
fn refresh_identity_change_consumes_pending_grant_instead_of_allowing_commit() {
    let controller = AuthController::default();
    let ready = google_ready(&controller);
    let p = Provider::GoogleImapSmtp;
    let error = ready
        .renew(
            &mut FakeHttp::new([token(p, ROTATED), who(p, EMAIL, "different-subject")]),
            &mut FakeProbe::default(),
        )
        .unwrap_err();
    assert_eq!(error, AuthError::IdentityChanged);
}

#[test]
fn add_only_config_adapter_refuses_existing_mailbox_without_keyring_access() {
    let account = AccountConfig::new(EMAIL);
    let mut store = ConfigAccountStore::new(vec![account.clone()]);
    assert_eq!(
        store.save_new(&account),
        Err(AuthError::AccountAlreadyExists)
    );
}

#[test]
fn probe_failure_is_not_a_successful_account() {
    let p = Provider::GoogleImapSmtp;
    let mut probe = FakeProbe {
        failure: Some(AuthError::OutgoingProbeFailed),
        ..Default::default()
    };
    assert_eq!(
        transaction(
            p,
            AuthController::default().begin(Duration::from_secs(5)),
            ""
        )
        .authorize(
            &mut FakeHttp::new([token(p, REFRESH), who(p, EMAIL, "stable-subject")]),
            &mut FakeBrowser::default(),
            &mut probe
        )
        .unwrap_err(),
        AuthError::OutgoingProbeFailed
    );
}

#[test]
fn malformed_oversized_tokens_and_nonbearer_responses_are_bounded_and_redacted() {
    let p = Provider::GoogleImapSmtp;
    for reply in [
        HttpResponse {
            status: 200,
            body: vec![b' '; 65537],
        },
        HttpResponse {
            status: 200,
            body: format!("not-json-{SECRET}").into_bytes(),
        },
        response(
            200,
            serde_json::json!({"token_type":"Basic", "access_token": ACCESS}),
        ),
    ] {
        let error = transaction(
            p,
            AuthController::default().begin(Duration::from_secs(5)),
            "",
        )
        .authorize(
            &mut FakeHttp::new([reply]),
            &mut FakeBrowser::default(),
            &mut FakeProbe::default(),
        )
        .unwrap_err();
        assert_eq!(error, AuthError::InvalidResponse);
        assert!(!error.to_string().contains(SECRET));
    }
}

#[test]
fn production_probe_rejects_custom_endpoints_before_network_or_token_disclosure() {
    let mut account = AccountConfig::new(EMAIL);
    account.imap_host = "untrusted.example.test".into();
    let attempt = AuthController::default().begin(Duration::from_secs(5));
    assert_eq!(
        ProviderProbe.validate(&account, ACCESS, &attempt),
        Err(AuthError::InvalidRegistration)
    );
}

#[test]
fn absolute_timeout_expires_before_launch_without_any_network_or_storage() {
    let controller = AuthController::default();
    assert_eq!(
        Transaction::new(
            Registration::from_owned_client(
                Provider::GoogleImapSmtp,
                "synthetic-owned-client".into(),
                "".into()
            )
            .unwrap(),
            EMAIL,
            controller.begin(Duration::ZERO)
        )
        .err(),
        Some(AuthError::TimedOut)
    );
}

mod production_transport_budget {
    use super::*;
    use std::net::{SocketAddr, TcpListener};
    use std::sync::mpsc;
    use std::thread::{self, JoinHandle};
    use std::time::Instant;

    // Only this test adapter substitutes the catalogue token endpoint. Both
    // HTTP methods delegate to the actual ProviderHttp/ureq transport; no fake
    // HTTP success can advance identity, probe or persistence after a stall.
    struct LoopbackHttp {
        endpoint: String,
        posts: usize,
        gets: usize,
        budget: Option<Duration>,
        elapsed: Option<Duration>,
    }

    impl LoopbackHttp {
        fn new(address: SocketAddr) -> Self {
            assert_eq!(address.ip(), std::net::Ipv4Addr::LOCALHOST);
            Self {
                endpoint: format!("http://{address}/token"),
                posts: 0,
                gets: 0,
                budget: None,
                elapsed: None,
            }
        }
    }

    impl Http for LoopbackHttp {
        fn post_form(
            &mut self,
            endpoint: &str,
            fields: &[(&str, &str)],
            timeout: Duration,
        ) -> Result<HttpResponse, AuthError> {
            assert_eq!(endpoint, "https://oauth2.googleapis.com/token");
            self.posts += 1;
            self.budget = Some(timeout);
            let start = Instant::now();
            let result = ProviderHttp.post_form(&self.endpoint, fields, timeout);
            self.elapsed = Some(start.elapsed());
            result
        }

        fn get_bearer(
            &mut self,
            _endpoint: &str,
            token: &str,
            timeout: Duration,
        ) -> Result<HttpResponse, AuthError> {
            self.gets += 1;
            // Even an unexpected identity call cannot reach a real provider.
            ProviderHttp.get_bearer(&self.endpoint, token, timeout)
        }
    }

    // Accepted TCP peer with either no response headers or an incomplete body.
    // The stop channel releases it on success AND unwinding. Accept/write/hold
    // all have finite fixture deadlines, independent of the adapter under test.
    struct StalledPeer {
        address: SocketAddr,
        accepted: mpsc::Receiver<()>,
        stop: mpsc::Sender<()>,
        worker: Option<JoinHandle<()>>,
    }

    impl StalledPeer {
        fn start(prefix: &'static [u8]) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let (stop, stopped) = mpsc::channel();
            let (ready, accepted) = mpsc::channel();
            let worker = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut peer = loop {
                    match listener.accept() {
                        Ok((peer, remote)) => {
                            assert!(remote.ip().is_loopback());
                            break peer;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            if Instant::now() >= deadline
                                || stopped.recv_timeout(Duration::from_millis(5))
                                    != Err(mpsc::RecvTimeoutError::Timeout)
                            {
                                return;
                            }
                        }
                        Err(error) => panic!("loopback fixture accept failed: {error}"),
                    }
                };
                peer.set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                peer.write_all(prefix).unwrap();
                ready.send(()).unwrap();
                let _ = stopped.recv_timeout(Duration::from_secs(5));
                drop(peer);
            });
            Self {
                address,
                accepted,
                stop,
                worker: Some(worker),
            }
        }

        fn assert_accepted(&self) {
            self.accepted.recv_timeout(Duration::from_secs(1)).unwrap();
        }
    }

    impl Drop for StalledPeer {
        fn drop(&mut self) {
            let _ = self.stop.send(());
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    fn assert_authorization_stall_is_bounded(address: SocketAddr, budget: Duration) {
        let mut http = LoopbackHttp::new(address);
        let mut probe = FakeProbe::default();
        let mut store = FakeStore::default();
        let result = transaction(
            Provider::GoogleImapSmtp,
            AuthController::default().begin(budget),
            "",
        )
        .authorize(&mut http, &mut FakeBrowser::default(), &mut probe);
        let error = match result {
            Err(error) => error,
            Ok(ready) => {
                let _ = ready.commit(&mut store);
                panic!("stalled production transport returned an authorized account");
            }
        };
        assert_eq!(error, AuthError::TemporarilyUnavailable);
        assert_eq!(
            (http.posts, http.gets, probe.calls, store.calls),
            (1, 0, 0, 0)
        );
        let passed_budget = http.budget.unwrap();
        assert!(passed_budget > Duration::ZERO && passed_budget <= budget);
        let elapsed = http.elapsed.unwrap();
        assert!(
            elapsed >= passed_budget / 2,
            "transport failed before the synthetic stall: {elapsed:?}"
        );
        assert!(
            elapsed <= passed_budget + Duration::from_secs(1),
            "transport exceeded remaining budget: {elapsed:?} vs {passed_budget:?}"
        );
    }

    #[test]
    fn production_token_header_stall_stops_before_identity_probe_or_store() {
        let peer = StalledPeer::start(b"");
        assert_authorization_stall_is_bounded(peer.address, Duration::from_millis(800));
        peer.assert_accepted();
    }

    #[test]
    fn production_token_body_stall_stops_before_identity_probe_or_store() {
        let peer = StalledPeer::start(
            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{",
        );
        assert_authorization_stall_is_bounded(peer.address, Duration::from_millis(800));
        peer.assert_accepted();
    }

    #[test]
    fn production_bearer_body_read_obeys_short_budget() {
        let peer = StalledPeer::start(
            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{",
        );
        let endpoint = format!("http://{}/identity", peer.address);
        let budget = Duration::from_millis(250);
        let start = Instant::now();
        let result = ProviderHttp.get_bearer(&endpoint, ACCESS, budget);
        let elapsed = start.elapsed();
        assert_eq!(result.err(), Some(AuthError::TemporarilyUnavailable));
        peer.assert_accepted();
        assert!(elapsed >= budget / 2 && elapsed <= budget + Duration::from_secs(1));
    }

    // Requires an isolated rootless Linux network namespace with only lo and
    // an OUTPUT SYN-drop rule for 127.0.0.1:49091. Ordinary response stalls do
    // NOT exercise connect_timeout. No automatic skip or public-net fallback.
    // See batch2-provider-budget-repair.md for the parent-owned execution gate.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires isolated loopback SYN-drop fixture; parent grouped window only"]
    fn production_connect_blackhole_obeys_remaining_authorization_budget() {
        assert_eq!(
            std::env::var("MEGAMAIL_PROVIDER_BLACKHOLE").as_deref(),
            Ok("isolated-loopback")
        );
        // /proc/net is per-process; an inherited sysfs mount can still show
        // the parent's interfaces after unshare, so do not inspect sysfs here.
        let devices = std::fs::read_to_string("/proc/net/dev").unwrap();
        let interfaces: Vec<_> = devices
            .lines()
            .skip(2)
            .map(|line| line.split_once(':').unwrap().0.trim())
            .collect();
        assert_eq!(
            interfaces,
            vec!["lo"],
            "refuse a non-isolated network fixture"
        );
        let address: SocketAddr = "127.0.0.1:49091".parse().unwrap();
        // Prove the fixture drops SYNs instead of producing immediate RST or
        // establishing TCP. This does not substitute for the ProviderHttp test.
        let calibration = Duration::from_millis(150);
        let start = Instant::now();
        let error = TcpStream::connect_timeout(&address, calibration).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() >= calibration / 2);
        for budget in [Duration::from_millis(500), Duration::from_millis(1000)] {
            assert_authorization_stall_is_bounded(address, budget);
        }
    }
}
