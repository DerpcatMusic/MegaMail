# MegaMail original-formatting helper (candidate, not shipped inline)

AGPL-3.0-or-later, like the parent project. Uses dynamically linked GTK3,
WebKitGTK 4.1, GLib/GIO, libsoup3, GdkPixbuf, json-glib and libxml2; packaging must
retain their notices and support security updates. No Rust/GTK ABI is linked
into GPUI. No new mail engine, account/profile access, browser opener or JS bridge.

**Patch phase: do not run make/cc here. Parent owns the one batch build window.**

## Source and protocol

- `main.c`: selectable WebKit document + selectable GtkTextView fallback, plain
  toggle and body zoom. Opaque white backing; no wallpaper/theme reset of sender
  colors. Window visibly says **Original formatting — temporary window**.
- `policy.c`: bounded v1 JSON, canonical raster/CID decoding, parser reconstruction
  into a separate inert HTML document. HTML/CSS source is untrusted; active trees
  and attributes are removed. Original CSS/media queries/table structure survive
  where allowed. This is **not** a complete CSS-token sanitizer or HTML5 parser.
- `request_policy.c`: exact document/resource capability membership and safe
  HTTP(S) link validation. Same function runs in UI scheme handler and extension.
- `extension.c`: pre-dispatch `WebPage::send-request` cancellation, including
  redirects; acknowledgement only after hook installation. Missing/failed
  extension prevents untrusted load; only trusted about:blank bootstraps it.
- `sandbox.c`: re-exec fixed `/usr/bin/bwrap` arguments before GTK/WebKit/input
  parsing. No shell. Private network/user/PID/IPC/UTS namespaces, read-only system
  runtime + helper package, ephemeral home/tmp/run, one display socket. Namespace
  identity is verified via inherited fd 3, then fd 3 is closed before GTK.
  No DBus/Secret Service/home/profile mounts or sandbox-disable workaround.
- `policy_tests.c`, `tests.py`: real compiled policy-seam tests, plus explicitly
  limited source/header audit available before compilation.
- `fixture_request.py`, `engine_probe.py`: synthetic-only v1 producer and a
  post-build engine readiness/heartbeat probe. Neither accesses real mail.

One process, one document, one owning generation. EOF delimits bounded stdin
JSON (20 MiB/10 seconds). Required fields: `version:1`, `html`, `plainText`, `title`.
Optional `resources` is an array of `{cid,mime,base64}`; no paths or fetching.
Each body representation <=2 MiB, title <=512 bytes, 32 resources, compressed
raster <=2 MiB each/8 MiB total; pixels <=8,388,608 each/16,777,216 total,
longest edge <=8192; normalized PNG <=4 MiB each/12 MiB total. PNG/JPEG/WebP only,
subject to installed decoder support. Duplicate normalized CIDs, animation exposed
by the loader, unknown media (including SVG), malformed/corrupt data and over-budget input reject
**the whole original mode**. A decoder that exposes only an animated raster's
first frame may normalize that frame; animation fidelity is not promised.
Native/plain fallback stays available.

`img[src=cid:...]` becomes `megamail-resource://<32-hex token>/part/<index>` after
bounded decode and PNG normalization. Only exact registered URLs are served.
Unresolved/remote/data image src and srcset/picture are removed; alt text remains.
CSS URLs/imports are retained as inert CSS syntax but blocked by CSP, extension
and the network namespace. **CSS CID/data images are not rewritten/supported**.
Direct IMAP's already-expanded data-image bodies therefore need the parent to
supply a MIME-scoped original/CID bundle; blindly forwarding that data URI cannot
be advertised as CID parity. Remote image consent/broker is deliberately absent.

Stdout newline JSON: `{version:1,event,token?,policy?,url?,code?}`.
`ready` has a random token + policy `v1:extension+sandbox+ephemeral+no-network`.
`heartbeat` repeats that token every 2 seconds while the GLib loop and WebKit
responsiveness flag are healthy. `user-click-link` has token + url, emitted only
for `LINK_CLICKED` plus WebKit's verified user gesture. Helper ignores navigation
and never opens the URL. `error` is an opaque code; `closed` ends the process.
Policy/error text does not log HTML, credentials or target URLs. Tokens are
process capabilities, not proof against a compromised executable/OS.

## Parent build window: exact commands

From repository root, **only after all owners freeze**:

```sh
make -j1 -C tools/megamail-html-reader all mutation
PYTHONDONTWRITEBYTECODE=1 python3 tools/megamail-html-reader/tests.py \
  --helper tools/megamail-html-reader/build/megamail-html-reader
```

`make all` executes these system-library builds sequentially (no Cargo):

