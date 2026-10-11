//! Read-only invitation presentation over the existing core MIME/invite helper.
//!
//! Feed actual `SenderCheck::invite` metadata, never HTML/text-body heuristics.
//! This module performs no I/O, URL opening, recurrence expansion or RSVP send.
//! Parsing belongs on the worker lane. Native callers must retain message/session
//! identity and the encrypted-body cache policy of the source document.

use crate::contacts::ContactScope;
use megamail_core::models::{Invite, SenderCheck};
use std::collections::BTreeMap;

const MAX_ICS_BYTES: usize = 256 * 1024;
const MAX_LINES: usize = 4096;
const MAX_LINE_BYTES: usize = 8192;

/// Never identify an invitation by UID alone, especially across accounts or epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvitationMessageKey {
    pub scope: ContactScope,
    pub folder_id: u32,
    pub message_id: u32,
    pub session_epoch: u64,
    pub body_generation: u64,
}

/// Scheduling state is distinct from a request to send an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationMethod {
    Request,
    Reply,
    Cancel,
    Publish,
    Counter,
    Other(String),
}

/// Preserve the sender's clock semantics instead of inventing the machine's timezone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationZone {
    DateOnly,
    Utc,
    Floating,
    Named(String),
}

/// A validated calendar date/date-time as written. All-day DTEND is exclusive.
/// This is display data, not a materialized scheduled event or a recurrence instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationStamp {
    pub value: String,
    pub zone: InvitationZone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Participation {
    NeedsAction,
    Accepted,
    Tentative,
    Declined,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationPerson {
    pub name: String,
    /// None for unsupported/invalid addresses; retain the person as read-only metadata.
    pub email: Option<String>,
    pub status: Participation,
    pub requested_response: bool,
    pub resource: bool,
}

/// Only explicit HTTP(S) destinations passing the core browser policy are actionable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationLink {
    pub destination: String,
}

/// A capability reason, not an action button that silently sends an acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RsvpCapability {
    NotRequested,
    Cancelled,
    /// Native RSVP command transport and confirmed durable send result are not wired.
    CommandTransportUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationWarning {
    MultipleEventsFirstOnly,
    MissingStart,
    FloatingTimeNoZone,
    NamedZoneResolutionUnavailable,
    RecurrenceNotExpanded,
    InvalidAddress,
    UnsafeUrlOmitted,
    UnknownMethod,
    UnknownParticipation,
    AttendeesTruncated,
    // Helper timestamps are not used to invent the exclusive end of an all-day event.
    AllDayEndExclusive,
    DurationNotMaterialized,
}

/// Bounded read-only data. No raw HTML or retained ICS, and no cache eligibility override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationPreview {
    pub message: InvitationMessageKey,
    pub uid: String,
    pub sequence: i64,
    pub method: InvitationMethod,
    pub cancelled: bool,
    pub summary: String,
    pub location: String,
    pub start: Option<InvitationStamp>,
    pub end: Option<InvitationStamp>,
    pub occurrence: Option<InvitationStamp>,
    pub recurrence_rule: Option<String>,
    pub organizer: Option<InvitationPerson>,
    pub attendees: Vec<InvitationPerson>,
    pub links: Vec<InvitationLink>,
    pub warnings: Vec<InvitationWarning>,
    pub rsvp: RsvpCapability,
}

/// Safe reasons without sender content in diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationError {
    TooLarge,
    Malformed,
    UnsupportedDate,
    MissingEvent,
}
impl std::fmt::Display for InvitationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "Invitation exceeds the preview limit",
            Self::Malformed => "Invitation data is malformed",
            Self::UnsupportedDate => "Invitation has an invalid or unsupported date",
            Self::MissingEvent => "Calendar contains no event to preview",
        })
    }
}
impl std::error::Error for InvitationError {}

/// Production direct-account seam: consume the metadata emitted alongside the body.
/// Missing metadata means no preview, not permission to search the HTML for a date.
pub fn from_sender_check(
    message: InvitationMessageKey,
    check: &SenderCheck,
) -> Result<Option<InvitationPreview>, InvitationError> {
    check
        .invite
        .as_deref()
        .map(|invite| from_metadata(message, invite))
        .transpose()
}

