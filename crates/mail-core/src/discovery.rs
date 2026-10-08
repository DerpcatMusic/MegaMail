//! One-address-first account discovery.
//!
//! This module reads only Thunderbird's public ISPDB format and provider-hosted
//! autoconfig XML. Discovery never opens an IMAP/SMTP connection and never
//! handles credentials. The caller must show the returned settings and get an
//! explicit user action before any connection or credential exchange.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_XML_NODES: u32 = 20_000;
const MAX_HOST_LEN: usize = 253;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const READ_TIMEOUT: Duration = Duration::from_secs(6);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const APP_PASSWORD_HELP: &str = "https://support.google.com/accounts/answer/185833";

/// Result of trying the fixed HTTPS discovery sources for an address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryReport {
    /// The address as entered, trimmed. It is never sent to ISPDB; lookup uses
    /// only the validated domain.
    pub email: String,
    /// Known presets come first, followed by valid provider/ISPDB candidates.
    pub candidates: Vec<ServerConfigCandidate>,
    /// Network/configuration misses are retained so the UI can explain why
    /// manual setup is the next option without exposing raw response bodies.
    pub attempts: Vec<DiscoveryAttempt>,
}

/// One server pair proposed by a known-provider preset or an autoconfig file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfigCandidate {
    pub provider_name: String,
    pub source: DiscoverySource,
    pub incoming: MailServer,
    pub outgoing: MailServer,
    /// True only when this candidate's settings advertise OAuth2, or the
    /// provider is known to support it. It does not mean MegaMail is registered.
    pub oauth_advertised: bool,
    pub warnings: Vec<CandidateWarning>,
    /// Present for Gmail, where a Google app password may be used when the
    /// system-account route is not selected and the user's account permits it.
    pub app_password_help: Option<&'static str>,
}

impl ServerConfigCandidate {
    /// Whether a warning or a private/local server name needs explicit review.
    pub fn requires_review(&self) -> bool {
        !self.warnings.is_empty()
            || self.incoming.is_suspicious_host()
            || self.outgoing.is_suspicious_host()
    }

    /// A caller may pass credentials only after the user has reviewed and
    /// accepted these settings. Unsupported auth and plaintext transport stay
    /// blocked even after review.
    pub fn credentials_may_be_used_after_review(&self, user_confirmed: bool) -> bool {
        user_confirmed
            && self.incoming.tls != TlsMode::Plaintext
            && self.outgoing.tls != TlsMode::Plaintext
            && self.incoming.auth == AuthMethod::Password
            && matches!(self.outgoing.auth, AuthMethod::Password | AuthMethod::None)
    }
}

/// A single IMAP or SMTP endpoint from the proposed account configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailServer {
    pub host: String,
    pub port: u16,
    pub tls: TlsMode,
    pub auth: AuthMethod,
    pub username: UsernameTemplate,
}

impl MailServer {
    /// Does not resolve DNS. Local, private, link-local, single-label and
    /// reserved hostnames need explicit user review before credentials leave.
    pub fn is_suspicious_host(&self) -> bool {
        is_suspicious_host(&self.host)
    }
}