```sh
cd tools/megamail-html-reader
mkdir -p build/extensions
cc -std=c11 -O2 -Wall -Wextra -Wformat=2 -Werror=implicit-function-declaration \
  -fstack-protector-strong -D_FORTIFY_SOURCE=2 \
  $(pkg-config --cflags gtk+-3.0 webkit2gtk-4.1 json-glib-1.0 libxml-2.0) \
  main.c policy.c request_policy.c sandbox.c policy_tests.c \
  -o build/megamail-html-reader -Wl,-z,relro,-z,now \
  $(pkg-config --libs gtk+-3.0 webkit2gtk-4.1 json-glib-1.0 libxml-2.0)
cc -std=c11 -O2 -Wall -Wextra -Wformat=2 -Werror=implicit-function-declaration \
  -fstack-protector-strong -D_FORTIFY_SOURCE=2 -fPIC -shared \
  $(pkg-config --cflags webkit2gtk-web-extension-4.1 json-glib-1.0 gmodule-2.0) \
  extension.c request_policy.c -o build/extensions/libmegamail-reader-policy.so \
  -Wl,-z,relro,-z,now \
  $(pkg-config --libs webkit2gtk-web-extension-4.1 json-glib-1.0 gmodule-2.0)
```

`make mutation` repeats in **build-no-csp** with `-DMM_TEST_NO_CSP` for synthetic
request-enforcement tests. Its `test-only:no-csp` ready policy is explicitly
rejected by native ReaderHost. Never install it or use real mail in it.

The trusted native executable path is the **absolute** packaged
`build/megamail-html-reader` path (or installed equivalent); its sibling
`extensions/libmegamail-reader-policy.so` is mandatory. The launcher mounts its
own containing directory at `/app` and internally starts
`/app/megamail-html-reader --contained`. It has no external extension path, shell
or general command argument. Install the complete read-only package and fixed
`/usr/bin/bwrap`; don't move the executable without the sibling extension.

## Post-build synthetic checks (not performed in patch phase)

```sh
# Native Wayland: environment provides one existing compositor socket.
GDK_BACKEND=wayland PYTHONDONTWRITEBYTECODE=1 python3 \
  tools/megamail-html-reader/engine_probe.py \
  --helper tools/megamail-html-reader/build/megamail-html-reader --fixture authored

# X11 research only: an explicitly access-control-free temporary synthetic Xvfb.
GDK_BACKEND=x11 PYTHONDONTWRITEBYTECODE=1 xvfb-run -a -s '-screen 0 1024x768x24 -ac' \
  python3 tools/megamail-html-reader/engine_probe.py \
  --helper tools/megamail-html-reader/build/megamail-html-reader --fixture hostile

# CSP mutation: same engine request extension; native refuses this policy.
GDK_BACKEND=wayland PYTHONDONTWRITEBYTECODE=1 python3 \
  tools/megamail-html-reader/engine_probe.py \
  --helper tools/megamail-html-reader/build-no-csp/megamail-html-reader \
  --fixture hostile --test-only-no-csp

# Interactive synthetic selection/copy/plain/zoom/resize; destination not opened.
python3 tools/megamail-html-reader/fixture_request.py --fixture authored | \
  GDK_BACKEND=wayland timeout 60s tools/megamail-html-reader/build/megamail-html-reader
```

The launcher presently accepts local `DISPLAY=:N`, not TCP or `:N.S`, and never
reads/mounts .Xauthority. Ordinary authenticated X11 can consequently fail closed;
a reviewed limited display authorization mechanism is **not implemented**.
Do not weaken host X11 authorization just to make this feature work. Software
WebKit acceleration is always NEVER, including Wayland. Actual WebKit in nested
bubblewrap, extension readiness and request policy still require runtime proof.

Readiness/heartbeat is not a pixels/selection/network test. Parent must trace
**all helper descendants** with an isolated synthetic endpoint/packet or syscall
observer. Require no HTTP/DNS/redirect egress for hostile/parser-confusion fixtures,
including no-CSP mutation. AF_UNIX compositor traffic is a distinct allowed grant.
Compare against clean startup traffic; count/assert attempted requests rather than
mistaking an unreachable host-side loopback server for extension cancellation.
Test missing extension in a separate synthetic package copy: no ready event, no
untrusted load. Test broken loader, wrong namespace, malformed stdout, cancellation,
account switch, crash, hung web process and orphan cleanup. No security-disabling
flags are an accepted fix. All displays/processes started by tests must be stopped.

AT-SPI bus bridging is disabled (`NO_AT_BRIDGE=1`) because granting a general DBus
socket would expose services beyond this reader. Toolkit text selection, Ctrl+C
and focus have actual widgets, but screen-reader access and isolated clipboard
acceptance are **not established**. This is a blocker for an accessibility-ready
release, not an accessibility claim from the presence of GTK.
