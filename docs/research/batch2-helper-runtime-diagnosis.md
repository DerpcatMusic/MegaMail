# Batch 2: compiled helper runtime diagnosis

**Release gate remains CLOSED.** The production executable's startup failure is an outer/inner sandbox filesystem-contract mismatch, not an extension failure or fatal DRI3 dependency in this fictional X11 case.

## Evidence and controlled delta

2026-10-10; installed Arch packages: WebKitGTK 4.1 **2.54.1-1**, bubblewrap **0.13.0-1.1**. Graft returned no matching nodes. Read helper README, sandbox.c, main.c, engine_probe.py and batch2-html-reader.md fully. No compiler/build, source, manifest, lockfile or test-script writes occurred. Only this new Markdown file was written inside the repository.

Owned evidence/launchers: `/tmp/megamail-helper-research.YK6Grp/`. Corrected baseline and experiment each used a fresh, explicitly selected **:35791** Xvfb, absent socket/lock preconditions, TCP disabled, fictional authored fixture, clean environment and core-dump limit zero.

Baseline: **134**, empty stdout. In `baseline.trace`:

- Line 13425 executes inner `bwrap --args 29 -- /usr/lib/webkit2gtk-4.1/WebKitWebProcess 4 24`.
- The sealed memfd argument write contains unconditional read-only binds for **/sys/block, /sys/bus, /sys/class, /sys/dev, /sys/devices**.
- Lines 13596–13601: resolving **/sys** returns ENOENT; bubblewrap prints `Can't find source path /sys/block`, exits 1.
- Line 13678: credential-channel `recvmsg` returns **0** for a one-byte buffer. Line 13689 prints the credential error; UI process aborts. WebKitWebProcess never executes.

External experiment: replayed every existing X11 launcher argument, adding **only** a read-only bind of a newly created private directory containing five empty children at /sys. It did not bind host sysfs, devices, credentials, buses or home. The genuine original network namespace FD remained inherited on **3**. Trace lines 1144–1146 show namespace inodes **4026531833 → 4026533699**, satisfying the existing compiled verifier; user/PID/IPC/UTS/network isolation, capability drop, read-only package/runtime, ephemeral paths and display grant remained.

Result: actual WebKitWebProcess exec, sibling extension opened, **production READY + seven matching-token heartbeats**. DRI3 warnings persist, so they are nonfatal here. Extension acknowledgement/document-load conditions in main.c therefore succeed in this case. No no-CSP variant ran; its enforcement remains unproven.

The 14-second bounded experiment was deliberately stopped, not a natural successful exit. Subreaper cleanup reaped all adopted children; both corrected runs report zero owned children and removed Xvfb socket. Diagnostic files remain privately owned outside the repo. An initial automatic-display attempt selected :0, whose existing socket made ownership ambiguous; excluded, no host authorization changed, its child processes reaped. All conclusions use the corrected dedicated-display runs.

## Primary sources and installed API

- [Pinned WebKitGTK 2.54.1 BubblewrapLauncher.cpp](https://github.com/WebKit/WebKit/blob/webkitgtk-2.54.1/Source/WebKit/UIProcess/Launcher/glib/BubblewrapLauncher.cpp#L790-L1015): mandatory sys paths, sealed argument FD, seccomp and subprocess dispatch.
- [Pinned ConnectionGLib.cpp](https://github.com/WebKit/WebKit/blob/webkitgtk-2.54.1/Source/WebKit/Platform/IPC/glib/ConnectionGLib.cpp#L534-L545): failed credential receive is fatal, explaining the downstream abort.
- [Sandbox API](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebContext.set_sandbox_enabled.html): enable before subprocess creation.
- [Extra-path API](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebContext.add_path_to_sandbox.html): **/sys paths are invalid**, so this is not an API-extra-path fix. Installed `/usr/include/webkitgtk-4.1/webkit/WebKitWebContext.h:217–226` confirms signatures.

## Smallest future source proposal / security review

In sandbox.c, create a private tmpfs /sys, its five empty directories, then remount /sys read-only. Installed bubblewrap supports these options. This exact source recipe is **proposed, not tested or compiled**; tested equivalent was an external read-only empty tree. Preserve every current flag/check and inner sandbox; never expose host /sys or /sys/devices. Review immutable emptiness, no symlinks, nested namespace/FD integrity, missing-path fail-closed behavior, dependency upgrades and descendant teardown before another serialized build.

Readiness is **not** no-egress/request-hook enforcement, pixels/layout/CID fidelity, selection/clipboard, accessibility or graceful cancellation acceptance. Private-bus A11y warning remains. The 35 C policy assertions and Native 267/core 599 results are separate, parent-reported evidence, not engine/security release proof.

## Reproduction commands (owned temporary launchers retained)

```sh
D=/tmp/megamail-helper-research.YK6Grp
ulimit -c 0
timeout --kill-after=5s 30s env -i PATH=/usr/bin PYTHONDONTWRITEBYTECODE=1 \
  python3 "$D/diagnose.py" baseline
timeout --kill-after=5s 30s env -i PATH=/usr/bin PYTHONDONTWRITEBYTECODE=1 \
  python3 "$D/diagnose.py" empty
# diagnose.py owns/reaps Xvfb and helper descendants; strace follows forks:
# strace -f -v -yy -s 65535 -o MODE.trace \
#   -e trace=execve,write,openat,newfstatat,readlink,clone,clone3,unshare,mount,recvmsg,sendmsg,wait4,exit_group,prctl,setns,fstat,fcntl,dup2 ...
# contained.py duplicates a legitimate /proc/self/ns/net FD to 3, clears CLOEXEC,
# then execs the unchanged outer X11 contract plus:
# --ro-bind /tmp/megamail-helper-research.YK6Grp/empty-sys /sys
# before /app/megamail-html-reader --contained
```

Compiled artifact SHA-256:

```text
helper     6887e82be32b7f85ae2fa2f0332bc38f0f23c127242a556a8400d6af98d2cc33
extension  1c21b3d81abdfac75361f87fe3df5825eba32f7dfd254d996a3897fbfbff411f
```
