//! Persistent appearance preferences and the off-thread wallpaper pipeline.
//!
//! A selected image is copied as a normalized PNG into MegaMail's private
//! configuration directory. Rendering and effect processing happen once on a
//! background executor; the UI retains one `Arc<RenderImage>` and only paints
//! that cached image. The original file path is never persisted.

#[path = "zeron_wallpaper.rs"]
mod zeron_wallpaper;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::RenderImage;
use image::{
    DynamicImage, Frame, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage, imageops,
};

pub(crate) use zeron_wallpaper::WallpaperEffect;

const PREFS_FILE: &str = "appearance.v1";
const WALLPAPER_DIR: &str = "wallpapers";
const MAX_SOURCE_BYTES: u64 = 24 * 1024 * 1024;
const MAX_STORED_BYTES: u64 = 32 * 1024 * 1024;
const MAX_DECODED_PIXELS: u64 = 16_000_000;
const MAX_DECODE_ALLOC: u64 = 96 * 1024 * 1024;
const MAX_RENDER_EDGE: u32 = 2500;
const MAX_BLUR_SIGMA: f32 = 24.0;
const MAX_PREFS_BYTES: u64 = 16 * 1024;
static WALLPAPER_FILE_ID: AtomicU64 = AtomicU64::new(0);

/// Built-in backgrounds included with MegaMail, plus the user's private image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppearancePreset {
    #[default]
    Aurora,
    Midnight,
    Paper,
    Custom,
}

