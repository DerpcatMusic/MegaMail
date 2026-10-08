//! Read-only discovery of Thunderbird IMAP account settings.
//!
//! The explicit import flow uses this module to read `profiles.ini` and
//! `prefs.js`. The separate Thunderbird bridge copies the encrypted login
//! store into MegaMail's private runtime profile; it never changes the source
//! profile or decodes credentials in this scanner.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_PROFILES_INI_BYTES: u64 = 64 * 1024;
const MAX_PREFS_BYTES: u64 = 1024 * 1024;
const MAX_PROFILES: usize = 32;
const MAX_ACCOUNTS: usize = 32;
const MAX_IDENTITIES: usize = 64;
const MAX_PREF_LINES: usize = 50_000;
const MAX_PREF_KEY_BYTES: usize = 1024;
const MAX_PREF_STRING_BYTES: usize = 16 * 1024;

/// A Thunderbird profile found in one of the standard Linux profile roots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThunderbirdProfile {
    pub name: String,
    pub path: PathBuf,
    pub accounts: Vec<ThunderbirdAccount>,
}

/// The non-secret settings for one Thunderbird IMAP account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThunderbirdAccount {
    /// Thunderbird's account key (for example `account3`), stable within the
    /// source profile and needed to select this account in the isolated bridge.
    pub account_id: String,
    pub name: String,
    pub email: Option<String>,
    pub username: String,
    pub incoming: ThunderbirdServerSettings,
    /// `None` means Thunderbird has no configured SMTP host for this account.
    pub outgoing: Option<ThunderbirdServerSettings>,
    /// All configured send-as identities. The primary identity is included.
    pub identities: Vec<ThunderbirdIdentity>,
}

/// An IMAP/SMTP endpoint copied from a Thunderbird profile, without secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThunderbirdServerSettings {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub security: ThunderbirdSecurity,
    pub authentication: ThunderbirdAuthentication,
}

/// Thunderbird's numeric socket mode translated to a displayable value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThunderbirdSecurity {
    Plaintext,
    StartTls,
    ImplicitTls,
    /// A legacy mode present in old profiles; it should be reviewed manually.
    LegacyTryStartTls,
    Unsupported(i64),
}

/// Thunderbird's numeric authentication mode translated for account review.
/// The runtime bridge lets Thunderbird itself authenticate from a private
/// clone of its encrypted login store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThunderbirdAuthentication {
    None,
    Password,
    OAuth2,
    Unsupported(i64),
}

/// A Thunderbird send-as identity, optionally with its own SMTP endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThunderbirdIdentity {
    pub email: Option<String>,
    pub display_name: String,
    pub username: String,
    pub outgoing: Option<ThunderbirdServerSettings>,
}

/// Find Thunderbird profiles under `~/.thunderbird` and the official Flatpak
/// profile root. Call this only after the user explicitly chooses import.
pub fn discover_profiles() -> io::Result<Vec<ThunderbirdProfile>> {
    let home = dirs::home_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not locate the home directory",
        )
    })?;
    discover_profiles_in(&home)
}

/// Find settings in the standard Linux and Flatpak profile locations below a
/// supplied home directory. This is public so the UI and tests can choose a
/// stable root without changing process environment variables.
pub fn discover_profiles_in(home: &Path) -> io::Result<Vec<ThunderbirdProfile>> {
    let roots = [
        home.join(".thunderbird"),
        home.join(".var/app/org.mozilla.Thunderbird/.thunderbird"),
    ];
    let mut profiles = Vec::new();
    let mut seen_paths = HashSet::new();
    let mut profile_count = 0;

    for root in roots {
        let ini_path = root.join("profiles.ini");
        match fs::symlink_metadata(&ini_path) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }

        let ini = read_bounded(&ini_path, MAX_PROFILES_INI_BYTES)?;
        let canonical_root = fs::canonicalize(&root)?;
        for profile in parse_profiles_ini(&ini) {
            if profile_count >= MAX_PROFILES {
                return Err(invalid_data("too many Thunderbird profiles"));
            }
            profile_count += 1;
            let path = match resolve_profile_path(&canonical_root, &profile.path, profile.relative)
            {
                Ok(path) => path,
                Err(_) => continue,
            };
            if !seen_paths.insert(path.clone()) {
                continue;
            }

            let prefs_path = path.join("prefs.js");
            match fs::symlink_metadata(&prefs_path) {
                Ok(metadata) if metadata.file_type().is_file() => {}
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            }
            let prefs = read_bounded(&prefs_path, MAX_PREFS_BYTES)?;
            let preferences = parse_preferences(&prefs);
            let accounts = import_imap_accounts(&preferences);
            if !accounts.is_empty() {
                profiles.push(ThunderbirdProfile {
                    name: profile.name,
                    path,
                    accounts,
                });
            }
        }
    }
    Ok(profiles)
}

