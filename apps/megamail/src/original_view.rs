//! Inline "Original" reader: the sender's HTML and CSS laid out by Blitz
//! (Stylo + Taffy + Parley) and painted on the CPU into an image, off the UI
//! thread. No script engine exists in this path; every sub-resource goes
//! through [`Provider`], which serves `data:` URIs (inline `cid:` images are
//! already embedded that way by the worker) and refuses the network unless
//! the user asked to load remote images for this message.
//!
//! In a dark theme, mail that declares no dark styles of its own is recoloured
//! the way Gmail does it: light backgrounds are pushed dark, and text that
//! would lose contrast is pushed light, in CIELAB so hues survive.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

use anyrender::ImageRenderer as _;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use blitz_traits::shell::{ColorScheme, Viewport};
use futures::channel::oneshot;

/// Largest image we will hand to the GPU, in device pixels.
const MAX_DEVICE_HEIGHT: f32 = 16_000.;
const MAX_DEVICE_BYTES: usize = 160 * 1024 * 1024;
const REMOTE_IMAGE_MAX: u64 = 6 * 1024 * 1024;
const REMOTE_REQUEST_MAX: usize = 60;
const DOCS_KEPT: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Theme {
    pub dark: bool,
    pub background: [u8; 3],
    pub text: [u8; 3],
    pub link: [u8; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Params {
    pub width: f32,
    pub scale: f32,
    pub theme: Theme,
    /// Keep the sender's colours even in a dark theme.
    pub original_colors: bool,
    pub allow_remote: bool,
    /// Fold the quoted reply history (shown again with "Show quoted history").
    pub hide_history: bool,
}

pub(crate) struct Rendered {
    pub image: Arc<gpui_kit::RenderImage>,
    pub width: f32,
    pub height: f32,
    /// Remote resources the provider refused; offer "Load images" when > 0.
    pub blocked_remote: usize,
    pub recolored: bool,
    /// Displayed size / layout size: below 1 when a fixed-width design was
    /// shrunk to fit the pane, as phone mail apps do.
    pub fit: f32,
    /// Resolves when web resources requested by this render are cached;
    /// render again then to show them.
    pub remote_done: Option<oneshot::Receiver<()>>,
}

enum Job {
    Render {
        key: u64,
        html: Arc<str>,
        params: Params,
        reply: oneshot::Sender<Result<Rendered, String>>,
    },
    Hit {
        key: u64,
        x: f32,
        y: f32,
        reply: oneshot::Sender<Option<String>>,
    },
}

fn worker() -> &'static Mutex<mpsc::Sender<Job>> {
    static TX: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("megamail-original".into())
            .spawn(move || run(rx))
            .expect("failed to spawn the original-view renderer");
        Mutex::new(tx)
    })
}

/// Lay out and paint `html` at `params`. `key` identifies the document so a
/// later [`link_at`] can hit-test the same layout.
pub(crate) fn render(
    key: u64,
    html: Arc<str>,
    params: Params,
) -> oneshot::Receiver<Result<Rendered, String>> {
    let (reply, rx) = oneshot::channel();
    let _ = worker()
        .lock()
        .map(|tx| tx.send(Job::Render { key, html, params, reply }));
    rx
}

/// The link under a point of a rendered document, in CSS pixels.
pub(crate) fn link_at(key: u64, x: f32, y: f32) -> oneshot::Receiver<Option<String>> {
    let (reply, rx) = oneshot::channel();
    let _ = worker().lock().map(|tx| tx.send(Job::Hit { key, x, y, reply }));
    rx
}

fn run(rx: mpsc::Receiver<Job>) {
    // (key, document, fit): `fit` maps displayed pixels back to layout pixels.
    let mut docs: VecDeque<(u64, HtmlDocument, f32)> = VecDeque::new();
    while let Ok(job) = rx.recv() {
        match job {
            Job::Render { key, html, params, reply } => {
                // Coalesce: a newer render for the same message supersedes this one.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    render_now(&html, &params)
                }))
                .unwrap_or_else(|_| Err("The original layout could not be drawn.".into()));
                match result {
                    Ok((doc, rendered)) => {
                        docs.retain(|(k, ..)| *k != key);
                        let fit = rendered.fit;
                        docs.push_front((key, doc, fit));
                        docs.truncate(DOCS_KEPT);
                        let _ = reply.send(Ok(rendered));
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Job::Hit { key, x, y, reply } => {
                let href = docs
                    .iter()
                    .find(|(k, ..)| *k == key)
                    .and_then(|(_, doc, fit)| link_in(doc, x / fit, y / fit));
                let _ = reply.send(href);
            }
        }
    }
}

fn link_in(doc: &HtmlDocument, x: f32, y: f32) -> Option<String> {
    let mut node = doc.get_node(doc.hit(x, y)?.node_id);
    while let Some(current) = node {
        if let Some(element) = current.element_data() {
            if element.name.local.as_ref() == "a" {
                if let Some(href) = element.attr(blitz_dom::local_name!("href")) {
                    return Some(href.trim().to_owned());
                }
            }
        }
        node = current.parent.and_then(|id| doc.get_node(id));
    }
    None
}

thread_local! {
    static FONTS: blitz_dom::FontContext = {
        use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
        let mut fonts = blitz_dom::FontContext {
            source_cache: SourceCache::new_shared(),
            collection: Collection::new(CollectionOptions { shared: false, system_fonts: true }),
        };
        fonts
            .collection
            .register_fonts(Blob::new(Arc::new(blitz_dom::BULLET_FONT) as _), None);
        alias_windows_fonts(&mut fonts);
        fonts
    };
}