impl AppearancePreset {
    pub(crate) const ALL: [Self; 3] = [Self::Aurora, Self::Midnight, Self::Paper];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Aurora => "Aurora",
            Self::Midnight => "Midnight",
            Self::Paper => "Paper",
            Self::Custom => "Custom image",
        }
    }

    fn as_key(self) -> &'static str {
        match self {
            Self::Aurora => "aurora",
            Self::Midnight => "midnight",
            Self::Paper => "paper",
            Self::Custom => "custom",
        }
    }

    fn from_key(value: &str) -> Option<Self> {
        match value {
            "aurora" => Some(Self::Aurora),
            "midnight" => Some(Self::Midnight),
            "paper" => Some(Self::Paper),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Wallpaper and processing preferences used by the native appearance panel.
#[derive(Clone)]
pub(crate) struct AppearanceState {
    dark: bool,
    preset: AppearancePreset,
    effect: WallpaperEffect,
    opacity: f32,
    blur_sigma: f32,
    wallpaper_file: Option<String>,
    wallpaper_name: Option<String>,
    image: Option<Arc<RenderImage>>,
    safe_opacity: f32,
}

impl Default for AppearanceState {
    fn default() -> Self {
        Self {
            dark: true,
            preset: AppearancePreset::Aurora,
            effect: WallpaperEffect::None,
            opacity: 0.86,
            blur_sigma: 10.0,
            wallpaper_file: None,
            wallpaper_name: None,
            image: None,
            safe_opacity: 1.0,
        }
    }
}

impl AppearanceState {
    /// Load the small preference record without decoding a wallpaper image.
    /// Call this on the background executor; then call `begin_wallpaper_restore`
    /// there as well to load the selected image after startup.
    pub(crate) fn load() -> Self {
        let mut state = Self::default();
        let Some(path) = preference_path() else {
            return state;
        };
        let Ok(metadata) = fs::metadata(&path) else {
            return state;
        };
        if !metadata.is_file() || metadata.len() > MAX_PREFS_BYTES {
            tracing::warn!("ignoring invalid MegaMail appearance preferences");
            return state;
        }
        let Ok(file) = File::open(&path) else {
            return state;
        };
        let mut contents = String::new();
        if file
            .take(MAX_PREFS_BYTES + 1)
            .read_to_string(&mut contents)
            .is_err()
            || contents.len() as u64 > MAX_PREFS_BYTES
        {
            tracing::warn!("ignoring oversized MegaMail appearance preferences");
            return state;
        }
        let mut values = std::collections::HashMap::new();
        for line in contents.lines().take(24) {
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim(), value.trim());
            }
        }
        if values.get("version").copied() != Some("1") {
            tracing::warn!("ignoring unsupported MegaMail appearance preferences version");
            return state;
        }
        state.preset = values
            .get("preset")
            .and_then(|value| AppearancePreset::from_key(value))
            .unwrap_or_default();
        state.dark = values
            .get("theme")
            .and_then(|value| match *value {
                "dark" => Some(true),
                "light" => Some(false),
                _ => None,
            })
            .unwrap_or(state.dark);
        state.effect = values
            .get("effect")
            .and_then(|value| WallpaperEffect::from_key(value))
            .unwrap_or_default();
        state.opacity = values
            .get("opacity")
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .map(clamp_opacity)
            .unwrap_or(state.opacity);
        state.blur_sigma = values
            .get("blur_sigma")
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .map(clamp_blur_sigma)
            .unwrap_or(state.blur_sigma);
        state.wallpaper_file = values
            .get("wallpaper_file")
            .filter(|name| safe_wallpaper_filename(name))
            .map(|name| (*name).to_owned());
        state.wallpaper_name = values
            .get("wallpaper_name")
            .and_then(|name| decode_preference_text(name))
            .map(|name| clean_display_name(&name));
        if state.wallpaper_file.is_none() {
            state.wallpaper_name = None;
            if state.preset == AppearancePreset::Custom {
                state.preset = AppearancePreset::Aurora;
            }
        }
        state
    }

    pub(crate) fn preset(&self) -> AppearancePreset {
        self.preset
    }

    pub(crate) fn dark(&self) -> bool {
        self.dark
    }

    pub(crate) fn effect(&self) -> WallpaperEffect {
        self.effect
    }

    pub(crate) fn opacity(&self) -> f32 {
        self.opacity
    }

    pub(crate) fn blur_sigma(&self) -> f32 {
        self.blur_sigma
    }

    pub(crate) fn wallpaper_file(&self) -> Option<&str> {
        self.wallpaper_file.as_deref()
    }

    pub(crate) fn wallpaper_name(&self) -> Option<&str> {
        self.wallpaper_name.as_deref()
    }

    pub(crate) fn has_wallpaper(&self) -> bool {
        self.wallpaper_file.is_some()
    }

    pub(crate) fn set_preset(&mut self, preset: AppearancePreset) {
        self.preset = if preset == AppearancePreset::Custom && !self.has_wallpaper() {
            AppearancePreset::Aurora
        } else {
            preset
        };
    }

    pub(crate) fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    pub(crate) fn set_effect(&mut self, effect: WallpaperEffect) {
        self.effect = effect;
    }

    pub(crate) fn set_opacity(&mut self, opacity: f32) {
        if opacity.is_finite() {
            self.opacity = clamp_opacity(opacity);
        }
    }

    pub(crate) fn set_blur_sigma(&mut self, sigma: f32) {
        if sigma.is_finite() {
            self.blur_sigma = clamp_blur_sigma(sigma);
        }
    }

    pub(crate) fn cached_image(&self) -> Option<Arc<RenderImage>> {
        self.image.clone()
    }

    /// Return a safe, serializable snapshot for persistence on a background
    /// executor. The selected wallpaper itself remains a separate private PNG.
    pub(crate) fn preference_snapshot(&self) -> AppearancePreferences {
        AppearancePreferences {
            dark: self.dark,
            preset: self.preset,
            effect: self.effect,
            opacity: self.opacity,
            blur_sigma: self.blur_sigma,
            wallpaper_file: self.wallpaper_file.clone(),
            wallpaper_name: self.wallpaper_name.clone(),
        }
    }

    /// Build the startup processing request if a saved wallpaper exists.
    pub(crate) fn restore_request(&self, light: bool) -> Option<WallpaperRequest> {
        if self.preset != AppearancePreset::Custom {
            return None;
        }
        let file = self.wallpaper_file.clone()?;
        if !safe_wallpaper_filename(&file) {
            return None;
        }
        Some(WallpaperRequest {
            file,
            name: self
                .wallpaper_name
                .clone()
                .unwrap_or_else(|| "Custom wallpaper".to_owned()),
            effect: self.effect,
            light,
            blur_sigma: self.blur_sigma,
        })
    }

    /// Install a newly imported wallpaper and its already-processed render
    /// image. The caller should persist `preference_snapshot()` off-thread.
    pub(crate) fn install_wallpaper(&mut self, result: WallpaperJobResult) {
        self.preset = AppearancePreset::Custom;
        self.wallpaper_file = Some(result.file.clone());
        self.wallpaper_name = Some(result.name.clone());
        self.install_render_result(result);
    }

    /// Install a reprocessed cached image only if its settings still match.
    /// This drops stale asynchronous work when the user changes effect/blur.
    pub(crate) fn install_processed_wallpaper(
        &mut self,
        result: WallpaperJobResult,
        expected_light: bool,
    ) -> bool {
        if self.preset != AppearancePreset::Custom
            || self.wallpaper_file.as_deref() != Some(result.file.as_str())
            || self.effect != result.effect
            || !same_sigma(self.blur_sigma, result.blur_sigma)
            || result.light != expected_light
        {
            return false;
        }
        self.install_render_result(result);
        true
    }

    fn install_render_result(&mut self, result: WallpaperJobResult) {
        self.image = Some(result.image);
        self.safe_opacity = result.safe_opacity;
    }

    pub(crate) fn safe_opacity(&self) -> f32 {
        self.safe_opacity
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AppearancePreferences {
    dark: bool,
    preset: AppearancePreset,
    effect: WallpaperEffect,
    opacity: f32,
    blur_sigma: f32,
    wallpaper_file: Option<String>,
    wallpaper_name: Option<String>,
}

pub(crate) fn persist_preferences(preferences: AppearancePreferences) -> Result<(), String> {
    let path =
        preference_path().ok_or_else(|| "MegaMail config directory is unavailable".to_owned())?;
    let contents = serialize_preferences(&preferences);
    megamail_core::config::write_private_file(&path, &contents)
        .map_err(|error| format!("could not save appearance preferences: {error}"))?;
    cleanup_wallpapers_except(preferences.wallpaper_file.as_deref());
    Ok(())
}

fn serialize_preferences(preferences: &AppearancePreferences) -> String {
    let file = preferences
        .wallpaper_file
        .as_deref()
        .filter(|name| safe_wallpaper_filename(name))
        .unwrap_or("");
    let name = preferences
        .wallpaper_name
        .as_deref()
        .map(encode_preference_text)
        .unwrap_or_default();
    format!(
        "version=1\ntheme={}\npreset={}\neffect={}\nopacity={:.3}\nblur_sigma={:.2}\nwallpaper_file={}\nwallpaper_name={}\n",
        if preferences.dark { "dark" } else { "light" },
        preferences.preset.as_key(),
        preferences.effect.as_key(),
        clamp_opacity(preferences.opacity),
        clamp_blur_sigma(preferences.blur_sigma),
        file,
        name,
    )
}

#[derive(Clone, Debug)]
pub(crate) struct WallpaperRequest {
    file: String,
    name: String,
    effect: WallpaperEffect,
    light: bool,
    blur_sigma: f32,
}

pub(crate) struct WallpaperJobResult {
    file: String,
    name: String,
    effect: WallpaperEffect,
    light: bool,
    blur_sigma: f32,
    safe_opacity: f32,
    image: Arc<RenderImage>,
}

/// Import a user-selected PNG, JPEG, or WebP. Call only from GPUI's background
/// executor. The selected image is downsampled and stored privately before it
/// is installed; the source path is not retained.
pub(crate) fn begin_wallpaper_import(
    source: PathBuf,
    effect: WallpaperEffect,
    light: bool,
    blur_sigma: f32,
) -> Result<WallpaperJobResult, String> {
    let directory = wallpaper_dir()?;
    let previous = saved_wallpaper_file();
    import_wallpaper_to_directory(
        &source,
        &directory,
        previous.as_deref(),
        effect,
        light,
        blur_sigma,
    )
}

fn import_wallpaper_to_directory(
    source: &Path,
    directory: &Path,
    keep_file: Option<&str>,
    effect: WallpaperEffect,
    light: bool,
    blur_sigma: f32,
) -> Result<WallpaperJobResult, String> {
    let bytes = read_limited_image(&source, MAX_SOURCE_BYTES)?;
    let format = image::guess_format(&bytes)
        .map_err(|_| "Choose a valid PNG, JPEG, or WebP image".to_owned())?;
    if !supported_format(format) || !extension_matches_format(&source, format) {
        return Err(
            "Choose a PNG, JPEG, or WebP image whose extension matches its contents".to_owned(),
        );
    }
    let decoded = decode_bounded(&bytes, format)?;
    let normalized = normalize_size(decoded);
    let normalized_png = encode_png(&normalized)?;
    if normalized_png.len() as u64 > MAX_STORED_BYTES {
        return Err("The normalized wallpaper is too large to store safely".to_owned());
    }

    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .map(|name| clean_display_name(&name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Custom wallpaper".to_owned());
    let (image, safe_opacity) = process_wallpaper(&normalized, effect, light, blur_sigma)?;
    let file = new_wallpaper_filename();
    let destination = directory.join(&file);
    cleanup_wallpapers_in(directory, keep_file);
    write_new_private_file(&destination, &normalized_png).map_err(|error| {
        format!("could not store wallpaper in MegaMail's private config directory: {error}")
    })?;
    Ok(WallpaperJobResult {
        file,
        name,
        effect,
        light,
        blur_sigma: clamp_blur_sigma(blur_sigma),
        safe_opacity,
        image,
    })
}

/// Restore or reprocess a previously normalized wallpaper. The request's
/// file is validated as a basename before it can be joined to the private
/// wallpaper directory.
pub(crate) fn begin_wallpaper_restore(
    request: WallpaperRequest,
) -> Result<WallpaperJobResult, String> {
    restore_wallpaper_from_directory(&wallpaper_dir()?, request)
}

fn restore_wallpaper_from_directory(
    directory: &Path,
    request: WallpaperRequest,
) -> Result<WallpaperJobResult, String> {
    if !safe_wallpaper_filename(&request.file) {
        return Err("The saved wallpaper reference is invalid".to_owned());
    }
    let path = directory.join(&request.file);
    let bytes = read_limited_image(&path, MAX_STORED_BYTES)?;
    if image::guess_format(&bytes).ok() != Some(ImageFormat::Png) {
        return Err("The saved wallpaper file is not a normalized PNG".to_owned());
    }
    let decoded = decode_bounded(&bytes, ImageFormat::Png)?;
    let normalized = normalize_size(decoded);
    let blur_sigma = clamp_blur_sigma(request.blur_sigma);
    let (image, safe_opacity) =
        process_wallpaper(&normalized, request.effect, request.light, blur_sigma)?;
    Ok(WallpaperJobResult {
        file: request.file,
        name: clean_display_name(&request.name),
        effect: request.effect,
        light: request.light,
        blur_sigma,
        safe_opacity,
        image,
    })
}

fn process_wallpaper(
    source: &RgbaImage,
    effect: WallpaperEffect,
    light: bool,
    blur_sigma: f32,
) -> Result<(Arc<RenderImage>, f32), String> {
    let blur_sigma = clamp_blur_sigma(blur_sigma);
    let blurred = if blur_sigma > 0.01 {
        imageops::blur(source, blur_sigma)
    } else {
        source.clone()
    };
    let effected = zeron_wallpaper::render(&blurred, effect, light);
    let safe_opacity = zeron_wallpaper::safe_opacity(
        &effected,
        if light { 0x62626A } else { 0xA9A9AE },
        if light { 0xF7F7F9 } else { 0x060606 },
        1.0,
        4.5,
        1.0,
    );
    let mut bgra = effected;
    for pixel in bgra.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    let image = Arc::new(RenderImage::new([Frame::new(bgra)]));
    Ok((image, safe_opacity))
}

fn decode_bounded(bytes: &[u8], format: ImageFormat) -> Result<RgbaImage, String> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_PIXELS as u32);
    limits.max_image_height = Some(MAX_DECODED_PIXELS as u32);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits.clone());
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "The wallpaper image could not be decoded".to_owned())?;
    let (width, height) = decoder.dimensions();
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "The wallpaper dimensions are invalid".to_owned())?;
    if width == 0 || height == 0 || pixels > MAX_DECODED_PIXELS {
        return Err("Wallpaper images must be at most 16 million pixels".to_owned());
    }
    // Dimensions are read from the encoded header before asking the decoder to
    // allocate an output frame. The strict per-image bounds plus allocation
    // budget cover codecs that honor ImageDecoder::set_limits.
    let orientation = decoder
        .orientation()
        .map_err(|_| "The wallpaper orientation metadata is invalid".to_owned())?;
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    decoder
        .set_limits(limits)
        .map_err(|_| "The wallpaper exceeds MegaMail's safe decode limits".to_owned())?;
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|_| "The wallpaper image could not be decoded within safe limits".to_owned())?;
    decoded.apply_orientation(orientation);
    Ok(decoded.into_rgba8())
}