struct ProfileEntry {
    name: String,
    path: PathBuf,
    relative: bool,
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_bounded(path: &Path, limit: u64) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid_data(
            "Thunderbird settings file exceeds the size limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid_data("Thunderbird settings are not UTF-8"))
}

fn parse_profiles_ini(input: &str) -> Vec<ProfileEntry> {
    let mut sections: Vec<(String, HashMap<String, String>)> = Vec::new();
    let mut current: Option<usize> = None;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            sections.push((line[1..line.len() - 1].to_string(), HashMap::new()));
            current = Some(sections.len() - 1);
        } else if let (Some(index), Some((key, value))) = (current, line.split_once('=')) {
            sections[index]
                .1
                .insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    sections
        .into_iter()
        .filter_map(|(section, values)| {
            if section.len() > 128 || !section.to_ascii_lowercase().starts_with("profile") {
                return None;
            }
            let raw_path = values.get("path")?;
            if raw_path.len() > 4096 {
                return None;
            }
            let relative = values
                .get("isrelative")
                .map(|value| value != "0")
                .unwrap_or(true);
            let name = values
                .get("name")
                .filter(|name| {
                    !name.trim().is_empty()
                        && name.len() <= 128
                        && !name.chars().any(char::is_control)
                })
                .cloned()
                .unwrap_or_else(|| section.clone());
            Some(ProfileEntry {
                name,
                path: PathBuf::from(raw_path),
                relative,
            })
        })
        .collect()
}

fn resolve_profile_path(root: &Path, raw_path: &Path, relative: bool) -> io::Result<PathBuf> {
    let path = if relative {
        if raw_path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(invalid_data(
                "relative Thunderbird profile path escapes its root",
            ));
        }
        root.join(raw_path)
    } else {
        if !raw_path.is_absolute() {
            return Err(invalid_data(
                "absolute Thunderbird profile path is not absolute",
            ));
        }
        raw_path.to_path_buf()
    };
    let path = fs::canonicalize(path)?;
    if relative && !path.starts_with(root) {
        return Err(invalid_data(
            "relative Thunderbird profile path escapes its root",
        ));
    }
    if path.is_dir() {
        Ok(path)
    } else {
        Err(invalid_data("Thunderbird profile path is not a directory"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PrefValue {
    String(String),
    Number(i64),
    Bool(bool),
}

fn parse_preferences(input: &str) -> HashMap<String, PrefValue> {
    let mut preferences = HashMap::new();
    let mut in_block_comment = false;
    for line in input.lines().take(MAX_PREF_LINES) {
        let trimmed = line.trim_start();
        if in_block_comment {
            if let Some(end) = trimmed.find("*/") {
                in_block_comment = false;
                if let Some((key, value)) = parse_pref_statement(&trimmed[end + 2..]) {
                    preferences.insert(key, value);
                }
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            in_block_comment = !trimmed.contains("*/");
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some((key, value)) = parse_pref_statement(trimmed) {
            preferences.insert(key, value);
        }
    }
    preferences
}

fn parse_pref_statement(input: &str) -> Option<(String, PrefValue)> {
    let mut parser = PrefParser { input, index: 0 };
    parser.skip_whitespace();
    parser.expect(b"user_pref")?;
    parser.skip_whitespace();
    parser.expect_byte(b'(')?;
    parser.skip_whitespace();
    let key = parser.parse_string(MAX_PREF_KEY_BYTES)?;
    parser.skip_whitespace();
    parser.expect_byte(b',')?;
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    parser.expect_byte(b')')?;
    parser.skip_whitespace();
    parser.expect_byte(b';')?;
    parser.skip_whitespace();
    (parser.index == input.len()).then_some((key, value))
}

struct PrefParser<'a> {
    input: &'a str,
    index: usize,
}

impl PrefParser<'_> {
    fn skip_whitespace(&mut self) {
        while self
            .input
            .as_bytes()
            .get(self.index)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.index += 1;
        }
    }

    fn expect(&mut self, expected: &[u8]) -> Option<()> {
        self.input
            .as_bytes()
            .get(self.index..)?
            .starts_with(expected)
            .then(|| {
                self.index += expected.len();
            })
    }

    fn expect_byte(&mut self, expected: u8) -> Option<()> {
        (self.input.as_bytes().get(self.index).copied()? == expected).then(|| {
            self.index += 1;
        })
    }

    fn parse_value(&mut self) -> Option<PrefValue> {
        let bytes = self.input.as_bytes();
        match bytes.get(self.index).copied()? {
            b'"' => self
                .parse_string(MAX_PREF_STRING_BYTES)
                .map(PrefValue::String),
            b't' if self.input[self.index..].starts_with("true") => {
                self.index += 4;
                Some(PrefValue::Bool(true))
            }
            b'f' if self.input[self.index..].starts_with("false") => {
                self.index += 5;
                Some(PrefValue::Bool(false))
            }
            b'-' | b'0'..=b'9' => {
                let start = self.index;
                if bytes[self.index] == b'-' {
                    self.index += 1;
                }
                let digits = self.index;
                while bytes.get(self.index).is_some_and(u8::is_ascii_digit) {
                    self.index += 1;
                }
                if self.index == digits {
                    return None;
                }
                self.input[start..self.index]
                    .parse()
                    .ok()
                    .map(PrefValue::Number)
            }
            _ => None,
        }
    }

    fn parse_string(&mut self, max_bytes: usize) -> Option<String> {
        self.expect_byte(b'"')?;
        let mut result = String::new();
        loop {
            let byte = *self.input.as_bytes().get(self.index)?;
            match byte {
                b'"' => {
                    self.index += 1;
                    return (result.len() <= max_bytes).then_some(result);
                }
                b'\\' => {
                    self.index += 1;
                    let escape = *self.input.as_bytes().get(self.index)?;
                    self.index += 1;
                    match escape {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'/' => result.push('/'),
                        b'b' => result.push('\u{0008}'),
                        b'f' => result.push('\u{000c}'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        b'x' => {
                            let scalar = self.parse_hex(2)?;
                            result.push(char::from_u32(scalar)?);
                        }
                        b'u' => {
                            let first = self.parse_hex(4)?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if self.input.as_bytes().get(self.index..)?.starts_with(b"\\u") {
                                    self.index += 2;
                                } else {
                                    return None;
                                }
                                let second = self.parse_hex(4)?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return None;
                                }
                                0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return None;
                            } else {
                                first
                            };
                            result.push(char::from_u32(scalar)?);
                        }
                        _ => return None,
                    }
                }
                0x00..=0x1f => return None,
                _ => {
                    let value = self.input.get(self.index..)?.chars().next()?;
                    result.push(value);
                    self.index += value.len_utf8();
                }
            }
            if result.len() > max_bytes {
                return None;
            }
        }
    }

    fn parse_hex(&mut self, digits: usize) -> Option<u32> {
        let end = self.index.checked_add(digits)?;
        let value = u32::from_str_radix(self.input.get(self.index..end)?, 16).ok()?;
        self.index = end;
        Some(value)
    }
}

fn pref_string<'a>(prefs: &'a HashMap<String, PrefValue>, key: &str) -> Option<&'a str> {
    match prefs.get(key)? {
        PrefValue::String(value) => Some(value),
        _ => None,
    }
}