/// Normalize a manually entered hostname or IP literal. URLs, ports,
/// credentials, paths and invalid DNS/IP syntax are rejected.
pub fn normalize_mail_host(host: &str) -> Option<String> {
    normalize_server_host(host)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoverySource {
    KnownPreset,
    ThunderbirdIspdb,
    ProviderAutoconfigSubdomain,
    ProviderAutoconfigWellKnown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlsMode {
    /// TLS begins immediately after connecting (usually 993/465).
    ImplicitTls,
    /// The server upgrades the connection with STARTTLS (usually 143/587).
    StartTls,
    /// The XML explicitly requests unencrypted transport. Never send a
    /// password through this mode without a separate, deliberate override.
    Plaintext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    OAuth2,
    None,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsernameTemplate {
    EmailAddress,
    LocalPart,
    UserProvided,
    Literal(String),
    Pattern(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateWarning {
    IncomingTransportIsNotEncrypted,
    OutgoingTransportIsNotEncrypted,
    IncomingHostLooksPrivateOrLocal,
    OutgoingHostLooksPrivateOrLocal,
    OAuth2NeedsMegaMailProviderRegistration,
    AuthenticationMethodUnsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryAttempt {
    pub source: DiscoverySource,
    pub outcome: AttemptOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttemptOutcome {
    Found,
    NotFound,
    InvalidConfiguration,
    TooLarge,
    NetworkUnavailable,
    HttpStatus(u16),
    SkippedPrivateDomain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    InvalidEmail,
    UnsupportedInternationalizedDomain,
    InvalidXml,
    NoMatchingProvider,
    NoUsableImapSmtpPair,
    ConfigurationTooLarge,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidEmail => "Enter a valid email address.",
            Self::UnsupportedInternationalizedDomain => {
                "Use the domain's ASCII IDNA form for account discovery."
            }
            Self::InvalidXml => "The provider's account settings are not valid XML.",
            Self::NoMatchingProvider => "The settings do not match this email domain.",
            Self::NoUsableImapSmtpPair => {
                "The settings do not include usable IMAP and SMTP servers."
            }
            Self::ConfigurationTooLarge => "The provider's account settings exceed the size limit.",
        })
    }
}

impl std::error::Error for DiscoveryError {}

/// Discover a known provider preset and fetch ISPDB/provider autoconfig files.
/// This is blocking network work and must run outside the UI/render thread.
/// Only fixed HTTPS URLs derived from the validated email domain are fetched;
/// automatic redirects and XML-directed URLs are never followed.
pub fn discover(email: &str) -> Result<DiscoveryReport, DiscoveryError> {
    let email = email.trim();
    let domain = email_domain(email)?;
    let mut report = DiscoveryReport {
        email: email.to_string(),
        candidates: Vec::new(),
        attempts: Vec::new(),
    };

    if let Some(candidate) = known_preset(&domain) {
        report.candidates.push(candidate);
        return Ok(report);
    }

    if is_suspicious_host(&domain) {
        report.attempts.push(DiscoveryAttempt {
            source: DiscoverySource::ThunderbirdIspdb,
            outcome: AttemptOutcome::SkippedPrivateDomain,
        });
        report.attempts.push(DiscoveryAttempt {
            source: DiscoverySource::ProviderAutoconfigSubdomain,
            outcome: AttemptOutcome::SkippedPrivateDomain,
        });
        report.attempts.push(DiscoveryAttempt {
            source: DiscoverySource::ProviderAutoconfigWellKnown,
            outcome: AttemptOutcome::SkippedPrivateDomain,
        });
        return Ok(report);
    }

    let sources = [
        (
            DiscoverySource::ThunderbirdIspdb,
            format!("https://autoconfig.thunderbird.net/v1.1/{domain}"),
        ),
        (
            DiscoverySource::ProviderAutoconfigSubdomain,
            format!("https://autoconfig.{domain}/mail/config-v1.1.xml"),
        ),
        (
            DiscoverySource::ProviderAutoconfigWellKnown,
            format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"),
        ),
    ];

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .redirects(0)
        .build();

    for (source, url) in sources {
        let fetched = match fetch_limited(&agent, &url) {
            FetchResult::Found(bytes) => match parse_autoconfig(&bytes, email) {
                Ok(mut candidate) => {
                    candidate.source = source;
                    push_if_new(&mut report.candidates, candidate);
                    AttemptOutcome::Found
                }
                Err(DiscoveryError::ConfigurationTooLarge) => AttemptOutcome::TooLarge,
                Err(_) => AttemptOutcome::InvalidConfiguration,
            },
            FetchResult::Miss(outcome) => outcome,
        };
        report.attempts.push(DiscoveryAttempt {
            source,
            outcome: fetched,
        });
    }

    Ok(report)
}

/// Parse Thunderbird ISPDB/provider autoconfig XML without DTDs or external
/// entities. Input is byte-bounded even for local fixtures.
pub fn parse_autoconfig(
    bytes: &[u8],
    email: &str,
) -> Result<ServerConfigCandidate, DiscoveryError> {
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(DiscoveryError::ConfigurationTooLarge);
    }
    let email = email.trim();
    let requested_domain = email_domain(email)?;
    let text = std::str::from_utf8(bytes).map_err(|_| DiscoveryError::InvalidXml)?;
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
        },
    )
    .map_err(|_| DiscoveryError::InvalidXml)?;

    let root = doc.root_element();
    if !root.has_tag_name("clientConfig") {
        return Err(DiscoveryError::InvalidXml);
    }

    let provider = root
        .children()
        .find(|node| node.has_tag_name("emailProvider"))
        .ok_or(DiscoveryError::NoMatchingProvider)?;
    let matches_domain = provider
        .children()
        .filter(|node| node.has_tag_name("domain"))
        .filter_map(|node| node.text())
        .map(normalize_host)
        .any(|domain| domain.as_deref() == Some(requested_domain.as_str()));
    if !matches_domain {
        return Err(DiscoveryError::NoMatchingProvider);
    }

    let provider_name = provider
        .children()
        .find(|node| node.has_tag_name("displayName"))
        .and_then(|node| node.text())
        .map(sanitize_display_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| requested_domain.to_string());

    let incoming = select_server(
        provider
            .children()
            .filter(|node| node.has_tag_name("incomingServer"))
            .filter(|node| {
                node.attribute("type")
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("imap"))
            }),
    )
    .ok_or(DiscoveryError::NoUsableImapSmtpPair)?;
    let outgoing = select_server(
        provider
            .children()
            .filter(|node| node.has_tag_name("outgoingServer"))
            .filter(|node| {
                node.attribute("type")
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("smtp"))
            }),
    )
    .ok_or(DiscoveryError::NoUsableImapSmtpPair)?;

    let oauth_advertised =
        incoming.auth == AuthMethod::OAuth2 || outgoing.auth == AuthMethod::OAuth2;
    let mut warnings = Vec::new();
    if incoming.tls == TlsMode::Plaintext {
        warnings.push(CandidateWarning::IncomingTransportIsNotEncrypted);
    }
    if outgoing.tls == TlsMode::Plaintext {
        warnings.push(CandidateWarning::OutgoingTransportIsNotEncrypted);
    }
    if incoming.is_suspicious_host() {
        warnings.push(CandidateWarning::IncomingHostLooksPrivateOrLocal);
    }
    if outgoing.is_suspicious_host() {
        warnings.push(CandidateWarning::OutgoingHostLooksPrivateOrLocal);
    }
    if oauth_advertised {
        warnings.push(CandidateWarning::OAuth2NeedsMegaMailProviderRegistration);
    }
    if incoming.auth == AuthMethod::Unsupported || outgoing.auth == AuthMethod::Unsupported {
        warnings.push(CandidateWarning::AuthenticationMethodUnsupported);
    }

    let app_password_help = (incoming.host == "imap.gmail.com"
        && matches!(
            outgoing.host.as_str(),
            "smtp.gmail.com" | "smtp.googlemail.com"
        ))
    .then_some(APP_PASSWORD_HELP);
    Ok(ServerConfigCandidate {
        provider_name,
        source: DiscoverySource::ThunderbirdIspdb,
        incoming,
        outgoing,
        oauth_advertised,
        warnings,
        app_password_help,
    })
}

