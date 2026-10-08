use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::{Disableable as _, Selectable as _};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Decorations, Entity, FocusHandle,
    FontWeight, InteractiveElement as _, IntoElement, KeyBinding, MouseButton, ParentElement as _,
    PathPromptOptions, Render, ScrollStrategy, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, UniformListScrollHandle, Window, WindowBounds, WindowDecorations,
    WindowOptions, div, prelude::FluentBuilder as _, px, relative, rgb, size, uniform_list,
};
use gpui_kit::{
    assets::IconName,
    component::{
        ActiveTheme as _, Icon, Sizable as _, Theme, ThemeMode, TitleBar,
        button::{Button, ButtonCustomVariant, ButtonVariants as _},
        h_flex,
        input::{Input, InputContentType, InputEvent, InputState, Textarea, TextareaState},
        menu::{DropdownMenu as _, PopupMenuItem},
        v_flex, window_border,
    },
};
use megamail_core::thunderbird::ThunderbirdProfile;
use megamail_core::{
    config::{self, AccountConfig},
    discovery::{self, CandidateWarning, DiscoveryReport, DiscoverySource, MailServer, TlsMode},
    models::{Folder, FolderKind, Message},
    worker::{self, OutgoingMessage, SendOutcome},
};

use crate::live::{
    AttachmentState, DraftSourceState, LiveMailbox, MailboxSnapshot, MessageFilter, MessageKey,
    MessageRow, ThunderbirdAccountOutcome, ThunderbirdAccountState,
};
use crate::onboarding::AccountForm;

mod action {
    gpui_kit::actions!(megamail, [NextMessage, PreviousMessage]);
}
mod appearance;
mod appearance_view;
mod compose;
mod instance;
mod live;
mod onboarding;
mod theme;
mod thunderbird_adapter;
mod zeron_background;
mod zeron_style;

use action::{NextMessage, PreviousMessage};

fn packed_rgb(color: gpui_kit::Hsla) -> u32 {
    let color = color.to_rgb();
    ((color.r * 255.0).round() as u32) << 16
        | ((color.g * 255.0).round() as u32) << 8
        | (color.b * 255.0).round() as u32
}

fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", NextMessage, Some("MegaMailMessages")),
        KeyBinding::new("up", PreviousMessage, Some("MegaMailMessages")),
    ]);
}

#[derive(Clone, Copy)]
struct Palette {
    background: gpui_kit::Hsla,
    sidebar: gpui_kit::Hsla,
    list: gpui_kit::Hsla,
    surface: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    text: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
    faint: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
    accent_wash: gpui_kit::Hsla,
    selected: gpui_kit::Hsla,
    hover: gpui_kit::Hsla,
    on_accent: gpui_kit::Hsla,
}

impl Palette {
    fn from_resolved(value: theme::ResolvedPalette) -> Self {
        Self {
            background: value.background,
            sidebar: value.sidebar,
            list: value.list,
            surface: value.surface,
            border: value.border,
            text: value.text,
            muted: value.muted,
            faint: value.faint,
            accent: value.accent,
            accent_wash: value.accent_wash,
            selected: value.selected,
            hover: value.hover,
            on_accent: value.on_accent,
        }
    }

    fn new(dark: bool) -> Self {
        let color = |hex: u32| -> gpui_kit::Hsla { rgb(hex).into() };
        if dark {
            let accent = color(0x7c86ff);
            Self {
                background: color(0x060606),
                sidebar: color(0x0d0d0d),
                list: color(0x090909),
                surface: color(0x0e0e0e),
                border: color(0x202020),
                text: color(0xe8e8ea),
                muted: color(0xa9a9ae),
                faint: color(0x85858a),
                accent,
                accent_wash: accent.opacity(0.14),
                selected: color(0x2e2e30),
                hover: color(0x252527),
                on_accent: color(0x101014),
            }
        } else {
            let accent = color(0x5b43e8);
            Self {
                background: color(0xffffff),
                sidebar: color(0xf7f7f9),
                list: color(0xfcfcfd),
                surface: color(0xffffff),
                border: color(0xe5e5e9),
                text: color(0x303035),
                muted: color(0x62626a),
                faint: color(0x73737a),
                accent,
                accent_wash: accent.opacity(0.10),
                selected: color(0xe8e8eb),
                hover: color(0xefeff2),
                on_accent: color(0xffffff),
            }
        }
    }
}

fn sidebar_button_variant(
    cx: &App,
    palette: Palette,
    _dark: bool,
    selected: bool,
) -> ButtonCustomVariant {
    let hover = palette.hover;
    ButtonCustomVariant::new(cx)
        .color(if selected {
            palette.selected
        } else {
            cx.theme().transparent
        })
        .foreground(palette.text)
        .hover(hover)
        .active(palette.selected)
}

fn floating_button_variant(cx: &App, palette: Palette, _dark: bool) -> ButtonCustomVariant {
    let hover = palette.hover;
    ButtonCustomVariant::new(cx)
        .color(cx.theme().transparent)
        .foreground(palette.text)
        .hover(hover)
        .active(hover)
}