fn pref_number(prefs: &HashMap<String, PrefValue>, key: &str) -> Option<i64> {
    match prefs.get(key)? {
        PrefValue::Number(value) => Some(*value),
        _ => None,
    }
}

fn list_pref(prefs: &HashMap<String, PrefValue>, key: &str, max: usize) -> Vec<String> {
    pref_string(prefs, key)
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|item| valid_pref_id(item))
        .take(max)
        .map(str::to_string)
        .collect()
}

fn valid_pref_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn import_imap_accounts(prefs: &HashMap<String, PrefValue>) -> Vec<ThunderbirdAccount> {
    let account_ids = list_pref(prefs, "mail.accountmanager.accounts", MAX_ACCOUNTS);
    let default_smtp = pref_string(prefs, "mail.smtp.defaultserver").filter(|id| valid_pref_id(id));
    let mut accounts = Vec::new();

    for account_id in account_ids {
        let Some(server_id) = pref_string(prefs, &format!("mail.account.{account_id}.server"))
            .filter(|id| valid_pref_id(id))
        else {
            continue;
        };
        if pref_string(prefs, &format!("mail.server.{server_id}.type")) != Some("imap") {
            continue;
        }
        let Some(incoming_host) = checked_host(pref_string(
            prefs,
            &format!("mail.server.{server_id}.hostname"),
        )) else {
            continue;
        };
        let incoming_security = security(
            pref_number(prefs, &format!("mail.server.{server_id}.socketType")).unwrap_or(-1),
        );
        let Some(incoming_port) = port(
            pref_number(prefs, &format!("mail.server.{server_id}.port")),
            match incoming_security {
                ThunderbirdSecurity::ImplicitTls => 993,
                _ => 143,
            },
        )
        .ok() else {
            continue;
        };
        let incoming = ThunderbirdServerSettings {
            host: incoming_host,
            port: incoming_port,
            username: pref_string(prefs, &format!("mail.server.{server_id}.userName"))
                .map(|value| safe_pref_text(value, 320))
                .unwrap_or_default(),
            security: incoming_security,
            authentication: authentication(
                pref_number(prefs, &format!("mail.server.{server_id}.authMethod")).unwrap_or(-1),
            ),
        };

        let identity_ids = list_pref(
            prefs,
            &format!("mail.account.{account_id}.identities"),
            MAX_IDENTITIES,
        );
        let mut identities = Vec::new();
        for identity_id in identity_ids {
            let smtp_id = pref_string(prefs, &format!("mail.identity.{identity_id}.smtpServer"))
                .filter(|id| valid_pref_id(id))
                .or(default_smtp);
            let identity = import_identity(prefs, &identity_id, smtp_id);
            identities.push(identity);
        }

        let primary = identities.iter().find(|identity| identity.email.is_some());
        let email = primary.and_then(|identity| identity.email.clone());
        let name = primary
            .map(|identity| identity.display_name.clone())
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                pref_string(prefs, &format!("mail.server.{server_id}.name"))
                    .map(|value| safe_pref_text(value, 256))
                    .filter(|value| !value.is_empty())
            })
            .or_else(|| email.clone())
            .unwrap_or_else(|| "IMAP account".to_string());
        let username = incoming.username.clone();
        let outgoing = primary
            .and_then(|identity| identity.outgoing.clone())
            .or_else(|| default_smtp.and_then(|smtp_id| import_smtp(prefs, smtp_id)));

        accounts.push(ThunderbirdAccount {
            account_id,
            name,
            email,
            username,
            incoming,
            outgoing,
            identities,
        });
    }
    accounts
}

