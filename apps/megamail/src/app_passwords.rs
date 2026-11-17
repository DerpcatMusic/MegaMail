//! Provider-specific app-password guidance for account setup: where to
//! create one, in a few plain steps, and how to clean what gets pasted.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Guide {
    pub provider: &'static str,
    /// The page where the app password is created.
    pub url: &'static str,
    pub steps: &'static [&'static str],
    /// Shown as a warning instead of steps when passwords no longer work.
    pub blocked: Option<&'static str>,
}

fn host_matches(host: &str, suffixes: &[&str]) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    suffixes
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
}

/// Guidance for the discovered incoming server (or the address's domain).
pub(crate) fn guide(imap_host: &str, email: &str) -> Option<Guide> {
    let domain = email.rsplit_once('@').map_or("", |(_, d)| d).to_ascii_lowercase();
    let either = |hosts: &[&str], domains: &[&str]| {
        host_matches(imap_host, hosts) || domains.contains(&domain.as_str())
    };
    if either(&["gmail.com", "googlemail.com"], &["gmail.com", "googlemail.com"]) {
        return Some(Guide {
            provider: "Google",
            url: "https://myaccount.google.com/apppasswords",
            steps: &[
                "Turn on 2-Step Verification for this Google account if it is off.",
                "Open the app passwords page, name it \"MegaMail\" and press Create.",
                "Paste the 16-letter password below (spaces are removed for you).",
            ],
            blocked: None,
        });
    }
    if either(&["mail.yahoo.com", "yahoo.com"], &["yahoo.com", "ymail.com", "rocketmail.com"]) {
        return Some(Guide {
            provider: "Yahoo",
            url: "https://login.yahoo.com/account/security/app-passwords",
            steps: &[
                "Open Account security → Generate app password.",
                "Name it \"MegaMail\" and press Generate.",
                "Paste the password below.",
            ],
            blocked: None,
        });
    }
    if either(&["aol.com"], &["aol.com"]) {
        return Some(Guide {
            provider: "AOL",
            url: "https://login.aol.com/account/security/app-passwords",
            steps: &["Open Account security → Generate app password.", "Paste it below."],
            blocked: None,
        });
    }
    if either(&["mail.me.com"], &["icloud.com", "me.com", "mac.com"]) {
        return Some(Guide {
            provider: "iCloud",
            url: "https://account.apple.com/account/manage",
            steps: &[
                "Sign in and open Sign-In and Security → App-Specific Passwords.",
                "Create one named \"MegaMail\".",
                "Paste it below (the dashes are part of it).",
            ],
            blocked: None,
        });
    }
    if either(&["fastmail.com", "messagingengine.com"], &["fastmail.com", "fastmail.fm"]) {
        return Some(Guide {
            provider: "Fastmail",
            url: "https://app.fastmail.com/settings/security/apps",
            steps: &[
                "Open Settings → Privacy & Security → Connected apps & API tokens.",
                "Create a new app password with IMAP and SMTP access.",
                "Paste it below.",
            ],
            blocked: None,
        });
    }
    if either(&["zoho.com", "zoho.eu", "zoho.in"], &["zoho.com", "zohomail.com"]) {
        return Some(Guide {
            provider: "Zoho",
            url: "https://accounts.zoho.com/home#security/app_password",
            steps: &["Open Security → App Passwords → Generate.", "Paste it below."],
            blocked: None,
        });
    }
    if either(
        &["outlook.office365.com", "imap-mail.outlook.com", "outlook.com"],
        &["outlook.com", "hotmail.com", "live.com", "msn.com"],
    ) {
        return Some(Guide {
            provider: "Microsoft",
            url: "https://account.live.com/proofs/AppPassword",
            steps: &[],
            blocked: Some(
                "Microsoft no longer accepts passwords (including app passwords) for Outlook.com and Hotmail IMAP; it requires browser sign-in. If this account is already in Thunderbird, use Find mail accounts to import it.",
            ),
        });
    }
    None
}

/// App passwords are shown grouped with spaces ("abcd efgh ijkl mnop") but
/// are sent without them. Only applied for providers known to do this.
pub(crate) fn clean(guide: Option<Guide>, password: &str) -> String {
    match guide.map(|g| g.provider) {
        Some("Google" | "Yahoo" | "AOL" | "Fastmail" | "Zoho") => {
            password.chars().filter(|c| !c.is_whitespace()).collect()
        }
        _ => password.trim_matches(['\n', '\r']).to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_are_recognized_by_server_or_address() {
        assert_eq!(guide("imap.gmail.com", "a@company.com").unwrap().provider, "Google");
        assert_eq!(guide("", "a@yahoo.com").unwrap().provider, "Yahoo");
        assert_eq!(guide("imap.mail.me.com", "a@icloud.com").unwrap().provider, "iCloud");
        assert!(guide("outlook.office365.com", "a@hotmail.com").unwrap().blocked.is_some());
        assert!(guide("mail.example.org", "a@example.org").is_none());
        assert!(guide("notgmail.com.evil", "a@x.test").is_none());
    }

    #[test]
    fn pasted_app_passwords_lose_their_grouping_spaces() {
        let google = guide("imap.gmail.com", "a@gmail.com");
        assert_eq!(clean(google, " abcd efgh ijkl mnop\n"), "abcdefghijklmnop");
        assert_eq!(clean(None, "pass word\n"), "pass word");
    }
}
