//! One-time read of Thunderbird's saved logins (NSS `key4.db` + `logins.json`).
//!
//! Only profiles without a Primary Password are supported: NSS then encrypts
//! with an empty password, so no secret needs to be typed. Values are handed
//! straight to the keyring by the caller and never logged.
use std::path::Path;

use openssl::hash::{hash, MessageDigest};
use openssl::symm::{decrypt, Cipher};

/// One saved Thunderbird login, e.g. origin `imap://imap.example.com`.
pub struct Login {
    pub origin: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Login").field("origin", &self.origin).finish_non_exhaustive()
    }
}

/// Minimal DER reader: just enough for NSS's PBES2 and login blobs.
#[derive(Debug)]
enum Der<'a> {
    Seq(Vec<Der<'a>>),
    Bytes(u8, &'a [u8]),
}

fn der(input: &[u8]) -> Option<(Der<'_>, &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, mut rest) = rest.split_first()?;
    let len = if first < 0x80 {
        first as usize
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return None;
        }
        let len = rest[..n].iter().fold(0usize, |a, &b| a << 8 | b as usize);
        rest = &rest[n..];
        len
    };
    if rest.len() < len {
        return None;
    }
    let (body, tail) = rest.split_at(len);
    if tag == 0x30 {
        let mut items = Vec::new();
        let mut inner = body;
        while !inner.is_empty() {
            let (item, next) = der(inner)?;
            items.push(item);
            inner = next;
        }
        Some((Der::Seq(items), tail))
    } else {
        Some((Der::Bytes(tag, body), tail))
    }
}

impl<'a> Der<'a> {
    fn seq(&self) -> Option<&[Der<'a>]> {
        match self {
            Der::Seq(items) => Some(items),
            _ => None,
        }
    }
    fn bytes(&self) -> Option<&'a [u8]> {
        match self {
            Der::Bytes(_, b) => Some(b),
            _ => None,
        }
    }
    fn int(&self) -> Option<usize> {
        let b = self.bytes()?;
        (b.len() <= 4).then(|| b.iter().fold(0usize, |a, &x| a << 8 | x as usize))
    }
}

const OID_PBES2: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x05, 0x0d];
const OID_DES3_CBC: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x03, 0x07];
const OID_AES256_CBC: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x01, 0x2a];

/// Decrypt a key4.db PBES2 (PBKDF2-SHA256 + AES-256-CBC) blob with the
/// empty Primary Password.
fn pbes2_decrypt(global_salt: &[u8], blob: &[u8]) -> Option<Vec<u8>> {
    let (root, _) = der(blob)?;
    let root = root.seq()?;
    let algorithm = root.first()?.seq()?;
    if algorithm.first()?.bytes()? != OID_PBES2 {
        // Pre-2019 profiles use SHA1+3DES PBE; not worth carrying for a one-off import.
        return None;
    }
    let params = algorithm.get(1)?.seq()?;
    let kdf = params.first()?.seq()?.get(1)?.seq()?;
    let entry_salt = kdf.first()?.bytes()?;
    let iterations = kdf.get(1)?.int()?;
    let key_len = kdf.get(2).and_then(Der::int).unwrap_or(32);
    let iv_tail = params.get(1)?.seq()?.get(1)?.bytes()?;
    let ciphertext = root.get(1)?.bytes()?;

    let password = hash(MessageDigest::sha1(), global_salt).ok()?;
    let mut key = vec![0u8; key_len];
    openssl::pkcs5::pbkdf2_hmac(&password, entry_salt, iterations, MessageDigest::sha256(), &mut key).ok()?;
    // NSS stores 14 bytes of the IV; the DER OCTET STRING header (04 0e) completes it.
    let mut iv = vec![0x04, 0x0e];
    iv.extend_from_slice(iv_tail);
    decrypt(Cipher::aes_256_cbc(), &key, Some(&iv), ciphertext).ok()
}