/// Return only literal, non-secret Thunderbird preferences required to
/// recreate the selected IMAP accounts in a private runtime profile.
/// Directory paths, arbitrary JavaScript prefs, POP accounts, and credentials
/// are deliberately omitted. The bridge copies Thunderbird's encrypted login
/// store separately and assigns new isolated mail directories.
pub(crate) fn runtime_preferences(
    profile: &ThunderbirdProfile,
) -> io::Result<Vec<(String, String)>> {
    let prefs_path = profile.path.join("prefs.js");
    let input = read_bounded(&prefs_path, MAX_PREFS_BYTES)?;
    let parsed = parse_preferences(&input);
    let account_ids: HashSet<&str> = profile
        .accounts
        .iter()
        .map(|account| account.account_id.as_str())
        .collect();
    let server_ids: HashSet<String> = profile
        .accounts
        .iter()
        .filter_map(|account| {
            pref_string(
                &parsed,
                &format!("mail.account.{}.server", account.account_id),
            )
            .map(str::to_owned)
        })
        .collect();
    let mut identity_ids = HashSet::new();
    let mut smtp_ids = HashSet::new();
    for account_id in &account_ids {
        for identity_id in list_pref(
            &parsed,
            &format!("mail.account.{account_id}.identities"),
            MAX_IDENTITIES,
        ) {
            if let Some(smtp_id) =
                pref_string(&parsed, &format!("mail.identity.{identity_id}.smtpServer"))
            {
                if valid_pref_id(smtp_id) {
                    smtp_ids.insert(smtp_id.to_owned());
                }
            }
            identity_ids.insert(identity_id);
        }
    }
    let default_smtp =
        pref_string(&parsed, "mail.smtp.defaultserver").filter(|id| valid_pref_id(id));
    if let Some(id) = default_smtp {
        smtp_ids.insert(id.to_owned());
    }

    let mut result = Vec::new();
    for (key, value) in parsed {
        let keep = match key.as_str() {
            "mail.accountmanager.accounts" | "mail.smtpservers" | "mail.smtp.defaultserver" => true,
            "mail.accountmanager.defaultaccount" | "mail.accountmanager.localfoldersserver" => {
                false
            }
            _ => {
                let parts: Vec<&str> = key.split('.').collect();
                match parts.as_slice() {
                    ["mail", "account", id, "server"] => account_ids.contains(id),
                    ["mail", "account", id, "identities"] => account_ids.contains(id),
                    ["mail", "identity", id, field] => {
                        identity_ids.contains(*id)
                            && matches!(
                                *field,
                                "useremail"
                                    | "fullName"
                                    | "smtpServer"
                                    | "organization"
                                    | "reply_to"
                                    | "fcc_folder"
                                    | "fcc_folder_picker_mode"
                                    | "fcc_reply_follows_parent"
                                    | "draft_folder"
                                    | "stationery_folder"
                                    | "archive_folder"
                                    | "archive_folder_picker_mode"
                            )
                    }
                    ["mail", "server", id, field] => {
                        server_ids.contains(*id)
                            && matches!(
                                *field,
                                "type"
                                    | "hostname"
                                    | "port"
                                    | "socketType"
                                    | "authMethod"
                                    | "userName"
                                    | "name"
                                    | "login_at_startup"
                                    | "check_new_mail"
                                    | "check_time"
                                    | "download_on_biff"
                                    | "biffMinutes"
                                    | "useSecAuth"
                                    | "limit_offline_message_size"
                                    | "max_size"
                                    | "offline_download"
                                    | "delete_model"
                                    | "trash_folder_name"
                                    | "empty_trash_on_exit"
                                    | "moveOnSpam"
                                    | "spamActionTargetAccount"
                                    | "spamActionTargetFolder"
                            )
                    }
                    ["mail", "smtpserver", id, field] => {
                        smtp_ids.contains(*id)
                            && matches!(
                                *field,
                                "hostname"
                                    | "port"
                                    | "username"
                                    | "try_ssl"
                                    | "authMethod"
                                    | "helloArgument"
                                    | "useSecAuth"
                            )
                    }
                    _ => false,
                }
            }
        };
        if keep {
            result.push((key, render_pref_value(value)));
        }
    }
    result.retain(|(key, _)| key != "mail.accountmanager.accounts" && key != "mail.smtpservers");
    result.push((
        "mail.accountmanager.accounts".to_owned(),
        quote_pref(
            &profile
                .accounts
                .iter()
                .map(|account| account.account_id.as_str())
                .collect::<Vec<_>>()
                .join(","),
        ),
    ));
    result.push((
        "mail.smtpservers".to_owned(),
        quote_pref(&smtp_ids.into_iter().collect::<Vec<_>>().join(",")),
    ));
    Ok(result)
}

