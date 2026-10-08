//! Provider-aware account setup helpers for the native client.
//!
//! UI code gathers explicit approval and credentials. This module validates
//! server settings, builds the portable account model, and resolves OAuth only
//! when MegaMail has its own usable provider registration.

use std::fmt;

use crate::config::{AccountConfig, OAuthSettings, Protocol, ServerSecurity};
use crate::discovery::{
    normalize_mail_host, validate_email_address, AuthMethod, MailServer, ServerConfigCandidate,
    TlsMode, UsernameTemplate,
};

/// A password/token input whose Debug output cannot disclose its contents.
/// This type is transient and is not serializable or cloneable.
pub struct SecretInput(String);

impl SecretInput {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretInput([REDACTED])")
    }
}

/// Authentication providers supported by MegaMail's account UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    Microsoft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthSetupError {
    UnsupportedProvider,
    MegaMailRegistrationRequired,
}

impl fmt::Display for OAuthSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedProvider => "This OAuth provider is not supported by MegaMail.",
            Self::MegaMailRegistrationRequired => {
                "OAuth is not configured for MegaMail. Use a supported system account or provider-approved sign-in method."
            }
        })
    }
}

impl std::error::Error for OAuthSetupError {}

/// Return an OAuth setup only when the app-owned client registration is usable.
/// Google/Microsoft endpoints and scopes come from the core provider catalogue;
/// client IDs/secrets come only from MegaMail config or MEGAMAIL_* settings.
pub fn oauth_settings(provider: OAuthProvider) -> Result<OAuthSettings, OAuthSetupError> {
    let name = match provider {
        OAuthProvider::Google => "google",
        OAuthProvider::Microsoft => "microsoft",
    };
    let preset = crate::oauth::preset(name).ok_or(OAuthSetupError::UnsupportedProvider)?;
    let (client_id, client_secret) = crate::oauth::provider_credentials(name);
    if client_id.trim().is_empty()
        || (provider == OAuthProvider::Google && client_secret.trim().is_empty())
    {
        return Err(OAuthSetupError::MegaMailRegistrationRequired);
    }

    Ok(OAuthSettings {
        auth_url: preset.auth_url.to_string(),
        token_url: preset.token_url.to_string(),
        client_id,
        client_secret,
        scopes: preset.scopes.to_string(),
        redirect_uri: String::new(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountSetupError {
    InvalidEmail,
    InvalidServerHost,
    InvalidPort,
    PlaintextTransportNotAllowed,
    PrivateHostNeedsExplicitApproval,
    ServerSettingsNeedExplicitReview,
    UnsupportedIncomingAuthentication,
    UnsupportedOutgoingAuthentication,
    MissingUsername,
    MissingPassword,
    InvalidUsername,
    UnsupportedProtocol,
}

impl fmt::Display for AccountSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidEmail => "Enter a valid email address.",
            Self::InvalidServerHost => "Enter a valid mail server hostname or IP address.",
            Self::InvalidPort => "Mail server ports must be between 1 and 65535.",
            Self::PlaintextTransportNotAllowed => "Mail account setup requires TLS or STARTTLS.",
            Self::PrivateHostNeedsExplicitApproval => {
                "This mail server looks local or private. Review it and confirm before continuing."
            }
            Self::ServerSettingsNeedExplicitReview => {
                "Review the discovered server settings before adding this account."
            }
            Self::UnsupportedIncomingAuthentication => {
                "This IMAP authentication method is not configured for MegaMail."
            }
            Self::UnsupportedOutgoingAuthentication => {
                "This SMTP authentication method is not configured for MegaMail."
            }
            Self::MissingUsername => "Enter the username required by the mail server.",
            Self::MissingPassword => "Enter the password or app password for this account.",
            Self::InvalidUsername => "The mail server username is not valid.",
            Self::UnsupportedProtocol => "Password onboarding currently supports IMAP accounts.",
        })
    }
}

impl std::error::Error for AccountSetupError {}

