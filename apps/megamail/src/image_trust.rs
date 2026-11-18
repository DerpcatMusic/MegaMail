//! Senders whose remote images load even when their mail fails sender checks
//! (e.g. a shop whose bulk mailer breaks DMARC). Lowercase addresses in
//! `image-senders.v1.json`; Junk/Spam still never loads remote content.

use std::collections::HashSet;

const FILE: &str = "image-senders.v1.json";
const MAX_BYTES: u64 = 512 * 1024;

pub(crate) fn load() -> HashSet<String> {
    use std::io::Read as _;
    let Some(path) = megamail_core::config::config_base().map(|dir| dir.join(FILE)) else {
        return HashSet::new();
    };
    let mut bytes = Vec::new();
    match std::fs::File::open(path).and_then(|file| file.take(MAX_BYTES).read_to_end(&mut bytes)) {
        Ok(_) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => HashSet::new(),
    }
}

pub(crate) fn save(senders: &HashSet<String>) -> Result<(), String> {
    let dir = megamail_core::config::config_base().ok_or("Settings storage is unavailable.")?;
    let json = serde_json::to_string(senders).map_err(|_| "Could not encode trusted image senders.")?;
    megamail_core::config::write_private_file(&dir.join(FILE), &json)
        .map_err(|_| "Could not save trusted image senders.".into())
}
