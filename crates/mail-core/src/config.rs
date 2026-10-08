//! Portable account and runtime preferences for MegaMail's mail core.
//!
//! Account metadata is kept below the MegaMail-specific XDG directories.
//! Passwords and OAuth client/refresh secrets are stored in the OS keyring,
//! while the TOML account file contains only non-secret configuration.

use std::fmt;
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const APP_DIR: &str = "megamail";
const KEYRING_SERVICE: &str = "com.megamail.MegaMail";
static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);
static CREDENTIALS_LOCK: Mutex<()> = Mutex::new(());

/// Create a private directory, tightening an existing directory as well.
fn ensure_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn app_dir(root: Option<PathBuf>) -> Option<PathBuf> {
    let path = root?.join(APP_DIR);
    if let Err(err) = ensure_private_dir(&path) {
        tracing::warn!(
            "could not secure MegaMail directory {}: {err}",
            path.display()
        );
        return None;
    }
    Some(path)
}

/// MegaMail's private configuration directory (`$XDG_CONFIG_HOME/megamail`).
pub fn config_base() -> Option<PathBuf> {
    app_dir(dirs::config_dir())
}

/// MegaMail's private cache directory (`$XDG_CACHE_HOME/megamail`).
pub fn cache_base() -> Option<PathBuf> {
    app_dir(dirs::cache_dir())
}

/// MegaMail's private persistent-data directory (`$XDG_DATA_HOME/megamail`).
pub fn data_base() -> Option<PathBuf> {
    app_dir(dirs::data_dir())
}

/// Location of MegaMail's account metadata. Hylki's config is never consulted.
pub fn path() -> Option<PathBuf> {
    Some(config_base()?.join("accounts.toml"))
}

/// Write a private file through a same-directory temporary file and atomic
/// rename. New files start mode 0600; their parent directory is mode 0700.
pub fn write_private_file(path: &Path, contents: &str) -> io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let temp_parent = parent.unwrap_or(Path::new("."));
    if let Some(parent) = parent {
        // The XDG app directories are secured by `config_base`/`cache_base`/
        // `data_base`. This helper also serves callers with an explicit path,
        // so create missing parents without chmodding an arbitrary existing
        // directory such as the user's home or `/tmp`.
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "private file has no name"))?
        .to_string_lossy();

    for _ in 0..32 {
        let serial = TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let temp = temp_parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), serial));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&temp) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        };

        let write_result = file
            .write_all(contents.as_bytes())
            .and_then(|_| file.sync_all());
        drop(file);
        if let Err(err) = write_result {
            let _ = fs::remove_file(&temp);
            return Err(err);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(err) = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600)) {
                let _ = fs::remove_file(&temp);
                return Err(err);
            }
        }
        if let Err(err) = fs::rename(&temp, path) {
            let _ = fs::remove_file(&temp);
            return Err(err);
        }
        // The file contents are durable before the rename. Syncing the
        // directory also makes the name update durable on Unix.
        #[cfg(unix)]
        if let Ok(dir) = File::open(temp_parent) {
            let _ = dir.sync_all();
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate private temporary file",
    ))
}

fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    write_private_file(path, contents)
}

fn io_keyring_error(context: &str, err: keyring::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{context}: {err}"))
}

fn default_on() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn default_imap_port() -> u16 {
    993
}

fn default_smtp_port() -> u16 {
    587
}

fn is_default_smtp_port(port: &u16) -> bool {
    *port == default_smtp_port()
}

/// Incoming-mail protocol supported by an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Imap,
    Pop3,
    Graph,
    Jmap,
}

/// Explicit TLS and SMTP-auth details supplied by account discovery or setup.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct ServerSecurity {
    #[serde(default)]
    pub imap_starttls: bool,
    #[serde(default)]
    pub smtp_starttls: bool,
    #[serde(default)]
    pub imap_accept_invalid_certs: bool,
    #[serde(default)]
    pub smtp_accept_invalid_certs: bool,
    #[serde(default)]
    pub smtp_no_auth: bool,
}

/// A configured mailbox. Secret strings are transient runtime values and are
/// persisted through MegaMail's keyring entries, never as account TOML fields.
#[derive(Clone, Deserialize, Serialize)]
pub struct AccountConfig {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub protocol: Protocol,
    pub imap_host: String,
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    pub username: String,
    #[serde(default, skip_serializing)]
    pub password: String,
    #[serde(default)]
    pub smtp_separate: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub jmap_token: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub jmap_generic: bool,
    #[serde(default)]
    pub tls_accept_hostname_mismatch: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<ServerSecurity>,
    #[serde(default)]
    pub smtp_username: String,
    #[serde(default, skip_serializing)]
    pub smtp_password: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub gravatar: bool,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub signature_html: bool,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, deserialize_with = "deserialize_aliases")]
    pub aliases: Vec<AliasConfig>,
    #[serde(default = "default_on")]
    pub enabled: bool,
    #[serde(default)]
    pub goa_id: Option<String>,
    #[serde(default)]
    pub goa_mail_disabled: bool,
    #[serde(default = "default_on")]
    pub goa_enabled_before_mail_disabled: bool,
    #[serde(default)]
    pub oauth: bool,
    #[serde(default)]
    pub oauth_settings: Option<OAuthSettings>,
    #[serde(default, skip_serializing)]
    pub oauth_refresh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<bool>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub folder_roles: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_folders: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub folders_seeded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_copy_path: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub server_saves_sent: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub empty_junk_days: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub empty_trash_days: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pgp_key: Option<String>,
    #[serde(default = "default_on", skip_serializing_if = "is_true")]
    pub in_unified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_sort: Option<FolderSort>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sign_by_default: bool,
}

