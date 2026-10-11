//! Isolated Thunderbird runtime and native-messaging transport.
//!
//! This starts Thunderbird against a MegaMail-owned profile. Only literal
//! allowlisted account settings and Thunderbird's encrypted credential store
//! are copied; source mail databases, message caches and extensions are not.

use std::io;
use std::path::{Path, PathBuf};

const MAX_FRAME: usize = 1024 * 1024;
const MAX_EXTENSION_FRAME: usize = 16 * 1024 * 1024;
const EXTENSION_ID: &str = "bridge@megamail.local";

/// Stable recovery categories for the existing string-returning API.
/// A request timeout or overload does not mean the shared runtime is dead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeFailureKind {
    Disconnected,
    Protocol,
    RequestTimeout,
    Busy,
    Unsupported,
    NeedsLogin,
    Operation,
}

/// Classify only bridge-generated tags, not arbitrary provider error prose.
/// `None` means the caller must retain its legacy/operation-specific handling.
pub fn classify_bridge_error(error: &str) -> Option<BridgeFailureKind> {
    let tags = [
        ("disconnected", BridgeFailureKind::Disconnected),
        ("protocol", BridgeFailureKind::Protocol),
        ("request-timeout", BridgeFailureKind::RequestTimeout),
        ("busy", BridgeFailureKind::Busy),
        ("unsupported", BridgeFailureKind::Unsupported),
        ("needs-login", BridgeFailureKind::NeedsLogin),
        ("operation", BridgeFailureKind::Operation),
    ];
    tags.into_iter().find_map(|(tag, kind)| {
        error
            .starts_with(&format!("[megamail-tb:{tag}] "))
            .then_some(kind)
    })
}

fn bridge_error(tag: &str, message: &str) -> String {
    format!("[megamail-tb:{tag}] {message}")
}

#[cfg(unix)]
mod unix {
    use super::*;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::{self, SyncSender};
    use std::sync::{Arc, Mutex, TryLockError};
    use std::time::{Duration, Instant};

    static PRIVATE_FILE_ID: AtomicU64 = AtomicU64::new(0);

    const BACKGROUND: &str =
        include_str!("../../../tools/thunderbird-bridge/extension/background.js");
    const MANIFEST: &str =
        include_str!("../../../tools/thunderbird-bridge/extension/manifest.json");
    const SYNC_SCHEMA: &str =
        include_str!("../../../tools/thunderbird-bridge/extension/api/schema.json");
    const SYNC_IMPL: &str =
        include_str!("../../../tools/thunderbird-bridge/extension/api/implementation.js");
    const MAX_PENDING: usize = 64;
    // JSON numbers must round-trip through JavaScript without losing identity.
    const SHUTDOWN_ID: u64 = (1_u64 << 53) - 1;
    const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
    const FRAME_TIMEOUT: Duration = Duration::from_secs(15);
    const METADATA_TIMEOUT: Duration = Duration::from_secs(15);
    const BODY_TIMEOUT: Duration = Duration::from_secs(30);
    const FOLDER_TIMEOUT: Duration = Duration::from_secs(90);
    const REFRESH_TIMEOUT: Duration = Duration::from_secs(165);
    const MUTATION_TIMEOUT: Duration = Duration::from_secs(120);

    struct RuntimeInner {
        stream: Mutex<UnixStream>,
        control: UnixStream,
        sequence: Arc<AtomicU64>,
        pending: Arc<Mutex<HashMap<u64, PendingReply>>>,
        broken: Arc<AtomicBool>,
        child: Arc<Mutex<ChildGuard>>,
        reader: Mutex<Option<std::thread::JoinHandle<()>>>,
        _profile_lock: File,
        manifest: PathBuf,
        temporary_dir: PathBuf,
        profile_dir: PathBuf,
    }

    struct PendingReply {
        sender: SyncSender<Result<Value, String>>,
        deadline: Instant,
    }

    struct ChildGuard {
        child: Option<Child>,
        isolated_group: bool,
    }

    impl ChildGuard {
        fn child_mut(&mut self) -> &mut Child {
            self.child.as_mut().expect("child guard is armed")
        }

