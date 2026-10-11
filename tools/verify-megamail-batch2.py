#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Parent-owned serial batch verification. Default prints a plan; it builds nothing.

Record a source receipt only after every editing owner has released its files.
Execution requires that receipt and stops if source changes. No installation,
private-profile probes, real provider operations, or global ignored-test run.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import stat
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
BASELINE = "d4dbd2bba14f474a33870269cc15bd98376ff360"
SUFFIXES = {".rs", ".toml", ".lock", ".c", ".h", ".js", ".cjs", ".py", ".sh", ".desktop", ".html"}
EXCLUDE = ("docs/licenses/", "tools/megamail-html-reader/build/", "tools/megamail-html-reader/build-no-csp/")
# Explicit reviewed build/contract asset namespaces: do not indiscriminately
# hash arbitrary JSON/EML files that could be private runtime state.
ASSET_PREFIXES = ("apps/megamail/assets/", "apps/megamail/src/fixtures/",
                  "crates/mail-core/tests/fixtures/", "tools/thunderbird-bridge/extension/",
                  "vendor/gpui-base/src/dock/fixtures/")


def snapshot():
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
    ).decode().split("\0")
    entries = {}
    for name in sorted(set(paths)):
        if not name or name.startswith(EXCLUDE):
            continue
        path = ROOT / name
        if path.suffix not in SUFFIXES and path.name != "Makefile" and not name.startswith(ASSET_PREFIXES):
            continue
        if not path.exists():
            entries[name] = "deleted"
            continue
        if not stat.S_ISREG(path.lstat().st_mode):
            raise ValueError(f"Source is not a regular file: {name}")
        entries[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return {"version": 1, "head": head, "sources": entries}


def steps():
    return [
        ("whitespace", ["git", "diff", "--check"], 30, [], "source check only"),
        ("installer-syntax", ["bash", "-n", "tools/install-megamail.sh"], 30, [], "syntax only; installer is NOT run"),
        ("driver-contracts", ["python3", "tools/test-megamail-batch2-verifier.py"], 60, [], "launch-free mocked driver contracts; no compiler or signal execution"),
        ("helper-viewport-source", ["python3", "tools/megamail-html-reader/viewport_contract.py"], 30, [], "chrome/source fingerprints and installed declarations only; not real allocation/pixels/accessibility"),
        ("native-qa-import", ["python3", "tools/test-megamail-native-qa-import.py"], 30, [], "launch-free real module import at host and root-level isolated mount; not app execution"),
        ("reader-focus-source", ["python3", "tools/test-megamail-reader-focus-wiring.py"], 30, [], "six source lifecycle markers only; NOT actual focus/clipboard acceptance"),
        ("bridge-fixtures", ["node", "tools/thunderbird-bridge/extension.test.cjs"], 180, [], "production extension with fictional Messenger fixtures"),
        ("activation-packaging", ["python3", "tools/test-megamail-activation-packaging.py"], 60, [], "pure actual-generator fixtures; no desktop installation"),
        ("runtime-kernel-fixtures", ["python3", "crates/mail-core/tests/runtime_batch2_kernel.py"], 120, [], "synthetic process/kernel fixtures; not actual Thunderbird"),
        ("static-integration-audit", ["python3", "tools/megamail-batch2-static-audit.py"], 60, [], "source markers only; not runtime acceptance"),
        ("core-tests", ["cargo", "test", "--manifest-path", "crates/mail-core/Cargo.toml", "--locked", "-j", "2", "--lib", "--tests", "--message-format=json", "--", "--test-threads=1"], 3600, ["driver-contracts"], "core units and public contracts; ordinary ignored tests stay ignored"),
        ("native-tests", ["cargo", "test", "--manifest-path", "apps/megamail/Cargo.toml", "--locked", "-j", "2", "--message-format=json", "--", "--test-threads=1"], 3600, ["core-tests"], "native tests; not real login or GUI acceptance"),
        ("theme-tests", ["cargo", "test", "--manifest-path", "crates/zeron-theme/Cargo.toml", "--locked", "-j", "2", "--", "--test-threads=1"], 1800, ["driver-contracts"], "theme regression suite"),
        ("native-build", ["cargo", "build", "--manifest-path", "apps/megamail/Cargo.toml", "--locked", "-j", "2"], 3600, ["native-tests"], "normal non-test native executable, not installation"),
        ("helper-build", ["make", "-j1", "-C", "tools/megamail-html-reader", "all", "mutation"], 300, ["driver-contracts"], "production C helper/extension and isolated test-only no-CSP variant"),
        ("helper-policy-tests", ["python3", "tools/megamail-html-reader/tests.py", "--helper", "tools/megamail-html-reader/build/megamail-html-reader"], 180, ["helper-build"], "real compiled parser/policy seams; not engine egress proof"),
        ("license-refresh", ["python3", "tools/license-inventory.py"], 900, ["native-build"], "locked dependency/license inventory; no installation"),
    ]


def run_owned(command, output, timeout, env=None):
    """On timeout/interruption, kill only this unreaped owned process group."""
    process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=output,
                               stderr=subprocess.STDOUT, start_new_session=True)
    try:
        return process.wait(timeout=timeout)
    except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        if process.returncode is not None:
            # Popen.wait may already reap during its own SIGINT handling.
            # Never signal a group using an already-released leader PID.
            raise
        # Do not reap the leader before group cleanup: its pinned PID prevents
        # reuse while descendants receive TERM then KILL. No global pkill.
        for sig in (signal.SIGTERM, signal.SIGKILL):
            try:
                os.killpg(process.pid, sig)
            except ProcessLookupError:
                pass
            if sig == signal.SIGTERM:
                time.sleep(0.5)
        process.wait(timeout=5)
        if isinstance(error, KeyboardInterrupt):
            raise
        return 124


