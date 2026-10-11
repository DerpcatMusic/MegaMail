# Foundation: faithful, isolated HTML mail reader

Research snapshot: **2026-10-10 UTC**. This is a source investigation and an isolated synthetic engine experiment, not an implementation or a production acceptance claim.

## Decision in brief

**The native reader cannot recover sender formatting from its present payload. Retain the original decoded HTML before `mail_text::reader_html`, and use a real HTML engine for “Original formatting”.** Keep the existing native semantic/plain-text reader as a fast, accessible fallback and an explicit user choice. Extending its small HTML AST is useful for readable text, not a credible shortcut to browser CSS/table/media-query parity.

**First implementation target: a separately packaged GTK3 + WebKitGTK 4.1 reader helper.** This host has the required development API, including the web-process extension API needed for request cancellation. A synthetic offscreen rendering experiment succeeded on X11 and Wayland, with an important software-rendering requirement on this Wayland setup. Build the safe helper and source-retention seam first. An ordinary helper window provides the shortest path to real selectable HTML with toolkit input/accessibility; it is an explicitly labeled temporary “Original formatting” window, **not fulfillment of an inline-reader requirement**.

For inline GPUI, time-box a helper-frame composition/input experiment. A bitmap displayed in GPUI is **not** a completed HTML reader. If WebKit's snapshot/GDK input/accessibility bridge fails the gates below, use **CEF windowless rendering in an isolated helper** for the inline adapter: it exposes explicit paint, resize and input APIs. Do not claim native Wayland embedding from wry's GTK-container example. Do not force XWayland as the normal solution, replace the Zeron shell with GTK, or link the legacy GTK4 application just to get its reader.

### What is established and what is not

- Established by source: exact stripping of CSS/images/layout tables in both native ingestion paths; TextView is a semantic text renderer, not a browser.
- Established by experiment: WebKitGTK 4.1 / engine 2.54.1 rendered the local synthetic table, authored colors and responsive media query in an offscreen GTK3 widget on both display backends.
- **Not established:** interactive GPUI composition, real CID transport through Thunderbird, safe remote-image consent, link hit testing in a composed frame, selection/copy/focus/IME/accessibility in GPUI, zero-network enforcement under hostile HTML, aggregate renderer RSS, or fastest real-provider fetch latency.
- No Cargo command, real mailbox/profile/credential access, provider operation, tracking URL fetch, production source edit, or commit was performed. Other agents' concurrent source changes were not modified. Function names below are more durable than line numbers.

## 1. Repository evidence: where fidelity disappears

Read: [AGENTS](../../AGENTS.md), [PRODUCT](../../PRODUCT.md), [DESIGN](../../DESIGN.md), [architecture](../MEGAMAIL_ARCHITECTURE.md), diagnosis/codebase-design/Rust/find-docs/impeccable guidance. The existing design contract deliberately specifies semantic escaped mail, strips sender styles, and puts the reader on a wallpaper-tinted plane. The new request changes **message content presentation**, not Zeron's navigation/control design. The parent should update that contract when the new mode is proven.

### Direct IMAP path

1. [`src/worker.rs`](../../src/worker.rs): body requests fetch MIME; `render_raw` checks sender/OpenPGP and calls `extract_body` on the raw or unwrapped inner message.
2. `extract_body` uses `mail_parser::MessageParser`. For a single HTML body it returns the decoded document after `inline_cid_images`. This stage **preserves the original stylesheet and table structure** in that common case.
3. `inline_cid_images` replaces resource-position `cid:` references, including `src` and CSS `url(...)`, with data URIs from MIME image parts; normalizes angle brackets/case and percent-encoding; avoids rewriting `href`. There is a **16 MiB decoded inline-image budget**. Unresolved references remain. Data-URI expansion can subsequently exceed the native 2 MiB reader cap. Its string scanning and image MIME acceptance are not a future renderer's full resource-security policy.
4. The worker emits `WorkerEvent::Body { body: String, ... }` and may save that body in the shared cache. `body_cacheable` returns false for **any OpenPGP verdict**, including signed mail; `attachments_of` has corresponding exclusions. Preserve them.
5. [`apps/megamail/src/live.rs`](../../apps/megamail/src/live.rs), worker-event translation / `body_content_and_links`: converts the document into plain text, **calls `mail_text::reader_html(body)`**, builds a quote-free semantic fragment and extracts links. This is the decisive native fidelity-loss stage. The original worker document does not survive as an original-formatting field in the native reader payload.
6. [`main.rs`](../../apps/megamail/src/main.rs), `ReaderBody::new` / `render_reader`: `source_html` is a misleading name here: it holds the already sanitized semantic fragment. `TextView::html` receives that fragment or escaped plain text.

Caveat before making “original” guarantees: `extract_body`'s multi-body branch concatenates body representations. A faithful document bundle must choose the intended MIME alternative and related-resource scope, not concatenate unrelated nested HTML documents or treat an attached message as another primary body. Keep the decoded selected part and its MIME identity separately.

### Thunderbird path

1. [`tools/thunderbird-bridge/extension/background.js`](../../tools/thunderbird-bridge/extension/background.js), `messageBody`: requests `messages.getFull(messageId, { decodeContent: true })`, headers and attachment metadata concurrently.
2. `collectBody` recursively takes the **first** plain and HTML parts. It returns decoded HTML with its head/styles still present. It does **not** return a Content-ID-to-part-name resource map or bytes, so native code has no way to resolve the HTML's CID images merely from this payload.
3. `MAX_BODY_SOURCE_BYTES` is 20 MiB and `MAX_BODY_BYTES` is 2 MiB. A shared UTF-8 byte budget processes **plainText before html** and can truncate the HTML document, with `truncated: true`. This is another fidelity loss **before** the Rust sanitizer for large alternatives. Truncation in the middle of HTML/CSS/UTF-8 is not a faithful document.
4. [`thunderbird_adapter.rs`](../../apps/megamail/src/thunderbird_adapter.rs), `parse_body_payload`: gets `source_html`, calls `mail_text::reader_html(&source_html)`, computes plain/quote-free bodies and links, and stores only those in `BodyPayload`. It does not preserve the original HTML or expose the bridge's truncation state in that payload. This is the decisive loss for normal-sized TB HTML.
5. The event enters live state and `ReaderBody`, then the same GPUI TextView rendering path. Its memory cache cannot reconstruct discarded styles or MIME resources.