/// Mail asks for Arial, Helvetica, Tahoma… by name. Linux rarely has them, and
/// an unknown family drops non-Latin text (Hebrew, Arabic) to whatever script
/// fallback fontconfig picks — often a monospace face, which looked like
/// letter-spaced text. Point those names at the metric-compatible free fonts.
fn alias_windows_fonts(fonts: &mut blitz_dom::FontContext) {
    use parley::fontique::FontInfoOverride;
    const SANS: &[&str] = &["Liberation Sans", "Arimo", "Noto Sans", "DejaVu Sans"];
    const SERIF: &[&str] = &["Liberation Serif", "Tinos", "Noto Serif", "DejaVu Serif"];
    const MONO: &[&str] = &["Liberation Mono", "Cousine", "Noto Sans Mono", "DejaVu Sans Mono"];
    const ALIASES: &[(&str, &[&str])] = &[
        ("Arial", SANS),
        ("Helvetica", SANS),
        ("Helvetica Neue", SANS),
        ("Tahoma", SANS),
        ("Verdana", SANS),
        ("Segoe UI", SANS),
        ("Trebuchet MS", SANS),
        ("Geneva", SANS),
        ("Lucida Grande", SANS),
        ("Times New Roman", SERIF),
        ("Times", SERIF),
        ("Georgia", SERIF),
        ("Courier New", MONO),
        ("Courier", MONO),
    ];
    for (alias, candidates) in ALIASES {
        if fonts.collection.family_id(alias).is_some() {
            continue;
        }
        let Some(family) = candidates.iter().find_map(|name| fonts.collection.family_by_name(name)) else {
            continue;
        };
        let mut seen = std::collections::HashSet::new();
        for font in family.fonts() {
            if !seen.insert(font.source().id()) {
                continue;
            }
            if let Some(blob) = font.load(Some(&mut fonts.source_cache)) {
                fonts.collection.register_fonts(
                    blob,
                    Some(FontInfoOverride { family_name: Some(alias), ..Default::default() }),
                );
            }
        }
    }
}

fn render_now(html: &str, params: &Params) -> Result<(HtmlDocument, Rendered), String> {
    let scale = params.scale.clamp(1., 3.);
    let width = params.width.clamp(240., 1600.);
    let declares_dark = declares_dark_styles(html);
    let recolor = params.theme.dark && !params.original_colors && !declares_dark;
    let html: std::borrow::Cow<str> = if params.hide_history {
        megamail_core::mail_text::raw_html_without_history(html)
            .map(std::borrow::Cow::Owned)
            .unwrap_or(std::borrow::Cow::Borrowed(html))
    } else {
        std::borrow::Cow::Borrowed(html)
    };
    let mut source = if recolor { recolor_for_dark(&html) } else { html.clone().into_owned() };
    // Blitz switches to its XML parser for an XHTML doctype (most marketing
    // mail declares one). Mail is never real XHTML: in XML mode <head>, its
    // <style> and <title> show up as page text and the styles are lost.
    // A leading newline keeps the HTML5 parser, which ignores it.
    if source.starts_with("<!DOCTYPE") || source.starts_with("<?xml") {
        source.insert(0, '\n');
    }
    let provider = Arc::new(Provider::new(params.allow_remote));
    let source = defer_stylesheets(&source, &provider);
    let source = tame_div_tables(&source);
    let source = tame_inline_tags(&source);
    let scheme = if params.theme.dark && !params.original_colors {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    };
    let mut doc = HtmlDocument::from_html(
        &source,
        DocumentConfig {
            viewport: Some(Viewport::new(
                (width * scale) as u32,
                (800. * scale) as u32,
                scale,
                scheme,
            )),
            net_provider: Some(provider.clone()),
            // One warmed font collection for every render: scanning the
            // system fonts per message cost more than the layout itself.
            font_ctx: Some(FONTS.with(|fonts| fonts.clone())),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(&base_css(params, recolor || declares_dark));
    doc.handle_messages();
    doc.resolve(0.);
    // CSS background images are requested while styles resolve, after the
    // first message pass; apply their (cached) bytes or they never paint.
    doc.handle_messages();
    doc.resolve(0.);

    // Anything that lays out wider than the pane (fixed-width designs, and
    // table cells whose padding overflows a percentage width) is shown whole,
    // shrunk to fit, instead of being clipped on the right.
    let content_width = right_extent(&doc, doc.root_element().id, 0., 0);
    let fit = if content_width > width + 2. {
        (width / content_width).max(0.4)
    } else {
        1.
    };
    let height = doc.root_element().final_layout.size.height.max(1.) * fit;
    let mut device_scale = scale;
    while height * device_scale > MAX_DEVICE_HEIGHT && device_scale > 1. {
        device_scale = (device_scale - 0.5).max(1.);
    }
    let device_width = (width * device_scale).ceil() as u32;
    let device_height = (height * device_scale).min(MAX_DEVICE_HEIGHT).ceil() as u32;
    if device_width as usize * device_height as usize * 4 > MAX_DEVICE_BYTES {
        return Err("This message is too large to draw in its original layout.".into());
    }
    let mut buffer = Vec::with_capacity(device_width as usize * device_height as usize * 4);
    let mut renderer = VelloCpuImageRenderer::new(device_width, device_height);
    renderer.render_to_vec(
        |scene| {
            blitz_paint::paint_scene(
                scene,
                &mut doc,
                (device_scale * fit) as f64,
                device_width,
                device_height,
                0,
                0,
            )
        },
        &mut buffer,
    );
    // GPUI images are BGRA.
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(device_width, device_height, buffer)
        .ok_or("The original layout produced an invalid image.")?;
    let rendered = Rendered {
        image: Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(image)])),
        width,
        height: device_height as f32 / device_scale,
        blocked_remote: provider.blocked.load(Ordering::Relaxed),
        recolored: recolor,
        fit,
        remote_done: provider.finish(),
    };
    Ok((doc, rendered))
}