/// Also usable by the Thunderbird adapter once it supplies the same actual MIME-derived
/// metadata. Revalidate the preserved calendar and re-use core parsing, with only the
/// helper's MIME Content-Type method fallback. Never manufacture an Invite from a body.
pub fn from_metadata(
    message: InvitationMessageKey,
    metadata: &Invite,
) -> Result<InvitationPreview, InvitationError> {
    preview_calendar(message, &metadata.ics, &metadata.method)
}

/// Explicit local decoded calendar-part preview. This is not MIME detection. The MIME
/// owner may call `validate_calendar` before invoking the shared core helper; afterwards
/// use `from_metadata`. No URL or attachment path is read here.
pub fn preview_calendar(
    message: InvitationMessageKey,
    ics: &str,
    mime_method: &str,
) -> Result<InvitationPreview, InvitationError> {
    let document = inspect_calendar(ics)?;
    let mut invite = megamail_core::invite::parse(ics).ok_or(InvitationError::Malformed)?;
    if invite.method.is_empty() {
        invite.method = if mime_method.is_empty() {
            "PUBLISH".into()
        } else {
            mime_method.to_ascii_uppercase()
        };
    }
    let mut warnings = Vec::new();
    if document.events > 1 {
        warnings.push(InvitationWarning::MultipleEventsFirstOnly);
    }
    let method = match invite.method.as_str() {
        "REQUEST" => InvitationMethod::Request,
        "REPLY" => InvitationMethod::Reply,
        "CANCEL" => InvitationMethod::Cancel,
        "PUBLISH" => InvitationMethod::Publish,
        "COUNTER" => InvitationMethod::Counter,
        other => {
            warnings.push(InvitationWarning::UnknownMethod);
            InvitationMethod::Other(display_text(other, 64))
        }
    };
    let start = stamp_property(&document.event, "DTSTART")?;
    let end = stamp_property(&document.event, "DTEND")?;
    let occurrence = stamp_property(&document.event, "RECURRENCE-ID")?;
    if let (Some(start), Some(end)) = (&start, &end) {
        let start_date = matches!(start.zone, InvitationZone::DateOnly);
        let end_date = matches!(end.zone, InvitationZone::DateOnly);
        if start_date != end_date
            || (start.zone == end.zone
                && (end.value < start.value || (start_date && end.value == start.value)))
        {
            return Err(InvitationError::UnsupportedDate);
        }
    }
    if let Some(start) = &start {
        match &start.zone {
            InvitationZone::Floating => warnings.push(InvitationWarning::FloatingTimeNoZone),
            InvitationZone::Named(_) if invite.start == 0 => {
                warnings.push(InvitationWarning::NamedZoneResolutionUnavailable)
            }
            InvitationZone::DateOnly => warnings.push(InvitationWarning::AllDayEndExclusive),
            _ => {}
        }
    } else {
        warnings.push(InvitationWarning::MissingStart);
    }
    if document.event.contains_key("DURATION") {
        warnings.push(InvitationWarning::DurationNotMaterialized);
    }
    if !invite.repeats.is_empty() || occurrence.is_some() {
        warnings.push(InvitationWarning::RecurrenceNotExpanded);
    }
    let cancelled = invite.cancelled || matches!(&method, InvitationMethod::Cancel);
    let organizer = invite.organizer.as_ref().map(|p| person(p, &mut warnings));
    if invite.attendees.len() > 256 {
        warnings.push(InvitationWarning::AttendeesTruncated);
    }
    let attendees = invite
        .attendees
        .iter()
        .take(256)
        .map(|p| person(p, &mut warnings))
        .collect();
    let mut links = Vec::new();
    if let Some(url) = document.event.get("URL") {
        if megamail_core::mail_text::safe_browser_url(&url.value) {
            links.push(InvitationLink {
                destination: url.value.clone(),
            });
        } else {
            warnings.push(InvitationWarning::UnsafeUrlOmitted);
        }
    }
    let rsvp = if cancelled {
        RsvpCapability::Cancelled
    } else if invite.asks_for_an_answer() && organizer.as_ref().is_some_and(|p| p.email.is_some()) {
        RsvpCapability::CommandTransportUnavailable
    } else {
        RsvpCapability::NotRequested
    };
    let mut distinct_warnings = Vec::new();
    for warning in warnings {
        if !distinct_warnings.contains(&warning) {
            distinct_warnings.push(warning);
        }
    }
    Ok(InvitationPreview {
        message,
        uid: display_text(&invite.uid, 1024),
        sequence: invite.sequence,
        method,
        cancelled,
        summary: display_text(&invite.summary, 2048),
        location: display_text(&invite.location, 2048),
        start,
        end,
        occurrence,
        recurrence_rule: (!invite.repeats.is_empty()).then(|| display_text(&invite.repeats, 2048)),
        organizer,
        attendees,
        links,
        warnings: distinct_warnings,
        rsvp,
    })
}