impl fmt::Debug for AccountConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountConfig")
            .field("name", &self.name)
            .field("email", &self.email)
            .field("protocol", &self.protocol)
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("smtp_password", &"[REDACTED]")
            .field("oauth_refresh", &"[REDACTED]")
            .field("oauth", &self.oauth)
            .field("oauth_settings", &self.oauth_settings)
            .field("aliases", &self.aliases)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl AccountConfig {
    /// Start a fully initialized account with conservative connection
    /// defaults. Discovery may then fill in provider-specific hosts, protocol,
    /// and explicit TLS settings before the account is saved.
    pub fn new(email: impl Into<String>) -> Self {
        let email = email.into();
        Self {
            name: email.clone(),
            email: email.clone(),
            protocol: Protocol::default(),
            imap_host: String::new(),
            imap_port: default_imap_port(),
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            username: email,
            password: String::new(),
            smtp_separate: false,
            jmap_token: false,
            jmap_generic: false,
            tls_accept_hostname_mismatch: false,
            security: None,
            smtp_username: String::new(),
            smtp_password: String::new(),
            color: None,
            emoji: None,
            avatar: None,
            gravatar: false,
            signature: None,
            signature_html: false,
            label: None,
            aliases: Vec::new(),
            enabled: true,
            goa_id: None,
            goa_mail_disabled: false,
            goa_enabled_before_mail_disabled: true,
            oauth: false,
            oauth_settings: None,
            oauth_refresh: String::new(),
            push: None,
            folder_roles: Default::default(),
            hidden_folders: Vec::new(),
            folders_seeded: false,
            sent_copy_path: None,
            server_saves_sent: false,
            empty_junk_days: 0,
            empty_trash_days: 0,
            pgp_key: None,
            in_unified: true,
            folder_sort: None,
            sign_by_default: false,
        }
    }

    /// The account's display label, falling back to its email address.
    pub fn display_label(&self) -> String {
        self.label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .unwrap_or(&self.email)
            .to_string()
    }
}

/// A send-as identity with optional provider-specific SMTP credentials.
#[derive(Clone, Deserialize, Serialize)]
pub struct AliasConfig {
    pub identity: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub smtp_host: String,
    #[serde(
        default = "default_smtp_port",
        skip_serializing_if = "is_default_smtp_port"
    )]
    pub smtp_port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub smtp_username: String,
    #[serde(default, skip_serializing)]
    pub smtp_password: String,
}

impl fmt::Debug for AliasConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AliasConfig")
            .field("identity", &self.identity)
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .field("smtp_username", &self.smtp_username)
            .field("smtp_password", &"[REDACTED]")
            .finish()
    }
}

impl Default for AliasConfig {
    fn default() -> Self {
        Self {
            identity: String::new(),
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            smtp_username: String::new(),
            smtp_password: String::new(),
        }
    }
}

impl AliasConfig {
    pub fn address(&self) -> String {
        split_identity(&self.identity).1
    }

    pub fn has_own_smtp(&self) -> bool {
        !self.smtp_host.trim().is_empty()
    }
}

/// Split `Name <address>` into name/address, or return an empty name for a
/// bare address.
pub fn split_identity(value: &str) -> (String, String) {
    match value.split_once('<') {
        Some((name, rest)) => (
            name.trim().trim_matches('"').to_string(),
            rest.trim_end_matches('>').trim().to_string(),
        ),
        None => (String::new(), value.trim().to_string()),
    }
}

fn deserialize_aliases<'de, D>(deserializer: D) -> Result<Vec<AliasConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Plain(String),
        Full(AliasConfig),
    }
    Ok(Vec::<Entry>::deserialize(deserializer)?
        .into_iter()
        .map(|entry| match entry {
            Entry::Plain(identity) => AliasConfig {
                identity,
                ..AliasConfig::default()
            },
            Entry::Full(alias) => alias,
        })
        .collect())
}

/// OAuth client configuration. `client_secret` keeps its original public Rust
/// shape, but is persisted in the keyring rather than the account TOML file.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct OAuthSettings {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    #[serde(default, skip_serializing)]
    pub client_secret: String,
    pub scopes: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub redirect_uri: String,
}

impl fmt::Debug for OAuthSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthSettings")
            .field("auth_url", &self.auth_url)
            .field("token_url", &self.token_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderSort {
    #[default]
    Custom,
    NameAsc,
    NameDesc,
    Path,
}

impl FolderSort {
    pub const ALL: [FolderSort; 4] = [Self::Custom, Self::NameAsc, Self::NameDesc, Self::Path];

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|item| *item == self).unwrap_or(0) as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DateStyle {
    #[default]
    System,
    MonthFirst,
    DayFirst,
    YearFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClockStyle {
    #[default]
    System,
    Twelve,
    TwentyFour,
}

/// Small runtime preference set consumed by the mail worker and reader. UI
/// layout, themes, and desktop integration preferences live in the GPUI app.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PrivacyFile {
    pub allowed_senders: Vec<String>,
    pub auto_remote_content: bool,
    pub fetch_interval_secs: u64,
    pub push: bool,
    pub blacklist: Vec<String>,
    pub preview_lines: u32,
    pub threading: bool,
    pub date_style: DateStyle,
    pub clock_style: ClockStyle,
}

impl Default for PrivacyFile {
    fn default() -> Self {
        Self {
            allowed_senders: Vec::new(),
            auto_remote_content: false,
            fetch_interval_secs: 300,
            push: true,
            blacklist: Vec::new(),
            preview_lines: 1,
            threading: true,
            date_style: DateStyle::default(),
            clock_style: ClockStyle::default(),
        }
    }
}

fn privacy_path() -> Option<PathBuf> {
    Some(config_base()?.join("privacy.toml"))
}

/// Read runtime settings. Missing or malformed files fail closed to defaults,
/// including blocking remote content by default.
pub fn load_privacy() -> PrivacyFile {
    let Some(path) = privacy_path() else {
        return PrivacyFile::default();
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return PrivacyFile::default();
    };
    toml::from_str(&contents).unwrap_or_default()
}

pub fn save_privacy(settings: PrivacyFile) {
    let Some(path) = privacy_path() else { return };
    match toml::to_string_pretty(&settings) {
        Ok(contents) => {
            if let Err(err) = write_private(&path, &contents) {
                tracing::warn!("could not save MegaMail runtime preferences: {err}");
            }
        }
        Err(err) => tracing::warn!("could not serialize MegaMail runtime preferences: {err}"),
    }
}

pub fn load_preview_lines() -> u32 {
    load_privacy().preview_lines.min(3)
}

pub fn load_date_format() -> (DateStyle, ClockStyle) {
    let settings = load_privacy();
    (settings.date_style, settings.clock_style)
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ConfigFile {
    /// The active versioned keyring namespace for each normalized mailbox.
    /// Empty/missing generations keep compatibility with this new app's first
    /// unversioned credential layout.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    credential_generations: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    accounts: Vec<AccountConfig>,
}

fn normalized_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn validate_unique_emails(accounts: &[AccountConfig]) -> io::Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for account in accounts {
        if !seen.insert(normalized_email(&account.email)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "multiple accounts use {}; this profile cannot key credentials by email",
                    account.email.trim()
                ),
            ));
        }
    }
    Ok(())
}

