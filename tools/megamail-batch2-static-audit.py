#!/usr/bin/env python3
"""Read-only checks for this desktop batch, NOT a security or parser test.

Default: scan only the four owned paths; report unrelated concurrent changes
without attributing them to this owner. --require-integration additionally
requires basic native call-site markers (not proof that routing works).
--self-test tests these audit functions with fictional source snippets only.
No Cargo, Rust compiler, desktop settings, profile, or provider operations.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess

OWNED = (
    "crates/mail-core/src/desktop.rs",
    "apps/megamail/src/desktop_actions.rs",
    "docs/research/batch2-desktop-privacy.md",
    "tools/megamail-batch2-static-audit.py",
)
MARKER = "/// Maximum encoded external compose URI size"
# Deliberately simple text scan: comments can cause false positives; aliases,
# macros, indirect calls and dependencies can evade it. Review findings manually.
FORBIDDEN = {
    "process or file I/O": r"\b(?:std::process|std::fs|Command::|OpenOptions::|File::open)",
    "network runtime/client": r"\b(?:reqwest|TcpStream|UdpSocket|ureq|tokio)::",
    "outbound/mail mutation dispatch": r"\b(?:MailRequest|send_request|send_message|queue_send)\b",
    "external launch or registration": r"\b(?:open_url|register_url_scheme)\s*\(",
    "panic/unchecked production result": r"\b(?:panic!|\.unwrap\s*\(|\.expect\s*\()",
    "unsafe Rust": r"\bunsafe\s*\{",
}


def production(text: str) -> str:
    # Not a Rust parser. Tests live after this exact marker in owned modules.
    return text.split("#[cfg(test)]", 1)[0]


def scan(text: str) -> list[str]:
    return [name for name, pattern in FORBIDDEN.items() if re.search(pattern, text)]


def integration_markers(main: str) -> list[str]:
    markers = {
        "module declaration": r"\bmod\s+desktop_actions\s*;",
        "native key binding seam": r"desktop_actions::bind_keys\s*\(",
        "external URI parser call": r"desktop_actions::external_compose\s*\(",
        "compose payload seam": r"desktop_actions::compose_message\s*\(",
        "action policy guard": r"\.allows\s*\(",
        "mailbox-only key context": r"desktop_actions::MAILBOX_CONTEXT",
    }
    return [name for name, pattern in markers.items() if not re.search(pattern, main)]


def git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args], check=True, capture_output=True, text=True
    )
    return result.stdout


def self_test() -> None:
    assert not scan("pub fn parser(uri: &str) -> Result<String, Error> { decode(uri) }")
    assert "external launch or registration" in scan("cx.open_url(uri);")
    assert "process or file I/O" in scan("std::process::Command::new(\"x\");")
    assert "panic/unchecked production result" in scan("value.unwrap();")
    assert not scan(production("fn production() {}\n#[cfg(test)]\nfn test() { x.unwrap(); }"))
    assert len(integration_markers("fn main() {}")) == 6
    print("PASS: audit scanner self-tests only; production Rust parser NOT executed.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--require-integration", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    root = Path(__file__).resolve().parents[1]
    failures: list[str] = []
    contents: dict[str, str] = {}
    for relative in OWNED:
        path = root / relative
        if path.is_symlink() or not path.is_file():
            failures.append(f"missing or symlinked owned path: {relative}")
            continue
        contents[relative] = path.read_text(encoding="utf-8")

    desktop = contents.get(OWNED[0], "")
    actions = contents.get(OWNED[1], "")
    if MARKER not in desktop:
        failures.append("portable parser section marker missing")
    else:
        for name in scan(production(desktop.split(MARKER, 1)[1])):
            failures.append(f"desktop parser section: {name}")
    for name in scan(production(actions)):
        failures.append(f"native desktop module: {name}")
    if "megamail_core::desktop::parse_mailto(uri)" not in actions:
        failures.append("native external_compose no longer delegates to core parser")
    if "crate::compose::new_message(account_id)" not in actions:
        failures.append("compose adapter no longer uses production message initializer")

    try:
        original = git(root, "show", "HEAD:" + OWNED[0])
        # Preserve the existing settings API bodies, excluding module docs and
        # the additive parser. This HEAD comparison is not an ownership ledger.
        old_settings = original.split("/// The string value", 1)[1].rstrip()
        new_settings = desktop.split("/// The string value", 1)[1].split(MARKER, 1)[0].rstrip()
        if old_settings != new_settings:
            failures.append("existing portable settings helpers differ from HEAD; inspect GTK compatibility")
        status = git(root, "status", "--porcelain=v1", "--untracked-files=all")
        changed = [line[3:] for line in status.splitlines()]
        owned_changes = [name for name in changed if name in OWNED]
        outside = [name for name in changed if name not in OWNED]
        print("Changed owned paths: " + ", ".join(owned_changes))
        print(f"Other checkout changes: {len(outside)} (not attributed to this owner).")
    except (subprocess.CalledProcessError, OSError, IndexError) as error:
        failures.append(f"could not inspect repository baseline: {type(error).__name__}")

    native_main = (root / "apps/megamail/src/main.rs").read_text(encoding="utf-8")
    missing = integration_markers(native_main)
    if missing:
        print("INTEGRATION PENDING (text markers only): " + ", ".join(missing))
        if args.require_integration:
            failures.append("native integration markers missing")
    else:
        print("Integration text markers present; behavior and Linux handoff are NOT validated.")
    for failure in failures:
        print("FAIL: " + failure)
    print("LIMIT: no production parser execution, compilation, security proof, Linux runtime, or performance measurement.")
    if not failures:
        print("PASS: bounded owned-source text checks only.")
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
