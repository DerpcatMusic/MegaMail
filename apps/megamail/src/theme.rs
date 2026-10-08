//! MegaMail's GPUI adapter for Zeron's source-neutral theme model.
//!
//! Theme loading, importing, and persistence perform filesystem work. Call
//! those methods on a background executor; rendering only reads `ThemeState`.

use std::fs::File;
use std::io::{Read, Take};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use gpui_kit::{Hsla, rgb};
use megamail_core::config::{config_base, data_base, write_private_file};
use serde::{Deserialize, Serialize};
use zeron_theme::{
    Appearance, Color, CustomThemeLibrary, CustomThemeStatus, InstallMode, ThemeRegistry,
    ThemeVariant,
};

pub use zeron_theme::{
    AccentPreset, AccentSelection, Appearance as ThemeAppearance, SurfaceTreatment, ThemeSelection,
};

const PREFERENCES_FILE: &str = "theme.v1";
const MAX_PREFERENCES_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn appearance(self, system_dark: bool) -> Appearance {
        match self {
            Self::System if system_dark => Appearance::Dark,
            Self::System | Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
        }
    }
}

/// An app-level surface override. The upstream theme's recommended treatment
/// remains available through [`ResolvedPalette::recommended_surface`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfacePreference {
    #[default]
    ThemeDefault,
    Frosted,
    Opaque,
}

impl SurfacePreference {
    pub const ALL: [Self; 3] = [Self::ThemeDefault, Self::Frosted, Self::Opaque];

    pub fn label(self) -> &'static str {
        match self {
            Self::ThemeDefault => "Theme default",
            Self::Frosted => "Frosted",
            Self::Opaque => "Opaque",
        }
    }

    fn resolve(self, recommended: SurfaceTreatment) -> SurfaceTreatment {
        match self {
            Self::ThemeDefault => recommended,
            Self::Frosted => SurfaceTreatment::Frosted,
            Self::Opaque => SurfaceTreatment::Opaque,
        }
    }
}

pub fn theme_appearance_label(appearance: ThemeAppearance) -> &'static str {
    match appearance {
        Appearance::Light => "Light",
        Appearance::Dark => "Dark",
    }
}