        fn terminate(&mut self) -> Option<std::process::ExitStatus> {
            if let Some(mut child) = self.child.take() {
                // The group was created by Command::process_group(0), and its
                // leader has NOT been reaped. Never target a name or a scanned PID.
                if self.isolated_group {
                    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
                        // SAFETY: negative PID addresses only our dedicated group.
                        unsafe {
                            libc::kill(-pid, libc::SIGKILL);
                        }
                    }
                }
                let _ = child.kill();
                return child.wait().ok();
            }
            None
        }
    }

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.terminate();
        }
    }

    // WNOWAIT keeps the leader unreaped until group cleanup. Reaping it first
    // would allow its numeric PID/PGID to be reused by an unrelated process.
    #[cfg(target_os = "linux")]
    fn child_exited(child: &Child) -> io::Result<bool> {
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: waitid initializes siginfo; it observes our child only.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: waitid succeeded and the zero-initialized structure is valid.
        Ok(unsafe { info.assume_init().si_pid() != 0 })
    }

    #[cfg(not(target_os = "linux"))]
    fn child_exited(_: &Child) -> io::Result<bool> {
        // Linux is the verified target; timeout cleanup still owns the group.
        Ok(false)
    }

    struct StartupArtifacts {
        manifest: PathBuf,
        temporary_dir: PathBuf,
        armed: bool,
    }

    impl Drop for StartupArtifacts {
        fn drop(&mut self) {
            if self.armed {
                let _ = fs::remove_file(&self.manifest);
                let _ = fs::remove_dir_all(&self.temporary_dir);
            }
        }
    }

    /// Cloneable synchronous access to the persistent, isolated Thunderbird
    /// profile. Requests share one ordered socket, while a response reader
    /// routes each reply independently by request ID.
    #[derive(Clone)]
    pub struct Runtime(Arc<RuntimeInner>);

    impl Runtime {
        /// Start Thunderbird using its private cloned profile and MegaMail's
        /// current executable as the native-messaging relay.
        pub fn start(profile: &crate::thunderbird::ThunderbirdProfile) -> Result<Self, String> {
            let app = std::env::current_exe()
                .map_err(|error| format!("Could not locate MegaMail: {error}"))?;
            let thunderbird = find_thunderbird()?;
            Self::start_with_executables(profile, &app, &thunderbird)
        }

        /// Test and diagnostic entry point. `app_executable` is invoked by the
        /// generated native-host wrapper and `thunderbird_executable` starts
        /// the isolated profile.
        pub fn start_with_executables(
            profile: &crate::thunderbird::ThunderbirdProfile,
            app_executable: &Path,
            thunderbird_executable: &Path,
        ) -> Result<Self, String> {
            let app_executable = fs::canonicalize(app_executable).map_err(io_message(
                "Could not locate MegaMail's native-messaging executable",
            ))?;
            let thunderbird_executable = fs::canonicalize(thunderbird_executable)
                .map_err(io_message("Could not locate Thunderbird"))?;
            if !thunderbird_executable.is_file() {
                return Err("Thunderbird executable was not found.".to_owned());
            }
            validate_thunderbird_version(&thunderbird_executable)?;
            if profile.accounts.is_empty() {
                return Err(
                    "The selected Thunderbird profile has no supported IMAP account.".to_owned(),
                );
            }
            let profile_lock = acquire_profile_lock(profile)?;
            let auth_stamp = credentials_stamp(&profile.path)?;
            let profile_dir = private_profile_dir(profile, &auth_stamp)?;
            prepare_profile(profile, &profile_dir, &auth_stamp)?;

            let temporary_dir = new_private_temp_dir()?;
            let mut artifacts = StartupArtifacts {
                manifest: PathBuf::new(),
                temporary_dir: temporary_dir.clone(),
                armed: true,
            };
            let host_name = bridge_host_name(&profile_dir);
            let socket_path = temporary_dir.join("bridge.sock");
            let wrapper_path = temporary_dir.join("host-launcher");
            let manifest_dir = dirs::home_dir()
                .ok_or_else(|| {
                    "Could not locate the home directory for Thunderbird's native host.".to_owned()
                })?
                .join(".mozilla/native-messaging-hosts");
            fs::create_dir_all(&manifest_dir).map_err(io_message(
                "Could not create Thunderbird's native-host directory",
            ))?;
            let manifest_path = manifest_dir.join(format!("{host_name}.json"));
            artifacts.manifest = manifest_path.clone();

            write_new_private_bytes(
                &wrapper_path,
                launcher_script(&app_executable, &socket_path).as_bytes(),
            )?;
            fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o700)).map_err(
                io_message("Could not secure the Thunderbird bridge launcher"),
            )?;
            let manifest = json!({
                "name": host_name,
                "description": "MegaMail's private Thunderbird bridge",
                "path": wrapper_path,
                "type": "stdio",
                "allowed_extensions": [EXTENSION_ID]
            });
            write_private_bytes(&manifest_path, manifest.to_string().as_bytes())?;

            let extension_dir = profile_dir.join("extensions");
            ensure_private_dir(&extension_dir)?;
            let xpi = build_extension(&host_name)?;
            write_private_bytes_if_changed(
                &extension_dir.join(format!("{EXTENSION_ID}.xpi")),
                &xpi,
            )?;

            let listener = UnixListener::bind(&socket_path).map_err(io_message(
                "Could not create the private Thunderbird bridge socket",
            ))?;
            fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).map_err(
                io_message("Could not secure the private Thunderbird bridge socket"),
            )?;
            listener
                .set_nonblocking(true)
                .map_err(io_message("Could not configure the private bridge socket"))?;

            let startup_deadline = Instant::now() + STARTUP_TIMEOUT;
            let child = Command::new(thunderbird_executable)
                .process_group(0)
                .arg("--headless")
                .arg("--no-remote")
                .arg("--profile")
                .arg(&profile_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(io_message("Could not start Thunderbird"))?;
            let mut child = ChildGuard {
                child: Some(child),
                isolated_group: true,
            };
            let expected_version = extension_version(
                MANIFEST,
                serde_json::from_str::<Value>(MANIFEST)
                    .map_err(|_| "Invalid bridge manifest".to_owned())?["version"]
                    .as_str()
                    .ok_or_else(|| "Missing bridge manifest version".to_owned())?,
                &BACKGROUND.replace("__HOST_NAME__", &host_name),
                SYNC_SCHEMA,
                SYNC_IMPL,
            );
            let stream =
                accept_ready_bridge(&listener, &mut child, startup_deadline, &expected_version)?;
            let runtime = Self::from_connected(
                stream,
                child,
                profile_lock,
                manifest_path,
                temporary_dir,
                profile_dir,
            )?;
            artifacts.armed = false;
            Ok(runtime)
        }

        fn from_connected(
            stream: UnixStream,
            child: ChildGuard,
            profile_lock: File,
            manifest: PathBuf,
            temporary_dir: PathBuf,
            profile_dir: PathBuf,
        ) -> Result<Self, String> {
            let reader = stream
                .try_clone()
                .map_err(io_message("Could not clone Thunderbird response socket"))?;
            let pending = Arc::new(Mutex::new(HashMap::new()));
            let broken = Arc::new(AtomicBool::new(false));
            let child = Arc::new(Mutex::new(child));
            let sequence = Arc::new(AtomicU64::new(1));
            let runtime = Self(Arc::new(RuntimeInner {
                control: stream
                    .try_clone()
                    .map_err(io_message("Could not clone bridge shutdown socket"))?,
                stream: Mutex::new(stream),
                sequence: sequence.clone(),
                pending: pending.clone(),
                broken: broken.clone(),
                child: child.clone(),
                reader: Mutex::new(None),
                _profile_lock: profile_lock,
                manifest,
                temporary_dir,
                profile_dir,
            }));
            let reader_thread = std::thread::Builder::new()
                .name("megamail-thunderbird-bridge".to_owned())
                .spawn(move || response_reader(reader, pending, broken, child, sequence))
                .map_err(io_message("Could not start Thunderbird response reader"))?;
            *runtime
                .0
                .reader
                .lock()
                .map_err(|_| "Thunderbird bridge lock failed".to_owned())? = Some(reader_thread);
            Ok(runtime)
        }

        /// Invoke one extension operation and return its JSON result. The
        /// budget includes writer admission, frame transfer and reply waiting.
        pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
            self.call_until(method, params, Instant::now() + operation_timeout(method))
        }

        fn call_until(
            &self,
            method: &str,
            params: Value,
            deadline: Instant,
        ) -> Result<Value, String> {
            if self.0.broken.load(Ordering::Acquire) {
                return Err(bridge_error("disconnected", "Thunderbird bridge is disconnected. Restart the account runtime before trying again."));
            }
            if method.is_empty() || method.len() > 128 || method.chars().any(char::is_control) {
                return Err("Invalid Thunderbird bridge operation.".to_owned());
            }
            let id = self.0.sequence.fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| {
                (id < SHUTDOWN_ID).then(|| id + 1)
            }).map_err(|_| bridge_error("busy", "Thunderbird request identifiers are exhausted. Reopen the runtime before submitting more work."))?;
            let request = json!({ "id": id, "method": method, "params": params });
            let bytes = encode_request(&request)?;
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            {
                let mut pending = self
                    .0
                    .pending
                    .lock()
                    .map_err(|_| "Thunderbird bridge lock failed".to_owned())?;
                // Pair admission with disconnect's drain. No waiter may be
                // installed after retirement of the shared transport.
                if self.0.broken.load(Ordering::Acquire) {
                    return Err(bridge_error(
                        "disconnected",
                        "Thunderbird bridge is disconnected.",
                    ));
                }
                if pending.len() >= MAX_PENDING {
                    return Err(bridge_error("busy", "Thunderbird is busy with 64 pending operations. Try again after an operation finishes."));
                }
                pending.insert(
                    id,
                    PendingReply {
                        sender: reply_tx,
                        deadline,
                    },
                );
            }
            let write_result = loop {
                if Instant::now() >= deadline {
                    break Err(operation_timeout_message(method));
                }
                match self.0.stream.try_lock() {
                    Ok(mut stream) => {
                        if self.0.broken.load(Ordering::Acquire) {
                            break Err(bridge_error(
                                "disconnected",
                                "Thunderbird bridge is disconnected.",
                            ));
                        }
                        break write_runtime_frame(&mut stream, &bytes, deadline);
                    }
                    Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(1)),
                    Err(TryLockError::Poisoned(_)) => {
                        break Err(bridge_error(
                            "disconnected",
                            "Thunderbird bridge writer lock failed.",
                        ))
                    }
                }
            };
            if let Err(error) = write_result {
                expire_pending(&self.0.pending, id);
                // No bytes sent: timeout is local. A partial frame cannot be
                // safely reused and must close the transport.
                if classify_bridge_error(&error) != Some(BridgeFailureKind::RequestTimeout) {
                    self.mark_broken(error.clone());
                }
                return Err(error);
            }
            match reply_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(reply) if Instant::now() <= deadline => reply,
                Ok(_) => Err(operation_timeout_message(method)),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    expire_pending(&self.0.pending, id);
                    Err(operation_timeout_message(method))
                }
                Err(mpsc::RecvTimeoutError::Disconnected) if Instant::now() >= deadline => {
                    Err(operation_timeout_message(method))
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(bridge_error(
                    "disconnected",
                    "Thunderbird bridge disconnected while the operation was running.",
                )),
            }
        }

        fn mark_broken(&self, message: String) {
            // Do not wait for the writer mutex merely to cancel its I/O.
            let _ = self.0.control.shutdown(std::net::Shutdown::Both);
            if let Ok(mut stream) = self.0.stream.try_lock() {
                break_transport(
                    Some(&mut stream),
                    &self.0.pending,
                    &self.0.broken,
                    &self.0.child,
                    message,
                );
            } else {
                break_transport(
                    None,
                    &self.0.pending,
                    &self.0.broken,
                    &self.0.child,
                    message,
                );
            }
        }

        /// App-owned persistent clone; never the source mail cache.
        pub fn profile_dir(&self) -> &Path {
            &self.0.profile_dir
        }
    }

    impl Drop for RuntimeInner {
        fn drop(&mut self) {
            let reader_started = self
                .reader
                .get_mut()
                .ok()
                .is_some_and(|reader| reader.is_some());
            let shutdown_deadline = Instant::now() + Duration::from_secs(2);
            if reader_started && !self.broken.load(Ordering::Acquire) {
                let (reply_tx, reply_rx) = mpsc::sync_channel(1);
                if let Ok(mut pending) = self.pending.lock() {
                    pending.insert(
                        SHUTDOWN_ID,
                        PendingReply {
                            sender: reply_tx,
                            deadline: shutdown_deadline,
                        },
                    );
                }
                let request = json!({ "id": SHUTDOWN_ID, "method": "shutdown", "params": {} });
                if let Ok(bytes) = serde_json::to_vec(&request) {
                    if let Ok(stream) = self.stream.get_mut() {
                        let _ = write_runtime_frame(stream, &bytes, shutdown_deadline);
                    }
                    let _ = reply_rx
                        .recv_timeout(shutdown_deadline.saturating_duration_since(Instant::now()));
                }
            }
            // Acknowledgment alone is not EOF. Wake and join the response
            // reader before releasing the clone lock and transient artifacts.
            let _ = self.control.shutdown(std::net::Shutdown::Both);
            self.broken.store(true, Ordering::Release);
            if let Ok(mut pending) = self.pending.lock() {
                pending.clear();
            }
            if let Ok(mut child) = self.child.lock() {
                let _ = child.terminate();
            }
            if let Ok(reader) = self.reader.get_mut() {
                if let Some(reader) = reader.take() {
                    let _ = reader.join();
                }
            }
            let _ = fs::remove_file(&self.manifest);
            let _ = fs::remove_dir_all(&self.temporary_dir);
        }
    }

    fn accept_ready_bridge(
        listener: &UnixListener,
        child: &mut ChildGuard,
        deadline: Instant,
        expected_version: &str,
    ) -> Result<UnixStream, String> {
        let mut stream = loop {
            if Instant::now() >= deadline {
                return Err(bridge_error(
                    "disconnected",
                    "Thunderbird did not start its private bridge before the startup deadline.",
                ));
            }
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if let Some(child) = child.child.as_ref() {
                        if child_exited(child)
                            .map_err(io_message("Could not inspect Thunderbird startup"))?
                        {
                            return Err(bridge_error("disconnected", "Thunderbird stopped during startup. The source profile was not changed."));
                        }
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(bridge_error(
                        "disconnected",
                        &format!("Could not accept the private Thunderbird bridge: {error}"),
                    ))
                }
            }
        };
        let hello = read_runtime_json_frame(&mut stream, Some(deadline))?;
        if hello.get("id").and_then(Value::as_u64) != Some(0)
            || hello.pointer("/result/ready").and_then(Value::as_bool) != Some(true)
            || hello.pointer("/result/version").and_then(Value::as_str) != Some(expected_version)
        {
            return Err(bridge_error("protocol", "Thunderbird started without the current MegaMail bridge handshake. Reopen the runtime to load the bundled extension."));
        }
        Ok(stream)
    }

    fn operation_timeout(method: &str) -> Duration {
        match method {
            "accounts" | "list" | "conversation" | "attachments" => METADATA_TIMEOUT,
            "body" | "raw_start" | "attachment_start" | "download_chunk" => BODY_TIMEOUT,
            "folders" => FOLDER_TIMEOUT,
            "refresh" => REFRESH_TIMEOUT,
            "send" | "save" | "update" | "move" | "delete" => MUTATION_TIMEOUT,
            _ => BODY_TIMEOUT,
        }
    }

    fn operation_timeout_message(method: &str) -> String {
        let message = match method {
            "send" => "Thunderbird did not confirm sending before the timeout. The message may still have been sent; check Sent and Outbox before retrying.",
            "save" => "Thunderbird did not confirm saving the draft before the timeout. Check Drafts before retrying.",
            _ => "Thunderbird did not finish this operation before its time limit. Its late reply will be ignored.",
        };
        bridge_error("request-timeout", message)
    }

    fn response_reader(
        mut stream: UnixStream,
        pending: Arc<Mutex<HashMap<u64, PendingReply>>>,
        broken: Arc<AtomicBool>,
        child: Arc<Mutex<ChildGuard>>,
        sequence: Arc<AtomicU64>,
    ) {
        loop {
            let response = match read_runtime_json_frame(&mut stream, None) {
                Ok(response) => response,
                Err(error) => {
                    break_transport(Some(&mut stream), &pending, &broken, &child, error);
                    return;
                }
            };
            let id = response.get("id").and_then(Value::as_u64);
            let shutdown_pending = id == Some(SHUTDOWN_ID)
                && pending.lock().is_ok_and(|p| p.contains_key(&SHUTDOWN_ID));
            if !shutdown_pending
                && !matches!(id, Some(id) if id > 0 && id < sequence.load(Ordering::Acquire))
            {
                break_transport(
                    Some(&mut stream),
                    &pending,
                    &broken,
                    &child,
                    bridge_error(
                        "protocol",
                        "Thunderbird bridge disconnected after an uncorrelated response ID.",
                    ),
                );
                return;
            }
            if let Err(error) = route_response(response, &pending) {
                break_transport(Some(&mut stream), &pending, &broken, &child, error);
                return;
            }
        }
    }

    fn route_response(
        mut response: Value,
        pending: &Mutex<HashMap<u64, PendingReply>>,
    ) -> Result<(), String> {
        let id = response.get("id").and_then(Value::as_u64).ok_or_else(|| {
            bridge_error(
                "protocol",
                "Thunderbird returned a response without a request ID.",
            )
        })?;
        if response.get("error").is_some() == response.get("result").is_some()
            || response.get("error").is_some_and(|v| !v.is_string())
        {
            return Err(bridge_error(
                "protocol",
                "Thunderbird bridge disconnected after an invalid response envelope.",
            ));
        }
        let reply = pending
            .lock()
            .map_err(|_| bridge_error("disconnected", "Thunderbird bridge lock failed"))?
            .remove(&id);
        let Some(reply) = reply else {
            return Ok(());
        };
        if Instant::now() > reply.deadline {
            return Ok(());
        }
        let result = if let Some(error) = response.get("error").and_then(Value::as_str) {
            // Only an explicit code is authentication evidence. Timeouts,
            // snapshot problems and cached folder data never prove login.
            let message: String = error
                .chars()
                .filter(|c| !c.is_control())
                .take(1024)
                .collect();
            if response.get("errorCode").and_then(Value::as_str) == Some("needs-login") {
                Err(bridge_error("needs-login", &message))
            } else {
                Err(bridge_error("operation", &message))
            }
        } else {
            Ok(response["result"].take())
        };
        let _ = reply.sender.try_send(result);
        Ok(())
    }

    fn expire_pending(pending: &Mutex<HashMap<u64, PendingReply>>, id: u64) -> bool {
        pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id))
            .is_some()
    }

    fn break_transport(
        stream: Option<&mut UnixStream>,
        pending: &Mutex<HashMap<u64, PendingReply>>,
        broken: &AtomicBool,
        child: &Mutex<ChildGuard>,
        message: String,
    ) {
        if broken.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(stream) = stream {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Ok(mut pending) = pending.lock() {
            for (_, reply) in pending.drain() {
                let _ = reply.sender.try_send(Err(message.clone()));
            }
        }
        if let Ok(mut child) = child.lock() {
            let _ = child.terminate();
        }
    }

    /// Entry point used by MegaMail's `--thunderbird-host SOCKET` dispatch.
    /// Native messaging uses a little-endian length prefix on both stdio and
    /// the private Unix socket.
    pub fn run_native_host(socket_path: &Path) -> Result<(), String> {
        let stream = UnixStream::connect(socket_path).map_err(io_message(
            "Could not connect to the private Thunderbird bridge",
        ))?;
        let mut socket_writer = stream
            .try_clone()
            .map_err(io_message("Could not clone bridge socket"))?;
        let _stdin_thread = std::thread::spawn(move || -> Result<(), String> {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            loop {
                let frame = match read_frame_with_limit(&mut input, MAX_EXTENSION_FRAME) {
                    Ok(Some(frame)) => frame,
                    Ok(None) => {
                        let _ = socket_writer.shutdown(std::net::Shutdown::Both);
                        return Ok(());
                    }
                    Err(error) => {
                        let _ = socket_writer.shutdown(std::net::Shutdown::Both);
                        return Err(error);
                    }
                };
                if let Err(error) =
                    write_frame_with_limit(&mut socket_writer, &frame, MAX_EXTENSION_FRAME)
                {
                    let _ = socket_writer.shutdown(std::net::Shutdown::Both);
                    return Err(error);
                }
            }
        });
        let mut socket_reader = stream;
        let stdout = io::stdout();
        let mut output = stdout.lock();
        loop {
            let Some(frame) = read_frame_with_limit(&mut socket_reader, MAX_FRAME)? else {
                break;
            };
            write_frame_with_limit(&mut output, &frame, MAX_FRAME)?;
            output
                .flush()
                .map_err(io_message("Could not flush Thunderbird bridge response"))?;
        }
        Ok(())
    }

    fn private_profile_dir(
        profile: &crate::thunderbird::ThunderbirdProfile,
        stamp: &str,
    ) -> Result<PathBuf, String> {
        let data = crate::config::data_base()
            .ok_or_else(|| "Could not create MegaMail's private data directory.".to_owned())?;
        // Reuse a clone while the source snapshot stays the same. A source
        // credential update gets a new isolated clone, so key4.db and
        // logins.json are never replaced under Thunderbird's running profile.
        let source_root = data
            .join("thunderbird/profiles")
            .join(source_profile_key(profile));
        ensure_private_dir(&source_root)?;
        let generation = Sha256::digest(stamp.as_bytes());
        let key = generation[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = source_root.join(key);
        ensure_private_dir(&path)?;
        Ok(path)
    }

    fn private_local_folders_dir(
        profile: &crate::thunderbird::ThunderbirdProfile,
    ) -> Result<PathBuf, String> {
        let data = crate::config::data_base()
            .ok_or_else(|| "Could not create MegaMail's private data directory.".to_owned())?;
        let mail_root = data
            .join("thunderbird/profiles")
            .join(source_profile_key(profile))
            .join("Mail");
        ensure_private_dir(&mail_root)?;
        let path = mail_root.join("LocalFolders");
        ensure_private_dir(&path)?;
        Ok(path)
    }

    fn source_profile_key(profile: &crate::thunderbird::ThunderbirdProfile) -> String {
        let digest = Sha256::digest(profile.path.to_string_lossy().as_bytes());
        digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn acquire_profile_lock(
        profile: &crate::thunderbird::ThunderbirdProfile,
    ) -> Result<File, String> {
        let data = crate::config::data_base()
            .ok_or_else(|| "Could not create MegaMail's private data directory.".to_owned())?;
        let lock_dir = data.join("thunderbird/locks");
        ensure_private_dir(&lock_dir)?;
        let lock_path = lock_dir.join(format!("{}.lock", source_profile_key(profile)));
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let file = options.open(&lock_path).map_err(io_message(
            "Could not open MegaMail's Thunderbird profile lock",
        ))?;
        let metadata = file.metadata().map_err(io_message(
            "Could not inspect MegaMail's Thunderbird profile lock",
        ))?;
        // SAFETY: geteuid is a read-only process identity query.
        if !metadata.file_type().is_file()
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(
                "MegaMail's Thunderbird profile lock is not an owned, singly linked regular file."
                    .to_owned(),
            );
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(io_message(
                "Could not secure MegaMail's Thunderbird profile lock",
            ))?;
        file.try_lock().map_err(|_| {
            "This Thunderbird profile is already connected to another MegaMail session.".to_owned()
        })?;
        Ok(file)
    }

    fn prepare_profile(
        profile: &crate::thunderbird::ThunderbirdProfile,
        target: &Path,
        wanted_stamp: &str,
    ) -> Result<(), String> {
        ensure_isolated_clone(&profile.path, target)?;
        let marker = target.join(".megamail-initialized");
        let stamp_path = target.join(".megamail-auth-stamp");
        validate_private_file_path(&marker)?;
        validate_private_file_path(&stamp_path)?;
        let current_stamp = read_bounded_source_file(&stamp_path, 4096, "private snapshot stamp")?
            .map(String::from_utf8).transpose()
            .map_err(|_| "MegaMail's private Thunderbird snapshot stamp is invalid. Repair the private clone before retrying.".to_owned())?;
        if current_stamp.as_deref() != Some(wanted_stamp) {
            if source_profile_locked(&profile.path) {
                return Err("Thunderbird is using this source profile. Close Thunderbird before importing its updated encrypted credentials.".to_owned());
            }
            for database in ["key4.db", "cert9.db"] {
                check_database_journal(target, database).map_err(|_| "MegaMail's private Thunderbird credential store is still finishing a write. Close and reopen MegaMail, then retry.".to_owned())?;
            }
            copy_encrypted_auth_store(&profile.path, target, wanted_stamp)?;
            let confirmed_stamp = credentials_stamp(&profile.path)?;
            if confirmed_stamp != wanted_stamp {
                return Err("Thunderbird's credential store changed during import. Close Thunderbird and retry.".to_owned());
            }
            write_private_bytes(&stamp_path, wanted_stamp.as_bytes())?;
        }
        if !marker.exists() {
            write_new_private_bytes(&marker, b"profile initialized\n")?;
        }
        let prefs = crate::thunderbird::runtime_preferences(profile).map_err(io_message(
            "Could not read Thunderbird's safe account settings",
        ))?;
        let servers = crate::thunderbird::runtime_server_ids(profile)
            .map_err(io_message("Could not read Thunderbird server identifiers"))?;
        let mail_root = target.join("Mail/ImapMail");
        ensure_private_dir(&mail_root)?;
        let mut lines = Vec::with_capacity(prefs.len() + servers.len() * 4 + 24);
        for (key, value) in prefs {
            if key == "mail.accountmanager.accounts" {
                continue;
            }
            lines.push(format!(
                "user_pref({}, {});",
                serde_json::to_string(&key).unwrap_or_else(|_| "\"\"".to_owned()),
                value
            ));
        }
        const LOCAL_ACCOUNT_ID: &str = "accountMegaMailLocal";
        const LOCAL_SERVER_ID: &str = "serverMegaMailLocal";
        let mut account_ids: Vec<&str> = profile
            .accounts
            .iter()
            .map(|account| account.account_id.as_str())
            .collect();
        account_ids.push(LOCAL_ACCOUNT_ID);
        lines.push(pref_line(
            "mail.accountmanager.accounts",
            &serde_json::to_string(&account_ids.join(",")).unwrap(),
        ));
        if let Some(default_account) = profile.accounts.first() {
            lines.push(pref_line(
                "mail.accountmanager.defaultaccount",
                &serde_json::to_string(&default_account.account_id).unwrap(),
            ));
        }
        lines.push(pref_line(
            "mail.accountmanager.localfoldersserver",
            &serde_json::to_string(LOCAL_SERVER_ID).unwrap(),
        ));
        lines.push(pref_line(
            &format!("mail.account.{LOCAL_ACCOUNT_ID}.server"),
            &serde_json::to_string(LOCAL_SERVER_ID).unwrap(),
        ));
        lines.push(pref_line(
            &format!("mail.server.{LOCAL_SERVER_ID}.type"),
            &serde_json::to_string("none").unwrap(),
        ));
        lines.push(pref_line(
            &format!("mail.server.{LOCAL_SERVER_ID}.hostname"),
            &serde_json::to_string("Local Folders").unwrap(),
        ));
        lines.push(pref_line(
            &format!("mail.server.{LOCAL_SERVER_ID}.name"),
            &serde_json::to_string("Local Folders").unwrap(),
        ));
        lines.push(pref_line(
            &format!("mail.server.{LOCAL_SERVER_ID}.userName"),
            &serde_json::to_string("nobody").unwrap(),
        ));
        let local_dir = private_local_folders_dir(profile)?;
        lines.push(pref_line(
            &format!("mail.server.{LOCAL_SERVER_ID}.directory"),
            &serde_json::to_string(&local_dir.to_string_lossy().to_string()).unwrap(),
        ));
        lines.push(pref_line(
            &format!("mail.server.{LOCAL_SERVER_ID}.directory-rel"),
            &serde_json::to_string("[ProfD]../Mail/LocalFolders").unwrap(),
        ));
        for (_, server_id) in servers {
            let mail_dir = mail_root.join(&server_id);
            ensure_private_dir(&mail_dir)?;
            let directory = serde_json::to_string(&mail_dir.to_string_lossy().to_string())
                .map_err(|_| "Could not encode the private mail directory.".to_owned())?;
            let relative = serde_json::to_string(&format!("[ProfD]Mail/ImapMail/{server_id}"))
                .map_err(|_| "Could not encode the private mail directory.".to_owned())?;
            lines.push(pref_line(
                &format!("mail.server.{server_id}.directory"),
                &directory,
            ));
            lines.push(pref_line(
                &format!("mail.server.{server_id}.directory-rel"),
                &relative,
            ));
            lines.extend(server_safety_preferences(&server_id));
        }
        lines.extend([
            "user_pref(\"extensions.autoDisableScopes\", 0);".to_owned(),
            "user_pref(\"extensions.enabledScopes\", 15);".to_owned(),
            "user_pref(\"extensions.experiments.enabled\", true);".to_owned(),
            "user_pref(\"extensions.update.enabled\", false);".to_owned(),
            "user_pref(\"app.update.auto\", false);".to_owned(),
            "user_pref(\"offline.send.unsent_messages\", 2);".to_owned(),
            "user_pref(\"mailnews.start_page.enabled\", false);".to_owned(),
            "user_pref(\"mailnews.start_page.url\", \"\");".to_owned(),
            "user_pref(\"calendar.autorefresh.enabled\", false);".to_owned(),
            "user_pref(\"services.sync.enabled\", false);".to_owned(),
            "user_pref(\"identity.fxaccounts.enabled\", false);".to_owned(),
            "user_pref(\"datareporting.healthreport.uploadEnabled\", false);".to_owned(),
            "user_pref(\"toolkit.telemetry.enabled\", false);".to_owned(),
            "user_pref(\"services.sync.engine.addressbook\", false);".to_owned(),
            "user_pref(\"services.sync.engine.bookmarks\", false);".to_owned(),
            "user_pref(\"services.sync.engine.history\", false);".to_owned(),
        ]);
        write_private_bytes_if_changed(&target.join("user.js"), lines.join("\n").as_bytes())?;
        Ok(())
    }

    fn bridge_host_name(profile_dir: &Path) -> String {
        let digest = Sha256::digest(profile_dir.to_string_lossy().as_bytes());
        let key = digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("com.megamail.bridge.{key}")
    }

    fn pref_line(key: &str, value: &str) -> String {
        format!(
            "user_pref({}, {});",
            serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_owned()),
            value
        )
    }

    fn server_safety_preferences(server_id: &str) -> [String; 5] {
        [
            pref_line(&format!("mail.server.{server_id}.check_new_mail"), "false"),
            pref_line(
                &format!("mail.server.{server_id}.download_on_biff"),
                "false",
            ),
            pref_line(
                &format!("mail.server.{server_id}.login_at_startup"),
                "false",
            ),
            // Thunderbird can expunge deleted mail or empty Trash while this
            // private runtime shuts down. Those source-profile preferences
            // must not become remote destructive actions in MegaMail.
            pref_line(
                &format!("mail.server.{server_id}.empty_trash_on_exit"),
                "false",
            ),
            pref_line(
                &format!("mail.server.{server_id}.cleanup_inbox_on_exit"),
                "false",
            ),
        ]
    }

    fn copy_encrypted_auth_store(
        source: &Path,
        target: &Path,
        expected_stamp: &str,
    ) -> Result<(), String> {
        ensure_isolated_clone(source, target)?;
        if credentials_stamp(source)? != expected_stamp {
            return Err("Thunderbird's credential snapshot changed before import. Close Thunderbird and retry.".to_owned());
        }
        for database in ["key4.db", "cert9.db"] {
            check_database_journal(source, database)?;
        }

        let stage = target.join(format!(".auth-stage-{}", random_hex(12)?));
        fs::create_dir(&stage).map_err(io_message(
            "Could not create a private encrypted credential staging directory",
        ))?;
        fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).map_err(io_message(
            "Could not secure the encrypted credential staging directory",
        ))?;
        let copy_result = (|| {
            let mut names = Vec::new();
            for (name, limit) in [
                ("key4.db", 64 * 1024 * 1024),
                ("logins.json", 64 * 1024 * 1024),
                ("cert9.db", 32 * 1024 * 1024),
            ] {
                if let Some(bytes) = read_bounded_source_file(&source.join(name), limit, name)? {
                    write_private_bytes(&stage.join(name), &bytes)?;
                }
                names.push(name);
            }
            for database in ["key4.db", "cert9.db"] {
                check_database_journal(source, database)?;
            }
            if credentials_stamp(source)? != expected_stamp {
                return Err("Thunderbird's credential store changed during import. Close Thunderbird and retry.".to_owned());
            }
            for name in names {
                let staged = stage.join(name);
                if staged.exists() {
                    fs::rename(&staged, target.join(name)).map_err(io_message(
                        "Could not install Thunderbird's private encrypted credential snapshot",
                    ))?;
                } else {
                    match fs::remove_file(target.join(name)) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => {
                            return Err(format!(
                                "Could not update MegaMail's private Thunderbird snapshot: {error}"
                            ));
                        }
                    }
                }
            }
            for database in ["key4.db", "cert9.db"] {
                check_database_journal(source, database)?;
            }
            if credentials_stamp(source)? != expected_stamp {
                return Err("Thunderbird's credential store changed during import. Close Thunderbird and retry.".to_owned());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&stage);
        copy_result
    }

    fn read_bounded_source_file(
        path: &Path,
        limit: u64,
        name: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let before = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Could not inspect Thunderbird's {name}: {error}")),
        };
        if !before.file_type().is_file() || before.len() > limit {
            return Err(format!(
                "Thunderbird's {name} is not a regular file within the allowed size."
            ));
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let mut file = options
            .open(path)
            .map_err(|error| format!("Could not securely open Thunderbird's {name}: {error}"))?;
        let opened = file
            .metadata()
            .map_err(|error| format!("Could not inspect Thunderbird's open {name}: {error}"))?;
        if !opened.file_type().is_file() || opened.len() > limit || !same_file(&before, &opened) {
            return Err(format!(
                "Thunderbird's {name} changed while MegaMail was opening it. Close Thunderbird and retry."
            ));
        }
        let capacity = usize::try_from(opened.len())
            .map_err(|_| format!("Thunderbird's {name} exceeds the allowed size."))?;
        let mut bytes = Vec::with_capacity(capacity);
        (&mut file)
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read Thunderbird's encrypted {name}: {error}"))?;
        let after_fd = file
            .metadata()
            .map_err(|error| format!("Could not verify Thunderbird's open {name}: {error}"))?;
        let after_path = fs::symlink_metadata(path)
            .map_err(|error| format!("Could not verify Thunderbird's {name}: {error}"))?;
        if bytes.len() as u64 > limit
            || bytes.len() as u64 != opened.len()
            || !same_file(&opened, &after_fd)
            || !same_file(&opened, &after_path)
        {
            return Err(format!(
                "Thunderbird's {name} changed while MegaMail was copying it. Close Thunderbird and retry."
            ));
        }
        Ok(Some(bytes))
    }

    fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
        left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.len() == right.len()
            && left.modified().ok() == right.modified().ok()
    }

    fn credentials_stamp(profile: &Path) -> Result<String, String> {
        let mut parts = Vec::new();
        for (name, limit) in [
            ("key4.db", 64 * 1024 * 1024),
            ("logins.json", 64 * 1024 * 1024),
            ("cert9.db", 32 * 1024 * 1024),
        ] {
            let path = profile.join(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if !metadata.file_type().is_file() || metadata.len() > limit {
                        return Err(format!(
                            "Thunderbird's {name} is not a regular file within the allowed size."
                        ));
                    }
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|time| time.as_nanos())
                        .unwrap_or(0);
                    parts.push(format!(
                        "{name}:{}:{}:{}:{modified}",
                        metadata.dev(),
                        metadata.ino(),
                        metadata.len()
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    parts.push(format!("{name}:missing"))
                }
                Err(error) => {
                    return Err(format!("Could not inspect Thunderbird's {name}: {error}"))
                }
            }
        }
        Ok(parts.join("\n"))
    }

    fn source_profile_locked(profile: &Path) -> bool {
        let fcntl_lock = match parent_lock_state(&profile.join(".parentlock")) {
            Ok(state) => state,
            Err(_) => return true,
        };
        if fcntl_lock == Some(true) {
            return true;
        }
        legacy_symlink_lock_active(profile, fcntl_lock)
    }

    /// Mozilla leaves `.parentlock` on disk after a clean exit. Its live state
    /// is the advisory fcntl write lock, so query that lock without creating,
    /// truncating, or otherwise changing the source profile.
    fn parent_lock_state(path: &Path) -> io::Result<Option<bool>> {
        let before = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !before.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Thunderbird's POSIX profile lock is not a regular file",
            ));
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let file = options.open(path)?;
        let opened = file.metadata()?;
        if !opened.file_type().is_file() || !same_file(&before, &opened) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Thunderbird's POSIX profile lock changed while opening",
            ));
        }
        let mut query = libc::flock {
            l_type: libc::F_WRLCK as _,
            l_whence: libc::SEEK_SET as _,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
        };
        // SAFETY: `file` is an open descriptor and `query` is a valid flock
        // structure for F_GETLK. This only queries the lock table.
        if unsafe {
            libc::fcntl(
                std::os::fd::AsRawFd::as_raw_fd(&file),
                libc::F_GETLK,
                &mut query,
            )
        } == -1
        {
            return Err(io::Error::last_os_error());
        }
        let after_path = fs::symlink_metadata(path)?;
        if !same_file(&opened, &after_path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Thunderbird's POSIX profile lock changed while checking",
            ));
        }
        Ok(Some(query.l_type != libc::F_UNLCK as libc::c_short))
    }

    /// Older Unix builds used a `lock` symlink containing an IP address and
    /// PID. Treat malformed/remote signatures as active; only clear a local
    /// dead PID or a `+PID` signature contradicted by an unlocked fcntl file.
    fn legacy_symlink_lock_active(profile: &Path, fcntl_lock: Option<bool>) -> bool {
        let path = profile.join("lock");
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return false,
            Err(_) => return true,
            Ok(metadata) if !metadata.file_type().is_symlink() => return true,
            Ok(_) => {}
        }
        let target = match fs::read_link(&path) {
            Ok(target) => target,
            Err(_) => return true,
        };
        let Some(target) = target.to_str().filter(|target| target.len() <= 1024) else {
            return true;
        };
        let Some((address, pid_text)) = target.split_once(':') else {
            return true;
        };
        let Some(address) = address.parse::<std::net::Ipv4Addr>().ok() else {
            return true;
        };
        if let Some(pid_text) = pid_text.strip_prefix('+') {
            return fcntl_lock != Some(false) || pid_text.parse::<libc::pid_t>().is_err();
        }
        let Ok(pid) = pid_text.parse::<libc::pid_t>() else {
            return true;
        };
        if pid <= 0 || !local_ipv4_addresses().contains(&address) {
            return true;
        }
        // SAFETY: kill(pid, 0) only asks the kernel whether this PID exists.
        unsafe {
            libc::kill(pid, 0) == 0
                || io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        }
    }

    fn local_ipv4_addresses() -> std::collections::HashSet<std::net::Ipv4Addr> {
        let mut addresses = std::collections::HashSet::from([std::net::Ipv4Addr::LOCALHOST]);
        let mut head = std::ptr::null_mut();
        // SAFETY: getifaddrs initializes `head`; each node is traversed before
        // it is released with freeifaddrs below.
        if unsafe { libc::getifaddrs(&mut head) } != 0 {
            return addresses;
        }
        let mut current = head;
        while !current.is_null() {
            // SAFETY: `current` points at a node in the list returned above.
            let interface = unsafe { &*current };
            if !interface.ifa_addr.is_null()
                // SAFETY: the address family is inspected before casting to IPv4.
                && unsafe { (*interface.ifa_addr).sa_family as i32 == libc::AF_INET }
            {
                // SAFETY: AF_INET guarantees this address has sockaddr_in layout.
                let address = unsafe { &*(interface.ifa_addr as *const libc::sockaddr_in) };
                addresses.insert(std::net::Ipv4Addr::from(
                    address.sin_addr.s_addr.to_ne_bytes(),
                ));
            }
            current = interface.ifa_next;
        }
        // SAFETY: `head` is the allocation returned by getifaddrs.
        unsafe { libc::freeifaddrs(head) };
        addresses
    }

    fn check_database_journal(source: &Path, database: &str) -> Result<(), String> {
        for suffix in ["-wal", "-journal", "-shm"] {
            let sidecar = source.join(format!("{database}{suffix}"));
            match fs::symlink_metadata(&sidecar) {
                Ok(metadata) if !metadata.file_type().is_file() => {
                    return Err(format!(
                        "Thunderbird's {database} sidecar is not a regular file."
                    ));
                }
                Ok(metadata) if metadata.len() > 0 => {
                    return Err(format!("Thunderbird's encrypted {database} store has an active {suffix} sidecar. Close Thunderbird, then import its profile again."));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Could not inspect Thunderbird's {database} sidecar: {error}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn reject_symlink_components(path: &Path) -> Result<(), String> {
        let mut prefix = PathBuf::new();
        for component in path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(
                    "Private Thunderbird paths must not contain parent traversal.".to_owned(),
                );
            }
            prefix.push(component.as_os_str());
            match fs::symlink_metadata(&prefix) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(
                        "Private Thunderbird paths must not follow symbolic links.".to_owned()
                    )
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Could not inspect a private Thunderbird path: {error}"
                    ))
                }
            }
        }
        Ok(())
    }

    fn ensure_isolated_clone(source: &Path, target: &Path) -> Result<(), String> {
        reject_symlink_components(target)?;
        let source = fs::canonicalize(source).map_err(io_message(
            "Could not locate the source Thunderbird profile",
        ))?;
        let target = fs::canonicalize(target)
            .map_err(io_message("Could not locate the private Thunderbird clone"))?;
        if source.starts_with(&target) || target.starts_with(&source) {
            return Err(
                "MegaMail's private Thunderbird clone must not overlap the source profile."
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn ensure_private_dir(path: &Path) -> Result<(), String> {
        use std::os::unix::fs::DirBuilderExt;
        reject_symlink_components(path)?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(io_message(
                "Could not create a private Thunderbird runtime directory",
            ))?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(path)
            .map_err(io_message(
                "Could not inspect the private Thunderbird directory",
            ))?;
        let metadata = directory
            .metadata()
            .map_err(io_message("Could not inspect private directory ownership"))?;
        // SAFETY: geteuid has no pointer arguments or side effects.
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err("The private Thunderbird directory belongs to another user.".to_owned());
        }
        directory
            .set_permissions(fs::Permissions::from_mode(0o700))
            .map_err(io_message(
                "Could not secure a private Thunderbird runtime directory",
            ))
    }

    fn validate_private_file_path(path: &Path) -> Result<(), String> {
        reject_symlink_components(path)?;
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                // SAFETY: geteuid is a read-only process identity query.
                if !metadata.file_type().is_file()
                    || metadata.nlink() != 1
                    || metadata.uid() != unsafe { libc::geteuid() }
                {
                    return Err(
                        "A private Thunderbird file is not an owned, singly linked regular file."
                            .to_owned(),
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not inspect a private Thunderbird file: {error}"
                ))
            }
        }
        Ok(())
    }

    fn write_private_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
        validate_private_file_path(path)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(io_message(
            "Could not create a Thunderbird bridge file directory",
        ))?;
        let name = path
            .file_name()
            .ok_or_else(|| "Thunderbird bridge file has no name.".to_owned())?
            .to_string_lossy();
        for _ in 0..32 {
            let serial = PRIVATE_FILE_ID.fetch_add(1, Ordering::Relaxed);
            let temporary = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), serial));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true).mode(0o600);
            let mut file = match options.open(&temporary) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "Could not create a private Thunderbird bridge file: {error}"
                    ))
                }
            };
            if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                let _ = fs::remove_file(&temporary);
                return Err(format!(
                    "Could not save a private Thunderbird bridge file: {error}"
                ));
            }
            drop(file);
            if let Err(error) = fs::rename(&temporary, path) {
                let _ = fs::remove_file(&temporary);
                return Err(format!(
                    "Could not install a private Thunderbird bridge file: {error}"
                ));
            }
            let _ = File::open(parent).and_then(|directory| directory.sync_all());
            return Ok(());
        }
        Err("Could not allocate a private Thunderbird bridge file name.".to_owned())
    }

    fn write_private_bytes_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
        validate_private_file_path(path)?;
        let current = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path);
        if let Ok(mut file) = current {
            let mut current = Vec::new();
            if (&mut file)
                .take(bytes.len() as u64 + 1)
                .read_to_end(&mut current)
                .is_ok()
                && current == bytes
            {
                file.set_permissions(fs::Permissions::from_mode(0o600))
                    .map_err(io_message(
                        "Could not secure an unchanged private Thunderbird file",
                    ))?;
                return Ok(());
            }
        }
        write_private_bytes(path, bytes)
    }

    fn write_new_private_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
        validate_private_file_path(path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_message(
                "Could not create a Thunderbird bridge file directory",
            ))?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut file = options.open(path).map_err(io_message(
            "Could not create a private Thunderbird bridge file",
        ))?;
        if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(format!(
                "Could not save a private Thunderbird bridge file: {error}"
            ));
        }
        Ok(())
    }

    fn new_private_temp_dir() -> Result<PathBuf, String> {
        for _ in 0..20 {
            let path =
                std::env::temp_dir().join(format!("mm-{}-{}", std::process::id(), random_hex(12)?));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(
                        io_message("Could not secure Thunderbird bridge temporary directory"),
                    )?;
                    return Ok(path);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "Could not create Thunderbird bridge temporary directory: {error}"
                    ))
                }
            }
        }
        Err("Could not allocate a private Thunderbird bridge directory.".to_owned())
    }

    fn random_hex(count: usize) -> Result<String, String> {
        let mut bytes = vec![0; count];
        getrandom::getrandom(&mut bytes).map_err(|_| {
            "Could not generate a private Thunderbird bridge identifier.".to_owned()
        })?;
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    fn launcher_script(app: &Path, socket: &Path) -> String {
        format!(
            "#!/bin/sh\nexec {} --thunderbird-host {}\n",
            shell_quote(&app.to_string_lossy()),
            shell_quote(&socket.to_string_lossy())
        )
    }

    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    fn find_thunderbird() -> Result<PathBuf, String> {
        for candidate in [
            PathBuf::from("/usr/bin/thunderbird"),
            PathBuf::from("/usr/lib/thunderbird/thunderbird"),
            PathBuf::from("/snap/bin/thunderbird"),
        ] {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        if let Some(path) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&path) {
                let candidate = directory.join("thunderbird");
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        Err("Thunderbird is not installed or could not be found on PATH.".to_owned())
    }

    fn validate_thunderbird_version(executable: &Path) -> Result<(), String> {
        let child = Command::new(executable)
            .process_group(0)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(io_message("Could not check the Thunderbird version"))?;
        let mut child = ChildGuard {
            child: Some(child),
            isolated_group: true,
        };
        let mut stdout = child
            .child_mut()
            .stdout
            .take()
            .ok_or_else(|| "Missing Thunderbird version output".to_owned())?;
        let mut stderr = child
            .child_mut()
            .stderr
            .take()
            .ok_or_else(|| "Missing Thunderbird version output".to_owned())?;
        for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
            // SAFETY: these are owned pipe descriptors. No other thread reads them.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                return Err("Could not configure bounded Thunderbird version output.".to_owned());
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();
        loop {
            drain_version_output(&mut stdout, &mut output)?;
            drain_version_output(&mut stderr, &mut output)?;
            if child_exited(child.child_mut())
                .map_err(io_message("Could not check Thunderbird version status"))?
            {
                // Terminate the isolated group BEFORE reaping its leader.
                let status = child.terminate();
                drain_version_output(&mut stdout, &mut output)?;
                drain_version_output(&mut stderr, &mut output)?;
                if !status.is_some_and(|status| status.success()) {
                    return Err("Could not verify the installed Thunderbird version.".to_owned());
                }
                return validate_version_output(&String::from_utf8_lossy(&output));
            }
            if Instant::now() >= deadline {
                return Err(bridge_error("unsupported", "Checking the Thunderbird version timed out. MegaMail requires Thunderbird 153 or newer."));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn drain_version_output(reader: &mut impl Read, output: &mut Vec<u8>) -> Result<(), String> {
        let mut buffer = [0u8; 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(size) if output.len() + size <= 16 * 1024 => {
                    output.extend_from_slice(&buffer[..size])
                }
                Ok(_) => {
                    return Err(bridge_error(
                        "unsupported",
                        "Thunderbird version output exceeds its 16 KiB limit.",
                    ))
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err("Could not read Thunderbird version output.".to_owned()),
            }
        }
    }

    fn validate_version_output(output: &str) -> Result<(), String> {
        let major = parse_thunderbird_major(output).ok_or_else(|| bridge_error("unsupported",
            "Could not determine the installed Thunderbird version. MegaMail requires Thunderbird 153 or newer."))?;
        // 153 is intentionally supported; 157 is the current loopback reference,
        // not an upper bound or a claim that every later API was verified.
        if major < 153 {
            return Err(bridge_error(
                "unsupported",
                &format!(
                    "MegaMail requires Thunderbird 153 or newer; this installation is {major}."
                ),
            ));
        }
        Ok(())
    }

    fn parse_thunderbird_major(output: &str) -> Option<u32> {
        output.lines().find_map(|line| {
            let version = line
                .trim()
                .strip_prefix("Mozilla Thunderbird ")
                .or_else(|| line.trim().strip_prefix("Thunderbird "))?;
            let token = version.split_whitespace().next()?;
            if !token
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'a' | 'b' | 'e' | 's' | 'r'))
            {
                return None;
            }
            token.split('.').next()?.parse().ok()
        })
    }

    fn build_extension(host_name: &str) -> Result<Vec<u8>, String> {
        let background = BACKGROUND.replace("__HOST_NAME__", host_name);
        let mut manifest: Value = serde_json::from_str(MANIFEST)
            .map_err(|_| "Could not parse the Thunderbird bridge manifest.".to_owned())?;
        let base_version = manifest
            .get("version")
            .and_then(Value::as_str)
            .ok_or_else(|| "Thunderbird bridge manifest has no version.".to_owned())?;
        let version =
            extension_version(MANIFEST, base_version, &background, SYNC_SCHEMA, SYNC_IMPL);
        manifest["version"] = Value::String(version);
        manifest["applications"]["gecko"]["id"] = Value::String(EXTENSION_ID.to_owned());
        let manifest = serde_json::to_string(&manifest)
            .map_err(|_| "Could not encode the Thunderbird bridge manifest.".to_owned())?;
        let files = [
            ("manifest.json", manifest),
            ("background.js", background),
            ("api/schema.json", SYNC_SCHEMA.to_owned()),
            ("api/implementation.js", SYNC_IMPL.to_owned()),
        ];
        stored_zip(&files)
    }

    fn extension_version(
        manifest_template: &str,
        base: &str,
        background: &str,
        schema: &str,
        implementation: &str,
    ) -> String {
        let mut digest = Sha256::new();
        for contents in [manifest_template, background, schema, implementation] {
            digest.update(contents.as_bytes());
            digest.update([0]);
        }
        let digest = digest.finalize();
        let revision = digest[..8]
            .iter()
            .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
        let prefix = base.split('.').take(2).collect::<Vec<_>>().join(".");
        let first = (revision >> 30) % 1_000_000_000;
        let second = (revision & ((1_u64 << 30) - 1)) % 1_000_000_000;
        format!("{prefix}.{first}.{second}")
    }

    fn stored_zip(files: &[(&str, String)]) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();
        let mut central = Vec::new();
        for (name, contents) in files {
            let name_bytes = name.as_bytes();
            let data = contents.as_bytes();
            let size = u32::try_from(data.len())
                .map_err(|_| "Thunderbird bridge extension file is too large.".to_owned())?;
            let offset = u32::try_from(output.len())
                .map_err(|_| "Thunderbird bridge extension is too large.".to_owned())?;
            let crc = crc32(data);
            push_u32(&mut output, 0x0403_4b50);
            push_u16(&mut output, 20);
            push_u16(&mut output, 0);
            push_u16(&mut output, 0);
            push_u16(&mut output, 0);
            push_u16(&mut output, 33);
            push_u32(&mut output, crc);
            push_u32(&mut output, size);
            push_u32(&mut output, size);
            push_u16(&mut output, name_bytes.len() as u16);
            push_u16(&mut output, 0);
            output.extend_from_slice(name_bytes);
            output.extend_from_slice(data);

            push_u32(&mut central, 0x0201_4b50);
            push_u16(&mut central, 20);
            push_u16(&mut central, 20);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 33);
            push_u32(&mut central, crc);
            push_u32(&mut central, size);
            push_u32(&mut central, size);
            push_u16(&mut central, name_bytes.len() as u16);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, offset);
            central.extend_from_slice(name_bytes);
        }
        let central_offset = u32::try_from(output.len())
            .map_err(|_| "Thunderbird bridge extension is too large.".to_owned())?;
        let central_size = u32::try_from(central.len())
            .map_err(|_| "Thunderbird bridge extension is too large.".to_owned())?;
        output.extend_from_slice(&central);
        push_u32(&mut output, 0x0605_4b50);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, files.len() as u16);
        push_u16(&mut output, files.len() as u16);
        push_u32(&mut output, central_size);
        push_u32(&mut output, central_offset);
        push_u16(&mut output, 0);
        Ok(output)
    }

    fn push_u16(output: &mut Vec<u8>, value: u16) {
        output.extend_from_slice(&value.to_le_bytes());
    }
    fn push_u32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }

    fn encode_request(request: &Value) -> Result<Vec<u8>, String> {
        struct BoundedBuffer(Vec<u8>);
        impl Write for BoundedBuffer {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if bytes.len() > MAX_FRAME.saturating_sub(self.0.len()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "request frame limit",
                    ));
                }
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut buffer = BoundedBuffer(Vec::new());
        serde_json::to_writer(&mut buffer, request).map_err(|_| "Thunderbird bridge request exceeded the 1 MiB protocol frame limit or could not be encoded.".to_owned())?;
        Ok(buffer.0)
    }

    fn wait_socket(
        stream: &UnixStream,
        events: libc::c_short,
        deadline: Option<Instant>,
    ) -> io::Result<()> {
        loop {
            let timeout = match deadline {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "bridge deadline expired",
                        ));
                    }
                    remaining.as_millis().clamp(1, i32::MAX as u128) as i32
                }
                None => -1,
            };
            let mut descriptor = libc::pollfd {
                fd: stream.as_raw_fd(),
                events,
                revents: 0,
            };
            // SAFETY: descriptor references our open socket for the whole call.
            let result = unsafe { libc::poll(&mut descriptor, 1, timeout) };
            if result > 0 {
                return Ok(());
            }
            if result == 0 {
                continue;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    struct FrameReader<'a> {
        stream: &'a UnixStream,
        deadline: Option<Instant>,
    }

    impl Read for FrameReader<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if bytes.is_empty() {
                return Ok(0);
            }
            loop {
                wait_socket(self.stream, libc::POLLIN, self.deadline)?;
                // MSG_DONTWAIT is per-call: changing O_NONBLOCK would also
                // affect the writer's duplicated socket descriptor.
                // SAFETY: bytes is a writable slice and the socket is live.
                let size = unsafe {
                    libc::recv(
                        self.stream.as_raw_fd(),
                        bytes.as_mut_ptr().cast(),
                        bytes.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if size >= 0 {
                    if size > 0 && self.deadline.is_none() {
                        self.deadline = Some(Instant::now() + FRAME_TIMEOUT);
                    }
                    return Ok(size as usize);
                }
                let error = io::Error::last_os_error();
                if !matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) {
                    return Err(error);
                }
            }
        }
    }

    fn read_runtime_json_frame(
        stream: &mut UnixStream,
        deadline: Option<Instant>,
    ) -> Result<Value, String> {
        let mut reader = FrameReader { stream, deadline };
        let bytes = read_frame_with_limit(&mut reader, MAX_EXTENSION_FRAME)
            .map_err(|message| {
                bridge_error(
                    "protocol",
                    &format!("Thunderbird bridge disconnected: {message}"),
                )
            })?
            .ok_or_else(|| {
                bridge_error("disconnected", "Thunderbird bridge closed the connection.")
            })?;
        serde_json::from_slice(&bytes).map_err(|_| {
            bridge_error(
                "protocol",
                "Thunderbird bridge disconnected after invalid JSON.",
            )
        })
    }

    fn write_runtime_frame(
        stream: &mut UnixStream,
        bytes: &[u8],
        deadline: Instant,
    ) -> Result<(), String> {
        if bytes.len() > MAX_FRAME {
            return Err("Thunderbird request exceeds the protocol frame limit.".to_owned());
        }
        let header = (bytes.len() as u32).to_le_bytes();
        let mut transferred = 0usize;
        for part in [&header[..], bytes] {
            let mut remaining = part;
            while !remaining.is_empty() {
                if let Err(error) = wait_socket(stream, libc::POLLOUT, Some(deadline)) {
                    return Err(
                        if transferred == 0 && error.kind() == io::ErrorKind::TimedOut {
                            bridge_error(
                                "request-timeout",
                                "Thunderbird request writer timed out before sending the request.",
                            )
                        } else {
                            bridge_error("disconnected", "Thunderbird bridge disconnected while writing a request frame. Its operation may be incomplete.")
                        },
                    );
                }
                // SAFETY: remaining is readable and the socket is live.
                // Suppress SIGPIPE without changing the process-wide handler.
                let size = unsafe {
                    libc::send(
                        stream.as_raw_fd(),
                        remaining.as_ptr().cast(),
                        remaining.len(),
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if size > 0 {
                    transferred += size as usize;
                    remaining = &remaining[size as usize..];
                } else if size == 0 {
                    return Err(bridge_error(
                        "disconnected",
                        "Thunderbird bridge disconnected during a request write.",
                    ));
                } else {
                    let error = io::Error::last_os_error();
                    if !matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) {
                        return Err(bridge_error(
                            "disconnected",
                            "Thunderbird bridge disconnected during a request write.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn read_json_frame(reader: &mut impl Read) -> Result<Value, String> {
        let bytes = read_frame_with_limit(reader, MAX_EXTENSION_FRAME)?
            .ok_or_else(|| "Thunderbird bridge closed the connection.".to_owned())?;
        serde_json::from_slice(&bytes)
            .map_err(|_| "Thunderbird bridge returned invalid JSON.".to_owned())
    }

    fn write_frame_with_limit(
        writer: &mut impl Write,
        bytes: &[u8],
        limit: usize,
    ) -> Result<(), String> {
        if bytes.len() > limit {
            return Err("Thunderbird bridge frame exceeded its protocol limit.".to_owned());
        }
        writer
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .and_then(|_| writer.write_all(bytes))
            .map_err(io_message("Could not write Thunderbird bridge frame"))
    }

    fn read_frame_with_limit(
        reader: &mut impl Read,
        limit: usize,
    ) -> Result<Option<Vec<u8>>, String> {
        let mut header = [0u8; 4];
        let mut count = 0;
        while count < header.len() {
            match reader.read(&mut header[count..]) {
                Ok(0) if count == 0 => return Ok(None),
                Ok(0) => return Err("Thunderbird bridge sent a truncated frame header.".to_owned()),
                Ok(bytes) => count += bytes,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(format!("Could not read Thunderbird bridge frame: {error}"))
                }
            }
        }
        let size = u32::from_le_bytes(header) as usize;
        if size > limit {
            return Err("Thunderbird bridge sent a frame over its protocol limit.".to_owned());
        }
        let mut bytes = vec![0u8; size];
        reader
            .read_exact(&mut bytes)
            .map_err(io_message("Thunderbird bridge sent a truncated frame"))?;
        Ok(Some(bytes))
    }

    fn io_message(context: &'static str) -> impl FnOnce(io::Error) -> String {
        move |error| format!("{context}: {error}")
    }

    #[cfg(test)]
    mod runtime_batch2 {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/runtime_batch2/contract.rs"
        ));
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::process::{Child, Command, Stdio};
        use std::time::Duration;

        struct ChildProcess(Child);

        impl Drop for ChildProcess {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        #[test]
        fn native_frames_are_little_endian_and_reject_oversized_payloads() {
            let mut frame = Vec::new();
            write_frame_with_limit(&mut frame, b"{} ", MAX_FRAME).unwrap();
            assert_eq!(&frame[..4], &[3, 0, 0, 0]);
            assert_eq!(
                read_frame_with_limit(&mut frame.as_slice(), MAX_FRAME).unwrap(),
                Some(b"{} ".to_vec())
            );
            assert!(
                write_frame_with_limit(&mut Vec::new(), &vec![0; MAX_FRAME + 1], MAX_FRAME)
                    .is_err()
            );
        }

        #[test]
        fn bridge_routes_out_of_order_replies_and_ignores_late_replies() {
            let pending = Mutex::new(HashMap::new());
            let (first_tx, first_rx) = mpsc::sync_channel(1);
            let (second_tx, second_rx) = mpsc::sync_channel(1);
            pending.lock().unwrap().insert(
                1,
                PendingReply {
                    sender: first_tx,
                    deadline: Instant::now() + Duration::from_secs(1),
                },
            );
            pending.lock().unwrap().insert(
                2,
                PendingReply {
                    sender: second_tx,
                    deadline: Instant::now() + Duration::from_secs(1),
                },
            );

            route_response(json!({ "id": 2, "result": "second" }), &pending).unwrap();
            route_response(json!({ "id": 1, "result": "first" }), &pending).unwrap();
            assert_eq!(first_rx.recv().unwrap().unwrap().as_str(), Some("first"));
            assert_eq!(second_rx.recv().unwrap().unwrap().as_str(), Some("second"));

            route_response(json!({ "id": 3, "result": "late" }), &pending).unwrap();
            assert!(pending.lock().unwrap().is_empty());
        }

        #[test]
        fn body_call_finishes_while_metadata_call_is_still_pending() {
            let root = std::env::temp_dir().join(format!(
                "megamail-bridge-multiplex-test-{}",
                random_hex(8).unwrap()
            ));
            fs::create_dir(&root).unwrap();
            let listener = UnixListener::bind(root.join("bridge.sock")).unwrap();
            let (metadata_seen_tx, metadata_seen_rx) = mpsc::sync_channel(1);
            let (release_metadata_tx, release_metadata_rx) = mpsc::sync_channel(1);
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let first = read_json_frame(&mut stream).unwrap();
                assert_eq!(first["method"], "conversation");
                metadata_seen_tx.send(()).unwrap();
                let second = read_json_frame(&mut stream).unwrap();
                assert_eq!(second["method"], "body");
                let body_reply = json!({ "id": second["id"], "result": "body-ready" });
                write_frame_with_limit(
                    &mut stream,
                    &serde_json::to_vec(&body_reply).unwrap(),
                    MAX_EXTENSION_FRAME,
                )
                .unwrap();
                release_metadata_rx.recv().unwrap();
                let metadata_reply = json!({ "id": first["id"], "result": "metadata-ready" });
                write_frame_with_limit(
                    &mut stream,
                    &serde_json::to_vec(&metadata_reply).unwrap(),
                    MAX_EXTENSION_FRAME,
                )
                .unwrap();
                let shutdown = read_json_frame(&mut stream).unwrap();
                assert_eq!(shutdown["method"], "shutdown");
                let response = json!({ "id": shutdown["id"], "result": { "stopping": true } });
                write_frame_with_limit(
                    &mut stream,
                    &serde_json::to_vec(&response).unwrap(),
                    MAX_EXTENSION_FRAME,
                )
                .unwrap();
            });
            let stream = UnixStream::connect(root.join("bridge.sock")).unwrap();
            let runtime = Runtime::from_connected(
                stream,
                ChildGuard {
                    child: None,
                    isolated_group: false,
                },
                File::create(root.join("profile.lock")).unwrap(),
                root.join("host.json"),
                root.clone(),
                root.clone(),
            )
            .unwrap();

            let (metadata_result_tx, metadata_result_rx) = mpsc::sync_channel(1);
            let metadata_runtime = runtime.clone();
            std::thread::spawn(move || {
                metadata_result_tx
                    .send(metadata_runtime.call("conversation", json!({})))
                    .unwrap();
            });
            metadata_seen_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap();
            let (body_result_tx, body_result_rx) = mpsc::sync_channel(1);
            let body_runtime = runtime.clone();
            std::thread::spawn(move || {
                body_result_tx
                    .send(body_runtime.call("body", json!({})))
                    .unwrap();
            });

            assert_eq!(
                body_result_rx
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .unwrap()
                    .as_str(),
                Some("body-ready")
            );
            release_metadata_tx.send(()).unwrap();
            assert_eq!(
                metadata_result_rx
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .unwrap()
                    .as_str(),
                Some("metadata-ready")
            );
            drop(runtime);
            server.join().unwrap();
            assert!(!root.exists());
        }

        #[test]
        fn timed_out_request_can_be_removed_without_breaking_transport() {
            let pending = Mutex::new(HashMap::new());
            let (reply_tx, _reply_rx) = mpsc::sync_channel(1);
            pending.lock().unwrap().insert(
                7,
                PendingReply {
                    sender: reply_tx,
                    deadline: Instant::now() + Duration::from_secs(1),
                },
            );
            let broken = AtomicBool::new(false);

            assert!(expire_pending(&pending, 7));
            route_response(json!({ "id": 7, "result": "late" }), &pending).unwrap();
            assert!(!broken.load(Ordering::Acquire));
            assert!(pending.lock().unwrap().is_empty());
        }

        #[test]
        fn request_deadlines_are_bounded_by_operation_kind() {
            assert_eq!(operation_timeout("conversation"), Duration::from_secs(15));
            assert_eq!(operation_timeout("list"), Duration::from_secs(15));
            assert_eq!(operation_timeout("body"), Duration::from_secs(30));
            assert_eq!(operation_timeout("folders"), Duration::from_secs(90));
            assert_eq!(operation_timeout("refresh"), Duration::from_secs(165));
            assert!(operation_timeout_message("send").contains("check Sent and Outbox"));
        }

        #[test]
        fn native_host_name_is_stable_and_scoped_to_the_private_clone() {
            let clone = Path::new(
                "/home/user/.local/share/megamail/thunderbird/profiles/profile-hash/credential-hash",
            );
            let same_clone = Path::new(
                "/home/user/.local/share/megamail/thunderbird/profiles/profile-hash/credential-hash",
            );
            let isolated_clone = Path::new(
                "/tmp/megamail-qa/share/megamail/thunderbird/profiles/profile-hash/credential-hash",
            );
            assert_eq!(bridge_host_name(clone), bridge_host_name(same_clone));
            assert_ne!(bridge_host_name(clone), bridge_host_name(isolated_clone));
            assert!(
                build_extension(&bridge_host_name(clone)).unwrap()
                    == build_extension(&bridge_host_name(same_clone)).unwrap()
            );
        }

        #[test]
        fn private_profile_files_are_not_replaced_when_content_is_unchanged() {
            let root = std::env::temp_dir().join(format!(
                "megamail-bridge-write-test-{}",
                random_hex(8).unwrap()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("profile.js");
            write_private_bytes_if_changed(&path, b"stable").unwrap();
            let before = fs::metadata(&path).unwrap().ino();
            write_private_bytes_if_changed(&path, b"stable").unwrap();
            assert_eq!(fs::metadata(&path).unwrap().ino(), before);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn generated_xpi_is_valid_stored_zip_with_private_host_name() {
            let xpi = build_extension("com.megamail.bridge.test123").unwrap();
            assert_eq!(&xpi[..4], b"PK\x03\x04");
            assert!(xpi
                .windows(b"com.megamail.bridge.test123".len())
                .any(|window| window == b"com.megamail.bridge.test123"));
            let version = extension_version(
                MANIFEST,
                "0.1.1",
                &BACKGROUND.replace("__HOST_NAME__", "com.megamail.bridge.test123"),
                SYNC_SCHEMA,
                SYNC_IMPL,
            );
            let encoded_version = format!("\"version\":\"{version}\"");
            assert!(xpi
                .windows(encoded_version.len())
                .any(|window| window == encoded_version.as_bytes()));
            assert!(!xpi.windows(12).any(|window| window == b"__HOST_NAME__"));
        }

        #[test]
        fn extension_version_changes_when_packaged_code_changes() {
            let stable =
                extension_version(MANIFEST, "0.1.1", "background", "schema", "implementation");
            assert_eq!(
                stable,
                extension_version(MANIFEST, "0.1.1", "background", "schema", "implementation")
            );
            assert_ne!(
                stable,
                extension_version(
                    MANIFEST,
                    "0.1.1",
                    "changed background",
                    "schema",
                    "implementation"
                )
            );
        }

        #[test]
        fn shell_wrapper_quotes_paths_with_spaces_and_single_quotes() {
            let result =
                launcher_script(Path::new("/tmp/app's binary"), Path::new("/tmp/a socket"));
            assert!(result.contains("'/tmp/app'\\''s binary'"));
            assert!(result.contains("'/tmp/a socket'"));
        }

        #[test]
        fn server_safety_preferences_use_thunderbirds_actual_pref_names() {
            let preferences = server_safety_preferences("server1");
            for key in [
                "check_new_mail",
                "download_on_biff",
                "login_at_startup",
                "empty_trash_on_exit",
                "cleanup_inbox_on_exit",
            ] {
                assert!(preferences.iter().any(|line| {
                    line == &pref_line(&format!("mail.server.server1.{key}"), "false")
                }));
            }
            assert!(!preferences
                .iter()
                .any(|line| line.contains("downloadOnBiff")));
            assert!(!preferences
                .iter()
                .any(|line| line.contains("loginAtStartup")));
        }

        #[test]
        fn thunderbird_version_guard_parses_mozilla_version_output() {
            assert_eq!(
                parse_thunderbird_major("Mozilla Thunderbird 157.0.1"),
                Some(157)
            );
            assert_eq!(parse_thunderbird_major("Thunderbird 115.9.0esr"), Some(115));
            assert_eq!(parse_thunderbird_major("Thunderbird"), None);
        }

        #[test]
        fn credential_snapshot_reads_are_bounded_and_reject_symlinks() {
            let root = std::env::temp_dir().join(format!(
                "megamail-credential-copy-test-{}",
                random_hex(8).unwrap()
            ));
            fs::create_dir(&root).unwrap();
            let original = root.join("key4.db");
            fs::write(&original, b"safe-test-bytes").unwrap();
            assert_eq!(
                read_bounded_source_file(&original, 64, "key4.db").unwrap(),
                Some(b"safe-test-bytes".to_vec())
            );
            assert!(read_bounded_source_file(&original, 4, "key4.db").is_err());

            let symlink = root.join("logins.json");
            std::os::unix::fs::symlink(&original, &symlink).unwrap();
            assert!(read_bounded_source_file(&symlink, 64, "logins.json").is_err());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn persistent_unlocked_parentlock_and_stale_symlinks_are_not_active_locks() {
            let root = std::env::temp_dir().join(format!(
                "megamail-parent-lock-test-{}",
                random_hex(8).unwrap()
            ));
            fs::create_dir(&root).unwrap();
            fs::write(root.join(".parentlock"), b"").unwrap();
            assert!(!source_profile_locked(&root));

            std::os::unix::fs::symlink("127.0.0.1:+2147483647", root.join("lock")).unwrap();
            assert!(!source_profile_locked(&root));
            fs::remove_file(root.join("lock")).unwrap();

            std::os::unix::fs::symlink("127.0.0.1:2147483647", root.join("lock")).unwrap();
            assert!(!source_profile_locked(&root));
            fs::remove_file(root.join("lock")).unwrap();

            std::os::unix::fs::symlink("192.0.2.1:2147483647", root.join("lock")).unwrap();
            assert!(
                source_profile_locked(&root),
                "remote-host locks fail closed"
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn active_parentlock_fcntl_write_lock_is_detected_read_only() {
            let root = std::env::temp_dir().join(format!(
                "megamail-active-parent-lock-test-{}",
                random_hex(8).unwrap()
            ));
            fs::create_dir(&root).unwrap();
            fs::write(root.join(".parentlock"), b"").unwrap();

            let mut child = ChildProcess(
                Command::new(std::env::current_exe().unwrap())
                    .arg("parent_lock_child_holds_fcntl_lock")
                    .arg("--nocapture")
                    .env("MEGAMAIL_PARENT_LOCK_TEST_PATH", &root)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let ready = root.join("child-ready");
            for _ in 0..200 {
                if ready.exists() {
                    break;
                }
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "lock-holder test process exited before taking the lock"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(ready.exists(), "child acquired its fcntl lock");
            assert!(source_profile_locked(&root));
            drop(child);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn parent_lock_child_holds_fcntl_lock() {
            let Ok(directory) = std::env::var("MEGAMAIL_PARENT_LOCK_TEST_PATH") else {
                return;
            };
            let path = Path::new(&directory).join(".parentlock");
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap();
            let mut lock = libc::flock {
                l_type: libc::F_WRLCK as _,
                l_whence: libc::SEEK_SET as _,
                l_start: 0,
                l_len: 0,
                l_pid: 0,
            };
            // SAFETY: this test-only child holds a write lock on its private
            // fixture until the parent terminates the process.
            assert_eq!(
                unsafe {
                    libc::fcntl(
                        std::os::fd::AsRawFd::as_raw_fd(&file),
                        libc::F_SETLK,
                        &mut lock,
                    )
                },
                0
            );
            fs::write(Path::new(&directory).join("child-ready"), b"locked").unwrap();
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

#[cfg(unix)]
pub use unix::{run_native_host, Runtime};

#[cfg(not(unix))]
pub struct Runtime;

#[cfg(not(unix))]
impl Runtime {
    pub fn start(_: &crate::thunderbird::ThunderbirdProfile) -> Result<Self, String> {
        Err("Thunderbird's native messaging bridge currently requires Unix sockets.".to_owned())
    }
    pub fn call(&self, _: &str, _: serde_json::Value) -> Result<serde_json::Value, String> {
        Err("Thunderbird's native messaging bridge currently requires Unix sockets.".to_owned())
    }
}

#[cfg(not(unix))]
pub fn run_native_host(_: &Path) -> Result<(), String> {
    Err("Thunderbird's native messaging bridge currently requires Unix sockets.".to_owned())
}
