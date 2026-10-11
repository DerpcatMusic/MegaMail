// Included under cfg(test) by contract.rs. Synthetic /bin/sh processes only;
// no Thunderbird executable or real profile is used by these tests.
use super::*;

#[cfg(target_os = "linux")]
#[test]
fn version_probe_reads_bounded_pipes_without_launching_thunderbird() {
    let root = new_private_temp_dir().unwrap();
    let executable = root.join("synthetic-version-probe");
    write_new_private_bytes(
        &executable,
        b"#!/bin/sh\nprintf 'Mozilla Thunderbird 153.0\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    validate_thunderbird_version(&executable).unwrap();
    write_private_bytes(
        &executable,
        b"#!/bin/sh\nprintf 'Mozilla Thunderbird 152.0\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        classify_bridge_error(&validate_thunderbird_version(&executable).unwrap_err()),
        Some(BridgeFailureKind::Unsupported)
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_group_cleanup_stops_synthetic_descendant_without_process_name_kills() {
    use std::io::BufRead;
    let child = Command::new("/bin/sh")
        .args(["-c", "sleep 30 & printf '%s\\n' \"$!\"; wait"])
        .process_group(0)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut guard = ChildGuard {
        child: Some(child),
        isolated_group: true,
    };
    let stdout = guard.child_mut().stdout.take().unwrap();
    let mut line = String::new();
    std::io::BufReader::new(stdout)
        .read_line(&mut line)
        .unwrap();
    let descendant: u32 = line.trim().parse().unwrap();
    let _ = guard.terminate();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // Reaping non-child zombies belongs to the OS, not this library.
        let stat = fs::read_to_string(format!("/proc/{descendant}/stat"));
        if stat
            .as_ref()
            .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
            || stat.as_ref().is_ok_and(|s| {
                s.split_once(") ")
                    .is_some_and(|(_, suffix)| suffix.starts_with("Z "))
            })
        {
            break;
        }
        assert!(Instant::now() < deadline, "synthetic descendant still live");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn failed_startup_guard_reaps_its_leader_and_spares_unrelated_process() {
    let unrelated_child = Command::new("/bin/sh")
        .args(["-c", "exec sleep 30"])
        .spawn()
        .unwrap();
    let mut unrelated = ChildGuard {
        child: Some(unrelated_child),
        isolated_group: false,
    };
    let child = Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .process_group(0)
        .spawn()
        .unwrap();
    let pid = child.id();
    let guard = ChildGuard {
        child: Some(child),
        isolated_group: true,
    };
    let deadline = Instant::now() + Duration::from_secs(1);
    while !child_exited(guard.child.as_ref().unwrap()).unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(guard);
    // waitid WNOWAIT is used before cleanup, never Child::try_wait: once the
    // guard finishes, waitid must report ECHILD for the reaped leader.
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    // SAFETY: initialized siginfo buffer and synthetic PID, observation only.
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid,
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    assert_eq!(result, -1);
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(!child_exited(unrelated.child_mut()).unwrap());
}
