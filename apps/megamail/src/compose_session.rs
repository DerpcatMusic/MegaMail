//! Native recovery orchestration; production capture uses the full outgoing model.
use crate::draft_recovery::{
    self, AttachmentLease, DraftKey, RecoveryStore, RecoveryWriter, Retention, WriterCommand,
    WriterEvent,
};
use megamail_core::{config::AccountConfig, worker::OutgoingMessage};
use std::{path::PathBuf, sync::mpsc, time::Duration};

pub(crate) struct Service {
    pub root: PathBuf,
    writer: RecoveryWriter,
    events: mpsc::Receiver<WriterEvent>,
    namespace: String,
    sequence: u32,
    reaper: mpsc::SyncSender<AttachmentLease>,
}
impl Service {
    /// Blocking executor only. Recovery has its own persistent private root,
    /// not cache/body staging. The outer activation lock remains held.
    pub fn open() -> Result<Self, String> {
        let root = megamail_core::config::config_base()
            .ok_or_else(|| "Private profile is unavailable.".to_owned())?
            .join("draft-recovery");
        Self::open_at(root)
    }
    fn open_at(root: PathBuf) -> Result<Self, String> {
        let store = RecoveryStore::open(&root).map_err(|e| e.to_string())?;
        store.prune_stale_work(64).map_err(|e| e.to_string())?;
        let (writer, events) =
            RecoveryWriter::start(store, Duration::from_millis(500)).map_err(|e| e.to_string())?;
        let namespace = megamail_core::rng::token(16)
            .map_err(|_| "Recovery identity is unavailable.".to_owned())?;
        let (reaper, leases) = mpsc::sync_channel::<AttachmentLease>(8);
        std::thread::Builder::new()
            .name("native-recovery-lease-release".into())
            .spawn(move || {
                while let Ok(lease) = leases.recv() {
                    drop(lease);
                }
            })
            .map_err(|_| "Recovery attachment cleanup is unavailable.".to_owned())?;
        Ok(Self {
            root,
            writer,
            events,
            namespace,
            sequence: 0,
            reaper,
        })
    }
    pub fn key(&mut self, account_id: u32) -> Result<DraftKey, String> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| "Recovery identity space exhausted.".to_owned())?;
        DraftKey::from_parts(
            account_id,
            format!("{}{:08x}", self.namespace, self.sequence),
        )
        .map_err(|e| e.to_string())
    }
    pub fn submit(&self, command: WriterCommand) -> Result<(), String> {
        self.writer.submit(command).map_err(|_| "Local recovery is busy or closed; current changes are not acknowledged. Keep this composer open.".into())
    }
    pub fn reaper(&self) -> mpsc::SyncSender<AttachmentLease> {
        self.reaper.clone()
    }
    pub fn release(&self, lease: AttachmentLease) -> Result<(), String> {
        release(&self.reaper, lease)
    }
    pub fn events(&self) -> Vec<WriterEvent> {
        (0..16)
            .filter_map(|_| self.events.try_recv().ok())
            .collect()
    }
}

/// Stale/closed-window restore deliveries must not remove staged files on UI.
/// The bounded worker reaper owns final lease destruction. If it cannot accept
/// the lease, retain its staging until restart rather than doing UI filesystem I/O.
fn release(
    reaper: &mpsc::SyncSender<AttachmentLease>,
    lease: AttachmentLease,
) -> Result<(), String> {
    if let Err(error) = reaper.try_send(lease) {
        let lease = match error {
            mpsc::TrySendError::Full(lease) | mpsc::TrySendError::Disconnected(lease) => lease,
        };
        std::mem::forget(lease);
        return Err(
            "Attachment cleanup is busy/unavailable; staging was retained for startup repair."
                .into(),
        );
    }
    Ok(())
}
pub(crate) struct LeaseDelivery {
    recovered: Option<draft_recovery::RecoveredCompose>,
    reaper: mpsc::SyncSender<AttachmentLease>,
}
impl LeaseDelivery {
    pub fn new(
        recovered: Option<draft_recovery::RecoveredCompose>,
        reaper: mpsc::SyncSender<AttachmentLease>,
    ) -> Self {
        Self { recovered, reaper }
    }
    pub fn take(&mut self) -> Option<draft_recovery::RecoveredCompose> {
        self.recovered.take()
    }
}
impl Drop for LeaseDelivery {
    fn drop(&mut self) {
        if let Some(recovered) = self.recovered.take() {
            let _ = release(&self.reaper, recovered.attachment_lease);
        }
    }
}