fn read_config_file(path: &Path) -> io::Result<ConfigFile> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(ConfigFile::default()),
        Err(err) => return Err(err),
    };
    let config: ConfigFile = toml::from_str(&text).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {err}", path.display()),
        )
    })?;
    validate_unique_emails(&config.accounts)?;
    Ok(config)
}

fn write_config_file(path: &Path, config: &ConfigFile) -> io::Result<()> {
    let text = toml::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    write_private(path, &text)
}

/// Load accounts only from MegaMail's own configuration directory. OAuth
/// client secrets written by an earlier MegaMail version are moved to its
/// keyring service and removed from disk after that succeeds.
pub fn load() -> Option<Vec<AccountConfig>> {
    match load_accounts_checked() {
        Ok(accounts) => accounts,
        Err(err) => {
            tracing::error!("could not load MegaMail accounts: {err}");
            None
        }
    }
}

fn load_accounts_checked() -> io::Result<Option<Vec<AccountConfig>>> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    load_accounts_checked_locked()
}

fn load_accounts_checked_locked() -> io::Result<Option<Vec<AccountConfig>>> {
    let path =
        path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))?;
    let mut cfg = read_config_file(&path)?;
    if cfg.accounts.is_empty() {
        return Ok(None);
    }

    for account in &mut cfg.accounts {
        // Preserve compatibility with pre-Graph GOA records, without looking
        // outside MegaMail's own account file.
        if account.protocol == Protocol::Imap
            && account.oauth
            && account.goa_id.is_some()
            && account.imap_host.trim().is_empty()
        {
            account.protocol = Protocol::Graph;
        }
    }

    let mut can_strip_legacy_client_secrets = false;
    for account in &mut cfg.accounts {
        let Some(settings) = account.oauth_settings.as_mut() else {
            continue;
        };
        if let Some(secret) = load_oauth_client_secret(&account.email) {
            if !settings.client_secret.is_empty() {
                can_strip_legacy_client_secrets = true;
            }
            settings.client_secret = secret;
        } else if !settings.client_secret.is_empty()
            && store_account_credential(
                &account.email,
                CredentialKind::OAuthClientSecret,
                None,
                &settings.client_secret,
            )
            .is_ok()
        {
            can_strip_legacy_client_secrets = true;
        }
    }
    if can_strip_legacy_client_secrets && secrets_are_in_keyring(&cfg.accounts) {
        if let Err(err) = write_config_file(&path, &cfg) {
            tracing::warn!("could not remove migrated OAuth client secrets from disk: {err}");
        }
    }

    Ok(Some(cfg.accounts))
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProfileIdsFile {
    /// Historical mappings are intentionally retained when an account is
    /// removed, so re-adding the same mailbox reconnects to its existing cache.
    #[serde(default)]
    profiles: std::collections::BTreeMap<String, u32>,
}

#[derive(Serialize)]
struct ProfileIdentity<'a> {
    email: &'a str,
    protocol: Protocol,
    imap_host: &'a str,
    imap_port: u16,
    username: &'a str,
}

fn profile_identity(account: &AccountConfig) -> io::Result<String> {
    toml::to_string(&ProfileIdentity {
        email: &account.email,
        protocol: account.protocol,
        imap_host: &account.imap_host,
        imap_port: account.imap_port,
        username: &account.username,
    })
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn profile_ids_path() -> io::Result<PathBuf> {
    config_base()
        .map(|base| base.join("profile_ids.toml"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))
}

fn read_profile_ids(path: &Path) -> io::Result<ProfileIdsFile> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(ProfileIdsFile::default()),
        Err(err) => return Err(err),
    };
    let file: ProfileIdsFile = toml::from_str(&text).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {err}", path.display()),
        )
    })?;
    validate_profile_ids(&file.profiles)?;
    Ok(file)
}

fn validate_profile_ids(ids: &std::collections::BTreeMap<String, u32>) -> io::Result<u32> {
    let mut seen = std::collections::BTreeSet::new();
    let mut maximum = 0;
    for id in ids.values().copied() {
        if id == 0 || !seen.insert(id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile IDs must be positive and unique",
            ));
        }
        maximum = maximum.max(id);
    }
    Ok(maximum)
}

fn assign_profile_ids(
    accounts: Vec<AccountConfig>,
    ids: &mut std::collections::BTreeMap<String, u32>,
) -> io::Result<(Vec<(u32, AccountConfig)>, bool)> {
    let mut next = validate_profile_ids(ids)?;
    let mut changed = false;
    let mut profiles = Vec::with_capacity(accounts.len());
    for account in accounts {
        let identity = profile_identity(&account)?;
        let id = match ids.get(&identity).copied() {
            Some(id) => id,
            None => {
                next = next.checked_add(1).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "profile ID space exhausted")
                })?;
                ids.insert(identity, next);
                changed = true;
                next
            }
        };
        profiles.push((id, account));
    }
    Ok((profiles, changed))
}

fn write_profile_ids(path: &Path, ids: &std::collections::BTreeMap<String, u32>) -> io::Result<()> {
    let text = toml::to_string_pretty(&ProfileIdsFile {
        profiles: ids.clone(),
    })
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    write_private(path, &text)
}

/// Load every configured account with a stable cache/worker ID. Disabled
/// accounts receive IDs too; the UI decides whether to display them. Mapping
/// entries outlive account removal, and malformed state is returned as an
/// error rather than replaced with an empty mapping.
pub fn load_profiles() -> io::Result<Vec<(u32, AccountConfig)>> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let accounts = load_accounts_checked_locked()?.unwrap_or_default();
    let ids_path = profile_ids_path()?;
    let mut ids = read_profile_ids(&ids_path)?.profiles;
    let (profiles, changed) = assign_profile_ids(accounts, &mut ids)?;
    if changed {
        write_profile_ids(&ids_path, &ids)?;
    }
    Ok(profiles)
}

#[derive(Clone, Copy)]
enum CredentialKind {
    Password,
    SmtpPassword,
    AliasSmtp,
    OAuthRefresh,
    OAuthClientSecret,
}

impl CredentialKind {
    fn name(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::SmtpPassword => "smtp",
            Self::AliasSmtp => "alias-smtp",
            Self::OAuthRefresh => "oauth-refresh",
            Self::OAuthClientSecret => "oauth-client",
        }
    }

    fn context(self) -> &'static str {
        match self {
            Self::Password => "account password",
            Self::SmtpPassword => "SMTP password",
            Self::AliasSmtp => "alias SMTP password",
            Self::OAuthRefresh => "OAuth refresh token",
            Self::OAuthClientSecret => "OAuth client secret",
        }
    }
}

