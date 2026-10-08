//! Native setup form data and conversion to the mail core's reviewed profiles.

use std::fmt;

use megamail_core::{
    config::AccountConfig,
    discovery::{AuthMethod, MailServer, ServerConfigCandidate, TlsMode, UsernameTemplate},
    onboarding::{self as core_onboarding, SecretInput},
};

/// Values displayed in the setup form. Discovery proposes server settings,
/// while the fields remain editable before a password is tested or stored.
pub struct AccountForm {
    pub email: String,
    pub name: String,
    pub username: String,
    pub password: String,
    /// Optional SMTP login override. Empty means use the IMAP username.
    pub smtp_username: String,
    /// Optional SMTP-only password. Empty means reuse the IMAP password.
    pub smtp_password: String,
    pub imap_host: String,
    pub imap_port: String,
    pub imap_tls: TlsMode,
    pub imap_auth: AuthMethod,
    pub smtp_host: String,
    pub smtp_port: String,
    pub smtp_tls: TlsMode,
    pub smtp_auth: AuthMethod,
    pub warning_reviewed: bool,
}

impl Default for AccountForm {
    fn default() -> Self {
        Self {
            email: String::new(),
            name: String::new(),
            username: String::new(),
            password: String::new(),
            smtp_username: String::new(),
            smtp_password: String::new(),
            imap_host: String::new(),
            imap_port: "993".into(),
            imap_tls: TlsMode::ImplicitTls,
            imap_auth: AuthMethod::Password,
            smtp_host: String::new(),
            smtp_port: "587".into(),
            smtp_tls: TlsMode::StartTls,
            smtp_auth: AuthMethod::Password,
            warning_reviewed: false,
        }
    }
}

impl fmt::Debug for AccountForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountForm")
            .field("email", &self.email)
            .field("name", &self.name)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("smtp_username", &self.smtp_username)
            .field("smtp_password", &"[REDACTED]")
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .finish()
    }
}

impl AccountForm {
    pub fn from_candidate(email: impl Into<String>, candidate: &ServerConfigCandidate) -> Self {
        let email = email.into();
        Self {
            name: candidate.provider_name.clone(),
            username: candidate.incoming.username.default_for(&email),
            smtp_username: candidate.outgoing.username.default_for(&email),
            imap_host: candidate.incoming.host.clone(),
            imap_port: candidate.incoming.port.to_string(),
            imap_tls: candidate.incoming.tls,
            imap_auth: candidate.incoming.auth,
            smtp_host: candidate.outgoing.host.clone(),
            smtp_port: candidate.outgoing.port.to_string(),
            smtp_tls: candidate.outgoing.tls,
            smtp_auth: candidate.outgoing.auth,
            warning_reviewed: !candidate.requires_review(),
            email,
            ..Self::default()
        }
    }