/// Build an IMAP/SMTP account from a candidate after the user has reviewed it.
/// This function does no network work and never accepts credentials. Call it
/// only in response to an explicit add/connect action, passing `true` after the
/// UI has displayed the server hosts, ports and TLS modes.
pub fn account_from_candidate(
    email: &str,
    display_name: Option<&str>,
    username_override: Option<&str>,
    candidate: &ServerConfigCandidate,
    user_confirmed_settings: bool,
) -> Result<AccountConfig, AccountSetupError> {
    if !user_confirmed_settings {
        return Err(AccountSetupError::ServerSettingsNeedExplicitReview);
    }
    let username = username_override
        .map(str::to_string)
        .unwrap_or_else(|| candidate.incoming.username.default_for(email));
    build_imap_account(
        email,
        display_name,
        &username,
        &candidate.incoming,
        &candidate.outgoing,
        user_confirmed_settings,
    )
}

/// Build a password-authenticated IMAP account from fields entered manually.
/// Local/private hosts are supported only after the user explicitly confirms
/// them (for example, a locally running mail bridge).
pub fn manual_imap_account(
    email: &str,
    display_name: Option<&str>,
    username: &str,
    incoming: &MailServer,
    outgoing: &MailServer,
    user_confirmed_settings: bool,
) -> Result<AccountConfig, AccountSetupError> {
    if !user_confirmed_settings && (incoming.is_suspicious_host() || outgoing.is_suspicious_host())
    {
        return Err(AccountSetupError::PrivateHostNeedsExplicitApproval);
    }
    if !user_confirmed_settings {
        return Err(AccountSetupError::ServerSettingsNeedExplicitReview);
    }
    build_imap_account(
        email,
        display_name,
        username,
        incoming,
        outgoing,
        user_confirmed_settings,
    )
}

fn build_imap_account(
    email: &str,
    display_name: Option<&str>,
    username: &str,
    incoming: &MailServer,
    outgoing: &MailServer,
    user_confirmed_settings: bool,
) -> Result<AccountConfig, AccountSetupError> {
    validate_email_address(email).map_err(|_| AccountSetupError::InvalidEmail)?;
    if !user_confirmed_settings && (incoming.is_suspicious_host() || outgoing.is_suspicious_host())
    {
        return Err(AccountSetupError::PrivateHostNeedsExplicitApproval);
    }
    let incoming_host = validate_server(incoming)?;
    let outgoing_host = validate_server(outgoing)?;
    if incoming.auth != AuthMethod::Password {
        return Err(AccountSetupError::UnsupportedIncomingAuthentication);
    }
    if !matches!(outgoing.auth, AuthMethod::Password | AuthMethod::None) {
        return Err(AccountSetupError::UnsupportedOutgoingAuthentication);
    }

    let username = username.trim();
    if username.is_empty() {
        return Err(AccountSetupError::MissingUsername);
    }
    if username.len() > 320 || username.chars().any(char::is_control) {
        return Err(AccountSetupError::InvalidUsername);
    }

    let smtp_username = match &outgoing.username {
        UsernameTemplate::UserProvided => username.to_string(),
        template => template.default_for(email),
    };
    let separate_smtp = outgoing.auth == AuthMethod::Password && smtp_username != username;
    let mut account = AccountConfig::new(email.trim().to_string());
    account.name = display_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(email.trim())
        .to_string();
    account.protocol = Protocol::Imap;
    account.imap_host = incoming_host;
    account.imap_port = incoming.port;
    account.smtp_host = outgoing_host;
    account.smtp_port = outgoing.port;
    account.username = username.to_string();
    account.smtp_separate = separate_smtp;
    account.smtp_username = if separate_smtp {
        smtp_username
    } else {
        String::new()
    };
    account.security = Some(ServerSecurity {
        imap_starttls: incoming.tls == TlsMode::StartTls,
        smtp_starttls: outgoing.tls == TlsMode::StartTls,
        imap_accept_invalid_certs: false,
        smtp_accept_invalid_certs: false,
        smtp_no_auth: outgoing.auth == AuthMethod::None,
    });
    Ok(account)
}