fn new_credential_generation() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|err| {
        io::Error::new(io::ErrorKind::Other, format!("secure random source: {err}"))
    })?;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
}

fn credential_key(
    generation: Option<&str>,
    kind: CredentialKind,
    email: &str,
    alias_addr: Option<&str>,
) -> String {
    match generation.filter(|generation| !generation.is_empty()) {
        Some(generation) => {
            let mailbox = normalized_email(email);
            let alias = alias_addr
                .map(|addr| addr.trim().to_ascii_lowercase())
                .unwrap_or_default();
            // Length prefixes keep unusual email/alias strings from creating
            // ambiguous separators in the keyring username.
            format!(
                "v1:{generation}:{}:{}:{mailbox}:{}:{alias}",
                kind.name(),
                mailbox.len(),
                alias.len()
            )
        }
        None => match kind {
            CredentialKind::Password => email.to_string(),
            CredentialKind::SmtpPassword => smtp_key(email),
            CredentialKind::AliasSmtp => alias_smtp_key(email, alias_addr.unwrap_or_default()),
            CredentialKind::OAuthRefresh => oauth_key(email),
            CredentialKind::OAuthClientSecret => oauth_client_key(email),
        },
    }
}

fn read_credential_at(
    generation: Option<&str>,
    kind: CredentialKind,
    email: &str,
    alias_addr: Option<&str>,
) -> keyring::Result<Option<String>> {
    let key = credential_key(generation, kind, email, alias_addr);
    match keyring_entry(&key)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err),
    }
}

fn store_credential_at(
    generation: &str,
    kind: CredentialKind,
    email: &str,
    alias_addr: Option<&str>,
    secret: &str,
) -> keyring::Result<()> {
    let key = credential_key(Some(generation), kind, email, alias_addr);
    keyring_entry(&key)?.set_password(secret)
}

trait CredentialBackend {
    fn read(
        &mut self,
        generation: Option<&str>,
        kind: CredentialKind,
        email: &str,
        alias_addr: Option<&str>,
    ) -> keyring::Result<Option<String>>;

    fn store(
        &mut self,
        generation: &str,
        kind: CredentialKind,
        email: &str,
        alias_addr: Option<&str>,
        secret: &str,
    ) -> keyring::Result<()>;
}

struct SystemCredentialBackend;

impl CredentialBackend for SystemCredentialBackend {
    fn read(
        &mut self,
        generation: Option<&str>,
        kind: CredentialKind,
        email: &str,
        alias_addr: Option<&str>,
    ) -> keyring::Result<Option<String>> {
        read_credential_at(generation, kind, email, alias_addr)
    }

    fn store(
        &mut self,
        generation: &str,
        kind: CredentialKind,
        email: &str,
        alias_addr: Option<&str>,
        secret: &str,
    ) -> keyring::Result<()> {
        store_credential_at(generation, kind, email, alias_addr, secret)
    }
}

fn stage_credential(
    email: &str,
    old_email: &str,
    old_generation: Option<&str>,
    new_generation: &str,
    kind: CredentialKind,
    alias_addr: Option<&str>,
    supplied: &str,
    prior_plaintext: Option<&str>,
) -> io::Result<()> {
    stage_credential_with_backend(
        &mut SystemCredentialBackend,
        email,
        old_email,
        old_generation,
        new_generation,
        kind,
        alias_addr,
        supplied,
        prior_plaintext,
    )
}

fn stage_credential_with_backend(
    backend: &mut impl CredentialBackend,
    email: &str,
    old_email: &str,
    old_generation: Option<&str>,
    new_generation: &str,
    kind: CredentialKind,
    alias_addr: Option<&str>,
    supplied: &str,
    prior_plaintext: Option<&str>,
) -> io::Result<()> {
    let retained = if !supplied.is_empty() {
        Some(supplied.to_string())
    } else if let Some(previous) = prior_plaintext.filter(|secret| !secret.is_empty()) {
        Some(previous.to_string())
    } else {
        backend
            .read(old_generation, kind, old_email, alias_addr)
            .map_err(|err| {
                io_keyring_error(&format!("could not read existing {}", kind.context()), err)
            })?
    };
    if let Some(secret) = retained {
        backend
            .store(new_generation, kind, email, alias_addr, &secret)
            .map_err(|err| io_keyring_error(&format!("could not stage {}", kind.context()), err))?;
    }
    Ok(())
}

/// Save account metadata atomically and put any supplied credentials in the
/// keyring first. Each save uses a fresh credential generation; the old
/// generation remains active until the atomic account-file rename commits.
pub fn save(accounts: &[AccountConfig]) -> io::Result<()> {
    validate_unique_emails(accounts)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))?;
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config_path =
        path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))?;
    let current = read_config_file(&config_path)?;
    let mut generations = current.credential_generations.clone();

    for account in accounts {
        let mailbox_key = normalized_email(&account.email);
        let old_generation = current
            .credential_generations
            .get(&mailbox_key)
            .map(String::as_str)
            .filter(|generation| !generation.is_empty());
        let previous = current
            .accounts
            .iter()
            .find(|candidate| normalized_email(&candidate.email) == mailbox_key);
        let old_email = previous.map_or(account.email.as_str(), |old| old.email.as_str());
        let new_generation = new_credential_generation()?;

        stage_credential(
            &account.email,
            old_email,
            old_generation,
            &new_generation,
            CredentialKind::Password,
            None,
            &account.password,
            previous.map(|old| old.password.as_str()),
        )?;
        stage_credential(
            &account.email,
            old_email,
            old_generation,
            &new_generation,
            CredentialKind::SmtpPassword,
            None,
            &account.smtp_password,
            previous.map(|old| old.smtp_password.as_str()),
        )?;
        stage_credential(
            &account.email,
            old_email,
            old_generation,
            &new_generation,
            CredentialKind::OAuthRefresh,
            None,
            &account.oauth_refresh,
            previous.map(|old| old.oauth_refresh.as_str()),
        )?;
        if let Some(settings) = &account.oauth_settings {
            stage_credential(
                &account.email,
                old_email,
                old_generation,
                &new_generation,
                CredentialKind::OAuthClientSecret,
                None,
                &settings.client_secret,
                previous
                    .and_then(|old| old.oauth_settings.as_ref())
                    .map(|old| old.client_secret.as_str()),
            )?;
        }
        for alias in &account.aliases {
            let address = alias.address();
            let old_alias = previous.and_then(|old| {
                old.aliases
                    .iter()
                    .find(|candidate| candidate.address().eq_ignore_ascii_case(&address))
            });
            stage_credential(
                &account.email,
                old_email,
                old_generation,
                &new_generation,
                CredentialKind::AliasSmtp,
                Some(&address),
                &alias.smtp_password,
                old_alias.map(|old| old.smtp_password.as_str()),
            )?;
        }
        generations.insert(mailbox_key, new_generation);
    }

    // ponytail:defer orphan-generation cleanup until it can be made safe across
    // keyring and file failures; old generations are harmless and preserve recovery.
    write_config_file(
        &config_path,
        &ConfigFile {
            credential_generations: generations,
            accounts: accounts.to_vec(),
        },
    )
}