/// Validate an email address for discovery/onboarding without making a network
/// request. Internationalized domains must be supplied in their ASCII IDNA form.
pub fn validate_email_address(email: &str) -> Result<(), DiscoveryError> {
    email_domain(email.trim()).map(|_| ())
}

fn parse_server(node: roxmltree::Node<'_, '_>) -> Option<MailServer> {
    let host = normalize_server_host(child_text(node, "hostname")?)?;
    let port = child_text(node, "port")?.trim().parse::<u16>().ok()?;
    if port == 0 {
        return None;
    }
    let tls = match child_text(node, "socketType")?
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "SSL" | "TLS" => TlsMode::ImplicitTls,
        "STARTTLS" => TlsMode::StartTls,
        "PLAIN" | "NONE" => TlsMode::Plaintext,
        _ => return None,
    };
    let auth = match child_text(node, "authentication")?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "plain" | "password-cleartext" | "password" => AuthMethod::Password,
        "oauth2" => AuthMethod::OAuth2,
        "none" => AuthMethod::None,
        _ => AuthMethod::Unsupported,
    };
    let username = match child_text(node, "username")
        .unwrap_or("%EMAILADDRESS%")
        .trim()
    {
        "%EMAILADDRESS%" => UsernameTemplate::EmailAddress,
        "%EMAILLOCALPART%" => UsernameTemplate::LocalPart,
        "%USERNAME%" => UsernameTemplate::UserProvided,
        literal
            if !literal.is_empty()
                && literal.len() <= 256
                && !has_controls(literal)
                && literal.contains('%')
                && supported_username_pattern(literal) =>
        {
            UsernameTemplate::Pattern(literal.to_string())
        }
        literal
            if !literal.is_empty()
                && literal.len() <= 256
                && !has_controls(literal)
                && !literal.contains('%') =>
        {
            UsernameTemplate::Literal(literal.to_string())
        }
        _ => return None,
    };

    Some(MailServer {
        host,
        port,
        tls,
        auth,
        username,
    })
}

