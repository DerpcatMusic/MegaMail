//! What the app has to ask the desktop itself, over GTK's head.
//!
//! Its settings (GNOME's `org.gnome.desktop.interface` and kin), which the
//! locale and GTK know nothing about: the clock format (#173), the
//! monospace font (#181). Read through the settings portal, which works
//! inside the Flatpak sandbox and on the host alike, or — outside the
//! sandbox, when no portal answers — straight from GSettings where the
//! schema is installed. Whether it wants quiet, for a new-mail sound (#292).
//!
//! And its window manager, when the window has to come to the front and
//! `present` is not enough to get it there belongs to the legacy GTK helper.
//! This portable module also validates untrusted external compose links without
//! reading settings, opening files, choosing a sender, or dispatching mail.

/// The string value of `key` in `namespace`, if the desktop has one.
pub fn setting(namespace: &str, key: &str) -> Option<String> {
    fn from_value(v: zbus::zvariant::Value<'_>) -> Option<String> {
        match v {
            zbus::zvariant::Value::Value(inner) => from_value(*inner),
            zbus::zvariant::Value::Str(s) => Some(s.as_str().to_string()),
            _ => None,
        }
    }
    let portal = portal_setting(namespace, key).and_then(|v| from_value(v.into()));
    if portal.is_some() || std::env::var_os("FLATPAK_ID").is_some() {
        return portal;
    }
    use gio::prelude::SettingsExt;
    let settings = host_settings(namespace)?;
    Some(settings.string(key).as_str().to_string())
}

/// The boolean value of `key` in `namespace`, if the desktop has one.
pub fn flag(namespace: &str, key: &str) -> Option<bool> {
    fn from_value(v: zbus::zvariant::Value<'_>) -> Option<bool> {
        match v {
            zbus::zvariant::Value::Value(inner) => from_value(*inner),
            zbus::zvariant::Value::Bool(b) => Some(b),
            _ => None,
        }
    }
    let portal = portal_setting(namespace, key).and_then(|v| from_value(v.into()));
    if portal.is_some() || std::env::var_os("FLATPAK_ID").is_some() {
        return portal;
    }
    use gio::prelude::SettingsExt;
    let settings = host_settings(namespace)?;
    Some(settings.boolean(key))
}

fn portal_setting(namespace: &str, key: &str) -> Option<zbus::zvariant::OwnedValue> {
    let conn = zbus::blocking::Connection::session().ok()?;
    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &(namespace, key),
        )
        .ok()?;
    reply.body().deserialize().ok()
}

/// GSettings for `namespace`, where its schema is installed: looking a
/// missing schema up is fatal to GLib, so it is asked for first.
fn host_settings(namespace: &str) -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(namespace, true)?;
    Some(gio::Settings::new(namespace))
}

/// Maximum encoded external compose URI size, before decoding or allocation.
pub const MAILTO_URI_LIMIT: usize = 64 * 1024;
/// Combined To/Cc/Bcc occurrences. Duplicates still count toward this limit.
pub const MAILTO_RECIPIENT_LIMIT: usize = 100;
/// Maximum decoded subject size in bytes; the MIME writer still owns folding.
pub const MAILTO_SUBJECT_LIMIT: usize = 512;
/// Maximum decoded plain-text body size in bytes.
pub const MAILTO_BODY_LIMIT: usize = 48 * 1024;
const MAILTO_FIELD_LIMIT: usize = 32;

/// Validated compose prefill, not a send request or an account-selection hint.
/// No attachments, arbitrary headers, HTML, sender identity, or scheduling.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailtoIntent {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
}

/// Errors intentionally contain no URI/mail data: safe for user-facing notices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailtoError {
    WrongScheme,
    TooLarge,
    InvalidEncoding,
    InvalidSyntax,
    UnsafeHeader,
    InvalidRecipient,
    TooManyRecipients,
    TooManyFields,
    UnsupportedField,
    DuplicateField,
    SubjectTooLarge,
    BodyTooLarge,
    UnsafeBody,
}