fn normalize_size(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let scale = (MAX_RENDER_EDGE as f64 / width as f64)
        .min(MAX_RENDER_EDGE as f64 / height as f64)
        .min(1.0);
    if scale >= 1.0 {
        return image;
    }
    let out_width = ((width as f64 * scale).round() as u32).max(1);
    let out_height = ((height as f64 * scale).round() as u32).max(1);
    imageops::resize(
        &image,
        out_width,
        out_height,
        imageops::FilterType::Lanczos3,
    )
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|_| "The wallpaper could not be normalized".to_owned())?;
    Ok(bytes.into_inner())
}

fn supported_format(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    )
}

fn extension_matches_format(path: &Path, format: ImageFormat) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    match extension.to_ascii_lowercase().as_str() {
        "png" => format == ImageFormat::Png,
        "jpg" | "jpeg" => format == ImageFormat::Jpeg,
        "webp" => format == ImageFormat::WebP,
        _ => false,
    }
}

fn read_limited_image(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|_| "The wallpaper file could not be inspected".to_owned())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(format!(
            "Wallpaper files must be at most {} MiB",
            max_bytes / (1024 * 1024)
        ));
    }
    let file = File::open(path).map_err(|_| "The wallpaper file could not be opened".to_owned())?;
    let mut limited = file.take(max_bytes + 1);
    let mut bytes = Vec::with_capacity(metadata.len().min(max_bytes) as usize);
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| "The wallpaper file could not be read".to_owned())?;
    if bytes.len() as u64 > max_bytes {
        return Err("The wallpaper file grew beyond the safe import limit".to_owned());
    }
    Ok(bytes)
}