/// Native accepted requests retain restored paths independently of the view.
/// Success/unknown send and confirmed save release only the exact ticket. Legacy
/// uncorrelated failures and window shutdown retain staging until restart: there
/// is no worker-retirement acknowledgment that would make deletion safe then.
#[derive(Default)]
pub(crate) struct HeldAttachments {
    leases: std::collections::HashMap<
        (bool, u32, u64),
        (AttachmentLease, mpsc::SyncSender<AttachmentLease>),
    >,
}
impl HeldAttachments {
    pub fn hold(
        &mut self,
        send: bool,
        account: u32,
        ticket: u64,
        lease: Option<AttachmentLease>,
        reaper: mpsc::SyncSender<AttachmentLease>,
    ) {
        if let Some(lease) = lease {
            self.leases.insert((send, account, ticket), (lease, reaper));
        }
    }
    pub fn finish(&mut self, send: bool, account: u32, ticket: u64) {
        if let Some((lease, reaper)) = self.leases.remove(&(send, account, ticket)) {
            let _ = release(&reaper, lease);
        }
    }
}
impl Drop for HeldAttachments {
    fn drop(&mut self) {
        for (_, (lease, _)) in self.leases.drain() {
            std::mem::forget(lease);
        }
    }
}

pub(crate) struct Capture {
    pub key: Option<DraftKey>,
    pub generation: u64,
    pub last: Option<OutgoingMessage>,
    pub acknowledged: u64,
    pub retention: Retention,
    pub lease: Option<AttachmentLease>,
    pub status: String,
    pub restored_hold: bool,
    pub retry: bool,
    pub requires_ack: bool,
    pub dirty_input: bool,
    pub in_flight: Option<u64>,
    // A pre-send Saved acknowledgment is not a deletion/retirement receipt.
    pub retired: Option<u64>,
    pub finished_send: Option<(u32, u64, u64)>,
    pub cleanup_in_flight: bool,
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            key: None,
            generation: 0,
            last: None,
            acknowledged: 0,
            retention: Retention::DecryptedSessionOnly,
            lease: None,
            status: "Local recovery starting; close/shutdown is not crash-safe.".into(),
            restored_hold: false,
            retry: false,
            requires_ack: true,
            dirty_input: true,
            in_flight: None,
            retired: None,
            finished_send: None,
            cleanup_in_flight: false,
        }
    }
}
impl Capture {
    pub fn blocks_provider_origin(&self, message: &OutgoingMessage) -> bool {
        self.restored_hold && (message.draft_origin.is_some() || message.outbox_origin.is_some())
    }
    /// Checked full-model dirty generation. Called by both the event pump and
    /// send/save dispatch, not just visible To/Subject/Body listeners.
    pub fn observe(&mut self, outgoing: OutgoingMessage) -> Result<bool, String> {
        if self
            .last
            .as_ref()
            .is_some_and(|previous| same(previous, &outgoing))
        {
            return Ok(false);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| "Draft generation exhausted. Keep this composer open.".to_owned())?;
        self.last = Some(outgoing);
        Ok(true)
    }
    pub fn update(&self, account: &AccountConfig) -> Result<WriterCommand, String> {
        let key = self
            .key
            .as_ref()
            .ok_or_else(|| "Recovery key is unavailable.".to_owned())?;
        let message = self
            .last
            .as_ref()
            .ok_or_else(|| "Recovery has no composition.".to_owned())?;
        crate::compose::prepare_recovery_update(
            message,
            key,
            self.generation,
            account,
            self.retention,
            self.lease.clone(),
        )
        .map(WriterCommand::Update)
    }
    pub fn event(&mut self, event: WriterEvent) {
        match event {
            WriterEvent::Saved {
                key,
                generation,
                outcome,
            } if self.key.as_ref() == Some(&key) && generation == self.generation => {
                self.acknowledged = generation;
                self.in_flight = None;
                self.status = match outcome { draft_recovery::SaveOutcome::Durable => "Private local recovery saved. Close/shutdown acknowledgment is not implemented.", draft_recovery::SaveOutcome::SessionOnly => "Session only: source sensitivity is unknown or decrypted. No local plaintext recovery was saved." }.into();
            }
            WriterEvent::Failed {
                key,
                generation,
                error,
            } if self.key.as_ref() == Some(&key) && generation == self.generation => {
                self.in_flight = None;
                self.cleanup_in_flight = false;
                self.status = format!("Local recovery failed: {error}. Keep this composer open.");
            }
            WriterEvent::Discarded {
                key,
                generation,
                removed,
            } if self.key.as_ref() == Some(&key)
                && generation == self.generation
                && self
                    .finished_send
                    .is_some_and(|(account, _, sent_generation)| {
                        account == key.account_id() && sent_generation == generation
                    }) =>
            {
                self.retired = Some(generation);
                self.cleanup_in_flight = false;
                self.in_flight = None;
                self.retry = false;
                self.status = if removed {
                    "Sent confirmed; matching local recovery record removed.".into()
                } else {
                    "Sent confirmed; no exact-generation record was removed. An older recovery copy may remain and must not be resent without Outbox review.".into()
                };
            }
            _ => {}
        }
    }
}
pub(crate) fn same(a: &OutgoingMessage, b: &OutgoingMessage) -> bool {
    a.from_account_id == b.from_account_id
        && a.from_alias == b.from_alias
        && a.to == b.to
        && a.cc == b.cc
        && a.bcc == b.bcc
        && a.reply_to == b.reply_to
        && a.subject == b.subject
        && a.body == b.body
        && a.html == b.html
        && a.attachments == b.attachments
        && a.in_reply_to == b.in_reply_to
        && a.references == b.references
        && a.draft_origin
            .as_ref()
            .map(|d| (d.account_id, d.folder_id, &d.path, d.uid))
            == b.draft_origin
                .as_ref()
                .map(|d| (d.account_id, d.folder_id, &d.path, d.uid))
        && a.outbox_origin == b.outbox_origin
        && a.sign == b.sign
        && a.encrypt == b.encrypt
        && a.send_at == b.send_at
        && a.calendar.as_ref().map(|c| (&c.ics, &c.method))
            == b.calendar.as_ref().map(|c| (&c.ics, &c.method))
}
#[cfg(test)]
mod tests {
    #[test]
    fn recovered_provider_origin_is_not_replacement_authority() {
        let mut capture = super::Capture {
            restored_hold: true,
            ..Default::default()
        };
        let mut message = crate::compose::new_message(7);
        assert!(!capture.blocks_provider_origin(&message));
        message.outbox_origin = Some(42);
        assert!(capture.blocks_provider_origin(&message));
        capture.restored_hold = false;
        assert!(!capture.blocks_provider_origin(&message));
    }
    #[test]
    fn native_retirement_is_not_a_presend_save_ack_and_needs_the_actual_sent_scope() {
        use super::*;
        let key = DraftKey::from_parts(7, "abcdefghijklmnopqrstuvwx".into()).unwrap();
        let mut capture = Capture {
            key: Some(key.clone()),
            generation: 2,
            acknowledged: 1,
            ..Default::default()
        };
        capture.event(WriterEvent::Discarded {
            key: key.clone(),
            generation: 2,
            removed: true,
        });
        assert_eq!(capture.retired, None, "no actual Sent completion admitted");
        capture.finished_send = Some((7, 42, 2));
        capture.event(WriterEvent::Discarded {
            key: DraftKey::from_parts(8, "abcdefghijklmnopqrstuvwx".into()).unwrap(),
            generation: 2,
            removed: true,
        });
        assert_eq!(capture.retired, None);
        capture.event(WriterEvent::Discarded {
            key,
            generation: 2,
            removed: false,
        });
        assert_eq!(capture.retired, Some(2));
        assert_eq!(
            capture.acknowledged, 1,
            "retirement must not relabel a Saved receipt"
        );
        assert!(capture.status.contains("older recovery copy"));
    }
    #[test]
    fn retained_send_cleanup_ticket_keeps_full_64_bit_request_identity() {
        let mut capture = super::Capture::default();
        capture.finished_send = Some((7, u64::MAX, 2));
        assert_eq!(capture.finished_send, Some((7, u64::MAX, 2)));
    }