pub(crate) fn runtime_server_ids(
    profile: &ThunderbirdProfile,
) -> io::Result<Vec<(String, String)>> {
    let input = read_bounded(&profile.path.join("prefs.js"), MAX_PREFS_BYTES)?;
    let parsed = parse_preferences(&input);
    let mut result = Vec::new();
    for account in &profile.accounts {
        let Some(server_id) = pref_string(
            &parsed,
            &format!("mail.account.{}.server", account.account_id),
        ) else {
            continue;
        };
        if valid_pref_id(server_id) && !result.iter().any(|(_, existing)| existing == server_id) {
            result.push((account.account_id.clone(), server_id.to_owned()));
        }
    }
    Ok(result)
}

fn render_pref_value(value: PrefValue) -> String {
    match value {
        PrefValue::String(value) => quote_pref(&value),
        PrefValue::Number(value) => value.to_string(),
        PrefValue::Bool(value) => value.to_string(),
    }
}

fn quote_pref(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => return "\"\"".to_owned(),
            ch => output.push(ch),
        }
    }
    output.push('"');
    output
}

fn import_identity(
    prefs: &HashMap<String, PrefValue>,
    identity_id: &str,
    smtp_id: Option<&str>,
) -> ThunderbirdIdentity {
    let email = pref_string(prefs, &format!("mail.identity.{identity_id}.useremail"))
        .map(str::trim)
        .filter(|value| valid_email_hint(value))
        .map(str::to_string);
    let display_name = pref_string(prefs, &format!("mail.identity.{identity_id}.fullName"))
        .map(|value| safe_pref_text(value, 256))
        .unwrap_or_default();
    let username = smtp_id
        .and_then(|id| pref_string(prefs, &format!("mail.smtpserver.{id}.username")))
        .map(|value| safe_pref_text(value, 320))
        .unwrap_or_default();
    ThunderbirdIdentity {
        email,
        display_name,
        username,
        outgoing: smtp_id.and_then(|id| import_smtp(prefs, id)),
    }
}

