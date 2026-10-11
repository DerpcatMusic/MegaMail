#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Launch-free contract tests for the parent verification driver, not build proof."""
import contextlib
import copy
import io
import json
from pathlib import Path
import signal
import subprocess
import tempfile
import types
import unittest
from unittest import mock

DRIVER = Path(__file__).with_name("verify-megamail-batch2.py").resolve()
module = types.ModuleType("verification_driver_under_test")
module.__file__ = str(DRIVER)
exec(compile(DRIVER.read_text(), str(DRIVER), "exec"), module.__dict__)


class DriverContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="megamail-verifier-fictional-")
        self.root = Path(self.temp.name)
        self.state = {"version": 1, "head": module.BASELINE, "sources": {"src/fixture.rs": "fictional-hash"}}
        self.receipt = self.root / "receipt.json"
        self.receipt.write_text(json.dumps(self.state))

    def tearDown(self):
        self.temp.cleanup()

    def invoke(self, args, **patches):
        with contextlib.ExitStack() as stack:
            stack.enter_context(mock.patch.object(module.sys, "argv", [str(DRIVER), *args]))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
            for name, value in patches.items():
                stack.enter_context(mock.patch.object(module, name, value))
            return module.main()

    def test_plan_never_calls_process_runner(self):
        runner = mock.Mock(side_effect=AssertionError("launch forbidden"))
        self.assertEqual(self.invoke([], snapshot=lambda: self.state, run_owned=runner), 0)
        runner.assert_not_called()

    def test_missing_or_changed_receipt_cannot_launch(self):
        runner = mock.Mock(side_effect=AssertionError("launch forbidden"))
        for args in [["--execute"], ["--execute", "--freeze-state", str(self.receipt)]]:
            changed = copy.deepcopy(self.state)
            changed["sources"]["src/fixture.rs"] = "changed"
            with self.assertRaises(SystemExit):
                self.invoke(args, snapshot=lambda: changed, run_owned=runner)
        runner.assert_not_called()

    def test_record_and_execute_cannot_be_combined(self):
        runner = mock.Mock(side_effect=AssertionError("launch forbidden"))
        with self.assertRaises(SystemExit):
            self.invoke(["--record-freeze", str(self.root / "new.json"), "--execute"], run_owned=runner)
        runner.assert_not_called()

    def test_receipt_is_private_and_cannot_replace_existing_file(self):
        path = self.root / "new.json"
        runner = mock.Mock(side_effect=AssertionError("launch forbidden"))
        self.assertEqual(self.invoke(["--record-freeze", str(path)], snapshot=lambda: self.state, run_owned=runner), 0)
        self.assertEqual(json.loads(path.read_text()), self.state)
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(FileExistsError):
            self.invoke(["--record-freeze", str(path)], snapshot=lambda: self.state, run_owned=runner)
        runner.assert_not_called()

    def test_midwindow_change_stops_before_any_next_step(self):
        changed = copy.deepcopy(self.state)
        changed["sources"]["src/fixture.rs"] = "changed"
        states = mock.Mock(side_effect=[self.state, changed])
        runner = mock.Mock(side_effect=AssertionError("launch forbidden"))
        logs = self.root / "logs"
        self.assertEqual(self.invoke(["--execute", "--freeze-state", str(self.receipt), "--log-dir", str(logs)],
                                     snapshot=states, run_owned=runner), 1)
        runner.assert_not_called()
        report = json.loads((logs / "results.json").read_text())
        self.assertEqual(report["results"][0]["status"], "blocked")
        self.assertFalse(report["release_accepted"])

    def test_failed_core_gate_skips_native_and_keeps_connect_not_run(self):
        steps = lambda: [("core-tests", ["FICTIONAL-NO-COMPILER"], 1, [], "mock only"),
                         ("native-tests", ["FICTIONAL-NO-COMPILER"], 1, ["core-tests"], "mock only")]
        runner = mock.Mock(return_value=1)
        logs = self.root / "logs"
        self.assertEqual(self.invoke(["--execute", "--freeze-state", str(self.receipt), "--log-dir", str(logs)],
                                     snapshot=lambda: self.state, steps=steps, run_owned=runner), 1)
        self.assertEqual(runner.call_count, 1)
        outcomes = {item["name"]: item["status"] for item in json.loads((logs / "results.json").read_text())["results"]}
        self.assertEqual(outcomes, {"core-tests": "failed", "native-tests": "not_run", "connect-blackhole": "not_run"})

    def test_timeout_signals_only_unreaped_owned_mock_group(self):
        child = mock.Mock(pid=432123, returncode=None)
        child.wait.side_effect = [subprocess.TimeoutExpired(["FICTIONAL"], 1), 0]
        with mock.patch.object(module.subprocess, "Popen", return_value=child) as popen, \
             mock.patch.object(module.os, "killpg") as kill, mock.patch.object(module.time, "sleep"):
            self.assertEqual(module.run_owned(["FICTIONAL"], io.StringIO(), 1), 124)
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        self.assertEqual(kill.call_args_list, [mock.call(432123, signal.SIGTERM), mock.call(432123, signal.SIGKILL)])

    def test_already_reaped_mock_leader_is_never_signaled(self):
        child = mock.Mock(pid=432123, returncode=0)
        child.wait.side_effect = KeyboardInterrupt()
        with mock.patch.object(module.subprocess, "Popen", return_value=child), \
             mock.patch.object(module.os, "killpg") as kill:
            with self.assertRaises(KeyboardInterrupt):
                module.run_owned(["FICTIONAL"], io.StringIO(), 1)
        kill.assert_not_called()

    def test_snapshot_includes_embedded_assets_without_reading_arbitrary_runtime_json(self):
        names = ["src/fixture.rs", "apps/megamail/assets/fonts/Fictional.ttf",
                 "apps/megamail/src/fixtures/Fictional.eml", "crates/mail-core/tests/fixtures/Fictional.xml",
                 "tools/thunderbird-bridge/extension/manifest.json", "tools/thunderbird-bridge/extension/api/schema.json",
                 "fictional-private-runtime.json"]
        for name in names:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fictional fixture only")
        original_read = Path.read_bytes

        def read(path):
            if path.name == "fictional-private-runtime.json":
                raise AssertionError("arbitrary runtime JSON must not be read")
            return original_read(path)

        outputs = [module.BASELINE + "\n", ("\0".join(names) + "\0").encode()]
        with mock.patch.object(module, "ROOT", self.root), \
             mock.patch.object(module.subprocess, "check_output", side_effect=outputs), \
             mock.patch.object(Path, "read_bytes", read):
            state = module.snapshot()
        self.assertEqual(set(state["sources"]), set(names[:-1]))

    def test_missing_connect_prerequisites_are_blocked_not_passed(self):
        with mock.patch.object(module, "run_owned", side_effect=AssertionError("launch forbidden")) as runner:
            outcome = module.connect_fixture(None, self.root)
        self.assertEqual(outcome["status"], "blocked")
        runner.assert_not_called()


if __name__ == "__main__":
    unittest.main(verbosity=2)