impl std::fmt::Display for MailtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::WrongScheme => "Only mailto compose links are supported.",
            Self::TooLarge => "This compose link exceeds the 64 KiB limit.",
            Self::InvalidEncoding => "This compose link has invalid percent encoding or UTF-8.",
            Self::InvalidSyntax => "This compose link has unsupported URI syntax.",
            Self::UnsafeHeader => "This compose link contains unsafe header characters.",
            Self::InvalidRecipient => "Use plain ASCII email addresses in this compose link (no display names or groups).",
            Self::TooManyRecipients => "This compose link exceeds 100 total To, Cc, and Bcc recipients.",
            Self::TooManyFields => "This compose link exceeds 32 query fields.",
            Self::UnsupportedField => "This compose link requests an unsupported field. Only To, Cc, Bcc, Subject, and Body are allowed.",
            Self::DuplicateField => "This compose link repeats Subject or Body.",
            Self::SubjectTooLarge => "This compose link's subject exceeds 512 bytes.",
            Self::BodyTooLarge => "This compose link's body exceeds 48 KiB.",
            Self::UnsafeBody => "This compose link's body contains unsupported control characters.",
        })
    }
}

impl std::error::Error for MailtoError {}

/// Validate a conservative RFC 6068 subset. Decode each component exactly once;
/// `+` stays literal, unlike form encoding. Reject unknown/unsafe headers rather
/// than silently changing the requested message. Empty recipients are allowed
/// for an incomplete draft; the normal send preflight must still validate it.
///
/// Address support is intentionally ASCII dot-atom + DNS/ACE domain only, not
/// all RFC 5322 syntax or SMTPUTF8. Unicode subject/body text is supported.
/// Body CRLF becomes LF for the native plain-text editor. This function does
/// not access the desktop, filesystem, profile, credentials, or network.
pub fn parse_mailto(uri: &str) -> Result<MailtoIntent, MailtoError> {
    if uri.len() > MAILTO_URI_LIMIT {
        return Err(MailtoError::TooLarge);
    }
    let (scheme, rest) = uri.split_once(':').ok_or(MailtoError::WrongScheme)?;
    if !scheme.eq_ignore_ascii_case("mailto") {
        return Err(MailtoError::WrongScheme);
    }
    if rest.starts_with('/') || rest.contains('#') || uri.chars().any(char::is_control) {
        return Err(MailtoError::InvalidSyntax);
    }
    let (addresses, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut intent = MailtoIntent::default();
    let mut recipients = 0;
    append_recipients(&mut intent.to, &decode_mailto(addresses)?, &mut recipients)?;
    let mut has_subject = false;
    let mut has_body = false;
    if !query.is_empty() {
        for (index, field) in query.split('&').enumerate() {
            if index >= MAILTO_FIELD_LIMIT {
                return Err(MailtoError::TooManyFields);
            }
            let (name, value) = field.split_once('=').ok_or(MailtoError::InvalidSyntax)?;
            let name = decode_mailto(name)?;
            validate_mailto_header(&name)?;
            // Header names must be ASCII; encoded names get the same allowlist.
            let value = decode_mailto(value)?;
            match name.to_ascii_lowercase().as_str() {
                "to" => append_recipients(&mut intent.to, &value, &mut recipients)?,
                "cc" => append_recipients(&mut intent.cc, &value, &mut recipients)?,
                "bcc" => append_recipients(&mut intent.bcc, &value, &mut recipients)?,
                "subject" => {
                    if has_subject {
                        return Err(MailtoError::DuplicateField);
                    }
                    has_subject = true;
                    validate_mailto_header(&value)?;
                    if value.len() > MAILTO_SUBJECT_LIMIT {
                        return Err(MailtoError::SubjectTooLarge);
                    }
                    intent.subject = value;
                }
                "body" => {
                    if has_body {
                        return Err(MailtoError::DuplicateField);
                    }
                    has_body = true;
                    if value.len() > MAILTO_BODY_LIMIT {
                        return Err(MailtoError::BodyTooLarge);
                    }
                    let body = value.replace("\r\n", "\n");
                    if body
                        .chars()
                        .any(|c| c.is_control() && c != '\n' && c != '\t')
                    {
                        return Err(MailtoError::UnsafeBody);
                    }
                    intent.body = body;
                }
                _ => return Err(MailtoError::UnsupportedField),
            }
        }
    }
    Ok(intent)
}

fn decode_mailto(value: &str) -> Result<String, MailtoError> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).copied().and_then(hex);
            let low = bytes.get(index + 2).copied().and_then(hex);
            let (Some(high), Some(low)) = (high, low) else {
                return Err(MailtoError::InvalidEncoding);
            };
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| MailtoError::InvalidEncoding)
}