Primary TB docs [T1] specify that `getFull` returns the MIME tree, `getAttachmentFile(messageId, partName)` returns attachment bytes, and `getFull`'s `decrypt` defaults to true. CID association must be derived from the **chosen related MIME tree**, not guessed from attachment filename or URL. This source-retention change must also carry encryption/cache eligibility: decrypted TB content must not start entering a new persistent HTML/image/screenshot cache.

### Exact sanitizer/renderer behavior

[`crates/mail-core/src/mail_text.rs`](../../crates/mail-core/src/mail_text.rs):

- `drops_reader_subtree` drops `head`, `style`, `link`, active content, forms and more.
- The `img` branch emits **alt text only**, without retaining even local/data/CID image nodes.
- Only selected semantic tags, direction and safe anchor destinations are re-emitted. Classes, IDs, inline CSS, color/alignment/size attributes and media queries are discarded.
- `is_data_table` identifies presentation tables; those lose table/row structure, while their cells become flowing `div` blocks.
- These are intentional privacy/readability restrictions, not an accidental missing stylesheet loader.

[`vendor/gpui-base/src/text/format/html.rs`](../../vendor/gpui-base/src/text/format/html.rs) parses into `ParsedDocument`, `BlockNode` and `InlineNode`. It drops style/script nodes; limited inline property/image-size parsing is **not** selector matching, cascade, a browser layout model or responsive CSS. [`text_view.rs`](../../vendor/gpui-base/src/text/text_view.rs) has image-resolver and selection hooks, but adding them cannot regenerate content already discarded upstream. Sending unsanitized HTML straight to TextView would also weaken its current resource policy without solving authored-layout fidelity.

### Other screenshot symptoms: distinct causes

- **Huge extra URL list:** `main.rs::links_panel` renders every extracted link as a label plus a raw URL, even though semantic anchors can already be clicked. Collapse this into optional link inspection; show destination/domain on hover or activation and offer copy. Retain the exact original destination internally. Do not auto-follow, resolve shorteners, strip arbitrary query parameters, or visit tracking URLs.
- **Concatenated labels/footer:** stripping table layout, sender spacing and inline styles explains lost grouping. Plain-text conversion is separately lossy. An engine must receive retained HTML, not the flattened text.
- **Clipped date:** native header flex/width behavior, independent of MIME/HTML. Reserve date width; move metadata to a second row at narrow sizes rather than ellipsizing or shrinking it. Long sender/recipient lines wrap.
- **Wallpaper beneath mail:** current shell's reader tint intentionally permits bleed. That cannot be the substrate for authored white/light email. The HTML canvas must be **opaque**; the Zeron chrome can retain its wallpaper treatment outside it.

The supplied screenshot's description is the problem evidence; this agent did not inspect its original pixels or private mail. No compiled Rust regression was run because the parent owns Cargo. Source-path diagnosis and the engine fixture are deliberately not presented as an end-to-end reproduction of that private message.

## 2. Retain a document, not a second flattened string

Add a deep reader-document module with two justified adapters: native semantic/plain view and original-formatting engine view. Its small interface should accept:

- account + folder + UID/session ID **and generation**;
- complete decoded selected HTML, preserving head/style/inline styles, layout tables, responsive rules, IDs/classes and author colors;
- the actual plain MIME alternative when available (generated text otherwise);
- selected MIME-part identity and related-part descriptors: Content-ID, part name, verified media type, size and a scoped byte-provider handle;
- completeness/too-large state and explicit cache eligibility, including decrypted/signed mail policy.

Keep original source immutable and untrusted. Produce a separate sanitized **engine document** that preserves inert layout/CSS. Do not replace the native sanitizer globally or render the retained source unchecked. Do not generate “original HTML” from `ReaderBody.source_html`. Existing direct cache bodies often retain richer source, but cache reads/versioning and encrypted exclusions must be checked; TB's sanitized memory cache needs a refetch.

Use a bounded opaque resource namespace such as `megamail-resource://<nonce>/part/<opaque-id>`. The capability maps to one selected message in one owning account/context; it is not a filesystem path, public server or fetch-any-URL endpoint. Reject traversal, foreign-account IDs, stale generations, duplicate/ambiguous CIDs and unknown types. Inline related resources load on demand, independently of general attachment download; unresolved CID gets an honest placeholder. Prefer byte handles over repeated base64 in IPC/HTML. Explicitly budget compressed bytes, decoded image pixels and total retained body/resource/frame bytes.

Store `Arc<str>` / shared byte buffers once rather than cloning several 2 MiB originals and quote variants. Parsing/rewriting/decoding belongs off GPUI's input/render thread. GTK/WebKit objects stay on their owning main context; do not make them `Send` through an unsafe assertion. Use typed protocol errors, bounded queues, stale-result cancellation and checked frame-size arithmetic. A process crash or unavailable engine must leave selectable fallback text, not a blank pane.

## 3. Engine/platform evidence and comparison

### Host availability, checked individually

| pkg-config module | Observed version |
| --- | --- |
| `webkit2gtk-4.1` | **2.54.1** |
| `webkit2gtk-web-extension-4.1` | **2.54.1** |
| `gtk+-3.0` | **3.24.52** |
| `libsoup-3.0` | **3.8.0** |
| `gtk4` | **4.24.1** |
| `webkitgtk-6.0` | absent |
| `wpe-webkit-2.0`, `wpe-webkit-1.1`, `wpebackend-fdo-1.0` | absent |

Python GI exposes GTK 3.0, WebKit2 4.1 and WebKit2WebExtension 4.1. Xvfb, bwrap and a C compiler are available. No package installation was performed.

