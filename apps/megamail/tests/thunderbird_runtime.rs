//! Explicit, read-only provider smoke check. Never sends or changes message flags.
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use megamail_core::thunderbird::discover_profiles;
use megamail_core::thunderbird_bridge::Runtime;
use serde_json::{Value, json};

fn source_stamp(profile: &Path) -> Vec<(PathBuf, u64, Option<SystemTime>)> {
    ["prefs.js", "key4.db", "logins.json", "cert9.db"]
        .into_iter()
        .filter_map(|name| {
            let path = profile.join(name);
            let metadata = std::fs::metadata(&path).ok()?;
            Some((path, metadata.len(), metadata.modified().ok()))
        })
        .collect()
}

fn report_error(stage: &str, _error: &str) {
    // Provider errors can contain private account or message data.
    println!("Thunderbird {stage} operation failed");
}

fn inbox(tree: &Value) -> Option<&str> {
    if tree.get("type").and_then(Value::as_str) == Some("inbox")
        || tree
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case("inbox"))
    {
        return tree.get("id").and_then(Value::as_str);
    }
    tree.get("subFolders")?.as_array()?.iter().find_map(inbox)
}

#[test]
#[ignore = "requires explicit private Thunderbird import authorization and a built MegaMail binary"]
fn thunderbird_read_only_account_probe() {
    assert_eq!(
        std::env::var("MEGAMAIL_TEST_REAL_THUNDERBIRD").as_deref(),
        Ok("1")
    );
    let app = std::env::var_os("MEGAMAIL_TEST_BINARY").expect("Provide a built MegaMail binary");
    // External temporary XDG roots keep this check outside the normal app profile.
    for variable in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"] {
        assert!(
            std::env::var_os(variable).is_some(),
            "Provide isolated XDG directories"
        );
    }
    let mut profiles = discover_profiles().expect("Thunderbird profile discovery failed");
    for profile in &mut profiles {
        profile.accounts.retain(|account| {
            account
                .email
                .as_deref()
                .is_some_and(|email| email.contains('@'))
        });
    }
    profiles.retain(|profile| !profile.accounts.is_empty());
    assert!(
        !profiles.is_empty(),
        "No supported Thunderbird IMAP accounts found"
    );
    let expected: usize = profiles.iter().map(|profile| profile.accounts.len()).sum();
    let mut unread_counts = 0;
    let mut bodies = 0;
    let mut populated = 0;
    let mut verified = 0;
    let mut failures = 0;
    for profile in profiles {
        let before = source_stamp(&profile.path);
        let runtime = Runtime::start_with_executables(
            &profile,
            Path::new(&app),
            Path::new("/usr/bin/thunderbird"),
        );
        if let Err(error) = &runtime {
            report_error("startup", error);
        }
        if let Ok(runtime) = runtime {
            let accounts = runtime.call("accounts", json!({}));
            if let Err(error) = &accounts {
                report_error("accounts", error);
            }
            if let Ok(Value::Array(accounts)) = accounts {
                for account in &profile.accounts {
                    if !accounts.iter().any(|remote| {
                        remote.get("id").and_then(Value::as_str)
                            == Some(account.account_id.as_str())
                    }) {
                        failures += 1;
                        continue;
                    }
                    let folders = runtime.call("folders", json!({"accountId": account.account_id}));
                    if let Err(error) = &folders {
                        report_error("folders", error);
                    }
                    if let Ok(folders) = folders {
                        if let Some(folder_id) = inbox(&folders) {
                            let page =
                                runtime.call("list", json!({"folderId": folder_id, "limit": 100}));
                            if let Err(error) = &page {
                                report_error("headers", error);
                            }
                            if page
                                .as_ref()
                                .ok()
                                .and_then(|page| page.get("messages"))
                                .and_then(Value::as_array)
                                .is_some_and(|rows| rows.len() <= 100)
                            {
                                if let Err(error) = &page {
                                    report_error("headers", error);
                                }
                                if page
                                    .as_ref()
                                    .ok()
                                    .and_then(|page| page.get("messages"))
                                    .and_then(Value::as_array)
                                    .is_some_and(|rows| {
                                        rows.iter().any(|row| {
                                            row.get("author")
                                                .and_then(Value::as_str)
                                                .is_some_and(|author| !author.is_empty())
                                        })
                                    })
                                {
                                    populated += 1;
                                }
                                if let Some(message_id) = page
                                    .as_ref()
                                    .ok()
                                    .and_then(|page| page.get("messages"))
                                    .and_then(Value::as_array)
                                    .and_then(|rows| {
                                        rows.iter().find(|row| {
                                            row.get("size")
                                                .and_then(Value::as_u64)
                                                .is_some_and(|size| size <= 1024 * 1024)
                                        })
                                    })
                                    .and_then(|row| row.get("id"))
                                    .and_then(Value::as_str)
                                {
                                    match runtime.call("body", json!({"messageId": message_id})) {
                                        Ok(body) if body.get("plainText").is_some() => bodies += 1,
                                        Ok(_) => report_error("body", "Malformed body response"),
                                        Err(error) => report_error("body", &error),
                                    }
                                }
                                if page
                                    .as_ref()
                                    .ok()
                                    .and_then(|page| page.get("unreadMessageCount"))
                                    .and_then(Value::as_u64)
                                    .is_some_and(|count| count <= u32::MAX as u64)
                                {
                                    unread_counts += 1;
                                }
                                verified += 1;
                                continue;
                            }
                        }
                    }
                    failures += 1;
                }
            } else {
                failures += profile.accounts.len();
            }
            drop(runtime);
        } else {
            failures += profile.accounts.len();
        }
        assert_eq!(
            source_stamp(&profile.path),
            before,
            "Source Thunderbird files changed during probe"
        );
    }
    // Counts only: account addresses, message metadata and credentials stay private.
    println!(
        "Thunderbird accounts verified: {verified}/{expected}; inboxes with parsed authors: {populated}; bounded bodies read: {bodies}; folders with unread metadata: {unread_counts}; failures: {failures}"
    );
    assert_eq!(
        unread_counts, expected,
        "Missing synchronized folder unread metadata"
    );
    assert_eq!(
        verified, expected,
        "Not every account completed authenticated folder/header access"
    );
}