/// Optional preview-subset validation. The shared core independently performs bounded
/// numeric and UTF-8-safe parsing; callers do not need this function to prevent its
/// previously unchecked duration/BYDAY failures. This does not expand recurrence.
pub fn validate_calendar(ics: &str) -> Result<(), InvitationError> {
    inspect_calendar(ics)?;
    megamail_core::invite::parse(ics).ok_or(InvitationError::Malformed)?;
    Ok(())
}

fn person(
    source: &megamail_core::models::InvitePerson,
    warnings: &mut Vec<InvitationWarning>,
) -> InvitationPerson {
    let email = crate::contacts::normalize_address(&source.email).ok();
    if email.is_none() {
        warnings.push(InvitationWarning::InvalidAddress);
    }
    let status = match source.status.as_str() {
        "NEEDS-ACTION" => Participation::NeedsAction,
        "ACCEPTED" => Participation::Accepted,
        "TENTATIVE" => Participation::Tentative,
        "DECLINED" => Participation::Declined,
        other => {
            warnings.push(InvitationWarning::UnknownParticipation);
            Participation::Other(display_text(other, 64))
        }
    };
    InvitationPerson {
        name: display_text(&source.name, 256),
        email,
        status,
        requested_response: source.rsvp,
        resource: source.resource,
    }
}
fn display_text(text: &str, characters: usize) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(characters)
        .collect()
}

struct Property {
    value: String,
    params: BTreeMap<String, String>,
}
struct CalendarDocument {
    events: usize,
    event: BTreeMap<String, Property>,
}

fn inspect_calendar(ics: &str) -> Result<CalendarDocument, InvitationError> {
    if ics.len() > MAX_ICS_BYTES {
        return Err(InvitationError::TooLarge);
    }
    let mut lines: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.len() > MAX_LINE_BYTES {
            return Err(InvitationError::TooLarge);
        }
        if line.chars().any(|c| c.is_control() && c != '\t') {
            return Err(InvitationError::Malformed);
        }
        if let Some(rest) = line.strip_prefix([' ', '\t']) {
            let previous = lines.last_mut().ok_or(InvitationError::Malformed)?;
            if previous.len() + rest.len() > MAX_LINE_BYTES {
                return Err(InvitationError::TooLarge);
            }
            previous.push_str(rest);
        } else if !line.is_empty() {
            lines.push(line.to_string());
        }
        if lines.len() > MAX_LINES {
            return Err(InvitationError::TooLarge);
        }
    }
    let mut stack: Vec<String> = Vec::new();
    let mut calendars = 0;
    let mut document = CalendarDocument {
        events: 0,
        event: BTreeMap::new(),
    };
    for line in lines {
        let (name, property) = read_property(&line)?;
        if name == "BEGIN" {
            let component = property.value.to_ascii_uppercase();
            if stack.is_empty() {
                if component != "VCALENDAR" || calendars != 0 {
                    return Err(InvitationError::Malformed);
                }
                calendars += 1;
            } else if component == "VCALENDAR" {
                return Err(InvitationError::Malformed);
            }
            if component == "VEVENT" {
                if stack.last().map(String::as_str) != Some("VCALENDAR") {
                    return Err(InvitationError::Malformed);
                }
                document.events += 1;
            }
            stack.push(component);
            if stack.len() > 8 {
                return Err(InvitationError::TooLarge);
            }
            continue;
        }
        if name == "END" {
            if stack.pop().as_deref() != Some(property.value.to_ascii_uppercase().as_str()) {
                return Err(InvitationError::Malformed);
            }
            continue;
        }
        if stack.is_empty() {
            return Err(InvitationError::Malformed);
        }
        // Validate display dates in every component. Core parsing independently
        // checks numeric grammar, bounds and UTF-8-safe timezone recurrence tokens.
        if matches!(name.as_str(), "DTSTART" | "DTEND" | "RECURRENCE-ID") {
            read_stamp(&property)?;
        }
        if name == "RRULE" && property.value.len() > 2048 {
            return Err(InvitationError::TooLarge);
        }
        // The production core owns numeric parsing. Do not reject UTF-8 text or
        // implement a second timezone/duration engine here.
        if stack.last().map(String::as_str) == Some("VEVENT")
            && document.events == 1
            && name != "ATTENDEE"
        {
            if document.event.insert(name.clone(), property).is_some() {
                return Err(InvitationError::Malformed);
            }
        }
    }
    if !stack.is_empty() || calendars != 1 {
        return Err(InvitationError::Malformed);
    }
    if document.events == 0 {
        return Err(InvitationError::MissingEvent);
    }
    Ok(document)
}