fn apply_palette(mode: ThemeMode, palette: Palette, window: &mut Window, cx: &mut App) {
    Theme::change(mode, Some(window), cx);
    Theme::update(cx, |theme| {
        theme.background = palette.background;
        theme.foreground = palette.text;
        theme.muted_foreground = palette.muted;
        theme.border = palette.border;
        theme.input = palette.border;
        theme.accent = palette.selected;
        theme.accent_foreground = palette.text;
        theme.primary = palette.accent;
        theme.primary_foreground = palette.on_accent;
        theme.primary_hover = palette.accent.opacity(0.88);
        theme.primary_active = palette.accent.opacity(0.76);
        theme.secondary = palette.surface;
        theme.secondary_foreground = palette.text;
        theme.secondary_hover = palette.hover;
        theme.secondary_active = palette.selected;
        theme.button = palette.surface;
        theme.button_foreground = palette.text;
        theme.button_hover = palette.hover;
        theme.button_active = palette.selected;
        theme.button_primary = palette.accent;
        theme.button_primary_foreground = palette.on_accent;
        theme.button_primary_hover = palette.accent.opacity(0.88);
        theme.button_primary_active = palette.accent.opacity(0.76);
        theme.sidebar = palette.sidebar;
        theme.sidebar_foreground = palette.text;
        theme.sidebar_accent = palette.selected;
        theme.sidebar_accent_foreground = palette.text;
        theme.sidebar_primary = palette.accent;
        theme.sidebar_primary_foreground = palette.on_accent;
        theme.sidebar_border = palette.border;
        theme.colors.list = palette.list;
        theme.list_active = palette.selected;
        theme.list_active_border = palette.border;
        theme.list_hover = palette.hover;
        theme.list_head = palette.surface;
        theme.popover = palette.surface;
        theme.popover_foreground = palette.text;
        theme.ring = palette.accent;
        theme.selection = palette.accent.opacity(0.28);
        theme.caret = palette.accent;
        theme.radius = px(6.);
        theme.radius_lg = px(10.);
        theme.font_family = "Geist".into();
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Mailbox,
    Setup,
    Compose,
    Appearance,
}

#[derive(Clone, Copy)]
enum AppearanceChange {
    Preset(appearance::AppearancePreset),
    Effect(appearance::WallpaperEffect),
    Opacity(f32),
    Blur(f32),
    EffectStrength(f32),
    Fade(f32),
}

#[derive(Clone)]
enum ThemeChange {
    Mode(theme::ThemeMode),
    Variant(zeron_theme::Appearance, String),
    Accent(zeron_theme::AccentSelection),
    Surface(theme::SurfacePreference),
    Remove(String),
}

struct SetupInputs {
    email: Entity<InputState>,
    name: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    smtp_username: Entity<InputState>,
    smtp_password: Entity<InputState>,
    imap_host: Entity<InputState>,
    imap_port: Entity<InputState>,
    smtp_host: Entity<InputState>,
    smtp_port: Entity<InputState>,
}

struct SetupState {
    inputs: SetupInputs,
    report: Option<DiscoveryReport>,
    discovery_error: Option<String>,
    selected_candidate: usize,
    discovering: bool,
    connecting: bool,
    manual: bool,
    warning_reviewed: bool,
    reviewed_settings: Option<String>,
    imap_tls: TlsMode,
    smtp_tls: TlsMode,
    smtp_auth: megamail_core::discovery::AuthMethod,
    error: Option<String>,
    smtp_credentials_expanded: bool,
    thunderbird_profiles: Vec<ThunderbirdProfile>,
    thunderbird_loading: bool,
    thunderbird_error: Option<String>,
    selected_thunderbird: HashSet<(PathBuf, String)>,
    observed_email: String,
}

impl SetupState {
    fn new(window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        Self {
            inputs: SetupInputs {
                email: new_input(window, cx, "you@example.com"),
                name: new_input(window, cx, "Name shown to recipients"),
                username: new_input(window, cx, "Usually your email address"),
                password: cx
                    .new(|cx| InputState::new(window, cx).placeholder("Password or app password")),
                smtp_username: new_input(window, cx, "Same as IMAP username"),
                smtp_password: cx
                    .new(|cx| InputState::new(window, cx).placeholder("Separate SMTP password")),
                imap_host: new_input(window, cx, "imap.example.com"),
                imap_port: new_input(window, cx, "993"),
                smtp_host: new_input(window, cx, "smtp.example.com"),
                smtp_port: new_input(window, cx, "587"),
            },
            report: None,
            discovery_error: None,
            selected_candidate: 0,
            discovering: false,
            connecting: false,
            manual: false,
            warning_reviewed: false,
            reviewed_settings: None,
            imap_tls: TlsMode::ImplicitTls,
            smtp_tls: TlsMode::StartTls,
            smtp_auth: megamail_core::discovery::AuthMethod::Password,
            error: None,
            smtp_credentials_expanded: false,
            thunderbird_profiles: Vec::new(),
            thunderbird_loading: false,
            thunderbird_error: None,
            selected_thunderbird: HashSet::new(),
            observed_email: String::new(),
        }
    }

    fn candidate(&self) -> Option<&megamail_core::discovery::ServerConfigCandidate> {
        self.report
            .as_ref()?
            .candidates
            .get(self.selected_candidate)
    }

    fn settings_fingerprint(&self, cx: &App) -> String {
        let candidate = self.candidate();
        let value = |input: &Entity<InputState>, fallback: &str| {
            let entered = input.read(cx).value().to_string();
            if entered.trim().is_empty() {
                fallback.to_owned()
            } else {
                entered
            }
        };
        let email = self.inputs.email.read(cx).value().to_string();
        let default_username = candidate
            .map(|candidate| candidate.incoming.username.default_for(&email))
            .unwrap_or_else(|| email.clone());
        let default_imap_host = candidate.map_or_else(String::new, |c| c.incoming.host.clone());
        let default_imap_port =
            candidate.map_or_else(|| "993".to_owned(), |c| c.incoming.port.to_string());
        let default_smtp_host = candidate.map_or_else(String::new, |c| c.outgoing.host.clone());
        let default_smtp_port =
            candidate.map_or_else(|| "587".to_owned(), |c| c.outgoing.port.to_string());
        let username = value(&self.inputs.username, &default_username);
        let imap_host = value(&self.inputs.imap_host, &default_imap_host);
        let imap_port = value(&self.inputs.imap_port, &default_imap_port);
        let smtp_host = value(&self.inputs.smtp_host, &default_smtp_host);
        let smtp_port = value(&self.inputs.smtp_port, &default_smtp_port);
        let smtp_username = self.inputs.smtp_username.read(cx).value().to_string();
        format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{:?}\0{}\0{}\0{:?}\0{}\0{}\0{:?}",
            email,
            self.selected_candidate,
            self.manual,
            username,
            imap_host,
            imap_port,
            self.imap_tls,
            smtp_host,
            smtp_port,
            self.smtp_tls,
            candidate.map_or("", |c| c.provider_name.as_str()),
            smtp_username,
            self.smtp_auth,
        )
    }

    fn review_is_current(&self, cx: &App) -> bool {
        self.warning_reviewed
            && self.reviewed_settings.as_deref() == Some(self.settings_fingerprint(cx).as_str())
    }
}

struct Composer {
    message: OutgoingMessage,
    to: Entity<InputState>,
    cc: Entity<InputState>,
    bcc: Entity<InputState>,
    subject: Entity<InputState>,
    body: Entity<TextareaState>,
    error: Option<String>,
    send_request_id: Option<u64>,
    send_uncertain: bool,
    send_queued: bool,
    draft_pending: Option<OutgoingMessage>,
    draft_saved: Option<OutgoingMessage>,
    selecting_attachments: bool,
    attachment_error: Option<String>,
    notice: Option<String>,
    temporary_attachment_paths: Vec<PathBuf>,
}

#[derive(Clone)]
struct SenderChoice {
    alias: Option<String>,
    label: String,
}

impl Composer {
    fn new(message: OutgoingMessage, window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        Self {
            to: new_input_value(window, cx, "recipient@example.com", message.to.clone()),
            cc: new_input_value(window, cx, "Copy to", message.cc.clone()),
            bcc: new_input_value(window, cx, "Blind copy to", message.bcc.clone()),
            subject: new_input_value(window, cx, "Subject", message.subject.clone()),
            body: cx.new(|cx| TextareaState::new(window, cx).default_value(message.body.clone())),
            message,
            error: None,
            send_request_id: None,
            send_uncertain: false,
            send_queued: false,
            draft_pending: None,
            draft_saved: None,
            selecting_attachments: false,
            attachment_error: None,
            notice: None,
            temporary_attachment_paths: Vec::new(),
        }
    }

    fn new_prepared_draft(
        message: OutgoingMessage,
        temporary_attachment_paths: Vec<PathBuf>,
        notice: Option<String>,
        window: &mut Window,
        cx: &mut Context<MailApp>,
    ) -> Self {
        let mut composer = Self::new(message, window, cx);
        composer.temporary_attachment_paths = temporary_attachment_paths;
        composer.notice = notice;
        composer
    }

    fn outgoing(&self, cx: &App) -> OutgoingMessage {
        let mut message = self.message.clone();
        message.to = self.to.read(cx).value().to_string();
        message.cc = self.cc.read(cx).value().to_string();
        message.bcc = self.bcc.read(cx).value().to_string();
        message.subject = self.subject.read(cx).value().to_string();
        message.body = self.body.read(cx).value().to_string();
        message.html.clear();
        message
    }

    fn is_saved(&self, cx: &App) -> bool {
        let Some(saved) = &self.draft_saved else {
            return false;
        };
        let current = self.outgoing(cx);
        current.from_account_id == saved.from_account_id
            && current.to == saved.to
            && current.cc == saved.cc
            && current.bcc == saved.bcc
            && current.subject == saved.subject
            && current.body == saved.body
            && current.attachments == saved.attachments
    }

    fn operation_pending(&self) -> bool {
        composer_operation_pending(
            self.selecting_attachments,
            self.send_request_id.is_some(),
            self.draft_pending.is_some(),
            self.send_uncertain,
        )
    }

    fn can_edit(&self) -> bool {
        composer_can_edit(
            self.selecting_attachments,
            self.send_request_id.is_some(),
            self.draft_pending.is_some(),
            self.send_queued,
            self.send_uncertain,
        )
    }

    fn can_close(&self) -> bool {
        composer_can_close(
            self.selecting_attachments,
            self.send_request_id.is_some(),
            self.draft_pending.is_some(),
            self.send_uncertain,
        )
    }
}

fn composer_operation_pending(
    selecting_attachments: bool,
    sending: bool,
    saving_draft: bool,
    send_uncertain: bool,
) -> bool {
    selecting_attachments || sending || saving_draft || send_uncertain
}

fn composer_can_edit(
    selecting_attachments: bool,
    sending: bool,
    saving_draft: bool,
    queued: bool,
    send_uncertain: bool,
) -> bool {
    !composer_operation_pending(selecting_attachments, sending, saving_draft, send_uncertain)
        && !queued
}

fn composer_can_close(
    selecting_attachments: bool,
    sending: bool,
    saving_draft: bool,
    send_uncertain: bool,
) -> bool {
    !composer_operation_pending(selecting_attachments, sending, saving_draft, send_uncertain)
}

struct MailApp {
    live: Entity<LiveMailbox>,
    live_subscription: Option<Subscription>,
    profiles: Vec<(u32, AccountConfig)>,
    search: Entity<InputState>,
    message_focus: FocusHandle,
    list_scroll: UniformListScrollHandle,
    setup: Option<SetupState>,
    composer: Option<Composer>,
    view: View,
    setup_return_view: View,
    appearance_return_view: View,
    sidebar_collapsed: bool,
    accounts_expanded: bool,
    other_folders_expanded: bool,
    demo: bool,
    thunderbird_auto_open_pending: bool,
    thunderbird_auto_open_seen_loading: bool,
    loading_profiles: bool,
    startup_error: Option<String>,
    attachment_save_pending: Option<String>,
    attachment_save_error: Option<String>,
    attachment_save_status: Option<String>,
    draft_open_error: Option<String>,
    dark: bool,
    system_dark: bool,
    theme: theme::ThemeState,
    theme_busy: bool,
    theme_error: Option<String>,
    theme_status: Option<String>,
    palette: Palette,
    surface_treatment: zeron_theme::SurfaceTreatment,
    _theme_subscription: Subscription,
    wallpaper_animating: bool,
    appearance: appearance::AppearanceState,
    appearance_loading: bool,
    appearance_busy: bool,
    appearance_error: Option<String>,
    _search_subscription: Subscription,
    setup_email_subscription: Option<Subscription>,
}

impl MailApp {
    fn new(window: &mut Window, cx: &mut Context<Self>, demo: bool) -> Self {
        window.set_window_title(if demo {
            "MegaMail · Local demo"
        } else {
            "MegaMail"
        });
        apply_palette(ThemeMode::Dark, Palette::new(true), window, cx);

        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search loaded mail"));
        let live = cx.new(|cx| {
            if demo {
                LiveMailbox::new_demo(cx)
            } else {
                LiveMailbox::new(Vec::new(), None, cx)
            }
        });
        let live_subscription = cx.observe(&live, |this, _, cx| this.on_live_change(cx));
        let message_focus = cx.focus_handle().tab_stop(true);
        let search_subscription = cx.subscribe_in(&search, window, {
            let search = search.clone();
            move |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = search.read(cx).value().to_string();
                    this.live.update(cx, |live, cx| live.set_search(query, cx));
                    this.list_scroll = UniformListScrollHandle::new();
                    cx.notify();
                }
            }
        });

        let theme_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            this.system_dark = matches!(
                window.appearance(),
                gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
            );
            if this.theme.mode() == theme::ThemeMode::System {
                this.activate_theme(window, cx);
            }
        });
        let system_dark = matches!(
            window.appearance(),
            gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
        );
        let mut app = Self {
            live,
            live_subscription: Some(live_subscription),
            profiles: Vec::new(),
            search,
            message_focus,
            list_scroll: UniformListScrollHandle::new(),
            setup: (!demo).then(|| SetupState::new(window, cx)),
            composer: None,
            view: if demo { View::Mailbox } else { View::Setup },
            setup_return_view: View::Mailbox,
            appearance_return_view: View::Mailbox,
            sidebar_collapsed: false,
            accounts_expanded: false,
            other_folders_expanded: false,
            demo,
            thunderbird_auto_open_pending: false,
            thunderbird_auto_open_seen_loading: false,
            loading_profiles: !demo,
            startup_error: None,
            attachment_save_pending: None,
            attachment_save_error: None,
            attachment_save_status: None,
            draft_open_error: None,
            dark: true,
            system_dark,
            theme: theme::ThemeState::default(),
            theme_busy: false,
            theme_error: None,
            theme_status: None,
            palette: Palette::new(true),
            surface_treatment: zeron_theme::SurfaceTreatment::Frosted,
            _theme_subscription: theme_subscription,
            wallpaper_animating: false,
            appearance: appearance::AppearanceState::default(),
            appearance_loading: true,
            appearance_busy: false,
            appearance_error: None,
            _search_subscription: search_subscription,
            setup_email_subscription: None,
        };

        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_spawn(async move {
                    (
                        appearance::AppearanceState::load(),
                        theme::ThemeState::load(),
                    )
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.appearance = loaded.0;
                match loaded.1 {
                    Ok(theme) => this.theme = theme,
                    Err(error) => this.theme_error = Some(error),
                }
                this.dark =
                    this.theme.appearance(this.system_dark) == zeron_theme::Appearance::Dark;
                let palette = Palette::from_resolved(this.theme.resolved(this.system_dark));
                apply_palette(
                    if this.dark {
                        ThemeMode::Dark
                    } else {
                        ThemeMode::Light
                    },
                    palette,
                    window,
                    cx,
                );
                this.appearance_loading = false;
                this.activate_theme(window, cx);
                cx.notify();
            });
        })
        .detach();

        if !demo {
            app.watch_setup_email(window, cx);
            cx.spawn(async move |this, cx| {
                let loaded = cx
                    .background_spawn(async move {
                        compose::prune_stale_draft_attachment_dirs();
                        config::load_profiles()
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.loading_profiles = false;
                    match loaded {
                        Ok(profiles) if !profiles.is_empty() => {
                            let selected = profiles.first().map(|(id, _)| *id);
                            this.profiles = profiles.clone();
                            this.replace_live(profiles, selected, cx);
                            this.live.update(cx, |live, cx| {
                                live.restore_thunderbird(cx);
                            });
                            this.setup = None;
                            this.setup_email_subscription = None;
                            this.view = View::Mailbox;
                        }
                        Ok(_) => {
                            this.thunderbird_auto_open_pending = this
                                .live
                                .update(cx, |live, cx| live.restore_thunderbird(cx));
                            this.thunderbird_auto_open_seen_loading =
                                this.thunderbird_auto_open_pending;
                            if this.thunderbird_auto_open_pending {
                                this.setup_return_view = View::Mailbox;
                            }
                            this.view = View::Setup;
                        }
                        Err(error) => {
                            this.startup_error =
                                Some(format!("Could not load MegaMail accounts: {error}"));
                            this.view = View::Setup;
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }

        app
    }

    fn watch_setup_email(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(email_input) = self.setup.as_ref().map(|setup| setup.inputs.email.clone()) else {
            self.setup_email_subscription = None;
            return;
        };
        self.setup_email_subscription = Some(cx.subscribe_in(&email_input, window, {
            let email_input = email_input.clone();
            move |this, _, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let email = email_input.read(cx).value().trim().to_owned();
                if let Some(setup) = this.setup.as_mut() {
                    let changed = !setup.observed_email.is_empty()
                        && !setup.observed_email.eq_ignore_ascii_case(&email);
                    setup.observed_email = email.clone();
                    if changed {
                        setup.report = None;
                        setup.selected_candidate = 0;
                        setup.discovery_error = None;
                        setup.manual = false;
                        setup.warning_reviewed = false;
                        setup.reviewed_settings = None;
                        setup.smtp_credentials_expanded = false;
                        setup
                            .inputs
                            .smtp_username
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        setup
                            .inputs
                            .smtp_password
                            .update(cx, |input, cx| input.set_value("", window, cx));
                    }
                }
                cx.notify();
            }
        }));
    }

    fn replace_live(
        &mut self,
        profiles: Vec<(u32, AccountConfig)>,
        selected: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        let live = cx.new(|cx| LiveMailbox::new(profiles.clone(), selected, cx));
        self.live_subscription = Some(cx.observe(&live, |this, _, cx| this.on_live_change(cx)));
        self.live = live;
        let query = self.search.read(cx).value().to_string();
        if !query.is_empty() {
            self.live.update(cx, |live, cx| live.set_search(query, cx));
        }
        self.list_scroll = UniformListScrollHandle::new();
    }

    fn on_live_change(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        if let Some(setup) = self.setup.as_mut() {
            for outcome in &snapshot.thunderbird_results {
                if outcome.state == ThunderbirdAccountState::Connected {
                    setup.selected_thunderbird.remove(&(
                        outcome.profile_path.clone(),
                        outcome.source_account_id.clone(),
                    ));
                }
            }
        }
        if self.thunderbird_auto_open_pending {
            self.thunderbird_auto_open_seen_loading |= snapshot.thunderbird_loading;
            if self.thunderbird_auto_open_seen_loading && !snapshot.thunderbird_loading {
                if snapshot.thunderbird_results.is_empty()
                    || snapshot
                        .thunderbird_results
                        .iter()
                        .any(|result| result.state != ThunderbirdAccountState::Connected)
                {
                    self.thunderbird_auto_open_pending = false;
                    self.thunderbird_auto_open_seen_loading = false;
                } else if !snapshot.accounts.is_empty()
                    && (!snapshot.folders.is_empty() || !snapshot.page.is_empty())
                {
                    self.thunderbird_auto_open_pending = false;
                    self.thunderbird_auto_open_seen_loading = false;
                    if self.view == View::Setup {
                        self.setup = None;
                        self.setup_email_subscription = None;
                        self.view = View::Mailbox;
                    }
                }
            }
        }
        let mut close_composer = false;
        if let Some(composer) = self.composer.as_mut() {
            if composer.draft_pending.is_some() {
                if snapshot.status == "Draft saved" {
                    composer.draft_saved = composer.draft_pending.take();
                    composer.error = None;
                } else if snapshot.status != "Saving draft…"
                    && (snapshot.action_error.is_some() || snapshot.error.is_some())
                {
                    composer.draft_pending = None;
                }
            }
        }
        if let (Some(composer), Some(result)) = (&mut self.composer, snapshot.send_result) {
            if composer.send_request_id == Some(result.request_id)
                && composer.message.from_account_id == result.account_id
            {
                composer.send_request_id = None;
                if result.uncertain {
                    composer.send_queued = false;
                    composer.send_uncertain = true;
                    composer.error = None;
                } else {
                    match result.outcome {
                        SendOutcome::Sent => close_composer = true,
                        SendOutcome::Queued => {
                            composer.send_queued = true;
                            composer.error = Some(
                                "The message is safely queued in Outbox. It will keep its own copy there; close this composer when you are ready.".into(),
                            );
                        }
                        SendOutcome::Failed => {
                            composer.send_queued = false;
                            composer.error = Some(
                                "The message was not sent. Its text is still here; review the error and try again."
                                    .into(),
                            );
                        }
                    }
                }
            }
        }
        if close_composer {
            self.cleanup_composer_attachments(cx);
            self.composer = None;
            if self.view == View::Appearance {
                if self.appearance_return_view == View::Compose {
                    self.appearance_return_view = View::Mailbox;
                }
            } else {
                self.view = View::Mailbox;
            }
        }
        cx.notify();
    }

    fn sender_choices(
        &self,
        account_id: u32,
        snapshot: &MailboxSnapshot,
        cx: &App,
    ) -> Vec<SenderChoice> {
        let email = snapshot
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .map(|account| account.email.clone())
            .unwrap_or_default();
        let mut choices = vec![SenderChoice {
            alias: None,
            label: email.clone(),
        }];
        let mut seen = HashSet::from([email.to_ascii_lowercase()]);
        if let Some((_, profile)) = self
            .profiles
            .iter()
            .find(|(profile_id, _)| *profile_id == account_id)
        {
            for alias in &profile.aliases {
                let address = alias.address();
                if address.is_empty()
                    || address.eq_ignore_ascii_case(&email)
                    || !seen.insert(address.to_ascii_lowercase())
                {
                    continue;
                }
                choices.push(SenderChoice {
                    alias: Some(alias.identity.clone()),
                    label: alias.identity.clone(),
                });
            }
        } else {
            let identities = self.live.read(cx).sender_identities(account_id);
            for (index, identity) in identities.iter().enumerate() {
                let address = identity.email.trim();
                if address.is_empty() {
                    continue;
                }
                let label = if identity.name.trim().is_empty() {
                    address.to_owned()
                } else {
                    format!("{} <{}>", identity.name.trim(), address)
                };
                if identity.primary || (index == 0 && choices.len() == 1) {
                    choices[0].label = label;
                    seen.insert(address.to_ascii_lowercase());
                } else if seen.insert(address.to_ascii_lowercase()) {
                    choices.push(SenderChoice {
                        alias: Some(label.clone()),
                        label,
                    });
                }
            }
        }
        choices
    }

    fn cycle_sender_identity(&mut self, account_id: u32, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let choices = self.sender_choices(account_id, &snapshot, cx);
        if choices.len() < 2 {
            return;
        }
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        if !composer.can_edit() || composer.message.from_account_id != account_id {
            return;
        }
        let current = choices
            .iter()
            .position(|choice| choice.alias == composer.message.from_alias)
            .unwrap_or(0);
        let next = (current + 1) % choices.len();
        composer.message.from_alias = choices[next].alias.clone();
        composer.draft_saved = None;
        composer.error = None;
        cx.notify();
    }

    fn cleanup_composer_attachments(&mut self, cx: &App) {
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        let paths = std::mem::take(&mut composer.temporary_attachment_paths);
        if !paths.is_empty() {
            cx.background_executor()
                .spawn(async move { compose::remove_staged_attachments(paths) })
                .detach();
        }
    }

    fn can_replace_composer(&mut self, cx: &App) -> bool {
        let Some(composer) = self.composer.as_mut() else {
            return true;
        };
        let error = if composer.send_uncertain {
            Some(
                "Check Sent before continuing. Confirm the message was not delivered to unlock this composer.",
            )
        } else if composer.operation_pending() {
            Some(
                "Finish the current send, draft save, or file selection before opening another message.",
            )
        } else if !composer.is_saved(cx) && !composer.send_queued {
            Some("Save this message as a draft or discard it before opening another.")
        } else {
            None
        };
        if let Some(error) = error {
            composer.error = Some(error.into());
            self.view = View::Compose;
            return false;
        }
        self.cleanup_composer_attachments(cx);
        true
    }

    fn open_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_replace_composer(cx) {
            return;
        }
        self.composer = None;
        self.setup_return_view = self.view;
        if self.setup.is_none() {
            self.setup = Some(SetupState::new(window, cx));
            self.watch_setup_email(window, cx);
        }
        self.view = View::Setup;
        cx.notify();
    }

    fn confirm_send_not_delivered(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        if !composer.send_uncertain {
            return;
        }
        composer.send_uncertain = false;
        composer.error = None;
        cx.notify();
    }

    fn pane_surface(&self, color: gpui_kit::Hsla, bleed: f32) -> gpui_kit::Hsla {
        if self.surface_treatment == zeron_theme::SurfaceTreatment::Opaque {
            return gpui_kit::Hsla { a: 1.0, ..color };
        }
        let limit = if self.appearance.visual_wallpaper_opacity() > 0.001 {
            (self.appearance.safe_opacity() / self.appearance.visual_wallpaper_opacity())
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        gpui_kit::Hsla {
            a: 1.0 - bleed.min(limit),
            ..color
        }
    }

    fn activate_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dark = self.theme.appearance(self.system_dark) == zeron_theme::Appearance::Dark;
        self.appearance.set_dark(self.dark);
        let resolved = self.theme.resolved(self.system_dark);
        self.surface_treatment = resolved.surface_treatment;
        let palette = Palette::from_resolved(resolved);
        self.palette = palette;
        self.appearance.set_accent_rgb(packed_rgb(palette.accent));
        self.appearance.set_contrast_colors(
            packed_rgb(palette.text),
            packed_rgb(palette.muted),
            packed_rgb(palette.background),
        );
        apply_palette(
            if self.dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            palette,
            window,
            cx,
        );
        self.restore_wallpaper(cx);
        cx.notify();
    }

    fn change_theme(&mut self, change: ThemeChange, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_busy || self.appearance_loading || self.appearance_busy {
            return;
        }
        let mut proposed = self.theme.clone();
        match change {
            ThemeChange::Mode(mode) => proposed.set_mode(mode),
            ThemeChange::Variant(appearance, id) => {
                if !proposed.set_variant(appearance, &id) {
                    return;
                }
            }
            ThemeChange::Accent(accent) => proposed.set_accent(accent),
            ThemeChange::Surface(surface) => proposed.set_surface(surface),
            ThemeChange::Remove(id) => {
                if !proposed.remove_library_entry(&id) {
                    return;
                }
            }
        }
        self.save_theme(proposed, window, cx);
    }

    fn save_theme(
        &mut self,
        mut proposed: theme::ThemeState,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_busy = true;
        self.theme_error = None;
        self.theme_status = None;
        let task = cx.background_spawn(async move { proposed.persist().map(|_| proposed) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.theme_busy = false;
                match result {
                    Ok(theme) => {
                        this.theme = theme;
                        this.activate_theme(window, cx);
                    }
                    Err(error) => this.theme_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reset_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_busy || self.appearance_busy || self.appearance_loading {
            return;
        }
        if self.theme.is_loaded() {
            let mut proposed = self.theme.clone();
            proposed.reset_preferences();
            self.save_theme(proposed, window, cx);
        } else {
            self.theme_busy = true;
            let task = cx.background_spawn(async move { theme::ThemeState::recover_defaults() });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update_in(cx, |this, window, cx| {
                    this.theme_busy = false;
                    match result {
                        Ok(theme) => this.save_theme(theme, window, cx),
                        Err(error) => this.theme_error = Some(error),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    fn import_theme(&mut self, cx: &mut Context<Self>) {
        if self.theme_busy || self.appearance_loading || self.appearance_busy {
            return;
        }
        self.theme_busy = true;
        self.theme_error = None;
        self.theme_status = None;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import a VS Code theme or package.json".into()),
        });
        let mut proposed = self.theme.clone();
        cx.spawn(async move |this,cx| {
            let chosen = picker.await;
            let path = match chosen {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None))|Err(_) => None,
                Ok(Err(error)) => {let _=this.update(cx,|this,cx|{this.theme_busy=false;this.theme_error=Some(format!("Could not open theme picker: {error}"));cx.notify();});return;}
            };
            let Some(path)=path else {let _=this.update(cx,|this,cx|{this.theme_busy=false;cx.notify();});return;};
            let result=cx.background_spawn(async move {let count=proposed.import_file(&path)?;proposed.persist()?;Ok::<_,String>((proposed,count))}).await;
            let _=this.update_in(cx,|this,window,cx|{
                this.theme_busy=false;
                match result {Ok((theme,count))=>{this.theme=theme;this.activate_theme(window,cx);this.theme_status=Some(format!("Imported {count} theme variants. Choose one in the light or dark theme menu."));},Err(error)=>this.theme_error=Some(error)}
                cx.notify();
            });
        }).detach();
    }

    fn animate_wallpaper(&mut self, cx: &mut Context<Self>) {
        if self.wallpaper_animating {
            return;
        }
        self.wallpaper_animating = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let active = this
                    .update(cx, |this, cx| {
                        let active = this.appearance.tick_transition(std::time::Instant::now());
                        this.wallpaper_animating = active;
                        cx.notify();
                        active
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        })
        .detach();
    }

    fn open_appearance(&mut self, cx: &mut Context<Self>) {
        if self.view == View::Appearance {
            self.view = self.appearance_return_view;
        } else {
            self.appearance_return_view = self.view;
            self.view = View::Appearance;
        }
        cx.notify();
    }

    fn restore_wallpaper(&mut self, cx: &mut Context<Self>) {
        let light = !self.dark;
        let Some(request) = self.appearance.restore_request(light) else {
            return;
        };
        self.appearance_busy = true;
        let task = cx.background_spawn(async move { appearance::begin_wallpaper_restore(request) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(result) => {
                        if !this.appearance.install_processed_wallpaper(result, !this.dark) {
                            this.appearance_error = Some(
                                "The saved wallpaper changed while it was loading. Reopen Appearance to retry.".into(),
                            );
                        }
                    }
                    Err(error) => this.appearance_error = Some(error),
                }
                this.appearance_busy = false;
                this.animate_wallpaper(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn persist_appearance(&mut self, cx: &mut Context<Self>) {
        self.persist_appearance_with_status(None, cx);
    }

    fn persist_appearance_with_status(&mut self, status: Option<String>, cx: &mut Context<Self>) {
        self.appearance_busy = true;
        let preferences = self.appearance.preference_snapshot();
        let task = cx.background_spawn(async move { appearance::persist_preferences(preferences) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.appearance_busy = false;
                match result {
                    Ok(()) => this.appearance_error = status,
                    Err(error) => this.appearance_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn change_appearance(&mut self, change: AppearanceChange, cx: &mut Context<Self>) {
        if self.appearance_busy || self.appearance_loading {
            return;
        }
        let mut proposed = self.appearance.clone();
        match change {
            AppearanceChange::Preset(preset) => proposed.set_preset(preset),
            AppearanceChange::Effect(effect) => proposed.set_effect(effect),
            AppearanceChange::Opacity(opacity) => proposed.set_opacity(opacity),
            AppearanceChange::Blur(sigma) => proposed.set_blur_sigma(sigma),
            AppearanceChange::EffectStrength(strength) => proposed.set_effect_strength(strength),
            AppearanceChange::Fade(fade) => proposed.set_fade(fade),
        }
        let needs_reprocess = matches!(
            change,
            AppearanceChange::Preset(_)
                | AppearanceChange::Effect(_)
                | AppearanceChange::Blur(_)
                | AppearanceChange::EffectStrength(_)
        );
        let light = !self.dark;
        if needs_reprocess {
            if let Some(request) = proposed.restore_request(light) {
                self.appearance_busy = true;
                self.appearance_error = None;
                let task = cx
                    .background_spawn(async move { appearance::begin_wallpaper_restore(request) });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    let _ = this.update(cx, |this, cx| {
                        match result {
                            Ok(result) => {
                                if proposed.install_processed_wallpaper(result, !this.dark) {
                                    this.appearance = proposed;
                                    this.animate_wallpaper(cx);
                                    this.persist_appearance(cx);
                                } else {
                                    this.appearance_busy = false;
                                    this.appearance_error = Some(
                                        "Appearance changed before the wallpaper effect finished. Try again.".into(),
                                    );
                                }
                            }
                            Err(error) => {
                                this.appearance_busy = false;
                                this.appearance_error = Some(error);
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
                cx.notify();
                return;
            }
        }
        self.appearance = proposed;
        self.appearance_error = None;
        self.persist_appearance(cx);
        cx.notify();
    }

    fn choose_wallpaper(&mut self, cx: &mut Context<Self>) {
        if self.appearance_busy || self.appearance_loading {
            return;
        }
        self.appearance_busy = true;
        self.appearance_error = None;
        let light = !self.dark;
        let effect = self.appearance.effect();
        let blur_sigma = self.appearance.blur_sigma();
        let strength = self.appearance.effect_strength();
        let contrast_colors = self.appearance.contrast_colors();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a wallpaper image".into()),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let chosen = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.appearance_busy = false;
                        this.appearance_error =
                            Some(format!("The image picker could not open: {error}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = chosen else {
                let _ = this.update(cx, |this, cx| {
                    this.appearance_busy = false;
                    cx.notify();
                });
                return;
            };
            let result = cx
                .background_spawn(async move {
                    appearance::begin_wallpaper_import(
                        path,
                        effect,
                        light,
                        blur_sigma,
                        strength,
                        contrast_colors,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(result) => {
                        this.appearance.install_wallpaper(result);
                        this.animate_wallpaper(cx);
                        this.persist_appearance(cx);
                    }
                    Err(error) => {
                        this.appearance_busy = false;
                        this.appearance_error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_theme(
            ThemeChange::Mode(if self.dark {
                theme::ThemeMode::Light
            } else {
                theme::ThemeMode::Dark
            }),
            window,
            cx,
        );
    }

    fn open_new_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_replace_composer(cx) {
            return;
        }
        let Some(account_id) = self.live.read(cx).snapshot().current_account_id else {
            return;
        };
        self.composer = Some(Composer::new(compose::new_message(account_id), window, cx));
        self.view = View::Compose;
        cx.notify();
    }

    fn open_reply(&mut self, message: Message, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_replace_composer(cx) {
            return;
        }
        let account_id = message.account_id;
        self.composer = Some(Composer::new(
            compose::reply(account_id, &message),
            window,
            cx,
        ));
        self.view = View::Compose;
        cx.notify();
    }

    fn open_forward(&mut self, message: Message, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_replace_composer(cx) {
            return;
        }
        let account_id = message.account_id;
        self.composer = Some(Composer::new(
            compose::forward(account_id, &message),
            window,
            cx,
        ));
        self.view = View::Compose;
        cx.notify();
    }

    fn open_draft(&mut self, row: MessageRow, cx: &mut Context<Self>, window: &mut Window) {
        if !self.can_replace_composer(cx) {
            return;
        }
        let key = row.key.clone();
        self.draft_open_error = None;
        let token = self
            .live
            .update(cx, |live, cx| live.request_edit_draft(key.clone(), cx));
        let Some(token) = token else {
            self.draft_open_error =
                Some("Could not load this draft for editing. Try refreshing the folder.".into());
            cx.notify();
            return;
        };
        let profiles = self.profiles.clone();
        cx.spawn_in(window, async move |this, cx| {
            for _ in 0..1800 {
                let source = this
                    .read_with(cx, |this, app| {
                        this.live
                            .read(app)
                            .snapshot()
                            .draft_source
                            .filter(|source| source.key == key && source.token == token)
                    })
                    .ok()
                    .flatten();
                let Some(source) = source else {
                    return;
                };
                match source.state {
                    DraftSourceState::Loading => {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                    DraftSourceState::Raw(Err(error)) => {
                        let _ = this.update_in(cx, |this, _, cx| {
                            this.draft_open_error = Some(error);
                            this.live.update(cx, |live, cx| {
                                live.dismiss_draft_source(&key, token, cx);
                            });
                            cx.notify();
                        });
                        return;
                    }
                    DraftSourceState::Raw(Ok(raw)) => {
                        let Some((_, account)) = profiles
                            .iter()
                            .find(|(id, _)| *id == source.origin.account_id)
                        else {
                            let _ = this.update_in(cx, |this, _, cx| {
                                this.draft_open_error =
                                    Some("The draft's account is no longer available.".into());
                                this.live.update(cx, |live, cx| {
                                    live.dismiss_draft_source(&key, token, cx);
                                });
                                cx.notify();
                            });
                            return;
                        };
                        let account = account.clone();
                        let origin = source.origin.clone();
                        let raw = raw.clone();
                        let prepared = cx
                            .background_executor()
                            .spawn(async move {
                                compose::prepare_draft_from_raw(&raw, &account, origin)
                            })
                            .await;
                        let prepared = match prepared {
                            Ok(prepared) => prepared,
                            Err(error) => {
                                let published = this.update(cx, |this, cx| {
                                    this.live.update(cx, |live, cx| {
                                        live.publish_prepared_draft(
                                            key.clone(),
                                            token,
                                            Arc::new(Err(error.clone())),
                                            cx,
                                        )
                                    })
                                });
                                if published.ok().unwrap_or(false) {
                                    let _ = this.update_in(cx, |this, _, cx| {
                                        this.draft_open_error = Some(error);
                                        this.live.update(cx, |live, cx| {
                                            live.dismiss_draft_source(&key, token, cx);
                                        });
                                        cx.notify();
                                    });
                                }
                                return;
                            }
                        };

                        let message = prepared.message.clone();
                        let paths = prepared.temporary_attachment_paths.clone();
                        let notice = prepared.notice.clone();
                        let to_publish = message.clone();
                        let published = this.update(cx, |this, cx| {
                            this.live.update(cx, |live, cx| {
                                live.publish_prepared_draft(
                                    key.clone(),
                                    token,
                                    Arc::new(Ok(to_publish)),
                                    cx,
                                )
                            })
                        });
                        if !published.ok().unwrap_or(false) {
                            let cleanup = paths;
                            cx.background_executor()
                                .spawn(async move { compose::remove_staged_attachments(cleanup) })
                                .detach();
                            return;
                        }
                        let paths_for_open = paths.clone();
                        let opened =
                            this.update_in(cx, |this, window, cx| {
                                let accepted =
                                    this.live.read(cx).snapshot().draft_source.is_some_and(
                                        |source| {
                                            source.key == key
                                                && source.token == token
                                                && matches!(
                                                    source.state,
                                                    DraftSourceState::Prepared(result)
                                                        if result.is_ok()
                                                )
                                        },
                                    );
                                if !accepted {
                                    return false;
                                }
                                this.composer = Some(Composer::new_prepared_draft(
                                    message,
                                    paths_for_open,
                                    notice,
                                    window,
                                    cx,
                                ));
                                this.draft_open_error = None;
                                this.view = View::Compose;
                                this.live.update(cx, |live, cx| {
                                    live.dismiss_draft_source(&key, token, cx);
                                });
                                cx.notify();
                                true
                            });
                        if !opened.ok().unwrap_or(false) {
                            cx.background_executor()
                                .spawn(async move { compose::remove_staged_attachments(paths) })
                                .detach();
                        }
                        return;
                    }
                    DraftSourceState::Prepared(_) => return,
                }
            }
            let _ = this.update_in(cx, |this, _, cx| {
                this.draft_open_error = Some(
                    "Opening this draft took too long. Refresh the folder and try again.".into(),
                );
                this.live.update(cx, |live, cx| {
                    live.dismiss_draft_source(&key, token, cx);
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn start_discovery(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        if setup.thunderbird_loading {
            return;
        }
        let email = setup.inputs.email.read(cx).value().trim().to_owned();
        if discovery::validate_email_address(&email).is_err() {
            setup.discovery_error = Some("Enter a valid email address first.".into());
            cx.notify();
            return;
        }
        setup.discovering = true;
        setup.discovery_error = None;
        setup.report = None;
        setup.error = None;
        setup.reviewed_settings = None;
        let request_email = email.clone();
        let task = cx.background_spawn(async move { discovery::discover(&request_email) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(setup) = this.setup.as_mut() {
                    setup.discovering = false;
                    match result {
                        Ok(report) => {
                            let current_email = setup.inputs.email.read(cx).value().trim().to_owned();
                            if !current_email.eq_ignore_ascii_case(&email)
                                || !report.email.eq_ignore_ascii_case(&email)
                            {
                                setup.discovering = false;
                                setup.report = None;
                                setup.discovery_error = if current_email.eq_ignore_ascii_case(&email) {
                                    Some("The settings lookup returned a different email address. Run it again or enter server settings manually.".into())
                                } else {
                                    None
                                };
                                cx.notify();
                                return;
                            }
                            if let Some(candidate) = report.candidates.first() {
                                setup.imap_tls = candidate.incoming.tls;
                                setup.smtp_tls = candidate.outgoing.tls;
                                setup.smtp_auth = candidate.outgoing.auth;
                                setup.inputs.smtp_username.update(cx, |input, cx| {
                                    input.set_value(
                                        candidate.outgoing.username.default_for(&email),
                                        window,
                                        cx,
                                    )
                                });
                                setup.warning_reviewed = !candidate.requires_review();
                                setup.reviewed_settings = None;
                                setup.manual = false;
                            } else {
                                setup.manual = true;
                                setup.warning_reviewed = false;
                                setup.reviewed_settings = None;
                            }
                            setup.report = Some(report);
                        }
                        Err(error) => {
                            setup.manual = true;
                            setup.discovery_error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn import_thunderbird_settings(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        let bridge_loading = self.live.read(cx).snapshot().thunderbird_loading;
        if setup.discovering || setup.connecting || setup.thunderbird_loading || bridge_loading {
            return;
        }
        setup.thunderbird_loading = true;
        setup.thunderbird_error = None;
        let task =
            cx.background_spawn(async move { megamail_core::thunderbird::discover_profiles() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                let existing_emails = this
                    .live
                    .read(cx)
                    .snapshot()
                    .accounts
                    .into_iter()
                    .map(|account| account.email.to_ascii_lowercase())
                    .collect::<HashSet<_>>();
                if let Some(setup) = this.setup.as_mut() {
                    setup.thunderbird_loading = false;
                    match result {
                        Ok(profiles) => {
                            setup.selected_thunderbird = default_thunderbird_selection(
                                &profiles,
                                &existing_emails,
                            );
                            setup.thunderbird_profiles = profiles;
                            if setup.thunderbird_profiles.is_empty() {
                                setup.thunderbird_error = Some(
                                    "No Thunderbird mail settings were found in the standard profile locations.".into(),
                                );
                            } else if setup.selected_thunderbird.is_empty() {
                                setup.thunderbird_error = Some(
                                    "No unadded Thunderbird mail accounts with valid email addresses were found.".into(),
                                );
                            }
                        }
                        Err(error) => {
                            setup.thunderbird_error = Some(format!(
                                "Could not read Thunderbird settings: {error}"
                            ));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_thunderbird_selection(&mut self, select_all: bool, cx: &mut Context<Self>) {
        let importing = self.live.read(cx).snapshot().thunderbird_loading;
        let existing_emails = self
            .live
            .read(cx)
            .snapshot()
            .accounts
            .into_iter()
            .map(|account| account.email.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        if setup.thunderbird_loading || importing {
            return;
        }
        setup.selected_thunderbird = if select_all {
            default_thunderbird_selection(&setup.thunderbird_profiles, &existing_emails)
        } else {
            HashSet::new()
        };
        setup.thunderbird_error = None;
        cx.notify();
    }

    fn toggle_thunderbird_selection(
        &mut self,
        profile_path: PathBuf,
        account_id: String,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.live.read(cx).snapshot();
        let importing = snapshot.thunderbird_loading;
        let existing_emails = snapshot
            .accounts
            .iter()
            .map(|account| account.email.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        if setup.thunderbird_loading || importing {
            return;
        }
        let key = (profile_path, account_id);
        if !setup
            .thunderbird_profiles
            .iter()
            .find(|profile| profile.path == key.0)
            .is_some_and(|profile| {
                profile.accounts.iter().any(|account| {
                    account.account_id == key.1
                        && thunderbird_account_email(account).is_some_and(|email| {
                            !existing_emails.contains(&email.to_ascii_lowercase())
                        })
                })
            })
        {
            return;
        }
        if !setup.selected_thunderbird.remove(&key) {
            setup.selected_thunderbird.insert(key);
        }
        setup.thunderbird_error = None;
        cx.notify();
    }

    fn import_selected_thunderbird(&mut self, cx: &mut Context<Self>) {
        let importing = self.live.read(cx).snapshot().thunderbird_loading;
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        if setup.thunderbird_loading || importing {
            return;
        }
        let mut selections = Vec::new();
        for profile in &setup.thunderbird_profiles {
            let account_ids = profile
                .accounts
                .iter()
                .filter(|account| {
                    thunderbird_account_email(account).is_some()
                        && setup
                            .selected_thunderbird
                            .contains(&(profile.path.clone(), account.account_id.clone()))
                })
                .map(|account| account.account_id.clone())
                .collect::<Vec<_>>();
            if !account_ids.is_empty() {
                selections.push((profile.clone(), account_ids));
            }
        }
        let selected_count = selections.iter().map(|(_, ids)| ids.len()).sum::<usize>();
        if selected_count == 0 {
            setup.thunderbird_error = Some("Select at least one Thunderbird mail account.".into());
            cx.notify();
            return;
        }
        setup.thunderbird_error = None;

        let result = self
            .live
            .update(cx, |live, cx| live.import_thunderbird(selections, cx));
        if let Some(setup) = self.setup.as_mut() {
            match result {
                Ok(()) => {
                    self.thunderbird_auto_open_pending = true;
                    self.thunderbird_auto_open_seen_loading = true;
                }
                Err(error) => setup.thunderbird_error = Some(error),
            }
        }
        cx.notify();
    }

    fn collect_account(&self, cx: &App) -> Result<AccountConfig, String> {
        let setup = self.setup.as_ref().ok_or("Account setup is closed.")?;
        let email = setup.inputs.email.read(cx).value().to_string();
        if !setup.manual
            && setup
                .report
                .as_ref()
                .is_some_and(|report| !email.trim().eq_ignore_ascii_case(report.email.trim()))
        {
            return Err("The email address changed. Find settings again for this address, or enter server settings manually.".into());
        }
        let mut form = setup
            .candidate()
            .map(|candidate| AccountForm::from_candidate(&email, candidate))
            .unwrap_or_default();
        form.email = email;
        let name = setup.inputs.name.read(cx).value().to_string();
        if !name.trim().is_empty() {
            form.name = name;
        }
        let username = setup.inputs.username.read(cx).value().to_string();
        if !username.trim().is_empty() {
            form.username = username;
        }
        form.password = setup.inputs.password.read(cx).value().to_string();
        form.smtp_username = setup.inputs.smtp_username.read(cx).value().to_string();
        form.smtp_password = setup.inputs.smtp_password.read(cx).value().to_string();
        if setup.manual || setup.candidate().is_none() {
            let imap_host = setup.inputs.imap_host.read(cx).value().to_string();
            if !imap_host.trim().is_empty() {
                form.imap_host = imap_host;
            }
            let imap_port = setup.inputs.imap_port.read(cx).value().to_string();
            if !imap_port.trim().is_empty() {
                form.imap_port = imap_port;
            }
            let smtp_host = setup.inputs.smtp_host.read(cx).value().to_string();
            if !smtp_host.trim().is_empty() {
                form.smtp_host = smtp_host;
            }
            let smtp_port = setup.inputs.smtp_port.read(cx).value().to_string();
            if !smtp_port.trim().is_empty() {
                form.smtp_port = smtp_port;
            }
        }
        form.imap_tls = setup.imap_tls;
        form.smtp_tls = setup.smtp_tls;
        form.smtp_auth = setup.smtp_auth;
        let needs_review = setup.manual
            || setup.candidate().is_none()
            || setup
                .candidate()
                .is_some_and(|candidate| candidate.requires_review());
        form.warning_reviewed = !needs_review || setup.review_is_current(cx);

        if let Some(candidate) = setup.candidate() {
            if setup.manual {
                form.build_manual()
            } else {
                form.build_from_candidate(candidate)
            }
        } else {
            form.build_manual()
        }
    }

    fn connect_account(&mut self, cx: &mut Context<Self>) {
        if self.setup.as_ref().is_some_and(|setup| setup.connecting) {
            return;
        }
        if self.loading_profiles || self.startup_error.is_some() {
            if let Some(setup) = self.setup.as_mut() {
                setup.error = Some(
                    "Saved accounts could not be loaded safely. Resolve the profile storage issue before adding another account.".into(),
                );
            }
            cx.notify();
            return;
        }
        let account = match self.collect_account(cx) {
            Ok(account) => account,
            Err(error) => {
                if let Some(setup) = self.setup.as_mut() {
                    setup.error = Some(error);
                }
                cx.notify();
                return;
            }
        };
        if self
            .profiles
            .iter()
            .any(|(_, profile)| profile.email.eq_ignore_ascii_case(&account.email))
        {
            if let Some(setup) = self.setup.as_mut() {
                setup.error = Some("This email address is already added to MegaMail.".into());
            }
            cx.notify();
            return;
        }
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        setup.connecting = true;
        setup.error = None;
        let email = account.email.clone();
        let email_for_lookup = email.clone();

        let mut profiles = self
            .profiles
            .iter()
            .map(|(_, profile)| profile.clone())
            .collect::<Vec<_>>();
        if let Some(index) = profiles
            .iter()
            .position(|profile| profile.email.eq_ignore_ascii_case(&account.email))
        {
            profiles[index] = account.clone();
        } else {
            profiles.push(account.clone());
        }

        let task = cx.background_spawn(async move {
            let outcome = worker::test_connection_blocking(account);
            outcome.incoming.map_err(|error| {
                format!("Could not connect to the incoming mail server: {error}")
            })?;
            outcome.smtp.map_err(|error| {
                format!("Could not connect to the outgoing mail server: {error}")
            })?;
            config::save(&profiles)
                .map_err(|error| format!("Could not save the account: {error}"))?;
            let profiles = config::load_profiles().map_err(|error| {
                format!("The account was saved, but its profile could not be reloaded: {error}")
            })?;
            let added = profiles
                .iter()
                .find(|(_, profile)| profile.email.eq_ignore_ascii_case(&email_for_lookup))
                .cloned()
                .ok_or_else(|| "The saved account was not found after reload.".to_owned())?;
            Ok::<_, String>((profiles, added))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((profiles, (account_id, profile))) => {
                        this.profiles = profiles;
                        let added = this
                            .live
                            .update(cx, |live, cx| live.add_profile(account_id, profile, cx));
                        match added {
                            Ok(()) => {
                                this.setup = None;
                                this.setup_email_subscription = None;
                                this.view = this.setup_return_view;
                                this.startup_error = None;
                            }
                            Err(error) => {
                                if let Some(setup) = this.setup.as_mut() {
                                    setup.connecting = false;
                                    setup.error = Some(format!(
                                        "The account was saved but could not be started: {error}"
                                    ));
                                }
                            }
                        }
                    }
                    Err(error) => {
                        if error.starts_with(
                            "The account was saved, but its profile could not be reloaded:",
                        ) {
                            this.startup_error = Some(
                                "The account was saved, but the profile list could not be reloaded. Resolve the profile storage issue and restart MegaMail before adding another account.".into(),
                            );
                        }
                        if let Some(setup) = this.setup.as_mut() {
                            setup.connecting = false;
                            setup.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        if snapshot.page.is_empty() {
            return;
        }
        let current = snapshot
            .selected_key
            .as_ref()
            .and_then(|selected| snapshot.page.iter().position(|row| &row.key == selected))
            .unwrap_or(0);
        let next = (current as isize + delta).clamp(0, snapshot.page.len() as isize - 1) as usize;
        let key = snapshot.page[next].key.clone();
        self.draft_open_error = None;
        self.live.update(cx, |live, cx| {
            live.select_message(key, cx);
            if next + 1 == snapshot.page.len() && snapshot.has_more && delta > 0 {
                live.load_more(cx);
            }
        });
        self.list_scroll
            .scroll_to_item(next, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn folder_button(
        &self,
        folder: &Folder,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        dark: bool,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = snapshot.current_folder_path.as_deref() == Some(folder.path.as_str());
        let path = folder.path.clone();
        let button = Button::new(SharedString::from(format!("folder-{}", folder.id)))
            .custom(sidebar_button_variant(cx, palette, dark, selected))
            .small()
            .selected(selected)
            .disabled(self.composer.is_some())
            .w_full()
            .h(px(zeron_style::NAV_ROW_MIN_HEIGHT))
            .rounded(px(4.))
            .px_2()
            .when(collapsed, |button| {
                button
                    .w(px(42.))
                    .px_0()
                    .icon(Icon::new(folder_icon(folder.kind)).small())
                    .tooltip(folder.name.clone())
            })
            .accessibility_label(format!("{} folder", folder.name))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.live
                    .update(cx, |live, cx| live.select_folder(path.clone(), cx));
                this.list_scroll = UniformListScrollHandle::new();
                this.view = View::Mailbox;
                cx.notify();
            }))
            .when(!collapsed, |button| {
                button.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            h_flex()
                                .min_w_0()
                                .flex_1()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(folder_icon(folder.kind))
                                        .size(px(14.))
                                        .text_color(if selected {
                                            palette.text
                                        } else {
                                            palette.muted
                                        }),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(zeron_style::NAV_ROW_TEXT_SIZE))
                                        .font_weight(if selected {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(zeron_style::text_role_color(
                                            &palette,
                                            if selected {
                                                zeron_style::TextRole::Primary
                                            } else {
                                                zeron_style::TextRole::Secondary
                                            },
                                        ))
                                        .child(folder.name.clone()),
                                ),
                        )
                        .child(if folder.unread > 0 {
                            div()
                                .min_w(px(18.))
                                .h(px(18.))
                                .px_1()
                                .rounded(px(5.))
                                .items_center()
                                .justify_center()
                                .text_size(px(10.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if selected {
                                    palette.text
                                } else {
                                    palette.muted
                                })
                                .child(folder.unread.to_string())
                                .into_any_element()
                        } else {
                            div().w(px(18.)).into_any_element()
                        }),
                )
            });
        button.into_any_element()
    }

    fn account_button(
        &self,
        account: &megamail_core::models::Account,
        selected: bool,
        palette: Palette,
        dark: bool,
        collapsed: bool,
        snapshot: &MailboxSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let account_id = account.id;
        let title = if account.name.trim().is_empty() {
            account.email.clone()
        } else {
            account.name.clone()
        };
        let button = Button::new(SharedString::from(format!("account-{account_id}")))
            .custom(sidebar_button_variant(cx, palette, dark, selected))
            .small()
            .selected(selected)
            .disabled(self.composer.is_some() || snapshot.pending_send)
            .w_full()
            .h(px(
                zeron_style::ACCOUNT_TRIGGER_HEIGHT + zeron_style::ACCOUNT_DETAIL_LINE_HEIGHT
            ))
            .rounded(px(4.))
            .px_2()
            .when(collapsed, |button| {
                button.w(px(42.)).px_0().tooltip(account.email.clone())
            })
            .accessibility_label(format!("Switch to {}", account.email))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.live
                    .update(cx, |live, cx| live.switch_account(account_id, cx));
                this.list_scroll = UniformListScrollHandle::new();
                this.view = View::Mailbox;
                cx.notify();
            }))
            .when(collapsed, |button| {
                button.child(
                    div()
                        .size(px(zeron_style::ACCOUNT_AVATAR_SIZE))
                        .rounded_full()
                        .bg(palette.accent_wash)
                        .text_color(palette.accent)
                        .text_size(px(zeron_style::ACCOUNT_AVATAR_GLYPH_SIZE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .items_center()
                        .justify_center()
                        .child(initials(&title)),
                )
            })
            .when(!collapsed, |button| {
                button.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap(px(zeron_style::ACCOUNT_TRIGGER_GAP))
                        .child(
                            div()
                                .size(px(zeron_style::ACCOUNT_AVATAR_SIZE))
                                .rounded_full()
                                .bg(palette.accent_wash)
                                .text_color(palette.accent)
                                .text_size(px(zeron_style::ACCOUNT_AVATAR_GLYPH_SIZE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .items_center()
                                .justify_center()
                                .child(initials(&title)),
                        )
                        .child(
                            v_flex()
                                .min_w_0()
                                .flex_1()
                                .gap_1()
                                .items_start()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(zeron_style::ACCOUNT_PRIMARY_TEXT_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(zeron_style::text_role_color(
                                            &palette,
                                            zeron_style::TextRole::Primary,
                                        ))
                                        .child(title.clone()),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(zeron_style::ACCOUNT_DETAIL_TEXT_SIZE))
                                        .text_color(zeron_style::text_role_color(
                                            &palette,
                                            zeron_style::TextRole::Secondary,
                                        ))
                                        .child(account.email.clone()),
                                ),
                        )
                        .when(selected, |row| {
                            row.child(
                                Icon::new(IconName::Check)
                                    .size(px(13.))
                                    .text_color(palette.accent),
                            )
                        }),
                )
            });
        button.into_any_element()
    }

    fn sidebar(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dark = self.dark;
        if self.sidebar_collapsed {
            return self.collapsed_sidebar(snapshot, palette, dark, cx);
        }
        let current = snapshot.current_account_id;
        let account = snapshot
            .accounts
            .iter()
            .find(|account| Some(account.id) == current);
        let mut primary = snapshot
            .folders
            .iter()
            .filter(|folder| primary_folder_rank(folder.kind).is_some())
            .collect::<Vec<_>>();
        primary.sort_by_key(|folder| primary_folder_rank(folder.kind).unwrap_or(usize::MAX));
        let mut other = snapshot
            .folders
            .iter()
            .filter(|folder| primary_folder_rank(folder.kind).is_none())
            .collect::<Vec<_>>();
        other.sort_by_key(|folder| folder.name.to_lowercase());
        v_flex()
            .w(px(232.))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .bg(self.pane_surface(palette.sidebar, 0.24))
            .border_r_1()
            .border_color(palette.border)
            .child(
                v_flex()
                    .flex_shrink_0()
                    .px_2()
                    .pt_3()
                    .pb_2()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_1()
                            .child(
                                Button::new("choose-account")
                                    .custom(floating_button_variant(cx, palette, dark))
                                    .small()
                                    .min_w_0()
                                    .flex_1()
                                    .h(px(38.))
                                    .icon(Icon::new(IconName::Mail).size(px(15.)))
                                    .label(
                                        account
                                            .map(|account| {
                                                if account.name.is_empty() {
                                                    account.email.clone()
                                                } else {
                                                    account.name.clone()
                                                }
                                            })
                                            .unwrap_or_else(|| "Mail accounts".into()),
                                    )
                                    .tooltip(
                                        account
                                            .map(|account| account.email.clone())
                                            .unwrap_or_else(|| "Choose an account".into()),
                                    )
                                    .accessibility_label("Choose mail account")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.accounts_expanded = !this.accounts_expanded;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("add-account")
                                    .custom(floating_button_variant(cx, palette, dark))
                                    .small()
                                    .icon(Icon::new(IconName::Plus).size(px(14.)))
                                    .disabled(snapshot.pending_send)
                                    .tooltip("Add mail account")
                                    .accessibility_label("Add mail account")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_setup(window, cx)
                                    })),
                            ),
                    )
                    .when(self.accounts_expanded, |column| {
                        column.children(snapshot.accounts.iter().map(|account| {
                            self.account_button(
                                account,
                                Some(account.id) == current,
                                palette,
                                dark,
                                false,
                                snapshot,
                                cx,
                            )
                        }))
                    })
                    .child(
                        Button::new("sidebar-compose")
                            .custom(floating_button_variant(cx, palette, dark))
                            .small()
                            .w_full()
                            .h(px(32.))
                            .icon(Icon::new(IconName::PenLine).size(px(14.)))
                            .label("New message")
                            .disabled(snapshot.pending_send || snapshot.accounts.is_empty())
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_new_message(window, cx)
                                }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .py_2()
                    .gap_1()
                    .children(primary.into_iter().map(|folder| {
                        self.folder_button(folder, snapshot, palette, dark, false, cx)
                    }))
                    .when(!other.is_empty(), |column| {
                        column.child(
                            v_flex()
                                .mt_3()
                                .gap_1()
                                .child(
                                    Button::new("toggle-other-folders")
                                        .custom(floating_button_variant(cx, palette, dark))
                                        .small()
                                        .w_full()
                                        .icon(
                                            Icon::new(if self.other_folders_expanded {
                                                IconName::ChevronDown
                                            } else {
                                                IconName::ChevronRight
                                            })
                                            .size(px(13.)),
                                        )
                                        .label(format!("Folders · {}", other.len()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.other_folders_expanded =
                                                !this.other_folders_expanded;
                                            cx.notify();
                                        })),
                                )
                                .when(self.other_folders_expanded, |column| {
                                    column.children(other.iter().map(|folder| {
                                        self.folder_button(
                                            folder, snapshot, palette, dark, false, cx,
                                        )
                                    }))
                                }),
                        )
                    }),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .px_2()
                    .py_2()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .h(px(26.))
                            .child(
                                Icon::new(if snapshot.loading {
                                    IconName::RefreshCw
                                } else {
                                    IconName::Mail
                                })
                                .size(px(12.))
                                .text_color(palette.muted),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child(if snapshot.loading {
                                        "Updating mail…".into()
                                    } else if snapshot.status.is_empty() {
                                        format!("{} unread", snapshot.unread_count)
                                    } else {
                                        snapshot.status.clone()
                                    }),
                            ),
                    )
                    .when(!snapshot.outbox.is_empty(), |column| {
                        column.child(
                            div()
                                .px_2()
                                .text_size(px(10.))
                                .text_color(palette.faint)
                                .child(format!("{} queued", snapshot.outbox.len())),
                        )
                    })
                    .child(
                        Button::new("open-appearance")
                            .custom(floating_button_variant(cx, palette, dark))
                            .small()
                            .w_full()
                            .h(px(32.))
                            .icon(Icon::new(IconName::Settings2).size(px(14.)))
                            .label("Appearance")
                            .selected(self.view == View::Appearance)
                            .on_click(cx.listener(|this, _, _, cx| this.open_appearance(cx))),
                    ),
            )
            .into_any_element()
    }

    fn collapsed_sidebar(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut primary_folders = snapshot
            .folders
            .iter()
            .filter(|folder| primary_folder_rank(folder.kind).is_some())
            .collect::<Vec<_>>();
        primary_folders
            .sort_by_key(|folder| primary_folder_rank(folder.kind).unwrap_or(usize::MAX));
        let mut other_folders = snapshot
            .folders
            .iter()
            .filter(|folder| primary_folder_rank(folder.kind).is_none())
            .collect::<Vec<_>>();
        other_folders.sort_by_key(|folder| folder.name.to_lowercase());

        v_flex()
            .w(px(60.))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .bg(self.pane_surface(palette.sidebar, 0.24))
            .border_r_1()
            .border_color(palette.border)
            .child(
                v_flex()
                    .id("sidebar-compact-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_3()
                    .child(
                        Button::new("add-account-compact")
                            .custom(floating_button_variant(cx, palette, dark))
                            .small()
                            .icon(Icon::new(IconName::Plus))
                            .disabled(snapshot.pending_send)
                            .tooltip("Add a mail account")
                            .accessibility_label("Add a mail account")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_setup(window, cx)),
                            ),
                    )
                    .children(snapshot.accounts.iter().map(|account| {
                        self.account_button(
                            account,
                            snapshot.current_account_id == Some(account.id),
                            palette,
                            dark,
                            true,
                            snapshot,
                            cx,
                        )
                    }))
                    .child(div().w_full().h(px(1.)).my_1().bg(palette.border))
                    .children(primary_folders.iter().map(|folder| {
                        self.folder_button(folder, snapshot, palette, dark, true, cx)
                    }))
                    .when(!other_folders.is_empty(), |column| {
                        column
                            .child(div().w_full().h(px(1.)).my_1().bg(palette.border))
                            .children(other_folders.iter().map(|folder| {
                                self.folder_button(folder, snapshot, palette, dark, true, cx)
                            }))
                    }),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .border_t_1()
                    .border_color(palette.border)
                    .py_2()
                    .child(
                        Button::new("open-appearance-compact")
                            .custom(floating_button_variant(cx, palette, dark))
                            .small()
                            .selected(self.view == View::Appearance)
                            .icon(Icon::new(IconName::Palette))
                            .tooltip("Appearance")
                            .accessibility_label("Appearance")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.composer.is_none() {
                                    this.open_appearance(cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }

    fn message_list(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.page.is_empty() {
            let message = if snapshot.loading {
                "Loading messages…"
            } else if snapshot.filter != MessageFilter::All {
                "No messages match this filter"
            } else if snapshot.search_query.is_empty() {
                "This folder is empty"
            } else {
                "No matches in loaded mail"
            };
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .px_5()
                .text_center()
                .child(
                    Icon::new(IconName::Inbox)
                        .size(px(22.))
                        .text_color(palette.faint),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(message),
                )
                .child(div().text_size(px(11.)).text_color(palette.muted).child(
                    if snapshot.filter != MessageFilter::All {
                        "Choose All or load more messages to broaden this view."
                    } else if snapshot.search_query.is_empty() {
                        "New mail will appear here when this account syncs."
                    } else {
                        "Try another search or load more messages."
                    },
                ))
                .into_any_element();
        }

        let rows = snapshot.page.clone();
        let selected = snapshot.selected_key.clone();
        let owner = cx.entity().downgrade();
        uniform_list("message-rows", rows.len(), move |range, _, _| {
            range
                .map(|index| {
                    let row = rows[index].clone();
                    let active = selected.as_ref() == Some(&row.key);
                    let owner = owner.clone();
                    let key = row.key.clone();
                    Button::new(SharedString::from(format!(
                        "message-{}-{}",
                        key.account_id, key.uid
                    )))
                    .ghost()
                    .small()
                    .selected(active)
                    .tab_stop(true)
                    .w_full()
                    .h(px(78.))
                    .rounded(px(4.))
                    .px_3()
                    .accessibility_label(format!(
                        "{}: {}",
                        message_sender(&row.message),
                        row.message.subject
                    ))
                    .on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.draft_open_error = None;
                            this.live
                                .update(cx, |live, cx| live.select_message(key.clone(), cx));
                            this.view = View::Mailbox;
                            cx.notify();
                        });
                    })
                    .child(message_row(&row.message, palette))
                    .into_any_element()
                })
                .collect()
        })
        .size_full()
        .track_scroll(&self.list_scroll)
        .into_any_element()
    }

    fn filter_menu(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let selected = snapshot.filter;
        Button::new("message-filter")
            .custom(floating_button_variant(cx, palette, self.dark))
            .small()
            .icon(Icon::new(IconName::ListFilter).size(px(15.)))
            .tooltip(format!("Filter loaded mail: {}", selected.label()))
            .accessibility_label("Filter loaded mail")
            .dropdown_menu(move |mut menu, _, _| {
                menu = menu.label("Loaded messages");
                for filter in MessageFilter::ALL {
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| div().child(filter.label()))
                            .checked(filter == selected)
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.live.update(cx, |live, cx| live.set_filter(filter, cx));
                                    this.list_scroll = UniformListScrollHandle::new();
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    fn message_pane(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let title = snapshot
            .current_folder_path
            .as_ref()
            .and_then(|path| snapshot.folders.iter().find(|folder| &folder.path == path))
            .map(|folder| folder.name.clone())
            .unwrap_or_else(|| "Mail".into());
        v_flex()
            .w(px(332.))
            .h_full()
            .flex_shrink_0()
            .min_h_0()
            .bg(self.pane_surface(palette.list, 0.10))
            .border_r_1()
            .border_color(palette.border)
            .child(
                h_flex()
                    .h(px(44.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .border_b_1()
                    .border_color(palette.border)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        Button::new("compose-new")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Mail))
                            .disabled(snapshot.pending_send)
                            .accessibility_label("Compose a new message")
                            .tooltip("Compose a new message")
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_new_message(window, cx)
                                }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Input::new(&self.search)
                                    .prefix(
                                        Icon::new(IconName::Search)
                                            .size(px(14.))
                                            .text_color(palette.faint),
                                    )
                                    .cleanable(true)
                                    .aria_label("Search loaded messages")
                                    .flex_1(),
                            )
                            .child(self.filter_menu(snapshot, palette, cx)),
                    )
                    .when_some(snapshot.error.clone(), |row, error| {
                        row.child(error_banner(
                            "Mail connection needs attention",
                            &error,
                            cx,
                            palette,
                        ))
                    })
                    .when_some(snapshot.action_error.clone(), |row, error| {
                        row.child(error_banner("Mail action failed", &error, cx, palette))
                    }),
            )
            .child(
                h_flex()
                    .h(px(28.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.faint)
                    .child(snapshot.filter.label())
                    .child(format!(
                        "{} shown · {} loaded",
                        snapshot.page.len(),
                        snapshot.loaded_count
                    )),
            )
            .child(
                div()
                    .id("message-list")
                    .flex_1()
                    .min_h_0()
                    .track_focus(&self.message_focus)
                    .key_context("MegaMailMessages")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.message_focus.focus(window, cx)),
                    )
                    .on_action(cx.listener(|this, _: &NextMessage, _, cx| {
                        this.move_selection(1, cx);
                    }))
                    .on_action(cx.listener(|this, _: &PreviousMessage, _, cx| {
                        this.move_selection(-1, cx);
                    }))
                    .child(
                        v_flex()
                            .size_full()
                            .px_2()
                            .pb_2()
                            .child(self.message_list(snapshot, palette, cx)),
                    ),
            )
            .child(
                h_flex()
                    .h(px(34.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_4()
                    .border_t_1()
                    .border_color(palette.border)
                    .text_size(px(10.))
                    .text_color(palette.muted)
                    .child(if snapshot.loading {
                        "Updating mail…".to_owned()
                    } else {
                        format!("{} unread", snapshot.unread_count)
                    })
                    .when(snapshot.has_more, |row| {
                        row.child(
                            Button::new("load-more")
                                .ghost()
                                .small()
                                .label("Load more")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.live.update(cx, |live, cx| live.load_more(cx));
                                })),
                        )
                    }),
            )
            .when(snapshot.display_cap_reached, |row| {
                row.child(
                    div()
                        .px_3()
                        .pb_2()
                        .text_size(px(10.))
                        .text_color(palette.faint)
                        .child("Showing the newest 5,000 messages."),
                )
            })
    }

    fn outbox_panel(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.outbox.is_empty() {
            return div().into_any_element();
        }
        v_flex()
            .w_full()
            .border_b_1()
            .border_color(palette.border)
            .px_6()
            .py_3()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Waiting to send"),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(palette.muted)
                            .child(format!("{} queued", snapshot.outbox.len())),
                    ),
            )
            .children(snapshot.outbox.iter().map(|item| {
                let id = item.id;
                let subject = if item.subject.trim().is_empty() {
                    "No subject".to_owned()
                } else {
                    item.subject.clone()
                };
                let subject_for_accessibility = subject.clone();
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(subject),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child(if item.last_error.is_empty() {
                                        item.recipients.clone()
                                    } else {
                                        item.last_error.clone()
                                    }),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("retry-outbox-{id}")))
                            .secondary()
                            .small()
                            .icon(Icon::new(IconName::RotateCw))
                            .label("Retry")
                            .disabled(snapshot.is_demo)
                            .accessibility_label(format!(
                                "Retry sending {subject_for_accessibility}"
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.live
                                    .update(cx, |live, cx| live.flush_outbox(Some(id), cx));
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("discard-outbox-{id}")))
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Trash))
                            .disabled(snapshot.is_demo)
                            .accessibility_label(format!(
                                "Discard queued message {subject_for_accessibility}"
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.live.update(cx, |live, cx| live.delete_outbox(id, cx));
                            })),
                    )
            }))
            .into_any_element()
    }

    fn links_panel(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if snapshot.links.is_empty() {
            return div().into_any_element();
        }
        v_flex()
            .w_full()
            .gap_2()
            .pt_6()
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Links in this message"),
            )
            .children(snapshot.links.iter().enumerate().map(|(index, link)| {
                let url = link.url.clone();
                let label = if link.label.trim().is_empty() {
                    link.url.clone()
                } else {
                    link.label.clone()
                };
                v_flex()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("message-link-{index}")))
                            .ghost()
                            .small()
                            .label(label.clone())
                            .accessibility_label(format!("Open {label} in your browser"))
                            .tooltip(format!("Open {} in your browser", link.url))
                            .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                    )
                    .child(
                        div()
                            .pl_2()
                            .text_size(px(10.))
                            .text_color(palette.faint)
                            .child(link.url.clone()),
                    )
            }))
            .into_any_element()
    }

    fn attachments_panel(
        &self,
        snapshot: &MailboxSnapshot,
        key: MessageKey,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let action = match &snapshot.attachment_state {
            AttachmentState::NotLoaded => Some(("Show attachments", false)),
            AttachmentState::Loading => Some(("Loading attachments…", false)),
            AttachmentState::DownloadRequired => Some(("Download attachments", true)),
            AttachmentState::Failed(_) => Some(("Try again", true)),
            AttachmentState::Ready { .. } => None,
        };
        let items: Option<Arc<Vec<crate::live::AttachmentSummary>>> =
            match &snapshot.attachment_state {
                AttachmentState::Ready { items, .. } => Some(items.clone()),
                _ => None,
            };
        let warning = match &snapshot.attachment_state {
            AttachmentState::Ready { warning, .. } => warning.clone(),
            _ => None,
        };
        let failure = match &snapshot.attachment_state {
            AttachmentState::Failed(error) => Some(error.clone()),
            _ => None,
        };
        if !snapshot
            .selected_message
            .as_ref()
            .is_some_and(|row| row.message.has_attachment)
        {
            return div().into_any_element();
        }

        v_flex()
            .w_full()
            .gap_2()
            .pt_6()
            .border_t_1()
            .border_color(palette.border)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Attachments"),
                    )
                    .when_some(action, |row, (label, download)| {
                        row.child(
                            Button::new("load-message-attachments")
                                .secondary()
                                .small()
                                .label(label)
                                .disabled(matches!(
                                    &snapshot.attachment_state,
                                    AttachmentState::Loading
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.live.update(cx, |live, cx| {
                                        live.request_attachments(key.clone(), download, cx);
                                    });
                                })),
                        )
                    }),
            )
            .when_some(failure, |row, error| {
                row.child(error_banner(
                    "Could not load attachments",
                    &error,
                    cx,
                    palette,
                ))
            })
            .when_some(warning, |row, warning| {
                row.child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.muted)
                        .child(warning),
                )
            })
            .when_some(items, |row, items| {
                row.children(items.iter().enumerate().map(|(index, attachment)| {
                    let name = attachment.name.clone();
                    let content = attachment.content.clone();
                    let saving = self.attachment_save_pending.as_deref() == Some(name.as_str());
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .rounded(px(8.))
                        .bg(palette.surface)
                        .child(
                            v_flex()
                                .min_w_0()
                                .flex_1()
                                .gap_1()
                                .child(div().truncate().text_size(px(11.)).child(name.clone()))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(palette.faint)
                                        .child(format!("{} bytes", content.len())),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!(
                                "save-message-attachment-{index}"
                            )))
                            .secondary()
                            .small()
                            .label(if saving { "Saving…" } else { "Save as…" })
                            .disabled(self.attachment_save_pending.is_some())
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.save_reader_attachment(
                                        name.clone(),
                                        content.clone(),
                                        window,
                                        cx,
                                    )
                                },
                            )),
                        )
                }))
            })
            .when_some(self.attachment_save_error.clone(), |row, error| {
                row.child(error_banner(
                    "Could not save attachment",
                    &error,
                    cx,
                    palette,
                ))
            })
            .when_some(self.attachment_save_status.clone(), |row, status| {
                row.child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.muted)
                        .child(status),
                )
            })
            .into_any_element()
    }

    fn reader(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = snapshot.selected_message.clone() else {
            return v_flex()
                .size_full()
                .min_w_0()
                .bg(self.pane_surface(palette.background, 0.05))
                .child(self.outbox_panel(snapshot, palette, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(
                            Icon::new(IconName::Mail)
                                .size(px(26.))
                                .text_color(palette.faint),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Select a message"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted)
                                .child("The message will open here."),
                        ),
                )
                .into_any_element();
        };
        let folder = snapshot
            .folders
            .iter()
            .find(|folder| Some(folder.path.as_str()) == snapshot.current_folder_path.as_deref());
        let is_archived = folder.is_some_and(|folder| folder.kind == FolderKind::Archive);
        let is_draft = folder.is_some_and(|folder| folder.kind == FolderKind::Drafts);
        let draft_opening = snapshot.draft_source.as_ref().is_some_and(|source| {
            source.key == row.key
                && matches!(
                    &source.state,
                    DraftSourceState::Loading | DraftSourceState::Raw(_)
                )
        });
        let can_archive = snapshot
            .folders
            .iter()
            .any(|folder| folder.kind == FolderKind::Archive);
        let can_restore = snapshot
            .folders
            .iter()
            .any(|folder| folder.kind == FolderKind::Inbox);
        let archive_label = if is_archived { "Restore" } else { "Archive" };
        let archive_icon = if is_archived {
            IconName::ArchiveRestore
        } else {
            IconName::Archive
        };
        let row_for_draft = row.as_ref().clone();
        let reply_message = row.message.clone();
        let forward_message = row.message.clone();
        v_flex()
            .size_full()
            .min_w_0()
            .bg(self.pane_surface(palette.background, 0.05))
            .child(
                h_flex()
                    .h(px(44.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .border_b_1()
                    .border_color(palette.border)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .child(
                                folder
                                    .map(|folder| folder.name.clone())
                                    .unwrap_or_else(|| "Mail".into()),
                            )
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(px(12.))
                                    .text_color(palette.faint),
                            )
                            .child(if is_draft { "Draft" } else { "Message" }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .when(!is_draft, |row| {
                                row.child(
                                    Button::new("reply-message")
                                        .secondary()
                                        .small()
                                        .icon(Icon::new(IconName::CornerUpLeft))
                                        .label("Reply")
                                        .disabled(snapshot.pending_send)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_reply(reply_message.clone(), window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("forward-message")
                                        .ghost()
                                        .small()
                                        .icon(Icon::new(IconName::CornerUpRight))
                                        .disabled(snapshot.pending_send)
                                        .accessibility_label("Forward message")
                                        .tooltip("Forward message")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_forward(forward_message.clone(), window, cx)
                                        })),
                                )
                            })
                            .when(is_draft, |row| {
                                row.child(
                                    Button::new("edit-draft")
                                        .secondary()
                                        .small()
                                        .icon(Icon::new(IconName::FileText))
                                        .label(if draft_opening {
                                            "Opening…"
                                        } else {
                                            "Edit draft"
                                        })
                                        .disabled(snapshot.pending_send || draft_opening)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_draft(row_for_draft.clone(), cx, window)
                                        })),
                                )
                            })
                            .child(
                                Button::new("archive-message")
                                    .secondary()
                                    .small()
                                    .icon(Icon::new(archive_icon))
                                    .label(archive_label)
                                    .disabled(
                                        snapshot.is_demo
                                            || if is_archived {
                                                !can_restore
                                            } else {
                                                !can_archive
                                            },
                                    )
                                    .accessibility_label(format!("{archive_label} this message"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.live.update(cx, |live, cx| {
                                            if is_archived {
                                                live.restore_selected(cx);
                                            } else {
                                                live.archive_selected(cx);
                                            }
                                        });
                                    })),
                            ),
                    ),
            )
            .child(self.outbox_panel(snapshot, palette, cx))
            .when_some(self.draft_open_error.clone(), |row, error| {
                row.child(error_banner("Could not open draft", &error, cx, palette))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .id("reader-scroll")
                    .overflow_y_scroll()
                    .items_center()
                    .px_6()
                    .py_6()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(680.))
                            .child(message_content(
                                &row.message,
                                snapshot.body_loading,
                                palette,
                            ))
                            .child(self.links_panel(snapshot, palette, cx))
                            .child(self.attachments_panel(snapshot, row.key.clone(), palette, cx)),
                    ),
            )
            .into_any_element()
    }

    fn setup_view(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(setup) = self.setup.as_ref() else {
            return v_flex().into_any_element();
        };
        let snapshot = self.live.read(cx).snapshot();
        let has_mail_accounts = !snapshot.accounts.is_empty();
        let thunderbird_waiting_for_mail = !snapshot.thunderbird_results.is_empty()
            && snapshot
                .thunderbird_results
                .iter()
                .all(|result| result.state == ThunderbirdAccountState::Connected)
            && !snapshot.accounts.is_empty()
            && snapshot.folders.is_empty()
            && snapshot.page.is_empty();
        let existing_emails = snapshot
            .accounts
            .iter()
            .map(|account| account.email.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let email = setup.inputs.email.read(cx).value().to_string();
        let candidate = setup.candidate();
        let settings_need_review = setup.manual
            || candidate.is_none()
            || candidate.is_some_and(|candidate| candidate.requires_review());
        let settings_reviewed = setup.review_is_current(cx);
        let password_label =
            if candidate.is_some_and(|candidate| candidate.app_password_help.is_some()) {
                "Google app password"
            } else {
                "Password or app password"
            };
        let password_accessible_label = password_label.to_owned();
        let username_suggestion = candidate
            .map(|candidate| candidate.incoming.username.default_for(&email))
            .unwrap_or_else(|| email.clone());
        let incoming_placeholder = candidate.map_or("imap.example.com".to_owned(), |candidate| {
            candidate.incoming.host.clone()
        });
        let incoming_port_placeholder = candidate.map_or("993".to_owned(), |candidate| {
            candidate.incoming.port.to_string()
        });
        let outgoing_placeholder = candidate.map_or("smtp.example.com".to_owned(), |candidate| {
            candidate.outgoing.host.clone()
        });
        let outgoing_port_placeholder = candidate.map_or("587".to_owned(), |candidate| {
            candidate.outgoing.port.to_string()
        });
        v_flex()
            .flex_1()
            .min_h_0()
            .bg(self.pane_surface(palette.background, 0.05))
            .child(
                v_flex()
                    .id("setup-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .items_center()
                    .px_6()
                    .py_7()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(600.))
                            .gap_5()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Icon::new(IconName::Mail)
                                            .size(px(22.))
                                            .text_color(palette.accent),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(22.))
                                            .line_height(relative(1.2))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(if has_mail_accounts {
                                                "Add another mailbox"
                                            } else {
                                                "Bring your mail"
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .max_w(px(520.))
                                    .text_size(px(13.))
                                    .line_height(relative(1.45))
                                    .text_color(palette.muted)
                                    .child(if has_mail_accounts {
                                        "Import another Thunderbird mailbox or connect a provider with IMAP and SMTP settings."
                                    } else {
                                        "Import your Thunderbird mailboxes in one pass, or connect another provider with IMAP and SMTP settings."
                                    }),
                            )
                            .when_some(self.startup_error.clone(), |row, error| {
                                row.child(error_banner("Profile storage", &error, cx, palette))
                            })
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(field_label("From Thunderbird", palette))
                                    .child(
                                        Button::new("import-thunderbird-settings")
                                            .ghost()
                                            .small()
                                            .icon(Icon::new(if setup.thunderbird_loading || snapshot.thunderbird_loading { IconName::LoaderCircle } else { IconName::Download }))
                                            .label(if setup.thunderbird_loading { "Reading Thunderbird…" } else if snapshot.thunderbird_loading { "Connecting Thunderbird…" } else { "Find mail accounts" })
                                            .disabled(setup.thunderbird_loading || setup.discovering || setup.connecting || snapshot.thunderbird_loading)
                                            .on_click(cx.listener(|this, _, _, cx| this.import_thunderbird_settings(cx))),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(540.))
                                            .text_size(px(11.))
                                            .line_height(relative(1.4))
                                            .text_color(palette.muted)
                                            .child("Use the sign-in already stored in Thunderbird. MegaMail imports selected mailboxes through an isolated profile and never displays or logs passwords or OAuth tokens."),
                                    ),
                            )
                            .when(!setup.thunderbird_profiles.is_empty(), |row| {
                                row.child(thunderbird_picker(
                                    &setup.thunderbird_profiles,
                                    &setup.selected_thunderbird,
                                    &snapshot.thunderbird_results,
                                    &existing_emails,
                                    setup.thunderbird_loading
                                        || setup.connecting
                                        || snapshot.thunderbird_loading,
                                    palette,
                                    cx,
                                ))
                            })
                            .when_some(snapshot.thunderbird_error.clone(), |row, error| {
                                row.child(error_banner(
                                    "Thunderbird access",
                                    &error,
                                    cx,
                                    palette,
                                ))
                            })
                            .when_some(snapshot.thunderbird_notice.clone(), |row, notice| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(relative(1.35))
                                        .text_color(palette.muted)
                                        .child(notice),
                                )
                            })
                            .when(thunderbird_waiting_for_mail, |row| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(relative(1.35))
                                        .text_color(palette.muted)
                                        .child("Thunderbird sign-in is ready. Loading folders and messages…"),
                                )
                            })
                            .when(
                                setup.thunderbird_profiles.is_empty()
                                    && !snapshot.thunderbird_results.is_empty(),
                                |row| {
                                    row.child(thunderbird_results_panel(
                                        &snapshot.thunderbird_results,
                                        palette,
                                    ))
                                },
                            )
                            .when_some(setup.thunderbird_error.clone(), |row, error| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(error),
                                )
                            })
                            .child(div().h(px(1.)).w_full().bg(palette.border))
                            .child(field_label("Add another mailbox", palette))
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(field_label("Email address", palette))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Input::new(&setup.inputs.email)
                                                    .content_type(InputContentType::EmailAddress)
                                                    .aria_label("Email address")
                                                    .disabled(setup.discovering || setup.connecting)
                                                    .flex_1(),
                                            )
                                            .child(
                                                Button::new("discover-account")
                                                    .secondary()
                                                    .small()
                                                    .icon(Icon::new(IconName::Search))
                                                    .label(if setup.discovering { "Looking…" } else { "Find settings" })
                                                    .disabled(setup.discovering || setup.connecting)
                                                    .on_click(cx.listener(|this, _, _, cx| this.start_discovery(cx))),
                                            ),
                                    ),
                            )
                            .when_some(setup.discovery_error.clone(), |row, error| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(error),
                                )
                            })
                            .when_some(setup.report.as_ref(), |row, report| {
                                row.child(candidate_picker(report, setup.selected_candidate, cx))
                            })
                            .when(!setup.manual && setup.report.is_none(), |row| {
                                row.child(
                                    Button::new("show-manual-settings")
                                        .ghost()
                                        .small()
                                        .label("Enter server settings manually")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(setup) = this.setup.as_mut() {
                                                setup.manual = true;
                                                setup.warning_reviewed = false;
                                                setup.reviewed_settings = None;
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when_some(candidate, |row, candidate| {
                                row.child(candidate_summary(candidate, palette))
                                    .when(candidate.oauth_advertised, |row| {
                                        row.child(
                                            div()
                                                .text_size(px(11.))
                                                .line_height(relative(1.4))
                                                .text_color(palette.muted)
                                                .child(if candidate.app_password_help.is_some() {
                                                    "Google direct OAuth is not configured in MegaMail. Use a Google app password for this IMAP connection."
                                                } else {
                                                    "This provider advertises OAuth, but MegaMail does not have its own provider registration yet."
                                                }),
                                        )
                                    })
                                    .when_some(candidate.app_password_help, |row, url| {
                                        row.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(palette.faint)
                                                .child(format!("App password help: {url}")),
                                        )
                                    })
                            })
                            .when(setup.candidate().is_some(), |row| {
                                row.child(
                                    Button::new("toggle-manual-settings")
                                        .ghost()
                                        .small()
                                        .label(if setup.manual { "Use discovered settings" } else { "Edit server settings" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(setup) = this.setup.as_mut() {
                                                setup.manual = !setup.manual;
                                                setup.reviewed_settings = None;
                                                setup.warning_reviewed = if setup.manual {
                                                    false
                                                } else {
                                                    !setup.candidate().is_some_and(|candidate| candidate.requires_review())
                                                };
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when(setup.manual, |row| {
                                row.child(
                                    v_flex()
                                        .gap_4()
                                        .border_t_1()
                                        .border_color(palette.border)
                                        .pt_4()
                                        .child(
                                            div()
                                                .text_size(px(14.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Server settings"),
                                        )
                                        .child(
                                            h_flex()
                                                .items_start()
                                                .gap_4()
                                                .child(server_fields(
                                                    "Incoming · IMAP",
                                                    &setup.inputs.imap_host,
                                                    &setup.inputs.imap_port,
                                                    &incoming_placeholder,
                                                    &incoming_port_placeholder,
                                                    setup.imap_tls,
                                                    true,
                                                    palette,
                                                    cx,
                                                ))
                                                .child(server_fields(
                                                    "Outgoing · SMTP",
                                                    &setup.inputs.smtp_host,
                                                    &setup.inputs.smtp_port,
                                                    &outgoing_placeholder,
                                                    &outgoing_port_placeholder,
                                                    setup.smtp_tls,
                                                    false,
                                                    palette,
                                                    cx,
                                                )),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(field_label("SMTP authentication", palette))
                                                .child(
                                                    Button::new("smtp-auth-password")
                                                        .ghost()
                                                        .small()
                                                        .selected(setup.smtp_auth == megamail_core::discovery::AuthMethod::Password)
                                                        .label("Password")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(setup) = this.setup.as_mut() {
                                                                setup.smtp_auth = megamail_core::discovery::AuthMethod::Password;
                                                                setup.reviewed_settings = None;
                                                                cx.notify();
                                                            }
                                                        })),
                                                )
                                                .child(
                                                    Button::new("smtp-auth-none")
                                                        .ghost()
                                                        .small()
                                                        .selected(setup.smtp_auth == megamail_core::discovery::AuthMethod::None)
                                                        .label("No login")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(setup) = this.setup.as_mut() {
                                                                setup.smtp_auth = megamail_core::discovery::AuthMethod::None;
                                                                setup.smtp_credentials_expanded = false;
                                                                setup.reviewed_settings = None;
                                                                cx.notify();
                                                            }
                                                        })),
                                                ),
                                        ),
                                )
                            })
                            .when(setup.manual || candidate.is_some(), |row| row.child(
                                v_flex()
                                    .gap_3()
                                    .border_t_1()
                                    .border_color(palette.border)
                                    .pt_4()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Sign in"),
                                    )
                                    .child(labeled_input(
                                        "Display name",
                                        Input::new(&setup.inputs.name)
                                            .content_type(InputContentType::Name)
                                            .aria_label("Display name"),
                                        palette,
                                    ))
                                    .child(labeled_input(
                                        "Username",
                                        Input::new(&setup.inputs.username)
                                            .content_type(InputContentType::Username)
                                            .aria_label("Mail server username"),
                                        palette,
                                    ))
                                    .child(labeled_input(
                                        "SMTP username",
                                        Input::new(&setup.inputs.smtp_username)
                                            .content_type(InputContentType::Username)
                                            .aria_label("SMTP username; leave blank to use the IMAP username"),
                                        palette,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(palette.faint)
                                            .child(format!("Suggested username: {username_suggestion}")),
                                    )
                                    .child(labeled_input(
                                        password_label,
                                            Input::new(&setup.inputs.password)
                                            .content_type(InputContentType::Password)
                                            .mask_toggle()
                                            .aria_label(password_accessible_label),
                                        palette,
                                    ))
                                    .when(setup.smtp_auth == megamail_core::discovery::AuthMethod::Password, |column| {
                                        column
                                            .child(
                                                Button::new("toggle-smtp-password")
                                                    .ghost()
                                                    .small()
                                                    .label(if setup.smtp_credentials_expanded { "Use the same password for SMTP" } else { "Use a different SMTP password" })
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        if let Some(setup) = this.setup.as_mut() {
                                                            setup.smtp_credentials_expanded = !setup.smtp_credentials_expanded;
                                                            if !setup.smtp_credentials_expanded {
                                                                setup.inputs.smtp_password.update(cx, |input, cx| input.set_value("", window, cx));
                                                            }
                                                            cx.notify();
                                                        }
                                                    })),
                                            )
                                            .when(setup.smtp_credentials_expanded, |column| {
                                                column.child(labeled_input(
                                                    "SMTP password",
                                                    Input::new(&setup.inputs.smtp_password)
                                                        .content_type(InputContentType::Password)
                                                        .mask_toggle()
                                                        .aria_label("Separate SMTP password"),
                                                    palette,
                                                ))
                                            })
                                    }),
                            ))
                            .when((setup.manual || candidate.is_some()) && settings_need_review, |row| {
                                row.child(
                                    Button::new("review-server-settings")
                                        .ghost()
                                        .small()
                                        .selected(settings_reviewed)
                                        .icon(Icon::new(if settings_reviewed { IconName::Check } else { IconName::Info }))
                                        .label("I reviewed these server settings")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(setup) = this.setup.as_mut() {
                                                if setup.review_is_current(cx) {
                                                    setup.warning_reviewed = false;
                                                    setup.reviewed_settings = None;
                                                } else {
                                                    setup.warning_reviewed = true;
                                                    setup.reviewed_settings = Some(setup.settings_fingerprint(cx));
                                                }
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when_some(setup.error.clone(), |row, error| {
                                row.child(error_banner("Could not add account", &error, cx, palette))
                            })
                            ,
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .border_t_1()
                    .border_color(palette.border)
                    .px_6()
                    .py_3()
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(px(600.))
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(if setup.manual || candidate.is_some() {
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child(if setup.connecting {
                                        "Testing both servers…"
                                    } else if self.loading_profiles {
                                        "Loading saved accounts…"
                                    } else {
                                        "Credentials stay in the OS keyring."
                                    })
                            } else {
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child("Find provider settings or import Thunderbird to continue.")
                            })
                            .when(has_mail_accounts, |row| {
                                row.child(
                                    Button::new("setup-back")
                                        .ghost()
                                        .small()
                                        .label(if self.profiles.is_empty() {
                                            "Open mail"
                                        } else {
                                            "Return"
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.view = this.setup_return_view;
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(setup.manual || candidate.is_some(), |row| {
                                row.child(
                                    Button::new("connect-account")
                                        .primary()
                                        .small()
                                        .icon(Icon::new(if setup.connecting {
                                            IconName::LoaderCircle
                                        } else {
                                            IconName::ArrowRight
                                        }))
                                        .label(if setup.connecting {
                                            "Testing…"
                                        } else {
                                            "Test and add"
                                        })
                                        .disabled(
                                            setup.connecting
                                                || setup.discovering
                                                || setup.thunderbird_loading
                                                || self.loading_profiles
                                                || self.startup_error.is_some()
                                                || (settings_need_review && !settings_reviewed),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| this.connect_account(cx))),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn appearance_view(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        appearance_view::render(self, palette, cx)
    }

    fn composer_view(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(composer) = self.composer.as_ref() else {
            return v_flex().into_any_element();
        };
        let profile = self
            .profiles
            .iter()
            .find(|(id, _)| *id == composer.message.from_account_id)
            .map(|(_, profile)| profile);
        let from = profile.map_or_else(
            || {
                snapshot
                    .accounts
                    .iter()
                    .find(|account| account.id == composer.message.from_account_id)
                    .map(|account| account.email.clone())
                    .unwrap_or_else(|| "Unknown account".to_owned())
            },
            |profile| profile.email.clone(),
        );
        let sender_choices = self.sender_choices(composer.message.from_account_id, snapshot, cx);
        let active_sender = sender_choices
            .iter()
            .position(|choice| choice.alias.as_deref() == composer.message.from_alias.as_deref())
            .unwrap_or(0);
        let sender_label = sender_choices
            .get(active_sender)
            .map(|choice| choice.label.clone())
            .unwrap_or(from);
        let account_id = composer.message.from_account_id;
        let from_control = if sender_choices.len() > 1 {
            Button::new("cycle-sender-identity")
                .ghost()
                .small()
                .icon(Icon::new(IconName::ChevronDown))
                .label(format!("From {sender_label}"))
                .disabled(!composer.can_edit())
                .accessibility_label("Switch sender address")
                .tooltip("Use another sender address")
                .on_click(
                    cx.listener(move |this, _, _, cx| this.cycle_sender_identity(account_id, cx)),
                )
                .into_any_element()
        } else {
            div()
                .text_size(px(11.))
                .text_color(palette.muted)
                .child(format!("From {sender_label}"))
                .into_any_element()
        };
        let composer_saved = composer.is_saved(cx);
        let composer_done = composer.send_queued || composer_saved;
        v_flex()
            .size_full()
            .min_w_0()
            .bg(palette.background)
            .child(
                h_flex()
                    .h(px(44.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .border_b_1()
                    .border_color(palette.border)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("New message"),
                            )
                            .child(from_control),
                    )
                    .child(
                        Button::new("close-composer")
                            .ghost()
                            .small()
                            .icon(Icon::new(if composer_done {
                                IconName::Check
                            } else {
                                IconName::Trash
                            }))
                            .label(if composer_done { "Done" } else { "Discard" })
                            .disabled(!composer.can_close())
                            .accessibility_label(if composer.send_queued {
                                "Close composer; queued message remains in Outbox"
                            } else if composer_saved {
                                "Close composer; saved draft remains in Drafts"
                            } else {
                                "Discard this unsent message"
                            })
                            .tooltip(if composer.send_queued {
                                "Close composer; queued message remains in Outbox"
                            } else if composer_saved {
                                "Close composer; saved draft remains in Drafts"
                            } else {
                                "Discard this unsent message"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.composer.as_ref().is_some_and(Composer::can_close) {
                                    this.cleanup_composer_attachments(cx);
                                    this.composer = None;
                                    this.view = View::Mailbox;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .id("composer-scroll")
                    .overflow_y_scroll()
                    .items_center()
                    .px_6()
                    .py_5()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(680.))
                            .gap_3()
                            .child(labeled_input(
                                "To",
                                Input::new(&composer.to)
                                    .aria_label("To")
                                    .disabled(!composer.can_edit()),
                                palette,
                            ))
                            .child(labeled_input(
                                "Cc",
                                Input::new(&composer.cc)
                                    .aria_label("Cc")
                                    .disabled(!composer.can_edit()),
                                palette,
                            ))
                            .child(labeled_input(
                                "Bcc",
                                Input::new(&composer.bcc)
                                    .aria_label("Bcc")
                                    .disabled(!composer.can_edit()),
                                palette,
                            ))
                            .child(labeled_input(
                                "Subject",
                                Input::new(&composer.subject)
                                    .aria_label("Subject")
                                    .disabled(!composer.can_edit()),
                                palette,
                            ))
                            .child(
                                Textarea::new(&composer.body)
                                    .h(px(360.))
                                    .aria_label("Message body")
                                    .disabled(!composer.can_edit()),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("add-attachments")
                                            .secondary()
                                            .small()
                                            .icon(Icon::new(IconName::FileText))
                                            .label(if composer.selecting_attachments {
                                                "Choosing files…"
                                            } else {
                                                "Attach files"
                                            })
                                            .disabled(!composer.can_edit())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.choose_attachments(window, cx)
                                            })),
                                    )
                                    .child(
                                        div().text_size(px(11.)).text_color(palette.muted).child(
                                            if composer.message.attachments.is_empty() {
                                                "No files attached".to_owned()
                                            } else {
                                                format!(
                                                    "{} file{} attached",
                                                    composer.message.attachments.len(),
                                                    if composer.message.attachments.len() == 1 {
                                                        ""
                                                    } else {
                                                        "s"
                                                    }
                                                )
                                            },
                                        ),
                                    ),
                            )
                            .children(composer.message.attachments.iter().enumerate().map(
                                |(index, path)| {
                                    let name = Path::new(path)
                                        .file_name()
                                        .and_then(|name| name.to_str())
                                        .unwrap_or("Attachment")
                                        .to_owned();
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(8.))
                                        .bg(palette.surface)
                                        .child(
                                            div()
                                                .min_w_0()
                                                .flex_1()
                                                .truncate()
                                                .text_size(px(11.))
                                                .child(name.clone()),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "remove-attachment-{index}"
                                            )))
                                            .ghost()
                                            .small()
                                            .label("Remove")
                                            .disabled(!composer.can_edit())
                                            .accessibility_label(format!(
                                                "Remove attachment {name}"
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(composer) = this.composer.as_mut() {
                                                    if composer.can_edit()
                                                        && index
                                                            < composer.message.attachments.len()
                                                    {
                                                        composer.message.attachments.remove(index);
                                                        composer.draft_saved = None;
                                                    }
                                                }
                                                cx.notify();
                                            })),
                                        )
                                },
                            ))
                            .when_some(composer.attachment_error.clone(), |row, error| {
                                row.child(error_banner("Attachment", &error, cx, palette))
                            })
                            .when_some(composer.notice.clone(), |row, notice| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(notice),
                                )
                            })
                            .when(composer.send_uncertain, |row| {
                                row.child(
                                    v_flex()
                                        .w_full()
                                        .gap_2()
                                        .rounded(px(8.))
                                        .border_1()
                                        .border_color(palette.border)
                                        .bg(palette.surface)
                                        .px_3()
                                        .py_3()
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Delivery not confirmed"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .line_height(relative(1.4))
                                                .text_color(palette.muted)
                                                .child("Thunderbird did not confirm whether the server accepted this message. Check Sent before trying again. This message is locked to prevent a duplicate send."),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Button::new("copy-uncertain-message")
                                                        .ghost()
                                                        .small()
                                                        .icon(Icon::new(IconName::Copy))
                                                        .label("Copy message text")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            let Some(composer) =
                                                                this.composer.as_ref()
                                                            else {
                                                                return;
                                                            };
                                                            if !composer.send_uncertain {
                                                                return;
                                                            }
                                                            let message = composer.outgoing(cx);
                                                            let mut text = format!(
                                                                "To: {}\nCc: {}\nBcc: {}\nSubject: {}\n\n{}",
                                                                message.to,
                                                                message.cc,
                                                                message.bcc,
                                                                message.subject,
                                                                message.body,
                                                            );
                                                            if !message.attachments.is_empty() {
                                                                let files = message
                                                                    .attachments
                                                                    .iter()
                                                                    .filter_map(|path| {
                                                                        Path::new(path)
                                                                            .file_name()
                                                                            .and_then(|name| name.to_str())
                                                                    })
                                                                    .collect::<Vec<_>>()
                                                                    .join(", ");
                                                                text.push_str(&format!(
                                                                    "\n\nAttachments not copied: {files}"
                                                                ));
                                                            }
                                                            cx.write_to_clipboard(
                                                                ClipboardItem::new_string(text),
                                                            );
                                                        })),
                                                )
                                                .child(
                                                    Button::new("confirm-not-sent")
                                                        .secondary()
                                                        .small()
                                                        .label("Confirmed not sent")
                                                        .tooltip("Only choose this after checking the Sent folder")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.confirm_send_not_delivered(cx)
                                                        })),
                                                ),
                                        ),
                                )
                            })
                            .when_some(composer.error.clone(), |row, error| {
                                row.child(error_banner("Message not sent", &error, cx, palette))
                            })
                            .when(!composer.send_uncertain && !snapshot.status.is_empty(), |row| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(snapshot.status.clone()),
                                )
                            })
                            .when_some(
                                (!composer.send_uncertain)
                                    .then(|| snapshot.action_error.clone())
                                    .flatten(),
                                |row, error| {
                                row.child(error_banner("Mail action failed", &error, cx, palette))
                                },
                            ),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .border_t_1()
                    .border_color(palette.border)
                    .px_6()
                    .py_3()
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(px(680.))
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(palette.faint)
                                    .child("Plain-text message"),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("save-draft")
                                            .secondary()
                                            .small()
                                            .icon(Icon::new(IconName::FileText))
                                            .label(if composer.draft_pending.is_some() {
                                                "Saving…"
                                            } else if composer_saved {
                                                "Saved"
                                            } else {
                                                "Save draft"
                                            })
                                            .disabled(
                                                self.demo || !composer.can_edit() || composer_saved,
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_current_draft(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("send-message")
                                            .primary()
                                            .small()
                                            .icon(Icon::new(IconName::Send))
                                            .label(if composer.send_request_id.is_some() {
                                                "Sending…"
                                            } else if composer.send_queued {
                                                "In Outbox"
                                            } else {
                                                "Send"
                                            })
                                            .disabled(self.demo || !composer.can_edit())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.send_current_message(cx)
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn send_current_message(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        if self.demo {
            composer.error = Some("Sending is disabled in the local demo.".into());
            cx.notify();
            return;
        }
        if !composer.can_edit() {
            return;
        }
        let message = composer.outgoing(cx);
        if message.to.trim().is_empty() {
            composer.error = Some("Add at least one recipient before sending.".into());
            cx.notify();
            return;
        }
        composer.error = None;
        composer.send_queued = false;
        let request_id = self
            .live
            .update(cx, |live, cx| live.send_message(message, cx));
        if let Some(request_id) = request_id {
            if let Some(composer) = self.composer.as_mut() {
                composer.send_request_id = Some(request_id);
            }
        }
        cx.notify();
    }

    fn save_current_draft(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.as_ref() else {
            return;
        };
        if self.demo {
            if let Some(composer) = self.composer.as_mut() {
                composer.error = Some("Draft saving is disabled in the local demo.".into());
            }
            cx.notify();
            return;
        }
        if !composer.can_edit() {
            return;
        }
        let message = composer.outgoing(cx);
        let pending_message = message.clone();
        let accepted = self
            .live
            .update(cx, |live, cx| live.save_draft(message, None, cx));
        if accepted {
            if let Some(composer) = self.composer.as_mut() {
                composer.draft_pending = Some(pending_message);
                composer.error = None;
            }
        }
        cx.notify();
    }

    fn choose_attachments(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        if !composer.can_edit() {
            return;
        }
        composer.selecting_attachments = true;
        composer.attachment_error = None;
        let existing = composer.message.attachments.clone();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach files to this message".into()),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let selection = picker.await;
            let selected = match selection {
                Ok(Ok(Some(paths))) => Some(paths),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(composer) = this.composer.as_mut() {
                            composer.selecting_attachments = false;
                            composer.attachment_error =
                                Some(format!("The file picker could not open: {error}"));
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(selected) = selected else {
                let _ = this.update(cx, |this, cx| {
                    if let Some(composer) = this.composer.as_mut() {
                        composer.selecting_attachments = false;
                    }
                    cx.notify();
                });
                return;
            };
            let validation = cx
                .background_spawn(async move { validate_attachment_paths(&existing, selected) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(composer) = this.composer.as_mut() {
                    composer.selecting_attachments = false;
                    match validation {
                        Ok(paths) => {
                            composer.message.attachments.extend(paths);
                            composer.draft_saved = None;
                            composer.attachment_error = None;
                        }
                        Err(error) => composer.attachment_error = Some(error),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_reader_attachment(
        &mut self,
        name: String,
        content: Arc<Vec<u8>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.attachment_save_pending.is_some() {
            return;
        }
        self.attachment_save_pending = Some(name.clone());
        self.attachment_save_error = None;
        self.attachment_save_status = None;
        let suggested_name = safe_attachment_filename(&name);
        let directory = std::env::var_os("XDG_DOWNLOAD_DIR")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let picker = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let selected_path = match picker.await {
                Ok(Ok(Some(path))) => Some(path),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.attachment_save_pending = None;
                        this.attachment_save_error =
                            Some(format!("The save dialog could not open: {error}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = selected_path else {
                let _ = this.update(cx, |this, cx| {
                    this.attachment_save_pending = None;
                    cx.notify();
                });
                return;
            };
            let write_result = cx
                .background_spawn(async move {
                    std::fs::write(&path, content.as_slice())
                        .map(|()| path)
                        .map_err(|error| format!("Could not save the selected file: {error}"))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.attachment_save_pending = None;
                match write_result {
                    Ok(path) => {
                        let saved_name = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("attachment")
                            .to_owned();
                        this.attachment_save_status = Some(format!("Saved {saved_name}"));
                    }
                    Err(error) => this.attachment_save_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

const MAX_COMPOSE_ATTACHMENTS: usize = 20;
const MAX_COMPOSE_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_COMPOSE_ATTACHMENT_TOTAL_BYTES: u64 = 50 * 1024 * 1024;

fn safe_attachment_filename(name: &str) -> String {
    let cleaned = name
        .chars()
        .filter(|character| !character.is_control())
        .map(|character| {
            if matches!(character, '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .take(180)
        .collect::<String>();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "attachment".into()
    } else {
        cleaned.to_owned()
    }
}

fn validate_attachment_paths(
    existing: &[String],
    selected: Vec<PathBuf>,
) -> Result<Vec<String>, String> {
    let mut seen = existing
        .iter()
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .collect::<std::collections::HashSet<_>>();
    let mut count = existing.len();
    let mut total_bytes = existing
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum::<u64>();
    let mut accepted = Vec::new();

    for path in selected {
        let metadata = std::fs::metadata(&path).map_err(|error| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("file");
            format!("Could not access {name}: {error}")
        })?;
        if !metadata.is_file() {
            return Err("Choose files, not folders or devices.".into());
        }
        if metadata.len() > MAX_COMPOSE_ATTACHMENT_BYTES {
            return Err(format!(
                "{} is larger than the 25 MiB per-file limit.",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("This file")
            ));
        }
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen.insert(canonical) {
            continue;
        }
        count += 1;
        total_bytes = total_bytes.saturating_add(metadata.len());
        if count > MAX_COMPOSE_ATTACHMENTS {
            return Err("A message can have up to 20 attachments.".into());
        }
        if total_bytes > MAX_COMPOSE_ATTACHMENT_TOTAL_BYTES {
            return Err("Attachments cannot total more than 50 MiB.".into());
        }
        accepted.push(path.to_string_lossy().into_owned());
    }
    Ok(accepted)
}

impl Render for MailApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let palette = self.palette;
        let snapshot = self.live.read(cx).snapshot();
        let client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let context = if self.demo {
            "Local demo · fictional mail · sending disabled".to_owned()
        } else if self.view == View::Setup {
            if snapshot.accounts.is_empty() {
                "Set up a mail account".to_owned()
            } else {
                "Account settings".to_owned()
            }
        } else if self.view == View::Compose {
            "Compose".to_owned()
        } else if self.view == View::Appearance {
            "Appearance".to_owned()
        } else {
            snapshot
                .current_account_id
                .and_then(|id| snapshot.accounts.iter().find(|account| account.id == id))
                .map(|account| account.email.clone())
                .unwrap_or_else(|| "Mail".to_owned())
        };
        let toolbar = h_flex()
            .w_full()
            .h(px(38.))
            .bg(palette.background)
            .px_4()
            .items_center()
            .justify_between()
            .when(client_decorated, |row| {
                row.child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::Mail)
                                .size(px(15.))
                                .text_color(palette.accent),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("MegaMail"),
                        ),
                )
            })
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(palette.muted)
                    .child(context),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .when(
                        !snapshot.accounts.is_empty() && self.view != View::Setup,
                        |row| {
                            row.child(
                                Button::new("toggle-sidebar")
                                    .custom(floating_button_variant(cx, palette, dark))
                                    .small()
                                    .icon(Icon::new(if self.sidebar_collapsed {
                                        IconName::PanelLeftOpen
                                    } else {
                                        IconName::PanelLeftClose
                                    }))
                                    .accessibility_label(if self.sidebar_collapsed {
                                        "Expand navigation sidebar"
                                    } else {
                                        "Collapse navigation sidebar"
                                    })
                                    .tooltip(if self.sidebar_collapsed {
                                        "Expand navigation sidebar"
                                    } else {
                                        "Collapse navigation sidebar"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.sidebar_collapsed = !this.sidebar_collapsed;
                                        cx.notify();
                                    })),
                            )
                        },
                    )
                    .when(
                        self.view == View::Mailbox && !snapshot.accounts.is_empty(),
                        |row| {
                            row.child(
                                Button::new("refresh-mail")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(IconName::RefreshCw))
                                    .accessibility_label("Refresh mail")
                                    .tooltip("Refresh mail")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.live.update(cx, |live, cx| live.refresh(cx));
                                    })),
                            )
                        },
                    )
                    .child(
                        Button::new("toggle-appearance")
                            .custom(floating_button_variant(cx, palette, dark))
                            .small()
                            .selected(self.view == View::Appearance)
                            .icon(Icon::new(IconName::Palette))
                            .accessibility_label(if self.view == View::Appearance {
                                "Return from appearance settings"
                            } else {
                                "Open appearance settings"
                            })
                            .tooltip("Appearance")
                            .on_click(cx.listener(|this, _, _, cx| this.open_appearance(cx))),
                    )
                    .child(
                        Button::new("toggle-theme")
                            .ghost()
                            .small()
                            .disabled(
                                self.appearance_busy || self.appearance_loading || self.theme_busy,
                            )
                            .icon(Icon::new(if dark { IconName::Sun } else { IconName::Moon }))
                            .accessibility_label(if dark {
                                "Switch to light theme"
                            } else {
                                "Switch to dark theme"
                            })
                            .tooltip(if dark {
                                "Switch to light theme"
                            } else {
                                "Switch to dark theme"
                            })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_theme(window, cx)),
                            ),
                    ),
            );
        let chrome = if client_decorated {
            TitleBar::new().h(px(38.)).child(toolbar).into_any_element()
        } else {
            toolbar.into_any_element()
        };
        let body = if self.view == View::Appearance {
            h_flex()
                .flex_1()
                .min_h_0()
                .child(self.sidebar(&snapshot, palette, cx))
                .child(self.appearance_view(palette, cx))
                .into_any_element()
        } else if self.view == View::Setup || self.loading_profiles {
            if self.loading_profiles {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_color(palette.muted)
                    .child("Loading MegaMail profiles…")
                    .into_any_element()
            } else {
                self.setup_view(palette, cx)
            }
        } else {
            h_flex()
                .flex_1()
                .min_h_0()
                .child(self.sidebar(&snapshot, palette, cx))
                .child(self.message_pane(&snapshot, palette, cx))
                .child(if self.view == View::Compose {
                    self.composer_view(&snapshot, palette, cx)
                } else {
                    self.reader(&snapshot, palette, cx)
                })
                .into_any_element()
        };
        window_border().child(
            v_flex()
                .size_full()
                .relative()
                .bg(palette.background)
                .text_color(palette.text)
                .child(zeron_background::background_element(
                    &self.appearance,
                    dark,
                    palette.accent,
                    palette.background,
                ))
                .child(chrome)
                .child(body),
        )
    }
}

fn message_row(message: &Message, palette: Palette) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_1()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .when(message.unread, |row| {
                            row.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(palette.accent))
                        })
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .font_weight(if message.unread {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(palette.text)
                                .child(message_sender(message)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(palette.faint)
                        .child(message.date.clone()),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .font_weight(if message.unread {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if message.unread {
                            palette.text
                        } else {
                            palette.muted
                        })
                        .child(if message.subject.trim().is_empty() {
                            "(No subject)".to_owned()
                        } else {
                            message.subject.clone()
                        }),
                )
                .when(message.starred, |row| {
                    row.child(
                        Icon::new(IconName::StarFill)
                            .size(px(12.))
                            .text_color(palette.accent),
                    )
                })
                .when(message.has_attachment, |row| {
                    row.child(
                        Icon::new(IconName::Paperclip)
                            .size(px(12.))
                            .text_color(palette.faint),
                    )
                }),
        )
        .child(
            div()
                .w_full()
                .truncate()
                .text_size(px(11.))
                .text_color(palette.muted)
                .child(message.preview.clone()),
        )
}

fn message_content(message: &Message, body_loading: bool, palette: Palette) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_5()
        .child(
            div()
                .text_size(px(22.))
                .line_height(relative(1.2))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text)
                .child(if message.subject.trim().is_empty() {
                    "(No subject)".to_owned()
                } else {
                    message.subject.clone()
                }),
        )
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size_9()
                        .rounded_full()
                        .bg(palette.selected)
                        .items_center()
                        .justify_center()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.muted)
                        .child(initials(&message_sender(message))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(message_sender(message)),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(palette.muted)
                                .truncate()
                                .child(if message.from_addr.is_empty() {
                                    message.to.clone()
                                } else {
                                    message.from_addr.clone()
                                }),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.faint)
                        .child(message.date.clone()),
                ),
        )
        .child(div().h(px(1.)).bg(palette.border))
        .when(message.body.is_empty() && body_loading, |row| {
            row.child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.muted)
                    .child("Loading message body…"),
            )
        })
        .when(message.body.is_empty() && !body_loading, |row| {
            row.child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.muted)
                    .child("This message has no plain-text body."),
            )
        })
        .when(!message.body.is_empty(), |row| {
            row.child(
                div()
                    .text_size(px(14.))
                    .line_height(relative(1.5))
                    .text_color(palette.text)
                    .child(message.body.clone()),
            )
        })
}