The root [Cargo manifest](../../Cargo.toml) requires GTK4 and `webkit6 = 0.4` with `v2_44`: **4.1 cannot satisfy that GTK4/6.0 ABI requirement**. A new GTK3 helper is a separate package, not a root feature that accidentally rebuilds the legacy application.

| Option | Fidelity and GPUI integration | Input/accessibility | Cost, licensing, disposition |
| --- | --- | --- | --- |
| Expand native TextView | Excellent fast readable fallback. Full CSS cascade, nested email tables, media queries and browser layout would be a new rendering engine, not a small patch. | Existing GPUI selection/focus; retain it. | No added browser process/startup. Lowest additional runtime footprint. Existing AGPL app and toolkit notices remain. **Not the authored-mail solution.** |
| WebKitGTK 4.1 direct GTK widget / wry | Mature HTML/CSS. X11 child windows possible. Wayland `build_gtk` requires a GTK container; GPUI is not one. | Native widget gives selection, editing commands, GTK focus/clipboard and ATK. Child/native overlays have z-order and focus problems. | Uses available shared engine; WebKit subprocesses add RSS/startup. GTK LGPL and WebKit LGPL/BSD notices [W9]; wry MIT/Apache-2.0. **Useful X11 experiment; not a cross-backend GPUI integration proof.** |
| WebKitGTK 4.1 separate helper | Independent GTK mainloop; ordinary helper window runs on Wayland and X11. Offscreen `get_snapshot` can supply pixels to GPUI. | Ordinary window retains toolkit behavior. Snapshot transport alone loses selection, link hit-testing, IME and accessibility; input/A11y bridge must be proven. | Adds a helper plus engine processes. Software snapshots copy pixels; no measured memory advantage over CEF. **Recommended first runnable integration and honest temporary fallback.** |
| CEF windowless helper | Explicit `GetViewRect`/`OnPaint`, resize and mouse/key/focus APIs [C1–C3]. CPU frames avoid foreign-window Wayland embedding. | Engine selection/zoom exists, but clipboard, IME, popups and accessibility-tree translation must be connected. OSR A11y exposes changes, not a ready GPUI tree [C4]. | Bundled Chromium binaries/resources, subprocesses, sandbox/update burden. CEF BSD plus Chromium third-party notices; cef-rs Apache-2.0. **Preferred inline contingency when GTK input bridge fails.** No GPUI proof here. |
| Servo | Current source exposes `EventLoopWaker`, `RenderingContext`, `WebViewDelegate::notify_new_frame_ready`, paint/present [S1]. It is not a drop-in GPUI widget. | Embedder must wire input, resource policy and accessibility; arbitrary-mail hardening and CSS compatibility need proof. | MPL-2.0 code with dependency notices; substantial Rust/SpiderMonkey/rendering build. Current book says evolving API, sparse embedding docs and best-effort LTS, not 1.0 [S2]. **Do not choose for the fastest untrusted-mail rollout.** |
| WPE WebKit | Exportable backend exposes SHM/dmabuf callbacks and release/frame-completion [P1], more appropriate than abusing a native Wayland child widget for long-term texture composition. | Custom input/clipboard/A11y integration remains. | Requires absent WPE packages and a new backend integration. WebKit licenses plus backend notices. **Credible later investigation, not the smallest current dependency path.** |

No universal RSS/startup figures are defensible from API docs. Measure parent **and descendants** (including browser/network/GPU processes and sandbox wrappers), ideally PSS as well as RSS; shared-library RSS sums double-count. Record cold spawn, engine-ready, document-ready, first visible paint, warm switch, selection latency, resize, idle CPU, and post-close memory. Legacy `WEB_CONTEXT`'s 512 MB pressure setting is a configured limit, not a measured small footprint. Lazy-create one viewer on first HTML use, reuse it within bounded account policy, avoid one engine per conversation message, and retire idle/crashed viewers.

### Why “just use wry” fails here

Primary tagged wry **0.57.0** source [R1–R3] is decisive:

- `build_as_child` on Linux is **X11 only**. Native Wayland uses `build_gtk(&gtk::Fixed)`, not a raw `wl_surface`.
- GPUI's vendored `Window` implements raw window/display handle traits, but that is insufficient. The locked **gpui-pre-linux 0.3.8** X11 implementation returns **XcbWindowHandle**. wry 0.57.0's `new_x11` matches **Xlib** and returns `UnsupportedWindowHandle` for other variants. A tested, lifetime-safe XID adapter on the same X server, or direct GTK foreign-child integration, is required even on X11. Do not cast an XCB connection pointer into an Xlib Display.
- wry needs GTK initialization and GLib dispatch alongside the host event loop. GPUI does not inherit winit's `about_to_wait` example. Prove integration of the GLib poll/wake cycle without unbounded polling, reentrant GPUI updates or UI stalls. A separate helper avoids this conflict.
- wry 0.57.0 depends on GTK 0.18 / webkit2gtk 2.0.2 / soup3 0.5; core currently uses GLib/GIO 0.20. Objects from those Rust versions are not interchangeable. This is a type/ownership/main-context seam to prove, **not a demonstrated Cargo links conflict**.
- A native X11 child window is outside GPUI's composited scene: GPUI menus, tooltips, rounded clipping, scroll virtualization and wallpaper cannot automatically paint above or clip it. Hide/move/resize the child for overlays and account/window changes; test screen scale and focus.
- Wayland object IDs are connection-scoped. A helper on another connection cannot turn its GTK surface into a subsurface of GPUI by exchanging an ID. Within one connection, GTK's widget hierarchy, surface roles, commits, event dispatch and lifecycle still need a custom integration. `xdg-foreign` establishes transient relationships, **not arbitrary embedded pixels**. Separate helper windows, explicit frame transport, WPE/CEF composition or a genuine backend adapter are distinct solutions [L1].

## 4. Security contract: engine enforcement first