    #[test]
    fn native_capture_writer_and_restore_cross_the_actual_outgoing_seam() {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "megamail-native-recovery-fixture-{}-{}",
            std::process::id(),
            megamail_core::rng::token(12).unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut service = Service::open_at(root.clone()).unwrap();
        let account = AccountConfig::new("from@example.test");
        let mut capture = Capture {
            key: Some(service.key(7).unwrap()),
            retention: Retention::Ordinary,
            ..Default::default()
        };
        let mut outgoing = crate::compose::new_message(7);
        outgoing.to = "recipient@example.test".into();
        outgoing.bcc = "hidden@example.test".into();
        outgoing.reply_to = "reply@example.test".into();
        outgoing.in_reply_to = "thread@example.test".into();
        outgoing.references = "ancestor@example.test thread@example.test".into();
        outgoing.body = "Fictional body".into();
        outgoing.html = "<p>Fictional body</p>".into();
        capture.observe(outgoing).unwrap();
        service.submit(capture.update(&account).unwrap()).unwrap();
        service.submit(WriterCommand::Flush).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while capture.acknowledged != capture.generation && std::time::Instant::now() < deadline {
            for event in service.events() {
                capture.event(event);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(capture.acknowledged, 1, "{}", capture.status);
        let store = RecoveryStore::open(&root).unwrap();
        let recovered = crate::compose::prepare_local_recovery(
            &store,
            capture.key.as_ref().unwrap(),
            &account,
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(recovered.message.from_account_id, 7);
        assert_eq!(recovered.source_address, "from@example.test");
        assert_eq!(recovered.message.bcc, "hidden@example.test");
        assert_eq!(recovered.message.reply_to, "reply@example.test");
        assert_eq!(recovered.message.html, "<p>Fictional body</p>");
        assert_eq!(recovered.message.in_reply_to, "thread@example.test");
        let stages: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "stage")
            })
            .collect();
        assert_eq!(stages.len(), 1);
        service.release(recovered.attachment_lease).unwrap();
        let cleanup_deadline = std::time::Instant::now() + Duration::from_secs(3);
        while stages.iter().any(|path| path.exists())
            && std::time::Instant::now() < cleanup_deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            stages.iter().all(|path| !path.exists()),
            "lease reaper did not finish fixture cleanup"
        );
        drop(store);
        drop(service);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hidden_metadata_is_a_real_dirty_generation() {
        let mut capture = super::Capture::default();
        let mut outgoing = crate::compose::new_message(1);
        assert!(capture.observe(outgoing.clone()).unwrap());
        assert!(!capture.observe(outgoing.clone()).unwrap());
        outgoing.bcc = "hidden@example.test".into();
        outgoing.from_alias = Some("Alias <alias@example.test>".into());
        assert!(capture.observe(outgoing).unwrap());
        assert_eq!(capture.generation, 2);
    }
    #[test]
    fn dirty_generation_never_wraps() {
        let mut capture = super::Capture {
            generation: u64::MAX,
            ..Default::default()
        };
        assert!(capture.observe(crate::compose::new_message(1)).is_err());
    }
}