fn candidate_picker(
    report: &DiscoveryReport,
    selected: usize,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let email = report.email.clone();
    v_flex()
        .gap_1()
        .when(!report.candidates.is_empty(), |row| {
            row.child(
                div()
                    .pb_1()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child("Mail settings found"),
            )
            .children(report.candidates.iter().enumerate().map(|(index, candidate)| {
                let name = candidate.provider_name.clone();
                let source = source_label(candidate.source);
                let email = email.clone();
                Button::new(SharedString::from(format!("server-candidate-{index}")))
                    .ghost()
                    .small()
                    .selected(index == selected)
                    .w_full()
                    .px_2()
                    .accessibility_label(format!("Use {name} settings from {source}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(setup) = this.setup.as_mut() {
                            setup.selected_candidate = index;
                            if let Some((imap_tls, smtp_tls, smtp_auth, needs_review, smtp_username)) = setup.candidate().map(|candidate| {
                                (
                                    candidate.incoming.tls,
                                    candidate.outgoing.tls,
                                    candidate.outgoing.auth,
                                    candidate.requires_review(),
                                    candidate.outgoing.username.default_for(&email),
                                )
                            }) {
                                setup.imap_tls = imap_tls;
                                setup.smtp_tls = smtp_tls;
                                setup.smtp_auth = smtp_auth;
                                setup.warning_reviewed = !needs_review;
                                setup.reviewed_settings = None;
                                setup.inputs.smtp_username.update(cx, |input, cx| {
                                    input.set_value(smtp_username, window, cx)
                                });
                            }
                            cx.notify();
                        }
                    }))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(source),
                            ),
                    )
            }))
        })
        .when(report.candidates.is_empty(), |row| {
            row.child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child("No automatic settings were found. Enter the incoming and outgoing servers manually."),
            )
        })
}