fn new_wallpaper_filename() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let serial = WALLPAPER_FILE_ID.fetch_add(1, Ordering::Relaxed);
    format!("wallpaper-{}-{nanos:x}-{serial:x}.png", std::process::id())
}

fn safe_wallpaper_filename(file: &str) -> bool {
    let Some(stem) = file.strip_suffix(".png") else {
        return false;
    };
    let Some(id) = stem.strip_prefix("wallpaper-") else {
        return false;
    };
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn clean_display_name(value: &str) -> String {
    let basename = Path::new(value)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let cleaned = basename
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect::<String>()
        .trim()
        .to_owned();
    if cleaned.is_empty() {
        "Custom wallpaper".to_owned()
    } else {
        cleaned
    }
}

fn clamp_opacity(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn clamp_blur_sigma(value: f32) -> f32 {
    value.clamp(0.0, MAX_BLUR_SIGMA)
}

fn same_sigma(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

fn preference_path() -> Option<PathBuf> {
    Some(megamail_core::config::config_base()?.join(PREFS_FILE))
}

fn saved_wallpaper_file() -> Option<String> {
    let file = File::open(preference_path()?).ok()?;
    let mut contents = String::new();
    file.take(MAX_PREFS_BYTES + 1)
        .read_to_string(&mut contents)
        .ok()?;
    if contents.len() as u64 > MAX_PREFS_BYTES {
        return None;
    }
    contents
        .lines()
        .filter_map(|line| line.strip_prefix("wallpaper_file="))
        .find(|name| safe_wallpaper_filename(name))
        .map(str::to_owned)
}

fn wallpaper_dir() -> Result<PathBuf, String> {
    let base = megamail_core::config::config_base()
        .ok_or_else(|| "MegaMail config directory is unavailable".to_owned())?;
    let directory = base.join(WALLPAPER_DIR);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create private wallpaper storage: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not secure wallpaper storage: {error}"))?;
    }
    Ok(directory)
}

/// Keep only the selected normalized copy after preferences are durably
/// updated. This bounds repeated wallpaper changes while preserving the prior
/// image if saving the new preference fails.
fn cleanup_wallpapers_except(keep: Option<&str>) {
    let Ok(directory) = wallpaper_dir() else {
        return;
    };
    cleanup_wallpapers_in(&directory, keep);
}

fn cleanup_wallpapers_in(directory: &Path, keep: Option<&str>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_file()
            && safe_wallpaper_filename(&name)
            && Some(name.as_ref()) != keep
            && let Err(error) = fs::remove_file(entry.path())
        {
            tracing::warn!("could not remove an unused private wallpaper copy: {error}");
        }
    }
}

fn write_new_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = fs::remove_file(path);
        return Err(error);
    }
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
    Ok(())
}