fn validate_mailto_header(value: &str) -> Result<(), MailtoError> {
    // Unicode separators and direction overrides can hide a header/recipient.
    if value.chars().any(|c| {
        c.is_control()
            || matches!(c, '\u{2028}' | '\u{2029}' | '\u{061c}' | '\u{200e}' | '\u{200f}'
                | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    }) {
        return Err(MailtoError::UnsafeHeader);
    }
    Ok(())
}

fn append_recipients(
    target: &mut Vec<String>,
    value: &str,
    total: &mut usize,
) -> Result<(), MailtoError> {
    validate_mailto_header(value)?;
    if value.is_empty() {
        return Ok(());
    }
    for address in value.split(',') {
        if *total >= MAILTO_RECIPIENT_LIMIT {
            return Err(MailtoError::TooManyRecipients);
        }
        let address = address.trim_matches(' ');
        if !valid_mailto_address(address) {
            return Err(MailtoError::InvalidRecipient);
        }
        *total += 1;
        target.push(address.to_owned());
    }
    Ok(())
}

fn valid_mailto_address(address: &str) -> bool {
    if !address.is_ascii() || address.len() > 254 {
        return false;
    }
    let Some((local, domain)) = address.split_once('@') else {
        return false;
    };
    if local.is_empty() || local.len() > 64 || domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let atom = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~".contains(&b);
    local
        .split('.')
        .all(|part| !part.is_empty() && part.bytes().all(atom))
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

#[cfg(test)]
mod mailto_tests {
    use super::*;

    #[test]
    fn parses_plain_compose_fields_without_form_decoding() {
        let intent = parse_mailto("MAILTO:a%2Btag@example.test?to=b@example.test&CC=c@example.test&Bcc=d@example.test&subject=Hi+%26+Bye&body=one%0D%0Atwo%0A%3Cscript%3E").unwrap();
        assert_eq!(
            intent,
            MailtoIntent {
                to: vec!["a+tag@example.test".into(), "b@example.test".into()],
                cc: vec!["c@example.test".into()],
                bcc: vec!["d@example.test".into()],
                subject: "Hi+&+Bye".into(),
                body: "one\ntwo\n<script>".into(),
            }
        );
    }

    #[test]
    fn permits_incomplete_and_unicode_drafts() {
        assert_eq!(
            parse_mailto("mailto:?subject=שלום&body=مرحبا")
                .unwrap()
                .subject,
            "שלום"
        );
        assert_eq!(parse_mailto("mailto:").unwrap(), MailtoIntent::default());
        assert!(
            parse_mailto("mailto:?bcc=hidden@example.test")
                .unwrap()
                .to
                .is_empty()
        );
    }

    #[test]
    fn rejects_header_injection_even_in_encoded_names_and_recipients() {
        for uri in [
            "mailto:a@example.test%0d%0aBcc:b@example.test",
            "mailto:a@example.test?subject=x%0ABcc%3Ax@example.test",
            "mailto:a@example.test?cc=b@example.test%0D",
            "mailto:?bcc=a@example.test%00",
            "mailto:?subject=%09x",
            "mailto:?subject=%E2%80%A8x",
            "mailto:?subject=x%E2%80%AE",
            "mailto:?sub%0Aject=x",
            "mailto:?%0Dfrom=x",
        ] {
            assert_eq!(parse_mailto(uri), Err(MailtoError::UnsafeHeader), "{uri}");
        }
    }

    #[test]
    fn rejects_malformed_encoding_instead_of_lossy_or_recursive_decoding() {
        for uri in [
            "mailto:a%",
            "mailto:a%2",
            "mailto:a%zz",
            "mailto:?body=%ff",
            "mailto:?subject=%c0%8a",
        ] {
            assert_eq!(
                parse_mailto(uri),
                Err(MailtoError::InvalidEncoding),
                "{uri}"
            );
        }
        assert_eq!(
            parse_mailto("mailto:?subject=%250a").unwrap().subject,
            "%0a"
        );
    }

    #[test]
    fn rejects_unsupported_fields_and_duplicate_singletons() {
        for field in [
            "from",
            "reply-to",
            "attach",
            "attachment",
            "in-reply-to",
            "send",
            "html",
            "x-header",
            "%66rom",
        ] {
            assert_eq!(
                parse_mailto(&format!("mailto:?{field}=x")),
                Err(MailtoError::UnsupportedField)
            );
        }
        for query in ["subject=x&Subject=y", "body=&BODY=y"] {
            assert_eq!(
                parse_mailto(&format!("mailto:?{query}")),
                Err(MailtoError::DuplicateField)
            );
        }
    }

    #[test]
    fn rejects_non_mailto_authority_fragments_and_bad_query_syntax() {
        assert_eq!(
            parse_mailto("https://example.test"),
            Err(MailtoError::WrongScheme)
        );
        for uri in [
            "mailto://a@example.test",
            "mailto:a@example.test#fragment",
            "mailto:?body=hi\nthere",
            "mailto:?subject",
            "mailto:?subject=x&",
        ] {
            assert_eq!(parse_mailto(uri), Err(MailtoError::InvalidSyntax), "{uri}");
        }
    }

    #[test]
    fn rejects_display_names_groups_and_invalid_dot_atom_domains() {
        for address in [
            "Name%20%3Ca@example.test%3E",
            "a@example.test,",
            "a@@example.test",
            "a@-bad.test",
            "a@bad-.test",
            "a@bad..test",
            "a..b@example.test",
            ".a@example.test",
            "a.@example.test",
            "group:a@example.test;",
            "a@[127.0.0.1]",
            "用户@example.test",
            "a@é.test",
            "%22a%22@example.test",
        ] {
            assert_eq!(
                parse_mailto(&format!("mailto:{address}")),
                Err(MailtoError::InvalidRecipient),
                "{address}"
            );
        }
        // A percent sign is legal in an ASCII local part. Do not decode it a
        // second time in the composer/MIME pipeline into a delimiter.
        assert_eq!(
            parse_mailto("mailto:a%252c@example.test").unwrap().to,
            vec!["a%2c@example.test"]
        );
    }

    #[test]
    fn bounds_combined_recipient_occurrences_and_repeated_fields() {
        let list = vec!["a@example.test"; MAILTO_RECIPIENT_LIMIT].join(",");
        assert_eq!(
            parse_mailto(&format!("mailto:{list}")).unwrap().to.len(),
            MAILTO_RECIPIENT_LIMIT
        );
        assert_eq!(
            parse_mailto(&format!("mailto:{list}?bcc=a@example.test")),
            Err(MailtoError::TooManyRecipients)
        );
        assert_eq!(
            parse_mailto(&format!(
                "mailto:?{}",
                vec!["cc="; MAILTO_FIELD_LIMIT].join("&")
            ))
            .unwrap(),
            MailtoIntent::default()
        );
        assert_eq!(
            parse_mailto(&format!(
                "mailto:?{}",
                vec!["cc="; MAILTO_FIELD_LIMIT + 1].join("&")
            )),
            Err(MailtoError::TooManyFields)
        );
    }

    #[test]
    fn enforces_uri_subject_body_and_address_size_boundaries() {
        assert_eq!(
            parse_mailto(&format!("mailto:{}", "a".repeat(MAILTO_URI_LIMIT))),
            Err(MailtoError::TooLarge)
        );
        assert!(
            parse_mailto(&format!(
                "mailto:?subject={}",
                "a".repeat(MAILTO_SUBJECT_LIMIT)
            ))
            .is_ok()
        );
        assert_eq!(
            parse_mailto(&format!(
                "mailto:?subject={}",
                "a".repeat(MAILTO_SUBJECT_LIMIT + 1)
            )),
            Err(MailtoError::SubjectTooLarge)
        );
        assert!(parse_mailto(&format!("mailto:?body={}", "a".repeat(MAILTO_BODY_LIMIT))).is_ok());
        assert_eq!(
            parse_mailto(&format!(
                "mailto:?body={}",
                "a".repeat(MAILTO_BODY_LIMIT + 1)
            )),
            Err(MailtoError::BodyTooLarge)
        );
        assert!(parse_mailto(&format!("mailto:{}@example.test", "a".repeat(64))).is_ok());
        assert_eq!(
            parse_mailto(&format!("mailto:{}@example.test", "a".repeat(65))),
            Err(MailtoError::InvalidRecipient)
        );
    }

    #[test]
    fn rejects_body_controls_but_preserves_literal_plain_text() {
        for value in ["%00", "%01", "%7f", "%0d"] {
            assert_eq!(
                parse_mailto(&format!("mailto:?body={value}")),
                Err(MailtoError::UnsafeBody)
            );
        }
        assert_eq!(
            parse_mailto("mailto:?body=%09%3Cb%3Etext%3C%2Fb%3E")
                .unwrap()
                .body,
            "\t<b>text</b>"
        );
    }
}