fn candidate_summary(
    candidate: &megamail_core::discovery::ServerConfigCandidate,
    palette: Palette,
) -> impl IntoElement {
    v_flex()
        .gap_2()
        .border_t_1()
        .border_color(palette.border)
        .pt_3()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(candidate.provider_name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(palette.muted)
                        .child(source_label(candidate.source)),
                ),
        )
        .child(endpoint_summary("IMAP", &candidate.incoming, palette))
        .child(endpoint_summary("SMTP", &candidate.outgoing, palette))
        .children(candidate.warnings.iter().map(|warning| {
            div()
                .text_size(px(11.))
                .line_height(relative(1.35))
                .text_color(palette.muted)
                .child(warning_text(warning))
        }))
}

fn endpoint_summary(
    label: &'static str,
    server: &MailServer,
    palette: Palette,
) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap_2()
        .text_size(px(11.))
        .child(div().w(px(42.)).text_color(palette.faint).child(label))
        .child(
            div()
                .text_color(palette.text)
                .child(format!("{}:{}", server.host, server.port)),
        )
        .child(div().text_color(palette.muted).child(tls_label(server.tls)))
}

fn server_fields(
    title: &'static str,
    host: &Entity<InputState>,
    port: &Entity<InputState>,
    host_placeholder: &str,
    port_placeholder: &str,
    tls: TlsMode,
    incoming: bool,
    palette: Palette,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let host_placeholder = host_placeholder.to_owned();
    let port_placeholder = port_placeholder.to_owned();
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .child(labeled_input(
            "Server",
            Input::new(host).aria_label(format!("{title} server host")),
            palette,
        ))
        .child(
            div()
                .text_size(px(10.))
                .text_color(palette.faint)
                .child(format!("Suggested host: {host_placeholder}")),
        )
        .child(labeled_input(
            "Port",
            Input::new(port).aria_label(format!("{title} server port")),
            palette,
        ))
        .child(
            div()
                .text_size(px(10.))
                .text_color(palette.faint)
                .child(format!("Suggested port: {port_placeholder}")),
        )
        .child(
            v_flex()
                .gap_1()
                .child(field_label("Encryption", palette))
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new(SharedString::from(format!("tls-implicit-{incoming}")))
                                .ghost()
                                .small()
                                .selected(tls == TlsMode::ImplicitTls)
                                .label("TLS")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(setup) = this.setup.as_mut() {
                                        if incoming {
                                            setup.imap_tls = TlsMode::ImplicitTls
                                        } else {
                                            setup.smtp_tls = TlsMode::ImplicitTls
                                        }
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("tls-starttls-{incoming}")))
                                .ghost()
                                .small()
                                .selected(tls == TlsMode::StartTls)
                                .label("STARTTLS")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(setup) = this.setup.as_mut() {
                                        if incoming {
                                            setup.imap_tls = TlsMode::StartTls
                                        } else {
                                            setup.smtp_tls = TlsMode::StartTls
                                        }
                                        cx.notify();
                                    }
                                })),
                        ),
                ),
        )
}

