// Included by thunderbird_bridge::unix under cfg(test). This is deliberately
// not a second library or toy actor: fixtures attach a Unix peer through the
// constructor used by Runtime::start, then call the production request route.
use super::*;

mod process_contract {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/runtime_batch2/process_contract.rs"
    ));
}

fn no_child() -> ChildGuard {
    ChildGuard {
        child: None,
        isolated_group: false,
    }
}

fn fixture() -> (Runtime, UnixStream, PathBuf) {
    let root = new_private_temp_dir().unwrap();
    let (stream, peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let runtime = Runtime::from_connected(
        stream,
        no_child(),
        File::create(root.join("profile.lock")).unwrap(),
        root.join("host.json"),
        root.clone(),
        root.join("persistent-clone"),
    )
    .unwrap();
    (runtime, peer, root)
}

fn reply(peer: &mut UnixStream, id: &Value, result: Value) {
    write_frame_with_limit(
        peer,
        &serde_json::to_vec(&json!({ "id": id, "result": result })).unwrap(),
        MAX_EXTENSION_FRAME,
    )
    .unwrap();
}

fn shutdown_reply(peer: &mut UnixStream) {
    let shutdown = read_json_frame(peer).unwrap();
    assert_eq!(shutdown["method"], "shutdown");
    assert_eq!(shutdown["id"].as_u64(), Some(SHUTDOWN_ID));
    reply(peer, &shutdown["id"], json!({"stopping": true}));
}

#[test]
fn timeout_ignores_late_completion_and_keeps_public_call_healthy() {
    let (runtime, mut peer, root) = fixture();
    let worker = runtime.clone();
    let request = std::thread::spawn(move || {
        worker.call_until(
            "body",
            json!({}),
            Instant::now() + Duration::from_millis(50),
        )
    });
    let first = read_json_frame(&mut peer).unwrap();
    let error = request.join().unwrap().unwrap_err();
    assert_eq!(
        classify_bridge_error(&error),
        Some(BridgeFailureKind::RequestTimeout)
    );
    assert!(!runtime.0.broken.load(Ordering::Acquire));
    assert!(runtime.0.pending.lock().unwrap().is_empty());
    let server = std::thread::spawn(move || {
        reply(&mut peer, &first["id"], json!("late body"));
        let next = read_json_frame(&mut peer).unwrap();
        assert_ne!(next["id"], first["id"]);
        reply(&mut peer, &next["id"], json!("current body"));
        shutdown_reply(&mut peer);
    });
    assert_eq!(
        runtime.call("body", json!({})).unwrap(),
        json!("current body")
    );
    drop(runtime);
    server.join().unwrap();
    assert!(!root.exists());
}

#[test]
fn writer_mutex_wait_is_inside_deadline_and_sends_no_expired_request() {
    let (runtime, mut peer, _) = fixture();
    let lock = runtime.0.stream.lock().unwrap();
    let worker = runtime.clone();
    let result = std::thread::spawn(move || {
        worker.call_until(
            "body",
            json!({}),
            Instant::now() + Duration::from_millis(30),
        )
    })
    .join()
    .unwrap();
    assert_eq!(
        classify_bridge_error(&result.unwrap_err()),
        Some(BridgeFailureKind::RequestTimeout)
    );
    assert!(!runtime.0.broken.load(Ordering::Acquire));
    drop(lock);
    let server = std::thread::spawn(move || {
        let request = read_json_frame(&mut peer).unwrap();
        assert_eq!(request["id"], 2); // expired ID 1 was not sent
        reply(&mut peer, &request["id"], json!("healthy"));
        shutdown_reply(&mut peer);
    });
    assert_eq!(runtime.call("body", json!({})).unwrap(), json!("healthy"));
    drop(runtime);
    server.join().unwrap();
}

#[test]
fn saturation_is_busy_then_disconnect_drains_all_admitted_calls() {
    let (runtime, mut peer, _) = fixture();
    let mut workers = Vec::new();
    for _ in 0..MAX_PENDING {
        let worker = runtime.clone();
        workers.push(std::thread::spawn(move || {
            worker.call_until(
                "conversation",
                json!({}),
                Instant::now() + Duration::from_secs(5),
            )
        }));
    }
    for _ in 0..MAX_PENDING {
        read_json_frame(&mut peer).unwrap();
    }
    assert_eq!(runtime.0.pending.lock().unwrap().len(), MAX_PENDING);
    assert_eq!(
        classify_bridge_error(&runtime.call("body", json!({})).unwrap_err()),
        Some(BridgeFailureKind::Busy)
    );
    assert!(!runtime.0.broken.load(Ordering::Acquire));
    drop(peer);
    for worker in workers {
        assert_eq!(
            classify_bridge_error(&worker.join().unwrap().unwrap_err()),
            Some(BridgeFailureKind::Disconnected)
        );
    }
    assert!(runtime.0.pending.lock().unwrap().is_empty());
    assert_eq!(
        classify_bridge_error(&runtime.call("body", json!({})).unwrap_err()),
        Some(BridgeFailureKind::Disconnected)
    );
}

fn fatal_reply(send: impl FnOnce(&mut UnixStream, &Value)) {
    let (runtime, mut peer, _) = fixture();
    let worker = runtime.clone();
    let request = std::thread::spawn(move || worker.call("body", json!({})));
    let request_json = read_json_frame(&mut peer).unwrap();
    send(&mut peer, &request_json["id"]);
    assert_eq!(
        classify_bridge_error(&request.join().unwrap().unwrap_err()),
        Some(BridgeFailureKind::Protocol)
    );
    assert!(runtime.0.broken.load(Ordering::Acquire));
    assert!(runtime.0.pending.lock().unwrap().is_empty());
    assert_eq!(
        classify_bridge_error(&runtime.call("body", json!({})).unwrap_err()),
        Some(BridgeFailureKind::Disconnected)
    );
}

#[test]
fn future_reply_id_cannot_complete_a_different_request() {
    fatal_reply(|peer, _| reply(peer, &json!(999), json!("wrong request")));
}

#[test]
fn oversized_inbound_header_breaks_runtime_without_waiting_for_payload() {
    fatal_reply(|peer, _| {
        peer.write_all(&((MAX_EXTENSION_FRAME + 1) as u32).to_le_bytes())
            .unwrap()
    });
}

#[test]
fn missing_reply_id_breaks_runtime_instead_of_completing_a_waiter() {
    fatal_reply(|peer, _| {
        write_frame_with_limit(peer, br#"{"result":"missing id"}"#, MAX_EXTENSION_FRAME).unwrap();
    });
}

#[test]
fn truncated_inbound_frame_breaks_runtime() {
    fatal_reply(|peer, _| {
        peer.write_all(&100_u32.to_le_bytes()).unwrap();
        peer.write_all(b"{").unwrap();
        peer.shutdown(std::net::Shutdown::Write).unwrap();
    });
}

#[test]
fn oversized_outbound_request_does_not_consume_pending_capacity() {
    let (runtime, mut peer, _) = fixture();
    assert!(runtime
        .call("body", json!({"payload": "x".repeat(MAX_FRAME + 1)}))
        .is_err());
    assert!(runtime.0.pending.lock().unwrap().is_empty());
    assert!(!runtime.0.broken.load(Ordering::Acquire));
    let server = std::thread::spawn(move || {
        let request = read_json_frame(&mut peer).unwrap();
        reply(&mut peer, &request["id"], json!("still healthy"));
        shutdown_reply(&mut peer);
    });
    assert_eq!(
        runtime.call("body", json!({})).unwrap(),
        json!("still healthy")
    );
    drop(runtime);
    server.join().unwrap();
}

#[test]
fn malformed_envelope_breaks_runtime_instead_of_faking_success() {
    fatal_reply(|peer, id| {
        write_frame_with_limit(
            peer,
            &serde_json::to_vec(&json!({"id": id, "result": "ok", "error": "bad"})).unwrap(),
            MAX_EXTENSION_FRAME,
        )
        .unwrap();
    });
}

#[test]
fn explicit_login_code_is_not_inferred_from_credential_error_prose() {
    let (runtime, mut peer, _) = fixture();
    let server = std::thread::spawn(move || {
        for explicit in [false, true] {
            let request = read_json_frame(&mut peer).unwrap();
            let mut response = json!({"id": request["id"], "error": "Encrypted credentials are temporarily unavailable"});
            if explicit {
                response["errorCode"] = json!("needs-login");
            }
            write_frame_with_limit(
                &mut peer,
                &serde_json::to_vec(&response).unwrap(),
                MAX_EXTENSION_FRAME,
            )
            .unwrap();
        }
        shutdown_reply(&mut peer);
    });
    assert_eq!(
        classify_bridge_error(&runtime.call("folders", json!({})).unwrap_err()),
        Some(BridgeFailureKind::Operation)
    );
    assert_eq!(
        classify_bridge_error(&runtime.call("folders", json!({})).unwrap_err()),
        Some(BridgeFailureKind::NeedsLogin)
    );
    assert!(!runtime.0.broken.load(Ordering::Acquire));
    drop(runtime);
    server.join().unwrap();
}

#[test]
fn request_ids_stop_before_javascript_precision_loss() {
    let (runtime, mut peer, _) = fixture();
    runtime.0.sequence.store(SHUTDOWN_ID - 1, Ordering::Release);
    let server = std::thread::spawn(move || {
        let request = read_json_frame(&mut peer).unwrap();
        assert_eq!(request["id"].as_u64(), Some(SHUTDOWN_ID - 1));
        reply(&mut peer, &request["id"], json!("last-safe-id"));
        shutdown_reply(&mut peer);
    });
    assert_eq!(
        runtime.call("body", json!({})).unwrap(),
        json!("last-safe-id")
    );
    assert_eq!(
        classify_bridge_error(&runtime.call("body", json!({})).unwrap_err()),
        Some(BridgeFailureKind::Busy)
    );
    drop(runtime);
    server.join().unwrap();
}

#[test]
fn acknowledged_shutdown_closes_reader_even_if_peer_keeps_socket_open() {
    let (runtime, mut peer, root) = fixture();
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        shutdown_reply(&mut peer);
        release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let dropper = std::thread::spawn(move || {
        drop(runtime);
        done_tx.send(()).unwrap();
    });
    done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!root.exists());
    release_tx.send(()).unwrap();
    dropper.join().unwrap();
    server.join().unwrap();
}

#[test]
fn startup_silent_peer_uses_same_accept_and_handshake_deadline() {
    let root = new_private_temp_dir().unwrap();
    let listener = UnixListener::bind(root.join("bridge.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut artifacts = StartupArtifacts {
        manifest: root.join("host.json"),
        temporary_dir: root.clone(),
        armed: true,
    };
    write_private_bytes(&artifacts.manifest, b"synthetic host").unwrap();
    let _peer = UnixStream::connect(root.join("bridge.sock")).unwrap();
    let start = Instant::now();
    let result = accept_ready_bridge(
        &listener,
        &mut no_child(),
        start + Duration::from_millis(40),
        "fixture-version",
    );
    assert_eq!(
        classify_bridge_error(&result.unwrap_err()),
        Some(BridgeFailureKind::Protocol)
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    artifacts.armed = true;
    drop(artifacts);
    assert!(!root.exists());
}

#[test]
fn startup_rejects_stale_extension_but_accepts_current_handshake() {
    for version in ["old-version", "current-version"] {
        let root = new_private_temp_dir().unwrap();
        let listener = UnixListener::bind(root.join("bridge.sock")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut peer = UnixStream::connect(root.join("bridge.sock")).unwrap();
        reply(
            &mut peer,
            &json!(0),
            json!({"ready": true, "version": version}),
        );
        let result = accept_ready_bridge(
            &listener,
            &mut no_child(),
            Instant::now() + Duration::from_secs(1),
            "current-version",
        );
        assert_eq!(result.is_ok(), version == "current-version");
        drop(result);
        drop(peer);
        drop(listener);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn floor_153_and_current_157_are_intentionally_supported() {
    for version in [
        "Mozilla Thunderbird 153.0",
        "Mozilla Thunderbird 157.0.1",
        "Thunderbird 157.0b1",
        "Thunderbird 158.0",
    ] {
        assert!(validate_version_output(version).is_ok(), "{version}");
    }
    for version in [
        "Thunderbird 152.9",
        "not Thunderbird 157.0",
        "Thunderbird 157malware",
        "",
    ] {
        assert_eq!(
            classify_bridge_error(&validate_version_output(version).unwrap_err()),
            Some(BridgeFailureKind::Unsupported)
        );
    }
}

#[test]
fn synthetic_clone_copy_is_private_and_does_not_copy_mail_or_change_source() {
    let root = new_private_temp_dir().unwrap();
    let source = root.join("source");
    let target = root.join("clone");
    ensure_private_dir(&source).unwrap();
    ensure_private_dir(&target).unwrap();
    write_private_bytes(
        &source.join("key4.db"),
        b"synthetic encrypted bytes, not a real NSS database",
    )
    .unwrap();
    write_private_bytes(&source.join("logins.json"), b"synthetic opaque fixture").unwrap();
    write_private_bytes(&source.join("prefs.js"), b"not copied by auth snapshot").unwrap();
    write_private_bytes(&source.join("Inbox"), b"not copied").unwrap();
    let stamp = credentials_stamp(&source).unwrap();
    let source_inode = fs::metadata(source.join("key4.db")).unwrap().ino();
    copy_encrypted_auth_store(&source, &target, &stamp).unwrap();
    assert_eq!(credentials_stamp(&source).unwrap(), stamp);
    assert_eq!(
        fs::metadata(source.join("key4.db")).unwrap().ino(),
        source_inode
    );
    assert!(!target.join("Inbox").exists());
    assert!(!target.join("prefs.js").exists());
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(target.join("key4.db"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(copy_encrypted_auth_store(&source, &source, &stamp).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_paths_reject_symlinked_directory_and_unchanged_file() {
    let root = new_private_temp_dir().unwrap();
    let real = root.join("real");
    ensure_private_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, root.join("redirect")).unwrap();
    assert!(ensure_private_dir(&root.join("redirect/child")).is_err());
    assert!(!real.join("child").exists());
    write_private_bytes(&real.join("auth"), b"opaque").unwrap();
    std::os::unix::fs::symlink(real.join("auth"), root.join("credential-link")).unwrap();
    assert!(write_private_bytes_if_changed(&root.join("credential-link"), b"opaque").is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unchanged_private_file_repairs_mode_and_rejects_hardlinked_destination() {
    let root = new_private_temp_dir().unwrap();
    let path = root.join("extension.xpi");
    write_private_bytes(&path, b"synthetic package").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let inode = fs::metadata(&path).unwrap().ino();
    write_private_bytes_if_changed(&path, b"synthetic package").unwrap();
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::hard_link(&path, root.join("linked-destination")).unwrap();
    assert!(
        write_private_bytes_if_changed(&root.join("linked-destination"), b"synthetic package")
            .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bounded_request_encoder_rejects_large_payload() {
    assert!(encode_request(&json!({"id": 1, "params": "x".repeat(MAX_FRAME + 1)})).is_err());
}
