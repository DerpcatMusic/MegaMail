//! Native add-only authorization. Secret-bearing transactions stay on the worker.
use megamail_core::{
    config::{self, AccountConfig},
    provider_auth::{self, AccountStore, AuthController, AuthError, Provider, Registration},
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// UI admission excludes TB import while setup is connecting. This additionally
// serializes checked direct roster load/reservation/save across password/OAuth.
pub(crate) static DIRECT_WRITE: Mutex<()> = Mutex::new(());

pub(crate) struct Availability {
    pub google: Result<(), AuthError>,
    pub microsoft: Result<(), AuthError>,
}
pub(crate) fn availability() -> Availability {
    Availability {
        google: Registration::configured(Provider::GoogleImapSmtp).map(|_| ()),
        microsoft: Registration::configured(Provider::MicrosoftGraph(
            provider_auth::MicrosoftAudience::PersonalAndWork,
        ))
        .map(|_| ()),
    }
}

pub(crate) struct Added {
    pub profiles: Vec<(u32, AccountConfig)>,
    pub added: (u32, AccountConfig),
}

struct AdmissionRoster {
    direct: Vec<AccountConfig>,
    known_emails: Vec<String>,
}

/// Match durable external IDs to verified live metadata. The v1 source store
/// has no addresses; an absent/blank row must not become an omitted duplicate.
fn verified_thunderbird_emails(
    store: &crate::account_session::SourceStore,
    live: &[megamail_core::models::Account],
) -> Result<Vec<String>, String> {
    store.profiles.iter().flat_map(|profile| &profile.accounts).map(|stored| {
        live.iter()
            .find(|account| account.id == stored.app_id && !account.email.trim().is_empty())
            .map(|account| account.email.clone())
            .ok_or_else(|| "A saved Thunderbird mailbox has no verified address yet. Retry it before adding an account so duplicate mailbox admission can be checked.".to_owned())
    }).collect()
}

/// Caller holds DIRECT_WRITE. Reload without allocating IDs before admission.
fn load_admission(live: &[megamail_core::models::Account]) -> Result<AdmissionRoster, String> {
    let tb = crate::account_session::read_store().map_err(|error| error.to_string())?;
    crate::account_registry::direct_inventory(&tb)?;
    let mut known_emails = verified_thunderbird_emails(&tb, live)?;
    known_emails.extend(live.iter().map(|account| account.email.clone()));
    let direct = config::load_profiles_existing()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(_, profile)| profile)
        .collect();
    Ok(AdmissionRoster {
        direct,
        known_emails,
    })
}

/// One normalized policy for UI prechecks, password admission and OAuth commit.
pub(crate) fn check_new_account<'a>(
    email: &str,
    direct: impl Iterator<Item = &'a str>,
    known: impl Iterator<Item = &'a str>,
) -> Result<(), AuthError> {
    let emails = direct.chain(known).collect::<Vec<_>>();
    if emails.iter().any(|email| email.trim().is_empty()) {
        return Err(AuthError::Storage);
    }
    if duplicate(email, emails.into_iter()) {
        return Err(AuthError::AccountAlreadyExists);
    }
    Ok(())
}

/// Actual password consumer. No probe or reserve/save can precede admission.
fn add_password_with(
    account: AccountConfig,
    load: impl FnOnce() -> Result<AdmissionRoster, String>,
    probe: impl FnOnce(&AccountConfig) -> Result<(), String>,
    save: impl FnOnce(&[AccountConfig]) -> Result<Vec<(u32, AccountConfig)>, String>,
) -> Result<Added, String> {
    let AdmissionRoster {
        mut direct,
        known_emails,
    } = load()?;
    check_new_account(
        &account.email,
        direct.iter().map(|profile| profile.email.as_str()),
        known_emails.iter().map(String::as_str),
    )
    .map_err(|error| error.to_string())?;
    probe(&account)?;
    let email = account.email.clone();
    direct.push(account);
    let profiles = save(&direct)?;
    let added = profiles
        .iter()
        .find(|(_, profile)| duplicate(&email, std::iter::once(profile.email.as_str())))
        .cloned()
        .ok_or_else(|| "The saved account was not found after reload.".to_owned())?;
    Ok(Added { profiles, added })
}

