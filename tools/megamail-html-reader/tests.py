#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Source-contract audit now; actual C policy seam tests only after parent builds.
Source audit is NOT engine security evidence or a duplicate sanitizer.
"""
import argparse
import base64
import json
import struct
import subprocess
import zlib
from html.parser import HTMLParser
from pathlib import Path

from fixture_request import request, PNG

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]


def source_audit():
    main = (ROOT / "main.c").read_text()
    extension = (ROOT / "extension.c").read_text()
    policy = (ROOT / "policy.c").read_text()
    sandbox = (ROOT / "sandbox.c").read_text()
    header = (ROOT / "policy.h").read_text()
    host = (REPO / "apps/megamail/src/reader_host.rs").read_text()
    doc = (REPO / "apps/megamail/src/reader_document.rs").read_text()
    checks = [
        ("pre-dispatch shared request policy", 'return !mm_request_allowed(' in extension),
        ("hook precedes acknowledgement", extension.index('"send-request"') < extension.index('webkit_web_page_send_message_to_view', extension.index('static void page_created'))),
        ("untrusted load guarded by acknowledgement", 'r->acknowledged && !r->loading' in main),
        ("ephemeral and sandbox readback", 'webkit_web_context_get_sandbox_enabled' in main and 'webkit_web_context_is_ephemeral' in main),
        ("JS and JS markup disabled", 'webkit_settings_set_enable_javascript(settings, FALSE)' in main and 'webkit_settings_set_enable_javascript_markup(settings, FALSE)' in main),
        ("header CSP and parser reconstruction", 'Content-Security-Policy' in main and 'htmlReadMemory' in policy and 'xmlNewChild' in policy),
        ("pixel and resource limits", 'size-prepared' in policy and 'MM_TOTAL_PIXELS_MAX' in policy and 'resource-normalized-size' in policy),
        ("real network namespace and identity verification", '"--unshare-net"' in sandbox and 'original.st_ino != current.st_ino' in sandbox),
        ("private empty read-only sys hierarchy", all(part in sandbox for part in ['"--tmpfs", "/sys"', '"/sys/block"', '"/sys/bus"', '"/sys/class"', '"/sys/dev"', '"/sys/devices"', '"--remount-ro", "/sys"']) and 'ro_bind(args, "/sys"' not in sandbox),
        ("no home or bus mount", '"--clearenv"' in sandbox and '"--tmpfs", "/home"' in sandbox and 'DBUS_SESSION_BUS_ADDRESS' not in sandbox),
        ("native spawn has no shell", 'Command::new(executable)' in host and '.env_clear()' in host and 'Command::new("sh")' not in host),
        ("native source stays separate", 'original_html: Option<Arc<str>>' in doc and 'semantic_html: Arc<str>' in doc),
        ("incomplete source cannot reach helper", 'self.completeness != Completeness::Complete' in doc),
        ("test mutation rejected by native policy", 'test-only:no-csp' in header and 'test-only:no-csp' not in host),
        ("no unsafe JS bridge", all(term not in main for term in ['run_javascript', 'evaluate_javascript', 'webkit_user_content_manager_register_script_message_handler'])),
    ]
    for name, passed in checks:
        if not passed:
            raise AssertionError(name)
    # Header availability only; never invoke a compiler for this audit.
    symbols = set()
    import re
    for text in [main, extension]:
        symbols.update(re.findall(r'\b(webkit_[a-z0-9_]+)\s*\(', text))
    include = subprocess.check_output(["pkg-config", "--variable=includedir", "webkit2gtk-4.1"], text=True).strip()
    installed = "\n".join(p.read_text(errors="replace") for p in (Path(include) / "webkitgtk-4.1").rglob("*.h"))
    # The extension entrypoint is exported by us, not declared by WebKit.
    symbols.discard("webkit_web_extension_initialize_with_user_data")
    missing = sorted(s for s in symbols if s not in installed)
    if missing:
        raise AssertionError("installed header APIs absent: " + ", ".join(missing))
    print(json.dumps({"source_contract_checks": len(checks), "installed_header_symbols": len(symbols),
                      "compiled": False, "engine_security_proven": False}))


class Tree(HTMLParser):
    def __init__(self, html):
        super().__init__()
        self.tags = []
        self.attrs = []
        self.feed(html)

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        self.attrs.extend((tag, name, value) for name, value in attrs)


def run_policy(helper, obj, accepted=True):
    payload = obj if isinstance(obj, bytes) else json.dumps(obj).encode()
    result = subprocess.run([str(helper), "--policy-test"], input=payload, capture_output=True, timeout=15)
    if (result.returncode == 0) != accepted:
        raise AssertionError("C production policy acceptance mismatch")
    records = [json.loads(line) for line in result.stdout.splitlines()]
    if not accepted:
        assert records[-1]["event"] == "error"
        return ""
    assert records[-1]["event"] == "policy-document"
    return records[-1]["html"]


def compiled_policy_tests(helper):
    result = subprocess.run([str(helper), "--self-test"], capture_output=True, timeout=15)
    assert result.returncode == 0, result.stderr.decode(errors="replace")
    print(result.stdout.decode().strip())
    authored = run_policy(helper, request())
    assert "@media" in authored and "<table" in authored and "megamail-resource://" in authored
    assert "wallpaper" not in authored
    for name in ["hostile", "parser-confusion"]:
        html = run_policy(helper, request(name))
        tree = Tree(html)
        forbidden = {"script", "iframe", "svg", "math", "object", "embed", "form", "input", "button", "source", "link", "base"}
        assert not forbidden.intersection(tree.tags)
        for tag, attr, value in tree.attrs:
            assert not attr.startswith("on")
            assert attr not in {"srcset", "srcdoc", "ping", "action", "formaction", "download"}
            if attr == "src":
                assert value.startswith("megamail-resource://")
            if attr == "href":
                assert value.startswith(("https://", "http://")) and "user:secret" not in value
        assert "form-action 'none'" in html
    bad = request(); bad["version"] = 2
    run_policy(helper, bad, False)
    bad = request(); bad["profilePath"] = "/home/forbidden"
    run_policy(helper, bad, False)
    bad = request(); bad["html"] = "x\0y"
    run_policy(helper, bad, False)
    bad = request(); bad["html"] = "x" * (2 * 1024 * 1024 + 1)
    run_policy(helper, bad, False)
    bad = request(); bad["resources"][0]["mime"] = "image/svg+xml"
    run_policy(helper, bad, False)
    bad = request(); bad["resources"][0]["base64"] = PNG + "!!!"
    run_policy(helper, bad, False)
    bad = request(); bad["resources"].append({**bad["resources"][0], "cid": "<LOGO@fixture.invalid>"})
    run_policy(helper, bad, False)
    bad = request(); bad["resources"] *= 33
    run_policy(helper, bad, False)
    # Valid large PNG header, no decompression required to trip the pixel gate.
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    bomb = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 8193, 8193, 8, 2, 0, 0, 0)) + chunk(b"IEND", b"")
    bad = request(); bad["resources"][0]["base64"] = base64.b64encode(bomb).decode()
    run_policy(helper, bad, False)
    run_policy(helper, b'{"version":1,"html":"x","plainText":"x","title":"x"}\0', False)
    run_policy(helper, b"[" * 100 + b"]" * 100, False)
    print(json.dumps({"production_C_policy_fixtures": "pass", "engine_security_proven": False}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--helper", type=Path, help="Parent-only: already-built helper, no compilation occurs")
    args = parser.parse_args()
    source_audit()
    if args.helper:
        compiled_policy_tests(args.helper.resolve())
