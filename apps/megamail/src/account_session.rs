//! Saved-account inventory and reconnect policy, independent of GPUI.
//! Missing storage is empty; unreadable storage is recovery. Backend sessions
//! may fail without deleting configured identities or allocating new IDs.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use megamail_core::config::{self, AccountConfig};
use megamail_core::thunderbird::{ThunderbirdAccount, ThunderbirdProfile};
use serde::{Deserialize, Serialize};

const STORE_VERSION: u32 = 1;
const STORE_NAME: &str = "thunderbird-sources.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct SourceStore {
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<StoredProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredProfile {
    pub path: PathBuf,
    pub accounts: Vec<StoredAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredAccount {
    pub source_id: String,
    pub app_id: u32,
}

#[derive(Clone)]
pub(super) struct SelectedAccount {
    pub account: ThunderbirdAccount,
    pub app_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThunderbirdAccountState {
    Connected,
    NeedsLogin,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThunderbirdAccountOutcome {
    pub profile_path: std::path::PathBuf,
    pub source_account_id: String,
    pub name: String,
    pub email: String,
    pub state: ThunderbirdAccountState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SavedIdentity {
    pub path: PathBuf,
    pub source_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StartupRoute {
    FirstRun,
    Mailbox,
    StorageRecovery,
}

pub(super) struct StartupInventory {
    pub direct: Vec<(u32, AccountConfig)>,
    pub direct_error: Option<String>,
    pub thunderbird: Result<SourceStore, String>,
}

impl StartupInventory {
    /// Call on a background executor. Both loaders run even if one fails.
    pub fn load() -> Self {
        // Read TB first: config loading can assign missing direct IDs. Reserve
        // all durable external IDs before that side effect, not just live rows.
        let store = read_store();
        let (direct, store, identity_error) = match store {
            Ok(store) => match crate::account_registry::direct_inventory(&store) {
                Ok(_) => (
                    config::load_profiles_with_reserved(&crate::account_registry::thunderbird_ids(
                        &store,
                    )),
                    Ok(store),
                    None,
                ),
                Err(error) => {
                    // Keep known direct workers available, but never start a TB
                    // account whose durable ID may own a direct cache/Outbox.
                    let direct = config::load_profiles_existing();
                    (
                        direct,
                        Err(io::Error::new(io::ErrorKind::InvalidData, error.clone())),
                        Some(error),
                    )
                }
            },
            Err(error) => (config::load_profiles_existing(), Err(error), None),
        };
        let mut inventory = Self::load_with(|| direct, || store);
        if let Some(error) = identity_error {
            inventory.direct_error.get_or_insert(error);
        }
        inventory
    }

    fn load_with(
        direct: impl FnOnce() -> io::Result<Vec<(u32, AccountConfig)>>,
        thunderbird: impl FnOnce() -> io::Result<SourceStore>,
    ) -> Self {
        let direct = direct();
        let thunderbird = thunderbird()
            .map_err(|error| format!("Could not read saved Thunderbird accounts: {error}. Repair the account storage, then Retry."));
        let (direct, direct_error) = match direct {
            Ok(profiles) => (profiles, None),
            Err(error) => (
                Vec::new(),
                Some(format!(
                    "Could not load MegaMail accounts: {error}. Repair the account storage, then Retry."
                )),
            ),
        };
        Self {
            direct,
            direct_error,
            thunderbird,
        }
    }

    pub fn route(&self) -> StartupRoute {
        if self.direct_error.is_some() || self.thunderbird.is_err() {
            StartupRoute::StorageRecovery
        } else if self.direct.is_empty()
            && self
                .thunderbird
                .as_ref()
                .is_ok_and(|store| store.profiles.iter().all(|p| p.accounts.is_empty()))
        {
            StartupRoute::FirstRun
        } else {
            StartupRoute::Mailbox
        }
    }
}

impl SourceStore {
    pub fn account_id(&self, path: &Path, source_id: &str) -> Option<u32> {
        self.profiles
            .iter()
            .find(|profile| profile.path == path)
            .and_then(|profile| {
                profile
                    .accounts
                    .iter()
                    .find(|account| account.source_id == source_id)
            })
            .map(|account| account.app_id)
    }

    pub fn remember(&mut self, app_id: u32, source: SavedIdentity) {
        if self.account_id(&source.path, &source.source_id).is_some() {
            return;
        }
        if let Some(profile) = self
            .profiles
            .iter_mut()
            .find(|profile| profile.path == source.path)
        {
            profile.accounts.push(StoredAccount {
                app_id,
                source_id: source.source_id,
            });
        } else {
            self.profiles.push(StoredProfile {
                path: source.path,
                accounts: vec![StoredAccount {
                    app_id,
                    source_id: source.source_id,
                }],
            });
        }
    }

    /// v1 has no email metadata. Never invent a sender address for a dormant row.
    pub fn dormant_accounts(&self) -> Vec<megamail_core::models::Account> {
        self.profiles
            .iter()
            .flat_map(|profile| {
                profile.accounts.iter().map(|account| {
                    let label = format!("Saved Thunderbird account {}", account.app_id);
                    megamail_core::models::Account {
                        id: account.app_id,
                        name: label.clone(),
                        email: String::new(),
                        label,
                        accent: "#6c8bc4".into(),
                    }
                })
            })
            .collect()
    }
}

/// Persist complete authorized intent before attempting any runtime connection.
/// The same seam is used by explicit import and synthetic failure regressions.
pub(super) fn commit_selection_with<T>(
    store: &mut SourceStore,
    selected: &[(ThunderbirdProfile, Vec<SelectedAccount>)],
    persist: impl FnOnce(&SourceStore) -> Result<(), String>,
    connect: impl FnOnce() -> T,
) -> Result<T, String> {
    let mut candidate = store.clone();
    for (profile, accounts) in selected {
        for selected in accounts {
            if candidate
                .account_id(&profile.path, &selected.account.account_id)
                .is_some_and(|id| id != selected.app_id)
            {
                return Err("Selected Thunderbird identity has a conflicting saved ID; no account was changed.".into());
            }
            candidate.remember(
                selected.app_id,
                SavedIdentity {
                    path: profile.path.clone(),
                    source_id: selected.account.account_id.clone(),
                },
            );
        }
    }
    validate_store(&candidate).map_err(|error| error.to_string())?;
    persist(&candidate)?;
    *store = candidate;
    Ok(connect())
}

pub(super) fn store_path() -> io::Result<PathBuf> {
    config::config_base()
        .map(|path| path.join(STORE_NAME))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not locate MegaMail's private config directory",
            )
        })
}

pub(super) fn read_store() -> io::Result<SourceStore> {
    read_store_at(&store_path()?)
}

pub(super) fn read_store_at(path: &Path) -> io::Result<SourceStore> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SourceStore {
                version: STORE_VERSION,
                profiles: Vec::new(),
            });
        }
        Err(error) => return Err(error),
    };
    const MAX_STORE_BYTES: usize = 256 * 1024;
    let mut bytes = Vec::with_capacity(MAX_STORE_BYTES.min(16 * 1024));
    file.by_ref()
        .take((MAX_STORE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_STORE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Thunderbird source manifest is too large",
        ));
    }
    let store: SourceStore = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if store.version != STORE_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported Thunderbird source manifest version",
        ));
    }
    validate_store(&store)?;
    Ok(store)
}

