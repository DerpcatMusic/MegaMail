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
fn previewable(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.len() <= INPUT_MAX
        && image::guess_format(bytes).is_ok_and(|format| {
            matches!(
                format,
                image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
            )
        })
}
fn decode(bytes: &[u8]) -> Result<(image::RgbaImage, (u32, u32)), String> {
    if !previewable(bytes) {
        return Err("Preview supports PNG, JPEG and WebP files up to 8 MiB. Save other attachments to open them.".into());
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
        .label("Preview image")
        .accessibility_label(format!("Preview image attachment {name}"))
        .disabled(busy)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.preview_attachment(name.clone(), content.clone(), cx)
        }))
        .into_any_element()
}
#[cfg(test)]
mod tests {
    use super::*;
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