    /// Convert reviewed settings through the core's provider-aware validator,
    /// then move the transient password directly into the account model.
    pub fn build_from_candidate(
        self,
        candidate: &ServerConfigCandidate,
    ) -> Result<AccountConfig, String> {
        validate_username(&self.username, "IMAP")?;
        let smtp_username = selected_smtp_username(&self.smtp_username, &self.username)?;
        let incoming = endpoint_from_fields(
            &self.imap_host,
            &self.imap_port,
            self.imap_tls,
            self.imap_auth,
            candidate.incoming.username.clone(),
        )?;
        let outgoing_username = username_template(&smtp_username, &self.username);
        let outgoing = endpoint_from_fields(
            &self.smtp_host,
            &self.smtp_port,
            self.smtp_tls,
            self.smtp_auth,
            outgoing_username,
        )?;
        let reviewed = self.warning_reviewed;
        let mut account = core_onboarding::account_from_candidate(
            &self.email,
            Some(&self.name),
            Some(&self.username),
            &ServerConfigCandidate {
                incoming,
                outgoing,
                ..candidate.clone()
            },
            reviewed,
        )
        .map_err(|error| error.to_string())?;
        core_onboarding::set_password(&mut account, SecretInput::new(self.password))
            .map_err(|error| error.to_string())?;
        if !self.smtp_password.is_empty() {
            if self.smtp_auth != AuthMethod::Password {
                return Err("A separate SMTP password requires password authentication.".into());
            }
            core_onboarding::set_smtp_password(
                &mut account,
                &smtp_username,
                SecretInput::new(self.smtp_password),
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(account)
    }

    /// Convert a manually entered encrypted IMAP/SMTP pair using the same
    /// validation and keyring handoff as discovered settings.
    pub fn build_manual(self) -> Result<AccountConfig, String> {
        validate_username(&self.username, "IMAP")?;
        let smtp_username = selected_smtp_username(&self.smtp_username, &self.username)?;
        let incoming = endpoint_from_fields(
            &self.imap_host,
            &self.imap_port,
            self.imap_tls,
            self.imap_auth,
            UsernameTemplate::UserProvided,
        )?;
        let outgoing = endpoint_from_fields(
            &self.smtp_host,
            &self.smtp_port,
            self.smtp_tls,
            self.smtp_auth,
            username_template(&smtp_username, &self.username),
        )?;
        let mut account = core_onboarding::manual_imap_account(
            &self.email,
            Some(&self.name),
            &self.username,
            &incoming,
            &outgoing,
            self.warning_reviewed,
        )
        .map_err(|error| error.to_string())?;
        core_onboarding::set_password(&mut account, SecretInput::new(self.password))
            .map_err(|error| error.to_string())?;
        if !self.smtp_password.is_empty() {
            if self.smtp_auth != AuthMethod::Password {
                return Err("A separate SMTP password requires password authentication.".into());
            }
            core_onboarding::set_smtp_password(
                &mut account,
                &smtp_username,
                SecretInput::new(self.smtp_password),
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(account)
    }
}

fn validate_username(username: &str, protocol: &str) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err(format!("Enter the {protocol} username."));
    }
    if username.len() > 320 || username.chars().any(char::is_control) {
        return Err(format!("The {protocol} username is invalid."));
    }
    Ok(())
}

fn selected_smtp_username(entered: &str, imap_username: &str) -> Result<String, String> {
    let username = if entered.trim().is_empty() {
        imap_username.trim()
    } else {
        entered.trim()
    };
    validate_username(username, "SMTP")?;
    Ok(username.to_owned())
}

fn username_template(smtp_username: &str, imap_username: &str) -> UsernameTemplate {
    if smtp_username == imap_username.trim() {
        UsernameTemplate::UserProvided
    } else {
        UsernameTemplate::Literal(smtp_username.to_owned())
    }
}

fn endpoint_from_fields(
    host: &str,
    port: &str,
    tls: TlsMode,
    auth: AuthMethod,
    username: UsernameTemplate,
) -> Result<MailServer, String> {
    let port = port
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| "Mail server ports must be between 1 and 65535.".to_owned())?;
    Ok(MailServer {
        host: host.trim().to_owned(),
        port,
        tls,
        auth,
        username,
    })
}

#[cfg(test)]
mod tests {
    use super::AccountForm;
    use megamail_core::discovery::{
        AuthMethod, DiscoverySource, MailServer, ServerConfigCandidate, TlsMode, UsernameTemplate,
    };

    fn gmail_candidate() -> ServerConfigCandidate {
        ServerConfigCandidate {
            provider_name: "Google".into(),
            source: DiscoverySource::KnownPreset,
            incoming: MailServer {
                host: "imap.gmail.com".into(),
                port: 993,
                tls: TlsMode::ImplicitTls,
                auth: AuthMethod::Password,
                username: UsernameTemplate::EmailAddress,
            },
            outgoing: MailServer {
                host: "smtp.gmail.com".into(),
                port: 587,
                tls: TlsMode::StartTls,
                auth: AuthMethod::Password,
                username: UsernameTemplate::EmailAddress,
            },
            oauth_advertised: true,
            warnings: Vec::new(),
            app_password_help: Some("https://support.google.com/accounts/answer/185833"),
        }
    }

