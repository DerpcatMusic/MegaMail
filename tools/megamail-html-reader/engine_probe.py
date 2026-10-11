#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Parent-only post-build synthetic readiness/heartbeat probe through real IPC.
Does not assert rendered layout, selection or zero egress. Use process tracing
and interactive acceptance separately. Does not open a provider/mail profile.
"""
import argparse
import json
import os
import selectors
import subprocess
import time
from pathlib import Path
from fixture_request import request


def probe(helper, fixture, no_csp):
    env = {k: os.environ[k] for k in ["DISPLAY", "GDK_BACKEND", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"] if k in os.environ}
    child = subprocess.Popen([str(helper.resolve())], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env)
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ)
    pending = bytearray()
    token = None
    ready_at = None
    deadline = time.monotonic() + 18
    try:
        child.stdin.write(json.dumps(request(fixture)).encode())
        child.stdin.close()
        while time.monotonic() < deadline:
            for key, _ in selector.select(0.25):
                chunk = os.read(key.fd, 8192)
                if not chunk:
                    raise AssertionError("helper closed before expected readiness/heartbeat")
                pending.extend(chunk)
                while b"\n" in pending:
                    line, _, tail = pending.partition(b"\n")
                    pending = bytearray(tail)
                    if len(line) > 8192:
                        raise AssertionError("unbounded helper event")
                    event = json.loads(line)
                    if event.get("version") != 1:
                        raise AssertionError("protocol version")
                    if event["event"] == "error":
                        raise AssertionError("engine failed closed: " + event.get("code", "unknown"))
                    if event["event"] == "ready":
                        expected = "test-only:no-csp" if no_csp else "v1:extension+sandbox+ephemeral+no-network"
                        assert event.get("policy") == expected
                        token = event["token"]
                        assert len(token) == 32 and all(c in "0123456789abcdef" for c in token)
                        ready_at = time.monotonic()
                    elif event["event"] == "heartbeat" and token and event.get("token") == token:
                        if time.monotonic() - ready_at >= 1:
                            print(json.dumps({"real_helper_IPC": "ready-and-heartbeat", "fixture": fixture,
                                              "no_csp_mutation": no_csp, "render_input_a11y_network_acceptance": "not-proven"}))
                            return
                if len(pending) > 8192:
                    raise AssertionError("unbounded helper event")
        raise AssertionError("engine readiness timeout")
    finally:
        selector.close()
        child.terminate()
        try:
            child.wait(timeout=2)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=2)
        child.stdout.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--helper", type=Path, required=True)
    parser.add_argument("--fixture", choices=["authored", "hostile", "parser-confusion"], default="authored")
    parser.add_argument("--test-only-no-csp", action="store_true")
    args = parser.parse_args()
    probe(args.helper, args.fixture, args.test_only_no_csp)
