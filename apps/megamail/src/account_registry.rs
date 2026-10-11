//! Cross-adapter ID admission. Native UI serializes direct Add and TB import;
//! durable inventories (including direct tombstones) are re-read off the UI thread.
//! Never repair a collision by renumbering a cache/outbox identity.

use crate::account_session::{SourceStore, read_store, validate_store};
use megamail_core::config::{self, AccountConfig};
use std::collections::HashSet;

pub(super) fn thunderbird_ids(store: &SourceStore) -> HashSet<u32> {
    store
        .profiles
        .iter()
        .flat_map(|profile| profile.accounts.iter().map(|account| account.app_id))
        .collect()
}

pub(super) fn validate_inventories(
    direct: &HashSet<u32>,
    store: &SourceStore,
) -> Result<(), String> {
    validate_store(store).map_err(|error| error.to_string())?;
    if direct.contains(&0)
        || direct
            .intersection(&thunderbird_ids(store))
            .next()
            .is_some()
    {
        return Err("Saved direct and Thunderbird identities conflict. Repair the account inventories; Retry cannot renumber cache or Outbox IDs.".into());
    }
    Ok(())
}

pub(super) fn direct_inventory(store: &SourceStore) -> Result<HashSet<u32>, String> {
    let direct = config::profile_id_inventory()
        .map_err(|error| format!("Could not read direct account IDs: {error}"))?;
    validate_inventories(&direct, store)?;
    Ok(direct)
}

pub(super) fn save_direct(profiles: &[AccountConfig]) -> Result<Vec<(u32, AccountConfig)>, String> {
    let store = read_store()
        .map_err(|error| format!("Could not read saved Thunderbird accounts: {error}"))?;
    direct_inventory(&store)?;
    let reserved = thunderbird_ids(&store);
    // Reserve before credentials/config mutation. Orphaned reservations are safe.
    config::reserve_profile_ids(profiles, &reserved)
        .map_err(|error| format!("Could not reserve the account identity: {error}"))?;
    config::save(profiles).map_err(|error| format!("Could not save the account: {error}"))?;
    config::load_profiles_with_reserved(&reserved).map_err(|error| {
        format!("The account was saved, but its profile could not be reloaded: {error}")
    })
}

/// Both Add and import buttons/actions use this admission rule. An import may
/// not reserve IDs while a direct connection test/save is in flight, or vice versa.
pub(super) fn mutation_blocked(direct_connecting: bool, thunderbird_loading: bool) -> bool {
    direct_connecting || thunderbird_loading
}

pub(super) fn next_id(reserved: &HashSet<u32>) -> Result<u32, String> {
    reserved
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "Account ID space is exhausted; no account was added.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_session::{StoredAccount, StoredProfile};
    fn store(id: u32) -> SourceStore {
        SourceStore {
            version: 1,
            profiles: vec![StoredProfile {
                path: "/synthetic/tb".into(),
                accounts: vec![StoredAccount {
                    source_id: "account1".into(),
                    app_id: id,
                }],
            }],
        }
    }
    #[test]
    fn collision_is_rejected_without_changing_inventory() {
        let saved = store(7);
        let before = serde_json::to_vec(&saved).unwrap();
        assert!(validate_inventories(&HashSet::from([7]), &saved).is_err());
        assert_eq!(before, serde_json::to_vec(&saved).unwrap());
    }
    #[test]
    fn dormant_and_removed_ids_are_reserved() {
        let saved = store(2);
        let mut reserved = HashSet::from([1, 18]);
        validate_inventories(&reserved, &saved).unwrap();
        reserved.extend(thunderbird_ids(&saved));
        assert_eq!(next_id(&reserved).unwrap(), 19);
        assert!(next_id(&HashSet::from([u32::MAX])).is_err());
    }
    #[test]
    fn first_import_and_direct_save_admission_are_mutually_exclusive() {
        assert!(mutation_blocked(false, true));
        assert!(mutation_blocked(true, false));
        assert!(mutation_blocked(true, true));
        assert!(!mutation_blocked(false, false));
    }
}
