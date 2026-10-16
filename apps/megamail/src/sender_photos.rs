//! Explicit local photos only. Bounded decoding and filesystem work stay off-thread.
use crate::*;
use gpui_kit::StyledImage as _;
use std::collections::{HashMap, HashSet};
use std::io::Read as _;

#[derive(Default)]
pub(crate) struct State {
    images: HashMap<String, Arc<gpui_kit::RenderImage>>,
    requested: HashSet<String>,
    busy: bool,
    active_loads: usize,
    pending: std::collections::VecDeque<(String, Option<String>)>,
    revisions: HashMap<String, u64>,
}
fn email_key(email: &str) -> Option<String> {
    let key = email.trim().to_lowercase();
    (key.len() <= 254 && key.contains('@') && !key.chars().any(char::is_control)).then_some(key)
}
fn local_path(key: &str) -> Option<PathBuf> {
    // Full reversible key, chunked below the filesystem's filename length limit.
    // No path characters from an email address ever become path components.
    let hex = key
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut path = config::data_base()?.join("sender-photos.v1");
    for chunk in hex.as_bytes().chunks(64) {
        path.push(std::str::from_utf8(chunk).ok()?);
    }
    Some(path.join("photo.png"))
}
fn decode(path: &Path) -> Result<image::RgbaImage, String> {
    let file = std::fs::File::open(path).map_err(|_| "The local photo could not be opened.")?;
    let mut bytes = Vec::new();
    file.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The local photo could not be read.")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Choose a PNG, JPEG or WebP photo smaller than 2 MiB.".into());
    }
    let format = image::guess_format(&bytes).map_err(|_| "Choose a PNG, JPEG or WebP photo.")?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
    ) {
        return Err("Choose a PNG, JPEG or WebP photo.".into());
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|image| {
            image
                .resize_to_fill(128, 128, image::imageops::FilterType::Triangle)
                .to_rgba8()
        })
        .map_err(|_| "The photo could not be decoded within the image limits.".into())
}
fn render_image(mut pixels: image::RgbaImage) -> Arc<gpui_kit::RenderImage> {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(pixels)]))
}
impl MailApp {
    pub(crate) fn load_sender_photo(&mut self, email: &str, cx: &mut Context<Self>) {
        let Some(key) = email_key(email) else {
            return;
        };
        if !self.sender_photos.requested.insert(key.clone()) {
            return;
        }
        // At most 128 retained thumbnails (8 MiB), including negative lookups.
        if self.sender_photos.requested.len() > 128 {
            self.sender_photos.requested.clear();
            self.sender_photos.images.clear();
            self.sender_photos.pending.clear();
            self.sender_photos.requested.insert(key.clone());
        }
        let configured = self
            .profiles
            .iter()
            .find(|(_, profile)| profile.email.eq_ignore_ascii_case(&key))
            .and_then(|(_, profile)| profile.avatar.clone())
            .filter(|name| Path::new(name).components().count() == 1 && !name.starts_with('.'));
        self.sender_photos.pending.push_back((key, configured));
        self.pump_sender_photos(cx);
    }
    fn pump_sender_photos(&mut self, cx: &mut Context<Self>) {
        if self.sender_photos.active_loads >= 2 {
            return;
        }
        let Some((key, configured)) = self.sender_photos.pending.pop_front() else {
            return;
        };
        self.sender_photos.active_loads += 1;
        let revision = self.sender_photos.revisions.get(&key).copied().unwrap_or(0);
        let worker_key = key.clone();
        let task = cx.background_spawn(async move {
            let local = local_path(&worker_key)?;
            let pixels = decode(&local).ok().or_else(|| {
                let name = configured?;
                decode(&config::data_base()?.join("avatars").join(name)).ok()
            })?;
            Some(render_image(pixels))
        });
        cx.spawn(async move |this, cx| {
            let image = task.await;
            let _ = this.update(cx, |this, cx| {
                this.sender_photos.active_loads -= 1;
                if revision == this.sender_photos.revisions.get(&key).copied().unwrap_or(0)
                    && this.sender_photos.requested.contains(&key)
                {
                    if let Some(image) = image {
                        this.sender_photos.images.insert(key, image);
                        cx.notify();
                    }
                }
                this.pump_sender_photos(cx);
            });
        })
        .detach();
        self.pump_sender_photos(cx);
    }
    pub(crate) fn sender_avatar(
        &self,
        email: &str,
        label: &str,
        color: gpui_kit::Hsla,
        _: &App,
    ) -> AnyElement {
        if let Some(image) = email_key(email).and_then(|key| self.sender_photos.images.get(&key)) {
            return gpui_kit::img(image.clone())
                .size(px(32.))
                .flex_shrink_0()
                .rounded_full()
                .object_fit(gpui_kit::ObjectFit::Cover)
                .into_any_element();
        }
        div()
            .flex()
            .flex_shrink_0()
            .size(px(32.))
            .rounded_full()
            .bg(color.opacity(if self.dark { 0.14 } else { 0.09 }))
            .text_color(color)
            .text_size(px(11.))
            .items_center()
            .justify_center()
            .child(initials(label))
            .into_any_element()
    }
    pub(crate) fn choose_sender_photo(&mut self, email: String, cx: &mut Context<Self>) {
        if self.sender_photos.busy {
            return;
        }
        let Some(key) = email_key(&email) else {
            return;
        };
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose local sender photo".into()),
        });
        self.sender_photos.busy = true;
        cx.spawn(async move |this, cx| {
            let chosen = picker.await;
            let result = match chosen {
                Ok(Ok(Some(paths))) => if let Some(path) = paths.into_iter().next() {
                    let worker_key = key.clone();
                    cx.background_spawn(async move {
                        let pixels = decode(&path)?;
                        let destination = local_path(&worker_key).ok_or("Local photo storage is unavailable.")?;
                        let mut encoded = std::io::Cursor::new(Vec::new());
                        pixels.write_to(&mut encoded, image::ImageFormat::Png).map_err(|_| "The photo could not be encoded.")?;
                        // Rename a private, fully written PNG into place.
                        let parent = destination.parent().ok_or("Local photo storage is unavailable.")?;
                        std::fs::create_dir_all(parent).map_err(|_| "The photo directory could not be created.")?;
                        let temporary = parent.join(format!("photo.{}.{}.pending", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| "The photo could not be saved.")?.as_nanos()));
                        use std::os::unix::fs::OpenOptionsExt as _;
                        use std::io::Write as _;
                        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).map_err(|_| "The photo could not be saved. Try choosing the photo again.")?;
                        let write = file.write_all(encoded.get_ref()).and_then(|_| file.sync_all()).and_then(|_| std::fs::rename(&temporary, destination));
                        if write.is_err() { let _ = std::fs::remove_file(&temporary); return Err("The photo could not be saved.".to_owned()); }
                        Ok(Some(render_image(pixels)))
                    }).await
                } else { Ok(None) },
                Ok(Ok(None)) => Ok(None),
                _ => Err("The photo picker could not be opened.".to_owned()),
            };
            let _ = this.update(cx, |this, cx| {
                this.sender_photos.busy = false;
                match result {
                    Ok(Some(image)) => { let revision = this.sender_photos.revisions.entry(key.clone()).or_default(); *revision = revision.saturating_add(1); this.sender_photos.images.insert(key, image); this.reader_session.notice = Some("Local sender photo saved. No image service was contacted.".into()); },
                    Ok(None) => {},
                    Err(error) => this.reader_session.notice = Some(error),
                }
                cx.notify();
            });
        }).detach();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_email_keys_are_bounded_and_normalized() {
        assert_eq!(
            email_key(" ADA@EXAMPLE.TEST ").as_deref(),
            Some("ada@example.test")
        );
        assert!(email_key("no-address").is_none());
        assert!(email_key("ada\n@example.test").is_none());
        assert!(email_key(&format!("{}@example.test", "x".repeat(254))).is_none());
    }
}