/// Rightmost painted edge of the subtree, in layout pixels.
fn right_extent(doc: &HtmlDocument, id: usize, x: f32, depth: usize) -> f32 {
    let Some(node) = doc.get_node(id) else {
        return 0.;
    };
    let layout = &node.final_layout;
    let left = x + layout.location.x;
    let mut right = left + layout.size.width;
    if depth < 64 {
        for &child in &node.children {
            right = right.max(right_extent(doc, child, left, depth + 1));
        }
    }
    right
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Lowest-priority defaults: the theme's canvas and text for mail that sets
/// none, and a readable measure. Author CSS still wins everywhere it speaks.
fn base_css(params: &Params, themed: bool) -> String {
    let (bg, text, link) = if themed || !params.theme.dark {
        (hex(params.theme.background), hex(params.theme.text), hex(params.theme.link))
    } else {
        // Original colours in a dark theme: the sender's paper, not ours.
        ("#ffffff".into(), "#1f1f23".into(), "#2b59c3".into())
    };
    let table_css: &str = &TABLE_CSS;
    format!(
        "html, body {{ background-color: {bg}; color: {text}; }}\n\
         body {{ margin: 0; padding: 0; font-family: system-ui, sans-serif; font-size: 14px; line-height: 1.5; overflow-wrap: anywhere; }}\n\
         a {{ color: {link}; }}\n\
         img {{ max-width: 100%; height: auto; }}\n\
         td, th {{ box-sizing: border-box; }}\n\
         pre {{ white-space: pre-wrap; }}\n\
         blockquote {{ margin: 0 0 0 0.8em; padding-left: 0.8em; border-left: 2px solid {link}55; }}\n\
         {table_css}"
    )
}

/// Blitz draws a stray rule around every `border-collapse: collapse` table
/// (nearly every email sets it) and ignores `cellspacing`/`cellpadding`.
/// Separate borders with the attribute values mapped in look the same as
/// in Gmail for layout tables, without the boxes.
static TABLE_CSS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let mut css = String::from("* { border-collapse: separate !important; }\ntable { border-spacing: 0; }\n");
    for n in 0..=24 {
        css.push_str(&format!("table[cellspacing=\"{n}\"] {{ border-spacing: {n}px; }}\n"));
        css.push_str(&format!(
            "table[cellpadding=\"{n}\"] > * > tr > td, table[cellpadding=\"{n}\"] > * > tr > th, table[cellpadding=\"{n}\"] > tr > td {{ padding: {n}px; }}\n"
        ));
    }
    css
});

/// Mail that brings its own dark styles is shown with them, unaltered.
fn declares_dark_styles(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    lower.contains("prefers-color-scheme") && lower.contains("dark")
}

// ---------------------------------------------------------------------------
// Resources

struct Provider {
    allow_remote: bool,
    blocked: AtomicUsize,
    requested: AtomicUsize,
    pending: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl Provider {
    fn new(allow_remote: bool) -> Self {
        Self {
            allow_remote,
            blocked: AtomicUsize::new(0),
            requested: AtomicUsize::new(0),
            pending: Mutex::new(Vec::new()),
        }
    }

    /// Never waits: hands back a signal for when the fetches started by
    /// this render have landed in the cache.
    fn finish(&self) -> Option<oneshot::Receiver<()>> {
        let handles = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()));
        if handles.is_empty() {
            return None;
        }
        let (tx, rx) = oneshot::channel();
        std::thread::spawn(move || {
            for handle in handles {
                let _ = handle.join();
            }
            let _ = tx.send(());
        });
        Some(rx)
    }
}

/// Web resources already fetched, shared by every render (and kept across
/// messages), so re-layout and newsletters from the same sender are instant.
/// Failed fetches are kept as empty entries so they are not retried forever.
struct RemoteCache {
    entries: std::collections::HashMap<String, Bytes>,
    order: VecDeque<String>,
    bytes: usize,
}

const REMOTE_CACHE_MAX: usize = 128 * 1024 * 1024;

fn remote_cache() -> &'static Mutex<RemoteCache> {
    static CACHE: OnceLock<Mutex<RemoteCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(RemoteCache { entries: Default::default(), order: VecDeque::new(), bytes: 0 })
    })
}

fn cached(url: &str) -> Option<Bytes> {
    remote_cache().lock().ok()?.entries.get(url).cloned()
}

fn store(url: String, bytes: Bytes) {
    let Ok(mut cache) = remote_cache().lock() else {
        return;
    };
    cache.bytes += bytes.len();
    if let Some(old) = cache.entries.insert(url.clone(), bytes) {
        cache.bytes -= old.len();
    } else {
        cache.order.push_back(url);
    }
    while cache.bytes > REMOTE_CACHE_MAX {
        let Some(oldest) = cache.order.pop_front() else { break };
        if let Some(old) = cache.entries.remove(&oldest) {
            cache.bytes -= old.len();
        }
    }
}

