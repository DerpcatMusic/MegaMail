//! Explicit local PNG/JPEG/WebP preview; no filesystem or provider reads.
use crate::*;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::{Sizable as _, button::ButtonVariants as _};
use gpui_kit::{
    AppContext as _, IntoElement as _, ParentElement as _, Styled as _, StyledImage as _,
    prelude::FluentBuilder as _,
};
use std::io::Cursor;
const INPUT_MAX: usize = 8 * 1024 * 1024;
const DIMENSION_MAX: u32 = 4096;

struct Preview {
    identity: reader_session::SemanticTextIdentity,
    name: String,
    content: Arc<Vec<u8>>,
    image: Arc<gpui_kit::RenderImage>,
    dimensions: (u32, u32),
}
#[derive(Default)]
pub(crate) struct State {
    busy: bool,
    revision: u64,
    preview: Option<Preview>,
    error: Option<String>,
}
impl State {
    pub fn clear(&mut self) {
        self.preview = None;
        self.error = None;
        self.revision = self.revision.saturating_add(1);
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
}
const PDF_INPUT_MAX: usize = 32 * 1024 * 1024;
/// Longest side of a rendered PDF page, in pixels.
const PDF_RENDER_MAX: f32 = 1600.;

fn is_pdf(bytes: &[u8]) -> bool {
    // The header may follow a little junk; readers accept it within 1 KiB.
    bytes[..bytes.len().min(1024)]
        .windows(5)
        .any(|window| window == b"%PDF-")
}

fn previewable(bytes: &[u8]) -> bool {
    if is_pdf(bytes) {
        return bytes.len() <= PDF_INPUT_MAX;
    }
    !bytes.is_empty()
        && bytes.len() <= INPUT_MAX
        && image::guess_format(bytes).is_ok_and(|format| {
            matches!(
                format,
                image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
            )
        })
}
/// First page of a PDF, rasterized in pure Rust (hayro) on a white page.
/// Malformed files are contained: a panic inside the interpreter becomes an error.
fn decode_pdf(bytes: &[u8]) -> Result<(image::RgbaImage, (u32, u32)), String> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    if bytes.len() > PDF_INPUT_MAX {
        return Err("PDF preview supports files up to 32 MiB. Save this attachment to open it.".into());
    }
    let data = bytes.to_vec();
    std::panic::catch_unwind(move || {
        let pdf = Pdf::new(data).map_err(|_| "This PDF could not be read (it may be encrypted or damaged).".to_owned())?;
        let pages = pdf.pages();
        let page = pages.first().ok_or("This PDF has no pages.")?;
        let (width, height) = page.render_dimensions();
        if !(width > 0. && height > 0.) {
            return Err("This PDF page has no size.".into());
        }
        let scale = (PDF_RENDER_MAX / width.max(height)).clamp(0.1, 3.);
        let settings = hayro::RenderSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
            ..Default::default()
        };
        let pixmap = hayro::render(page, &hayro::RenderCache::new(), &InterpreterSettings::default(), &settings);
        let (w, h) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
        // Opaque white background: premultiplied equals straight alpha.
        let mut pixels = pixmap.data_as_u8_slice().to_vec();
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let image = image::RgbaImage::from_raw(w, h, pixels).ok_or("The PDF page could not be drawn.")?;
        Ok((image, (w, h)))
    })
    .unwrap_or_else(|_| Err("This PDF could not be drawn.".into()))
}