pub(super) fn validate_store(store: &SourceStore) -> io::Result<()> {
    if store.profiles.len() > 32 || store.profiles.iter().any(|p| p.accounts.len() > 32) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "too many saved Thunderbird sources",
        ));
    }
    let mut profile_paths = HashSet::new();
    let mut source_accounts = HashSet::new();
    let mut app_ids = HashSet::new();
    for profile in &store.profiles {
        if !profile.path.is_absolute() || !profile_paths.insert(profile.path.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Thunderbird source manifest has a duplicate or invalid profile path",
            ));
        }
        for account in &profile.accounts {
            if account.source_id.trim().is_empty()
                || account.source_id.len() > 128
                || account.source_id.chars().any(char::is_control)
                || account.app_id == 0
                || !source_accounts.insert((profile.path.clone(), account.source_id.clone()))
                || !app_ids.insert(account.app_id)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Thunderbird source manifest has a duplicate or invalid account ID",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn restore_with<T>(
    mut store: SourceStore,
    healthy: HashMap<u32, SavedIdentity>,
    mut discover: impl FnMut() -> Result<Vec<ThunderbirdProfile>, String>,
    mut start: impl FnMut(
        ThunderbirdProfile,
        Vec<SelectedAccount>,
        HashSet<u32>,
        HashSet<u32>,
        HashSet<String>,
    ) -> Result<
        (Vec<T>, Vec<ThunderbirdAccountOutcome>),
        (ThunderbirdAccountState, String),
    >,
    reserved_ids: HashSet<u32>,
    reconnectable_ids: HashSet<u32>,
    existing_emails: Vec<String>,
) -> Result<(Vec<T>, Vec<ThunderbirdAccountOutcome>, Option<String>), String> {
    let mut results = Vec::new();
    // Match qualified ownership before discovery: a healthy source is never
    // reopened just because a sibling needs Retry. Numeric equality alone
    // must not turn a direct-account collision into "already connected".
    for profile in &mut store.profiles {
        profile.accounts.retain(|account| {
            let identity = SavedIdentity {
                path: profile.path.clone(),
                source_id: account.source_id.clone(),
            };
            if healthy.get(&account.app_id) == Some(&identity) {
                return false;
            }
            if reserved_ids.contains(&account.app_id) {
                results.push(ThunderbirdAccountOutcome {
                    profile_path: profile.path.clone(),
                    source_account_id: account.source_id.clone(),
                    name: format!("Saved Thunderbird account {}", account.app_id),
                    email: String::new(),
                    state: ThunderbirdAccountState::Failed,
                    message: Some("This saved account ID belongs to another account. Retry cannot renumber it; resolve the identity conflict before connecting.".into()),
                });
                return false;
            }
            true
        });
    }
    store
        .profiles
        .retain(|profile| !profile.accounts.is_empty());
    if store.profiles.is_empty() {
        return Ok((Vec::new(), results, None));
    }
    let available = discover()?;
    let mut by_path = HashMap::new();
    for profile in available {
        if let Ok(path) = fs::canonicalize(&profile.path) {
            by_path.insert(path, profile);
        }
    }
    let mut started = Vec::new();
    let mut connected_ids = reserved_ids;
    let mut connected_emails: HashSet<String> = existing_emails
        .into_iter()
        .map(|email| normalize_email(&email))
        .filter(|email| !email.is_empty())
        .collect();
    let mut claimed_ids = connected_ids.clone();
    let mut claimed_emails = connected_emails.clone();
    for stored in store.profiles {
        let canonical = fs::canonicalize(&stored.path).unwrap_or(stored.path.clone());
        let Some(mut profile) = by_path.remove(&canonical) else {
            for account in stored.accounts {
                results.push(ThunderbirdAccountOutcome {
                    profile_path: stored.path.clone(),
                    source_account_id: account.source_id,
                    name: "Thunderbird account".into(),
                    email: String::new(),
                    state: ThunderbirdAccountState::Failed,
                    message: Some("The saved Thunderbird source profile is unavailable. Restore it at its saved location, then Retry.".into()),
                });
            }
            continue;
        };
        profile.path = canonical;
        let mut selected = Vec::new();
        let stored_accounts = stored.accounts;
        for account in &stored_accounts {
            if let Some(source) = profile
                .accounts
                .iter()
                .find(|item| item.account_id == account.source_id)
                .cloned()
            {
                if !claimed_ids.insert(account.app_id) {
                    results.push(outcome(
                        &profile,
                        &account.source_id,
                        source.email.as_deref().unwrap_or_default(),
                        ThunderbirdAccountState::Failed,
                        Some("This Thunderbird account ID conflicts with another configured account.".into()),
                    ));
                    continue;
                }
                let email = source.email.as_deref().unwrap_or_default();
                if !email.trim().is_empty() && !claimed_emails.insert(normalize_email(email)) {
                    results.push(outcome(
                        &profile,
                        &account.source_id,
                        email,
                        ThunderbirdAccountState::Failed,
                        Some("An account with this email address is already connected.".into()),
                    ));
                    continue;
                }
                selected.push(SelectedAccount {
                    account: source,
                    app_id: account.app_id,
                });
            } else {
                results.push(outcome(
                    &profile,
                    &account.source_id,
                    "",
                    ThunderbirdAccountState::Unsupported,
                    Some("This Thunderbird account is no longer available in its profile.".into()),
                ));
            }
        }
        if selected.is_empty() {
            continue;
        }
        let attempted_ids: HashSet<u32> = selected.iter().map(|account| account.app_id).collect();
        match start(
            profile.clone(),
            selected,
            reconnectable_ids.clone(),
            connected_ids.clone(),
            connected_emails.clone(),
        ) {
            Ok((mut ready, mut outcomes)) => {
                for result in &outcomes {
                    if result.state == ThunderbirdAccountState::Connected {
                        if let Some(saved) = stored_accounts
                            .iter()
                            .find(|saved| saved.source_id == result.source_account_id)
                        {
                            connected_ids.insert(saved.app_id);
                        }
                        connected_emails.insert(normalize_email(&result.email));
                    }
                }
                started.append(&mut ready);
                results.append(&mut outcomes);
            }
            Err((state, message)) => {
                for stored_account in stored_accounts
                    .iter()
                    .filter(|account| attempted_ids.contains(&account.app_id))
                {
                    let email = profile
                        .accounts
                        .iter()
                        .find(|account| account.account_id == stored_account.source_id)
                        .and_then(|account| account.email.as_deref())
                        .unwrap_or_default();
                    results.push(outcome(
                        &profile,
                        &stored_account.source_id,
                        email,
                        state,
                        Some(message.clone()),
                    ));
                }
            }
        }
    }
    let connected = started.len();
    let failed = results
        .iter()
        .filter(|result| result.state != ThunderbirdAccountState::Connected)
        .count();
    let notice = (connected > 0).then(|| {
        if failed == 0 {
            format!(
                "Restored {connected} Thunderbird account{}.",
                if connected == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Restored {connected} Thunderbird account{}; {failed} need attention.",
                if connected == 1 { "" } else { "s" }
            )
        }
    });
    Ok((started, results, notice))
}

