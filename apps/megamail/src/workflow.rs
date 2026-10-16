//! Local conversation work state, independent of unread and SMTP delivery.
//! Persistence is called only on a background executor. No message bodies stored.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECORDS: usize = 5000;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Status {
    #[default]
    Review,
    NeedsReply,
    Waiting,
    Snoozed,
    Done,
}
impl Status {
    pub const ALL: [Self; 5] = [
        Self::Review,
        Self::NeedsReply,
        Self::Waiting,
        Self::Snoozed,
        Self::Done,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Review => "To review",
            Self::NeedsReply => "Needs reply",
            Self::Waiting => "Waiting",
            Self::Snoozed => "Snoozed",
            Self::Done => "Done",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub account: u32,
    pub ids: Vec<String>,
    pub status: Status,
    pub observed_inbound: i64,
    pub snooze_until: Option<i64>,
    pub updated: i64,
}
impl Record {
    pub fn effective(&self, latest_inbound: i64, now: i64) -> Status {
        if matches!(self.status, Status::Waiting | Status::Done)
            && latest_inbound > self.observed_inbound
        {
            Status::NeedsReply
        } else if self.status == Status::Snoozed
            && self.snooze_until.is_none_or(|until| until <= now)
        {
            Status::NeedsReply
        } else {
            self.status
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct Ledger {
    records: Vec<Record>,
    index: HashMap<(u32, String), usize>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    records: Vec<Record>,
}
impl Ledger {
    pub fn load() -> Result<Self, String> {
        let root =
            megamail_core::config::config_base().ok_or("Work-state storage is unavailable.")?;
        let file = match std::fs::File::open(root.join("work-state.v1.json")) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => {
                return Err(
                    "Could not read saved conversation states. The file was preserved.".into(),
                );
            }
        };
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read conversation states.")?;
        Self::decode(&bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("Conversation-state file exceeds its size limit; it was preserved.".into());
        }
        let file: File = serde_json::from_slice(bytes).map_err(
            |_| "Saved conversation states could not be parsed; the file was preserved.",
        )?;
        if file.version != 1
            || file.records.len() > MAX_RECORDS
            || file.records.iter().any(|r| {
                r.account == 0
                    || r.ids.is_empty()
                    || r.ids.len() > 128
                    || r.ids.iter().any(|id| {
                        id.is_empty() || id.len() > 512 || id.chars().any(char::is_control)
                    })
            })
        {
            return Err(
                "Saved conversation states have an unsupported format; the file was preserved."
                    .into(),
            );
        }
        let mut ledger = Self {
            records: file.records,
            index: HashMap::new(),
        };
        ledger.reindex();
        Ok(ledger)
    }
    fn reindex(&mut self) {
        self.index.clear();
        self.records.sort_by_key(|r| r.updated);
        for (i, record) in self.records.iter().enumerate() {
            for id in &record.ids {
                self.index.insert((record.account, id.clone()), i);
            }
        }
    }
    pub fn find(&self, account: u32, ids: &[String]) -> Option<&Record> {
        ids.iter()
            .filter_map(|id| {
                self.index
                    .get(&(account, id.clone()))
                    .and_then(|i| self.records.get(*i))
            })
            .max_by_key(|r| r.updated)
    }
    pub fn with_record(&self, mut record: Record) -> Result<Self, String> {
        if record.ids.is_empty() {
            return Err(
                "This message has no stable Message-ID; work state cannot be saved safely.".into(),
            );
        }
        let mut next = self.clone();
        // Preserve identifiers when a newly discovered reply joins an existing thread.
        let matches: Vec<_> = next
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.account == record.account && r.ids.iter().any(|id| record.ids.contains(id))
            })
            .map(|(i, _)| i)
            .collect();
        for i in &matches {
            record.ids.extend(next.records[*i].ids.clone());
        }
        let mut seen = HashSet::new();
        record.ids.retain(|id| seen.insert(id.clone()));
        if record.ids.len() > 128 {
            record.ids.truncate(128);
        }
        next.records = next
            .records
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !matches.contains(i))
            .map(|(_, r)| r)
            .collect();
        if next.records.len() >= MAX_RECORDS {
            return Err("Conversation-state storage is full; no saved state was removed.".into());
        }
        next.records.push(record);
        next.reindex();
        Ok(next)
    }
    pub fn save(&self) -> Result<(), String> {
        let bytes = serde_json::to_string(&File {
            version: 1,
            records: self.records.clone(),
        })
        .map_err(|_| "Could not encode conversation states.")?;
        if bytes.len() > MAX_BYTES {
            return Err("Conversation-state storage is full; previous data was preserved.".into());
        }
        let root =
            megamail_core::config::config_base().ok_or("Work-state storage is unavailable.")?;
        megamail_core::config::write_private_file(&root.join("work-state.v1.json"), &bytes)
            .map_err(|e| format!("Conversation state was not saved: {e}"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(account: u32, id: &str) -> Record {
        Record {
            account,
            ids: vec![id.into()],
            status: Status::Done,
            observed_inbound: 10,
            snooze_until: None,
            updated: 20,
        }
    }
    #[test]
    fn inbound_reopens_waiting_or_done_but_read_state_is_irrelevant() {
        let r = record(1, "a");
        assert_eq!(r.effective(10, 50), Status::Done);
        assert_eq!(r.effective(11, 50), Status::NeedsReply);
        let r = Record {
            status: Status::Snoozed,
            snooze_until: Some(60),
            ..r
        };
        assert_eq!(r.effective(30, 59), Status::Snoozed);
        assert_eq!(r.effective(30, 60), Status::NeedsReply);
    }
    #[test]
    fn thread_merges_retain_previous_ids_and_accounts_never_collide() {
        let a = Ledger::default()
            .with_record(record(1, "root"))
            .unwrap()
            .with_record(record(2, "root"))
            .unwrap();
        let mut r = record(1, "reply");
        r.ids.push("root".into());
        r.status = Status::Waiting;
        r.updated = 30;
        let a = a.with_record(r).unwrap();
        assert_eq!(
            a.find(1, &["reply".into()]).unwrap().status,
            Status::Waiting
        );
        assert_eq!(a.find(2, &["root".into()]).unwrap().status, Status::Done);
        let bytes = serde_json::to_vec(&File {
            version: 1,
            records: a.records,
        })
        .unwrap();
        assert_eq!(
            Ledger::decode(&bytes)
                .unwrap()
                .find(1, &["root".into()])
                .unwrap()
                .status,
            Status::Waiting
        );
        assert!(Ledger::decode(br#"{"version":2,"records":[]}"#).is_err());
        assert!(Ledger::decode(b"broken").is_err());
    }
}