fn select_server<'a, 'input: 'a>(
    servers: impl Iterator<Item = roxmltree::Node<'a, 'input>>,
) -> Option<MailServer> {
    servers.filter_map(parse_server).min_by_key(|server| {
        let transport_rank = match server.tls {
            TlsMode::ImplicitTls => 0,
            TlsMode::StartTls => 1,
            TlsMode::Plaintext => 8,
        };
        let auth_rank = match server.auth {
            AuthMethod::Password => 0,
            AuthMethod::OAuth2 => 2,
            AuthMethod::None => 4,
            AuthMethod::Unsupported => 6,
        };
        transport_rank + auth_rank
    })
}

fn child_text<'a, 'input: 'a>(node: roxmltree::Node<'a, 'input>, name: &str) -> Option<&'a str> {
    node.children()
        .find(|child| child.has_tag_name(name))
        .and_then(|child| child.text())
}

fn known_preset(domain: &str) -> Option<ServerConfigCandidate> {
    let (provider_name, imap_host, smtp_host, smtp_port, smtp_tls, auth, help) = match domain {
        "gmail.com" | "googlemail.com" => (
            "Google",
            "imap.gmail.com",
            "smtp.gmail.com",
            587,
            TlsMode::StartTls,
            AuthMethod::Password,
            Some(APP_PASSWORD_HELP),
        ),
        "outlook.com" | "hotmail.com" | "live.com" => (
            "Microsoft",
            "outlook.office365.com",
            "smtp.office365.com",
            587,
            TlsMode::StartTls,
            AuthMethod::OAuth2,
            None,
        ),
        _ => return None,
    };
    let oauth = provider_name == "Google" || auth == AuthMethod::OAuth2;
    let mut warnings = Vec::new();
    if oauth && provider_name != "Google" {
        warnings.push(CandidateWarning::OAuth2NeedsMegaMailProviderRegistration);
    }
    Some(ServerConfigCandidate {
        provider_name: provider_name.to_string(),
        source: DiscoverySource::KnownPreset,
        incoming: MailServer {
            host: imap_host.to_string(),
            port: 993,
            tls: TlsMode::ImplicitTls,
            auth,
            username: UsernameTemplate::EmailAddress,
        },
        outgoing: MailServer {
            host: smtp_host.to_string(),
            port: smtp_port,
            tls: smtp_tls,
            auth,
            username: UsernameTemplate::EmailAddress,
        },
        oauth_advertised: oauth,
        warnings,
        app_password_help: help,
    })
}

fn email_domain(email: &str) -> Result<String, DiscoveryError> {
    if email.is_empty()
        || email.len() > 254
        || has_controls(email)
        || email.chars().any(char::is_whitespace)
    {
        return Err(DiscoveryError::InvalidEmail);
    }
    let (local, domain) = email.split_once('@').ok_or(DiscoveryError::InvalidEmail)?;
    if local.is_empty() || local.len() > 64 || domain.is_empty() || domain.contains('@') {
        return Err(DiscoveryError::InvalidEmail);
    }
    if !local
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&byte))
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return Err(DiscoveryError::InvalidEmail);
    }
    if !domain.is_ascii() {
        return Err(DiscoveryError::UnsupportedInternationalizedDomain);
    }
    let normalized = domain.to_ascii_lowercase();
    if !valid_dns_name(&normalized)
        || !normalized.contains('.')
        || normalized.ends_with('.')
        || normalized.parse::<IpAddr>().is_ok()
        || looks_like_ipv4(&normalized)
    {
        return Err(DiscoveryError::InvalidEmail);
    }
    Ok(normalized)
}