The mail engine must never receive account passwords, OAuth tokens, the profile directory, general filesystem access or an IPC command capable of invoking arbitrary native methods.

### Required WebKit 4.1 controls

Use an account-scoped ephemeral `WebContext`/`WebsiteDataManager` [W3], not the legacy shared persistent context. Enable sandboxing **before the first web process** [W4]; check that it is effective. Never make a sandbox-disable environment flag the workaround for a deployment failure.

- Set engine `enable-javascript = false` and, when supported, `enable-javascript-markup = false`. The synthetic test with JS disabled rejected embedder `run_javascript` with **“Cannot execute JavaScript in this document (699)”**. Do not plan on JS-based resize/selection extraction while claiming engine JS is disabled.
- Disable DOM local storage, HTML5 database, offline application cache and page cache; reject all cookies in the cookie manager. Ephemeral means no durable store, **not no in-memory storage**. Fresh contexts + JS off + no reused worker registrations + `worker-src 'none'` address service workers. Verify those invariants rather than inventing a universal “disable storage” switch.
- Disable plugins/Java, media/WebRTC/capture/WebGL, automatic windows, developer extras, permission grants and downloads. Some obsolete settings are no-ops: this engine warned that hyperlink-auditing's setter is deprecated and does nothing. Drop `ping` attributes and enforce the actual request policy instead.
- Register only the scoped resource scheme [W5]. Only generated document URLs and known resource capabilities are allowed.
- **Install a WebKit web-process extension before load.** `WebKitWebPage::send-request` can cancel a request by returning TRUE and is emitted again for redirects [W1]. Deny HTTP/HTTPS/file/FTP/custom unexpected requests irrespective of HTML location, resource type or consent. A UI `resource-load-started` observer and a navigation callback are **not** a general pre-network cancellation gate.
- All network stays denied inside the renderer, even after image consent. An app-owned broker may fetch only explicitly approved images and deliver verified local bytes. Missing extension/policy readiness must fail closed before displaying HTML. Test parser preloads and every redirect. Add a network-namespace/seccomp or equivalent egress restriction to helper/network subprocesses as another containment layer; allowing a display socket is not permission to expose the home directory or Secret Service.
- Reject `decide-policy` navigation/new windows except the exact application load and intentional safe link handoff. Reject form submission/resubmission, meta refresh, frames and downloads. Refuse automatic link launches; require a verified user click and validate again in GPUI. Deny file/data/javascript/custom scheme links. Support http/https; any mailto support must route through a separately validated native composer.
- Treat a form as inert: engine navigation cancellation + CSP `form-action 'none'`, and remove form controls/action attributes in sanitizer. There is no assumption that a WebKit settings boolean turns all forms off.

CEF's equivalent requires pre-navigation denial, `OnBeforeResourceLoad -> RV_CANCEL`, custom resource handlers, redirect handling, JS-disabled browser settings, cookie/storage isolation, no popups/downloads/permissions and sandboxed subprocesses [C3,C5]. A request callback that misses worker/background traffic is not enough; deny with no frame too, and keep egress blocked.

### CSP and sanitizer: preserve styles without permitting network

Serve the generated document with a policy equivalent to:

```text
default-src 'none';
script-src 'none'; worker-src 'none'; connect-src 'none';
object-src 'none'; frame-src 'none'; child-src 'none';
form-action 'none'; base-uri 'none';
style-src 'unsafe-inline';
img-src megamail-resource:;
font-src 'none'; media-src 'none';
```

This is a starting policy, not proof of scheme/origin behavior on every engine. Prefer a generated response header; if using `load_html`, rebuild a document with a controlled head and early CSP meta. Meta CSP cannot replace all header-only protections; a raw string prefix is not a robust parser-based sanitizer. Renderer navigation denial remains necessary.

Retain inert CSS, `@media`, tables, `class/id`, width/height/cell spacing/alignment and supported author presentation. Remove scripts/event handlers, executable URLs, forms, base/meta refresh, iframe/object/embed, active SVG/foreign content and preload/prefetch/service-worker entry points. Parse CSS tokens, including nested rules and escapes: **`url()`, `@import`, font sources, background/list/cursor/mask/content images, image-set and srcset/picture are resource paths too**. Strip/rewrite every nonlocal reference; retain local referenced CID imagery only through validated capabilities. Initially disallow remote stylesheets/imports/fonts, even with image consent. An image toggle must not secretly allow remote CSS/font tracking. System font fallbacks can differ; arbitrary email/Outlook quirks or blocked external dependencies prevent universal pixel parity.

Do not use ammonia defaults or `css-inline` as the whole fidelity/security solution: the former's style removal defeats the goal, and inlining is not a browser cascade/media-query implementation. Never regex-remove network URLs as the only protection.

### Remote-image consent broker

Default off; native banner says what is blocked and offers “Load images once”. Consent belongs to account/message/generation, not a global HTML boolean. If sender trust is supported, scope it to the account and make revocation real. The broker:

- performs no pre-consent fetch, DNS resolution or tracking-URL probing;
- accepts approved image URLs only; validates scheme/host/IP and **each redirect**, rejects credentials, loopback/private/link-local targets and non-image destinations, and bounds time/bytes/decode pixels;
- sends no account cookies/auth, no referring synthetic origin, and no renderer-provided arbitrary request headers;
- verifies/normalizes supported raster types before making a local capability; avoids active SVG;
- does not fetch anchor URLs, CSS/font imports or invisible beacons merely because the mail contains them;
- documents that consent may disclose the user's network address and still expose image tracking.

Keep sensitive bodies/CID/remote bytes and screenshots out of durable caches for encrypted mail. Don't let WebKit's own cache bypass the application exclusion. Account removal purges capabilities, engine contexts and buffers. Avoid mail/URL/clipboard contents in logs or crash artifacts; production helper core dumps must be disabled or excluded from persistent collection. Read/cache policy should be explicit when TB decryption status is unknown.

## 5. Legacy WebKit reader: retain lessons, not its full policy

