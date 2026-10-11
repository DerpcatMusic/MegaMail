#!/usr/bin/env python3
"""Synthetic Linux kernel/static checks. NOT execution of the Rust runtime.

No Thunderbird, private profile, provider, credentials, compilation, or repo
writes. Tests kernel assumptions that production Rust uses and source seam
wiring; Rust contract.rs must still execute in the parent's build window.
"""
from pathlib import Path
import os
import signal
import socket
import subprocess
import sys
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]


class KernelContracts(unittest.TestCase):
    def test_duplicate_socket_shutdown_wakes_reader(self):
        local, peer = socket.socketpair()
        duplicate = local.dup()
        try:
            duplicate.settimeout(0.5)
            local.shutdown(socket.SHUT_RDWR)
            self.assertEqual(duplicate.recv(1), b"")
        finally:
            local.close()
            peer.close()
            duplicate.close()

    @unittest.skipUnless(sys.platform == "linux", "WNOWAIT policy is Linux-first")
    def test_group_cleanup_retains_leader_identity_and_spares_unrelated_child(self):
        # A stopped leader retains its PID/PGID until reaping. Its synthetic
        # descendant stays in that dedicated group; an unrelated process does not.
        unrelated = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        script = (
            "import os,time; "
            "pid=os.fork(); "
            "print(pid,flush=True) if pid else None; "
            "os._exit(0) if pid else time.sleep(30)"
        )
        leader = subprocess.Popen([sys.executable, "-c", script], process_group=0, stdout=subprocess.PIPE)
        descendant = None
        try:
            descendant = int(leader.stdout.readline())
            deadline = time.monotonic() + 1
            status = None
            while time.monotonic() < deadline:
                status = os.waitid(os.P_PID, leader.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                if status:
                    break
                time.sleep(0.01)
            self.assertIsNotNone(status)
            self.assertEqual(status.si_pid, leader.pid)
            os.killpg(leader.pid, signal.SIGKILL)
            leader.wait(timeout=1)
            self.assertIsNone(unrelated.poll())
            # Non-child zombie reaping belongs to the OS/subreaper. Assert no
            # live descendant, not a claim that we can reap somebody else's child.
            deadline = time.monotonic() + 1
            while time.monotonic() < deadline:
                stat = Path(f"/proc/{descendant}/stat")
                if not stat.exists() or stat.read_text().split(") ", 1)[1].split()[0] == "Z":
                    break
                time.sleep(0.01)
            else:
                self.fail("synthetic runtime descendant remains live")
        finally:
            if leader.returncode is None:
                os.killpg(leader.pid, signal.SIGKILL)
                leader.wait(timeout=1)
            if leader.stdout:
                leader.stdout.close()
            unrelated.kill()
            unrelated.wait(timeout=1)

    def test_rust_contracts_are_included_and_use_production_route(self):
        source = (ROOT / "crates/mail-core/src/thunderbird_bridge.rs").read_text()
        tests = (ROOT / "crates/mail-core/tests/runtime_batch2/contract.rs").read_text()
        self.assertIn('/tests/runtime_batch2/contract.rs', source)
        self.assertIn('let runtime = Self::from_connected(', source)
        self.assertIn('Runtime::from_connected(', tests)
        self.assertIn('self.call_until(method, params, Instant::now() + operation_timeout(method))', source)
        self.assertIn('runtime.call("body"', tests)
        self.assertIn('const MAX_PENDING: usize = 64;', source)
        self.assertIn('libc::WNOWAIT', source)
        self.assertIn('.process_group(0)', source)
        self.assertIn('classify_bridge_error', source)


if __name__ == "__main__":
    unittest.main(verbosity=2)