impl NetProvider for Provider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url;
        match url.scheme() {
            "data" => {
                if let Some(bytes) = decode_data_url(url.as_str()) {
                    handler.bytes(url.to_string(), Bytes::from(bytes));
                }
            }
            "http" | "https" if self.allow_remote => {
                if let Some(bytes) = cached(url.as_str()) {
                    // A cached failure (empty) stays unanswered, like a block.
                    if !bytes.is_empty() {
                        handler.bytes(url.to_string(), bytes);
                    }
                    return;
                }
                // Not answered: an unanswered image just isn't drawn (an empty
                // answer would draw a broken-image box). Stylesheets that are
                // not cached yet were taken out before parsing (see
                // `defer_stylesheets`), so nothing holds up the first paint.
                self.prefetch(url.to_string());
            }
            "http" | "https" => {
                self.blocked.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

impl Provider {
    /// Fetch into the shared cache in the background; the next render uses it.
    fn prefetch(&self, url: String) {
                if self.requested.fetch_add(1, Ordering::Relaxed) >= REMOTE_REQUEST_MAX {
                    return;
                }
                let handle = std::thread::spawn(move || {
                    let agent = ureq::AgentBuilder::new()
                        .timeout(Duration::from_secs(8))
                        .redirects(3)
                        .build();
                    let mut bytes = Vec::new();
                    if let Ok(response) = agent.get(url.as_str()).call() {
                        use std::io::Read as _;
                        if response.into_reader().take(REMOTE_IMAGE_MAX).read_to_end(&mut bytes).is_err() {
                            bytes.clear();
                        }
                    }
                    store(url.to_string(), Bytes::from(bytes));
                });
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(handle);
    }
}

/// MJML/Loops-style mail turns column `<div>`s into `display: table` /
/// `table-cell` from a stylesheet; Blitz lays those anonymous tables out with
/// the first column squeezed to its minimum. In rules that don't target real
/// table elements, fall back to the inline-block columns the mail already
/// declares inline (what narrow clients see), which Blitz handles well.
fn tame_div_tables(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    let mut from = 0;
    while let Some(found) = lower[from..].find("<style") {
        let open = from + found;
        let Some(body) = lower[open..].find('>').map(|e| open + e + 1) else { break };
        let Some(close) = lower[body..].find("</style").map(|e| body + e) else { break };
        from = close;
        out.push_str(&html[last..body]);
        out.push_str(&tame_css(&html[body..close]));
        last = close;
    }
    out.push_str(&html[last..]);
    out
}

/// Inline-style patterns Blitz lays out wrong, rewritten per tag:
/// - `<td style="display:block">` (Meta/Instagram buttons) drops the whole
///   cell, text included; a plain cell renders what clients show.
/// - A small fixed-size `<img>` beside a `width:100%` cell gets squeezed to
///   zero width; `min-width` keeps its column.
/// - `<img width=24 height=24 style="display:block">` ignores the size
///   attributes; copying them into the style keeps the box.
fn tame_inline_tags(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len() + html.len() / 64);
    let mut last = 0;
    let mut i = 0;
    while let Some(found) = lower[i..].find('<') {
        let start = i + found;
        let name_end = lower[start + 1..]
            .find(|c: char| !c.is_ascii_alphanumeric())
            .map_or(lower.len(), |e| start + 1 + e);
        let name = &lower[start + 1..name_end];
        // Skip raw-text bodies, so CSS or script text is never treated as tags.
        if name == "style" || name == "script" {
            i = lower[name_end..].find(&format!("</{name}")).map_or(lower.len(), |e| name_end + e + 1);
            continue;
        }
        let Some(end) = tag_end(&html[start..]).map(|len| start + len) else { break };
        i = end;
        if !matches!(name, "img" | "td" | "th" | "div" | "a" | "span" | "table") {
            continue;
        }
        let tag = &html[start..end];
        if let Some(fixed) = tame_tag(name, tag) {
            out.push_str(&html[last..start]);
            out.push_str(&fixed);
            last = end;
        }
    }
    out.push_str(&html[last..]);
    out
}

/// The value of `name="…"` (or `'…'`, or bare) in one tag, with its byte span.
fn attr(tag: &str, name: &str) -> Option<(usize, usize, String)> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        let before = lower[..at].chars().next_back();
        if !before.is_some_and(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let rest = lower[from..].trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let value_at = lower.len() - rest[1..].trim_start().len();
        let bytes = tag.as_bytes();
        let (open, close) = match bytes.get(value_at) {
            Some(q @ (b'"' | b'\'')) => {
                let close = tag[value_at + 1..].find(*q as char)? + value_at + 1;
                (value_at + 1, close)
            }
            _ => {
                let close = tag[value_at..]
                    .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                    .map_or(tag.len(), |e| value_at + e);
                (value_at, close)
            }
        };
        return Some((open, close, tag[open..close].to_owned()));
    }
    None
}

/// A CSS length in px from a declaration list (`width:90px` → 90).
fn style_px(style: &str, property: &str) -> Option<f32> {
    style.split(';').find_map(|decl| {
        let (key, value) = decl.split_once(':')?;
        (key.trim().eq_ignore_ascii_case(property))
            .then(|| value.trim().trim_end_matches("!important").trim().strip_suffix("px")?.trim().parse().ok())
            .flatten()
    })
}

fn has_property(style: &str, property: &str) -> bool {
    style.split(';').any(|decl| decl.split_once(':').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(property)))
}

fn is_display_block(style: &str) -> bool {
    style.split(';').any(|decl| {
        decl.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("display") && v.trim().trim_end_matches("!important").trim().eq_ignore_ascii_case("block")
        })
    })
}

fn tame_tag(name: &str, tag: &str) -> Option<String> {
    if name == "img" {
        // `width="undefined" height="NaN"` (template bugs) makes Blitz draw the
        // image at zero size; clients ignore a size they can't parse.
        let cleaned = strip_bogus_sizes(tag);
        if cleaned != tag {
            return Some(tame_style(name, &cleaned).unwrap_or(cleaned));
        }
    }
    tame_style(name, tag)
}

fn strip_bogus_sizes(tag: &str) -> String {
    let mut tag = tag.to_owned();
    for name in ["width", "height"] {
        let Some((open, close, value)) = attr(&tag, name) else { continue };
        let v = value.trim().trim_end_matches("px").trim_end_matches('%');
        if v.parse::<f32>().is_ok_and(|n| n.is_finite() && n >= 0.) {
            continue;
        }
        let Some(start) = tag.to_ascii_lowercase()[..open].rfind(name) else { continue };
        let quoted = tag.as_bytes().get(close).is_some_and(|b| matches!(b, b'"' | b'\''));
        let end = if quoted { close + 1 } else { close };
        tag.replace_range(start..end, "");
    }
    tag
}