[`src/ui/message_view.rs`](../../src/ui/message_view.rs) has real authored rendering:

- `message_frame` embeds each document in a distinct iframe `srcdoc`, preventing sender CSS from styling app headers/chrome.
- Iframes use `sandbox="allow-same-origin allow-popups"` **without `allow-scripts`**. Contrary to the module's initial “JavaScript disabled” summary, `build_webview` actually enables engine JS for a trusted outer wrapper that sizes frames and handles native actions. Do not repeat the stale comment as a global-engine security fact.
- `strip_remote` neutralizes resource URLs and `inject_csp` separately uses the **consent state**, not the resource detector's guess.
- When blocked, CSP limits local data/CID images and inline/data styles/fonts/media. When allowed it broadens img/style/font/media sources to HTTP/HTTPS. This is more permissive than “remote images only”.
- Click/new-window policy opens validated web/mail links externally and ignores WebKit reload. It is not proof that all arbitrary non-click navigation or subresource network is engine-denied.
- Reader-mode conversion in `src/reader.rs` intentionally strips authored design. It must stay a separate choice, not the retained original source.
- Legacy optional dark adaptation, font/color overrides, color-scheme pinning and transparent/scrolling frame treatment also change authored appearance. Do not copy them into “Original formatting” by default.

Reuse CSS isolation, dual blocking logic and external-link validation concepts. Do **not** copy the JS-enabled wrapper, shared context, broad remote consent policy, automatic dark recoloring or transparent canvas into the strict new helper.

## 6. Zeron shell versus authored email canvas

The sender's document is an artifact, not another Zeron-themed control.

- Preserve rail/list hierarchy, hairlines, theme families, wallpaper and compact GPUI controls.
- Inside the message rectangle paint opaque engine pixels. Default unstyled original HTML to a neutral opaque canvas (normally white); retain explicitly authored light/dark backgrounds and foreground colors. No wallpaper, global Geist reset, app-palette inversion or global `max-width` injected into sender content.
- A loading/failure cover should match this canvas rather than reveal wallpaper or flash a contrasting black page.
- Viewport width must equal the available message pane in CSS/logical pixels so authored media queries run. Do not expand a 360px viewport to the email's desktop scrollWidth: that defeats responsiveness. Let genuinely fixed-width mail scroll horizontally within its content rectangle.
- Body zoom uses the engine zoom operation; it must not scale the GPUI sender/date/toolbar. Offer original/native readable/plain-text mode and a visible zoom reset.
- At narrow desktop sizes collapse or resize the rail/list so reading space remains usable; wrap actions/metadata and keep selection/navigation available. Test 1000/1280 logical-pixel windows, narrow reader widths and 100/150/200% display scaling. The engine canvas and native date fix have independent acceptance tests.
- Remove the permanently expanded raw tracking-URL appendix. Footer links belong in the authored footer, with safe hover/copy/activation. The optional inspection view is not a second message body.

## 7. Synthetic experiment: repeatable command and limitations

Fixture: [`authored-newsletter.html`](../fixtures/html-reader/authored-newsletter.html). Fictional text only; no remote resources. Its clickable destinations are reserved `.invalid` addresses and were never opened. CID is replaced **in memory** with a local PNG for the engine test; this is not proof of the TB CID adapter.

Two colored cells give an objective signal: at 680 logical pixels they have the same top edge and disjoint horizontal spans; at 360 they share a left edge and have disjoint vertical spans. CSS removal/table flattening cannot preserve both relationships with these fixture styles.

Observed successful runs, hardware acceleration set to NEVER:

| Backend | Logical width | Snapshot pixels | Left/right colored-cell boxes (x1,y1,x2,y2) | Media-layout assertion |
| --- | --- | --- | --- | --- |
| X11/Xvfb | 680 | 680 × 600 | (41,211,339,429) / (341,211,639,429) | pass |
| X11/Xvfb | 360 | 360 × 600 | (1,186,359,328) / (1,331,359,511) | pass |
| Wayland | 680 | 1360 × 1200 | (81,422,678,859) / (681,422,1278,859) | pass |
| Wayland | 360 | 720 × 1200 | (2,373,719,657) / (2,662,719,1023) | pass |

This Wayland compositor uses 2× backing scale. Geometry/input/texture transport must distinguish logical viewport and physical pixel dimensions. Experimental elapsed times after context construction, including a deliberate 300ms settle and Python pixel scanning, were 744/1309ms for X11 and 1972/3182ms for Wayland, cumulative within each two-document run. **These are not startup/fetch/first-paint benchmarks or backend speed rankings.**

Default Wayland offscreen acceleration first aborted with “GDK is not able to create a GL context: The current backend does not support OpenGL.” The retry used WebKit's public `HardwareAccelerationPolicy.NEVER`, kept sandboxing enabled, and passed. Xvfb emitted DRI3 warnings but produced pixels. No global sandbox or browser-security disable was used.

To repeat without Cargo, from the repo root use `GDK_BACKEND=x11 NO_AT_BRIDGE=1 timeout 18s xvfb-run -a python3 -` or `GDK_BACKEND=wayland NO_AT_BRIDGE=1 timeout 18s python3 -`, with the following stdin. Run `ulimit -c 0` first. The script retains images in memory; it saves no screenshots or private data.