fn thunderbird_account_email(
    account: &megamail_core::thunderbird::ThunderbirdAccount,
) -> Option<String> {
    account
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| discovery::validate_email_address(email).is_ok())
        .or_else(|| {
            account.identities.iter().find_map(|identity| {
                identity
                    .email
                    .as_deref()
                    .map(str::trim)
                    .filter(|email| discovery::validate_email_address(email).is_ok())
            })
        })
        .map(str::to_owned)
}

fn default_thunderbird_selection(
    profiles: &[ThunderbirdProfile],
    existing_emails: &HashSet<String>,
) -> HashSet<(PathBuf, String)> {
    let mut selected = HashSet::new();
    for profile in profiles {
        for account in &profile.accounts {
            let Some(email) = thunderbird_account_email(account) else {
                continue;
            };
            if !existing_emails.contains(&email.to_ascii_lowercase()) {
                selected.insert((profile.path.clone(), account.account_id.clone()));
            }
        }
    }
    selected
}

fn thunderbird_result_message(outcome: &ThunderbirdAccountOutcome) -> Option<String> {
    match outcome.state {
        ThunderbirdAccountState::Connected => None,
        ThunderbirdAccountState::NeedsLogin => Some(
            "Thunderbird’s isolated profile needs a login or primary-password unlock. MegaMail cannot show that prompt yet, so this account was not imported. The original Thunderbird profile was not changed."
                .into(),
        ),
        ThunderbirdAccountState::Unsupported => Some(
            outcome
                .message
                .clone()
                .unwrap_or_else(|| "This Thunderbird account type is not supported yet.".into()),
        ),
        ThunderbirdAccountState::Failed => Some(
            outcome
                .message
                .clone()
                .unwrap_or_else(|| "MegaMail could not start this Thunderbird account.".into()),
        ),
    }
}