fn valid_server_host(host: &str) -> bool {
    if host.is_empty() || host.len() > MAX_HOST_LEN || has_controls(host) {
        return false;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return !ip.is_unspecified() && !ip.is_multicast();
    }
    if looks_like_ipv4(host) {
        return false;
    }
    if host
        .chars()
        .any(|ch| matches!(ch, '/' | '@' | ':' | '%' | '[' | ']'))
    {
        return false;
    }
    valid_dns_name(host)
}

fn valid_dns_name(host: &str) -> bool {
    if host.is_empty() || host.len() > MAX_HOST_LEN || !host.is_ascii() {
        return false;
    }
    host.trim_end_matches('.').split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.as_bytes()[0].is_ascii_alphanumeric()
            && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn normalize_host(host: &str) -> Option<String> {
    let value = host.trim().trim_end_matches('.').to_ascii_lowercase();
    valid_server_host(&value).then_some(value)
}

fn normalize_server_host(host: &str) -> Option<String> {
    let host = host.trim();
    let unwrapped = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = unwrapped.parse::<IpAddr>() {
        if ip.is_unspecified() || ip.is_multicast() {
            return None;
        }
        return Some(ip.to_string());
    }
    normalize_host(unwrapped)
}

fn is_suspicious_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(&host);
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ip) => suspicious_v4(ip),
            IpAddr::V6(ip) => suspicious_v6(ip),
        };
    }
    if looks_like_ipv4(host) {
        return true;
    }
    let reserved = [
        "localhost",
        "local",
        "internal",
        "lan",
        "home",
        "home.arpa",
        "corp",
        "private",
        "intranet",
        "localdomain",
        "test",
        "example",
        "invalid",
        "onion",
    ];
    host.split('.').count() < 2
        || reserved
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
}

fn suspicious_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || (octets[0] == 100 && (64..=127).contains(&octets[1])) // shared CGNAT range
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19)) // benchmarking
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2) // documentation
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        || octets[0] >= 240 // reserved
}

fn suspicious_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let ipv4_tail = (segments[..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff)
        || (segments[..6] == [0, 0, 0, 0, 0, 0] && (segments[6] != 0 || segments[7] > 1));
    let mapped_v4_is_suspicious = ipv4_tail
        && suspicious_v4(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ));
    mapped_v4_is_suspicious
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (ip.segments()[0] & 0xfe00) == 0xfc00 // unique-local fc00::/7
        || (ip.segments()[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8) // documentation
}