```python
import gi,time,json,os
from pathlib import Path
gi.require_version('Gtk','3.0');gi.require_version('WebKit2','4.1')
from gi.repository import Gtk,WebKit2,GLib
ctx=WebKit2.WebContext.new_ephemeral();ctx.set_sandbox_enabled(True)
html=Path('docs/fixtures/html-reader/authored-newsletter.html').read_text().replace('cid:synthetic-logo@example.invalid','data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jH0sAAAAASUVORK5CYII=')
keep=[];start=time.monotonic();ok=True
def create(width):
 v=WebKit2.WebView.new_with_context(ctx);s=v.get_settings();s.set_enable_javascript(False);s.set_enable_html5_local_storage(False);s.set_enable_html5_database(False)
 s.set_hardware_acceleration_policy(WebKit2.HardwareAccelerationPolicy.NEVER)
 w=Gtk.OffscreenWindow();v.set_size_request(width,600);w.add(v);w.show_all();keep.extend([v,w])
 def loaded(v,e):
  if e==WebKit2.LoadEvent.FINISHED:GLib.timeout_add(300,lambda:(v.get_snapshot(WebKit2.SnapshotRegion.VISIBLE,WebKit2.SnapshotOptions.NONE,None,done,None),False)[1])
 def done(v,r,d):
  global ok
  try:
   surf=v.get_snapshot_finish(r);surf.flush();data=memoryview(surf.get_data());stride=surf.get_stride();boxes={}
   for name,color in [('left',(233,240,237)),('right',(245,241,232))]:
    pts=[(x,y) for y in range(surf.get_height()) for x in range(surf.get_width()) if tuple(data[y*stride+x*4+i] for i in (2,1,0))==color]
    boxes[name]=[min(x for x,y in pts),min(y for x,y in pts),max(x for x,y in pts),max(y for x,y in pts)] if pts else None
   a,b=boxes['left'],boxes['right'];passed=bool(a and b and ((a[1]==b[1] and a[2]<b[0]) if width==680 else (a[0]==b[0] and a[3]<b[1])))
   ok=ok and passed
   print(json.dumps({'backend':os.getenv('GDK_BACKEND'),'width':width,'snapshot':[surf.get_width(),surf.get_height()],'columns':boxes,'media_layout_pass':passed,'elapsed_ms':round((time.monotonic()-start)*1000)}),flush=True)
  except Exception as e:ok=False;print('SNAPSHOT FAILED',type(e).__name__,str(e),flush=True)
  w.destroy()
  if width==680:create(360)
  else:Gtk.main_quit()
 v.connect('load-changed',loaded);v.load_html(html,'about:blank')
create(680);GLib.timeout_add_seconds(12,lambda:(print('TIMEOUT',flush=True),Gtk.main_quit(),False)[2]);Gtk.main()
raise SystemExit(0 if ok else 1)
```

The timer in the exact experiment above ends the GTK loop; a harness should also treat a TIMEOUT log or absent expected records as failure (rather than relying only on the process exit code). All four expected records were observed here. The renderer experiment is red-capable for the fixture's responsive-layout invariants; it does **not** execute Rust's production sanitizer, prove network policy, or reproduce the whole user's reader issue. The parent should add an actual adapter-through-renderer fixture test once the implementation seam exists.

## 8. Smallest proof-of-integration plan, with stop gates

### Gate A — preserve source and an honest fallback

Have the existing owners carry original HTML/resources/completeness/cache policy through the seams below. Keep the current safe native fragment for fallback. Display truncation/too-large errors explicitly. Verify source retention with this synthetic fixture and a synthetic MIME multipart/alternative + related test; no real profile is needed. Preserve existing account/folder/generation guards.

**Stop:** if the engine is receiving semantic HTML rather than original-preserving safe HTML, do not work on screenshots to disguise the problem.

### Gate B — hardened GTK3 helper, independently runnable

Create a separate package/process with no dependency on mail-core's credential/network/profile modules. Initially feed only the synthetic document over an inherited bounded Unix socket. Helper owns GTK/GLib; creates an account-scoped ephemeral WebKit4.1 context; configures sandbox/settings/request extension before any document. Use ordinary GTK window for interactive verification, offscreen widget for frame experiments. Host widgets supply controls; do not add a trusted JS wrapper.

Exercise selection drag, Ctrl+C against a **fixture-only isolated clipboard**, tab traversal, safe link interception without opening the destination, body zoom, resize, and screen-reader access. Verify actual network denial with controlled local test endpoints and syscall/packet observation in isolation: hostile HTML/CSS must cause **zero egress**, including DNS, redirects, forms, pings, SVG references, external styles/fonts and service-worker attempts. Mutation-test with CSP removed so the engine request deny gate must still pass; independently test JS off without relying only on CSP. Fail closed if the web-process extension fails to initialize.

**Stop:** if sandbox or request enforcement cannot be guaranteed, don't load real mail, even in a separate helper window.

### Gate C — one GPUI rectangle, not a fake integration

Parent allocates a logical canvas and sends `Load(document_token, generation)`, `Viewport(width,height,scale)`, `Zoom`, `Focus`, `Input`, `Copy`, `Close`. Responses include `Ready(policy)`, bounded `Frame(generation, pixel_size, stride, format)`, `OpenLink(validated_user_action)`, copy result/accessibility events and typed errors. Authenticate the inherited socket; no HTTP listener or arbitrary JS/native eval command. A capability in a late frame/link/copy response must match the active account/message/generation.

1. Transfer one WebKit snapshot using bounded SHM/memfd (or ordinary bytes for the first proof); GPUI displays the opaque image with current image APIs. Validate stride, byte length, format/alpha, DPR and stale responses.
2. Resize to 680/360 logical pixels; assert the fixture's actual media layout inside GPUI, not just helper screenshots. Menu/tooltip/scroll and wallpaper must compose correctly.
3. Forward real pointer/key/focus/wheel events to the GTK offscreen widget and prove selection/link/clipboard synchronization. WebKitGTK `get_snapshot` [W2] is an **asynchronous snapshot**, not a documented continuous OSR/input interface. GDK synthetic-event routing, damage/snapshot scheduling and layout hit testing are experiments; never advertise them as already solved.
4. Prove accessibility is attached to the visible GPUI content rectangle and focus order, not only an invisible GTK widget. No screen-reader parity claim from a pixel stream.
5. Check crash/hang timeout, cancellation, overlay clipping, teardown, monitor scale changes and long messages. Limit frames to damage/input/load/resize events, not 60Hz idle screenshots. At 1280×800 an RGBA frame is ~4.1MB; 60 full copies/s is ~246MB/s before extra copies/texture uploads. This arithmetic is a transport warning, not measured usage.