fn thunderbird_results_panel(
    results: &[ThunderbirdAccountOutcome],
    palette: Palette,
) -> impl IntoElement {
    let rows = results.iter().filter_map(|outcome| {
        thunderbird_result_message(outcome).map(|message| {
            h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .text_size(px(11.))
                .text_color(palette.muted)
                .child(div().w(px(176.)).min_w_0().child(outcome.email.clone()))
                .child(div().flex_1().min_w_0().child(message))
        })
    });
    v_flex()
        .w_full()
        .items_start()
        .gap_2()
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .child("Accounts that need attention"),
        )
        .children(rows)
}

fn thunderbird_picker(
    profiles: &[ThunderbirdProfile],
    selected: &HashSet<(PathBuf, String)>,
    results: &[ThunderbirdAccountOutcome],
    existing_emails: &HashSet<String>,
    loading: bool,
    palette: Palette,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let eligible_count = profiles
        .iter()
        .flat_map(|profile| profile.accounts.iter())
        .filter(|account| {
            thunderbird_account_email(account)
                .is_some_and(|email| !existing_emails.contains(&email.to_ascii_lowercase()))
        })
        .count();
    let importable_count = selected.len();
    let select_all = importable_count < eligible_count;
    let mut groups = Vec::new();
    for (profile_index, profile) in profiles.iter().enumerate() {
        let mut rows = Vec::new();
        for (account_index, account) in profile.accounts.iter().enumerate() {
            let Some(email) = thunderbird_account_email(account) else {
                continue;
            };
            let key = (profile.path.clone(), account.account_id.clone());
            let is_selected = selected.contains(&key);
            let already_imported = existing_emails.contains(&email.to_ascii_lowercase());
            let profile_path = profile.path.clone();
            let account_id = account.account_id.clone();
            let outgoing = account
                .outgoing
                .as_ref()
                .map(|server| server.host.as_str())
                .unwrap_or("SMTP not configured");
            let endpoint_summary = format!("IMAP {} · SMTP {outgoing}", account.incoming.host);
            let result = results.iter().find(|result| {
                result.profile_path == profile.path
                    && result.source_account_id == account.account_id
            });

            rows.push(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!(
                            "thunderbird-account-{profile_index}-{account_index}"
                        )))
                        .ghost()
                        .small()
                        .selected(is_selected || already_imported)
                        .w_full()
                        .icon(Icon::new(if is_selected || already_imported {
                            IconName::Check
                        } else {
                            IconName::Mail
                        }))
                        .label(email.clone())
                        .accessibility_label(format!(
                            "{} Thunderbird account {email}",
                            if already_imported {
                                "Already added"
                            } else if is_selected {
                                "Deselect"
                            } else {
                                "Select"
                            }
                        ))
                        .disabled(loading || already_imported)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_thunderbird_selection(
                                profile_path.clone(),
                                account_id.clone(),
                                cx,
                            )
                        })),
                    )
                    .child(
                        div()
                            .pl(px(34.))
                            .text_size(px(10.5))
                            .text_color(palette.muted)
                            .child(if already_imported {
                                "Already in MegaMail".to_owned()
                            } else {
                                endpoint_summary
                            }),
                    )
                    .when_some(
                        result.and_then(thunderbird_result_message),
                        |row, message| {
                            row.child(
                                div()
                                    .pl(px(34.))
                                    .text_size(px(10.5))
                                    .line_height(relative(1.35))
                                    .text_color(palette.muted)
                                    .child(message),
                            )
                        },
                    )
                    .into_any_element(),
            );
        }
        if !rows.is_empty() {
            let profile_name = profile.name.clone();
            groups.push(
                v_flex()
                    .w_full()
                    .items_start()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.muted)
                            .child(profile_name),
                    )
                    .children(rows),
            );
        }
    }

    let import_label = if loading {
        "Connecting to Thunderbird…".to_owned()
    } else {
        format!("Import all selected · {importable_count}")
    };
    v_flex()
        .w_full()
        .items_start()
        .gap_2()
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Thunderbird mailboxes"),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(palette.muted)
                                .child(format!("{importable_count} of {eligible_count} selected")),
                        ),
                )
                .child(
                    Button::new("toggle-all-thunderbird")
                        .ghost()
                        .small()
                        .label(if select_all { "Select all" } else { "Clear" })
                        .disabled(loading || eligible_count == 0)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_thunderbird_selection(select_all, cx)
                        })),
                ),
        )
        .children(groups)
        .child(
            Button::new("import-selected-thunderbird")
                .primary()
                .small()
                .icon(Icon::new(if loading {
                    IconName::LoaderCircle
                } else {
                    IconName::Download
                }))
                .label(import_label)
                .disabled(loading || importable_count == 0)
                .on_click(cx.listener(|this, _, _, cx| this.import_selected_thunderbird(cx))),
        )
}