fn tame_style(name: &str, tag: &str) -> Option<String> {
    let style = attr(tag, "style");
    let current = style.as_ref().map_or("", |(_, _, s)| s.as_str());
    let new_style = if !matches!(name, "img" | "td" | "th") {
        // `background:url(…) center / cover` (Instagram thumbnails): Blitz
        // drops the size from the shorthand, so spell out the longhands.
        let lower = current.to_ascii_lowercase();
        let size = ["cover", "contain"].into_iter().find(|s| lower.contains(&format!("/ {s}")) || lower.contains(&format!("/{s}")));
        let (Some(size), false) = (size.filter(|_| lower.contains("url(")), has_property(current, "background-size")) else {
            return None;
        };
        let sep = if current.trim_end().ends_with(';') { "" } else { ";" };
        format!("{current}{sep}background-size:{size};background-position:center;background-repeat:no-repeat")
    } else if name == "img" {
        let attr_px = |n: &str| attr(tag, n).and_then(|(_, _, v)| v.trim().trim_end_matches("px").parse::<f32>().ok());
        let width = style_px(current, "width").or_else(|| attr_px("width"));
        let height = style_px(current, "height").or_else(|| attr_px("height"));
        let mut add = String::new();
        if is_display_block(current) {
            if let (false, Some(w)) = (has_property(current, "width"), width) {
                add.push_str(&format!("width:{w}px;"));
            }
            if let (false, Some(h)) = (has_property(current, "height"), height) {
                add.push_str(&format!("height:{h}px;"));
            }
        }
        // ponytail: only small images (avatars, icons) get a hard minimum, so
        // a responsive hero image can still shrink below its declared width.
        if let Some(w) = width.filter(|w| *w > 0. && *w <= 160.)
            && !has_property(current, "min-width")
        {
            add.push_str(&format!("min-width:{w}px;"));
        }
        if add.is_empty() {
            return None;
        }
        let sep = if current.trim().is_empty() || current.trim_end().ends_with(';') { "" } else { ";" };
        format!("{current}{sep}{add}")
    } else {
        if !is_display_block(current) {
            return None;
        }
        current
            .split(';')
            .filter(|decl| !decl.split_once(':').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("display")))
            .collect::<Vec<_>>()
            .join(";")
    };
    let escaped = new_style.replace('"', "&quot;");
    Some(match style {
        // Rewrite the value in place, always as a double-quoted attribute.
        Some((open, close, _)) => {
            let quoted = tag.as_bytes().get(open.wrapping_sub(1)).is_some_and(|b| matches!(b, b'"' | b'\''));
            let (from, to) = if quoted { (open - 1, close + 1) } else { (open, close) };
            format!("{}\"{escaped}\"{}", &tag[..from], &tag[to..])
        }
        None => {
            let insert = tag.trim_end_matches('>').trim_end_matches('/').len();
            format!("{} style=\"{escaped}\"{}", &tag[..insert], &tag[insert..])
        }
    })
}

fn tame_css(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut selector_start = 0;
    let mut i = 0;
    let bytes = css.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                let Some(end) = css[i + 1..].find(['{', '}']).map(|e| i + 1 + e) else { break };
                if bytes[end] == b'{' {
                    // An @media/@supports block: descend.
                    out.push_str(&css[selector_start..=i]);
                    selector_start = i + 1;
                    i += 1;
                    continue;
                }
                let selector = css[selector_start..i].to_ascii_lowercase();
                let real_table = selector
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.' && c != '#')
                    .any(|token| matches!(token, "table" | "td" | "th" | "tr" | "tbody"));
                out.push_str(&css[selector_start..=i]);
                let decls = &css[i + 1..end];
                if real_table {
                    out.push_str(decls);
                } else {
                    let mut tamed = String::with_capacity(decls.len());
                    for decl in decls.split_inclusive(';') {
                        let compact: String = decl.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
                        if compact.starts_with("display:table-cell") {
                            tamed.push_str("display:inline-block !important;");
                        } else if compact.starts_with("display:table") && !compact.starts_with("display:table-") {
                            tamed.push_str("display:block !important;");
                        } else if compact.starts_with("table-layout:") {
                            // dropped with the table display
                        } else {
                            tamed.push_str(decl);
                        }
                    }
                    out.push_str(&tamed);
                }
                out.push('}');
                i = end + 1;
                selector_start = i;
            }
            b'}' => {
                out.push_str(&css[selector_start..=i]);
                i += 1;
                selector_start = i;
            }
            _ => i += 1,
        }
    }
    out.push_str(&css[selector_start.min(css.len())..]);
    out
}

/// Remote `<link rel=stylesheet>` holds Blitz's first paint until it loads.
/// Keep only those already cached; fetch the rest (when allowed) for the
/// redraw that follows.
fn defer_stylesheets(html: &str, provider: &Provider) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    let mut from = 0;
    while let Some(found) = lower[from..].find("<link") {
        let start = from + found;
        let Some(end) = lower[start..].find('>').map(|e| start + e + 1) else { break };
        from = end;
        let tag = &lower[start..end];
        if !tag.contains("stylesheet") {
            continue;
        }
        let href = tag.find("href=").and_then(|i| {
            let rest = &html[start + i + 5..end];
            let quote = rest.chars().next()?;
            if quote == '"' || quote == '\'' {
                rest[1..].find(quote).map(|j| rest[1..1 + j].replace("&amp;", "&"))
            } else {
                Some(rest.split(|c: char| c.is_whitespace() || c == '>').next()?.to_owned())
            }
        });
        let Some(href) = href.filter(|h| h.starts_with("http://") || h.starts_with("https://")) else {
            continue;
        };
        if cached(&href).is_some_and(|bytes| !bytes.is_empty()) {
            continue;
        }
        if provider.allow_remote && cached(&href).is_none() {
            provider.prefetch(href);
        }
        out.push_str(&html[last..start]);
        last = end;
    }
    out.push_str(&html[last..]);
    out
}

fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let rest = url.strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    if meta.to_ascii_lowercase().ends_with(";base64") {
        let cleaned: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
        let cleaned = percent_decode(&cleaned);
        base64::engine::general_purpose::STANDARD
            .decode(cleaned.trim_end_matches('='))
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(cleaned.trim_end_matches('=')))
            .ok()
    } else {
        Some(percent_decode(payload).into_bytes())
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// Dark recolouring

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Background,
    Foreground,
}

/// Rewrite every colour the sender set (inline styles, `<style>` blocks and
/// legacy attributes) for a dark canvas.
pub(crate) fn recolor_for_dark(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    while let Some(rel) = lower[i..].find('<') {
        let start = i + rel;
        out.push_str(&html[i..start]);
        // <style> … </style>: rewrite declarations inside.
        if lower[start..].starts_with("<style") {
            if let Some(open_end) = lower[start..].find('>') {
                let body_start = start + open_end + 1;
                let close = lower[body_start..].find("</style").map_or(html.len(), |c| body_start + c);
                out.push_str(&html[start..body_start]);
                out.push_str(&recolor_declarations(&html[body_start..close]));
                i = close;
                continue;
            }
        }
        // Comments pass through untouched.
        if lower[start..].starts_with("<!--") {
            let end = lower[start..].find("-->").map_or(html.len(), |e| start + e + 3);
            out.push_str(&html[start..end]);
            i = end;
            continue;
        }
        let Some(tag_len) = tag_end(&html[start..]) else {
            out.push_str(&html[start..]);
            return out;
        };
        out.push_str(&recolor_tag(&html[start..start + tag_len]));
        i = start + tag_len;
    }
    out.push_str(&html[i..]);
    out
}

