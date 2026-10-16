#!/usr/bin/env python3
"""Bounded fictional native observations. Default/dry-run launches NOTHING.

Parent-only execution after source freeze and the grouped build. See
 docs/research/batch2-native-qa-plan.md. No provider, TB, mailto, send, browser,
installer, keyring, or default-handler test is executed here.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time

# The isolated mount is /qa-run.py: indexing parents[1] would abort before
# checking isolation or starting Xvfb. parent.parent is also valid at root.
ROOT = Path(__file__).resolve().parent.parent
TOOLS = ("bwrap", "Xvfb", "xdotool", "magick", "python3")
CHECKS = {
    "selected_body": "Manual pixel/text review; no body-ready/first-paint telemetry.",
    "metadata_links": "Manual wrapping/collapse review; do not open destinations.",
    "source_generation": "No demo causal stale-body injector; use frozen production Rust regressions.",
    "composer_preservation": "Manual Bcc/cursor/hidden-model review; keyboard delivery is not an assertion.",
    "contacts_snippets": "No reviewed reachable demo controls; prepared modules are not UI coverage.",
    "invitation": "Demo metadata is not causally correlated direct BodyAtEpoch evidence.",
    "mailto_ipc": "Demo bypasses activation and rejects --demo plus URI; never launch normal mode.",
    "account_provider_recovery": "Real roster/provider/TB/keyring operations prohibited; separate fixtures required.",
    "helper": "With --helper, fictional mail renders inline; manually inspect captured pixels. The separate developer window remains gated.",
    "physical_performance_accessibility": "Xvfb/software captures prove neither FPS nor accessibility.",
}


def result(name: str, status: str, detail: str, **facts: object) -> dict:
    return dict(name=name, status=status, detail=detail, **facts)


def trusted_file(value: str, executable: bool = True) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError("Executable must be absolute, canonical, and non-symlinked.")
    for part in (path, *path.parents):
        info = part.lstat()
        if info.st_uid not in (0, os.getuid()) or info.st_mode & 0o022:
            raise ValueError("Executable and ancestors must be trusted and not group/other writable.")
    if not path.is_file() or (executable and not os.access(path, os.X_OK)):
        raise ValueError("Executable must be a regular executable file.")
    return path


def source_digest() -> str:
    # A freeze/review receipt, NOT a Rust parser or proof of control flow.
    paths = {ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "apps/megamail/Cargo.toml",
             ROOT / "apps/megamail/Cargo.lock", ROOT / "crates/mail-core/Cargo.toml",
             ROOT / "docs/fixtures/html-reader/batch2/authored.html"}
    for directory in ("apps/megamail/src", "crates/mail-core/src", "src"):
        paths.update((ROOT / directory).rglob("*.rs"))
    for pattern in ("*.c", "*.h", "Makefile"):
        paths.update((ROOT / "tools/megamail-html-reader").glob(pattern))
    digest = hashlib.sha256()
    total = 0
    for path in sorted(paths):
        if path.is_symlink():
            raise ValueError("Source receipt refuses symlinked files.")
        data = path.read_bytes()
        total += len(data)
        if total > 64 * 1024 * 1024:
            raise ValueError("Source receipt exceeds 64 MiB budget.")
        digest.update(str(path.relative_to(ROOT)).encode() + b"\0" + data + b"\0")
    return digest.hexdigest()


def stop(child: subprocess.Popen) -> None:
    # Only live groups created by this harness with start_new_session=True.
    # Do not signal a reaped PID: it could already have been reused.
    if child.poll() is not None:
        return
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        child.wait(timeout=2)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(timeout=2)


def interrupted(signum: int, _frame: object) -> None:
    raise InterruptedError(f"interrupted by signal {signum}")


def sandbox_command(root: Path, binary: Path, args: argparse.Namespace) -> list[str]:
    # Never bind /, /home, /mnt, host /tmp, /run/user, Xauthority, a bus, or /dev/dri.
    command = ["/usr/bin/bwrap", "--die-with-parent", "--new-session", "--unshare-all",
               "--cap-drop", "ALL", "--clearenv", "--ro-bind", "/usr", "/usr"]
    for name in ("bin", "sbin", "lib", "lib64"):
        path = Path("/") / name
        if path.is_symlink():
            command += ["--symlink", os.readlink(path), str(path)]
        elif path.is_dir():
            command += ["--ro-bind", str(path), str(path)]
    command += ["--dir", "/etc"]
    for path in ("/etc/fonts", "/etc/ld.so.cache"):
        if Path(path).exists():
            command += ["--ro-bind", path, path]
    if args.developer_helper_preview:
        helper = trusted_file(args.helper)
        extension = trusted_file(str(helper.parent / "extensions/libmegamail-reader-policy.so"), executable=False)
        command += ["--dir", "/helper", "--dir", "/helper/extensions",
                    "--ro-bind", str(helper), "/helper/megamail-html-reader",
                    "--ro-bind", str(extension), "/helper/extensions/libmegamail-reader-policy.so"]
    command += ["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--tmpfs", "/run",
                "--dir", "/app", "--ro-bind", str(binary), "/app/megamail",
                "--ro-bind", str(Path(__file__).resolve()), "/qa-run.py",
                "--bind", str(root), "/qa", "--chdir", "/qa",
                "--setenv", "PATH", "/usr/bin:/bin", "--setenv", "LANG", "C.UTF-8",
                "--setenv", "HOME", "/qa/home", "--setenv", "XDG_CONFIG_HOME", "/qa/config",
                "--setenv", "XDG_DATA_HOME", "/qa/data", "--setenv", "XDG_CACHE_HOME", "/qa/cache",
                "--setenv", "XDG_STATE_HOME", "/qa/state", "--setenv", "XDG_RUNTIME_DIR", "/qa/run",
                "--setenv", "TMPDIR", "/tmp", "--setenv", "LIBGL_ALWAYS_SOFTWARE", "1",
                "--setenv", "GSETTINGS_BACKEND", "memory", "--setenv", "GTK_A11Y", "none"]
    if args.helper:
        helper = trusted_file(args.helper)
        extension = trusted_file(str(helper.parent / "extensions/libmegamail-reader-policy.so"), executable=False)
        command += ["--dir", "/qa/data/megamail", "--dir", "/qa/data/megamail/html-reader",
                    "--dir", "/qa/data/megamail/html-reader/extensions",
                    "--ro-bind", str(helper), "/qa/data/megamail/html-reader/megamail-html-reader",
                    "--ro-bind", str(extension), "/qa/data/megamail/html-reader/extensions/libmegamail-reader-policy.so"]
    if args.helper:
        runtime = Path(args.helper).parent / "runtime"
        if runtime.is_dir():
            if runtime.resolve(strict=True) != runtime:
                raise ValueError("Runtime directory must be canonical, not symlinked.")
            command += ["--ro-bind", str(runtime), "/qa/data/megamail/html-reader/runtime"]
    command += ["--", "/usr/bin/python3", "/qa-run.py", "--inside", "--execute",
                "--timeout", str(args.timeout), "--settle", str(args.settle), "--theme", args.theme]
    if args.developer_helper_preview:
        command += ["--developer-helper-preview"]
    return command


def inside(args: argparse.Namespace) -> int:
    """Only entered inside the owned namespace, never as a host execution shortcut."""
    root = Path("/qa")
    receipt = json.loads((root / "receipt.json").read_text())
    if (os.environ.get("HOME") != "/qa/home"
            or os.readlink("/proc/self/ns/net") == receipt["host_net_namespace"]
            or os.environ.get("DISPLAY") or os.environ.get("DBUS_SESSION_BUS_ADDRESS")
            or Path("/home").exists() or Path("/mnt").exists()
            or Path("/run/user").exists() or Path("/dev/dri").exists()):
        raise ValueError("Isolation preconditions not established; native execution blocked.")
    os.umask(0o077)
    deadline = time.monotonic() + args.timeout
    observations: list[dict] = []
    children: list[subprocess.Popen] = []
    logs = []
    started = time.monotonic()

    def run(command: list[str]) -> str:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("whole-session deadline reached")
        return subprocess.run(command, check=True, capture_output=True, text=True,
                              timeout=min(5, remaining)).stdout.strip()

    def pause() -> None:
        remaining = deadline - time.monotonic()
        if remaining < args.settle:
            raise TimeoutError("no remaining settle budget")
        time.sleep(args.settle)
        if app.poll() is not None:
            raise RuntimeError("native demo exited during observation")

    def capture(label: str) -> None:
        # Revalidate ownership immediately before EVERY window-only capture/action.
        owner = int(run(["xdotool", "getwindowpid", window]))
        if owner != app.pid or app.poll() is not None:
            raise RuntimeError("window no longer belongs to the owned native process")
        geometry = run(["xdotool", "getwindowgeometry", "--shell", window])
        facts = dict(window_id=window, owning_pid=owner, display=os.environ["DISPLAY"],
                     geometry=geometry, utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                     elapsed_seconds=round(time.monotonic() - started, 3), fictional=True,
                     renderer="Xvfb/software; not physical FPS", label=label)
        target = root / "artifacts" / (label + ".png")
        run(["magick", "import", "-window", window, str(target)])
        facts["file"] = target.name
        observations.append(result("capture", "not_run", "Observation only; semantic/manual review pending.", **facts))

    try:
        # Xvfb allocates its own display in namespace-private /tmp; no personal auth.
        log = (root / "xvfb.log").open("w")
        logs.append(log)
        xvfb = subprocess.Popen(["Xvfb", "-displayfd", "1", "-screen", "0", "1600x1000x24",
                                 "-nolisten", "tcp", "-ac"], stdout=subprocess.PIPE,
                                stderr=log, start_new_session=True)
        children.append(xvfb)
        # select, rather than unbounded readline if display startup fails.
        import select
        ready, _, _ = select.select([xvfb.stdout], [], [], min(8, args.timeout))
        if not ready:
            raise TimeoutError("owned Xvfb did not allocate a display")
        display = xvfb.stdout.readline(16).decode().strip()
        if not display.isdecimal() or xvfb.poll() is not None:
            raise RuntimeError("owned Xvfb allocation failed")
        os.environ["DISPLAY"] = ":" + display
        log = (root / "native.log").open("w")
        logs.append(log)
        preferences = root / "config/megamail/theme.v1"
        preferences.parent.mkdir(parents=True, exist_ok=True)
        preferences.write_text(json.dumps({"mode": args.theme}))
        app = subprocess.Popen(["/app/megamail", "--demo"], stdout=log, stderr=log,
                               start_new_session=True)
        children.append(app)
        window = ""
        while time.monotonic() < deadline:
            if app.poll() is not None:
                raise RuntimeError("native demo exited before window discovery")
            try:
                windows = run(["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)])
                candidates = windows.splitlines()
                if len(candidates) == 1:
                    window = candidates[0]
                    break
            except subprocess.CalledProcessError:
                pass
            time.sleep(0.1)
        if not window:
            raise TimeoutError("no unique owned native window")
        title = run(["xdotool", "getwindowname", window])
        if "Local demo" not in title:
            raise RuntimeError("owned window not labeled as demo; refusing interaction")
        observations.append(result("demo_window", "pass", "Only PID ownership and demo title asserted; not body readiness.",
                                   window_id=window, owning_pid=app.pid, title=title))
        run(["xdotool", "windowmove", "--sync", window, "0", "0"])
        pause()
        capture("startup")
        # Only non-destructive focus/navigation keys; no URLs/f5/send.
        for label, keys in (("select-next", ["ctrl+alt+l", "Down"]),
                            ("select-previous", ["ctrl+alt+l", "Up"]),
                            ("reader-focus", ["ctrl+alt+r"]),
                            ("all-sent", ["ctrl+alt+2"]),
                            ("all-drafts", ["ctrl+alt+3"]),
                            ("all-inboxes", ["ctrl+shift+l"]),
                            ("sort-sender", ["ctrl+alt+s"])):
            capture(label + "-before")
            run(["xdotool", "windowfocus", "--sync", window])
            for key in keys:
                run(["xdotool", "key", "--clearmodifiers", key])
            pause()
            capture(label)
        for width, height in ((1060, 640), (1520, 900)):
            capture(f"resize-{width}x{height}-before")
            run(["xdotool", "windowsize", window, str(width), str(height)])
            pause()
            capture(f"resize-{width}x{height}")
        # Guarded compose request, NOT proof that a composer opened or preserved fields.
        capture("compose-request-before")
        run(["xdotool", "windowfocus", "--sync", window])
        run(["xdotool", "key", "--clearmodifiers", "ctrl+n"])
        pause()
        capture("compose-request")
        # Fictional text and known coordinates from the reviewed 1520x900 capture.
        # All clicks target this isolated owned window; demo cannot send/save mail.
        def click(x: int, y: int) -> None:
            if int(run(["xdotool", "getwindowpid", window])) != app.pid:
                raise RuntimeError("refusing click outside the owned demo")
            run(["xdotool", "mousemove", "--window", window, str(x), str(y), "click", "1"])
        click(850, 398)
        run(["xdotool", "type", "--clearmodifiers", "--delay", "1", "Fictional QA retained draft"])
        click(850, 500)
        run(["xdotool", "type", "--clearmodifiers", "--delay", "1", "**Fictional formatted draft** - retained while browsing."])
        click(860, 438)
        pause()
        click(1000, 438)
        pause()
        capture("formatted-compose")
        click(90, 191)
        pause()
        capture("draft-retained-in-sent")
        click(110, 331)
        pause()
        capture("draft-resumed")
        observations.append(result("interaction_observations", "not_run", "Inputs delivered; inspect captures manually. No field/model assertion."))
        returncode = 0
        if args.developer_helper_preview:
            # Separate developer-only engine probe, never native gate enablement.
            # No helper screenshot: nested PID namespaces require a separate owning-PID audit.
            request = dict(version=1, title="Fictional Batch 2 QA", plainText="Fictional local formatting fixture.",
                           html='<html><head><style>body{color:#223;background:#fff}table{border-collapse:collapse}'
                                '@media(max-width:600px){td{display:block}}</style></head><body>'
                                '<h1>Fictional Batch 2 QA</h1><table><tr><td>Left sample</td>'
                                '<td>Right sample</td></tr></table><p>No browser or mail action.</p></body></html>',
                           resources=[])
            output = (root / "helper.jsonl").open("w")
            errors = (root / "helper.log").open("w")
            logs.extend((output, errors))
            helper = subprocess.Popen(["/helper/megamail-html-reader"], stdin=subprocess.PIPE,
                                      stdout=output, stderr=errors, start_new_session=True)
            children.append(helper)
            helper.stdin.write(json.dumps(request).encode())
            helper.stdin.close()
            helper_deadline = min(deadline, time.monotonic() + 15)
            ready_event = None
            while time.monotonic() < helper_deadline and helper.poll() is None:
                data = (root / "helper.jsonl").read_bytes()
                if len(data) > 65536:
                    raise RuntimeError("helper protocol observation exceeded 64 KiB")
                for line in data.splitlines(keepends=True):
                    if not line.endswith(b"\n"):
                        continue
                    event = json.loads(line)
                    if event.get("version") == 1 and event.get("event") == "ready":
                        ready_event = event
                if ready_event:
                    break
                time.sleep(0.1)
            observations.append(result("helper_readiness_observation", "not_run" if ready_event else "blocked",
                                       "Ready event observed only; no fidelity, inline, accessibility or security acceptance."
                                       if ready_event else "No ready event within budget; inspect fictional helper logs.",
                                       owning_launcher_pid=helper.pid, fictional=True))
            if not ready_event:
                returncode = 1
    except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.SubprocessError, InterruptedError) as error:
        observations.append(result("session", "fail", str(error)))
        returncode = 1
    finally:
        for child in reversed(children):
            try:
                stop(child)
            except (OSError, subprocess.SubprocessError) as error:
                observations.append(result("cleanup", "fail", type(error).__name__))
                returncode = 1
        for log in logs:
            log.close()
        (root / "observations.json").write_text(json.dumps(observations, indent=2) + "\n")
    return returncode


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", help="Absolute canonical parent-built executable; no PATH lookup")
    p.add_argument("--helper", help="Optional trusted compiled helper for isolated fictional inline rendering")
    p.add_argument("--developer-helper-preview", action="store_true",
                   help="Separate synthetic helper probe only; never enables native original-formatting release gate")
    p.add_argument("--dry-run", action="store_true", help="Plan only, even if --execute was supplied")
    p.add_argument("--execute", action="store_true", help="Parent opt-in AFTER source review/freeze/grouped build")
    p.add_argument("--reviewed-source-sha256", help="Receipt from dry-run AFTER manually reviewing final demo isolation")
    p.add_argument("--output-dir", help="New absolute directory for fictional artifacts; parent must already exist")
    p.add_argument("--timeout", type=int, default=60, help="Whole inner session budget, 20..120 seconds")
    p.add_argument("--settle", type=float, default=1.0, help="Fixed observation pause, 0.2..3 seconds; not readiness telemetry")
    p.add_argument("--theme", choices=("light", "dark"), default="light", help="Synthetic profile theme")
    p.add_argument("--inside", action="store_true", help=argparse.SUPPRESS)
    return p


def main() -> int:
    args = parser().parse_args()
    os.umask(0o077)
    if not 20 <= args.timeout <= 120 or not 0.2 <= args.settle <= 3:
        parser().error("timeout must be 20..120 and settle 0.2..3")
    for signum in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT):
        signal.signal(signum, interrupted)
    if args.inside:
        if args.dry_run or not args.execute:
            print(json.dumps(dict(mode="blocked", results=[result("internal_entry", "blocked", "Internal execution requires the parent's isolated execute path.")])))
            return 2
        return inside(args)
    report = [result(name, "blocked" if name in ("source_generation", "mailto_ipc", "contacts_snippets", "invitation")
                     else "not_run", detail) for name, detail in CHECKS.items()]
    try:
        digest = source_digest()
        binary = trusted_file(args.binary) if args.binary else None
        helper = trusted_file(args.helper) if args.helper else None
        if args.developer_helper_preview and not helper:
            raise ValueError("Developer helper probe requires an explicit trusted --helper.")
        if args.developer_helper_preview:
            trusted_file(str(helper.parent / "extensions/libmegamail-reader-policy.so"), executable=False)
        dependencies = {name: shutil.which(name, path="/usr/bin:/bin") for name in TOOLS}
        metadata = dict(source_sha256=digest, binary=str(binary) if binary else None,
                        helper=str(helper) if helper else None, developer_helper_preview=args.developer_helper_preview,
                        dependencies=dependencies,
                        limits=dict(timeout=args.timeout, settle=args.settle), results=report)
        if args.dry_run or not args.execute:
            metadata["mode"] = "dry_run" if args.dry_run else "not_run"
            print(json.dumps(metadata, indent=2))
            return 0
        if not binary or args.reviewed_source_sha256 != digest:
            raise ValueError("Execution blocked: explicit binary and matching manually reviewed freeze receipt required.")
        if not all(dependencies.values()):
            raise ValueError("Execution blocked: missing isolation/capture dependencies; no fallback.")
        destination = None
        if args.output_dir:
            destination = Path(args.output_dir)
            if (not destination.is_absolute() or destination.exists() or destination.is_symlink()
                    or destination.parent.resolve(strict=True) != destination.parent):
                raise ValueError("Output must be a NEW absolute canonical directory with an existing parent.")
            destination.mkdir(mode=0o700)
        with tempfile.TemporaryDirectory(prefix="megamail-native-batch2-qa-") as temporary:
            root = Path(temporary)
            for directory in ("home", "config", "data", "cache", "state", "run", "artifacts"):
                (root / directory).mkdir(mode=0o700)
            with binary.open("rb") as stream:
                binary_sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
            receipt = dict(host_net_namespace=os.readlink("/proc/self/ns/net"), source_sha256=digest,
                           binary_sha256=binary_sha256)
            (root / "receipt.json").write_text(json.dumps(receipt))
            command = sandbox_command(root, binary, args)
            child = None
            code = 1
            try:
                with (root / "sandbox.log").open("w") as log:
                    child = subprocess.Popen(command, env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
                                             stdout=log, stderr=log, start_new_session=True)
                    code = child.wait(timeout=args.timeout + 15)
            except (OSError, InterruptedError, subprocess.SubprocessError) as error:
                report.append(result("sandbox", "blocked", type(error).__name__))
            finally:
                if child is not None:
                    stop(child)
                observed = root / "observations.json"
                if observed.exists():
                    report.extend(json.loads(observed.read_text()))
                else:
                    report.append(result("sandbox", "blocked", "No isolated session results; inspect sandbox.log. No weaker retry."))
                metadata["mode"] = "execute"
                metadata["exit_code"] = code
                if destination:
                    # Export only fictional capture/log/receipt files, never the temporary HOME/profile.
                    for name in ("receipt.json", "observations.json", "native.log", "xvfb.log", "sandbox.log", "helper.jsonl", "helper.log"):
                        if (root / name).is_file():
                            shutil.copyfile(root / name, destination / name)
                    shutil.copytree(root / "artifacts", destination / "artifacts")
                    (destination / "result.json").write_text(json.dumps(metadata, indent=2) + "\n")
            print(json.dumps(metadata, indent=2))
            return int(code != 0)
    except (OSError, ValueError, InterruptedError, subprocess.SubprocessError) as error:
        report.append(result("preconditions", "blocked", str(error)))
        print(json.dumps(dict(mode="blocked", results=report), indent=2))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