fn write_config(accounts: &[AccountConfig]) -> io::Result<()> {
    let path =
        path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))?;
    validate_unique_emails(accounts)?;
    let mut config = read_config_file(&path)?;
    config.accounts = accounts.to_vec();
    write_config_file(&path, &config)
}

/// Remove any legacy plaintext credential fields from MegaMail's account file
/// only after every such value is confirmed in the MegaMail keyring.
pub fn strip_passwords_on_disk() {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let accounts = match load_accounts_checked_locked() {
        Ok(Some(accounts)) => accounts,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!("keeping account TOML because it could not be read safely: {err}");
            return;
        }
    };
    for account in &accounts {
        if (!account.password.is_empty()
            && !credential_matches(
                &account.email,
                CredentialKind::Password,
                None,
                &account.password,
            ))
            || (!account.smtp_password.is_empty()
                && !credential_matches(
                    &account.email,
                    CredentialKind::SmtpPassword,
                    None,
                    &account.smtp_password,
                ))
            || (!account.oauth_refresh.is_empty()
                && !credential_matches(
                    &account.email,
                    CredentialKind::OAuthRefresh,
                    None,
                    &account.oauth_refresh,
                ))
        {
            tracing::warn!("keeping account TOML until its secrets are safely in the keyring");
            return;
        }
        if let Some(settings) = &account.oauth_settings {
            if !settings.client_secret.is_empty()
                && !credential_matches(
                    &account.email,
                    CredentialKind::OAuthClientSecret,
                    None,
                    &settings.client_secret,
                )
            {
                tracing::warn!(
                    "keeping account TOML until its OAuth client secret is safely in the keyring"
                );
                return;
            }
        }
        for alias in &account.aliases {
            if !alias.smtp_password.is_empty()
                && !credential_matches(
                    &account.email,
                    CredentialKind::AliasSmtp,
                    Some(&alias.address()),
                    &alias.smtp_password,
                )
            {
                tracing::warn!(
                    "keeping account TOML until its alias SMTP secret is safely in the keyring"
                );
                return;
            }
        }
    }
    if let Err(err) = write_config(&accounts) {
        tracing::warn!("could not rewrite account TOML without secrets: {err}");
    }
}

fn credential_matches(
    email: &str,
    kind: CredentialKind,
    alias_addr: Option<&str>,
    expected: &str,
) -> bool {
    matches!(read_account_credential(email, kind, alias_addr), Ok(Some(found)) if found == expected)
}

fn secrets_are_in_keyring(accounts: &[AccountConfig]) -> bool {
    accounts.iter().all(|account| {
        (account.password.is_empty()
            || credential_matches(
                &account.email,
                CredentialKind::Password,
                None,
                &account.password,
            ))
            && (account.smtp_password.is_empty()
                || credential_matches(
                    &account.email,
                    CredentialKind::SmtpPassword,
                    None,
                    &account.smtp_password,
                ))
            && (account.oauth_refresh.is_empty()
                || credential_matches(
                    &account.email,
                    CredentialKind::OAuthRefresh,
                    None,
                    &account.oauth_refresh,
                ))
            && account.oauth_settings.as_ref().map_or(true, |settings| {
                settings.client_secret.is_empty()
                    || credential_matches(
                        &account.email,
                        CredentialKind::OAuthClientSecret,
                        None,
                        &settings.client_secret,
                    )
            })
            && account.aliases.iter().all(|alias| {
                alias.smtp_password.is_empty()
                    || credential_matches(
                        &account.email,
                        CredentialKind::AliasSmtp,
                        Some(&alias.address()),
                        &alias.smtp_password,
                    )
            })
    })
}

fn keyring_entry(key: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, key)
}

fn smtp_key(email: &str) -> String {
    format!("smtp:{email}")
}

fn alias_smtp_key(email: &str, alias_addr: &str) -> String {
    format!("smtp-alias:{email}:{}", alias_addr.to_lowercase())
}

fn oauth_key(email: &str) -> String {
    format!("oauth:{email}")
}

fn oauth_client_key(email: &str) -> String {
    format!("oauth-client:{email}")
}

fn config_error_for_keyring(err: io::Error) -> keyring::Error {
    keyring::Error::PlatformFailure(Box::new(err))
}

fn active_generation(email: &str) -> keyring::Result<Option<String>> {
    let path = path().ok_or_else(|| {
        config_error_for_keyring(io::Error::new(
            io::ErrorKind::NotFound,
            "no MegaMail config directory",
        ))
    })?;
    let config = read_config_file(&path).map_err(config_error_for_keyring)?;
    Ok(config
        .credential_generations
        .get(&normalized_email(email))
        .cloned()
        .filter(|generation| !generation.is_empty()))
}

fn read_account_credential(
    email: &str,
    kind: CredentialKind,
    alias_addr: Option<&str>,
) -> keyring::Result<Option<String>> {
    let generation = active_generation(email)?;
    read_credential_at(generation.as_deref(), kind, email, alias_addr)
}

fn store_account_credential(
    email: &str,
    kind: CredentialKind,
    alias_addr: Option<&str>,
    secret: &str,
) -> keyring::Result<()> {
    let generation = active_generation(email)?;
    let key = credential_key(generation.as_deref(), kind, email, alias_addr);
    keyring_entry(&key)?.set_password(secret)
}

fn load_account_credential(
    email: &str,
    kind: CredentialKind,
    alias_addr: Option<&str>,
) -> Option<String> {
    read_account_credential(email, kind, alias_addr).unwrap_or_else(|err| {
        tracing::warn!("could not read MegaMail credential: {err}");
        None
    })
}

fn delete_account_credential(
    email: &str,
    generation: Option<&str>,
    kind: CredentialKind,
    alias_addr: Option<&str>,
) {
    delete_key(&credential_key(generation, kind, email, alias_addr));
}

pub fn store_oauth_refresh(email: &str, token: &str) -> keyring::Result<()> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store_account_credential(email, CredentialKind::OAuthRefresh, None, token)
}