pub fn surface_treatment_label(treatment: SurfaceTreatment) -> &'static str {
    match treatment {
        SurfaceTreatment::Frosted => "Frosted",
        SurfaceTreatment::Opaque => "Opaque",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemePreferences {
    pub mode: ThemeMode,
    pub selection: ThemeSelection,
    pub accent: AccentSelection,
    pub surface: SurfacePreference,
}

impl Default for ThemePreferences {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            selection: ThemeSelection::default(),
            accent: AccentSelection::ThemeDefault,
            surface: SurfacePreference::ThemeDefault,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThemeState {
    preferences: ThemePreferences,
    library: CustomThemeLibrary,
    preferences_path: PathBuf,
    data_dir: PathBuf,
    library_dirty: bool,
    registry: Arc<ThemeRegistry>,
    loaded: bool,
}

#[derive(Clone, Debug)]
pub struct ThemeChoice {
    pub id: String,
    pub family_id: String,
    pub family_name: String,
    pub name: String,
    pub background: Hsla,
    pub text: Hsla,
    pub accent: Hsla,
}

#[derive(Clone, Debug)]
pub struct ThemeLibraryEntry {
    pub id: String,
    pub name: String,
    pub variant_count: usize,
    pub source_label: String,
    pub warning: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResolvedPalette {
    pub appearance: ThemeAppearance,
    pub variant_id: String,
    pub variant_name: String,
    pub family_name: String,
    pub background: Hsla,
    pub sidebar: Hsla,
    pub list: Hsla,
    pub surface: Hsla,
    pub dialog: Hsla,
    pub overlay: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub accent: Hsla,
    pub accent_strong: Hsla,
    pub accent_wash: Hsla,
    pub on_accent: Hsla,
    pub solid: Hsla,
    pub on_solid: Hsla,
    pub selected: Hsla,
    pub hover: Hsla,
    pub danger: Hsla,
    pub danger_muted: Hsla,
    pub warning: Hsla,
    pub warning_muted: Hsla,
    pub success: Hsla,
    pub success_muted: Hsla,
    pub input: Hsla,
    pub cursor: Hsla,
    pub diff_add: Hsla,
    pub diff_delete: Hsla,
    pub diff_hunk: Hsla,
    pub recommended_surface: SurfaceTreatment,
    pub surface_treatment: SurfaceTreatment,
}

impl ThemeState {
    /// Load preferences and custom themes. This reads local files and should be
    /// called from a GPUI background task, never from `Render`.
    pub fn load() -> Result<Self, String> {
        let config_dir = config_base().ok_or_else(|| {
            "could not create MegaMail's private configuration directory".to_string()
        })?;
        let data_dir = data_base()
            .ok_or_else(|| "could not create MegaMail's private data directory".to_string())?;
        Self::load_from_dirs(config_dir, data_dir)
    }

    /// Recover with built-in preferences while retaining the stored custom
    /// theme library. This intentionally skips `theme.v1`, so it can repair a
    /// malformed preferences file. A malformed library is still reported
    /// instead of silently deleting the user's imports.
    pub fn recover_defaults() -> Result<Self, String> {
        let config_dir = config_base().ok_or_else(|| {
            "could not create MegaMail's private configuration directory".to_string()
        })?;
        let data_dir = data_base()
            .ok_or_else(|| "could not create MegaMail's private data directory".to_string())?;
        Self::recover_defaults_from_dirs(config_dir, data_dir)
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn load_from_dirs(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self, String> {
        let library = CustomThemeLibrary::load(&data_dir)
            .map_err(|error| format!("could not load custom themes: {error:#}"))?;
        let preferences_path = config_dir.join(PREFERENCES_FILE);
        let preferences = read_preferences(&preferences_path)?;
        let mut state = Self {
            preferences,
            registry: Arc::new(registry_for(&library)),
            library,
            preferences_path,
            data_dir,
            library_dirty: false,
            loaded: true,
        };
        state.repair_missing_selections();
        state.library.install_runtime();
        Ok(state)
    }

    fn recover_defaults_from_dirs(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self, String> {
        let library = CustomThemeLibrary::load(&data_dir)
            .map_err(|error| format!("could not load custom themes: {error:#}"))?;
        let mut state = Self {
            preferences: ThemePreferences::default(),
            registry: Arc::new(registry_for(&library)),
            library,
            preferences_path: config_dir.join(PREFERENCES_FILE),
            data_dir,
            library_dirty: false,
            loaded: true,
        };
        state.repair_missing_selections();
        state.library.install_runtime();
        Ok(state)
    }

    /// Persist on a background task. The library is written first when dirty,
    /// preferences second, and custom variants become active only after both
    /// writes succeed.
    pub fn persist(&mut self) -> Result<(), String> {
        if !self.loaded {
            return Err("theme preferences are not loaded yet".into());
        }
        let library_was_dirty = self.library_dirty;
        if library_was_dirty {
            self.library
                .save(&self.data_dir)
                .map_err(|error| format!("could not save custom themes: {error:#}"))?;
        }
        let encoded = serde_json::to_vec_pretty(&self.preferences)
            .map_err(|error| format!("could not encode theme preferences: {error}"))?;
        let encoded = String::from_utf8(encoded)
            .map_err(|error| format!("theme preferences were not valid UTF-8: {error}"))?;
        write_private_file(&self.preferences_path, &encoded)
            .map_err(|error| format!("could not save theme preferences: {error}"))?;
        self.library_dirty = false;
        if library_was_dirty {
            self.library.install_runtime();
        }
        Ok(())
    }

    /// Restore built-in theme preferences without discarding the loaded theme
    /// library or its persistence paths.
    pub fn reset_preferences(&mut self) {
        self.preferences = ThemePreferences::default();
    }

    pub fn preferences_snapshot(&self) -> ThemePreferences {
        self.preferences.clone()
    }

    pub fn mode(&self) -> ThemeMode {
        self.preferences.mode
    }

    pub fn selection(&self) -> &ThemeSelection {
        &self.preferences.selection
    }

    pub fn accent(&self) -> AccentSelection {
        self.preferences.accent
    }

    pub fn surface(&self) -> SurfacePreference {
        self.preferences.surface
    }

    pub fn appearance(&self, system_dark: bool) -> ThemeAppearance {
        self.preferences.mode.appearance(system_dark)
    }

    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.preferences.mode = mode;
    }

    pub fn set_variant(&mut self, appearance: ThemeAppearance, id: &str) -> bool {
        if self
            .registry
            .variant(id)
            .is_some_and(|variant| variant.appearance == appearance)
        {
            self.preferences
                .selection
                .set_variant(appearance, id.to_string());
            true
        } else {
            false
        }
    }

    pub fn set_accent(&mut self, accent: AccentSelection) {
        self.preferences.accent = accent;
    }

    pub fn set_surface(&mut self, surface: SurfacePreference) {
        self.preferences.surface = surface;
    }

    pub fn variants(&self, appearance: ThemeAppearance) -> Vec<ThemeChoice> {
        self.registry
            .families
            .iter()
            .flat_map(|family| {
                family
                    .variants
                    .iter()
                    .filter(move |variant| variant.appearance == appearance)
                    .map(move |variant| (family, variant))
            })
            .map(|(family, variant)| {
                let accent = variant.accent_for(self.preferences.accent);
                ThemeChoice {
                    id: variant.id.clone(),
                    family_id: family.id.clone(),
                    family_name: family.name.clone(),
                    name: variant.name.clone(),
                    background: to_hsla(variant.colors.background),
                    text: to_hsla(variant.colors.text),
                    accent: to_hsla(accent.primary),
                }
            })
            .collect()
    }

    pub fn resolved(&self, system_dark: bool) -> ResolvedPalette {
        let appearance = self.appearance(system_dark);
        let variant = self
            .registry
            .resolve(&self.preferences.selection, appearance);
        let family_name = self
            .registry
            .families
            .iter()
            .find(|family| family.id == variant.family_id)
            .map(|family| family.name.clone())
            .unwrap_or_else(|| variant.family_id.clone());
        resolve_palette(
            variant,
            family_name,
            self.preferences.accent,
            self.preferences.surface,
        )
    }

    /// Compile and snapshot-import a VS Code theme file or extension package.
    /// This parses source files and must run off the UI thread. Call `persist`
    /// on the returned state before publishing it to the visible app state.
    pub fn import_file(&mut self, path: &Path) -> Result<usize, String> {
        if !self.loaded {
            return Err("theme preferences are not loaded yet".into());
        }
        let display_name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or("Imported theme");
        let family_id = format!("imported-{}", slug(display_name));
        let compilation = CustomThemeLibrary::compile(path, &family_id, display_name)
            .map_err(|error| format!("could not import theme: {error:#}"))?;
        let variant_count = compilation.family.variants.len();
        self.library
            .install(compilation, &[], InstallMode::Snapshot)
            .map_err(|error| format!("could not install imported theme: {error:#}"))?;
        self.library_dirty = true;
        self.rebuild_registry();
        Ok(variant_count)
    }

    pub fn library_entries(&self) -> Vec<ThemeLibraryEntry> {
        self.library
            .entries
            .iter()
            .map(|entry| ThemeLibraryEntry {
                id: entry.id.clone(),
                name: entry.name.clone(),
                variant_count: entry.family.variants.len(),
                source_label: entry.source.label().to_string(),
                warning: match &entry.status {
                    CustomThemeStatus::Ready => None,
                    CustomThemeStatus::Warning { message } => Some(message.clone()),
                },
            })
            .collect()
    }

    pub fn remove_library_entry(&mut self, id: &str) -> bool {
        if !self.loaded {
            return false;
        }
        let Some(entry) = self.library.entry(id) else {
            return false;
        };
        let removed_variant_ids = entry
            .family
            .variants
            .iter()
            .map(|variant| variant.id.clone())
            .collect::<Vec<_>>();
        if !self.library.remove(id) {
            return false;
        }
        for appearance in Appearance::ALL {
            if removed_variant_ids
                .iter()
                .any(|removed| removed == self.preferences.selection.variant_id(appearance))
            {
                self.preferences.selection.set_variant(
                    appearance,
                    if appearance.is_dark() {
                        "zeron-dark"
                    } else {
                        "zeron-light"
                    },
                );
            }
        }
        self.library_dirty = true;
        self.rebuild_registry();
        true
    }

    fn rebuild_registry(&mut self) {
        self.registry = Arc::new(registry_for(&self.library));
    }

    fn repair_missing_selections(&mut self) {
        let registry = Arc::clone(&self.registry);
        for appearance in Appearance::ALL {
            let selected = self.preferences.selection.variant_id(appearance);
            if !registry
                .variant(selected)
                .is_some_and(|variant| variant.appearance == appearance)
            {
                self.preferences.selection.set_variant(
                    appearance,
                    if appearance.is_dark() {
                        "zeron-dark"
                    } else {
                        "zeron-light"
                    },
                );
            }
        }
    }
}

impl Default for ThemeState {
    /// Build an in-memory default state without reading or creating app files.
    /// Persisting this placeholder is blocked until [`ThemeState::load`] has
    /// installed the real MegaMail paths and loaded its custom library.
    fn default() -> Self {
        Self {
            preferences: ThemePreferences::default(),
            library: CustomThemeLibrary::default(),
            preferences_path: PathBuf::new(),
            data_dir: PathBuf::new(),
            library_dirty: false,
            registry: builtin_registry_arc(),
            loaded: false,
        }
    }
}

fn builtin_registry_arc() -> Arc<ThemeRegistry> {
    static REGISTRY: OnceLock<Arc<ThemeRegistry>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| Arc::new(ThemeRegistry::builtin().clone()))
        .clone()
}

fn registry_for(library: &CustomThemeLibrary) -> ThemeRegistry {
    let mut families = ThemeRegistry::builtin().families.clone();
    families.extend(library.entries.iter().map(|entry| entry.family.clone()));
    ThemeRegistry { families }
}

fn read_preferences(path: &Path) -> Result<ThemePreferences, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ThemePreferences::default());
        }
        Err(error) => return Err(format!("could not open theme preferences: {error}")),
    };
    let mut bytes = Vec::new();
    take_bounded(file, MAX_PREFERENCES_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read theme preferences: {error}"))?;
    if bytes.len() as u64 > MAX_PREFERENCES_BYTES {
        return Err("theme preferences exceed the 64 KiB limit".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("could not parse theme preferences: {error}"))
}

fn take_bounded(file: File, limit: u64) -> Take<File> {
    file.take(limit + 1)
}

fn resolve_palette(
    variant: &ThemeVariant,
    family_name: String,
    accent_selection: AccentSelection,
    surface_preference: SurfacePreference,
) -> ResolvedPalette {
    let colors = &variant.colors;
    let accent = variant.accent_for(accent_selection);
    ResolvedPalette {
        appearance: variant.appearance,
        variant_id: variant.id.clone(),
        variant_name: variant.name.clone(),
        family_name,
        background: to_hsla(colors.background),
        sidebar: to_hsla(colors.shell),
        list: to_hsla(colors.card),
        surface: to_hsla(colors.raised),
        dialog: to_hsla(colors.dialog),
        overlay: to_hsla(colors.overlay),
        border: to_hsla(colors.border),
        border_strong: to_hsla(colors.border_strong),
        text: to_hsla(colors.text),
        muted: to_hsla(colors.text_muted),
        faint: to_hsla(colors.text_faint),
        accent: to_hsla(accent.primary),
        accent_strong: to_hsla(accent.strong),
        accent_wash: to_hsla(accent.wash),
        on_accent: to_hsla(accent.on),
        solid: to_hsla(colors.solid),
        on_solid: to_hsla(colors.on_solid),
        selected: to_hsla(colors.active),
        hover: to_hsla(colors.hover),
        danger: to_hsla(colors.danger),
        danger_muted: to_hsla(colors.danger_muted),
        warning: to_hsla(colors.warning),
        warning_muted: to_hsla(colors.warning_muted),
        success: to_hsla(colors.success),
        success_muted: to_hsla(colors.success_muted),
        input: to_hsla(colors.input),
        cursor: to_hsla(colors.cursor),
        diff_add: to_hsla(colors.diff_add),
        diff_delete: to_hsla(colors.diff_delete),
        diff_hunk: to_hsla(colors.diff_hunk),
        recommended_surface: variant.recommended_surface_treatment,
        surface_treatment: surface_preference.resolve(variant.recommended_surface_treatment),
    }
}

fn to_hsla(color: Color) -> Hsla {
    let red = u32::from(color.r) << 16;
    let green = u32::from(color.g) << 8;
    let mut converted: Hsla = rgb(red | green | u32::from(color.b)).into();
    converted.a = f32::from(color.a) / 255.0;
    converted
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !output.is_empty() {
                output.push('-');
            }
            output.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    if output.is_empty() {
        "theme".into()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempThemeDirs(PathBuf);

    impl Drop for TempThemeDirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_theme_dirs() -> (TempThemeDirs, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "megamail-theme-state-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let config = root.join("config");
        let data = root.join("data");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        (TempThemeDirs(root), config, data)
    }

    fn empty_state() -> ThemeState {
        ThemeState::default()
    }

    #[test]
    fn system_mode_selects_each_independent_variant() {
        let mut state = empty_state();
        assert!(state.set_variant(Appearance::Light, "github-light"));
        assert!(state.set_variant(Appearance::Dark, "dracula"));
        assert_eq!(state.resolved(false).variant_id, "github-light");
        assert_eq!(state.resolved(true).variant_id, "dracula");
    }

    #[test]
    fn variant_choices_reject_the_wrong_appearance() {
        let mut state = empty_state();
        assert!(!state.set_variant(Appearance::Light, "dracula"));
        assert!(!state.set_variant(Appearance::Dark, "github-light"));
    }

    #[test]
    fn converted_theme_colors_keep_source_alpha() {
        assert!((to_hsla(Color::rgba(0x40, 0x60, 0x80, 0x80)).a - (128.0 / 255.0)).abs() < 0.001);
    }

    #[test]
    fn default_is_pure_memory_and_cannot_persist_before_load() {
        let mut state = ThemeState::default();
        assert!(!state.loaded);
        assert!(state.persist().is_err());
        assert!(state.preferences_path.as_os_str().is_empty());
        assert!(state.data_dir.as_os_str().is_empty());
    }

    #[test]
    fn reset_preserves_loaded_paths_and_custom_library() {
        let mut state = ThemeState::default();
        state.loaded = true;
        state.preferences_path = PathBuf::from("private/config/theme.v1");
        state.data_dir = PathBuf::from("private/data");
        let library_before = state.library.clone();
        let config_path = state.preferences_path.clone();
        let data_dir = state.data_dir.clone();
        state.set_mode(ThemeMode::Dark);

        state.reset_preferences();

        assert_eq!(state.mode(), ThemeMode::System);
        assert_eq!(state.preferences_path, config_path);
        assert_eq!(state.data_dir, data_dir);
        assert_eq!(state.library, library_before);
    }

    #[test]
    fn resolving_and_listing_themes_reuse_the_cached_registry() {
        let state = ThemeState::default();
        let registry = Arc::clone(&state.registry);
        let _ = state.resolved(false);
        let _ = state.variants(Appearance::Dark);
        assert!(Arc::ptr_eq(&registry, &state.registry));
    }

    #[test]
    fn recovery_skips_corrupt_preferences_but_keeps_real_paths() {
        let (_temp, config, data) = temp_theme_dirs();
        let prefs_path = config.join(PREFERENCES_FILE);
        std::fs::write(&prefs_path, b"{broken").unwrap();
        assert!(ThemeState::load_from_dirs(config.clone(), data.clone()).is_err());

        let state = ThemeState::recover_defaults_from_dirs(config.clone(), data.clone()).unwrap();

        assert!(state.is_loaded());
        assert_eq!(state.mode(), ThemeMode::System);
        assert_eq!(state.preferences_path, prefs_path);
        assert_eq!(state.data_dir, data);
    }

    #[test]
    fn recovery_reports_a_corrupt_custom_library_without_replacing_it() {
        let (_temp, config, data) = temp_theme_dirs();
        let library_path = CustomThemeLibrary::path(&data);
        std::fs::write(&library_path, b"{broken-library").unwrap();

        assert!(ThemeState::recover_defaults_from_dirs(config, data).is_err());
        assert_eq!(std::fs::read(library_path).unwrap(), b"{broken-library");
    }
}