fn import_smtp(
    prefs: &HashMap<String, PrefValue>,
    smtp_id: &str,
) -> Option<ThunderbirdServerSettings> {
    let prefix = format!("mail.smtpserver.{smtp_id}");
    let host = checked_host(pref_string(prefs, &format!("{prefix}.hostname")))?;
    let security = security(pref_number(prefs, &format!("{prefix}.try_ssl")).unwrap_or(-1));
    let port = port(
        pref_number(prefs, &format!("{prefix}.port")),
        match security {
            ThunderbirdSecurity::ImplicitTls => 465,
            _ => 587,
        },
    )
    .ok()?;
    Some(ThunderbirdServerSettings {
        host,
        port,
        username: pref_string(prefs, &format!("{prefix}.username"))
            .map(|value| safe_pref_text(value, 320))
            .unwrap_or_default(),
        security,
        authentication: authentication(
            pref_number(prefs, &format!("{prefix}.authMethod")).unwrap_or(-1),
        ),
    })
}

fn checked_host(host: Option<&str>) -> Option<String> {
    let host = host?.trim();
    (!host.is_empty()
        && host.len() <= 253
        && !host.chars().any(char::is_control)
        && !host.contains('/')
        && !host.contains('@'))
    .then(|| host.to_string())
}

fn safe_pref_text(value: &str, max_bytes: usize) -> String {
    let value = value.trim();
    if value.len() <= max_bytes && !value.chars().any(char::is_control) {
        value.to_string()
    } else {
        String::new()
    }
}

fn valid_email_hint(value: &str) -> bool {
    value.len() <= 320
        && !value.chars().any(char::is_control)
        && value
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty())
}

fn port(value: Option<i64>, default: u16) -> io::Result<u16> {
    match value {
        None => Ok(default),
        Some(value) if (1..=u16::MAX as i64).contains(&value) => Ok(value as u16),
        Some(_) => Err(invalid_data(
            "Thunderbird mail port is outside the valid range",
        )),
    }
}

fn security(value: i64) -> ThunderbirdSecurity {
    match value {
        0 => ThunderbirdSecurity::Plaintext,
        1 => ThunderbirdSecurity::LegacyTryStartTls,
        2 => ThunderbirdSecurity::StartTls,
        3 => ThunderbirdSecurity::ImplicitTls,
        other => ThunderbirdSecurity::Unsupported(other),
    }
}