fn outcome(
    profile: &ThunderbirdProfile,
    source_id: &str,
    email: &str,
    state: ThunderbirdAccountState,
    message: Option<String>,
) -> ThunderbirdAccountOutcome {
    let name = profile
        .accounts
        .iter()
        .find(|account| account.account_id == source_id)
        .map(|account| account.name.clone())
        .unwrap_or_else(|| "Thunderbird account".into());
    ThunderbirdAccountOutcome {
        profile_path: profile.path.clone(),
        source_account_id: source_id.to_owned(),
        name,
        email: email.to_owned(),
        state,
        message,
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        manifest: PathBuf,
    }

    impl Fixture {
        fn new(ids: &[(&str, u32)]) -> Self {
            let root = std::env::temp_dir().join(format!(
                "megamail-account-session-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            ));
            // create_dir, not create_dir_all: refuse a pre-existing fixture root.
            fs::create_dir(&root).unwrap();
            let source = root.join(".thunderbird/Profiles/synthetic");
            fs::create_dir_all(&source).unwrap();
            fs::write(
                root.join(".thunderbird/profiles.ini"),
                "[Profile0]\nName=Synthetic\nIsRelative=1\nPath=Profiles/synthetic\n",
            )
            .unwrap();
            fs::write(
                source.join("prefs.js"),
                include_str!("fixtures/account-session-prefs.js"),
            )
            .unwrap();
            let source = fs::canonicalize(source).unwrap();
            let manifest = root.join("thunderbird-sources.json");
            let store = SourceStore {
                version: 1,
                profiles: if ids.is_empty() {
                    Vec::new()
                } else {
                    vec![StoredProfile {
                        path: source.clone(),
                        accounts: ids
                            .iter()
                            .map(|(source_id, app_id)| StoredAccount {
                                source_id: (*source_id).into(),
                                app_id: *app_id,
                            })
                            .collect(),
                    }]
                },
            };
            fs::write(&manifest, serde_json::to_vec(&store).unwrap()).unwrap();
            Self {
                root,
                source,
                manifest,
            }
        }

        fn load(&self, fail_direct: bool) -> StartupInventory {
            StartupInventory::load_with(
                || {
                    if fail_direct {
                        Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "synthetic direct metadata failure",
                        ))
                    } else {
                        Ok(Vec::new())
                    }
                },
                || read_store_at(&self.manifest),
            )
        }

        fn discover(&self) -> Result<Vec<ThunderbirdProfile>, String> {
            megamail_core::thunderbird::discover_profiles_in(&self.root)
                .map_err(|error| error.to_string())
        }

        fn identity(&self, source_id: &str) -> SavedIdentity {
            SavedIdentity {
                path: self.source.clone(),
                source_id: source_id.into(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fake_start(
        profile: ThunderbirdProfile,
        selected: Vec<SelectedAccount>,
        fail: Option<u32>,
        calls: &mut Vec<u32>,
    ) -> Result<(Vec<u32>, Vec<ThunderbirdAccountOutcome>), (ThunderbirdAccountState, String)> {
        let mut ready = Vec::new();
        let results = selected
            .into_iter()
            .map(|selected| {
                calls.push(selected.app_id);
                let state = if fail == Some(selected.app_id) {
                    ThunderbirdAccountState::Failed
                } else {
                    ready.push(selected.app_id);
                    ThunderbirdAccountState::Connected
                };
                outcome(
                    &profile,
                    &selected.account.account_id,
                    selected.account.email.as_deref().unwrap_or_default(),
                    state,
                    (state != ThunderbirdAccountState::Connected)
                        .then(|| "Synthetic offline failure. Retry.".into()),
                )
            })
            .collect();
        Ok((ready, results))
    }

    #[test]
    fn authorized_failed_selection_is_durable_before_connect_and_next_restart() {
        let fixture = Fixture::new(&[]);
        let mut store = read_store_at(&fixture.manifest).unwrap();
        let profile = fixture.discover().unwrap().remove(0);
        let selected = vec![(
            profile.clone(),
            vec![SelectedAccount {
                account: profile
                    .accounts
                    .iter()
                    .find(|a| a.account_id == "account1")
                    .unwrap()
                    .clone(),
                app_id: 23,
            }],
        )];
        let runtime_result = commit_selection_with(
            &mut store,
            &selected,
            |candidate| {
                config::write_private_file(
                    &fixture.manifest,
                    &serde_json::to_string(candidate).unwrap(),
                )
                .map_err(|e| e.to_string())
            },
            || {
                // The runtime boundary sees the persisted authorization even
                // when authentication fails. No bridge or credential access.
                assert_eq!(
                    read_store_at(&fixture.manifest)
                        .unwrap()
                        .account_id(&fixture.source, "account1"),
                    Some(23)
                );
                Err::<(), _>("synthetic needs login")
            },
        )
        .unwrap();
        assert!(runtime_result.is_err());
        let restart = fixture.load(false);
        assert_eq!(restart.route(), StartupRoute::Mailbox);
        assert_eq!(
            restart
                .thunderbird
                .unwrap()
                .account_id(&fixture.source, "account1"),
            Some(23)
        );
    }

    #[test]
    fn failed_intent_write_never_reaches_runtime_or_changes_roster() {
        let fixture = Fixture::new(&[]);
        let mut store = read_store_at(&fixture.manifest).unwrap();
        let profile = fixture.discover().unwrap().remove(0);
        let selected = vec![(
            profile.clone(),
            vec![SelectedAccount {
                account: profile.accounts[0].clone(),
                app_id: 24,
            }],
        )];
        let before = serde_json::to_vec(&store).unwrap();
        let called = std::cell::Cell::new(false);
        assert!(
            commit_selection_with(
                &mut store,
                &selected,
                |_| Err("synthetic disk failure".into()),
                || called.set(true)
            )
            .is_err()
        );
        assert!(!called.get());
        assert_eq!(serde_json::to_vec(&store).unwrap(), before);
    }

    #[test]
    fn saved_thunderbird_survives_direct_metadata_error_without_import() {
        let fixture = Fixture::new(&[("account1", 7)]);
        let inventory = fixture.load(true);
        assert_eq!(inventory.route(), StartupRoute::StorageRecovery);
        let store = inventory.thunderbird.as_ref().unwrap();
        assert_eq!(store.dormant_accounts()[0].id, 7);
        let mut calls = Vec::new();
        let (ready, outcomes, _) = restore_with(
            store.clone(),
            HashMap::new(),
            || fixture.discover(),
            |p, s, _, _, _| fake_start(p, s, Some(7), &mut calls),
            HashSet::new(),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert!(ready.is_empty());
        assert_eq!(
            calls,
            [7],
            "Direct load failure must not suppress TB restore"
        );
        assert_eq!(outcomes[0].state, ThunderbirdAccountState::Failed);
        assert!(outcomes[0].message.as_ref().unwrap().contains("Retry"));
        calls.clear();
        let (ready, _, _) = restore_with(
            store.clone(),
            HashMap::new(),
            || fixture.discover(),
            |p, s, _, _, _| fake_start(p, s, None, &mut calls),
            HashSet::new(),
            HashSet::from([7]),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(ready, [7]);
        assert_eq!(calls, [7]);
        assert!(
            inventory.direct_error.is_some(),
            "TB recovery cannot clear direct storage error"
        );
        let restarted = fixture.load(true);
        assert_eq!(restarted.route(), StartupRoute::StorageRecovery);
        assert_eq!(restarted.thunderbird.unwrap().dormant_accounts()[0].id, 7);
    }

    #[test]
    fn malformed_manifest_is_storage_recovery_and_is_untouched() {
        let fixture = Fixture::new(&[]);
        let bytes = b"{malformed synthetic manifest";
        fs::write(&fixture.manifest, bytes).unwrap();
        let inventory = fixture.load(false);
        assert_eq!(inventory.route(), StartupRoute::StorageRecovery);
        assert!(inventory.thunderbird.is_err());
        assert_eq!(fs::read(&fixture.manifest).unwrap(), bytes);
    }

    #[test]
    fn genuinely_empty_first_run_never_starts_a_backend() {
        let fixture = Fixture::new(&[]);
        // Missing manifest is also genuinely empty, not unreadable.
        fs::remove_file(&fixture.manifest).unwrap();
        let inventory = fixture.load(false);
        assert_eq!(inventory.route(), StartupRoute::FirstRun);
        let (ready, outcomes, _) = restore_with::<u32>(
            inventory.thunderbird.unwrap(),
            HashMap::new(),
            || panic!("Empty inventory must not discover source profiles"),
            |_, _, _, _, _| panic!("Empty inventory must not start a backend"),
            HashSet::new(),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert!(ready.is_empty() && outcomes.is_empty());
    }

    #[test]
    fn dormant_saved_roster_survives_source_unavailable_without_fabricated_email() {
        let fixture = Fixture::new(&[("account1", 7)]);
        fs::remove_dir_all(&fixture.source).unwrap();
        let inventory = fixture.load(false);
        assert_eq!(inventory.route(), StartupRoute::Mailbox);
        let store = inventory.thunderbird.unwrap();
        let (ready, results, _) = restore_with::<u32>(
            store.clone(),
            HashMap::new(),
            || fixture.discover(),
            |_, _, _, _, _| panic!("Missing source must not start a runtime"),
            HashSet::new(),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert!(ready.is_empty());
        assert_eq!(results[0].state, ThunderbirdAccountState::Failed);
        let rows = store.dormant_accounts();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 7);
        assert_eq!(rows[0].label, "Saved Thunderbird account 7");
        assert!(rows[0].email.is_empty());
    }

    #[test]
    fn partial_healthy_and_failed_retry_skips_qualified_healthy_owner() {
        let fixture = Fixture::new(&[("account1", 7), ("account2", 8)]);
        let store = fixture.load(false).thunderbird.unwrap();
        let mut calls = Vec::new();
        let (ready, results, _) = restore_with(
            store.clone(),
            HashMap::new(),
            || fixture.discover(),
            |p, s, _, _, _| fake_start(p, s, Some(8), &mut calls),
            HashSet::new(),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(ready, [7]);
        assert_eq!(
            results
                .iter()
                .filter(|r| r.state == ThunderbirdAccountState::Failed)
                .count(),
            1
        );
        calls.clear();
        let (ready, results, _) = restore_with(
            store,
            HashMap::from([(7, fixture.identity("account1"))]),
            || fixture.discover(),
            |p, s, _, _, _| fake_start(p, s, None, &mut calls),
            HashSet::from([7]),
            HashSet::from([8]),
            vec!["synthetic@example.test".into()],
        )
        .unwrap();
        assert_eq!(calls, [8]);
        assert_eq!(ready, [8]);
        assert!(
            results
                .iter()
                .all(|r| r.state == ThunderbirdAccountState::Connected)
        );
    }

    #[test]
    fn all_healthy_retry_is_noop_without_source_discovery() {
        let fixture = Fixture::new(&[("account1", 7)]);
        let store = fixture.load(false).thunderbird.unwrap();
        let (ready, results, _) = restore_with::<u32>(
            store,
            HashMap::from([(7, fixture.identity("account1"))]),
            || panic!("Healthy source must not be scanned on sibling Retry"),
            |_, _, _, _, _| panic!("Healthy account must not be restarted"),
            HashSet::from([7]),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert!(ready.is_empty() && results.is_empty());
    }

    #[test]
    fn direct_id_collision_is_rejected_before_discovery_without_renumbering() {
        let fixture = Fixture::new(&[("account1", 7)]);
        let store = fixture.load(false).thunderbird.unwrap();
        let bytes = fs::read(&fixture.manifest).unwrap();
        let (ready, results, _) = restore_with::<u32>(
            store,
            HashMap::new(),
            || panic!("Colliding saved identity must not be scanned"),
            |_, _, _, _, _| panic!("Colliding identity must not start a backend"),
            HashSet::from([7]),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert!(ready.is_empty());
        assert_eq!(results[0].state, ThunderbirdAccountState::Failed);
        assert!(
            results[0]
                .message
                .as_ref()
                .unwrap()
                .contains("identity conflict")
        );
        assert_eq!(fs::read(&fixture.manifest).unwrap(), bytes);
        assert_eq!(
            read_store_at(&fixture.manifest).unwrap().dormant_accounts()[0].id,
            7
        );
    }

    #[test]
    fn different_thunderbird_owner_with_same_id_is_not_healthy_saved_identity() {
        let fixture = Fixture::new(&[("account1", 7)]);
        let store = fixture.load(false).thunderbird.unwrap();
        let (_, results, _) = restore_with::<u32>(
            store,
            HashMap::from([(7, fixture.identity("account2"))]),
            || panic!("Mismatched qualified owner must not discover"),
            |_, _, _, _, _| panic!("Mismatched qualified owner must not start"),
            HashSet::from([7]),
            HashSet::new(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(results[0].state, ThunderbirdAccountState::Failed);
    }
}