pub fn load_oauth_refresh(email: &str) -> Option<String> {
    load_account_credential(email, CredentialKind::OAuthRefresh, None)
}

pub fn store_oauth_client_secret(email: &str, secret: &str) -> keyring::Result<()> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store_account_credential(email, CredentialKind::OAuthClientSecret, None, secret)
}

pub fn load_oauth_client_secret(email: &str) -> Option<String> {
    load_account_credential(email, CredentialKind::OAuthClientSecret, None)
}

pub fn store_password(email: &str, password: &str) -> keyring::Result<()> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store_account_credential(email, CredentialKind::Password, None, password)
}

pub fn load_password(email: &str) -> Option<String> {
    load_account_credential(email, CredentialKind::Password, None)
}

pub fn store_smtp_password(email: &str, password: &str) -> keyring::Result<()> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store_account_credential(email, CredentialKind::SmtpPassword, None, password)
}

pub fn load_smtp_password(email: &str) -> Option<String> {
    load_account_credential(email, CredentialKind::SmtpPassword, None)
}

pub fn store_alias_smtp_password(
    email: &str,
    alias_addr: &str,
    password: &str,
) -> keyring::Result<()> {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store_account_credential(email, CredentialKind::AliasSmtp, Some(alias_addr), password)
}

pub fn load_alias_smtp_password(email: &str, alias_addr: &str) -> Option<String> {
    load_account_credential(email, CredentialKind::AliasSmtp, Some(alias_addr))
}

pub fn delete_alias_smtp_password(email: &str, alias_addr: &str) {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active_generation(email) {
        Ok(generation) => delete_account_credential(
            email,
            generation.as_deref(),
            CredentialKind::AliasSmtp,
            Some(alias_addr),
        ),
        Err(err) => tracing::warn!("could not resolve alias credential generation: {err}"),
    }
}

pub fn store_cloud_password(key: &str, password: &str) -> keyring::Result<()> {
    keyring_entry(key)?.set_password(password)
}

pub fn load_cloud_password(key: &str) -> Option<String> {
    load_key(key)
}

pub fn delete_cloud_password(key: &str) {
    delete_key(key);
}

pub fn delete_account_secrets(account: &AccountConfig) {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active_generation(&account.email) {
        Ok(generation) => {
            let generation = generation.as_deref();
            delete_account_credential(&account.email, generation, CredentialKind::Password, None);
            delete_account_credential(
                &account.email,
                generation,
                CredentialKind::SmtpPassword,
                None,
            );
            delete_account_credential(
                &account.email,
                generation,
                CredentialKind::OAuthRefresh,
                None,
            );
            delete_account_credential(
                &account.email,
                generation,
                CredentialKind::OAuthClientSecret,
                None,
            );
            for alias in &account.aliases {
                delete_account_credential(
                    &account.email,
                    generation,
                    CredentialKind::AliasSmtp,
                    Some(&alias.address()),
                );
            }
        }
        Err(err) => tracing::warn!("could not resolve account credential generation: {err}"),
    }
}

pub fn delete_password(email: &str) {
    let _guard = CREDENTIALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active_generation(email) {
        Ok(generation) => {
            let generation = generation.as_deref();
            delete_account_credential(email, generation, CredentialKind::Password, None);
            delete_account_credential(email, generation, CredentialKind::SmtpPassword, None);
            delete_account_credential(email, generation, CredentialKind::OAuthRefresh, None);
            delete_account_credential(email, generation, CredentialKind::OAuthClientSecret, None);
        }
        Err(err) => tracing::warn!("could not resolve account credential generation: {err}"),
    }
}

fn delete_key(key: &str) {
    let Ok(entry) = keyring_entry(key) else {
        return;
    };
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(err) => tracing::warn!("could not delete MegaMail keyring entry: {err}"),
    }
}

fn read_key(key: &str) -> Result<Option<String>, String> {
    match keyring_entry(key).and_then(|entry| entry.get_password()) {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn load_key(key: &str) -> Option<String> {
    read_key(key).unwrap_or_else(|err| {
        tracing::warn!("could not read MegaMail keyring entry: {err}");
        None
    })
}

pub fn read_password(email: &str) -> Result<Option<String>, String> {
    read_account_credential(email, CredentialKind::Password, None).map_err(|err| err.to_string())
}

pub fn read_smtp_password(email: &str) -> Result<Option<String>, String> {
    read_account_credential(email, CredentialKind::SmtpPassword, None)
        .map_err(|err| err.to_string())
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct FetchModesFile {
    #[serde(default)]
    modes: std::collections::BTreeMap<String, String>,
}

fn fetch_modes_path() -> Option<PathBuf> {
    Some(config_base()?.join("fetch-modes.toml"))
}

fn load_fetch_modes() -> FetchModesFile {
    let Some(path) = fetch_modes_path() else {
        return FetchModesFile::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_fetch_modes(modes: &FetchModesFile) -> io::Result<()> {
    let path = fetch_modes_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))?;
    let text = toml::to_string_pretty(modes)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    write_private(&path, &text)
}

/// Remember whether this account's server accepts ENVELOPE or previews.
pub fn load_fetch_mode(email: &str) -> (bool, bool) {
    match load_fetch_modes()
        .modes
        .get(&email.to_ascii_lowercase())
        .map(String::as_str)
    {
        Some("headers") => (false, false),
        Some("no-previews") => (true, true),
        Some("headers-no-previews") => (false, true),
        _ => (true, false),
    }
}

pub fn save_fetch_mode(email: &str, use_envelope: bool, previews_rejected: bool) {
    let mode = match (use_envelope, previews_rejected) {
        (true, false) => None,
        (false, false) => Some("headers"),
        (true, true) => Some("no-previews"),
        (false, true) => Some("headers-no-previews"),
    };
    let key = email.to_ascii_lowercase();
    let mut file = load_fetch_modes();
    match mode {
        Some(mode) => {
            file.modes.insert(key, mode.to_string());
        }
        None => {
            file.modes.remove(&key);
        }
    }
    if let Err(err) = save_fetch_modes(&file) {
        tracing::warn!("could not save mail fetch mode: {err}");
    }
}

/// A message filter applied to an arriving message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterRule {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub account_email: String,
    pub field: FilterField,
    pub matcher: FilterMatch,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more: Vec<FilterCondition>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub any: bool,
    #[serde(default)]
    pub dest_path: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default = "count_unread_default")]
    pub count_unread: bool,
}

