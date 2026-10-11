use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

/// Keep the returned file alive while workers run: a second process must not
/// sync or retry the same Outbox concurrently.
pub fn acquire(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(file)
}

/// Linux activation uses real kernel peer credentials. The native package must
/// depend directly on `libc`; a transitive dependency is not an authentication API.
#[cfg(target_os = "linux")]
pub use activation::{ActivationError, ActivationInbox, ActivationStartup, start_activation};

#[cfg(target_os = "linux")]
mod activation {
    use super::*;
    use megamail_core::desktop::{MAILTO_URI_LIMIT, MailtoError, MailtoIntent, parse_mailto};
    use std::{
        collections::VecDeque,
        ffi::CString,
        fs,
        io::{Read, Seek, SeekFrom, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{
                ffi::OsStrExt,
                fs::{DirBuilderExt, FileTypeExt, MetadataExt},
                net::{UnixListener, UnixStream},
            },
        },
        path::{Component, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    const CAPACITY: usize = 8;
    const REQUEST_BUDGET: Duration = Duration::from_millis(750);
    const START_BUDGET: Duration = Duration::from_secs(2);
    const TICK: Duration = Duration::from_millis(25);
    const SOCKET: &str = "compose.sock";
    const MAGIC: [u8; 4] = *b"MMA1";
    const ACCEPTED: u8 = 0;
    const BUSY: u8 = 1;
    const INVALID: u8 = 2;

    /// Errors never include the URI, body, recipients, or filesystem paths.
    #[derive(Debug)]
    pub enum ActivationError {
        Security,
        AlreadyRunning,
        InvalidIntent(MailtoError),
        Busy,
        Unavailable,
        /// A complete request may have been queued. Do not automatically retry.
        HandoffUncertain,
        Io(io::Error),
    }

    impl std::fmt::Display for ActivationError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                Self::Security => "The private activation endpoint failed security checks.",
                Self::AlreadyRunning => "MegaMail is already using this profile.",
                Self::InvalidIntent(_) => "This external compose link is invalid or unsupported.",
                Self::Busy => "MegaMail has eight pending compose links. Finish or close a composer first.",
                Self::Unavailable => "The running MegaMail activation endpoint is unavailable.",
                Self::HandoffUncertain => "MegaMail may have received this compose link. Check its composer before retrying.",
                Self::Io(_) => "MegaMail could not start the private activation transport.",
            })
        }
    }
    impl std::error::Error for ActivationError {}
    impl From<io::Error> for ActivationError {
        fn from(error: io::Error) -> Self {
            Self::Io(error)
        }
    }

    /// Only Primary may create workers. Forwarded must exit immediately.
    pub enum ActivationStartup {
        Primary(ActivationInbox),
        Forwarded,
    }

    /// Holds the original profile lock for its entire lifetime. Not cloneable.
    /// Drop cancels/joins the one I/O thread and removes only its own socket inode.
    pub struct ActivationInbox {
        queue: Arc<Mutex<VecDeque<MailtoIntent>>>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
        _profile_lock: File,
        _profile_directory: File,
    }

    impl ActivationInbox {
        /// Non-I/O UI seam. Leave `ready` false during setup, account selection,
        /// any existing composer/modal, or send/save uncertainty. Choose a usable
        /// explicit account BEFORE taking; the URI never selects a sender.
        pub fn take_if_ready(&self, ready: bool) -> Option<MailtoIntent> {
            if !ready {
                return None;
            }
            self.queue.lock().ok()?.pop_front()
        }
    }
    impl Drop for ActivationInbox {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// Call before starting GPUI, or on a background executor, never on its UI
    /// thread. `profile_dir` is the app-selected private config directory, NOT a
    /// URI/CLI-supplied socket path. The existing `acquire` lock is mandatory.
    /// Initial cold links and received running-instance links use parse_mailto
    /// exactly once, in the accepting primary, before queue admission.
    pub fn start_activation(
        profile_dir: &Path,
        initial_uri: Option<&str>,
    ) -> Result<ActivationStartup, ActivationError> {
        let directory = open_profile(profile_dir)?;
        let lock_path = pinned(&directory).join("instance.lock");
        match fs::symlink_metadata(&lock_path) {
            Ok(metadata) => validate_file(&metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        use std::os::unix::fs::OpenOptionsExt;
        // Open the actual lock inode without following a replaced symlink, then
        // reuse acquire through its pinned fd alias. The original GTK-compatible
        // acquire implementation and its automatic File lifetime stay unchanged.
        let lock_inode = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lock_path)?;
        validate_file(&lock_inode.metadata()?)?;
        if identity(&lock_inode.metadata()?) != identity(&fs::symlink_metadata(&lock_path)?) {
            return Err(ActivationError::Security);
        }
        let lock_alias = PathBuf::from(format!("/proc/self/fd/{}", lock_inode.as_raw_fd()));
        let mut lock = match acquire(&lock_alias) {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                let uri = initial_uri.ok_or(ActivationError::AlreadyRunning)?;
                handoff(&directory, uri)?;
                return Ok(ActivationStartup::Forwarded);
            }
            Err(error) => return Err(error.into()),
        };
        validate_file(&lock.metadata()?)?;
        if identity(&lock.metadata()?) != identity(&fs::symlink_metadata(&lock_path)?) {
            return Err(ActivationError::Security);
        }
        let mut pending = VecDeque::new();
        if let Some(uri) = initial_uri {
            pending.push_back(parse_mailto(uri).map_err(ActivationError::InvalidIntent)?);
        }
        // Owner PID binds a client-side SO_PEERCRED check to this lock owner.
        // This is not a secret or a trust boundary against malicious same-UID code.
        lock.seek(SeekFrom::Start(0))?;
        lock.write_all(&MAGIC)?;
        lock.write_all(&std::process::id().to_be_bytes())?;
        lock.set_len(8)?;
        let endpoint_directory = endpoint_dir(&directory, true)?;
        let path = pinned(&endpoint_directory).join(SOCKET);
        remove_stale(&path)?; // Only reached by the successful profile-lock owner.
        let listener = UnixListener::bind(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        let cleanup = SocketCleanup {
            directory: endpoint_directory,
            profile: directory.try_clone()?,
            inode: identity(&metadata),
        };
        // The enclosing directory was already private while bind used the umask.
        fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
        validate_socket(&fs::symlink_metadata(&path)?)?;
        listener.set_nonblocking(true)?;
        let queue = Arc::new(Mutex::new(pending));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_queue = Arc::clone(&queue);
        let thread_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("megamail-activation".into())
            .spawn(move || {
                serve(listener, &cleanup, &thread_queue, &thread_stop);
            })?;
        Ok(ActivationStartup::Primary(ActivationInbox {
            queue,
            stop,
            thread: Some(thread),
            _profile_lock: lock,
            _profile_directory: directory,
        }))
    }

    fn uid() -> u32 {
        // SAFETY: geteuid has no pointer arguments or side effects.
        unsafe { libc::geteuid() }
    }
    fn identity(metadata: &fs::Metadata) -> (u64, u64) {
        (metadata.dev(), metadata.ino())
    }
    fn private(metadata: &fs::Metadata) -> bool {
        metadata.uid() == uid() && metadata.mode() & 0o077 == 0
    }
    fn validate_file(metadata: &fs::Metadata) -> Result<(), ActivationError> {
        if !metadata.is_file() || !private(metadata) || metadata.nlink() != 1 {
            return Err(ActivationError::Security);
        }
        Ok(())
    }
    fn validate_socket(metadata: &fs::Metadata) -> Result<(), ActivationError> {
        if !metadata.file_type().is_socket() || !private(metadata) || metadata.nlink() != 1 {
            return Err(ActivationError::Security);
        }
        Ok(())
    }
    fn pinned(directory: &File) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
    }
    fn open_child(parent: &File, name: &std::ffi::OsStr) -> Result<File, ActivationError> {
        let name = CString::new(name.as_bytes()).map_err(|_| ActivationError::Security)?;
        // SAFETY: name is a live NUL-terminated component; flags forbid symlinks,
        // require a directory, and prevent fd inheritance. The returned fd is owned.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: openat returned a new owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    fn open_profile(path: &Path) -> Result<File, ActivationError> {
        if !path.is_absolute() {
            return Err(ActivationError::Security);
        }
        let mut directory = File::open("/")?;
        for component in path.components() {
            match component {
                Component::RootDir => continue,
                Component::Normal(name) => directory = open_child(&directory, name)?,
                _ => return Err(ActivationError::Security),
            }
            let metadata = directory.metadata()?;
            // Do not traverse writable foreign parents. A root-owned sticky /tmp
            // is safe for an exclusively created private synthetic profile.
            if (metadata.uid() != 0 && metadata.uid() != uid())
                || (metadata.mode() & 0o022 != 0
                    && !(metadata.uid() == 0 && metadata.mode() & libc::S_ISVTX != 0))
            {
                return Err(ActivationError::Security);
            }
        }
        if !private(&directory.metadata()?) {
            return Err(ActivationError::Security);
        }
        Ok(directory)
    }
    fn endpoint_dir(profile: &File, create: bool) -> Result<File, ActivationError> {
        let path = pinned(profile).join("activation");
        if create {
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error.into()),
            }
        }
        let directory = open_child(profile, std::ffi::OsStr::new("activation"))?;
        if !private(&directory.metadata()?) {
            return Err(ActivationError::Security);
        }
        Ok(directory)
    }
    struct SocketCleanup {
        directory: File,
        profile: File,
        inode: (u64, u64),
    }
    impl SocketCleanup {
        fn valid_location(&self) -> bool {
            let Ok(profile) = self.profile.metadata() else {
                return false;
            };
            let Ok(directory) = self.directory.metadata() else {
                return false;
            };
            let Ok(linked) = fs::symlink_metadata(pinned(&self.profile).join("activation")) else {
                return false;
            };
            private(&profile)
                && private(&directory)
                && linked.is_dir()
                && identity(&linked) == identity(&directory)
        }
        fn valid_socket(&self) -> bool {
            self.valid_location()
                && fs::symlink_metadata(pinned(&self.directory).join(SOCKET)).is_ok_and(
                    |metadata| {
                        validate_socket(&metadata).is_ok() && identity(&metadata) == self.inode
                    },
                )
        }
    }
    impl Drop for SocketCleanup {
        fn drop(&mut self) {
            if self.valid_socket() {
                let _ = fs::remove_file(pinned(&self.directory).join(SOCKET));
            }
        }
    }
    fn remove_stale(path: &Path) -> Result<(), ActivationError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        validate_socket(&metadata)?;
        // Never unlink a connectable/busy/foreign endpoint, even with the lock.
        match connect(
            path,
            Instant::now() + REQUEST_BUDGET,
            &AtomicBool::new(false),
        ) {
            Err(error) if error.raw_os_error() == Some(libc::ECONNREFUSED) => (),
            _ => return Err(ActivationError::Security),
        }
        let current = fs::symlink_metadata(path)?;
        validate_socket(&current)?;
        if identity(&current) != identity(&metadata) {
            return Err(ActivationError::Security);
        }
        fs::remove_file(path)?;
        Ok(())
    }
    fn peer(stream: &UnixStream) -> io::Result<libc::ucred> {
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: credentials is correctly aligned/writable for exactly length
        // bytes, length is writable, and stream's descriptor stays live.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if length as usize != std::mem::size_of::<libc::ucred>()
            || credentials.pid <= 0
            || credentials.uid != uid()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Activation peer rejected",
            ));
        }
        Ok(credentials)
    }
    fn wait(
        fd: i32,
        events: libc::c_short,
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        loop {
            if stop.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Activation cancelled",
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Activation timed out",
                ));
            }
            let mut descriptor = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            let milliseconds = remaining.min(TICK).as_millis().max(1) as i32;
            // SAFETY: descriptor is one live writable pollfd; timeout is bounded.
            let result = unsafe { libc::poll(&mut descriptor, 1, milliseconds) };
            if result > 0 {
                return Ok(());
            }
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
    }
    fn connect(path: &Path, deadline: Instant, stop: &AtomicBool) -> io::Result<UnixStream> {
        let bytes = path.as_os_str().as_bytes();
        // SAFETY: zero is a valid initial representation of sockaddr_un.
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        if bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Activation path rejected",
            ));
        }
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (target, source) in address.sun_path.iter_mut().zip(bytes) {
            *target = *source as libc::c_char;
        }
        // SAFETY: socket takes no pointers; returned fd is newly owned.
        let fd = unsafe {
            libc::socket(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is a new AF_UNIX stream descriptor, owned here.
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        // SAFETY: address is live, correctly sized, NUL-terminated, and AF_UNIX.
        let result = unsafe {
            libc::connect(
                fd,
                (&address as *const libc::sockaddr_un).cast(),
                std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
            )
        };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(error);
            }
            wait(fd, libc::POLLOUT, deadline, stop)?;
            if let Some(error) = stream.take_error()? {
                return Err(error);
            }
        }
        Ok(stream)
    }
    fn read_exact(
        stream: &mut UnixStream,
        mut bytes: &mut [u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        while !bytes.is_empty() {
            wait(stream.as_raw_fd(), libc::POLLIN, deadline, stop)?;
            match stream.read(bytes) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Truncated activation frame",
                    ));
                }
                Ok(size) => bytes = &mut bytes[size..],
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    ()
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    fn write_all(
        stream: &mut UnixStream,
        mut bytes: &[u8],
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<()> {
        while !bytes.is_empty() {
            wait(stream.as_raw_fd(), libc::POLLOUT, deadline, stop)?;
            match stream.write(bytes) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "Activation write failed",
                    ));
                }
                Ok(size) => bytes = &bytes[size..],
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    ()
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    fn receive(
        stream: &mut UnixStream,
        deadline: Instant,
        stop: &AtomicBool,
    ) -> io::Result<MailtoIntent> {
        peer(stream)?;
        let mut header = [0; 8];
        read_exact(stream, &mut header, deadline, stop)?;
        let size = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if header[..4] != MAGIC || size == 0 || size > MAILTO_URI_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid activation frame",
            ));
        }
        let mut bytes = vec![0; size];
        read_exact(stream, &mut bytes, deadline, stop)?;
        // One frame only; the sender must half-close, preventing trailing data.
        loop {
            wait(stream.as_raw_fd(), libc::POLLIN, deadline, stop)?;
            match stream.read(&mut [0; 1]) {
                Ok(0) => break,
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Trailing activation data",
                    ));
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    ()
                }
                Err(error) => return Err(error),
            }
        }
        let uri = std::str::from_utf8(&bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid activation encoding")
        })?;
        parse_mailto(uri)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid compose link"))
    }
    fn serve(
        listener: UnixListener,
        cleanup: &SocketCleanup,
        queue: &Mutex<VecDeque<MailtoIntent>>,
        stop: &AtomicBool,
    ) {
        while !stop.load(Ordering::Acquire) {
            if wait(
                listener.as_raw_fd(),
                libc::POLLIN,
                Instant::now() + TICK,
                stop,
            )
            .is_err()
            {
                continue;
            }
            let Ok((mut stream, _)) = listener.accept() else {
                // Avoid a hot loop on resource exhaustion such as EMFILE.
                thread::sleep(TICK);
                continue;
            };
            if !cleanup.valid_socket()
                || stream.set_nonblocking(true).is_err()
                || peer(&stream).is_err()
            {
                continue;
            }
            let deadline = Instant::now() + REQUEST_BUDGET;
            let status = match receive(&mut stream, deadline, stop) {
                Ok(intent) if cleanup.valid_socket() && !stop.load(Ordering::Acquire) => {
                    match queue.lock() {
                        Ok(mut queue) if queue.len() < CAPACITY => {
                            queue.push_back(intent);
                            ACCEPTED
                        }
                        _ => BUSY,
                    }
                }
                _ => INVALID,
            };
            let _ = write_all(&mut stream, &[status], deadline, stop);
        }
    }
    fn owner_pid(profile: &File) -> Result<u32, ActivationError> {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(pinned(profile).join("instance.lock"))?;
        let metadata = file.metadata()?;
        validate_file(&metadata)?;
        if metadata.len() != 8 {
            return Err(ActivationError::Unavailable);
        }
        let mut bytes = [0; 8];
        file.read_exact(&mut bytes)?;
        if bytes[..4] != MAGIC {
            return Err(ActivationError::Unavailable);
        }
        Ok(u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]))
    }
    #[cfg(test)]
    mod activation_tests {
        use super::*;
        use std::os::unix::fs::{PermissionsExt, symlink};
        use std::sync::atomic::AtomicU64;

        static NEXT: AtomicU64 = AtomicU64::new(0);
        struct Profile(PathBuf);
        impl Profile {
            fn new() -> Self {
                let path = std::env::temp_dir().join(format!(
                    "megamail-activation-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
                Self(path)
            }
            fn socket(&self) -> PathBuf {
                self.0.join("activation").join(SOCKET)
            }
            fn primary(&self, uri: Option<&str>) -> ActivationInbox {
                match start_activation(&self.0, uri).unwrap() {
                    ActivationStartup::Primary(inbox) => inbox,
                    ActivationStartup::Forwarded => panic!("fixture must own the lock"),
                }
            }
            fn endpoint(&self) {
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(self.0.join("activation"))
                    .unwrap();
            }
        }
        impl Drop for Profile {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
        fn raw(profile: &Profile, bytes: &[u8]) -> u8 {
            let deadline = Instant::now() + Duration::from_secs(3);
            let stop = AtomicBool::new(false);
            let mut stream = connect(&profile.socket(), deadline, &stop).unwrap();
            write_all(&mut stream, bytes, deadline, &stop).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            let mut status = [255];
            read_exact(&mut stream, &mut status, deadline, &stop).unwrap();
            status[0]
        }
        fn frame(uri: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::from(MAGIC);
            bytes.extend_from_slice(&(uri.len() as u32).to_be_bytes());
            bytes.extend_from_slice(uri);
            bytes
        }

        #[test]
        fn cold_and_running_links_use_one_parser_and_one_profile_lock() {
            let profile = Profile::new();
            let inbox = profile.primary(Some("mailto:cold@example.test?body=%2520+literal"));
            assert!(inbox.take_if_ready(false).is_none());
            assert_eq!(
                acquire(&profile.0.join("instance.lock"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            assert!(matches!(
                start_activation(
                    &profile.0,
                    Some("mailto:running@example.test?bcc=private@example.test&body=%252B")
                ),
                Ok(ActivationStartup::Forwarded)
            ));
            let cold = inbox.take_if_ready(true).unwrap();
            assert_eq!(cold.to, ["cold@example.test"]);
            assert_eq!(cold.body, "%20+literal");
            let running = inbox.take_if_ready(true).unwrap();
            assert_eq!(running.to, ["running@example.test"]);
            assert_eq!(running.bcc, ["private@example.test"]);
            assert_eq!(running.body, "%2B");
            assert!(inbox.take_if_ready(true).is_none());
            drop(inbox);
            assert!(!profile.socket().exists());
            drop(acquire(&profile.0.join("instance.lock")).unwrap());
        }

        #[test]
        fn eight_pending_links_include_cold_link_and_busy_never_overwrites() {
            let profile = Profile::new();
            let inbox = profile.primary(Some("mailto:?subject=0"));
            for index in 1..CAPACITY {
                assert!(matches!(
                    start_activation(&profile.0, Some(&format!("mailto:?subject={index}"))),
                    Ok(ActivationStartup::Forwarded)
                ));
            }
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?subject=overflow")),
                Err(ActivationError::Busy)
            ));
            assert!(inbox.take_if_ready(false).is_none());
            for index in 0..CAPACITY {
                assert_eq!(
                    inbox.take_if_ready(true).unwrap().subject,
                    index.to_string()
                );
            }
            assert!(inbox.take_if_ready(true).is_none());
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?subject=after")),
                Ok(ActivationStartup::Forwarded)
            ));
            assert_eq!(inbox.take_if_ready(true).unwrap().subject, "after");
        }

        #[test]
        fn invalid_frames_and_uris_cannot_enter_the_production_queue() {
            let profile = Profile::new();
            let inbox = profile.primary(None);
            for bytes in [
                frame(b"mailto:a@example.test?subject=bad%0D%0ABcc:x@example.test"),
                frame(b"https://example.test/"),
                frame(b"mailto:?attach=/etc/passwd"),
                frame(b"mailto:?body=%GG"),
                frame(&[255]),
                frame(b""),
                b"NOPE\0\0\0\x01x".to_vec(),
                [
                    MAGIC.as_slice(),
                    &((MAILTO_URI_LIMIT + 1) as u32).to_be_bytes(),
                ]
                .concat(),
                b"MMA1\0\0".to_vec(),
                [frame(b"mailto:?subject=valid"), b"trailing".to_vec()].concat(),
            ] {
                assert_eq!(raw(&profile, &bytes), INVALID);
                assert!(inbox.take_if_ready(true).is_none());
            }
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?subject=works")),
                Ok(ActivationStartup::Forwarded)
            ));
            assert_eq!(inbox.take_if_ready(true).unwrap().subject, "works");
        }

        #[test]
        fn setup_queue_is_profile_isolated_and_ordinary_second_launch_stays_locked() {
            let a = Profile::new();
            let b = Profile::new();
            let a_inbox = a.primary(None);
            let b_inbox = b.primary(None);
            assert!(matches!(
                start_activation(&a.0, None),
                Err(ActivationError::AlreadyRunning)
            ));
            assert!(matches!(
                start_activation(&a.0, Some("mailto:a@example.test")),
                Ok(ActivationStartup::Forwarded)
            ));
            assert!(b_inbox.take_if_ready(true).is_none());
            assert_eq!(a_inbox.take_if_ready(true).unwrap().to, ["a@example.test"]);
        }

        #[test]
        fn bad_permissions_symlinks_and_non_socket_endpoints_fail_closed() {
            let profile = Profile::new();
            fs::set_permissions(&profile.0, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(
                start_activation(&profile.0, None),
                Err(ActivationError::Security)
            ));
            assert!(!profile.0.join("instance.lock").exists());
            fs::set_permissions(&profile.0, fs::Permissions::from_mode(0o700)).unwrap();
            profile.endpoint();
            fs::set_permissions(
                profile.0.join("activation"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            assert!(matches!(
                start_activation(&profile.0, None),
                Err(ActivationError::Security)
            ));
            fs::set_permissions(
                profile.0.join("activation"),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            fs::write(profile.socket(), b"not a socket").unwrap();
            assert!(matches!(
                start_activation(&profile.0, None),
                Err(ActivationError::Security)
            ));
            assert_eq!(fs::read(profile.socket()).unwrap(), b"not a socket");
            fs::remove_file(profile.socket()).unwrap();
            symlink("missing", profile.socket()).unwrap();
            assert!(matches!(
                start_activation(&profile.0, None),
                Err(ActivationError::Security)
            ));
            assert!(
                fs::symlink_metadata(profile.socket())
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }

        #[test]
        fn lock_symlink_never_creates_or_touches_its_target() {
            let profile = Profile::new();
            let target = profile.0.join("untouched");
            symlink(&target, profile.0.join("instance.lock")).unwrap();
            assert!(matches!(
                start_activation(&profile.0, None),
                Err(ActivationError::Security)
            ));
            assert!(!target.exists());
        }

        #[test]
        fn stale_socket_cleanup_requires_lock_owner_and_active_socket_is_not_unlinked() {
            let profile = Profile::new();
            profile.endpoint();
            let listener = UnixListener::bind(profile.socket()).unwrap();
            fs::set_permissions(profile.socket(), fs::Permissions::from_mode(0o600)).unwrap();
            let inode = identity(&fs::symlink_metadata(profile.socket()).unwrap());
            assert!(matches!(
                start_activation(&profile.0, None),
                Err(ActivationError::Security)
            ));
            assert_eq!(
                identity(&fs::symlink_metadata(profile.socket()).unwrap()),
                inode
            );
            drop(listener);
            let lock = acquire(&profile.0.join("instance.lock")).unwrap();
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:")),
                Err(_)
            ));
            assert_eq!(
                identity(&fs::symlink_metadata(profile.socket()).unwrap()),
                inode
            );
            drop(lock);
            let inbox = profile.primary(None);
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?subject=after-crash")),
                Ok(ActivationStartup::Forwarded)
            ));
            assert_eq!(inbox.take_if_ready(true).unwrap().subject, "after-crash");
        }

        #[test]
        fn receiver_deadline_and_shutdown_cancel_silent_client() {
            let profile = Profile::new();
            let inbox = profile.primary(None);
            let silent = connect(
                &profile.socket(),
                Instant::now() + REQUEST_BUDGET,
                &AtomicBool::new(false),
            )
            .unwrap();
            // This request queues behind a silent peer. The peer's fixed deadline
            // must release the receiver without extending on partial progress.
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?subject=deadline")),
                Ok(ActivationStartup::Forwarded)
            ));
            assert_eq!(inbox.take_if_ready(true).unwrap().subject, "deadline");
            drop(silent);
            let silent = connect(
                &profile.socket(),
                Instant::now() + REQUEST_BUDGET,
                &AtomicBool::new(false),
            )
            .unwrap();
            let begin = Instant::now();
            drop(inbox);
            assert!(begin.elapsed() < Duration::from_secs(1));
            assert!(!profile.socket().exists());
            drop(silent);
        }

        #[test]
        fn no_half_close_does_not_accept_and_kernel_peer_identity_is_real() {
            let profile = Profile::new();
            let inbox = profile.primary(None);
            let stop = AtomicBool::new(false);
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut stream = connect(&profile.socket(), deadline, &stop).unwrap();
            let credentials = peer(&stream).unwrap();
            assert_eq!(credentials.uid, uid());
            assert_eq!(credentials.pid as u32, std::process::id());
            write_all(
                &mut stream,
                &frame(b"mailto:?subject=not-yet"),
                deadline,
                &stop,
            )
            .unwrap();
            let mut byte = [0];
            // The expired connection is closed; no queued compose intent exists.
            assert!(read_exact(&mut stream, &mut byte, deadline, &stop).is_err());
            assert!(inbox.take_if_ready(true).is_none());
        }

        #[test]
        fn different_pid_endpoint_is_rejected_before_uri_transmission() {
            let profile = Profile::new();
            profile.endpoint();
            let mut lock = acquire(&profile.0.join("instance.lock")).unwrap();
            lock.write_all(&MAGIC).unwrap();
            lock.write_all(&u32::MAX.to_be_bytes()).unwrap();
            let listener = UnixListener::bind(profile.socket()).unwrap();
            fs::set_permissions(profile.socket(), fs::Permissions::from_mode(0o600)).unwrap();
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?body=must-not-transmit")),
                Err(ActivationError::Security)
            ));
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = [0];
            assert_eq!(stream.read(&mut bytes).unwrap(), 0);
            assert!(profile.socket().exists());
        }

        #[test]
        fn permission_change_after_bind_rejects_new_intents() {
            let profile = Profile::new();
            let inbox = profile.primary(None);
            fs::set_permissions(profile.socket(), fs::Permissions::from_mode(0o666)).unwrap();
            assert!(matches!(
                start_activation(&profile.0, Some("mailto:?subject=rejected")),
                Err(ActivationError::Security)
            ));
            assert!(inbox.take_if_ready(true).is_none());
            fs::set_permissions(profile.socket(), fs::Permissions::from_mode(0o600)).unwrap();
            fs::set_permissions(
                profile.0.join("activation"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            let deadline = Instant::now() + REQUEST_BUDGET;
            let mut stream = connect(&profile.socket(), deadline, &AtomicBool::new(false)).unwrap();
            let mut byte = [0];
            assert_eq!(
                read_exact(&mut stream, &mut byte, deadline, &AtomicBool::new(false))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::UnexpectedEof
            );
            assert!(inbox.take_if_ready(true).is_none());
            fs::set_permissions(
                profile.0.join("activation"),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }

        #[test]
        fn subprocess_primary_entry() {
            let Some(path) = std::env::var_os("MEGAMAIL_SYNTHETIC_ACTIVATION_PROFILE") else {
                return;
            };
            let path = PathBuf::from(path);
            assert!(path.starts_with(std::env::temp_dir()));
            assert!(
                path.file_name()
                    .unwrap()
                    .as_bytes()
                    .starts_with(b"megamail-activation-")
            );
            let ActivationStartup::Primary(inbox) = start_activation(&path, None).unwrap() else {
                panic!("child must own profile");
            };
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(intent) = inbox.take_if_ready(true) {
                    assert_eq!(intent.to, ["child@example.test"]);
                    assert_eq!(intent.body, "%20+literal");
                    // Keep the primary alive until the client observed its ACK;
                    // shutdown before ACK is intentionally an uncertain handoff.
                    while !path.join("test-handoff-confirmed").exists() {
                        assert!(Instant::now() < deadline, "client did not confirm ACK");
                        thread::sleep(TICK);
                    }
                    break;
                }
                assert!(Instant::now() < deadline, "child never received intent");
                thread::sleep(TICK);
            }
        }

        #[test]
        fn distinct_process_primary_receives_through_the_production_seam() {
            let profile = Profile::new();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "instance::activation::activation_tests::subprocess_primary_entry",
                    "--nocapture",
                ])
                .env("MEGAMAIL_SYNTHETIC_ACTIVATION_PROFILE", &profile.0)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !profile.socket().exists() {
                if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("synthetic child failed to bind its private socket");
                }
                thread::sleep(TICK);
            }
            let result = start_activation(
                &profile.0,
                Some("mailto:child@example.test?body=%2520+literal"),
            );
            if !matches!(result, Ok(ActivationStartup::Forwarded)) {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "handoff to distinct-process primary failed: {:?}",
                    result.err()
                );
            }
            fs::write(profile.0.join("test-handoff-confirmed"), b"ack").unwrap();
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("synthetic primary did not shut down");
                }
                thread::sleep(TICK);
            }
            assert!(!profile.socket().exists());
            drop(acquire(&profile.0.join("instance.lock")).unwrap());
        }

        #[test]
        fn cleanup_does_not_unlink_replaced_socket_or_foreign_file() {
            let profile = Profile::new();
            let inbox = profile.primary(None);
            fs::remove_file(profile.socket()).unwrap();
            fs::write(profile.socket(), b"replacement").unwrap();
            drop(inbox);
            assert_eq!(fs::read(profile.socket()).unwrap(), b"replacement");
        }
    }

    fn handoff(profile: &File, uri: &str) -> Result<(), ActivationError> {
        if uri.is_empty() || uri.len() > MAILTO_URI_LIMIT {
            return Err(ActivationError::InvalidIntent(MailtoError::TooLarge));
        }
        let deadline = Instant::now() + START_BUDGET;
        let stop = AtomicBool::new(false);
        // Retry only endpoint readiness. Never retry after writing any request.
        let mut stream = loop {
            let attempt = (|| {
                let directory = endpoint_dir(profile, false)?;
                let path = pinned(&directory).join(SOCKET);
                let metadata = fs::symlink_metadata(&path)?;
                validate_socket(&metadata)?;
                let stream = connect(&path, deadline, &stop)?;
                let credentials = peer(&stream).map_err(|_| ActivationError::Security)?;
                if credentials.pid as u32 != owner_pid(profile)?
                    || identity(&fs::symlink_metadata(&path)?) != identity(&metadata)
                {
                    return Err(ActivationError::Security);
                }
                Ok(stream)
            })();
            match attempt {
                Ok(stream) => break stream,
                Err(ActivationError::Io(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::WouldBlock
                    ) && Instant::now() < deadline =>
                {
                    thread::sleep(TICK)
                }
                Err(ActivationError::Unavailable) if Instant::now() < deadline => {
                    thread::sleep(TICK)
                }
                Err(error) => return Err(error),
            }
        };
        let mut header = [0; 8];
        header[..4].copy_from_slice(&MAGIC);
        header[4..].copy_from_slice(&(uri.len() as u32).to_be_bytes());
        // Any failure after transmission starts is conservatively ambiguous.
        write_all(&mut stream, &header, deadline, &stop)
            .map_err(|_| ActivationError::HandoffUncertain)?;
        write_all(&mut stream, uri.as_bytes(), deadline, &stop)
            .map_err(|_| ActivationError::HandoffUncertain)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| ActivationError::HandoffUncertain)?;
        let mut status = [0];
        read_exact(&mut stream, &mut status, deadline, &stop)
            .map_err(|_| ActivationError::HandoffUncertain)?;
        match status[0] {
            ACCEPTED => Ok(()),
            BUSY => Err(ActivationError::Busy),
            INVALID => Err(ActivationError::InvalidIntent(MailtoError::InvalidSyntax)),
            _ => Err(ActivationError::HandoffUncertain),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_process_lock_is_rejected_and_crash_free_release_is_automatic() {
        let path =
            std::env::temp_dir().join(format!("megamail-instance-{}.lock", std::process::id()));
        let first = acquire(&path).unwrap();
        assert_eq!(
            acquire(&path).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(first);
        drop(acquire(&path).unwrap());
        std::fs::remove_file(path).unwrap();
    }
}