fn encode_preference_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'-' | b'_' | b'.') {
            output.push(byte as char);
        } else {
            output.push('%');
            output.push(hex_digit(byte >> 4));
            output.push(hex_digit(byte & 0x0f));
        }
    }
    output
}

fn decode_preference_text(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hi = *bytes.get(index + 1)?;
            let lo = *bytes.get(index + 2)?;
            decoded.push((hex_value(hi)? << 4) | hex_value(lo)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_digit(value: u8) -> char {
    b"0123456789ABCDEF"[value as usize] as char
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'A'..=b'F' => Some(value - b'A' + 10),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn wallpaper_filename_validation_rejects_paths_and_other_extensions() {
        assert!(safe_wallpaper_filename("wallpaper-123-a0-0.png"));
        assert!(!safe_wallpaper_filename("../wallpaper-123-a0-0.png"));
        assert!(!safe_wallpaper_filename("wallpaper-123-a0-0.jpg"));
        assert!(!safe_wallpaper_filename("wallpaper-../x.png"));
    }

    #[test]
    fn display_name_preferences_round_trip_without_line_injection() {
        let original = "night sky=blue\nwallpaper.png";
        let encoded = encode_preference_text(original);
        assert!(!encoded.contains('\n'));
        assert_eq!(decode_preference_text(&encoded).as_deref(), Some(original));
    }

    #[test]
    fn theme_appearance_is_part_of_the_existing_preferences_record() {
        let mut state = AppearanceState::default();
        state.set_dark(false);
        assert!(serialize_preferences(&state.preference_snapshot()).contains("theme=light\n"));
        state.set_dark(true);
        assert!(serialize_preferences(&state.preference_snapshot()).contains("theme=dark\n"));
    }

    #[test]
    fn image_size_is_reduced_without_changing_its_aspect() {
        let image = RgbaImage::new(4000, 2000);
        let scaled = normalize_size(image);
        assert_eq!(scaled.dimensions(), (2500, 1250));
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = WALLPAPER_FILE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "megamail-wallpaper-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated wallpaper test directory");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .expect("secure isolated wallpaper test directory");
            }
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn isolated_import_persist_effect_restore_and_bgra_round_trip() {
        let directory = TestDirectory::new();
        let source_image = RgbaImage::from_fn(16, 8, |x, y| {
            Rgba([(x * 11) as u8, (y * 23) as u8, 180, 255])
        });
        let source = directory.0.join("source.png");
        fs::write(&source, encode_png(&source_image).unwrap()).unwrap();

        let imported = import_wallpaper_to_directory(
            &source,
            &directory.0,
            None,
            WallpaperEffect::Scanlines,
            false,
            1.5,
        )
        .unwrap();
        assert!(safe_wallpaper_filename(&imported.file));
        let stored_path = directory.0.join(&imported.file);
        let stored = fs::metadata(&stored_path).unwrap();
        assert!(stored.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(stored.permissions().mode() & 0o777, 0o600);
        }

        let restored = restore_wallpaper_from_directory(
            &directory.0,
            WallpaperRequest {
                file: imported.file.clone(),
                name: imported.name.clone(),
                effect: imported.effect,
                light: imported.light,
                blur_sigma: imported.blur_sigma,
            },
        )
        .unwrap();
        let size = restored.image.size(0);
        assert_eq!(u32::from(size.width), 16);
        assert_eq!(u32::from(size.height), 8);
        assert_eq!(restored.image.as_bytes(0).unwrap().len(), 16 * 8 * 4);

        let solid_red = RgbaImage::from_pixel(2, 1, Rgba([220, 70, 35, 255]));
        let (render, _) = process_wallpaper(&solid_red, WallpaperEffect::None, false, 0.0).unwrap();
        assert_eq!(&render.as_bytes(0).unwrap()[..4], &[35, 70, 220, 255]);
    }

    #[test]
    fn decoder_rejects_malformed_and_oversized_png_headers_before_pixels() {
        assert!(decode_bounded(b"not a png", ImageFormat::Png).is_err());

        let tiny = encode_png(&RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]))).unwrap();
        let mut oversized = tiny;
        let width = 4_000u32;
        let height = 4_001u32;
        oversized[16..20].copy_from_slice(&width.to_be_bytes());
        oversized[20..24].copy_from_slice(&height.to_be_bytes());
        let checksum = png_crc32(&oversized[12..29]);
        oversized[29..33].copy_from_slice(&checksum.to_be_bytes());
        assert!(
            decode_bounded(&oversized, ImageFormat::Png)
                .unwrap_err()
                .contains("16 million pixels")
        );
    }

    #[test]
    fn encoded_source_byte_limit_is_checked_before_reading() {
        let directory = TestDirectory::new();
        let oversized = directory.0.join("oversized.png");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_SOURCE_BYTES + 1)
            .unwrap();
        assert!(
            read_limited_image(&oversized, MAX_SOURCE_BYTES)
                .unwrap_err()
                .contains("24 MiB")
        );
    }

    fn png_crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}