fn count_unread_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterCondition {
    pub field: FilterField,
    pub matcher: FilterMatch,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterField {
    FromAddress,
    FromName,
    Subject,
    Recipients,
    ReplyTo,
    Body,
}

impl FilterField {
    pub const ALL: [FilterField; 6] = [
        Self::FromAddress,
        Self::FromName,
        Self::Subject,
        Self::Recipients,
        Self::ReplyTo,
        Self::Body,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMatch {
    Contains,
    Equals,
    StartsWith,
    EndsWith,
}

impl FilterMatch {
    pub const ALL: [FilterMatch; 4] = [
        Self::Contains,
        Self::Equals,
        Self::StartsWith,
        Self::EndsWith,
    ];
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FilterInput<'a> {
    pub from_addr: &'a str,
    pub from_name: &'a str,
    pub subject: &'a str,
    pub recipients: &'a str,
    pub reply_to: &'a str,
    pub body: &'a str,
    pub body_hits: &'a [String],
}

impl FilterCondition {
    pub fn alternatives(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|part| part.trim().to_lowercase())
            .filter(|part| !part.is_empty())
            .collect()
    }

    pub fn matches(&self, input: &FilterInput<'_>) -> bool {
        let alternatives = Self::alternatives(&self.value);
        if alternatives.is_empty() {
            return false;
        }
        if self.field == FilterField::Body {
            let text = input.body.to_lowercase();
            return alternatives.iter().any(|needle| {
                input.body_hits.iter().any(|hit| hit == needle) || text.contains(needle.as_str())
            });
        }

        let haystack = match self.field {
            FilterField::FromAddress => input.from_addr,
            FilterField::FromName => input.from_name,
            FilterField::Subject => input.subject,
            FilterField::Recipients => input.recipients,
            FilterField::ReplyTo if input.reply_to.trim().is_empty() => input.from_addr,
            FilterField::ReplyTo => input.reply_to,
            FilterField::Body => unreachable!(),
        }
        .to_lowercase();
        let candidates =
            if self.field == FilterField::Recipients && self.matcher != FilterMatch::Contains {
                mailboxes(&haystack)
            } else {
                vec![haystack]
            };
        alternatives.iter().any(|needle| {
            candidates.iter().any(|candidate| match self.matcher {
                FilterMatch::Contains => candidate.contains(needle.as_str()),
                FilterMatch::Equals => candidate == needle,
                FilterMatch::StartsWith => candidate.starts_with(needle.as_str()),
                FilterMatch::EndsWith => candidate.ends_with(needle.as_str()),
            })
        })
    }
}

fn mailboxes(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in list
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match (part.rfind('<'), part.rfind('>')) {
            (Some(left), Some(right)) if left < right => {
                let name = part[..left].trim().trim_matches('"').trim();
                if !name.is_empty() {
                    out.push(name.to_string());
                }
                out.push(part[left + 1..right].trim().to_string());
            }
            _ => out.push(part.to_string()),
        }
    }
    out
}

impl FilterRule {
    pub fn conditions(&self) -> Vec<FilterCondition> {
        let mut conditions = Vec::with_capacity(1 + self.more.len());
        conditions.push(FilterCondition {
            field: self.field,
            matcher: self.matcher,
            value: self.value.clone(),
        });
        conditions.extend(self.more.iter().cloned());
        conditions
    }

    pub fn set_conditions(&mut self, mut conditions: Vec<FilterCondition>) -> bool {
        if conditions.is_empty() {
            return false;
        }
        let first = conditions.remove(0);
        self.field = first.field;
        self.matcher = first.matcher;
        self.value = first.value;
        self.more = conditions;
        true
    }

    pub fn label(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        self.conditions()
            .iter()
            .map(|condition| {
                format!(
                    "{:?} {:?} {}",
                    condition.field, condition.matcher, condition.value
                )
            })
            .collect::<Vec<_>>()
            .join(if self.any { " or " } else { " and " })
    }

    pub fn body_needles(&self) -> Vec<String> {
        self.conditions()
            .iter()
            .filter(|condition| condition.field == FilterField::Body)
            .flat_map(|condition| FilterCondition::alternatives(&condition.value))
            .collect()
    }

    pub fn matches(&self, input: &FilterInput<'_>) -> bool {
        let conditions = self.conditions();
        if self.any {
            conditions.iter().any(|condition| condition.matches(input))
        } else {
            conditions.iter().all(|condition| condition.matches(input))
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FiltersFile {
    #[serde(default)]
    rules: Vec<FilterRule>,
}

fn filters_path() -> Option<PathBuf> {
    Some(config_base()?.join("filters.toml"))
}

pub fn load_filters() -> Vec<FilterRule> {
    let Some(path) = filters_path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    toml::from_str::<FiltersFile>(&text)
        .map(|file| file.rules)
        .unwrap_or_default()
}

pub fn save_filters(rules: &[FilterRule]) {
    let Some(path) = filters_path() else { return };
    let text = match toml::to_string_pretty(&FiltersFile {
        rules: rules.to_vec(),
    }) {
        Ok(text) => text,
        Err(err) => {
            tracing::warn!("could not serialize mail filters: {err}");
            return;
        }
    };
    if let Err(err) = write_private(&path, &text) {
        tracing::warn!("could not save mail filters: {err}");
    }
}

/// Body search terms needed by this account's rules, freshly read and
/// lowercased/deduplicated for the next inbox synchronization.
pub fn filter_body_needles(email: &str) -> Vec<String> {
    let mut needles: Vec<String> = load_filters()
        .iter()
        .filter(|rule| rule.account_email.eq_ignore_ascii_case(email))
        .flat_map(FilterRule::body_needles)
        .collect();
    needles.sort();
    needles.dedup();
    needles
}

/// Persist the selected UI language in MegaMail's config directory. The GPUI
/// app can use this without bringing GTK/i18n code into the mail core.
pub fn load_language() -> String {
    config_base()
        .map(|base| base.join("language"))
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

pub fn save_language(language: &str) {
    let Some(base) = config_base() else { return };
    let path = base.join("language");
    if language.trim().is_empty() {
        if let Err(err) = fs::remove_file(path) {
            if err.kind() != io::ErrorKind::NotFound {
                tracing::warn!("could not clear MegaMail language preference: {err}");
            }
        }
    } else if let Err(err) = write_private(&path, &format!("{}\n", language.trim())) {
        tracing::warn!("could not save MegaMail language preference: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeCredentialBackend {
        entries: std::collections::BTreeMap<String, String>,
    }

    impl CredentialBackend for FakeCredentialBackend {
        fn read(
            &mut self,
            generation: Option<&str>,
            kind: CredentialKind,
            email: &str,
            alias_addr: Option<&str>,
        ) -> keyring::Result<Option<String>> {
            Ok(self
                .entries
                .get(&credential_key(generation, kind, email, alias_addr))
                .cloned())
        }

        fn store(
            &mut self,
            generation: &str,
            kind: CredentialKind,
            email: &str,
            alias_addr: Option<&str>,
            secret: &str,
        ) -> keyring::Result<()> {
            self.entries.insert(
                credential_key(Some(generation), kind, email, alias_addr),
                secret.to_string(),
            );
            Ok(())
        }
    }

    #[test]
    fn account_config_serialization_keeps_secrets_out_of_toml_and_debug() {
        let mut account = AccountConfig::new("person@example.test");
        account.password = "account-secret".into();
        account.smtp_separate = true;
        account.smtp_password = "smtp-secret".into();
        account.oauth_refresh = "refresh-secret".into();
        account.oauth_settings = Some(OAuthSettings {
            auth_url: "https://auth.example.test/authorize".into(),
            token_url: "https://auth.example.test/token".into(),
            client_id: "public-client".into(),
            client_secret: "client-secret".into(),
            scopes: "mail.read".into(),
            redirect_uri: String::new(),
        });
        account.aliases.push(AliasConfig {
            identity: "Person <alias@example.test>".into(),
            smtp_host: "smtp.example.test".into(),
            smtp_port: 587,
            smtp_username: "alias-user".into(),
            smtp_password: "alias-secret".into(),
        });

        let encoded = toml::to_string(&ConfigFile {
            accounts: vec![account.clone()],
            ..ConfigFile::default()
        })
        .unwrap();
        for secret in [
            "account-secret",
            "smtp-secret",
            "refresh-secret",
            "client-secret",
            "alias-secret",
        ] {
            assert!(
                !encoded.contains(secret),
                "serialized account config leaked {secret}"
            );
        }
        let debug = format!("{account:?}");
        for secret in [
            "account-secret",
            "smtp-secret",
            "refresh-secret",
            "client-secret",
            "alias-secret",
        ] {
            assert!(!debug.contains(secret), "Debug output leaked {secret}");
        }

        let decoded: ConfigFile = toml::from_str(&encoded).unwrap();
        let loaded = &decoded.accounts[0];
        assert_eq!(loaded.email, "person@example.test");
        assert!(loaded.password.is_empty());
        assert!(loaded.smtp_password.is_empty());
        assert!(loaded.oauth_refresh.is_empty());
        assert!(loaded
            .oauth_settings
            .as_ref()
            .unwrap()
            .client_secret
            .is_empty());
        assert!(loaded.aliases[0].smtp_password.is_empty());
    }

    #[test]
    fn staging_credentials_keeps_the_active_generation_unchanged() {
        let email = "person@example.test";
        let old_key = credential_key(
            Some("old-generation"),
            CredentialKind::Password,
            email,
            None,
        );
        let new_key = credential_key(
            Some("new-generation"),
            CredentialKind::Password,
            email,
            None,
        );
        let mut backend = FakeCredentialBackend::default();
        backend
            .entries
            .insert(old_key.clone(), "original-secret".into());

        stage_credential_with_backend(
            &mut backend,
            email,
            email,
            Some("old-generation"),
            "new-generation",
            CredentialKind::Password,
            None,
            "",
            None,
        )
        .unwrap();

        assert_eq!(backend.entries.get(&old_key).unwrap(), "original-secret");
        assert_eq!(backend.entries.get(&new_key).unwrap(), "original-secret");
    }

    #[test]
    fn private_file_writer_sets_private_modes() {
        let root = std::env::temp_dir().join(format!(
            "megamail-config-test-{}-{}",
            std::process::id(),
            TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        ensure_private_dir(&root).unwrap();
        let path = root.join("settings.toml");
        write_private_file(&path, "safe = true\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "safe = true\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn profile_ids_survive_reordering_removal_and_readdition() {
        let mut first = AccountConfig::new("same@example.test");
        first.imap_host = "imap.one.example.test".into();
        first.enabled = false;
        let mut second = AccountConfig::new("same@example.test");
        second.imap_host = "imap.two.example.test".into();
        let mut ids = std::collections::BTreeMap::new();

        let (initial, changed) =
            assign_profile_ids(vec![first.clone(), second.clone()], &mut ids).unwrap();
        assert!(changed);
        assert_eq!((initial[0].0, initial[1].0), (1, 2));
        assert!(
            !initial[0].1.enabled,
            "disabled profiles still receive stable IDs"
        );

        let (reordered, changed) =
            assign_profile_ids(vec![second.clone(), first.clone()], &mut ids).unwrap();
        assert!(!changed);
        assert_eq!((reordered[0].0, reordered[1].0), (2, 1));

        let (removed, changed) = assign_profile_ids(vec![first.clone()], &mut ids).unwrap();
        assert!(!changed);
        assert_eq!(removed[0].0, 1);
        assert_eq!(
            ids.len(),
            2,
            "removed profiles keep their historical mapping"
        );

        let (readded, changed) = assign_profile_ids(vec![second.clone()], &mut ids).unwrap();
        assert!(!changed);
        assert_eq!(readded[0].0, 2);

        let mut third = second.clone();
        third.imap_host = "imap.three.example.test".into();
        let (distinct, changed) = assign_profile_ids(vec![third.clone()], &mut ids).unwrap();
        assert!(changed);
        assert_eq!(
            distinct[0].0, 3,
            "same address on a different host gets a fresh ID"
        );

        let mut alternate_port = third;
        alternate_port.imap_port = 1143;
        let (port_scoped, changed) = assign_profile_ids(vec![alternate_port], &mut ids).unwrap();
        assert!(changed);
        assert_eq!(
            port_scoped[0].0, 4,
            "same host on a different port gets a fresh ID"
        );
    }

    #[test]
    fn invalid_persisted_profile_ids_are_rejected() {
        let ids =
            std::collections::BTreeMap::from([("first".to_string(), 4), ("second".to_string(), 4)]);
        assert_eq!(
            validate_profile_ids(&ids).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let ids = std::collections::BTreeMap::from([("zero".to_string(), 0)]);
        assert_eq!(
            validate_profile_ids(&ids).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn duplicate_emails_are_rejected_before_credentials_can_be_written() {
        let mut first = AccountConfig::new("same@example.test");
        first.password = "first-secret".into();
        let mut second = AccountConfig::new("SAME@example.test");
        second.imap_host = "another.example.test".into();
        second.password = "second-secret".into();

        let error = save(&[first, second]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
