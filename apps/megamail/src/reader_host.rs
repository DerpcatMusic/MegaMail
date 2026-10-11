// SPDX-License-Identifier: AGPL-3.0-or-later
//! One immutable document per sandboxed helper. No shell, eval, fetch, profile
//! path, or JS selection bridge. All encoding/spawn/pipe I/O runs off GPUI.
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use serde::Deserialize;

use crate::reader_document::{ReaderDocument, ReaderScope};

const EVENT_MAX: usize = 8192;
const READY_TIMEOUT: Duration = Duration::from_secs(12);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(6);
const SESSION_MAX: Duration = Duration::from_secs(30 * 60);
const POLICY: &str = "v1:extension+sandbox+ephemeral+no-network";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostFailure {
    Unavailable,
    DocumentRejected,
    Pipe,
    Protocol,
    Policy,
    Timeout,
    Crashed,
    Backpressure,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderHostEvent {
    Ready {
        scope: ReaderScope,
    },
    /// Candidate only. Native owner must compare active scope again and require
    /// its explicit browser-opening policy. This module never opens destinations.
    UserClickedLink {
        scope: ReaderScope,
        url: String,
    },
    Fallback {
        scope: ReaderScope,
        reason: HostFailure,
    },
    Closed {
        scope: ReaderScope,
    },
}

pub struct ReaderHost {
    cancel: Arc<AtomicBool>,
    events: Receiver<ReaderHostEvent>,
}
impl ReaderHost {
    /// `executable` is the packaged, trusted absolute helper binary, supplied by
    /// the installer/native owner, NOT a message resource or user HTML string.
    /// The binary enforces bubblewrap containment before starting WebKit.
    pub fn start(executable: PathBuf, document: Arc<ReaderDocument>) -> std::io::Result<Self> {
        if !executable.is_absolute()
            || executable.file_name() != Some(std::ffi::OsStr::new("megamail-html-reader"))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "absolute packaged megamail-html-reader executable required",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (tx, events) = mpsc::sync_channel(16);
        thread::Builder::new()
            .name("megamail-reader-host".into())
            .spawn(move || {
                let scope = document.scope().clone();
                let result = supervise(executable, document, &worker_cancel, &tx);
                if !worker_cancel.load(Ordering::Acquire) {
                    if let Err(reason) = result {
                        // If the bounded queue is full, disconnection is the
                        // terminal fallback signal. Owner must still check scope.
                        let _ = tx.try_send(ReaderHostEvent::Fallback { scope, reason });
                    }
                }
            })?;
        Ok(Self { cancel, events })
    }
    /// Nonblocking. A disconnected worker with no terminal event is a fallback,
    /// never permission to render unchecked source. Owner handles this case.
    pub fn try_recv(&self) -> Result<ReaderHostEvent, TryRecvError> {
        self.events.try_recv()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Drop for ReaderHost {
    fn drop(&mut self) {
        self.cancel();
    } // Never join/block on the UI thread.
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEvent {
    version: u8,
    event: String,
    token: Option<String>,
    policy: Option<String>,
    url: Option<String>,
    code: Option<String>,
}
struct Protocol {
    token: Option<String>,
    links: usize,
}
impl Protocol {
    fn accept(
        &mut self,
        wire: WireEvent,
        scope: &ReaderScope,
    ) -> Result<Option<ReaderHostEvent>, HostFailure> {
        if wire.version != 1 {
            return Err(HostFailure::Protocol);
        }
        let valid_token = wire
            .token
            .as_deref()
            .is_some_and(|t| t.len() == 32 && t.bytes().all(|b| b.is_ascii_hexdigit()));
        match wire.event.as_str() {
            "ready"
                if self.token.is_none()
                    && valid_token
                    && wire.policy.as_deref() == Some(POLICY)
                    && wire.url.is_none()
                    && wire.code.is_none() =>
            {
                self.token = wire.token;
                Ok(Some(ReaderHostEvent::Ready {
                    scope: scope.clone(),
                }))
            }
            "error"
                if wire.code.as_deref().is_some_and(|c| c.len() <= 64)
                    && wire.url.is_none()
                    && wire.policy.is_none() =>
            {
                Err(HostFailure::Policy)
            }
            "closed"
                if wire.policy.is_none()
                    && wire.url.is_none()
                    && wire.code.is_none()
                    && valid_token
                    && (self.token.is_none() || wire.token == self.token) =>
            {
                Ok(Some(ReaderHostEvent::Closed {
                    scope: scope.clone(),
                }))
            }
            "heartbeat"
                if self.token.is_some()
                    && wire.token == self.token
                    && wire.url.is_none()
                    && wire.code.is_none()
                    && wire.policy.is_none() =>
            {
                Ok(None)
            }
            "user-click-link"
                if self.token.is_some()
                    && wire.token == self.token
                    && wire.policy.is_none()
                    && wire.code.is_none()
                    && self.links < 256 =>
            {
                let url = wire.url.ok_or(HostFailure::Protocol)?;
                if url.len() > 4096 || !megamail_core::mail_text::safe_browser_url(&url) {
                    return Err(HostFailure::Protocol);
                }
                self.links += 1;
                Ok(Some(ReaderHostEvent::UserClickedLink {
                    scope: scope.clone(),
                    url,
                }))
            }
            _ => Err(HostFailure::Protocol),
        }
    }
}
fn read_event(reader: &mut impl BufRead) -> Result<Option<WireEvent>, HostFailure> {
    let mut line = Vec::with_capacity(256);
    let length = reader
        .take((EVENT_MAX + 1) as u64)
        .read_until(b'\n', &mut line)
        .map_err(|_| HostFailure::Pipe)?;
    if length == 0 {
        return Ok(None);
    }
    if length > EVENT_MAX || line.last() != Some(&b'\n') {
        return Err(HostFailure::Protocol);
    }
    serde_json::from_slice(&line)
        .map(Some)
        .map_err(|_| HostFailure::Protocol)
}
fn publish(tx: &SyncSender<ReaderHostEvent>, event: ReaderHostEvent) -> Result<(), HostFailure> {
    tx.try_send(event).map_err(|_| HostFailure::Backpressure)
}
fn supervise(
    executable: PathBuf,
    document: Arc<ReaderDocument>,
    cancel: &AtomicBool,
    events: &SyncSender<ReaderHostEvent>,
) -> Result<(), HostFailure> {
    let bytes = document
        .helper_request()
        .map_err(|_| HostFailure::DocumentRejected)?;
    if cancel.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut command = Command::new(executable);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // No LD_PRELOAD, WebKit sandbox-disable, DBus/Secret Service, home, profile
    // or credential variables. Helper narrows the display grant to one socket.
    for name in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "GDK_BACKEND",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().map_err(|_| HostFailure::Unavailable)?;
    let (wire_tx, wire_rx) = mpsc::sync_channel(8);
    let (write_tx, write_rx) = mpsc::sync_channel(1);
    let setup = (|| {
        let mut stdin = child.stdin.take().ok_or(HostFailure::Pipe)?;
        let stdout = child.stdout.take().ok_or(HostFailure::Pipe)?;
        thread::Builder::new()
            .name("megamail-reader-write".into())
            .spawn(move || {
                let result = stdin
                    .write_all(&bytes)
                    .and_then(|()| stdin.flush())
                    .map_err(|_| HostFailure::Pipe);
                drop(stdin); // EOF delimits exactly one immutable document.
                let _ = write_tx.try_send(result);
            })
            .map_err(|_| HostFailure::Pipe)?;
        thread::Builder::new()
            .name("megamail-reader-read".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_event(&mut reader) {
                        Ok(Some(event)) => {
                            if wire_tx.try_send(Ok(event)).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            let _ = wire_tx.try_send(Err(error));
                            break;
                        }
                    }
                }
            })
            .map_err(|_| HostFailure::Pipe)?;
        let start = Instant::now();
        let mut pulse = start;
        let mut protocol = Protocol {
            token: None,
            links: 0,
        };
        loop {
            if cancel.load(Ordering::Acquire) {
                return Ok(());
            }
            if start.elapsed() > SESSION_MAX
                || (protocol.token.is_none() && start.elapsed() > READY_TIMEOUT)
                || (protocol.token.is_some() && pulse.elapsed() > HEARTBEAT_TIMEOUT)
            {
                return Err(HostFailure::Timeout);
            }
            if let Ok(result) = write_rx.try_recv() {
                result?;
            }
            match wire_rx.recv_timeout(Duration::from_millis(25)) {
                Ok(result) => {
                    let wire = result?;
                    let closed = wire.event == "closed";
                    let event = protocol.accept(wire, document.scope())?;
                    pulse = Instant::now();
                    if cancel.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    if let Some(event) = event {
                        publish(events, event)?;
                    }
                    if closed {
                        return Ok(());
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(HostFailure::Crashed),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if child.try_wait().map_err(|_| HostFailure::Pipe)?.is_some() {
                        return Err(HostFailure::Crashed);
                    }
                }
            }
        }
    })();
    // The exec'd bubblewrap supervisor and --die-with-parent own descendant
    // teardown. This must be tested on the packaged backend; don't infer it from
    // Child::kill alone. No blocking wait reaches the GPUI thread.
    let _ = child.kill();
    let reap_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < reap_deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => thread::sleep(Duration::from_millis(10)),
        }
    }
    // No unbounded wait, even after cancellation. A kernel-stuck child requires
    // OS/process-owner recovery; descendant cleanup is a packaging acceptance gate.
    setup
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> ReaderScope {
        ReaderScope {
            account_id: "a".into(),
            folder_id: "Inbox".into(),
            message_id: "1:9".into(),
            generation: 7,
        }
    }
    fn parse(json: &str) -> WireEvent {
        serde_json::from_str(json).unwrap()
    }
    #[test]
    fn forged_or_early_link_cannot_cross_production_protocol_seam() {
        let mut p = Protocol {
            token: None,
            links: 0,
        };
        assert!(p.accept(parse(r#"{"version":1,"event":"user-click-link","token":"0123456789abcdef0123456789abcdef","url":"https://example.invalid"}"#), &scope()).is_err());
    }
    #[test]
    fn stale_capability_cannot_cross_production_protocol_seam() {
        let mut p = Protocol {
            token: None,
            links: 0,
        };
        p.accept(parse(r#"{"version":1,"event":"ready","token":"0123456789abcdef0123456789abcdef","policy":"v1:extension+sandbox+ephemeral+no-network"}"#), &scope()).unwrap();
        assert!(p.accept(parse(r#"{"version":1,"event":"user-click-link","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","url":"https://example.invalid"}"#), &scope()).is_err());
    }
    #[test]
    fn valid_link_retains_native_scope_and_does_not_launch_anything() {
        let mut p = Protocol {
            token: Some("0123456789abcdef0123456789abcdef".into()),
            links: 0,
        };
        assert_eq!(p.accept(parse(r#"{"version":1,"event":"user-click-link","token":"0123456789abcdef0123456789abcdef","url":"https://example.invalid/a"}"#), &scope()).unwrap(), Some(ReaderHostEvent::UserClickedLink { scope: scope(), url: "https://example.invalid/a".into() }));
    }
    #[test]
    fn unbounded_or_unterminated_stdout_is_rejected() {
        assert!(matches!(
            read_event(&mut std::io::Cursor::new(vec![b'a'; EVENT_MAX + 2])),
            Err(HostFailure::Protocol)
        ));
        assert!(matches!(
            read_event(&mut std::io::Cursor::new(b"{}")),
            Err(HostFailure::Protocol)
        ));
    }
}