/// Length of the tag starting at `s[0] == '<'`, honouring quoted attributes.
fn tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (index, byte) in s.bytes().enumerate().skip(1) {
        match (quote, byte) {
            (Some(q), b) if b == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'>') => return Some(index + 1),
            _ => {}
        }
    }
    None
}

fn recolor_tag(tag: &str) -> String {
    if tag.starts_with("</") || tag.starts_with("<!") {
        return tag.to_owned();
    }
    let mut out = String::with_capacity(tag.len() + 16);
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Attribute name at a word boundary.
        if bytes[i].is_ascii_alphabetic() && i > 0 && bytes[i - 1].is_ascii_whitespace() {
            let name_end = tag[i..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .map_or(tag.len(), |n| i + n);
            let name = tag[i..name_end].to_ascii_lowercase();
            let rest = &tag[name_end..];
            let trimmed = rest.trim_start();
            if let Some(after_eq) = trimmed.strip_prefix('=') {
                let value_start = name_end + (rest.len() - trimmed.len()) + 1;
                let lead = after_eq.len() - after_eq.trim_start().len();
                let value_start = value_start + lead;
                let (value, value_end, quote) = match tag.as_bytes().get(value_start) {
                    Some(&q @ (b'"' | b'\'')) => {
                        let close = tag[value_start + 1..]
                            .find(q as char)
                            .map_or(tag.len(), |c| value_start + 1 + c);
                        (&tag[value_start + 1..close], (close + 1).min(tag.len()), Some(q as char))
                    }
                    _ => {
                        let end = tag[value_start..]
                            .find(|c: char| c.is_ascii_whitespace() || c == '>')
                            .map_or(tag.len(), |e| value_start + e);
                        (&tag[value_start..end], end, None)
                    }
                };
                let replaced = match name.as_str() {
                    "style" => Some(recolor_declarations(value)),
                    "bgcolor" => Some(recolor_value(value, Role::Background)),
                    "color" | "text" | "link" | "vlink" | "alink" => {
                        Some(recolor_value(value, Role::Foreground))
                    }
                    _ => None,
                };
                if let Some(replaced) = replaced {
                    out.push_str(&tag[i..value_start]);
                    match quote {
                        Some(q) => {
                            out.push(q);
                            out.push_str(&replaced.replace(q, if q == '"' { "'" } else { "\"" }));
                            out.push(q);
                        }
                        None => out.push_str(&replaced),
                    }
                    i = value_end;
                    continue;
                }
            }
            out.push_str(&tag[i..name_end]);
            i = name_end;
            continue;
        }
        let ch = tag[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Rewrite `prop: value` declarations in a style attribute or sheet.
fn recolor_declarations(css: &str) -> String {
    let mut out = String::with_capacity(css.len() + 16);
    let mut rest = css;
    loop {
        let Some(colon) = rest.find(':') else {
            out.push_str(rest);
            return out;
        };
        // Property name: the identifier right before the colon.
        let before = &rest[..colon];
        let name_start = before
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .map_or(0, |p| p + 1);
        let property = before[name_start..].to_ascii_lowercase();
        let value_end = rest[colon + 1..]
            .find([';', '}', '{'])
            .map_or(rest.len(), |e| colon + 1 + e);
        // A selector pseudo-class (`a:hover {`) is not a declaration.
        let is_selector = rest[colon + 1..value_end].trim().is_empty()
            || rest[colon + 1..].find('{').is_some_and(|b| colon + 1 + b == value_end);
        out.push_str(&rest[..colon + 1]);
        let value = &rest[colon + 1..value_end];
        let role = match property.as_str() {
            _ if is_selector => None,
            "background" | "background-color" => Some(Role::Background),
            p if p.starts_with("border") || p.starts_with("outline") => Some(Role::Background),
            "color" | "text-decoration-color" | "caret-color" | "fill" | "stroke" => {
                Some(Role::Foreground)
            }
            _ => None,
        };
        match role {
            Some(role) => out.push_str(&recolor_value(value, role)),
            None => out.push_str(value),
        }
        rest = &rest[value_end..];
    }
}

/// Replace every colour token in a CSS value.
fn recolor_value(value: &str, role: Role) -> String {
    let mut out = String::with_capacity(value.len());
    let lower = value.to_ascii_lowercase();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &lower[i..];
        if bytes[i] == b'#' {
            let len = rest[1..]
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(rest.len() - 1);
            if matches!(len, 3 | 4 | 6 | 8) {
                if let Some(color) = parse_hex(&rest[1..1 + len]) {
                    out.push_str(&transform(color, role));
                    i += 1 + len;
                    continue;
                }
            }
        } else if rest.starts_with("rgb") {
            if let Some(close) = rest.find(')') {
                if let Some(color) = parse_rgb_function(&rest[..=close]) {
                    out.push_str(&transform(color, role));
                    i += close + 1;
                    continue;
                }
            }
        } else if bytes[i].is_ascii_alphabetic()
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-'))
        {
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .unwrap_or(rest.len());
            if let Some(color) = named_color(&rest[..len]) {
                out.push_str(&transform(color, role));
                i += len;
                continue;
            }
            out.push_str(&value[i..i + len]);
            i += len;
            continue;
        }
        let ch = value[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

type Rgba = ([f32; 3], f32);

fn parse_hex(hex: &str) -> Option<Rgba> {
    let digit = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f32 / 255.);
    let double = |c: &str| digit(&format!("{c}{c}"));
    match hex.len() {
        3 | 4 => Some((
            [double(&hex[0..1])?, double(&hex[1..2])?, double(&hex[2..3])?],
            if hex.len() == 4 { double(&hex[3..4])? } else { 1. },
        )),
        6 | 8 => Some((
            [digit(&hex[0..2])?, digit(&hex[2..4])?, digit(&hex[4..6])?],
            if hex.len() == 8 { digit(&hex[6..8])? } else { 1. },
        )),
        _ => None,
    }
}

fn parse_rgb_function(text: &str) -> Option<Rgba> {
    let inner = &text[text.find('(')? + 1..text.rfind(')')?];
    let parts: Vec<&str> = inner
        .split([',', ' ', '/'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |p: &str| -> Option<f32> {
        if let Some(pct) = p.strip_suffix('%') {
            pct.parse::<f32>().ok().map(|v| v / 100.)
        } else {
            p.parse::<f32>().ok().map(|v| v / 255.)
        }
    };
    let alpha = match parts.get(3) {
        Some(a) => match a.strip_suffix('%') {
            Some(pct) => pct.parse::<f32>().ok()? / 100.,
            None => a.parse::<f32>().ok()?,
        },
        None => 1.,
    };
    Some(([channel(parts[0])?, channel(parts[1])?, channel(parts[2])?], alpha))
}

fn named_color(name: &str) -> Option<Rgba> {
    let hex = match name {
        "white" => "ffffff",
        "black" => "000000",
        "gray" | "grey" => "808080",
        "silver" => "c0c0c0",
        "lightgray" | "lightgrey" => "d3d3d3",
        "gainsboro" => "dcdcdc",
        "whitesmoke" => "f5f5f5",
        "darkgray" | "darkgrey" => "a9a9a9",
        "dimgray" | "dimgrey" => "696969",
        "red" => "ff0000",
        "maroon" => "800000",
        "green" => "008000",
        "navy" => "000080",
        "blue" => "0000ff",
        "purple" => "800080",
        "teal" => "008080",
        "olive" => "808000",
        "orange" => "ffa500",
        "beige" => "f5f5dc",
        "ivory" => "fffff0",
        "linen" => "faf0e6",
        "snow" => "fffafa",
        "aliceblue" => "f0f8ff",
        "ghostwhite" => "f8f8ff",
        "floralwhite" => "fffaf0",
        "lightyellow" => "ffffe0",
        "lightblue" => "add8e6",
        "lightgreen" => "90ee90",
        _ => return None,
    };
    parse_hex(hex)
}

fn transform((rgb, alpha): Rgba, role: Role) -> String {
    let [l, a, b] = srgb_to_lab(rgb);
    let (l, a, b) = match role {
        // Brand colours (buttons, accent bars) stay recognisable; only
        // paper-like light, low-chroma fills go dark.
        Role::Background if (a * a + b * b).sqrt() > 35. => (l.min(64.), a, b),
        Role::Background if l > 40. => (8. + (100. - l) * 0.35, a * 0.6, b * 0.6),
        Role::Foreground if l < 65. => (88. - l * 0.25, a * 0.9, b * 0.9),
        _ => (l, a, b),
    };
    let [r, g, bl] = lab_to_srgb([l, a, b]);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    if alpha < 1. {
        format!("rgba({}, {}, {}, {:.3})", byte(r), byte(g), byte(bl), alpha)
    } else {
        format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(bl))
    }
}

const WHITE: [f32; 3] = [0.950_47, 1.0, 1.088_83];

fn srgb_to_lab(rgb: [f32; 3]) -> [f32; 3] {
    let lin = rgb.map(|c| {
        if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    });
    let xyz = [
        0.4124 * lin[0] + 0.3576 * lin[1] + 0.1805 * lin[2],
        0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2],
        0.0193 * lin[0] + 0.1192 * lin[1] + 0.9505 * lin[2],
    ];
    let f = |t: f32| if t > 0.008_856 { t.cbrt() } else { 7.787 * t + 16. / 116. };
    let (fx, fy, fz) = (f(xyz[0] / WHITE[0]), f(xyz[1] / WHITE[1]), f(xyz[2] / WHITE[2]));
    [116. * fy - 16., 500. * (fx - fy), 200. * (fy - fz)]
}

fn lab_to_srgb([l, a, b]: [f32; 3]) -> [f32; 3] {
    let fy = (l + 16.) / 116.;
    let fx = fy + a / 500.;
    let fz = fy - b / 200.;
    let inv = |t: f32| {
        let cube = t * t * t;
        if cube > 0.008_856 { cube } else { (t - 16. / 116.) / 7.787 }
    };
    let (x, y, z) = (inv(fx) * WHITE[0], inv(fy) * WHITE[1], inv(fz) * WHITE[2]);
    let lin = [
        3.2406 * x - 1.5372 * y - 0.4986 * z,
        -0.9689 * x + 1.8758 * y + 0.0415 * z,
        0.0557 * x - 0.2040 * y + 1.0570 * z,
    ];
    lin.map(|c| {
        let c = c.clamp(0., 1.);
        if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1. / 2.4) - 0.055 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bogus_image_sizes_are_dropped() {
        let out = tame_inline_tags(r#"<img style="max-width:100%" src="a.jpg" width="undefined" height="NaN"><img width="24" src=b>"#);
        assert!(!out.contains("undefined") && !out.contains("NaN"), "{out}");
        assert!(out.contains(r#"width="24""#), "{out}");
    }

    #[test]
    fn inline_tags_get_layouts_blitz_can_draw() {
        let html = concat!(
            r#"<style>td{display:block}</style>"#,
            r#"<td style="border-radius:12px;display:block;min-width:370px">btn</td>"#,
            r#"<TD class=x>plain</TD>"#,
            r#"<img src="a.jpg" style="height:90px;width:90px;border-radius:50%;" />"#,
            r#"<img width="24" src='b.jpg?a=1>2' height=24 style="display:block;margin-left:18px">"#,
            r#"<img src="hero.jpg" width="600" style="max-width:100%">"#,
        );
        let out = tame_inline_tags(html);
        assert!(out.starts_with("<style>td{display:block}</style>"), "CSS text untouched: {out}");
        assert!(out.contains(r#"<td style="border-radius:12px;min-width:370px">btn</td>"#), "{out}");
        assert!(out.contains("<TD class=x>plain</TD>"));
        assert!(out.contains(r#"style="height:90px;width:90px;border-radius:50%;min-width:90px;" />"#), "{out}");
        assert!(out.contains(r#"src='b.jpg?a=1>2' height=24 style="display:block;margin-left:18px;width:24px;height:24px;min-width:24px;">"#), "{out}");
        assert!(out.contains(r#"<img src="hero.jpg" width="600" style="max-width:100%">"#), "large images stay responsive: {out}");
    }

    #[test]
    fn light_backgrounds_go_dark_and_dark_text_goes_light() {
        let html = r##"<table bgcolor="#ffffff"><tr><td style="color:#333333;background-color:white;border:1px solid #eeeeee">Hi</td></tr></table><style>.a{color:rgb(0,0,0)} a:hover{color:#000}</style>"##;
        let out = recolor_for_dark(html);
        assert!(!out.contains("#ffffff") && !out.contains("white;"), "{out}");
        assert!(!out.contains("#333333"), "{out}");
        assert!(out.contains("a:hover{color:"), "selectors survive: {out}");
        assert!(out.contains(">Hi<"));
        let [l, ..] = srgb_to_lab(parse_hex(&out[out.find("bgcolor=\"#").unwrap() + 10..][..6]).unwrap().0);
        assert!(l < 30., "background lightness {l}");
    }

    #[test]
    fn text_content_and_urls_are_untouched() {
        let html = r#"<p>white black #fff</p><a href="https://x.test/#red">red</a>"#;
        assert_eq!(recolor_for_dark(html), html);
    }

    #[test]
    fn wide_fixed_designs_are_shrunk_to_fit() {
        let html = r#"<table width="900" style="width:900px"><tr><td style="height:300px;background:#eee">wide</td></tr></table>"#;
        let params = Params {
            width: 450.,
            scale: 1.,
            theme: Theme { dark: false, background: [255; 3], text: [0; 3], link: [0, 0, 255] },
            original_colors: false,
            allow_remote: false,
            hide_history: false,
        };
        let (_, rendered) = render_now(html, &params).expect("rendered");
        assert!(rendered.fit < 0.6, "fit {}", rendered.fit);
        assert_eq!(rendered.width, 450.);
    }

    #[test]
    fn percentage_cells_with_padding_stay_inside_the_table() {
        let html = r#"<table style="width:100%;border-collapse:collapse"><tr><td style="width:50%;padding:24px">Left panel text that wraps</td><td style="width:50%;padding:24px">Right panel text that wraps</td></tr></table>"#;
        let params = Params {
            width: 640.,
            scale: 1.,
            theme: Theme { dark: false, background: [255; 3], text: [0; 3], link: [0, 0, 255] },
            original_colors: false,
            allow_remote: false,
            hide_history: false,
        };
        let (doc, rendered) = render_now(html, &params).expect("rendered");
        assert_eq!(rendered.fit, 1., "no overflow, no shrink");
        assert!(right_extent(&doc, doc.root_element().id, 0., 0) <= 641.);
    }

    #[test]
    fn data_urls_decode() {
        assert_eq!(decode_data_url("data:text/plain;base64,aGk=").unwrap(), b"hi");
        assert_eq!(decode_data_url("data:text/plain,a%20b").unwrap(), b"a b");
    }

    #[test]
    fn renders_a_newsletter_and_finds_its_link() {
        let html = r#"<html><body><table width="600"><tr><td style="background:#5b43e8;color:#fff;padding:20px"><h1>Digest</h1></td></tr><tr><td style="padding:20px"><a href="https://example.com/go" style="display:block;height:40px">Open</a></td></tr></table></body></html>"#;
        let params = Params {
            width: 640.,
            scale: 1.,
            theme: Theme { dark: true, background: [13, 13, 13], text: [232, 232, 234], link: [124, 134, 255] },
            original_colors: false,
            allow_remote: false,
            hide_history: false,
        };
        let (doc, rendered) = render_now(html, &params).expect("rendered");
        assert!(rendered.height > 60.);
        // Somewhere on the link block, the hit test finds the href.
        let found = (0..20).any(|step| {
            link_in(&doc, 40., 80. + step as f32 * 5.).as_deref() == Some("https://example.com/go")
        });
        assert!(found, "link not hit");
    }

    /// Manual check: MM_HTML=mail.html cargo test render_file -- --ignored
    #[test]
    #[ignore]
    fn render_file() {
        let Ok(path) = std::env::var("MM_HTML") else { return };
        let html = std::fs::read_to_string(&path).unwrap();
        for dark in [true, false] {
            let params = Params {
                width: 720.,
                scale: 1.,
                theme: if dark {
                    Theme { dark: true, background: [30, 30, 34], text: [230; 3], link: [140, 160, 255] }
                } else {
                    Theme { dark: false, background: [255; 3], text: [0; 3], link: [0, 0, 255] }
                },
                original_colors: false,
                allow_remote: std::env::var("MM_REMOTE").is_ok(),
                hide_history: false,
            };
            let start = std::time::Instant::now();
            let (_, mut rendered) = render_now(&html, &params).expect("rendered");
            if let Some(done) = rendered.remote_done.take() {
                let _ = futures::executor::block_on(done);
                rendered = render_now(&html, &params).expect("rendered").1;
            }
            eprintln!("dark={dark} {:?} fit={}", start.elapsed(), rendered.fit);
            let size = rendered.image.size(0);
            let mut bytes = rendered.image.as_bytes(0).unwrap().to_vec();
            for pixel in bytes.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, bytes)
                .unwrap()
                .save(format!(
                    "{}/render-{}.png",
                    std::env::var("MM_OUT").unwrap_or_else(|_| "/tmp".into()),
                    if dark { "dark" } else { "light" }
                ))
                .unwrap();
        }
    }
}

#[cfg(test)]
mod timing {
    #[test]
    #[ignore]
    fn render_twice() {
        let Ok(path) = std::env::var("MM_HTML") else { return };
        let html = std::fs::read_to_string(&path).unwrap();
        let params = super::Params {
            width: 720.,
            scale: 1.,
            theme: super::Theme { dark: true, background: [30, 30, 34], text: [230; 3], link: [140, 160, 255] },
            original_colors: false,
            allow_remote: false,
            hide_history: false,
        };
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let _ = super::render_now(&html, &params).unwrap();
            eprintln!("render {:?}", start.elapsed());
        }
    }
}
