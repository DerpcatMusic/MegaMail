//! One-time conversion of Thunderbird accounts into native IMAP/SMTP accounts.
//! After this runs, Thunderbird is never started; MegaMail talks to the
//! servers directly with the imported credentials.
use crate::config::{AccountConfig, OAuthSettings};
use crate::discovery::{AuthMethod, MailServer, TlsMode, UsernameTemplate};
use crate::thunderbird::{
    ThunderbirdAccount, ThunderbirdAuthentication, ThunderbirdProfile, ThunderbirdSecurity,
    ThunderbirdServerSettings,
};
use crate::thunderbird_secrets::{read_logins, Login};

/// Thunderbird's own OAuth registrations, so its refresh tokens keep working.
/// Values are from Thunderbird's OAuth2Providers.sys.mjs.
pub const THUNDERBIRD_GOOGLE_CLIENT_ID: &str =
    "406964657835-aq8lmia8j95dhl1a2bvharmfk3t1hgqj.apps.googleusercontent.com";
pub const THUNDERBIRD_GOOGLE_CLIENT_SECRET: &str = "kSmqreRr0qwBWJgbf5Y-PjSU";
const THUNDERBIRD_YAHOO_CLIENT_ID: &str =
    "dj0yJmk9WVZUaWRNUUZSQTBNJmQ9WVdrOVNqbHJUMGhtTkU4bWNHbzlNQT09JnM9Y29uc3VtZXJzZWNyZXQmc3Y9MCZ4PTgz";

/// OAuth issuer for a Thunderbird server host: `(saved-login origin, settings)`.
fn oauth_issuer(host: &str) -> Option<(&'static str, OAuthSettings)> {
    let host = host.to_ascii_lowercase();
    let under = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    if under("gmail.com") || under("googlemail.com") {
        Some((
            "oauth://accounts.google.com",
            OAuthSettings {
                auth_url: "https://accounts.google.com/o/oauth2/auth".into(),
                token_url: "https://www.googleapis.com/oauth2/v3/token".into(),
                client_id: THUNDERBIRD_GOOGLE_CLIENT_ID.into(),
                client_secret: THUNDERBIRD_GOOGLE_CLIENT_SECRET.into(),
                scopes: "https://mail.google.com/".into(),
                redirect_uri: String::new(),
            },
        ))
    } else if under("yahoo.com") || under("att.net") {
        Some((
            "oauth://login.yahoo.com",
            OAuthSettings {
                auth_url: "https://api.login.yahoo.com/oauth2/request_auth".into(),
                token_url: "https://api.login.yahoo.com/oauth2/get_token".into(),
                client_id: THUNDERBIRD_YAHOO_CLIENT_ID.into(),
                client_secret: String::new(),
                scopes: "mail-w".into(),
                redirect_uri: "https://127.0.0.1".into(),
            },
        ))
    } else {
        None
    }
}

fn server(settings: &ThunderbirdServerSettings) -> Result<MailServer, String> {
    let tls = match settings.security {
        ThunderbirdSecurity::ImplicitTls => TlsMode::ImplicitTls,
        ThunderbirdSecurity::StartTls | ThunderbirdSecurity::LegacyTryStartTls => TlsMode::StartTls,
        _ => return Err(format!("{} does not use TLS; add it manually.", settings.host)),
    };
    Ok(MailServer {
        host: settings.host.clone(),
        port: settings.port,
        tls,
        // OAuth accounts are built as password accounts, then switched to XOAUTH2.
        auth: match settings.authentication {
            ThunderbirdAuthentication::None => AuthMethod::None,
            _ => AuthMethod::Password,
        },
        username: UsernameTemplate::Literal(settings.username.clone()),
    })
}

fn login<'a>(logins: &'a [Login], origin: &str, username: &str) -> Option<&'a Login> {
    let matching = || logins.iter().filter(|l| l.origin.eq_ignore_ascii_case(origin));
    matching()
        .find(|l| l.username.eq_ignore_ascii_case(username))
        .or_else(|| matching().nth(0).filter(|_| matching().count() == 1))
}

/// Build one native account, with its secrets attached for `config::save`.
pub fn convert(account: &ThunderbirdAccount, logins: &[Login]) -> Result<AccountConfig, String> {
    let email = account
        .email
        .as_deref()
        .ok_or("This Thunderbird account has no email address.")?;
    let incoming = &account.incoming;
    let outgoing = account
        .outgoing
        .as_ref()
        .ok_or("This Thunderbird account has no outgoing (SMTP) server.")?;
    let username = if incoming.username.is_empty() { email } else { &incoming.username };
    let display = account
        .identities
        .first()
        .map(|identity| identity.display_name.as_str())
        .filter(|name| !name.trim().is_empty());
    let mut config = crate::onboarding::manual_imap_account(
        email,
        display,
        username,
        &server(incoming)?,
        &server(outgoing)?,
        true,
    )
    .map_err(|e| e.to_string())?;

    if incoming.authentication == ThunderbirdAuthentication::OAuth2 {
        let (origin, settings) = oauth_issuer(&incoming.host)
            .ok_or_else(|| format!("OAuth sign-in for {} is not supported yet.", incoming.host))?;
        let token = login(logins, origin, username)
            .or_else(|| login(logins, origin, email))
            .ok_or("Thunderbird has no saved sign-in for this account. Sign in again.")?;
        config.oauth = true;
        config.oauth_settings = Some(settings);
        config.oauth_refresh = token.password.clone();
        // XOAUTH2 covers both IMAP and SMTP with the one token.
        config.smtp_separate = false;
        config.smtp_username.clear();
        return Ok(config);
    }

    let imap = login(logins, &format!("imap://{}", incoming.host), username)
        .ok_or("Thunderbird has no saved password for this account.")?;
    config.password = imap.password.clone();
    if outgoing.authentication == ThunderbirdAuthentication::Password {
        let smtp_user = if outgoing.username.is_empty() { username } else { &outgoing.username };
        let smtp = login(logins, &format!("smtp://{}", outgoing.host), smtp_user);
        match smtp {
            Some(smtp) if smtp.password != config.password || smtp_user != username => {
                config.smtp_separate = true;
                config.smtp_username = smtp_user.to_owned();
                config.smtp_password = smtp.password.clone();
            }
            _ if config.smtp_separate => config.smtp_password = config.password.clone(),
            _ => {}
        }
    }
    Ok(config)
}