pub(crate) fn add_password(
    account: AccountConfig,
    live: Vec<megamail_core::models::Account>,
) -> Result<Added, String> {
    let _guard = DIRECT_WRITE
        .lock()
        .map_err(|_| "Account admission is unavailable.".to_owned())?;
    add_password_with(
        account,
        || load_admission(&live),
        |account| {
            let outcome = megamail_core::worker::test_connection_blocking(account.clone());
            outcome.incoming.map_err(|error| {
                format!("Could not connect to the incoming mail server: {error}")
            })?;
            outcome
                .smtp
                .map_err(|error| format!("Could not connect to the outgoing mail server: {error}"))
        },
        crate::account_registry::save_direct,
    )
}

/// Save accounts converted from Thunderbird in one roster write. Addresses
/// already configured (directly or live) are skipped, not overwritten.
pub(crate) fn add_migrated(
    accounts: Vec<AccountConfig>,
    live: &[megamail_core::models::Account],
) -> Result<Vec<(u32, AccountConfig)>, String> {
    let _guard = DIRECT_WRITE
        .lock()
        .map_err(|_| "Account admission is unavailable.".to_owned())?;
    let mut direct: Vec<AccountConfig> = config::load_profiles_existing()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(_, profile)| profile)
        .collect();
    let before = direct.len();
    for account in accounts {
        let known = direct
            .iter()
            .map(|p| p.email.as_str())
            .chain(live.iter().map(|a| a.email.as_str()));
        if !duplicate(&account.email, known) {
            direct.push(account);
        }
    }
    if direct.len() == before {
        return Ok(Vec::new());
    }
    crate::account_registry::save_direct(&direct)
}

/// Concrete AccountStore adapter: reserve native stable IDs BEFORE credentials
/// are saved. Never use ConfigAccountStore's plain save followed by allocation.
struct RegistryStore {
    known_emails: Vec<String>,
    known_accounts: Vec<megamail_core::models::Account>,
    saved: Option<Vec<(u32, AccountConfig)>>,
    reload_failed: bool,
}
impl AccountStore for RegistryStore {
    fn save_new(&mut self, account: &AccountConfig) -> Result<(), AuthError> {
        let _guard = DIRECT_WRITE.lock().map_err(|_| AuthError::Storage)?;
        let roster = load_admission(&self.known_accounts).map_err(|_| AuthError::Storage)?;
        self.known_emails = roster.known_emails;
        self.save_checked(account, roster.direct, crate::account_registry::save_direct)
    }
}
impl RegistryStore {
    fn save_checked(
        &mut self,
        account: &AccountConfig,
        mut roster: Vec<AccountConfig>,
        save: impl FnOnce(&[AccountConfig]) -> Result<Vec<(u32, AccountConfig)>, String>,
    ) -> Result<(), AuthError> {
        check_new_account(
            &account.email,
            roster.iter().map(|p| p.email.as_str()),
            self.known_emails.iter().map(String::as_str),
        )?;
        roster.push(account.clone());
        match save(&roster) {
            Ok(saved) => {
                self.saved = Some(saved);
                Ok(())
            }
            Err(error) => {
                self.reload_failed = error
                    .starts_with("The account was saved, but its profile could not be reloaded:");
                Err(AuthError::Storage)
            }
        }
    }
}
fn duplicate<'a>(email: &str, emails: impl Iterator<Item = &'a str>) -> bool {
    emails
        .into_iter()
        .any(|existing| existing.trim().eq_ignore_ascii_case(email.trim()))
}

