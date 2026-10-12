//! Ephemeral, isolated WebKit frames; selectable text remains an explicit view.
use crate::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const WIRE_MAX: u64 = 28 * 1024 * 1024;
#[derive(Clone)]
struct Request {
    identity: reader_session::SemanticTextIdentity,
    source: Arc<str>,
    width: u32,
    /// Device pixels per CSS px for the snapshot (1 or 2); layout stays in CSS px.
    scale: u32,
}
impl Request {
    fn matches(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.width == other.width
            && self.scale == other.scale
            && Arc::ptr_eq(&self.source, &other.source)
    }
}
#[derive(Clone, serde::Deserialize)]
struct Link {
    url: String,
    label: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
struct Frame {
    image: Arc<gpui_kit::RenderImage>,
    height: u32,
    width: u32,
    links: Vec<Link>,
    clipped: bool,
}
#[derive(Default)]
pub(crate) struct State {
    desired: Option<Request>,
    completed: Option<Request>,
    frame: Option<Frame>,
    busy: bool,
    cancel: Option<Arc<AtomicBool>>,
    width: u32,
    scale: u32,
}
impl State {
    fn retire(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.frame = None;
        self.completed = None;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.retire();
    }
}
fn helper() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("Home directory unavailable.")?;
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    let directory = data.join("megamail/html-reader");
    reader_session::trusted_helper(directory.join("extensions/libmegamail-reader-policy.so"))?;
    reader_session::trusted_helper(directory.join("megamail-html-reader"))
}
fn wire(request: &Request) -> Result<Vec<u8>, String> {
    if request.source.len() > reader_document::SOURCE_MAX || request.source.contains('\0') {
        return Err("Message exceeds the HTML reader limits.".into());
    }
    // Frozen layout makes link rectangles correspond to the displayed pixels.
    let mut source = request.source.to_string();
    source.push_str("<style>*{animation:none!important;transition:none!important}</style>");
    if !request.identity.quote_visible {
        source.push_str(
            "<style>blockquote,.gmail_quote,.yahoo_quoted{display:none!important}</style>",
        );
    }
    if source.len() > reader_document::SOURCE_MAX {
        return Err("Message exceeds the HTML reader limits.".into());
    }
    serde_json::to_vec(
        &serde_json::json!({"version":1,"html":source,"plainText":"","title":"Message", "viewportWidth":request.width, "deviceScale":request.scale}),
    )
    .map_err(|_| "The HTML reader request could not be prepared.".into())
}
fn parse_frame(bytes: &[u8], scale: u32) -> Result<Frame, String> {
    let fail = || "Styled message rendering was not confirmed.".to_owned();
    if bytes.len() as u64 > WIRE_MAX {
        return Err(fail());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| fail())?;
    let mut token: Option<String> = None;
    let mut links = None;
    let mut png = None;
    let mut clipped = false;
    let mut closed = false;
    for line in text.lines() {
        if closed {
            return Err(fail());
        }
        let event: serde_json::Value = serde_json::from_str(line).map_err(|_| fail())?;
        if event["version"] != 1 {
            return Err(fail());
        }
        let kind = event["event"].as_str().ok_or_else(fail)?;
        if kind == "ready" {
            let t = event["token"].as_str().ok_or_else(fail)?;
            if token.is_some()
                || t.len() != 32
                || !t.bytes().all(|b| b.is_ascii_hexdigit())
                || event["policy"] != "v1:extension+sandbox+ephemeral+no-network"
            {
                return Err(fail());
            }
            token = Some(t.into());
            continue;
        }
        if token.as_deref().is_none() || event["token"].as_str() != token.as_deref() {
            return Err(fail());
        }
        match kind {
            "frame-links" if links.is_none() => {
                let source = event["links"].as_str().ok_or_else(fail)?;
                if source.len() > 3 * 1024 * 1024 {
                    return Err(fail());
                }
                let parsed: Vec<Link> = serde_json::from_str(source).map_err(|_| fail())?;
                if parsed.len() > 256 {
                    return Err(fail());
                }
                links = Some(parsed);
            }
            "frame-clipped" => clipped = true,
            "frame" if png.is_none() => {
                let encoded = event["png"].as_str().ok_or_else(fail)?;
                if encoded.len() > 24 * 1024 * 1024 {
                    return Err(fail());
                }
                png = Some(STANDARD.decode(encoded).map_err(|_| fail())?);
            }
            "closed" if png.is_some() && links.is_some() => closed = true,
            _ => return Err(fail()),
        }
    }
    if !closed {
        return Err(fail());
    }
    let png = png.ok_or_else(fail)?;
    if png.len() > 16 * 1024 * 1024
        || image::guess_format(&png).ok() != Some(image::ImageFormat::Png)
    {
        return Err(fail());
    }
    let dimensions =
        image::ImageReader::with_format(std::io::Cursor::new(&png), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|_| fail())?;
    if scale == 0
        || dimensions.0 % scale != 0
        || !(320..=1280).contains(&(dimensions.0 / scale))
        || dimensions.1 < scale
        || dimensions.1 > 8192 * scale
    {
        return Err(fail());
    }
    // Links and the displayed aspect ratio are in CSS px.
    let logical = (dimensions.0 / scale, dimensions.1 / scale);
    let links = links.ok_or_else(fail)?;
    if links.iter().any(|l| {
        !megamail_core::mail_text::safe_browser_url(&l.url)
            || l.url.len() > 8192
            || l.label.len() > 512
            || ![l.x, l.y, l.w, l.h].iter().all(|v| v.is_finite())
            || l.x < 0.
            || l.y < 0.
            || l.w <= 0.
            || l.h <= 0.
            || l.x + l.w > logical.0 as f32
            || l.y + l.h > logical.1 as f32
    }) {
        return Err(fail());
    }
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1280 * scale);
    limits.max_image_height = Some(8192 * scale);
    limits.max_alloc = Some(64 * 1024 * 1024 * (scale as u64).pow(2));
    reader.limits(limits);
    let mut rgba = reader.decode().map_err(|_| fail())?.to_rgba8();
    for pixel in rgba.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Ok(Frame {
        image: Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(rgba)])),
        height: logical.1,
        width: logical.0,
        links,
        clipped,
    })
}
fn render(request: Request, cancel: Arc<AtomicBool>) -> Result<Frame, String> {
    let mut command = Command::new(helper()?);
    command
        .arg("--snapshot")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let bytes = wire(&request)?;
    if cancel.load(Ordering::Acquire) {
        return Err("Rendering canceled.".into());
    }
    let mut child = command
        .spawn()
        .map_err(|_| "The HTML renderer could not start.")?;
    let mut input = child
        .stdin
        .take()
        .ok_or("The HTML renderer input is unavailable.")?;
    let output = child
        .stdout
        .take()
        .ok_or("The HTML renderer output is unavailable.")?;
    let writer = std::thread::spawn(move || input.write_all(&bytes));
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .take(WIRE_MAX + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let accepted = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status.success();
        }
        if cancel.load(Ordering::Acquire) || start.elapsed() > Duration::from_secs(20) {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            break false;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let written = writer.join().ok().is_some_and(|r| r.is_ok());
    let output = reader
        .join()
        .ok()
        .and_then(Result::ok)
        .ok_or("The HTML renderer output is unavailable.")?;
    if !accepted || !written || cancel.load(Ordering::Acquire) {
        return Err("Styled rendering was interrupted.".into());
    }
    let frame = parse_frame(&output, request.scale)?;
    if frame.width != request.width {
        return Err("The HTML renderer returned a different viewport.".into());
    }
    Ok(frame)
}
impl MailApp {
    pub(crate) fn sync_styled_reader(
        &mut self,
        snapshot: &MailboxSnapshot,
        cx: &mut Context<Self>,
    ) {
        let desired = self
            .reader_text_identity(snapshot)
            .filter(|id| !id.plain && snapshot.body_loaded)
            .and_then(|identity| {
                snapshot
                    .selected_original
                    .as_ref()?
                    .source
                    .clone()
                    .map(|source| Request {
                        identity,
                        source,
                        width: if self.styled_reader.width == 0 {
                            640
                        } else {
                            self.styled_reader.width
                        },
                        scale: self.styled_reader.scale.max(1),
                    })
            });
        if desired.as_ref().is_none_or(|r| {
            self.styled_reader
                .desired
                .as_ref()
                .is_none_or(|d| !d.matches(r))
        }) {
            self.styled_reader.retire();
            self.styled_reader.desired = desired;
        }
        let Some(request) = self.styled_reader.desired.clone() else {
            return;
        };
        if self.styled_reader.busy
            || self
                .styled_reader
                .completed
                .as_ref()
                .is_some_and(|r| r.matches(&request))
        {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.styled_reader.cancel = Some(cancel.clone());
        self.styled_reader.busy = true;
        let original = request.clone();
        let task = cx.background_spawn(async move { render(request, cancel) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.styled_reader.busy = false;
                if this
                    .styled_reader
                    .desired
                    .as_ref()
                    .is_some_and(|d| d.matches(&original))
                {
                    this.styled_reader.completed = Some(original);
                    this.styled_reader.frame = result.ok();
                }
                let snapshot = this.live.read(cx).snapshot();
                this.sync_styled_reader(&snapshot, cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn styled_width_probe(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let current = (self.styled_reader.width, self.styled_reader.scale);
        gpui_kit::canvas(
            move |bounds, window, cx| {
                let width =
                    (((f32::from(bounds.size.width) / 16.).round() * 16.) as u32).clamp(320, 1280);
                // Fractional displays (1.25/1.5) downsample the 2x frame instead of
                // upscaling a 1x one, which is what made text look pixelated.
                let scale = if window.scale_factor() > 1. { 2 } else { 1 };
                if (width, scale) != current {
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if (this.styled_reader.width, this.styled_reader.scale) == (width, scale) {
                                return;
                            }
                            this.styled_reader.width = width;
                            this.styled_reader.scale = scale;
                            let snapshot = this.live.read(cx).snapshot();
                            this.sync_styled_reader(&snapshot, cx);
                            cx.notify();
                        });
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
    pub(crate) fn styled_body(
        &self,
        snapshot: &MailboxSnapshot,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let desired = self.styled_reader.desired.as_ref()?;
        if self.reader_text_identity(snapshot).as_ref() != Some(&desired.identity)
            || !snapshot
                .selected_original
                .as_ref()
                .and_then(|o| o.source.as_ref())
                .is_some_and(|s| Arc::ptr_eq(s, &desired.source))
        {
            return None;
        }
        let frame = self.styled_reader.frame.as_ref()?;
        let mut element = div()
            .relative()
            .w_full()
            .aspect_ratio(frame.width as f32 / frame.height as f32)
            .child(gpui_kit::img(frame.image.clone()).size_full());
        for (index, link) in frame.links.iter().enumerate() {
            let expected = desired.clone();
            let url = link.url.clone();
            element = element.child(
                Button::new(SharedString::from(format!("styled-mail-link-{index}")))
                    .ghost()
                    .min_w_0()
                    .min_h_0()
                    .p_0()
                    .absolute()
                    .left(relative(link.x / frame.width as f32))
                    .top(relative(link.y / frame.height as f32))
                    .w(relative(link.w / frame.width as f32))
                    .h(relative(link.h / frame.height as f32))
                    .accessibility_label(link.label.clone())
                    .tooltip(url.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let snapshot = this.live.read(cx).snapshot();
                        if this.reader_text_identity(&snapshot).as_ref() == Some(&expected.identity)
                            && snapshot
                                .selected_original
                                .as_ref()
                                .and_then(|o| o.source.as_ref())
                                .is_some_and(|s| Arc::ptr_eq(s, &expected.source))
                            && megamail_core::mail_text::safe_browser_url(&url)
                        {
                            cx.open_url(&url);
                        }
                    })),
            );
        }
        Some(
            v_flex()
                .w_full()
                .gap_2()
                .child(element)
                .when(frame.clipped, |el| {
                    el.child(div().text_size(px(11.)).child(
                        "Long message truncated in styled view. Switch to text to read the rest.",
                    ))
                })
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn protocol(link: serde_json::Value) -> Vec<u8> {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(320, 2)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let token = "0123456789abcdef0123456789abcdef";
        [
            serde_json::json!({"version":1,"event":"ready","token":token,"policy":"v1:extension+sandbox+ephemeral+no-network"}),
            serde_json::json!({"version":1,"event":"frame-links","token":token,"links":serde_json::to_string(&link).unwrap()}),
            serde_json::json!({"version":1,"event":"frame","token":token,"png":STANDARD.encode(png.into_inner())}),
            serde_json::json!({"version":1,"event":"closed","token":token}),
        ].iter().map(|event| event.to_string()+"\n").collect::<String>().into_bytes()
    }
    #[test]
    fn confirmed_frame_decodes_and_rejects_uncorrelated_or_out_of_frame_links() {
        let link = serde_json::json!([{"url":"https://example.test/notes","label":"Read notes","x":1,"y":0,"w":10,"h":2}]);
        let wire = protocol(link.clone());
        let frame = parse_frame(&wire, 1).unwrap();
        assert_eq!((frame.width, frame.height, frame.links.len()), (320, 2, 1));
        let mut outside = link.clone();
        outside[0]["w"] = 320.into();
        assert!(parse_frame(&protocol(outside), 1).is_err());
        let mut unsafe_link = link;
        unsafe_link[0]["url"] = "javascript:alert(1)".into();
        assert!(parse_frame(&protocol(unsafe_link), 1).is_err());
        let text = String::from_utf8(wire).unwrap();
        let unfinished = text.lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(parse_frame(unfinished.as_bytes(), 1).is_err());
        let mut events: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        events[2]["token"] = "wrong".into();
        let wrong_token = events
            .iter()
            .map(|event| event.to_string() + "\n")
            .collect::<String>();
        assert!(parse_frame(wrong_token.as_bytes(), 1).is_err());
    }
    #[test]
    fn reader_requests_retire_for_selection_generation_source_and_width_changes() {
        let request = Request {
            identity: reader_session::SemanticTextIdentity {
                key: MessageKey {
                    account_id: 7,
                    folder_path: "INBOX".into(),
                    uid: 42,
                },
                version: None,
                generation: 3,
                plain: false,
                quote_visible: true,
            },
            source: Arc::from("<p>Fixture</p>"),
            width: 640,
            scale: 1,
        };
        assert!(request.matches(&request.clone()));
        let mut changed = request.clone();
        changed.width = 400;
        assert!(!request.matches(&changed));
        changed = request.clone();
        changed.identity.generation += 1;
        assert!(!request.matches(&changed));
        changed = request.clone();
        changed.identity.key.uid += 1;
        assert!(!request.matches(&changed));
        changed = request.clone();
        changed.source = Arc::from("<p>Fixture</p>");
        assert!(!request.matches(&changed));
        let cancel = Arc::new(AtomicBool::new(false));
        let mut state = State::default();
        state.desired = Some(request.clone());
        state.completed = Some(request);
        state.busy = true;
        state.cancel = Some(cancel.clone());
        state.retire();
        assert!(cancel.load(Ordering::Acquire));
        assert!(
            state.busy,
            "retirement must not admit a second physical renderer"
        );
        assert!(state.completed.is_none());
    }
    #[test]
    fn forged_frames_and_policy_downgrades_are_rejected() {
        assert!(
            parse_frame(br#"{"version":1,"event":"frame","token":"forged","png":""}"#, 1).is_err()
        );
        assert!(parse_frame(br#"{"version":1,"event":"ready","token":"0123456789abcdef0123456789abcdef","policy":"test-only:no-csp"}"#, 1).is_err());
        assert!(parse_frame(&vec![0; WIRE_MAX as usize + 1], 1).is_err());
    }
}