fn read_property(line: &str) -> Result<(String, Property), InvitationError> {
    let mut quoted = false;
    let colon = line
        .char_indices()
        .find_map(|(i, c)| {
            if c == '"' {
                quoted = !quoted;
            }
            (c == ':' && !quoted).then_some(i)
        })
        .ok_or(InvitationError::Malformed)?;
    let head = &line[..colon];
    let mut sections = Vec::new();
    let mut from = 0;
    let mut quoted = false;
    for (i, c) in head.char_indices() {
        if c == '"' {
            quoted = !quoted;
        }
        if c == ';' && !quoted {
            sections.push(&head[from..i]);
            from = i + 1;
        }
    }
    if quoted {
        return Err(InvitationError::Malformed);
    }
    sections.push(&head[from..]);
    let name = sections[0].to_ascii_uppercase();
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(InvitationError::Malformed);
    }
    let mut params = BTreeMap::new();
    for section in &sections[1..] {
        let (key, raw) = section.split_once('=').ok_or(InvitationError::Malformed)?;
        let value = raw
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(raw);
        if params
            .insert(key.to_ascii_uppercase(), value.to_string())
            .is_some()
        {
            return Err(InvitationError::Malformed);
        }
    }
    Ok((
        name,
        Property {
            value: line[colon + 1..].to_string(),
            params,
        },
    ))
}
fn stamp_property(
    properties: &BTreeMap<String, Property>,
    name: &str,
) -> Result<Option<InvitationStamp>, InvitationError> {
    properties.get(name).map(read_stamp).transpose()
}
fn read_stamp(property: &Property) -> Result<InvitationStamp, InvitationError> {
    let value = &property.value;
    if !value.is_ascii() {
        return Err(InvitationError::UnsupportedDate);
    }
    let date = value.len() == 8;
    let utc = value.len() == 16 && value.ends_with(['Z', 'z']);
    if !date && value.len() != 15 && !utc {
        return Err(InvitationError::UnsupportedDate);
    }
    let raw_digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if raw_digits.len() != if date { 8 } else { 14 }
        || (!date && value.as_bytes().get(8) != Some(&b'T'))
    {
        return Err(InvitationError::UnsupportedDate);
    }
    let number = |a, b| {
        raw_digits
            .get(a..b)
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or(InvitationError::UnsupportedDate)
    };
    let year = number(0, 4)?;
    let month = number(4, 6)?;
    let day = number(6, 8)?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if !(1..=9999).contains(&year) || day == 0 || day > days {
        return Err(InvitationError::UnsupportedDate);
    }
    if !date && (number(8, 10)? > 23 || number(10, 12)? > 59 || number(12, 14)? > 59) {
        return Err(InvitationError::UnsupportedDate);
    }
    if property.params.get("VALUE").is_some_and(|v| {
        if date {
            !v.eq_ignore_ascii_case("DATE")
        } else {
            !v.eq_ignore_ascii_case("DATE-TIME")
        }
    }) {
        return Err(InvitationError::UnsupportedDate);
    }
    let tzid = property.params.get("TZID");
    if tzid.is_some_and(|v| v.is_empty() || v.len() > 256 || v.chars().any(char::is_control))
        || ((date || utc) && tzid.is_some())
    {
        return Err(InvitationError::UnsupportedDate);
    }
    let zone = if date {
        InvitationZone::DateOnly
    } else if utc {
        InvitationZone::Utc
    } else if let Some(tzid) = tzid {
        InvitationZone::Named(display_text(tzid, 256))
    } else {
        InvitationZone::Floating
    };
    Ok(InvitationStamp {
        value: value.clone(),
        zone,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::contacts::ContactBackend;
    fn key() -> InvitationMessageKey {
        InvitationMessageKey {
            scope: ContactScope {
                backend: ContactBackend::Direct,
                account_id: 7,
            },
            folder_id: 2,
            message_id: 1,
            session_epoch: 3,
            body_generation: 4,
        }
    }
    fn event(fields: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:synthetic-event\r\nSEQUENCE:2\r\nSUMMARY:Review\r\nORGANIZER;CN=Host:mailto:host@example.test\r\nATTENDEE;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:me@example.test\r\n{fields}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        )
    }
    fn through_mime(ics: &str, media: &str) -> InvitationPreview {
        // Tests use the real helper that supplies production SenderCheck metadata,
        // then the same seam the native owner must wire. Never duplicate its parser.
        validate_calendar(ics).unwrap();
        let raw = format!(
            "From: host@example.test\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nNo body heuristics\r\n--b\r\nContent-Type: {media}\r\nContent-Disposition: attachment; filename=event.ics\r\n\r\n{ics}\r\n--b--\r\n"
        );
        let metadata = megamail_core::invite::detect_raw(raw.as_bytes()).unwrap();
        let check = SenderCheck {
            invite: Some(Box::new(metadata)),
            ..SenderCheck::default()
        };
        from_sender_check(key(), &check).unwrap().unwrap()
    }
    #[test]
    fn calendar_and_octet_stream_attachment_mime_cross_the_real_helper() {
        for media in [
            "text/calendar; method=REQUEST; charset=utf-8",
            "application/octet-stream",
        ] {
            let preview = through_mime(
                &event("DTSTART:20261102T080000Z\r\nDTEND:20261102T090000Z"),
                media,
            );
            assert_eq!(preview.start.unwrap().zone, InvitationZone::Utc);
            assert_eq!(preview.rsvp, RsvpCapability::CommandTransportUnavailable);
            assert_eq!(preview.message, key());
            assert_eq!(preview.attendees[0].status, Participation::NeedsAction);
        }
    }
    #[test]
    fn missing_document_method_preserves_mime_method_fallback() {
        let ics = event("DTSTART:20261102T080000Z").replace("METHOD:REQUEST\r\n", "");
        let preview = through_mime(&ics, "text/calendar; method=CANCEL");
        assert_eq!(preview.method, InvitationMethod::Cancel);
        assert_eq!(preview.rsvp, RsvpCapability::Cancelled);
    }
    #[test]
    fn recurrence_occurrence_retains_timezone_without_expansion_or_reply() {
        let preview = through_mime(
            &event(
                "DTSTART;TZID=Europe/Paris:20261102T090000\r\nRECURRENCE-ID;TZID=Europe/Paris:20261102T090000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO",
            ),
            "text/calendar",
        );
        assert_eq!(
            preview.occurrence.unwrap().zone,
            InvitationZone::Named("Europe/Paris".into())
        );
        assert!(
            preview
                .warnings
                .contains(&InvitationWarning::RecurrenceNotExpanded)
        );
        assert!(preview.recurrence_rule.is_some());
    }
    #[test]
    fn cancellation_by_method_or_status_disables_response() {
        for ics in [
            event("DTSTART:20261102T080000Z\r\nSTATUS:CANCELLED"),
            event("DTSTART:20261102T080000Z").replace("METHOD:REQUEST", "METHOD:CANCEL"),
        ] {
            let preview = through_mime(&ics, "text/calendar");
            assert!(preview.cancelled);
            assert_eq!(preview.rsvp, RsvpCapability::Cancelled);
        }
    }
    #[test]
    fn all_day_floating_and_unknown_zones_are_explicit_not_guessed() {
        let all_day = through_mime(
            &event("DTSTART;VALUE=DATE:20261102\r\nDTEND;VALUE=DATE:20261103"),
            "text/calendar",
        );
        assert_eq!(all_day.start.unwrap().zone, InvitationZone::DateOnly);
        assert_eq!(all_day.end.unwrap().value, "20261103");
        assert!(
            all_day
                .warnings
                .contains(&InvitationWarning::AllDayEndExclusive)
        );
        let floating = through_mime(&event("DTSTART:20261102T090000"), "text/calendar");
        assert!(
            floating
                .warnings
                .contains(&InvitationWarning::FloatingTimeNoZone)
        );
        let unknown = through_mime(
            &event("DTSTART;TZID=Unknown/SyntheticZone:20261102T090000"),
            "text/calendar",
        );
        assert!(
            unknown
                .warnings
                .contains(&InvitationWarning::NamedZoneResolutionUnavailable)
        );
    }
    #[test]
    fn windows_vtimezone_uses_existing_core_resolution() {
        let zone = "BEGIN:VTIMEZONE\r\nTZID:W. Europe Standard Time\r\nBEGIN:STANDARD\r\nDTSTART:16010101T030000\r\nTZOFFSETTO:+0100\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
        let ics = event("DTSTART;TZID=\"W. Europe Standard Time\":20261102T090000")
            .replace("BEGIN:VEVENT\r\n", &format!("{zone}BEGIN:VEVENT\r\n"));
        let preview = through_mime(&ics, "text/calendar");
        assert!(
            !preview
                .warnings
                .contains(&InvitationWarning::NamedZoneResolutionUnavailable)
        );
    }
    #[test]
    fn unsafe_urls_and_unknown_attendee_status_are_not_actions() {
        for url in [
            "javascript:alert(1)",
            "https://user:pass@example.test/",
            "file:///tmp/x",
        ] {
            let preview = through_mime(
                &event(&format!("DTSTART:20261102T080000Z\r\nURL:{url}")),
                "text/calendar",
            );
            assert!(preview.links.is_empty());
            assert!(
                preview
                    .warnings
                    .contains(&InvitationWarning::UnsafeUrlOmitted)
            );
        }
        let ics = event("DTSTART:20261102T080000Z\r\nURL:https://example.test/review")
            .replace("PARTSTAT=NEEDS-ACTION", "PARTSTAT=DELEGATED");
        let preview = through_mime(&ics, "text/calendar");
        assert_eq!(preview.links.len(), 1);
        assert!(
            preview
                .warnings
                .contains(&InvitationWarning::UnknownParticipation)
        );
    }
    #[test]
    fn malformed_dates_unicode_recurrence_oversize_and_overflow_are_rejected_before_core() {
        for fields in [
            "DTSTART:20260230T090000Z",
            "DTSTART:garbage20261102T090000Z",
            "DTSTART:20261102T990000Z",
            "DTSTART:20261102T080000Z\r\nDURATION:P999999999999999999999999999999999W",
            "DTSTART;TZID=Europe/Paris:20261102T080000Z",
        ] {
            assert!(preview_calendar(key(), &event(fields), "REQUEST").is_err());
        }
        assert_eq!(
            validate_calendar(&"x".repeat(MAX_ICS_BYTES + 1)),
            Err(InvitationError::TooLarge)
        );
        assert!(
            preview_calendar(
                key(),
                &event("DTSTART:20261102T080000Z").replace("END:VEVENT", "END:VTIMEZONE"),
                ""
            )
            .is_err()
        );
    }
    #[test]
    fn unicode_text_and_multiple_events_preview_only_the_helpers_first_event() {
        let second = "BEGIN:VEVENT\r\nUID:second\r\nSUMMARY:Not selected\r\nDTSTART:20261103T080000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let ics = event("DTSTART:20261102T080000Z")
            .replace("SUMMARY:Review", "SUMMARY:会議 — Chloé")
            .replace("END:VCALENDAR\r\n", second);
        let preview = through_mime(&ics, "text/calendar");
        assert_eq!(preview.summary, "会議 — Chloé");
        assert_eq!(preview.uid, "synthetic-event");
        assert!(
            preview
                .warnings
                .contains(&InvitationWarning::MultipleEventsFirstOnly)
        );
    }

    #[test]
    fn participant_preview_is_bounded_and_time_range_is_not_reversed() {
        let attendees = "ATTENDEE;PARTSTAT=ACCEPTED:mailto:guest@example.test\r\n".repeat(257);
        let ics = event(&format!("DTSTART:20261102T080000Z\r\n{attendees}"));
        let preview = through_mime(&ics, "text/calendar");
        assert_eq!(preview.attendees.len(), 256);
        assert!(
            preview
                .warnings
                .contains(&InvitationWarning::AttendeesTruncated)
        );
        assert!(
            preview_calendar(
                key(),
                &event("DTSTART:20261102T090000Z\r\nDTEND:20261102T080000Z"),
                ""
            )
            .is_err()
        );
        assert!(
            preview_calendar(
                key(),
                &event("DTSTART;VALUE=DATE:20261102\r\nDTEND;VALUE=DATE:20261102"),
                ""
            )
            .is_err()
        );
    }

    #[test]
    fn missing_metadata_and_plain_body_cannot_manufacture_a_meeting() {
        let raw = b"From: host@example.test\r\nContent-Type: text/plain\r\n\r\nBEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert!(megamail_core::invite::detect_raw(raw).is_none());
        assert!(
            from_sender_check(key(), &SenderCheck::default())
                .unwrap()
                .is_none()
        );
    }
}