/// Attach a user-entered IMAP password to a new account. If the discovered
/// SMTP username differs, the same password is placed in the transient SMTP
/// field so `config::save` can store both entries in the MegaMail keyring.
pub fn set_password(
    account: &mut AccountConfig,
    password: SecretInput,
) -> Result<(), AccountSetupError> {
    if account.protocol != Protocol::Imap {
        return Err(AccountSetupError::UnsupportedProtocol);
    }
    if password.is_empty() {
        return Err(AccountSetupError::MissingPassword);
    }
    let value = password.into_inner();
    account.password = value.clone();
    if account.smtp_separate {
        account.smtp_password = value;
    }
    Ok(())
}

/// Supply distinct SMTP credentials when a provider requires them.
pub fn set_smtp_password(
    account: &mut AccountConfig,
    username: &str,
    password: SecretInput,
) -> Result<(), AccountSetupError> {
    if account.protocol != Protocol::Imap {
        return Err(AccountSetupError::UnsupportedProtocol);
    }
    if password.is_empty() {
        return Err(AccountSetupError::MissingPassword);
    }
    let username = username.trim();
    if username.is_empty() {
        return Err(AccountSetupError::MissingUsername);
    }
    if username.len() > 320 || username.chars().any(char::is_control) {
        return Err(AccountSetupError::InvalidUsername);
    }
    account.smtp_separate = true;
    account.smtp_username = username.to_string();
    account.smtp_password = password.into_inner();
    Ok(())
}

fn validate_server(server: &MailServer) -> Result<String, AccountSetupError> {
    if server.port == 0 {
        return Err(AccountSetupError::InvalidPort);
    }
    let host = normalize_mail_host(&server.host).ok_or(AccountSetupError::InvalidServerHost)?;
    if server.tls == TlsMode::Plaintext {
        return Err(AccountSetupError::PlaintextTransportNotAllowed);
    }
    Ok(host)
}