fn looks_like_ipv4(host: &str) -> bool {
    host.contains('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
}

fn sanitize_display_name(name: &str) -> String {
    name.chars()
        .filter(|ch| !ch.is_control())
        .take(120)
        .collect::<String>()
        .trim()
        .to_string()
}

fn supported_username_pattern(value: &str) -> bool {
    let remainder = value
        .replace("%EMAILADDRESS%", "")
        .replace("%EMAILLOCALPART%", "")
        .replace("%EMAILDOMAIN%", "");
    remainder.is_empty() || !remainder.contains('%')
}

fn has_controls(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn push_if_new(candidates: &mut Vec<ServerConfigCandidate>, candidate: ServerConfigCandidate) {
    let duplicate = candidates.iter().any(|old| {
        old.incoming.host == candidate.incoming.host
            && old.incoming.port == candidate.incoming.port
            && old.incoming.tls == candidate.incoming.tls
            && old.outgoing.host == candidate.outgoing.host
            && old.outgoing.port == candidate.outgoing.port
            && old.outgoing.tls == candidate.outgoing.tls
    });
    if !duplicate {
        candidates.push(candidate);
    }
}

enum FetchResult {
    Found(Vec<u8>),
    Miss(AttemptOutcome),
}

fn fetch_limited(agent: &ureq::Agent, url: &str) -> FetchResult {
    let response = match agent
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .set("Accept", "application/xml, text/xml;q=0.9")
        .set("User-Agent", "MegaMail account discovery")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return FetchResult::Miss(AttemptOutcome::NotFound),
        Err(ureq::Error::Status(status, _)) => {
            return FetchResult::Miss(AttemptOutcome::HttpStatus(status))
        }
        Err(_) => return FetchResult::Miss(AttemptOutcome::NetworkUnavailable),
    };
    if !(200..300).contains(&response.status()) {
        return FetchResult::Miss(AttemptOutcome::HttpStatus(response.status()));
    }
    if response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_CONFIG_BYTES)
    {
        return FetchResult::Miss(AttemptOutcome::TooLarge);
    }
    let mut bytes = Vec::with_capacity(16 * 1024);
    let read = response
        .into_reader()
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes);
    if read.is_err() {
        return FetchResult::Miss(AttemptOutcome::NetworkUnavailable);
    }
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return FetchResult::Miss(AttemptOutcome::TooLarge);
    }
    FetchResult::Found(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
        <clientConfig version="1.1">
          <emailProvider id="example.org">
            <domain>example.org</domain>
            <displayName>Example Mail</displayName>
            <incomingServer type="imap">
              <hostname>imap.example.org</hostname><port>993</port>
              <socketType>SSL</socketType><authentication>password-cleartext</authentication>
              <username>%EMAILADDRESS%</username>
            </incomingServer>
            <outgoingServer type="smtp">
              <hostname>smtp.example.org</hostname><port>587</port>
              <socketType>STARTTLS</socketType><authentication>plain</authentication>
              <username>%EMAILLOCALPART%</username>
            </outgoingServer>
          </emailProvider>
        </clientConfig>
    "#;

    #[test]
    fn parses_secure_imap_and_starttls_smtp() {
        let candidate = parse_autoconfig(VALID.as_bytes(), "person@example.org").unwrap();
        assert_eq!(candidate.provider_name, "Example Mail");
        assert_eq!(candidate.incoming.port, 993);
        assert_eq!(candidate.incoming.tls, TlsMode::ImplicitTls);
        assert_eq!(candidate.incoming.auth, AuthMethod::Password);
        assert_eq!(candidate.outgoing.port, 587);
        assert_eq!(candidate.outgoing.tls, TlsMode::StartTls);
        assert_eq!(candidate.outgoing.username, UsernameTemplate::LocalPart);
        assert!(!candidate.requires_review());
    }

    #[test]
    fn dtd_and_external_entities_are_rejected_without_resolution() {
        let xml = r#"<!DOCTYPE x [<!ENTITY ext SYSTEM "file:///etc/passwd">]>
            <clientConfig><emailProvider><domain>example.org</domain>
            <displayName>&ext;</displayName></emailProvider></clientConfig>"#;
        assert_eq!(
            parse_autoconfig(xml.as_bytes(), "a@example.org"),
            Err(DiscoveryError::InvalidXml)
        );
    }

    #[test]
    fn rejects_wrong_domain_and_invalid_ports() {
        assert_eq!(
            parse_autoconfig(VALID.as_bytes(), "person@elsewhere.org"),
            Err(DiscoveryError::NoMatchingProvider)
        );
        let xml = VALID.replace("<port>993</port>", "<port>0</port>");
        assert_eq!(
            parse_autoconfig(xml.as_bytes(), "person@example.org"),
            Err(DiscoveryError::NoUsableImapSmtpPair)
        );
        let xml = VALID.replace("<port>993</port>", "<port>65536</port>");
        assert_eq!(
            parse_autoconfig(xml.as_bytes(), "person@example.org"),
            Err(DiscoveryError::NoUsableImapSmtpPair)
        );
    }

    #[test]
    fn secure_server_is_preferred_to_plaintext_fallback() {
        let insecure = r#"<incomingServer type="imap"><hostname>imap.example.org</hostname>
            <port>143</port><socketType>plain</socketType><authentication>plain</authentication>
            <username>%EMAILADDRESS%</username></incomingServer>"#;
        let xml = VALID.replace(
            "<incomingServer type=\"imap\">",
            &format!("{insecure}<incomingServer type=\"imap\">"),
        );
        let candidate = parse_autoconfig(xml.as_bytes(), "person@example.org").unwrap();
        assert_eq!(candidate.incoming.tls, TlsMode::ImplicitTls);
    }

    #[test]
    fn suspicious_hosts_and_plaintext_are_visible_for_review() {
        let xml = VALID.replace("imap.example.org", "127.0.0.1").replace(
            "<socketType>STARTTLS</socketType>",
            "<socketType>plain</socketType>",
        );
        let candidate = parse_autoconfig(xml.as_bytes(), "person@example.org").unwrap();
        assert!(candidate.requires_review());
        assert!(!candidate.credentials_may_be_used_after_review(true));
        assert!(candidate.incoming.is_suspicious_host());
        assert!(candidate
            .warnings
            .contains(&CandidateWarning::OutgoingTransportIsNotEncrypted));
    }

    #[test]
    fn oauth_is_advertised_but_never_claimed_ready() {
        let xml = VALID.replace("password-cleartext", "OAuth2");
        let candidate = parse_autoconfig(xml.as_bytes(), "person@example.org").unwrap();
        assert!(candidate.oauth_advertised);
        assert!(candidate
            .warnings
            .contains(&CandidateWarning::OAuth2NeedsMegaMailProviderRegistration));
        assert!(!candidate.credentials_may_be_used_after_review(false));
    }

    #[test]
    fn known_gmail_uses_app_password_guidance_without_secret_or_oauth_claim() {
        let candidate = known_preset("gmail.com").unwrap();
        assert_eq!(candidate.incoming.host, "imap.gmail.com");
        assert_eq!(candidate.incoming.port, 993);
        assert_eq!(candidate.outgoing.host, "smtp.gmail.com");
        assert_eq!(candidate.app_password_help, Some(APP_PASSWORD_HELP));
        assert!(candidate.oauth_advertised); // Google supports OAuth; MegaMail registration is separate.
        assert!(candidate.incoming.auth == AuthMethod::Password);
    }

    #[test]
    fn known_provider_discovery_is_local_and_does_not_wait_on_network() {
        let report = discover("person@gmail.com").unwrap();
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].source, DiscoverySource::KnownPreset);
        assert!(report.attempts.is_empty());
    }

    #[test]
    fn oversized_xml_and_url_shaped_hosts_are_rejected() {
        let oversized = vec![b' '; (MAX_CONFIG_BYTES + 1) as usize];
        assert_eq!(
            parse_autoconfig(&oversized, "person@example.org"),
            Err(DiscoveryError::ConfigurationTooLarge)
        );
        let xml = VALID.replace("imap.example.org", "https://127.0.0.1/imap");
        assert_eq!(
            parse_autoconfig(xml.as_bytes(), "person@example.org"),
            Err(DiscoveryError::NoUsableImapSmtpPair)
        );
        assert_eq!(
            normalize_mail_host("https://user:pass@host.example.org/"),
            None
        );
    }

    #[test]
    fn email_domain_is_never_treated_as_a_url_or_local_target() {
        assert_eq!(
            email_domain("person@localhost"),
            Err(DiscoveryError::InvalidEmail)
        );
        assert_eq!(
            email_domain("person@127.0.0.1"),
            Err(DiscoveryError::InvalidEmail)
        );
        assert_eq!(
            email_domain("person@example.org/path"),
            Err(DiscoveryError::InvalidEmail)
        );
        assert_eq!(
            email_domain("person@exämple.org"),
            Err(DiscoveryError::UnsupportedInternationalizedDomain)
        );
        assert_eq!(
            email_domain("person@Example.ORG"),
            Ok("example.org".to_string())
        );
    }

    #[test]
    fn username_patterns_expand_without_leaving_raw_tokens() {
        let template = UsernameTemplate::Pattern("%EMAILLOCALPART%@%EMAILDOMAIN%".into());
        assert_eq!(
            template.default_for("person@example.org"),
            "person@example.org"
        );
        let xml = VALID.replace("%EMAILLOCALPART%", "%EMAILLOCALPART%@%EMAILDOMAIN%");
        let candidate = parse_autoconfig(xml.as_bytes(), "person@example.org").unwrap();
        assert_eq!(
            candidate
                .outgoing
                .username
                .default_for("person@example.org"),
            "person@example.org"
        );
    }
}