**Stop/contingency:** if input/A11y/composition is unreliable, retain native reader + explicit hardened original-formatting helper window. Start a minimal CEF OSR helper using `OnPaint`, `GetViewRect`, `WasResized`, mouse/key/focus APIs and the same document/resource protocol. Require the same security/clipboard/A11y gates before enabling it inline. Do not spend weeks interpreting TextView CSS or silently deliver an unselectable screenshot as the mail viewer.

An X11-only child-widget experiment is optional after the raw-handle adapter is proven. It does not remove the Wayland gate and may need hiding the foreign window under every GPUI overlay. A GTK-hosted rewrite of the shell would solve a different problem and is not authorized.

### Gate D — performance and feature acceptance

“Fastest fetching” must separate selected-body retrieval from HTML presentation and image loading:

- Return cached selected bodies immediately; prioritize selected body requests over refresh/prefetch; keep concurrency bounded per account/runtime. Renderer startup must not serialize the fetch queue.
- Avoid raw MIME + getFull duplication on every open. Return the chosen HTML before downloading unrelated attachments; CID part bytes can arrive independently. TB currently waits for headers/attachment metadata alongside getFull; profile those steps before changing them.
- Parse the retained document once off-thread; cache only permitted sanitized/decoded results with generation/version keys and byte budgets. Reuse a warm viewer, don't create one per message.
- Show safe text while engine starts or images are blocked; switching to original shouldn't discard scroll/selection arbitrarily. User-consented images must never gate first readable content.
- Measure synthetic and eventually user-authorized representative mail: queue wait, network, MIME decode, sanitize, IPC, engine-ready, first visible paint and UI input latency. Report p50/p95, cold/warm context and aggregate memory. No toolkit choice or helper snapshot proves fastest provider performance.

Acceptance must cover table layout, authored style/media queries, CID resources without provider mutation, default-blocked remote image then explicit consent, safe link hover/copy/click, selection/copy, zoom/reset, plain toggle, original opaque canvas, date wrapping, account isolation, encrypted cache exclusion, restart/crash and both X11/native Wayland. Universal Gmail/Outlook rendering parity is not promised; supported fidelity needs its own fixture corpus.

## 9. Concrete ownership and dependencies for the next implementation

Coordinate with the existing owners; this research did not edit their files.

| Owner / production path | Required handoff |
| --- | --- |
| Native main/live/TB adapter owner: `apps/megamail/src/{main.rs,live.rs,thunderbird_adapter.rs}` | Carry original document bundle separately from semantic HTML; completeness/cache eligibility and CID handles; retained-byte accounting; fallback/original/plain controls; collapsed links inspector; date wrapping; opaque canvas; generation-checked helper IPC. |
| Parent/new reader implementation owner: proposed `crates/mail-core/src/reader_document.rs` + `crates/mail-core/src/lib.rs` | Pure bounded MIME/document policy structures and original-preserving HTML/CSS rewrite. Keep `mail_text.rs` native behavior intact. Current OAuth/discovery owners need no changes for HTML viewing. |
| Shared worker owner: `src/worker.rs`; cache owner/parent: `src/cache.rs` | Preserve richer direct source and selected MIME/related scope, resource handles, encrypted/signed eligibility, size limits and priority; version cache only as necessary; preserve GTK consumers of shared worker events. |
| TB bridge owner/parent: `tools/thunderbird-bridge/extension/background.js`; transport `crates/mail-core/src/thunderbird_bridge.rs` | Complete selected HTML; don't spend its entire limit on the plain alternative or ignore truncation. Scoped Content-ID/part-name descriptors and bounded chunked part reads using TB APIs; carry decryption/cache eligibility. Keep session IDs/profile credentials confined to TB actor/runtime. |
| Parent/new helper owner: proposed `apps/html-reader-helper/{Cargo.toml,src/main.rs}`, request-extension package and native renderer adapter `apps/megamail/src/html_reader.rs` | Package-specific GLib/GTK/WebKit ABI, extension readiness handshake, process sandbox/egress restrictions, scoped resources, input/clipboard/A11y/focus/resize, crash handling. No direct profile dependencies. |
| Parent docs/packaging owner | Update PRODUCT/DESIGN/architecture only after proof; explicit mode/limitations, distribution runtime dependencies, license inventory, independent helper install path. |
| Vendor code | **No production vendor change recommended now.** If a backend hook becomes indispensable, assign it separately with upstreamable lifecycle/input/composition tests; do not modify TextView to impersonate a browser. |

**GTK3 helper dependency set:** system `gtk+-3.0`, `webkit2gtk-4.1`, `webkit2gtk-web-extension-4.1`, GLib/GIO/GObject, libsoup3, Cairo/GdkPixbuf/ATK and engine runtime subprocess/sandbox support. Available versions above. For the Rust helper, `gtk 0.18`, `webkit2gtk 2.0.2`, compatible GLib/GIO/Cairo/GDK 0.18-family bindings, serde + bounded Unix IPC; optional `libc/rustix` for SHM/FD transport and audited process policy. Use direct WebKit APIs rather than wry if its wrapper doesn't expose the policy/settings/extension hooks needed. The web-process extension may be a minimal independently built C ABI module using the available extension pkg-config API; do not make it a general plugin loader. Python GI + pycairo + Xvfb is **research/test tooling**, not a proposed production runtime requirement.

**Optional wry experiment:** pin `wry = 0.57.0`, `os-webview/x11`, raw-window-handle 0.6 and explicit XID adaptation; tagged manifest uses GTK0.18/WebKit2GTK2.0.2/soup3 0.5. No GTK4/WebKit6 required for this route. Parent must run Cargo resolution/build; none was attempted here.

**CEF contingency:** `cef`/`cef-dll-sys` from maintained cef-rs [C6], version-matched libcef/resources/locales/ICU/subprocess binaries and a working Linux sandbox; distro libraries and accessibility facilities required by the chosen CEF binary. Pin binary ABI to bindings, inventory Chromium notices, and own timely security updates. Its build tooling can download CEF binaries: audit/pin that supply chain rather than letting a routine production build fetch an unspecified latest engine. No CEF installation/build/runtime proof was performed.