impl UsernameTemplate {
    /// Expand a known ISPDB username pattern to a default editable username.
    pub fn default_for(&self, email: &str) -> String {
        match self {
            Self::EmailAddress | Self::UserProvided => email.trim().to_string(),
            Self::LocalPart => email
                .trim()
                .split_once('@')
                .map(|(local, _)| local)
                .unwrap_or(email.trim())
                .to_string(),
            Self::Literal(value) => value.clone(),
            Self::Pattern(value) => {
                let email = email.trim();
                let (local, domain) = email.split_once('@').unwrap_or((email, ""));
                value
                    .replace("%EMAILADDRESS%", email)
                    .replace("%EMAILLOCALPART%", local)
                    .replace("%EMAILDOMAIN%", domain)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::DiscoverySource;

    fn endpoint(
        host: &str,
        port: u16,
        tls: TlsMode,
        auth: AuthMethod,
        username: UsernameTemplate,
    ) -> MailServer {
        MailServer {
            host: host.into(),
            port,
            tls,
            auth,
            username,
        }
    }

    #[test]
    fn account_uses_discovered_tls_and_username_without_credentials() {
        let candidate = ServerConfigCandidate {
            provider_name: "Example".into(),
            source: DiscoverySource::ThunderbirdIspdb,
            incoming: endpoint(
                "imap.example.org",
                993,
                TlsMode::ImplicitTls,
                AuthMethod::Password,
                UsernameTemplate::EmailAddress,
            ),
            outgoing: endpoint(
                "smtp.example.org",
                587,
                TlsMode::StartTls,
                AuthMethod::Password,
                UsernameTemplate::LocalPart,
            ),
            oauth_advertised: false,
            warnings: Vec::new(),
            app_password_help: None,
        };
        let account = account_from_candidate(
            "person@example.org",
            Some("Person Example"),
            None,
            &candidate,
            true,
        )
        .unwrap();
        assert_eq!(account.name, "Person Example");
        assert_eq!(account.username, "person@example.org");
        assert_eq!(account.smtp_username, "person");
        assert!(account.smtp_separate);
        assert_eq!(account.security.as_ref().unwrap().smtp_starttls, true);
        assert!(account.password.is_empty());
        assert!(account.smtp_password.is_empty());
    }

    #[test]
    fn username_patterns_are_expanded_for_defaults() {
        let pattern = UsernameTemplate::Pattern("%EMAILLOCALPART%@%EMAILDOMAIN%".into());
        assert_eq!(
            pattern.default_for("person@example.org"),
            "person@example.org"
        );
    }

    #[test]
    fn manual_user_provided_smtp_username_uses_the_entered_imap_username() {
        let incoming = endpoint(
            "imap.example.org",
            993,
            TlsMode::ImplicitTls,
            AuthMethod::Password,
            UsernameTemplate::UserProvided,
        );
        let outgoing = endpoint(
            "smtp.example.org",
            465,
            TlsMode::ImplicitTls,
            AuthMethod::Password,
            UsernameTemplate::UserProvided,
        );
        let mut account = manual_imap_account(
            "person@example.org",
            None,
            "person",
            &incoming,
            &outgoing,
            true,
        )
        .unwrap();

        assert_eq!(account.username, "person");
        assert!(!account.smtp_separate);
        assert!(account.smtp_username.is_empty());

        set_password(&mut account, SecretInput::new("test-app-password")).unwrap();
        assert_eq!(account.password, "test-app-password");
        assert!(account.smtp_password.is_empty());
    }

    #[test]
    fn plaintext_and_unreviewed_private_hosts_are_rejected() {
        let plain = endpoint(
            "imap.example.org",
            143,
            TlsMode::Plaintext,
            AuthMethod::Password,
            UsernameTemplate::EmailAddress,
        );
        let smtp = endpoint(
            "smtp.example.org",
            587,
            TlsMode::StartTls,
            AuthMethod::Password,
            UsernameTemplate::EmailAddress,
        );
        assert_eq!(
            manual_imap_account(
                "person@example.org",
                None,
                "person@example.org",
                &plain,
                &smtp,
                true
            )
            .unwrap_err(),
            AccountSetupError::PlaintextTransportNotAllowed
        );

        let local = endpoint(
            "127.0.0.1",
            1143,
            TlsMode::StartTls,
            AuthMethod::Password,
            UsernameTemplate::EmailAddress,
        );
        assert_eq!(
            manual_imap_account(
                "person@example.org",
                None,
                "person@example.org",
                &local,
                &smtp,
                false
            )
            .unwrap_err(),
            AccountSetupError::PrivateHostNeedsExplicitApproval
        );
        assert!(manual_imap_account(
            "person@example.org",
            None,
            "person@example.org",
            &local,
            &smtp,
            true
        )
        .is_ok());
    }

    #[test]
    fn secrets_are_redacted_and_attached_only_to_transient_fields() {
        let secret = SecretInput::new("not-a-real-secret");
        assert!(!format!("{secret:?}").contains("not-a-real-secret"));
        let incoming = endpoint(
            "imap.example.org",
            993,
            TlsMode::ImplicitTls,
            AuthMethod::Password,
            UsernameTemplate::EmailAddress,
        );
        let outgoing = endpoint(
            "smtp.example.org",
            465,
            TlsMode::ImplicitTls,
            AuthMethod::Password,
            UsernameTemplate::EmailAddress,
        );
        let mut account = manual_imap_account(
            "person@example.org",
            None,
            "person@example.org",
            &incoming,
            &outgoing,
            true,
        )
        .unwrap();
        set_password(&mut account, secret).unwrap();
        assert_eq!(account.password, "not-a-real-secret");
    }

    #[test]
    fn oauth_is_refused_without_a_megamail_client_registration() {
        // Production provider credentials may be supplied by environment or
        // MegaMail's oauth.toml, so assert the invariant rather than a fixed result.
        for provider in [OAuthProvider::Google, OAuthProvider::Microsoft] {
            if let Ok(settings) = oauth_settings(provider) {
                assert!(!settings.client_id.trim().is_empty());
                if provider == OAuthProvider::Google {
                    assert!(!settings.client_secret.trim().is_empty());
                }
            }
        }
    }
}