fn decode(bytes: &[u8]) -> Result<(image::RgbaImage, (u32, u32)), String> {
    if is_pdf(bytes) {
        return decode_pdf(bytes);
    }
    if !previewable(bytes) {
        return Err("Preview supports PNG, JPEG and WebP images up to 8 MiB and PDFs up to 32 MiB. Save other attachments to open them.".into());
    }
    let format =
        image::guess_format(bytes).map_err(|_| "This image format could not be recognized.")?;
    let dimensions = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| "The image dimensions could not be read.")?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > DIMENSION_MAX
        || dimensions.1 > DIMENSION_MAX
    {
        return Err(
            "Preview supports images up to 4096 × 4096 pixels. Save this attachment to open it."
                .into(),
        );
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(DIMENSION_MAX);
    limits.max_image_height = Some(DIMENSION_MAX);
    // Allocation limits are best effort; dimension limits and a single flight bound decoded output.
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| "The image could not be decoded within preview limits.")?;
    let mut buffer = decoded.thumbnail(1600, 1600).to_rgba8();
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok((buffer, dimensions))
}
fn current_attachment(
    snapshot: &MailboxSnapshot,
    key: &MessageKey,
    name: &str,
    content: &Arc<Vec<u8>>,
) -> bool {
    snapshot.selected_key.as_ref() == Some(key)
        && snapshot.attachment_key.as_ref() == Some(key)
        && matches!(&snapshot.attachment_state, AttachmentState::Ready { items, .. }
            if items.iter().any(|a| a.name == name && Arc::ptr_eq(&a.content, content)))
}
impl MailApp {
    pub(crate) fn preview_attachment(
        &mut self,
        name: String,
        content: Arc<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        if self.attachment_preview.busy || self.view != View::Mailbox || self.composer.is_some() {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(identity) = self.reader_text_identity(&snapshot) else {
            return;
        };
        if !current_attachment(&snapshot, &identity.key, &name, &content) {
            return;
        }
        let Some(revision) = self.attachment_preview.revision.checked_add(1) else {
            self.attachment_preview.error =
                Some("Preview lifetime exhausted. Restart before another preview.".into());
            cx.notify();
            return;
        };
        self.attachment_preview.revision = revision;
        self.attachment_preview.busy = true;
        self.attachment_preview.preview = None;
        self.attachment_preview.error = None;
        let task = cx.background_spawn(async move {
            decode(&content).map(|(buffer, dimensions)| Preview {
                identity,
                name,
                content,
                dimensions,
                image: Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(buffer)])),
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.attachment_preview.busy = false;
                if this.attachment_preview.revision != revision {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(preview) => {
                        let snapshot = this.live.read(cx).snapshot();
                        if this.view == View::Mailbox
                            && this.reader_text_identity(&snapshot).as_ref()
                                == Some(&preview.identity)
                            && current_attachment(
                                &snapshot,
                                &preview.identity.key,
                                &preview.name,
                                &preview.content,
                            )
                        {
                            this.attachment_preview.preview = Some(preview);
                        }
                    }
                    Err(error) => this.attachment_preview.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn attachment_preview_panel(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.attachment_preview;
        let preview = state.preview.as_ref().filter(|p| {
            self.reader_text_identity(snapshot).as_ref() == Some(&p.identity)
                && current_attachment(snapshot, &p.identity.key, &p.name, &p.content)
        });
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .when(state.busy, |el| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.muted)
                        .child("Decoding image preview…"),
                )
            })
            .when_some(state.error.clone(), |el, error| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.muted)
                        .child(error),
                )
            })
            .when_some(preview, |el, preview| {
                el.child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.))
                                .child(format!(
                                    "{} · {} × {} · static preview",
                                    preview.name, preview.dimensions.0, preview.dimensions.1
                                )),
                        )
                        .child(
                            Button::new("close-attachment-preview")
                                .ghost()
                                .small()
                                .label("Close preview")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.attachment_preview.clear();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    gpui_kit::img(preview.image.clone())
                        .w_full()
                        .h(px(360.))
                        .object_fit(gpui_kit::ObjectFit::Contain),
                )
            })
            .into_any_element()
    }
}
pub(crate) fn preview_button(
    name: String,
    content: Arc<Vec<u8>>,
    index: usize,
    busy: bool,
    cx: &mut Context<MailApp>,
) -> AnyElement {
    if !previewable(&content) {
        return div().into_any_element();
    }
    Button::new(SharedString::from(format!("preview-message-image-{index}")))
        .ghost()
        .small()
        .label(if is_pdf(&content) { "Preview PDF" } else { "Preview image" })
        .accessibility_label(format!("Preview attachment {name}"))
        .disabled(busy)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.preview_attachment(name.clone(), content.clone(), cx)
        }))
        .into_any_element()
}
#[cfg(test)]
mod tests {
    use super::*;
    /// A one-page PDF with a blue square, with a correct xref table.
    fn tiny_pdf() -> Vec<u8> {
        let content = "0 0 1 rg 20 20 100 100 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).bytes());
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
        for offset in offsets {
            out.extend(format!("{offset:010} 00000 n \n").bytes());
        }
        out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).bytes());
        out
    }

    #[test]
    fn pdf_first_page_renders_on_white() {
        let pdf = tiny_pdf();
        assert!(previewable(&pdf));
        let (image, (w, h)) = decode(&pdf).unwrap();
        assert_eq!(w, h);
        // Corner is white page; centre is the blue square (stored BGRA).
        assert_eq!(image.get_pixel(2, 2).0, [255, 255, 255, 255]);
        let centre = image.get_pixel(w / 2, h / 2).0;
        assert!(centre[0] > 200 && centre[2] < 60, "{centre:?}");
        assert!(decode(b"%PDF-1.4 garbage").is_err());
    }

    #[test]
    fn preview_validates_format_dimensions_and_gpui_channel_order() {
        let buffer = image::RgbaImage::from_pixel(2, 1, image::Rgba([10, 20, 30, 255]));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(buffer)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let (decoded, dimensions) = decode(encoded.get_ref()).unwrap();
        assert_eq!(dimensions, (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [30, 20, 10, 255]);
        assert!(decode(b"not an image").is_err());
        assert!(!previewable(&vec![0; INPUT_MAX + 1]));
        let huge = image::RgbaImage::from_pixel(DIMENSION_MAX + 1, 1, image::Rgba([0, 0, 0, 0]));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(huge)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        assert!(decode(encoded.get_ref()).unwrap_err().contains("4096"));
    }
}