    #[test]
    fn candidate_fills_reviewable_tls_and_username_defaults() {
        let form = AccountForm::from_candidate("reader@gmail.com", &gmail_candidate());
        assert_eq!(form.username, "reader@gmail.com");
        assert_eq!(form.imap_host, "imap.gmail.com");
        assert_eq!(form.smtp_port, "587");
        assert_eq!(form.imap_tls, TlsMode::ImplicitTls);
        assert_eq!(form.smtp_tls, TlsMode::StartTls);
    }

    #[test]
    fn profile_builder_uses_core_tls_and_password_contract() {
        let mut form = AccountForm::from_candidate("reader@gmail.com", &gmail_candidate());
        form.password = "app-password".into();
        let account = form.build_from_candidate(&gmail_candidate()).unwrap();
        assert_eq!(account.email, "reader@gmail.com");
        assert_eq!(account.protocol, megamail_core::config::Protocol::Imap);
        assert_eq!(account.password, "app-password");
        assert!(!account.security.as_ref().unwrap().imap_starttls);
        assert!(account.security.as_ref().unwrap().smtp_starttls);
        assert!(!account.oauth);
        assert!(!account.smtp_separate);
    }

    #[test]
    fn profile_builder_preserves_distinct_smtp_username_and_password() {
        let mut form = AccountForm::from_candidate("reader@gmail.com", &gmail_candidate());
        form.password = "imap-app-password".into();
        form.smtp_username = "mailer@example.net".into();
        form.smtp_password = "smtp-app-password".into();
        let account = form.build_from_candidate(&gmail_candidate()).unwrap();
        assert!(account.smtp_separate);
        assert_eq!(account.smtp_username, "mailer@example.net");
        assert_eq!(account.smtp_password, "smtp-app-password");
    }

    #[test]
    fn manual_builder_shares_imap_username_when_smtp_override_is_blank() {
        let mut form = AccountForm::default();
        form.email = "reader@example.net".into();
        form.username = "reader".into();
        form.password = "password".into();
        form.imap_host = "imap.example.net".into();
        form.smtp_host = "smtp.example.net".into();
        form.warning_reviewed = true;
        let account = form.build_manual().unwrap();
        assert_eq!(account.username, "reader");
        assert!(!account.smtp_separate);
        assert!(account.smtp_username.is_empty());
    }

    #[test]
    fn invalid_smtp_username_is_rejected_before_keyring_handoff() {
        let mut form = AccountForm::from_candidate("reader@gmail.com", &gmail_candidate());
        form.password = "password".into();
        form.smtp_username = "bad\nuser".into();
        assert!(
            form.build_from_candidate(&gmail_candidate())
                .unwrap_err()
                .contains("SMTP username")
        );
    }

    #[test]
    fn plaintext_and_unconfigured_oauth_are_rejected_by_core_validation() {
        let mut candidate = gmail_candidate();
        candidate.incoming.tls = TlsMode::Plaintext;
        let mut form = AccountForm::from_candidate("reader@gmail.com", &candidate);
        form.password = "secret".into();
        assert!(
            form.build_from_candidate(&candidate)
                .unwrap_err()
                .contains("TLS")
        );

        let mut candidate = gmail_candidate();
        candidate.incoming.auth = AuthMethod::OAuth2;
        let mut form = AccountForm::from_candidate("reader@gmail.com", &candidate);
        form.password = "secret".into();
        assert!(
            form.build_from_candidate(&candidate)
                .unwrap_err()
                .contains("authentication")
        );
    }

    #[test]
    fn account_form_debug_redacts_password() {
        let mut form = AccountForm::from_candidate("reader@gmail.com", &gmail_candidate());
        form.password = "not-for-logs".into();
        form.smtp_password = "smtp-not-for-logs".into();
        let rendered = format!("{form:?}");
        assert!(!rendered.contains("not-for-logs"));
        assert!(!rendered.contains("smtp-not-for-logs"));
        assert!(rendered.contains("REDACTED"));
    }
}
