//! One qualified operation, one terminal receipt; no automatic mutation retry.
use super::*;

pub(super) struct Lifetime<F: Fn(WorkerEvent)> {
    pub session: u64,
    pub emit: F,
}
impl<F: Fn(WorkerEvent)> Drop for Lifetime<F> {
    fn drop(&mut self) {
        (self.emit)(WorkerEvent::NativeWorkerStopped {
            session: self.session,
        });
    }
}

pub(super) fn reject(request: NativeMutationRequest, emit: &impl Fn(WorkerEvent), reason: &str) {
    emit(WorkerEvent::NativeMutationFinished(NativeMutationReceipt {
        request,
        outcome: NativeMutationOutcome::Rejected(reason.into()),
    }));
}

fn matches_epoch(request: &NativeMutationRequest, cache: Option<&Cache>, account: u32) -> bool {
    let NativeMutationScope::Imap {
        uid_validity,
        generation,
        ..
    } = request.scope
    else {
        return false;
    };
    cache
        .and_then(|c| c.mailbox_epoch(account, &request.path).ok())
        .is_some_and(|epoch| {
            epoch.uid_validity == Some(uid_validity) && epoch.generation == generation
        })
}

pub(super) async fn execute(
    request: NativeMutationRequest,
    session: &mut Option<ImapSession>,
    cache: Option<&Cache>,
    account_id: u32,
    account: &AccountConfig,
    native_session: u64,
    emit: &impl Fn(WorkerEvent),
) {
    if !request.valid()
        || !matches!(request.scope, NativeMutationScope::Imap { session, .. } if session == native_session)
        || !matches_epoch(&request, cache, account_id)
    {
        reject(
            request,
            emit,
            "This operation belongs to an invalid or retired mailbox. Refresh before trying again.",
        );
        return;
    }
    if let NativeMutationAction::Move { destination } = &request.action {
        if !cache.is_some_and(|c| {
            c.load_folders(account_id)
                .iter()
                .any(|f| f.path == *destination)
        }) {
            reject(
                request,
                emit,
                "The destination folder is no longer discovered in this account.",
            );
            return;
        }
    }
    if session.is_none() {
        *session = connect_and_list(account_id, account, cache, emit).await;
    }
    let Some(sess) = session.as_mut() else {
        reject(
            request,
            emit,
            "The account is offline. This action was not queued.",
        );
        return;
    };
    let NativeMutationScope::Imap { generation, .. } = request.scope else {
        unreachable!()
    };
    IMAP_UID_REQUEST_EPOCH
        .with(|stamp| *stamp.borrow_mut() = Some((request.path.clone(), generation)));
    // A read-only preflight must finish before any UID-changing command is sent.
    let preflight = async {
        let mailbox = sel(sess, &request.path).await?;
        let rows: Vec<Fetch> = fetch_uids(sess, request.uid.to_string(), "FLAGS")
            .await?
            .try_collect()
            .await?;
        Ok::<_, async_imap::error::Error>((mailbox, rows))
    }
    .await;
    let (mailbox, rows) = match preflight {
        Ok(result) if matches_epoch(&request, cache, account_id) => result,
        _ => {
            *session = None;
            set_epoch_connection_state(account_id, false);
            reject(
                request,
                emit,
                "The message could not be validated. No mutation was submitted.",
            );
            return;
        }
    };
    if !rows.iter().any(|row| row.uid == Some(request.uid)) {
        reject(
            request,
            emit,
            "The message is no longer present in this mailbox.",
        );
        return;
    }
    if let Some(expected) = request.required_current_flag {
        let unchanged = rows
            .iter()
            .find(|row| row.uid == Some(request.uid))
            .is_some_and(|row| match request.action {
                NativeMutationAction::SetSeen(_) => {
                    row.flags().any(|f| f == Flag::Seen) == expected
                }
                NativeMutationAction::SetFlagged(_) => {
                    row.flags().any(|f| f == Flag::Flagged) == expected
                }
                NativeMutationAction::Move { .. } => false,
            });
        if !unchanged {
            reject(
                request,
                emit,
                "The message flags changed since the original action. Undo was not submitted.",
            );
            return;
        }
        // shortcut: preflight is not atomic with STORE; use CONDSTORE before promising race-free undo.
    }
    let flag = match request.action {
        NativeMutationAction::SetSeen(_) => Some(Flag::Seen),
        NativeMutationAction::SetFlagged(_) => Some(Flag::Flagged),
        _ => None,
    };
    if flag.as_ref().is_some_and(|flag| {
        !mailbox.permanent_flags.is_empty()
            && !mailbox
                .permanent_flags
                .iter()
                .any(|f| f == flag || *f == Flag::MayCreate)
    }) {
        reject(
            request,
            emit,
            "This mailbox does not permit that persistent flag change.",
        );
        return;
    }
    let result = match &request.action {
        NativeMutationAction::SetSeen(seen) => {
            store_flag(sess, &request.path, request.uid, "\\Seen", *seen).await
        }
        NativeMutationAction::SetFlagged(flagged) => {
            store_flag(sess, &request.path, request.uid, "\\Flagged", *flagged).await
        }
        // Use MOVE directly: legacy fallback can retry or expunge unrelated deleted mail.
        NativeMutationAction::Move { destination } => {
            mv_uids(sess, &request.uid.to_string(), destination).await
        }
    };
    let mut outcome = match result {
        Ok(()) if matches_epoch(&request, cache, account_id) => NativeMutationOutcome::Succeeded,
        _ => NativeMutationOutcome::Uncertain("The server did not confirm this operation. Refresh and review it; no automatic retry was made.".into()),
    };
    if outcome == NativeMutationOutcome::Succeeded
        && !matches!(request.action, NativeMutationAction::Move { .. })
    {
        // STORE can be accepted yet ignored; verify the actual returned flags.
        let verified = async {
            let rows: Vec<Fetch> = fetch_uids(sess, request.uid.to_string(), "FLAGS")
                .await?
                .try_collect()
                .await?;
            Ok::<_, async_imap::error::Error>(
                rows.iter()
                    .find(|r| r.uid == Some(request.uid))
                    .is_some_and(|row| match request.action {
                        NativeMutationAction::SetSeen(want) => {
                            row.flags().any(|f| f == Flag::Seen) == want
                        }
                        NativeMutationAction::SetFlagged(want) => {
                            row.flags().any(|f| f == Flag::Flagged) == want
                        }
                        _ => false,
                    }),
            )
        }
        .await;
        if !matches!(verified, Ok(true)) || !matches_epoch(&request, cache, account_id) {
            outcome = NativeMutationOutcome::Uncertain(
                "The requested flags could not be confirmed. Refresh and review before retrying."
                    .into(),
            );
        }
    }
    if outcome == NativeMutationOutcome::Succeeded {
        if let Some(c) = cache {
            match &request.action {
                NativeMutationAction::SetSeen(seen) => {
                    c.set_unread(account_id, &request.path, request.uid, !seen)
                }
                NativeMutationAction::SetFlagged(flagged) => {
                    c.set_starred(account_id, &request.path, request.uid, *flagged)
                }
                NativeMutationAction::Move { destination } => {
                    drop_gmail_label_copies(
                        c,
                        account_id,
                        account,
                        &request.path,
                        &[request.uid],
                        destination,
                    );
                    c.delete_message(account_id, &request.path, request.uid);
                }
            }
        }
    } else {
        *session = None;
        set_epoch_connection_state(account_id, false);
    }
    emit(WorkerEvent::NativeMutationFinished(NativeMutationReceipt {
        request,
        outcome,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> NativeMutationRequest {
        NativeMutationRequest {
            token: NativeMutationToken {
                batch_id: 1,
                item_index: 0,
            },
            path: "INBOX".into(),
            uid: 42,
            scope: NativeMutationScope::Imap {
                uid_validity: 10,
                generation: 1,
                session: 1,
            },
            action: NativeMutationAction::SetSeen(true),
            required_current_flag: None,
        }
    }
    #[test]
    fn owned_worker_lifetime_reports_the_same_terminal_session() {
        let result = std::cell::RefCell::new(None);
        {
            let _lifetime = Lifetime {
                session: 19,
                emit: |event| {
                    if let WorkerEvent::NativeWorkerStopped { session } = event {
                        *result.borrow_mut() = Some(session);
                    }
                },
            };
        }
        assert_eq!(result.into_inner(), Some(19));
    }

    #[test]
    fn qualification_rejects_reused_uid_and_preserves_exact_rejection_receipt() {
        let cache = Cache::in_memory().unwrap();
        let first = cache.observe_uid_validity(7, "INBOX", 10).unwrap();
        let mut r = request();
        r.scope = NativeMutationScope::Imap {
            uid_validity: 10,
            generation: first.generation,
            session: 1,
        };
        assert!(r.valid());
        assert!(matches_epoch(&r, Some(&cache), 7));
        assert!(!matches_epoch(&r, Some(&cache), 8));
        cache.observe_uid_validity(7, "INBOX", 11).unwrap();
        assert!(!matches_epoch(&r, Some(&cache), 7));
        let receipt = std::cell::RefCell::new(None);
        reject(
            r.clone(),
            &|event| {
                if let WorkerEvent::NativeMutationFinished(value) = event {
                    *receipt.borrow_mut() = Some(value);
                }
            },
            "retired",
        );
        assert_eq!(receipt.into_inner().unwrap().request, r);
        r.token.item_index = 500;
        assert!(!r.valid());
        r = request();
        r.action = NativeMutationAction::Move {
            destination: "INBOX".into(),
        };
        assert!(!r.valid());
        r.action = NativeMutationAction::Move {
            destination: "Archive\nInjected".into(),
        };
        assert!(!r.valid());
        r.action = NativeMutationAction::Move {
            destination: "Archive".into(),
        };
        assert!(r.valid());
        r.required_current_flag = Some(true);
        assert!(!r.valid());
    }
}