fn new_input(
    window: &mut Window,
    cx: &mut Context<MailApp>,
    placeholder: &'static str,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}

fn new_input_value(
    window: &mut Window,
    cx: &mut Context<MailApp>,
    placeholder: &'static str,
    value: String,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

fn labeled_input(
    label: &'static str,
    input: impl IntoElement,
    palette: Palette,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(field_label(label, palette))
        .child(input)
}

fn field_label(label: &'static str, palette: Palette) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .text_color(palette.muted)
        .child(label)
}

fn error_banner(
    heading: &'static str,
    detail: &str,
    cx: &mut Context<MailApp>,
    palette: Palette,
) -> impl IntoElement {
    h_flex()
        .items_start()
        .gap_2()
        .px_2()
        .py_2()
        .rounded(px(6.))
        .bg(palette.hover)
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(px(13.))
                .text_color(palette.accent),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(heading),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .line_height(relative(1.35))
                        .text_color(palette.muted)
                        .child(detail.to_owned()),
                ),
        )
        .child(
            Button::new(SharedString::from(format!("dismiss-error-{heading}")))
                .ghost()
                .small()
                .icon(Icon::new(IconName::Close))
                .accessibility_label("Dismiss error")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.live.update(cx, |live, cx| live.dismiss_error(cx));
                    if let Some(setup) = this.setup.as_mut() {
                        setup.error = None;
                    }
                    cx.notify();
                })),
        )
}

