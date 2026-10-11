//! Public-interface callback regressions. No browser, keyring, or provider I/O.
use megamail_core::config::OAuthSettings;
use megamail_core::oauth::Authorization;

fn request() -> (OAuthSettings, Authorization) {
    let settings = OAuthSettings {
        auth_url: "https://login.example.test/authorize".into(),
        token_url: "https://login.example.test/token".into(),
        client_id: "synthetic-public-client".into(),
        scopes: "mail".into(),
        ..Default::default()
    };
    let request = Authorization::new(&settings, "http://127.0.0.1:49152/", "")
        .expect("synthetic authorization request");
    (settings, request)
}

#[test]
fn denial_from_another_transaction_is_not_a_provider_refusal() {
    let (settings, request) = request();
    let callback = format!(
        "{}?error=access_denied&error_description=UNTRUSTED_REFUSAL&state=other-transaction",
        request.redirect
    );
    let outcome = request.answer(&callback, &settings);
    assert!(
        outcome.is_none()
            || matches!(outcome, Some(Err(ref error)) if error.contains("state") && !error.contains("UNTRUSTED_REFUSAL")),
        "A mismatched-state denial must be ignored or rejected as state mismatch, not attributed to the provider: {outcome:?}"
    );
}

#[test]
fn duplicate_state_cannot_complete_authorization() {
    let (settings, request) = request();
    let callback = format!(
        "{}?code=synthetic-code&state=other-transaction&state={}",
        request.redirect, request.state
    );
    assert!(
        !matches!(request.answer(&callback, &settings), Some(Ok(_))),
        "Duplicate state parameters must not authorize, regardless of order"
    );
}

#[test]
fn duplicate_code_cannot_complete_authorization() {
    let (settings, request) = request();
    let callback = format!(
        "{}?code=first-code&code=second-code&state={}",
        request.redirect, request.state
    );
    assert!(
        !matches!(request.answer(&callback, &settings), Some(Ok(_))),
        "Duplicate code parameters must not authorize"
    );
}

#[test]
fn matching_unique_callback_still_completes_authorization() {
    let (settings, request) = request();
    let callback = format!(
        "{}?code=synthetic-code&state={}",
        request.redirect, request.state
    );
    assert_eq!(
        request.answer(&callback, &settings),
        Some(Ok("synthetic-code".into()))
    );
}