/// The 3DES or AES key that encrypts logins.json values.
fn master_key(profile: &Path) -> Result<Vec<u8>, String> {
    let db = rusqlite::Connection::open_with_flags(
        profile.join("key4.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Could not open Thunderbird's key database: {e}"))?;
    let (salt, check): (Vec<u8>, Vec<u8>) = db
        .query_row("SELECT item1, item2 FROM metaData WHERE id = 'password'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| format!("Thunderbird's key database is unreadable: {e}"))?;
    if pbes2_decrypt(&salt, &check).as_deref() != Some(b"password-check") {
        return Err("Thunderbird has a Primary Password set. Remove it in Thunderbird \
                    (Settings → Privacy & Security), then import again."
            .into());
    }
    let mut statement = db
        .prepare("SELECT a11 FROM nssPrivate")
        .map_err(|e| format!("Thunderbird's key database is unreadable: {e}"))?;
    let rows: Vec<Vec<u8>> = statement
        .query_map([], |r| r.get(0))
        .and_then(|rows| rows.collect())
        .map_err(|e| format!("Thunderbird's key database is unreadable: {e}"))?;
    // There is normally a single private key; take the first that decrypts.
    rows.into_iter()
        .find_map(|a11| pbes2_decrypt(&salt, &a11))
        .ok_or_else(|| "Thunderbird's saved-password key could not be decrypted.".into())
}

fn decrypt_value(key: &[u8], value: &str) -> Option<String> {
    use base64::Engine as _;
    let blob = base64::engine::general_purpose::STANDARD.decode(value).ok()?;
    let (root, _) = der(&blob)?;
    let root = root.seq()?;
    let algorithm = root.get(1)?.seq()?;
    let iv = algorithm.get(1)?.bytes()?;
    let ciphertext = root.get(2)?.bytes()?;
    let plain = match algorithm.first()?.bytes()? {
        oid if oid == OID_DES3_CBC => decrypt(Cipher::des_ede3_cbc(), key.get(..24)?, Some(iv), ciphertext),
        oid if oid == OID_AES256_CBC => decrypt(Cipher::aes_256_cbc(), key.get(..32)?, Some(iv), ciphertext),
        _ => return None,
    }
    .ok()?;
    String::from_utf8(plain).ok()
}

/// Every saved login in a Thunderbird profile, decrypted.
pub fn read_logins(profile: &Path) -> Result<Vec<Login>, String> {
    let text = match std::fs::read_to_string(profile.join("logins.json")) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Could not read Thunderbird's saved logins: {e}")),
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "Thunderbird's saved logins are malformed.")?;
    let entries = json["logins"].as_array().cloned().unwrap_or_default();
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let key = master_key(profile)?;
    Ok(entries
        .iter()
        .filter_map(|e| {
            Some(Login {
                origin: e["hostname"].as_str()?.to_owned(),
                username: decrypt_value(&key, e["encryptedUsername"].as_str()?)?,
                password: decrypt_value(&key, e["encryptedPassword"].as_str()?)?,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_reads_nested_sequences_and_long_lengths() {
        let mut long = vec![0x04, 0x81, 0x80];
        long.extend([7u8; 0x80]);
        let mut blob = vec![0x30, 0x81, (long.len() + 3) as u8, 0x02, 0x01, 0x05];
        blob.extend(&long);
        let (root, rest) = der(&blob).unwrap();
        assert!(rest.is_empty());
        let items = root.seq().unwrap();
        assert_eq!(items[0].int(), Some(5));
        assert_eq!(items[1].bytes().unwrap().len(), 0x80);
        assert!(der(&[0x30, 0x05, 0x01]).is_none());
    }

    #[test]
    fn decrypts_des3_login_value() {
        use base64::Engine as _;
        let key = [0x11u8; 24];
        let iv = [0x22u8; 8];
        let ct = openssl::symm::encrypt(Cipher::des_ede3_cbc(), &key, Some(&iv), b"hunter2").unwrap();
        let mut alg = vec![0x06, OID_DES3_CBC.len() as u8];
        alg.extend(OID_DES3_CBC);
        alg.extend([0x04, 8]);
        alg.extend(iv);
        let mut body = vec![0x04, 16];
        body.extend([0u8; 16]);
        body.extend([0x30, alg.len() as u8]);
        body.extend(&alg);
        body.extend([0x04, ct.len() as u8]);
        body.extend(&ct);
        let mut blob = vec![0x30, body.len() as u8];
        blob.extend(body);
        let encoded = base64::engine::general_purpose::STANDARD.encode(blob);
        assert_eq!(decrypt_value(&key, &encoded).as_deref(), Some("hunter2"));
    }

    /// `MEGAMAIL_TEST_TB_PROFILE=/path cargo test real_profile -- --ignored --nocapture`
    #[test]
    #[ignore = "reads a real Thunderbird profile"]
    fn real_profile_logins_decrypt() {
        let profile = std::env::var("MEGAMAIL_TEST_TB_PROFILE").unwrap();
        let logins = read_logins(Path::new(&profile)).unwrap();
        for login in &logins {
            println!("{} user={} pw_len={}", login.origin, !login.username.is_empty(), login.password.len());
        }
        assert!(!logins.is_empty());
    }
}