def connect_fixture(test_binary, logs):
    """Only isolated loopback SYN drop; never change the host firewall."""
    needed = ["unshare", "ip", "nft", "timeout"]
    if not test_binary or any(not shutil.which(tool) for tool in needed):
        return {"name": "connect-blackhole", "status": "blocked", "proof": "built contract binary and namespace tools required"}
    env = dict(os.environ)
    env["AUTH_TEST_BIN"] = str(test_binary)
    recipe = '''set -eu
ip link set lo up
nft -f - <<'RULES'
table ip megamail_provider_budget {
 chain output {
  type filter hook output priority 0; policy accept;
  oifname "lo" ip daddr 127.0.0.1 tcp dport 49091 tcp flags & syn == syn drop
 }
}
RULES
MEGAMAIL_PROVIDER_BLACKHOLE=isolated-loopback timeout 10s "$AUTH_TEST_BIN" production_transport_budget::production_connect_blackhole_obeys_remaining_authorization_budget --exact --ignored --test-threads=1
'''
    command = ["unshare", "--user", "--map-root-user", "--net", "sh", "-c", recipe]
    with (logs / "connect-blackhole.log").open("w") as log:
        code = run_owned(command, log, 20, env)
    return {"name": "connect-blackhole", "status": "passed" if code == 0 else "failed", "exit_code": code,
            "proof": "isolated SYN timeout calibration + actual production adapter; setup/calibration failure is not a pass"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record-freeze", type=Path, help="Create NEW receipt after all editing owners release; does not build")
    parser.add_argument("--execute", action="store_true", help="Parent-only grouped build/test window")
    parser.add_argument("--freeze-state", type=Path, help="Previously reviewed source receipt required for execute")
    parser.add_argument("--log-dir", type=Path, help="NEW directory for retained logs (default: private owned temp directory)")
    parser.add_argument("--connect-blackhole", action="store_true", help="Run ONLY exact isolated synthetic ignored connect test after compilation")
    args = parser.parse_args()
    if args.record_freeze and args.execute:
        parser.error("Recording a freeze and executing must be separate reviewed actions")
    current = snapshot()
    if current["head"] != BASELINE:
        parser.error("Unexpected HEAD; review the baseline before this batch window")
    if args.record_freeze:
        with args.record_freeze.open("x") as output:
            os.chmod(args.record_freeze, 0o600)
            json.dump(current, output, indent=2)
        print(json.dumps({"mode": "recorded-source-only", "receipt": str(args.record_freeze), "files": len(current["sources"]), "builds": 0}))
        return 0
    if not args.execute:
        print(json.dumps({"mode": "plan-only", "builds": 0, "steps": [
            {"name": name, "command": command, "timeout_seconds": timeout, "needs": needs, "proof": proof}
            for name, command, timeout, needs, proof in steps()
        ], "separate_gates": ["exact isolated SYN-blackhole fixture", "legacy GTK executable if WebKitGTK6 available", "helper descendant request tracing and missing-extension failures", "native isolated GUI QA", "real provider/inline/A11y/physical latency"]}, indent=2))
        return 0
    if not args.freeze_state or json.loads(args.freeze_state.read_text()) != current:
        parser.error("Execution requires a matching manually reviewed whole-batch freeze receipt")
    if args.log_dir:
        args.log_dir.mkdir(mode=0o700, parents=False, exist_ok=False)
        logs = args.log_dir.resolve()
    else:
        logs = Path(tempfile.mkdtemp(prefix="megamail-batch2-verification-"))
    results = []
    statuses = {}
    contract_binary = None
    for name, command, timeout, needs, proof in steps():
        if snapshot() != current:
            results.append({"name": name, "status": "blocked", "proof": "source changed after freeze; remaining steps not run"})
            break
        if any(statuses.get(dependency) != "passed" for dependency in needs):
            statuses[name] = "not_run"
            results.append({"name": name, "status": "not_run", "proof": "required prior gate did not pass"})
            continue
        started = time.monotonic()
        print(f"START {name}: {' '.join(command)}", flush=True)
        log_path = logs / f"{name}.log"
        with log_path.open("w") as output:
            try:
                code = run_owned(command, output, timeout)
            except OSError:
                code = 127
        statuses[name] = "passed" if code == 0 else "failed"
        results.append({"name": name, "status": statuses[name], "exit_code": code,
                        "seconds": round(time.monotonic() - started, 3), "log": str(log_path), "proof": proof})
        if name == "core-tests":
            for line in log_path.read_text(errors="replace").splitlines():
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if event.get("reason") == "compiler-artifact" and event.get("target", {}).get("name") == "provider_auth_contract" and event.get("executable"):
                    contract_binary = Path(event["executable"])
        print(f"END {name}: {statuses[name]} ({code})", flush=True)
    if args.connect_blackhole and statuses.get("core-tests") == "passed" and snapshot() == current:
        results.append(connect_fixture(contract_binary, logs))
    else:
        results.append({"name": "connect-blackhole", "status": "not_run", "proof": "separate explicit isolated gate; ordinary tests leave it ignored"})
    report = {"version": 1, "source_receipt": str(args.freeze_state), "logs": str(logs), "results": results,
              "release_accepted": False, "unresolved_gates": ["helper actual descendant network/request tracing", "native GUI/lease/dirty retry acceptance", "GTK executable availability", "real providers", "inline HTML/A11y/physical latency"]}
    (logs / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 1 if any(item["status"] in {"failed", "blocked"} for item in results) else 0


if __name__ == "__main__":
    raise SystemExit(main())