**Sanitizer/resource dependencies:** existing html5ever/RcDom can parse HTML; an audited tokenizing CSS parser such as `cssparser` is needed for resource-aware rewrite, plus strict URL parsing and bounded image decoding. Select and verify current versions in the parent build slot. Do not add remote-fetch behavior to the pure sanitizer.

## 10. Primary references and source pins

Repository links above are the inspected checkout; concurrent owners may move line numbers. External sources were retrieved directly, not inferred from a generic “webview” integration article. Context7's wry query was also checked against the tagged source.

- **[T1]** [Thunderbird messages API](https://webextension-api.thunderbird.net/en/latest/messages.html#getfull-messageid-options): MIME tree, decodeContent/decrypt; [attachment bytes](https://webextension-api.thunderbird.net/en/latest/messages.html#getattachmentfile-messageid-partname).
- **[R1]** [wry 0.57.0 README](https://github.com/tauri-apps/wry/blob/wry-v0.57.0/README.md): GTK/container/event loop and X11-only child embedding.
- **[R2]** [wry 0.57.0 WebKitGTK source](https://github.com/tauri-apps/wry/blob/wry-v0.57.0/src/webkitgtk/mod.rs): `new_x11` Xlib-only match, GTK foreign window, child creation and JS setting.
- **[R3]** [wry 0.57.0 manifest](https://github.com/tauri-apps/wry/blob/wry-v0.57.0/Cargo.toml): exact binding dependencies/features. Latest stable version checked against crates.io metadata: 0.57.0.
- **[W1]** [WebKitWebPage send-request](https://webkitgtk.org/reference/webkit2gtk-web-extension/stable/signal.WebPage.send-request.html): cancellation and redirect callbacks.
- **[W2]** [WebView get_snapshot](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebView.get_snapshot.html), [GtkOffscreenWindow](https://docs.gtk.org/gtk3/class.OffscreenWindow.html): snapshot semantics, not a full OSR UI.
- **[W3]** [ephemeral website data manager](https://webkitgtk.org/reference/webkit2gtk/stable/ctor.WebsiteDataManager.new_ephemeral.html).
- **[W4]** [WebContext set_sandbox_enabled](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebContext.set_sandbox_enabled.html): before web process creation.
- **[W5]** [register_uri_scheme](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebContext.register_uri_scheme.html): custom local byte provider.
- **[W6]** [Settings](https://webkitgtk.org/reference/webkit2gtk/stable/class.Settings.html): JS/storage/media settings. Some hosted stable 4.1 docs identify an older library version; the relevant APIs were cross-checked against installed **2.54.1 headers/GI/runtime**, including deprecated no-op behavior. Do not equate a boolean's existence with effective protection.
- **[W9]** [WebKit licensing](https://webkit.org/licensing/).
- **[L1]** [Wayland core protocol](https://gitlab.freedesktop.org/wayland/wayland/-/blob/main/protocol/wayland.xml), particularly surface roles and `wl_subcompositor.get_subsurface`. GPUI's locked Linux source is `gpui-pre-linux 0.3.8/src/linux/{x11,wayland}/window.rs`, inspected in the local Cargo registry (no build).
- **[C1]** [CEF RenderHandler](https://github.com/chromiumembedded/cef/blob/c5b06184bdf407fabf18bf93dee48af3461e1b38/include/cef_render_handler.h): view rect/paint.
- **[C2]** [CEF Linux window info](https://github.com/chromiumembedded/cef/blob/c5b06184bdf407fabf18bf93dee48af3461e1b38/include/internal/cef_types_linux.h): windowless mode and shared-texture platform limitation.
- **[C3]** [CEF BrowserHost](https://github.com/chromiumembedded/cef/blob/c5b06184bdf407fabf18bf93dee48af3461e1b38/include/cef_browser.h): zoom/resize/input/accessibility.
- **[C4]** [CEF accessibility callbacks](https://github.com/chromiumembedded/cef/blob/c5b06184bdf407fabf18bf93dee48af3461e1b38/include/cef_accessibility_handler.h).
- **[C5]** [CEF request handler](https://github.com/chromiumembedded/cef/blob/c5b06184bdf407fabf18bf93dee48af3461e1b38/include/cef_request_handler.h), [resource-request cancellation](https://github.com/chromiumembedded/cef/blob/c5b06184bdf407fabf18bf93dee48af3461e1b38/include/cef_resource_request_handler.h).
- **[C6]** [cef-rs maintained bindings/build requirements](https://github.com/tauri-apps/cef-rs): supported Linux architectures and binary download/ABI packaging.
- **[S1]** [Servo embedding source](https://github.com/servo/servo/blob/3dcc4e02bc13ff51ff08938c6e682d94a3825f37/components/servo/lib.rs).
- **[S2]** [Servo embedding overview](https://github.com/servo/book/blob/f8863401a0e211586e05ed20f329eef90c9ad1d2/src/embedding/overview.md), [best-effort LTS and production caveats](https://github.com/servo/book/blob/f8863401a0e211586e05ed20f329eef90c9ad1d2/src/embedding/lts-release.md). HTML book fetch was blocked (403); the primary Markdown sources were retrieved instead.
- **[P1]** [WPEBackend-fdo exportable backend API](https://github.com/Igalia/WPEBackend-fdo/blob/master/include/wpe/view-backend-exportable.h): SHM/dmabuf export and release lifecycle. WPE integration was not attempted; two guessed reference URLs returned 404, so this comparison relies on the actual primary header rather than those URLs.

**Bottom line:** source retention is mandatory and belongs to the existing ingestion owners. WebKit4.1 is available and can render this fixture on both backends, including native Wayland with software snapshots. Interactive, secure inline GPUI integration remains a gated engineering task; the available engine does not justify a full-parity claim.