fn authentication(value: i64) -> ThunderbirdAuthentication {
    match value {
        1 => ThunderbirdAuthentication::None,
        2 | 3 | 4 => ThunderbirdAuthentication::Password,
        10 => ThunderbirdAuthentication::OAuth2,
        other => ThunderbirdAuthentication::Unsupported(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_home() -> PathBuf {
        let id = TEST_DIR_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "megamail-thunderbird-test-{}-{id}",
            std::process::id()
        ))
    }

    fn write_profile(root: &Path, ini: &str, prefs: &str) -> PathBuf {
        fs::create_dir_all(root.join("Profiles/default")).unwrap();
        fs::write(root.join("profiles.ini"), ini).unwrap();
        fs::write(root.join("Profiles/default/prefs.js"), prefs).unwrap();
        root.join("Profiles/default")
    }

    #[test]
    fn imports_only_reviewable_imap_settings_from_multiple_roots() {
        let home = temp_home();
        let standard = home.join(".thunderbird");
        let flatpak = home.join(".var/app/org.mozilla.Thunderbird/.thunderbird");
        let ini = "[Profile0]\nName=Personal\nIsRelative=1\nPath=Profiles/default\n";
        let prefs = r#"
user_pref("mail.accountmanager.accounts", "account1,account2");
user_pref("mail.account.account1.server", "server1");
user_pref("mail.account.account1.identities", "id1,id2");
user_pref("mail.server.server1.type", "imap");
user_pref("mail.server.server1.hostname", "imap.example.net");
user_pref("mail.server.server1.port", 993);
user_pref("mail.server.server1.socketType", 3);
user_pref("mail.server.server1.authMethod", 10);
user_pref("mail.server.server1.userName", "alice@example.net");
user_pref("mail.identity.id1.useremail", "alice@example.net");
user_pref("mail.identity.id1.fullName", "Alice Example");
user_pref("mail.identity.id1.smtpServer", "smtp1");
user_pref("mail.identity.id2.useremail", "alias@example.net");
user_pref("mail.identity.id2.fullName", "Alice Alias");
user_pref("mail.identity.id2.smtpServer", "smtp2");
user_pref("mail.smtpserver.smtp1.hostname", "smtp.example.net");
user_pref("mail.smtpserver.smtp1.port", 587);
user_pref("mail.smtpserver.smtp1.try_ssl", 2);
user_pref("mail.smtpserver.smtp1.authMethod", 10);
user_pref("mail.smtpserver.smtp1.username", "alice@example.net");
user_pref("mail.smtpserver.smtp2.hostname", "smtp.alias.net");
user_pref("mail.smtpserver.smtp2.port", 465);
user_pref("mail.smtpserver.smtp2.try_ssl", 3);
user_pref("mail.smtpserver.smtp2.authMethod", 3);
user_pref("mail.smtpserver.smtp2.username", "alice-alias");
user_pref("mail.server.server1.password", "must-not-be-imported");
user_pref("mail.account.account2.server", "server2");
user_pref("mail.server.server2.type", "pop3");
user_pref("mail.server.server2.hostname", "pop.example.net");
"#;
        let standard_path = write_profile(&standard, ini, prefs);
        let flatpak_path = write_profile(&flatpak, ini, prefs);
        // These stores are deliberately invalid; import must not inspect them.
        fs::write(standard_path.join("logins.json"), "not json").unwrap();
        fs::write(standard_path.join("key4.db"), "not a database").unwrap();

        let profiles = discover_profiles_in(&home).unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().any(|profile| profile.path == standard_path));
        assert!(profiles.iter().any(|profile| profile.path == flatpak_path));
        for profile in profiles {
            assert_eq!(profile.accounts.len(), 1, "POP accounts are skipped");
            let account = &profile.accounts[0];
            assert_eq!(account.email.as_deref(), Some("alice@example.net"));
            assert_eq!(account.account_id, "account1");
            assert_eq!(account.name, "Alice Example");
            assert_eq!(account.username, "alice@example.net");
            assert_eq!(account.incoming.host, "imap.example.net");
            assert_eq!(account.incoming.port, 993);
            assert_eq!(account.incoming.security, ThunderbirdSecurity::ImplicitTls);
            assert_eq!(
                account.incoming.authentication,
                ThunderbirdAuthentication::OAuth2
            );
            let smtp = account.outgoing.as_ref().unwrap();
            assert_eq!(smtp.host, "smtp.example.net");
            assert_eq!(smtp.port, 587);
            assert_eq!(smtp.security, ThunderbirdSecurity::StartTls);
            assert_eq!(smtp.authentication, ThunderbirdAuthentication::OAuth2);
            assert_eq!(account.identities.len(), 2);
            assert_eq!(
                account.identities[1].email.as_deref(),
                Some("alias@example.net")
            );
            assert_eq!(
                account.identities[1].outgoing.as_ref().unwrap().security,
                ThunderbirdSecurity::ImplicitTls
            );
        }

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn parser_ignores_executable_javascript_and_handles_escaped_strings() {
        let prefs = r#"
user_pref("safe.key", "line\nquote: \\\"\u263a");
user_pref("unsafe.key", do_not_run_anything());
user_pref("also.unsafe", "x"); process.exit(1);
// user_pref("commented.key", "bad");
"#;
        let parsed = parse_preferences(prefs);
        assert_eq!(pref_string(&parsed, "safe.key"), Some("line\nquote: \\\"☺"));
        assert!(!parsed.contains_key("unsafe.key"));
        assert!(!parsed.contains_key("also.unsafe"));
        assert!(!parsed.contains_key("commented.key"));
    }

    #[test]
    fn runtime_preferences_are_allowlisted_and_drop_original_mail_paths_and_other_accounts() {
        let home = temp_home();
        let root = home.join(".thunderbird");
        let prefs = r#"
user_pref("mail.accountmanager.accounts", "account1,account2");
user_pref("mail.accountmanager.defaultaccount", "account2");
user_pref("mail.smtpservers", "smtp1,smtp2");
user_pref("mail.smtp.defaultserver", "smtp1");
user_pref("mail.account.account1.server", "server1");
user_pref("mail.account.account1.identities", "id1");
user_pref("mail.account.account2.server", "server2");
user_pref("mail.server.server1.type", "imap");
user_pref("mail.server.server1.hostname", "imap.example.net");
user_pref("mail.server.server1.userName", "alice@example.net");
user_pref("mail.server.server1.directory", "/home/alice/.thunderbird/old/Mail/imap.example.net");
user_pref("mail.server.server1.check_new_mail", true);
user_pref("mail.server.server1.login_at_startup", true);
user_pref("mail.server.server1.download_on_biff", true);
user_pref("mail.server.server1.loginAtStartup", true);
user_pref("mail.server.server1.downloadOnBiff", true);
user_pref("mail.server.server2.type", "pop3");
user_pref("mail.server.server2.hostname", "pop.example.net");
user_pref("mail.identity.id1.useremail", "alice@example.net");
user_pref("mail.identity.id1.smtpServer", "smtp1");
user_pref("mail.identity.id2.useremail", "other@example.net");
user_pref("mail.smtpserver.smtp1.hostname", "smtp.example.net");
user_pref("mail.smtpserver.smtp2.hostname", "unselected.example.net");
user_pref("mail.server.server1.password", "never-copy-this");
user_pref("network.cookie.cookieBehavior", 0);
"#;
        let profile_path = write_profile(
            &root,
            "[Profile0]\nName=Test\nIsRelative=1\nPath=Profiles/default\n",
            prefs,
        );
        let profile = discover_profiles_in(&home).unwrap().remove(0);
        assert_eq!(profile.path, profile_path);
        let rendered: HashMap<_, _> = runtime_preferences(&profile).unwrap().into_iter().collect();
        assert_eq!(
            rendered
                .get("mail.accountmanager.accounts")
                .map(String::as_str),
            Some("\"account1\"")
        );
        assert!(!rendered.contains_key("mail.accountmanager.defaultaccount"));
        assert!(!rendered.contains_key("mail.account.account2.server"));
        assert!(!rendered.contains_key("mail.server.server1.directory"));
        assert!(!rendered.contains_key("mail.server.server2.hostname"));
        assert!(!rendered.contains_key("mail.server.server1.password"));
        assert!(!rendered.contains_key("mail.identity.id2.useremail"));
        assert!(!rendered.contains_key("mail.smtpserver.smtp2.hostname"));
        assert_eq!(
            rendered
                .get("mail.server.server1.check_new_mail")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(
            rendered
                .get("mail.server.server1.login_at_startup")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(
            rendered
                .get("mail.server.server1.download_on_biff")
                .map(String::as_str),
            Some("true")
        );
        assert!(!rendered.contains_key("mail.server.server1.loginAtStartup"));
        assert!(!rendered.contains_key("mail.server.server1.downloadOnBiff"));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn oversized_and_escaping_profiles_are_rejected_or_skipped() {
        let home = temp_home();
        let root = home.join(".thunderbird");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("profiles.ini"),
            "x".repeat(MAX_PROFILES_INI_BYTES as usize + 1),
        )
        .unwrap();
        assert_eq!(
            discover_profiles_in(&home).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let _ = fs::remove_dir_all(&home);

        let home = temp_home();
        let root = home.join(".thunderbird");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("profiles.ini"),
            "[Profile0]\nName=Outside\nIsRelative=1\nPath=../outside\n",
        )
        .unwrap();
        assert!(discover_profiles_in(&home).unwrap().is_empty());
        let _ = fs::remove_dir_all(&home);
    }
}