/// Begin and authorize on the blocking executor. The callback gives the UI only
/// a cancellation handle; ReadyAccount is never returned across the UI seam.
pub(crate) fn authorize(
    controller: AuthController,
    provider: Provider,
    email: String,
    known_accounts: Vec<megamail_core::models::Account>,
    cancellation: Arc<Mutex<Option<provider_auth::Attempt>>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) -> Result<Added, String> {
    // Preflight before browser/network/probe work; commit rechecks a fresh roster.
    {
        let _guard = DIRECT_WRITE
            .lock()
            .map_err(|_| "Account admission is unavailable.".to_owned())?;
        let roster = load_admission(&known_accounts)?;
        check_new_account(
            &email,
            roster.direct.iter().map(|p| p.email.as_str()),
            roster.known_emails.iter().map(String::as_str),
        )
        .map_err(|error| error.to_string())?;
    }
    let attempt = controller.begin(Duration::from_secs(300));
    if let Ok(mut slot) = cancellation.lock() {
        *slot = Some(attempt.clone());
    }
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        attempt.cancel();
    }
    let mut store = RegistryStore {
        known_emails: Vec::new(),
        known_accounts,
        saved: None,
        reload_failed: false,
    };
    let result = (|| {
        let transaction =
            megamail_core::onboarding::provider_authorization(provider, &email, attempt)?;
        let ready = transaction.authorize(
            &mut provider_auth::ProviderHttp,
            &mut provider_auth::SystemBrowser,
            &mut provider_auth::ProviderProbe,
        )?;
        let saved = ready.commit(&mut store).map_err(|failure| failure.error)?;
        let profiles = store.saved.take().ok_or(AuthError::Storage)?;
        let added = profiles
            .iter()
            .find(|(_, p)| duplicate(&saved.account.email, std::iter::once(p.email.as_str())))
            .cloned()
            .ok_or(AuthError::Storage)?;
        Ok::<_, AuthError>(Added { profiles, added })
    })();
    if let Ok(mut slot) = cancellation.lock() {
        *slot = None;
    }
    result.map_err(|e| if store.reload_failed { "The account was saved, but its profile could not be reloaded: repair storage and restart before adding another account.".into() } else { e.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn saved_tb() -> crate::account_session::SourceStore {
        crate::account_session::SourceStore {
            version: 1,
            profiles: vec![crate::account_session::StoredProfile {
                path: "/synthetic/connected-tb".into(),
                accounts: vec![crate::account_session::StoredAccount {
                    source_id: "account7".into(),
                    app_id: 7,
                }],
            }],
        }
    }
    fn connected_tb(email: &str) -> megamail_core::models::Account {
        megamail_core::models::Account {
            id: 7,
            name: "Synthetic connected Thunderbird".into(),
            email: email.into(),
            label: "Synthetic".into(),
            accent: "#000000".into(),
        }
    }
    fn direct(email: &str) -> AccountConfig {
        AccountConfig::new(email)
    }

    #[test]
    fn actual_password_admission_rejects_mixed_tb_duplicate_before_probe_reserve_or_save() {
        let tb = saved_tb();
        let before = serde_json::to_vec(&tb).unwrap();
        let probes = Cell::new(0);
        let reservations = Cell::new(0);
        let saves = Cell::new(0);
        let live = vec![connected_tb(" Tb@Example.TEST ")];
        let result = add_password_with(
            direct(" tb@example.test "),
            || {
                Ok(AdmissionRoster {
                    direct: vec![direct("old@example.test")],
                    known_emails: verified_thunderbird_emails(&tb, &live)?,
                })
            },
            |_| {
                probes.set(probes.get() + 1);
                Ok(())
            },
            |_| {
                reservations.set(reservations.get() + 1);
                saves.set(saves.get() + 1);
                panic!("duplicate reached reserve/save")
            },
        );
        assert!(
            matches!(result, Err(error) if error == AuthError::AccountAlreadyExists.to_string())
        );
        assert_eq!((probes.get(), reservations.get(), saves.get()), (0, 0, 0));
        assert_eq!(serde_json::to_vec(&tb).unwrap(), before);
        assert_eq!(live[0].id, 7);
        assert_eq!(live[0].email, " Tb@Example.TEST ");
    }

    #[test]
    fn actual_password_admission_accepts_distinct_mailbox_and_preserves_existing_ids() {
        let tb = saved_tb();
        let before = serde_json::to_vec(&tb).unwrap();
        let probes = Cell::new(0);
        let reservations = Cell::new(0);
        let saves = Cell::new(0);
        let result = add_password_with(
            direct("new@example.test"),
            || {
                Ok(AdmissionRoster {
                    direct: vec![direct("old@example.test")],
                    known_emails: verified_thunderbird_emails(
                        &tb,
                        &[connected_tb("tb@example.test")],
                    )?,
                })
            },
            |account| {
                assert_eq!(account.email, "new@example.test");
                probes.set(probes.get() + 1);
                Ok(())
            },
            |roster| {
                assert_eq!(probes.get(), 1);
                assert_eq!(roster[0].email, "old@example.test");
                assert_eq!(roster[1].email, "new@example.test");
                reservations.set(reservations.get() + 1);
                saves.set(saves.get() + 1);
                Ok(vec![(4, roster[0].clone()), (17, roster[1].clone())])
            },
        )
        .unwrap();
        assert_eq!((probes.get(), reservations.get(), saves.get()), (1, 1, 1));
        assert_eq!(
            result
                .profiles
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            vec![4, 17]
        );
        assert_eq!(result.added.0, 17);
        assert_eq!(serde_json::to_vec(&tb).unwrap(), before);
    }

    #[test]
    fn actual_password_admission_blocks_unknown_tb_address_or_new_unverified_saved_id() {
        for live in [
            vec![],
            vec![connected_tb("   ")],
            vec![megamail_core::models::Account {
                id: 8,
                ..connected_tb("tb@example.test")
            }],
        ] {
            let result = add_password_with(
                direct("new@example.test"),
                || {
                    Ok(AdmissionRoster {
                        direct: vec![],
                        known_emails: verified_thunderbird_emails(&saved_tb(), &live)?,
                    })
                },
                |_| panic!("unknown TB address reached probe"),
                |_| panic!("unknown TB address reached reserve/save"),
            );
            assert!(matches!(result, Err(error) if error.contains("no verified address")));
        }
    }

    #[test]
    fn actual_password_admission_rejects_reloaded_direct_duplicate_before_probe() {
        let result = add_password_with(
            direct(" A@example.test "),
            || {
                Ok(AdmissionRoster {
                    direct: vec![direct("a@EXAMPLE.TEST")],
                    known_emails: vec![],
                })
            },
            |_| panic!("direct duplicate reached probe"),
            |_| panic!("direct duplicate reached save"),
        );
        assert!(
            matches!(result, Err(error) if error == AuthError::AccountAlreadyExists.to_string())
        );
    }

    #[test]
    fn actual_password_admission_does_not_persist_a_failed_probe() {
        let result = add_password_with(
            direct("new@example.test"),
            || {
                Ok(AdmissionRoster {
                    direct: vec![],
                    known_emails: vec!["tb@example.test".into()],
                })
            },
            |_| Err("synthetic TLS refusal".into()),
            |_| panic!("failed probe reached reserve/save"),
        );
        assert!(matches!(result, Err(error) if error == "synthetic TLS refusal"));
    }

    #[test]
    fn shared_oauth_admission_policy_fails_closed_on_unknown_address() {
        let mut store = RegistryStore {
            known_emails: vec![" ".into()],
            known_accounts: vec![],
            saved: None,
            reload_failed: false,
        };
        assert_eq!(
            store.save_checked(&direct("new@example.test"), vec![], |_| panic!(
                "unknown address reached OAuth save"
            )),
            Err(AuthError::Storage)
        );
        assert!(store.saved.is_none());
    }

    #[test]
    fn actual_registry_adapter_saves_checked_roster_without_losing_existing_ids() {
        use super::*;
        let mut store = RegistryStore {
            known_emails: vec!["tb@example.test".into()],
            known_accounts: Vec::new(),
            saved: None,
            reload_failed: false,
        };
        let existing = AccountConfig::new("old@example.test");
        let account = AccountConfig {
            email: "new@example.test".into(),
            oauth_refresh: "fictional-pending-refresh".into(),
            ..AccountConfig::new("new@example.test")
        };
        store
            .save_checked(&account, vec![existing], |roster| {
                assert_eq!(roster.len(), 2);
                assert_eq!(roster[1].oauth_refresh, "fictional-pending-refresh");
                Ok(vec![(4, roster[0].clone()), (17, roster[1].clone())])
            })
            .unwrap();
        assert_eq!(store.saved.as_ref().unwrap()[1].0, 17);
    }
    #[test]
    fn actual_registry_adapter_never_calls_save_for_direct_or_thunderbird_duplicate() {
        use super::*;
        let mut store = RegistryStore {
            known_emails: vec!["tb@example.test".into()],
            known_accounts: Vec::new(),
            saved: None,
            reload_failed: false,
        };
        let account = AccountConfig::new("TB@EXAMPLE.TEST");
        assert_eq!(
            store.save_checked(&account, vec![], |_| panic!(
                "duplicate reached persistence"
            )),
            Err(AuthError::AccountAlreadyExists)
        );
        assert!(store.saved.is_none());
    }
    #[test]
    fn post_save_reload_failure_is_not_reported_as_prior_configuration_retained() {
        use super::*;
        let mut store = RegistryStore {
            known_emails: vec![],
            known_accounts: Vec::new(),
            saved: None,
            reload_failed: false,
        };
        let account = AccountConfig::new("new@example.test");
        assert_eq!(
            store.save_checked(&account, vec![], |_| Err(
                "The account was saved, but its profile could not be reloaded: synthetic".into()
            )),
            Err(AuthError::Storage)
        );
        assert!(store.reload_failed);
    }
    #[test]
    fn native_checked_add_rejects_case_and_whitespace_duplicates() {
        assert!(super::duplicate(
            " A@Example.test ",
            ["a@example.TEST"].into_iter()
        ));
        assert!(!super::duplicate(
            "a@example.test",
            ["b@example.test"].into_iter()
        ));
    }
}