fn folder_icon(kind: FolderKind) -> IconName {
    match kind {
        FolderKind::Inbox => IconName::Inbox,
        FolderKind::Starred => IconName::Star,
        FolderKind::Sent => IconName::Send,
        FolderKind::Drafts | FolderKind::Templates => IconName::FileText,
        FolderKind::Archive => IconName::Archive,
        FolderKind::Junk => IconName::TriangleAlert,
        FolderKind::Trash => IconName::Trash,
        FolderKind::Custom => IconName::Folder,
    }
}

fn primary_folder_rank(kind: FolderKind) -> Option<usize> {
    match kind {
        FolderKind::Inbox => Some(0),
        FolderKind::Starred => Some(1),
        FolderKind::Drafts => Some(2),
        FolderKind::Sent => Some(3),
        FolderKind::Archive => Some(4),
        FolderKind::Templates | FolderKind::Junk | FolderKind::Trash | FolderKind::Custom => None,
    }
}

fn sidebar_section_label(label: &'static str, palette: Palette) -> impl IntoElement {
    div()
        .px_2()
        .text_size(px(zeron_style::text_role_size(
            zeron_style::TextRole::Tertiary,
        )))
        .font_weight(FontWeight::MEDIUM)
        .text_color(zeron_style::text_role_color(
            &palette,
            zeron_style::TextRole::Tertiary,
        ))
        .child(label)
}

fn source_label(source: DiscoverySource) -> &'static str {
    match source {
        DiscoverySource::KnownPreset => "Known provider",
        DiscoverySource::ThunderbirdIspdb => "Thunderbird ISPDB",
        DiscoverySource::ProviderAutoconfigSubdomain => "Provider settings",
        DiscoverySource::ProviderAutoconfigWellKnown => "Provider settings",
    }
}

fn warning_text(warning: &CandidateWarning) -> &'static str {
    match warning {
        CandidateWarning::IncomingTransportIsNotEncrypted => {
            "Incoming server does not use encryption."
        }
        CandidateWarning::OutgoingTransportIsNotEncrypted => {
            "Outgoing server does not use encryption."
        }
        CandidateWarning::IncomingHostLooksPrivateOrLocal => {
            "Incoming server looks local or private; review it before sending credentials."
        }
        CandidateWarning::OutgoingHostLooksPrivateOrLocal => {
            "Outgoing server looks local or private; review it before sending credentials."
        }
        CandidateWarning::OAuth2NeedsMegaMailProviderRegistration => {
            "OAuth is advertised but MegaMail's own provider registration is not configured."
        }
        CandidateWarning::AuthenticationMethodUnsupported => {
            "The provider lists an authentication method that is not supported here."
        }
    }
}

fn tls_label(tls: TlsMode) -> &'static str {
    match tls {
        TlsMode::ImplicitTls => "TLS",
        TlsMode::StartTls => "STARTTLS",
        TlsMode::Plaintext => "Unencrypted",
    }
}

fn message_sender(message: &Message) -> String {
    if message.from_name.trim().is_empty() {
        message.from_addr.clone()
    } else {
        message.from_name.clone()
    }
}

fn initials(name: &str) -> String {
    let initials = name
        .split_whitespace()
        .take(2)
        .filter_map(|part| part.chars().next())
        .collect::<String>();
    if initials.is_empty() {
        "?".into()
    } else {
        initials.to_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::{composer_can_close, composer_can_edit, composer_operation_pending};

    #[test]
    fn compose_actions_share_one_busy_policy_including_uncertain_delivery() {
        assert!(composer_can_edit(false, false, false, false, false));
        assert!(composer_can_close(false, false, false, false));

        for (selecting, sending, saving) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            assert!(composer_operation_pending(
                selecting, sending, saving, false
            ));
            assert!(!composer_can_edit(selecting, sending, saving, false, false));
            assert!(!composer_can_close(selecting, sending, saving, false));
        }

        assert!(composer_operation_pending(false, false, false, true));
        assert!(!composer_can_edit(false, false, false, false, true));
        assert!(!composer_can_close(false, false, false, true));
        assert!(!composer_can_edit(false, false, false, true, false));
        assert!(composer_can_close(false, false, false, false));
    }
}

fn main() {
    let mut arguments = std::env::args_os();
    arguments.next();
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--thunderbird-host")) {
        let result = arguments
            .next()
            .ok_or_else(|| "Missing Thunderbird bridge socket".to_string())
            .and_then(|path| megamail_core::thunderbird_bridge::run_native_host(Path::new(&path)));
        if let Err(error) = result {
            eprintln!("Thunderbird bridge: {error}");
            std::process::exit(1);
        }
        return;
    }
    let demo = std::env::args().any(|argument| argument == "--demo");
    let _instance_lock = if demo {
        None
    } else {
        let Some(config_dir) = config::config_base() else {
            eprintln!("Could not create MegaMail's private configuration directory.");
            return;
        };
        match instance::acquire(&config_dir.join("instance.lock")) {
            Ok(lock) => Some(lock),
            Err(error) => {
                eprintln!(
                    "Could not start MegaMail: another instance may be using its mail account data ({error})."
                );
                return;
            }
        }
    };
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.text_system()
                .add_fonts(vec![
                    Cow::Borrowed(include_bytes!("../assets/fonts/Geist.ttf").as_slice()),
                    Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Medium.ttf").as_slice()),
                    Cow::Borrowed(include_bytes!("../assets/fonts/Geist-SemiBold.ttf").as_slice()),
                ])
                .expect("Failed to register bundled Geist fonts");
            init(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(800.)), cx)),
                window_min_size: Some(size(px(1060.), px(640.))),
                app_id: Some("com.megamail.MegaMail".into()),
                window_decorations: Some(WindowDecorations::Client),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| MailApp::new(window, cx, demo))
            })
            .expect("Failed to open MegaMail window");
            cx.activate(true);
        });
}