/// Convert the chosen accounts of one profile. Fails as a whole only when the
/// profile's saved logins cannot be read (e.g. a Primary Password is set).
pub fn convert_profile(
    profile: &ThunderbirdProfile,
    account_ids: &[String],
) -> Result<Vec<(String, Result<AccountConfig, String>)>, String> {
    let logins = read_logins(&profile.path)?;
    Ok(profile
        .accounts
        .iter()
        .filter(|account| account_ids.contains(&account.account_id))
        .map(|account| {
            let label = account.email.clone().unwrap_or_else(|| account.name.clone());
            (label, convert(account, &logins))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(host: &str, auth: ThunderbirdAuthentication) -> ThunderbirdServerSettings {
        ThunderbirdServerSettings {
            host: host.into(),
            port: 993,
            username: "me@example.org".into(),
            security: ThunderbirdSecurity::ImplicitTls,
            authentication: auth,
        }
    }
    fn account(imap: &str, smtp: &str, auth: ThunderbirdAuthentication) -> ThunderbirdAccount {
        ThunderbirdAccount {
            account_id: "account1".into(),
            name: "Me".into(),
            email: Some("me@example.org".into()),
            username: "me@example.org".into(),
            incoming: settings(imap, auth),
            outgoing: Some(ThunderbirdServerSettings { port: 465, ..settings(smtp, auth) }),
            identities: Vec::new(),
        }
    }
    fn saved(origin: &str, password: &str) -> Login {
        Login { origin: origin.into(), username: "me@example.org".into(), password: password.into() }
    }

    #[test]
    fn password_account_takes_imap_password_and_shares_it_with_smtp() {
        let logins = [saved("imap://imap.example.org", "pw"), saved("smtp://smtp.example.org", "pw")];
        let config = convert(&account("imap.example.org", "smtp.example.org", ThunderbirdAuthentication::Password), &logins).unwrap();
        assert_eq!((config.imap_host.as_str(), config.password.as_str()), ("imap.example.org", "pw"));
        assert!(!config.smtp_separate && !config.oauth);
    }

    #[test]
    fn distinct_smtp_password_is_kept_separately() {
        let logins = [saved("imap://imap.example.org", "a"), saved("smtp://smtp.example.org", "b")];
        let config = convert(&account("imap.example.org", "smtp.example.org", ThunderbirdAuthentication::Password), &logins).unwrap();
        assert!(config.smtp_separate);
        assert_eq!(config.smtp_password, "b");
    }

    #[test]
    fn gmail_oauth_reuses_thunderbird_refresh_token() {
        let logins = [saved("oauth://accounts.google.com", "1//refresh")];
        let config = convert(&account("imap.gmail.com", "smtp.gmail.com", ThunderbirdAuthentication::OAuth2), &logins).unwrap();
        assert!(config.oauth);
        assert_eq!(config.oauth_refresh, "1//refresh");
        assert_eq!(config.oauth_settings.unwrap().client_id, THUNDERBIRD_GOOGLE_CLIENT_ID);
    }

    #[test]
    fn missing_password_is_reported_not_guessed() {
        assert!(convert(&account("imap.example.org", "smtp.example.org", ThunderbirdAuthentication::Password), &[]).is_err());
    }

    /// Converts the real profile and checks each account against its servers
    /// without saving anything. Never sends mail.
    #[test]
    #[ignore = "connects to real mail servers"]
    fn real_profile_accounts_connect() {
        let profiles = crate::thunderbird::discover_profiles().unwrap();
        for profile in profiles {
            let ids: Vec<String> = profile.accounts.iter().map(|a| a.account_id.clone()).collect();
            let Ok(converted) = convert_profile(&profile, &ids) else { continue };
            for (index, (_, result)) in converted.into_iter().enumerate() {
                let config = match result {
                    Ok(config) => config,
                    Err(e) => { println!("account {index}: convert failed: {e}"); continue }
                };
                if config.oauth {
                    let settings = config.oauth_settings.clone().unwrap();
                    let started = std::time::Instant::now();
                    let refreshed = crate::oauth::refresh_access_token(&settings, &config.oauth_refresh);
                    println!("account {index} ({}): oauth refresh ok={} in {:?} {}", config.imap_host, refreshed.is_ok(), started.elapsed(),
                        refreshed.as_ref().err().map(|e| e.chars().take(120).collect::<String>()).unwrap_or_default());
                } else {
                    let started = std::time::Instant::now();
                    let outcome = crate::worker::test_connection_blocking(config.clone());
                    println!("account {index} ({}): imap ok={} smtp ok={} in {:?}", config.imap_host,
                        outcome.incoming.is_ok(), outcome.smtp.is_ok(), started.elapsed());
                }
            }
        }
    }
}
