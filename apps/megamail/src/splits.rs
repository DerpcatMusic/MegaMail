//! Split inbox and screener: People / Screener / Updates / Newsletters,
//! decided locally from the sender (no content leaves the machine).
//!
//! A sender is *known* when you have written to them (the cache's address
//! history), replied in the thread, or allowed them in the screener. Known
//! senders are People. Unknown senders that look automated are Updates or
//! Newsletters; unknown people go to the Screener, where Allow moves them to
//! People and Block keeps them out of every split (they stay in All).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Split {
    #[default]
    All,
    People,
    Screener,
    Updates,
    Newsletters,
}

impl Split {
    pub const ALL: [Split; 5] = [
        Split::All,
        Split::People,
        Split::Screener,
        Split::Updates,
        Split::Newsletters,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Split::All => "All",
            Split::People => "People",
            Split::Screener => "Screener",
            Split::Updates => "Updates",
            Split::Newsletters => "Newsletters",
        }
    }
    pub fn next(self, delta: isize) -> Split {
        let index = Self::ALL.iter().position(|s| *s == self).unwrap_or(0) as isize;
        Self::ALL[(index + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

const NEWSLETTER_LOCAL: &[&str] = &[
    "news", "newsletter", "newsletters", "digest", "weekly", "daily", "marketing", "promo",
    "promotions", "offers", "deals", "hello", "editor", "editors", "letters", "mailing",
    "campaign", "community", "info", "updates", "update", "events", "learn",
];
const AUTOMATED_LOCAL: &[&str] = &[
    "noreply", "no-reply", "no_reply", "donotreply", "do-not-reply", "do_not_reply",
    "notifications", "notification", "notify", "alerts", "alert", "mailer", "mailer-daemon",
    "postmaster", "bounce", "bounces", "automated", "robot", "bot", "system", "support",
    "billing", "receipts", "receipt", "invoice", "invoices", "orders", "order", "shipping",
    "security", "account", "accounts", "verify", "verification", "calendar", "reminders",
    "reminder", "service", "services", "admin", "team", "help", "status", "jobs", "careers",
];

fn local_part(address: &str) -> String {
    address
        .rsplit_once('@')
        .map_or(address, |(local, _)| local)
        .to_ascii_lowercase()
}

fn matches_word(local: &str, words: &[&str]) -> bool {
    let local = local.split('+').next().unwrap_or(local);
    words.iter().any(|word| {
        local == *word
            || local
                .split(['.', '-', '_'])
                .any(|part| part == *word)
            || local.starts_with(&format!("{word}-"))
            || local.starts_with(&format!("{word}_"))
    })
}

/// Which split a message from `address` belongs to.
pub(crate) fn classify(address: &str, name: &str, known: bool, screener: &Screener) -> Option<Split> {
    let key = address.trim().to_ascii_lowercase();
    if screener.blocked.contains(&key) {
        return None;
    }
    if known || screener.allowed.contains(&key) {
        return Some(Split::People);
    }
    let local = local_part(&key);
    let name = name.to_ascii_lowercase();
    if matches_word(&local, NEWSLETTER_LOCAL)
        || ["newsletter", "digest", "weekly"].iter().any(|w| name.contains(w))
    {
        return Some(Split::Newsletters);
    }
    if matches_word(&local, AUTOMATED_LOCAL) || name.contains("no-reply") || name.contains("noreply") {
        return Some(Split::Updates);
    }
    Some(Split::Screener)
}

/// The user's screener decisions, persisted in the private config directory.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Screener {
    pub allowed: HashSet<String>,
    pub blocked: HashSet<String>,
}

const FILE: &str = "screener.v1.json";
const MAX_BYTES: u64 = 2 * 1024 * 1024;

impl Screener {
    pub fn load() -> Self {
        use std::io::Read as _;
        let Some(path) = megamail_core::config::config_base().map(|dir| dir.join(FILE)) else {
            return Self::default();
        };
        let mut bytes = Vec::new();
        let ok = std::fs::File::open(path)
            .and_then(|file| file.take(MAX_BYTES).read_to_end(&mut bytes))
            .is_ok();
        if !ok {
            return Self::default();
        }
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = megamail_core::config::config_base().ok_or("Screener storage is unavailable.")?;
        let json = serde_json::to_string(self).map_err(|_| "Screener decisions could not be encoded.")?;
        megamail_core::config::write_private_file(&dir.join(FILE), &json)
            .map_err(|_| "Screener decisions could not be saved.".into())
    }

    pub fn decide(&mut self, address: &str, allow: bool) {
        let key = address.trim().to_ascii_lowercase();
        if allow {
            self.blocked.remove(&key);
            self.allowed.insert(key);
        } else {
            self.allowed.remove(&key);
            self.blocked.insert(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senders_land_in_the_right_split() {
        let mut screener = Screener::default();
        assert_eq!(classify("noreply@github.com", "GitHub", false, &screener), Some(Split::Updates));
        assert_eq!(classify("notifications+abc@service.test", "", false, &screener), Some(Split::Updates));
        assert_eq!(classify("news@brand.test", "Brand", false, &screener), Some(Split::Newsletters));
        assert_eq!(classify("hello@startup.test", "Startup Weekly", false, &screener), Some(Split::Newsletters));
        assert_eq!(classify("dana@company.test", "Dana", false, &screener), Some(Split::Screener));
        assert_eq!(classify("dana@company.test", "Dana", true, &screener), Some(Split::People));
        assert_eq!(classify("teamwork-lead@company.test", "Lead", false, &screener), Some(Split::Screener), "substring 'team' alone is not a match");
        screener.decide("Dana@Company.test", true);
        assert_eq!(classify("dana@company.test", "Dana", false, &screener), Some(Split::People));
        screener.decide("dana@company.test", false);
        assert_eq!(classify("dana@company.test", "Dana", false, &screener), None);
    }

    #[test]
    fn tab_cycles_wrap() {
        assert_eq!(Split::All.next(-1), Split::Newsletters);
        assert_eq!(Split::Newsletters.next(1), Split::All);
    }
}
