use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::{Disableable as _, Selectable as _};
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, AppContext as _, ClipboardItem, Context,
    Decorations, Entity, FocusHandle, Focusable as _, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, PathPromptOptions, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    UniformListScrollHandle, Window, WindowBounds, WindowDecorations, WindowOptions, div,
    prelude::FluentBuilder as _, px, relative, rgb, size, uniform_list,
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
use crate::live::{ConversationKey, ConversationRow, MailboxScope};
use crate::onboarding::AccountForm;
use crate::preferences::{Density, MailPreferences};

use desktop_actions::action;
mod account_registry;
mod account_session;
mod appearance;
mod app_passwords;
mod appearance_view;
mod attachment_preview;
mod avatars;
mod comfort;
mod command_palette;
mod compose;
mod compose_markdown;
mod compose_productivity;
mod compose_session;
mod compose_tools_ui;
mod contacts;
mod desktop_actions;
mod draft_recovery;
mod feature_integration;
mod instance;
mod invitation;
mod live;
mod mcp;
mod original_view;
mod newsletter_actions;
mod onboarding;
mod outbox_review;
mod preferences;
mod provider_setup;
mod reader_actions;
mod reader_document;
mod reader_find;
mod reader_host;
mod reader_session;
mod rules_editor;
mod rules_preview;
mod rules_ui;
mod scheduling;
mod sender_photos;
mod splits;
mod batch_send;
mod composer_ui;
mod list_ui;
mod original_ui;
mod reader_ui;
mod setup_ui;

mod image_trust;
mod subscriptions;
mod theme;
mod thunderbird_adapter;
mod triage;
mod triage_ui;
mod workflow;
mod zeron_background;
mod zeron_style;

use action::*;

fn packed_rgb(color: gpui_kit::Hsla) -> u32 {
    let color = color.to_rgb();
    ((color.r * 255.0).round() as u32) << 16
        | ((color.g * 255.0).round() as u32) << 8
        | (color.b * 255.0).round() as u32
}

// Static icon bytes also work in isolated debug builds with no registry access.
gpui_kit::assets::icon_assets!(
    MailIcons,
    [
        Archive,
        ArchiveRestore,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        Check,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        ChevronUp,
        ChevronsUpDown,
        Close,
        Columns2,
        Copy,
        CornerUpLeft,
        CornerUpRight,
        Download,
        Ellipsis,
        FileText,
        Folder,
        FolderOpen,
        Image,
        Inbox,
        Info,
        ListFilter,
        LoaderCircle,
        Mail,
        MailOpen,
        Moon,
        Palette,
        PanelLeftClose,
        PanelLeftOpen,
        Paperclip,
        PenLine,
        Plus,
        RefreshCw,
        Rows2,
        Search,
        Send,
        Settings,
        Settings2,
        SlidersHorizontal,
        Star,
        StarFill,
        Sun,
        Trash,
        TriangleAlert,
        X,
    ]
);

fn init(cx: &mut App) {
    desktop_actions::bind_keys(cx);
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

/// Left-aligned icon + label for sidebar buttons (Button centres its own label).
fn nav_content(icon: Icon, label: impl Into<SharedString>) -> gpui_kit::Div {
    h_flex()
        .w_full()
        .min_w_0()
        .items_center()
        .gap_2()
        .child(icon)
        .child(div().min_w_0().truncate().child(label.into()))
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
        theme.table_head = palette.hover;
        theme.table_head_foreground = palette.text;
        theme.link = palette.accent;
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

fn startup_view(route: account_session::StartupRoute) -> View {
    match route {
        account_session::StartupRoute::FirstRun => View::Setup,
        account_session::StartupRoute::Mailbox | account_session::StartupRoute::StorageRecovery => {
            View::Mailbox
        }
    }
}

#[derive(Clone, Copy)]
enum PreferenceChange {
    GroupConversations(bool),
    Density(Density),
    HideQuotedText(bool),
    ReducedMotion(bool),
    DefaultUnified(bool),
    ListAvatars(bool),
    InternetAvatars(bool),
    Gravatar(bool),
    LoadRemote(bool),
    Layout(preferences::PaneLayout),
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
    markup: compose_markdown::Markup,
    message: OutgoingMessage,
    to: Entity<InputState>,
    cc: Entity<InputState>,
    bcc: Entity<InputState>,
    subject: Entity<InputState>,
    body: Entity<TextareaState>,
    tools: compose_tools_ui::Tools,
    schedule: Entity<InputState>,
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
    recovery: compose_session::Capture,
    draft_ticket: Option<u32>,
    send_generation: Option<u64>,
    sent_complete: bool,
    draft_save_completed: bool,
    /// The quoted original of a reply, kept out of the editor and appended
    /// on send/save, the way Superhuman and Gmail fold it away.
    quoted: Option<String>,
    show_quoted: bool,
    show_cc: bool,
    /// Formatting, local contacts and Send Later, folded until asked for.
    show_more: bool,
    /// Send one personalized copy per To recipient ({name}, {first_name}, {email}).
    individually: bool,
    _input_subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct SenderChoice {
    alias: Option<String>,
    label: String,
}

impl Composer {
    fn new(message: OutgoingMessage, window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        let formatted = message.html.is_empty();
        let mut composer = Self {
            // Formatting is the default; recovered HTML drafts keep their HTML.
            markup: compose_markdown::Markup { enabled: formatted, ..Default::default() },
            to: new_input_value(window, cx, "recipient@example.com", message.to.clone()),
            cc: new_input_value(window, cx, "Copy to", message.cc.clone()),
            bcc: new_input_value(window, cx, "Blind copy to", message.bcc.clone()),
            subject: new_input_value(window, cx, "Subject", message.subject.clone()),
            body: cx.new(|cx| TextareaState::new(window, cx).placeholder("Write your message…").default_value(message.body.clone())),
            tools: compose_tools_ui::Tools::new(window, cx),
            schedule: new_input_value(
                window,
                cx,
                "YYYY-MM-DD HH:MM +HH:MM",
                message
                    .send_at
                    .and_then(megamail_core::datefmt::editable_send_time)
                    .unwrap_or_default(),
            ),
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
            recovery: Default::default(),
            draft_ticket: None,
            send_generation: None,
            sent_complete: false,
            draft_save_completed: false,
            quoted: None,
            show_quoted: false,
            show_cc: false,
            show_more: false,
            individually: false,
            _input_subscriptions: Vec::new(),
        };
        composer.show_cc = !composer.message.cc.trim().is_empty()
            || !composer.message.bcc.trim().is_empty();
        for input in [&composer.to, &composer.cc, &composer.bcc, &composer.subject] {
            composer._input_subscriptions.push(cx.subscribe(
                input,
                |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        if let Some(composer) = &mut this.composer {
                            composer.recovery.dirty_input = true;
                        }
                        cx.notify();
                    }
                },
            ));
        }
        composer._input_subscriptions.push(cx.subscribe(
            &composer.body,
            |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(composer) = &mut this.composer {
                        composer.recovery.dirty_input = true;
                    }
                    this.refresh_markdown(cx);
                    cx.notify();
                }
            },
        ));
        composer
    }

    fn new_prepared_draft(
        message: OutgoingMessage,
        temporary_attachment_paths: Vec<PathBuf>,
        notice: Option<String>,
        attachment_lease: Option<draft_recovery::AttachmentLease>,
        window: &mut Window,
        cx: &mut Context<MailApp>,
    ) -> Self {
        let mut composer = Self::new(message, window, cx);
        composer.temporary_attachment_paths = temporary_attachment_paths;
        composer.notice = notice;
        composer.recovery.lease = attachment_lease;
        composer
    }

    fn outgoing(&self, cx: &App) -> OutgoingMessage {
        let mut message = self.message.clone();
        message.to = self.to.read(cx).value().to_string();
        message.cc = self.cc.read(cx).value().to_string();
        message.bcc = self.bcc.read(cx).value().to_string();
        message.subject = self.subject.read(cx).value().to_string();
        message.body = self.body.read(cx).value().to_string();
        if self.markup.enabled && !self.markup.busy && self.markup.ready_source == message.body {
            message.html = self.markup.html.clone();
        }
        if let Some(quoted) = self.quoted.as_deref().filter(|q| !q.trim().is_empty()) {
            if !message.html.is_empty() {
                message.html.push_str(&quoted_html(quoted));
            }
            message.body = format!("{}\n\n{}", message.body.trim_end(), quoted);
        }
        // Keep recovered HTML unchanged. Editing it requires an explicit
        // plain-text conversion; never silently flatten a restored alternative.
        message
    }

    fn is_saved(&self, cx: &App) -> bool {
        let Some(saved) = &self.draft_saved else {
            return false;
        };
        let current = self.outgoing(cx);
        compose_session::same(&current, saved)
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
        !self.sent_complete
            && self.message.html.is_empty()
            && composer_can_edit(
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

fn changed_mailbox_scopes(
    previous: &std::collections::HashMap<(u32, String), live::NativeMailboxEpoch>,
    current: &std::collections::HashMap<(u32, String), live::NativeMailboxEpoch>,
) -> HashSet<(u32, String)> {
    current
        .iter()
        .filter(|(scope, epoch)| previous.get(*scope) != Some(*epoch))
        .map(|(scope, _)| scope.clone())
        .collect()
}

fn retire_reader_body_cache(
    cache: &mut std::collections::VecDeque<ReaderBody>,
    retired: &HashSet<(u32, String)>,
) {
    cache.retain(|body| reader_scope_retained(&body.key, retired));
}

fn reader_scope_retained(key: &MessageKey, retired: &HashSet<(u32, String)>) -> bool {
    !retired.contains(&(key.account_id, key.folder_path.clone()))
}

fn reader_follows_selection<'a>(
    previous: Option<&MessageKey>,
    selected: Option<&MessageKey>,
    mut members: impl Iterator<Item = &'a MessageKey>,
) -> bool {
    previous.is_some_and(|previous| {
        selected.is_some_and(|selected| previous != selected)
            && members.any(|member| member == previous)
    })
}

fn reader_recipient(
    key: &MessageKey,
    label: &'static str,
    value: &str,
    palette: Palette,
) -> AnyElement {
    h_flex()
        .w_full()
        .min_w_0()
        .items_start()
        .gap_2()
        .child(
            div()
                .flex_shrink_0()
                .w(px(48.))
                .text_size(px(12.))
                .text_color(palette.muted)
                .child(label),
        )
        .child(
            gpui_kit::component::text::TextView::html(
                SharedString::from(format!(
                    "reader-recipient-{}-{}-{}-{label}",
                    key.account_id, key.folder_path, key.uid
                )),
                preferences::plain_text_html(value),
            )
            .selectable(true)
            .flex_1()
            .min_w_0()
            .text_size(px(12.))
            .text_color(palette.muted)
            .style(gpui_kit::component::text::TextViewStyle {
                paragraph_gap: gpui_kit::rems(0.),
                ..Default::default()
            }),
        )
        .into_any_element()
}

fn composer_can_close(
    selecting_attachments: bool,
    sending: bool,
    saving_draft: bool,
    send_uncertain: bool,
) -> bool {
    !composer_operation_pending(selecting_attachments, sending, saving_draft, send_uncertain)
}

struct ReaderBody {
    key: MessageKey,
    text: SharedString,
    without_quote: SharedString,
    has_quote: bool,
    source_html: SharedString,
    html: SharedString,
    html_without_quote: SharedString,
    plain_html: SharedString,
    plain_html_without_quote: SharedString,
    retained_bytes: usize,
    current_state: Option<Entity<gpui_kit::component::text::TextViewState>>,
    plain_state: Option<Entity<gpui_kit::component::text::TextViewState>>,
    quote_state: Option<Entity<gpui_kit::component::text::TextViewState>>,
    plain_quote_state: Option<Entity<gpui_kit::component::text::TextViewState>>,
}

impl ReaderBody {
    fn markup(&self, plain: bool, quote_visible: bool) -> SharedString {
        match (plain, quote_visible) {
            (true, true) => self.plain_html.clone(),
            (true, false) => self.plain_html_without_quote.clone(),
            (false, true) => self.html.clone(),
            (false, false) => self.html_without_quote.clone(),
        }
    }

    fn prepare(&mut self, cx: &mut App) {
        let state = |markup: &str, cx: &mut App| {
            Some(cx.new(|cx| {
                gpui_kit::component::text::TextViewState::html(markup, cx).selectable(true)
            }))
        };
        let (_, quote) = preferences::split_quoted_text(&self.text);
        let quote_markup = preferences::quoted_html(&self.source_html)
            .unwrap_or_else(|| preferences::plain_text_html(quote.unwrap_or_default()));
        self.current_state = state(&self.html_without_quote, cx);
        self.plain_state = state(&self.plain_html_without_quote, cx);
        self.quote_state = state(&quote_markup, cx);
        self.plain_quote_state =
            state(&preferences::plain_text_html(quote.unwrap_or_default()), cx);
    }

    fn new(
        key: MessageKey,
        body: &str,
        source_html: SharedString,
        formatted_without_quote: Option<SharedString>,
    ) -> Self {
        let (without_quote, quote) = preferences::split_quoted_text(body);
        let text: SharedString = body.to_owned().into();
        let without_quote: SharedString = if quote.is_some() {
            without_quote.to_owned().into()
        } else {
            text.clone()
        };
        let plain_html: SharedString = preferences::plain_text_html(body).into();
        let plain_html_without_quote = if quote.is_some() {
            preferences::plain_text_html(&without_quote).into()
        } else {
            plain_html.clone()
        };
        let formatted_without_quote = formatted_without_quote
            .or_else(|| quote.is_some().then(|| plain_html_without_quote.clone()));
        let formatted = !source_html.is_empty();
        let html = if formatted {
            source_html.clone()
        } else {
            plain_html.clone()
        };
        let html_without_quote = if formatted {
            formatted_without_quote
                .clone()
                .unwrap_or_else(|| html.clone())
        } else {
            plain_html_without_quote.clone()
        };
        let retained_bytes = text.len()
            + source_html.len()
            + plain_html.len()
            + if quote.is_some() {
                without_quote.len() + plain_html_without_quote.len()
            } else {
                0
            }
            + formatted_without_quote
                .as_ref()
                .map_or(0, |html| html.len());
        Self {
            key,
            text,
            without_quote,
            has_quote: if formatted {
                formatted_without_quote.is_some()
            } else {
                quote.is_some()
            },
            source_html,
            html,
            html_without_quote,
            plain_html,
            plain_html_without_quote,
            retained_bytes,
            current_state: None,
            plain_state: None,
            quote_state: None,
            plain_quote_state: None,
        }
    }
}

#[derive(Clone)]
struct ListRow {
    row: Arc<MessageRow>,
    conversation: Option<ConversationRow>,
    child: bool,
}

/// What a hover button, right-click item or swipe does to one row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Archive,
    Trash,
    Star,
    Read,
}

/// Horizontal travel (px) that commits a trackpad swipe on a row.
const SWIPE_COMMIT: f32 = 110.;

/// Seconds a sent message waits in the Outbox so Undo can stop it.
const UNDO_SEND_SECONDS: i64 = 10;

struct PendingUndo {
    account: u32,
    send_at: i64,
    request_id: u64,
    original: OutgoingMessage,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReaderMode {
    Original,
    Clean,
    Text,
}

struct OriginalState {
    key: MessageKey,
    source: Arc<str>,
    params: original_view::Params,
    allow_remote: bool,
    /// The user pressed Load images: wins over the untrusted-sender block.
    remote_forced: bool,
    original_colors: bool,
    generation: u64,
    pending: bool,
    doc_id: u64,
    result: Option<Result<original_view::Rendered, String>>,
}

struct PaletteState {
    input: Entity<InputState>,
    index: usize,
    _subscription: Subscription,
}

struct MailApp {
    live: Entity<LiveMailbox>,
    sender_photos: sender_photos::State,
    /// Organisation domains whose verified logo was already looked up.
    logo_attempts: HashSet<String>,
    /// Split inbox: the active split, screener decisions, the addresses you
    /// have written to, and unread counts per split for the tabs.
    split: splits::Split,
    screener: splits::Screener,
    known_senders: HashSet<String>,
    split_unread: [usize; 5],
    /// The message just sent with a short delay, while it can be undone.
    undo_send: Option<PendingUndo>,
    /// Row being swiped on the trackpad and its horizontal travel.
    swipe: Option<(MessageKey, f32)>,
    swipe_generation: u64,
    /// Superhuman layout: the inbox is a full-width list; opening a
    /// conversation replaces it until Esc.
    conversation_open: bool,
    /// The folder sidebar is out of the way unless asked for (Ctrl+B).
    sidebar_hidden: bool,
    mail_panes: Entity<gpui_kit::component::resizable::ResizableState>,
    split_right_panes: Entity<gpui_kit::component::resizable::ResizableState>,
    split_below_panes: Entity<gpui_kit::component::resizable::ResizableState>,
    compact_mail_panes: Entity<gpui_kit::component::resizable::ResizableState>,
    _live_subscription: Subscription,
    profiles: Vec<(u32, AccountConfig)>,
    comfort: comfort::State,
    search: Entity<InputState>,
    message_focus: FocusHandle,
    mailbox_focus: FocusHandle,
    mailbox_focus_admitted: bool,
    compose_tools_busy: bool,
    reader_text: Option<reader_session::SemanticTextTarget>,
    rule_preview: Vec<String>,
    newsletter_status: Option<String>,
    rule_preview_busy: bool,
    rule_move_plan: Option<rules_ui::MovePlan>,
    rule_editor: rules_editor::State,
    reader_export_busy: bool,
    reader_find: reader_find::State,
    attachment_preview: attachment_preview::State,
    reader_export_status: Option<String>,
    triage: triage_ui::State,
    queue_cancel_confirmation: Option<(u32, worker::QueuedCancelRequest)>,
    outbox_review: Option<(u32, u32)>,
    activation: Option<std::sync::Arc<instance::ActivationInbox>>,
    activation_confirmed_account: Option<u32>,
    activation_pending: usize,
    auth_controller: megamail_core::provider_auth::AuthController,
    auth_cancel: Arc<std::sync::Mutex<Option<megamail_core::provider_auth::Attempt>>>,
    auth_availability: Option<provider_setup::Availability>,
    auth_cancelled: Arc<std::sync::atomic::AtomicBool>,
    auth_inflight: bool,
    next_draft_ticket: u32,
    reader_source: Option<Arc<reader_session::OriginalBody>>,
    invitation_preview: Option<Arc<invitation::InvitationPreview>>,
    invitation_error: Option<String>,
    microsoft_audience: megamail_core::provider_auth::MicrosoftAudience,
    reader_session: reader_session::Session,
    recovery: Option<compose_session::Service>,
    recovery_error: Option<String>,
    recovery_keys: Vec<draft_recovery::DraftKey>,
    request_attachment_leases: compose_session::HeldAttachments,
    recovery_listing: bool,
    list_scroll: UniformListScrollHandle,
    reader_scroll: gpui_kit::ScrollHandle,
    reader_anchor: gpui_kit::ScrollAnchor,
    reader_selected_key: Option<MessageKey>,
    reader_selected_position: Option<usize>,
    reader_should_focus: bool,
    reader_follow_selection: bool,
    setup: Option<SetupState>,
    composer: Option<Composer>,
    view: View,
    setup_return_view: View,
    appearance_return_view: View,
    settings_section: appearance_view::Section,
    sidebar_collapsed: bool,
    /// Ctrl+K palette, when open.
    command_palette: Option<PaletteState>,
    /// Rows archived/trashed from the keyboard: hidden at once, before the
    /// worker's refresh confirms the move (cleared if an action fails).
    hidden_rows: HashSet<MessageKey>,
    /// Sort, work-state, scope and sync controls, folded by default.
    list_controls_open: bool,
    /// Inline Blitz rendering of the selected message's original HTML.
    original: Option<OriginalState>,
    /// The user chose Clean over Original; remembered for this session.
    prefer_clean: bool,
    /// Width available to the reader body and where the original image
    /// was last painted, recorded during paint for layout and hit-testing.
    original_measure: std::rc::Rc<std::cell::Cell<f32>>,
    original_bounds: std::rc::Rc<std::cell::Cell<gpui_kit::Bounds<gpui_kit::Pixels>>>,
    window_scale: f32,
    original_generation: u64,
    accounts_expanded: bool,
    other_folders_expanded: bool,
    demo: bool,
    /// Senders whose images load despite failed sender checks.
    image_senders: std::collections::HashSet<String>,
    subscriptions: Option<subscriptions::Screen>,
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
    wallpaper_restore_generation: u64,
    preferences: MailPreferences,
    preferences_busy: bool,
    preferences_loading: bool,
    preferences_error: Option<String>,
    expanded_conversations: std::collections::HashSet<ConversationKey>,
    expanded_messages: std::collections::HashSet<MessageKey>,
    collapsed_messages: std::collections::HashSet<MessageKey>,
    visible_quotes: std::collections::HashSet<MessageKey>,
    hidden_quotes: std::collections::HashSet<MessageKey>,
    plain_messages: std::collections::HashSet<MessageKey>,
    link_details: std::collections::HashSet<MessageKey>,
    reader_bodies: std::collections::VecDeque<ReaderBody>,
    reader_mailbox_epochs: std::collections::HashMap<(u32, String), live::NativeMailboxEpoch>,
    list_rows: Arc<Vec<ListRow>>,
    appearance: appearance::AppearanceState,
    appearance_loading: bool,
    appearance_busy: bool,
    appearance_error: Option<String>,
    _search_subscription: Subscription,
    setup_email_subscription: Option<Subscription>,
}

impl MailApp {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        demo: bool,
        activation: Option<Arc<instance::ActivationInbox>>,
    ) -> Self {
        window.set_window_title(if demo {
            "MegaMail · Local demo"
        } else {
            "MegaMail"
        });
        apply_palette(ThemeMode::Dark, Palette::new(true), window, cx);

        let search = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search · from: subject: is: body:"));
        let live = cx.new(|cx| {
            if demo {
                LiveMailbox::new_demo(cx)
            } else {
                LiveMailbox::new(Vec::new(), None, cx)
            }
        });
        let live_subscription = cx.observe_in(&live, window, |this, _, window, cx| {
            let reader_had_focus = this.reader_text.as_ref().is_some_and(|target| {
                target
                    .state
                    .read(cx)
                    .focus_handle()
                    .contains_focused(window, cx)
            });
            this.on_live_change(cx);
            if reader_had_focus
                && this.view == View::Mailbox
                && !this.reader_text.as_ref().is_some_and(|target| {
                    target
                        .state
                        .read(cx)
                        .focus_handle()
                        .contains_focused(window, cx)
                })
            {
                // The old semantic node can disappear during a folder/body update.
                // Keep shortcuts in the stable mailbox context instead of a retired node.
                if this.list_rows.is_empty() {
                    this.mailbox_focus.focus(window, cx);
                } else {
                    this.message_focus.focus(window, cx);
                }
            }
        });
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
        let reader_scroll = gpui_kit::ScrollHandle::default();
        let reader_anchor = gpui_kit::ScrollAnchor::for_handle(reader_scroll.clone());
        let mut app = Self {
            live,
            _live_subscription: live_subscription,
            profiles: Vec::new(),
            comfort: comfort::State::default(),
            search,
            message_focus,
            mailbox_focus: cx.focus_handle().tab_stop(true),
            mailbox_focus_admitted: false,
            compose_tools_busy: false,
            mail_panes: cx.new(|_| Default::default()),
            split_right_panes: cx.new(|_| Default::default()),
            split_below_panes: cx.new(|_| Default::default()),
            compact_mail_panes: cx.new(|_| Default::default()),
            sender_photos: Default::default(),
            logo_attempts: HashSet::new(),
            split: splits::Split::All,
            screener: splits::Screener::default(),
            known_senders: HashSet::new(),
            split_unread: [0; 5],
            undo_send: None,
            swipe: None,
            swipe_generation: 0,
            conversation_open: false,
            sidebar_hidden: true,
            reader_text: None,
            rule_preview: Vec::new(),
            newsletter_status: None,
            rule_preview_busy: false,
            rule_move_plan: None,
            rule_editor: rules_editor::State::new(window, cx),
            reader_export_busy: false,
            reader_find: reader_find::State::new(window, cx),
            attachment_preview: Default::default(),
            reader_export_status: None,
            triage: triage_ui::State::new(window, cx),
            queue_cancel_confirmation: None,
            outbox_review: None,
            activation,
            activation_confirmed_account: None,
            activation_pending: 0,
            auth_controller: Default::default(),
            auth_cancel: Default::default(),
            auth_availability: None,
            auth_cancelled: Default::default(),
            auth_inflight: false,
            next_draft_ticket: 0,
            reader_source: None,
            invitation_preview: None,
            invitation_error: None,
            microsoft_audience: megamail_core::provider_auth::MicrosoftAudience::PersonalAndWork,
            reader_session: Default::default(),
            recovery: None,
            recovery_error: None,
            recovery_keys: Vec::new(),
            request_attachment_leases: Default::default(),
            recovery_listing: false,
            list_scroll: UniformListScrollHandle::new(),
            reader_scroll,
            reader_anchor,
            reader_selected_key: None,
            reader_selected_position: None,
            reader_should_focus: false,
            reader_follow_selection: false,
            setup: (!demo).then(|| SetupState::new(window, cx)),
            composer: None,
            view: if demo { View::Mailbox } else { View::Setup },
            setup_return_view: View::Mailbox,
            appearance_return_view: View::Mailbox,
            settings_section: Default::default(),
            sidebar_collapsed: true,
            command_palette: None,
            hidden_rows: HashSet::new(),
            list_controls_open: false,
            original: None,
            prefer_clean: false,
            original_measure: Default::default(),
            original_bounds: Default::default(),
            window_scale: 1.,
            original_generation: 0,
            accounts_expanded: false,
            other_folders_expanded: false,
            demo,
            subscriptions: None,
            image_senders: Default::default(),
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
            wallpaper_restore_generation: 0,
            preferences: MailPreferences::default(),
            preferences_busy: false,
            preferences_loading: true,
            preferences_error: None,
            expanded_conversations: std::collections::HashSet::new(),
            expanded_messages: std::collections::HashSet::new(),
            collapsed_messages: std::collections::HashSet::new(),
            visible_quotes: std::collections::HashSet::new(),
            hidden_quotes: std::collections::HashSet::new(),
            plain_messages: std::collections::HashSet::new(),
            link_details: std::collections::HashSet::new(),
            reader_bodies: std::collections::VecDeque::new(),
            reader_mailbox_epochs: std::collections::HashMap::new(),
            list_rows: Arc::new(Vec::new()),
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
                        MailPreferences::load(),
                        if demo {
                            Ok(workflow::Ledger::default())
                        } else {
                            workflow::Ledger::load()
                        },
                        if demo { Default::default() } else { image_trust::load() },
                    )
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.appearance = loaded.0;
                this.image_senders = loaded.4;
                match loaded.1 {
                    Ok(theme) => this.theme = theme,
                    Err(error) => this.theme_error = Some(error),
                }
                match loaded.2 {
                    Ok(preferences) => this.preferences = preferences,
                    Err(error) => this.preferences_error = Some(error),
                }
                match loaded.3 {
                    Ok(ledger) => {
                        this.comfort.ledger = ledger;
                        this.comfort.loaded = true;
                    }
                    Err(error) => this.comfort.error = Some(error),
                }
                this.preferences_loading = false;
                let snapshot = this.live.read(cx).snapshot();
                this.rebuild_list_rows(&snapshot, cx);
                this.refresh_reader_text(&snapshot, cx);
                if this.preferences.default_unified {
                    this.live.update(cx, |live, cx| live.select_unified(cx));
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
            app.load_saved_accounts(cx);
            app.start_native_features(cx);
        }
        app.start_native_event_pump(cx);

        app
    }

    fn start_native_features(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let (screener, known) = cx
                .background_spawn(async move {
                    let known: HashSet<String> = megamail_core::cache::Cache::open()
                        .map(|cache| {
                            cache
                                .address_history()
                                .into_iter()
                                .map(|(email, _, _)| email.to_ascii_lowercase())
                                .collect()
                        })
                        .unwrap_or_default();
                    (splits::Screener::load(), known)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.screener = screener;
                this.known_senders.extend(known);
                let snapshot = this.live.read(cx).snapshot();
                this.rebuild_list_rows(&snapshot, cx);
                cx.notify();
            });
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let (availability, recovery) = cx
                .background_spawn(async move {
                    (
                        provider_setup::availability(),
                        compose_session::Service::open(),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.auth_availability = Some(availability);
                match recovery {
                    Ok(service) => this.recovery = Some(service),
                    Err(error) => this.recovery_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn action_state(&self, cx: &App) -> desktop_actions::ActionState {
        let snapshot = self.live.read(cx).snapshot();
        desktop_actions::ActionState {
            mailbox_visible: self.view == View::Mailbox,
            composer_present: self.composer.is_some() && self.view == View::Compose,
            modal_active: self.loading_profiles || self.preferences_busy || self.recovery_listing,
            has_accounts: !snapshot.accounts.is_empty(),
            usable_sender: self.live.read(cx).composer_account_id().is_some(),
            has_rows: !self.list_rows.is_empty(),
            body_ready: snapshot.body_loaded
                && !snapshot.body_loading
                && snapshot.selected_message.as_ref().is_some_and(|row| {
                    self.live.read(cx).composer_account_id() == Some(row.key.account_id)
                }),
        }
    }

    fn start_native_event_pump(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| {
                        let pending = this.activation.as_ref().map_or(0, |inbox| inbox.pending_count());
                        if pending != this.activation_pending {
                            this.activation_pending = pending;
                            cx.notify();
                        }
                        this.pump_triage(cx);
                        let minute = megamail_core::datefmt::now() / 60;
                        if minute != this.comfort.last_minute {
                            this.comfort.last_minute = minute;
                            if this.comfort.filter.is_some() { this.rebuild_list_rows(&this.live.read(cx).snapshot(), cx); }
                            cx.notify();
                        }
                        let previous_recovery_status =
                            this.composer.as_ref().map(|c| c.recovery.status.clone());
                        let previous_reader_notice = this.reader_session.notice.clone();
                        let ready = this.composer.is_none() && this
                            .action_state(cx)
                            .allows(desktop_actions::MailboxAction::ComposeMessage)
                            && this.setup.is_none()
                            && this.startup_error.is_none()
                            && !this.attachment_save_pending.is_some();
                        // Explicit current account only: a Unified selection/URI
                        // cannot choose the sender for an external composition.
                        let snapshot = this.live.read(cx).snapshot();
                        let account = snapshot.current_account_id.filter(|id| {
                            this.activation_confirmed_account == Some(*id)
                                && this.live.read(cx).composer_account_id() == Some(*id)
                        });
                        this.activation_confirmed_account = None;
                        if let Some(intent) = this
                            .activation
                            .as_ref()
                            .and_then(|inbox| inbox.take_if_ready(ready && account.is_some()))
                        {
                            let message =
                                desktop_actions::compose_message(intent, account.unwrap_or(0));
                            let mut composer = Composer::new(message, window, cx);
                            composer.recovery.retention = draft_recovery::Retention::Ordinary;
                            this.composer = Some(composer);
                            this.view = View::Compose;
                            cx.notify();
                        }
                        this.capture_recovery(cx);
                        if let Some(service) = &this.recovery {
                            for event in service.events() {
                                match &event {
                                    draft_recovery::WriterEvent::Discarded { key, generation, removed }
                                        if this.composer.as_ref().is_some_and(|c| c.sent_complete && c.recovery.key.as_ref() == Some(key) && c.recovery.generation == *generation) => {
                                        if *removed { this.recovery_keys.retain(|candidate| candidate != key); }
                                        this.recovery_error = if *removed { None } else { Some("Sent was confirmed, but no exact-generation local record was removed. Older recovery copies may remain; resend review is unavailable.".into()) };
                                    }
                                    draft_recovery::WriterEvent::Failed { key, generation, error }
                                        if this.composer.as_ref().is_some_and(|c| c.sent_complete && c.recovery.key.as_ref() == Some(key) && c.recovery.generation == *generation) => {
                                        this.recovery_error = Some(format!("Sent was confirmed; local recovery cleanup failed: {error}. Retry cleanup, not Send."));
                                    }
                                    _ => {}
                                }
                                if let Some(composer) = &mut this.composer {
                                    composer.recovery.event(event);
                                }
                                cx.notify();
                            }
                        }
                        let sent_can_close = this.composer.as_ref().is_some_and(|c| {
                            c.sent_complete
                                && (this.recovery.is_none()
                                    || !c.recovery.requires_ack
                                    || c.recovery.retired == Some(c.recovery.generation))
                        });
                        if sent_can_close {
                            this.cleanup_composer_attachments(cx);
                            this.composer = None;
                            this.view = View::Mailbox;
                            cx.notify();
                        }
                        if this.view != View::Mailbox
                            && (this.reader_session.active.is_some()
                                || this.reader_session.host.is_some())
                        {
                            this.retire_reader_session(cx);
                        }
                        let reader_generation_before_poll = this.reader_session.generation;
                        for url in this.reader_session.poll() {
                            cx.background_spawn(async move {
                                megamail_core::oauth::open_uri(&url);
                            })
                            .detach();
                        }
                        if reader_generation_before_poll != this.reader_session.generation {
                            let snapshot = this.live.read(cx).snapshot();
                            this.refresh_reader_text(&snapshot, cx);
                        }
                        if previous_recovery_status
                            != this.composer.as_ref().map(|c| c.recovery.status.clone())
                            || previous_reader_notice != this.reader_session.notice
                        {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn list_local_recovery(&mut self, cx: &mut Context<Self>) {
        if self.demo
            || self.composer.is_some()
            || self.recovery_listing
            || self.view != View::Mailbox
        {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(id) = snapshot.current_account_id else {
            return;
        };
        let Some(service) = &self.recovery else {
            return;
        };
        if !self
            .profiles
            .iter()
            .any(|(account_id, _)| *account_id == id)
        {
            self.recovery_error = Some("Local restore requires a validated direct-account From configuration; Thunderbird restore is not integrated.".into());
            cx.notify();
            return;
        }
        let root = service.root.clone();
        self.recovery_listing = true;
        self.recovery_keys.clear();
        self.recovery_error = None;
        let task = cx.background_spawn(async move {
            draft_recovery::RecoveryStore::open(&root)
                .map_err(|e| e.to_string())?
                .list(id)
                .map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.recovery_listing = false;
                if this.live.read(cx).snapshot().current_account_id != Some(id) { return; }
                match result {
                    Ok((keys, complete)) => { this.recovery_keys = keys; this.recovery_error = if !complete { Some("Recovery list is partial (256-entry limit). More records may exist; repair/pagination is not available.".into()) } else if this.recovery_keys.is_empty() { Some("No local recovery records found for this account. Session-only drafts are not crash recoverable.".into()) } else { None }; },
                    Err(error) => this.recovery_error = Some(error),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn restore_local_recovery(&mut self, key: draft_recovery::DraftKey, cx: &mut Context<Self>) {
        if self.composer.is_some() || self.recovery_listing || self.view != View::Mailbox {
            return;
        }
        let id = key.account_id();
        if self.live.read(cx).snapshot().current_account_id != Some(id) {
            return;
        }
        let Some((_, account)) = self
            .profiles
            .iter()
            .find(|(id, _)| *id == key.account_id())
            .cloned()
        else {
            return;
        };
        let Some(service) = &self.recovery else {
            return;
        };
        let root = service.root.clone();
        self.recovery_listing = true;
        let reaper = service.reaper();
        let task = cx.background_spawn(async move {
            let store = draft_recovery::RecoveryStore::open(&root).map_err(|e| e.to_string())?;
            // Sensitive restore requires a future explicit consent surface.
            compose::prepare_local_recovery(&store, &key, &account, false)
                .map(|recovered| compose_session::LeaseDelivery::new(recovered, reaper))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.recovery_listing = false;
                if this.composer.is_some() || this.view != View::Mailbox || this.live.read(cx).snapshot().current_account_id != Some(id) { return; }
                match result {
                    Ok(mut delivery) => {
                        let Some(recovered) = delivery.take() else { this.recovery_error = Some("This recovery record is no longer available.".into()); cx.notify(); return; };
                        let mut composer = Composer::new_prepared_draft(recovered.message, Vec::new(), Some("Recovered local copy. Sending is blocked until durable Outbox reconciliation; HTML editing needs explicit plain-text conversion.".into()), Some(recovered.attachment_lease), window, cx);
                        composer.recovery.key = Some(recovered.key); composer.recovery.generation = recovered.generation;
                        composer.recovery.retention = if recovered.sensitive { draft_recovery::Retention::DecryptedSessionOnly } else { draft_recovery::Retention::Ordinary };
                        composer.recovery.restored_hold = true;
                        this.composer = Some(composer); this.recovery_keys.clear(); this.view = View::Compose;
                    },
                    Err(error) => this.recovery_error = Some(error),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn recovery_panel(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.activation_pending == 0
            && !self.recovery_listing
            && self.recovery_error.is_none()
            && self.recovery_keys.is_empty()
        {
            return div().into_any_element();
        }
        v_flex()
            .gap_1()
            .when(self.activation_pending > 0, |el| {
                el.child(
                    Button::new("confirm-external-compose-account")
                        .ghost()
                        .small()
                        .label("Open queued email link")
                        .disabled(
                            self.activation.is_none()
                                || !self
                                    .action_state(cx)
                                    .allows(desktop_actions::MailboxAction::ComposeMessage),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.activation_confirmed_account =
                                this.live.read(cx).snapshot().current_account_id;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.muted)
                        .child(format!(
                            "{} queued email links. Choose an account to open a draft.",
                            self.activation_pending
                        )),
                )
            })
            .when(self.recovery_listing, |el| {
                el.child(div().text_size(px(11.)).child("Reading recovered drafts…"))
            })
            .when_some(self.recovery_error.clone(), |el, error| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.muted)
                        .child(error),
                )
            })
            .children(
                self.recovery_keys
                    .iter()
                    .enumerate()
                    .filter(|(_, key)| {
                        Some(key.account_id()) == self.live.read(cx).snapshot().current_account_id
                    })
                    .map(|(index, key)| {
                        let key = key.clone();
                        Button::new(SharedString::from(format!("restore-local-{index}")))
                            .ghost()
                            .small()
                            .label(format!("Open local recovery {}", index + 1))
                            .disabled(self.recovery_listing || self.composer.is_some())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.restore_local_recovery(key.clone(), cx)
                            }))
                    }),
            )
            .into_any_element()
    }

    fn retry_native_recovery(&mut self, cx: &mut Context<Self>) {
        let Some(service) = &self.recovery else {
            return;
        };
        let Some(composer) = &mut self.composer else {
            return;
        };
        if composer.operation_pending() {
            return;
        }
        if let Some((account_id, request_id, generation)) = composer.recovery.finished_send {
            if composer.recovery.cleanup_in_flight || composer.recovery.retired == Some(generation)
            {
                return;
            }
            let Some(key) = composer.recovery.key.clone() else {
                return;
            };
            if account_id != composer.message.from_account_id
                || account_id != key.account_id()
                || generation != composer.recovery.generation
            {
                return;
            }
            match service.submit(draft_recovery::WriterCommand::SendFinished {
                key,
                generation,
                expected_request: request_id,
                account_id,
                request_id,
                outcome: SendOutcome::Sent,
                uncertain: false,
            }) {
                Ok(()) => {
                    composer.recovery.cleanup_in_flight = true;
                    composer.recovery.status =
                        "Sent confirmed; local recovery cleanup is pending.".into();
                }
                Err(error) => composer.recovery.status = error,
            }
            cx.notify();
            return;
        }
        composer.recovery.retry = true;
        self.capture_recovery(cx);
        cx.notify();
    }

    fn capture_recovery(&mut self, cx: &App) {
        let (Some(service), Some(composer)) = (&mut self.recovery, &mut self.composer) else {
            return;
        };
        if composer.recovery.key.is_none() {
            match service.key(composer.message.from_account_id) {
                Ok(key) => composer.recovery.key = Some(key),
                Err(error) => {
                    composer.recovery.status = error;
                    return;
                }
            }
        }
        if !composer.recovery.dirty_input && !composer.recovery.retry {
            return;
        }
        let changed = if composer.recovery.dirty_input {
            let outgoing = composer.outgoing(cx);
            match composer.recovery.observe(outgoing) {
                Ok(changed) => {
                    composer.recovery.dirty_input = false;
                    changed
                }
                Err(error) => {
                    composer.recovery.status = error;
                    return;
                }
            }
        } else {
            false
        };
        if !changed && !composer.recovery.retry {
            return;
        }
        // Thunderbird source identity/config is not a native AccountConfig.
        // Do not synthesize one and claim durable From validation.
        let Some((_, account)) = self
            .profiles
            .iter()
            .find(|(id, _)| *id == composer.message.from_account_id)
        else {
            composer.recovery.requires_ack = false;
            composer.recovery.status =
                "Session only: this adapter has no validated native recovery From configuration."
                    .into();
            return;
        };
        match composer
            .recovery
            .update(account)
            .and_then(|command| service.submit(command))
        {
            Ok(()) => {
                composer.recovery.retry = false;
                composer.recovery.in_flight = Some(composer.recovery.generation);
                composer.recovery.status =
                    "Local recovery pending; keep this composer open.".into();
            }
            Err(error) => {
                composer.recovery.retry = true;
                composer.recovery.status = error;
            }
        }
    }

    fn begin_provider_authorization(
        &mut self,
        provider: megamail_core::provider_auth::Provider,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.live.read(cx).snapshot();
        if self.demo
            || self.loading_profiles
            || self.startup_error.is_some()
            || snapshot.thunderbird_loading
            || self.live.read(cx).direct_add_block_reason().is_some()
        {
            return;
        }
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        if setup.connecting || setup.discovering || setup.thunderbird_loading {
            return;
        }
        let available = self
            .auth_availability
            .as_ref()
            .is_some_and(|a| match provider {
                megamail_core::provider_auth::Provider::GoogleImapSmtp => a.google.is_ok(),
                _ => a.microsoft.is_ok(),
            });
        if !available {
            setup.error =
                Some(megamail_core::provider_auth::AuthError::RegistrationRequired.to_string());
            cx.notify();
            return;
        }
        let email = setup.inputs.email.read(cx).value().to_string();
        if let Err(error) = provider_setup::check_new_account(
            &email,
            self.profiles.iter().map(|(_, p)| p.email.as_str()),
            snapshot.accounts.iter().map(|a| a.email.as_str()),
        ) {
            setup.error = Some(error.to_string());
            cx.notify();
            return;
        }
        setup.connecting = true;
        setup.error = None;
        self.auth_inflight = true;
        self.auth_cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let controller = self.auth_controller.clone();
        let cancellation = self.auth_cancel.clone();
        let cancelled = self.auth_cancelled.clone();
        let known_accounts = snapshot.accounts;
        let task = cx.background_spawn(async move {
            provider_setup::authorize(
                controller,
                provider,
                email,
                known_accounts,
                cancellation,
                cancelled,
            )
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                // A committed SavedAccount wins a last-moment cancellation.
                this.auth_inflight = false;
                match result {
                    Ok(added) => {
                        this.profiles = added.profiles;
                        let (id, profile) = added.added;
                        match this.live.update(cx, |live, cx| live.add_profile(id, profile, cx)) {
                            Ok(()) => { this.setup = None; this.setup_email_subscription = None; this.view = View::Mailbox; },
                            Err(error) => { if let Some(setup) = &mut this.setup { setup.connecting = false; setup.error = Some(format!("Account saved, not connected: {error}. Restart after repairing storage/runtime.")); } },
                        }
                    },
                    Err(error) => {
                        if error.starts_with("The account was saved, but its profile could not be reloaded:") { this.startup_error = Some("Account saved, but reload failed. Repair profile storage and restart before adding another account.".into()); }
                        if let Some(setup) = &mut this.setup { setup.connecting = false; setup.error = Some(error); }
                    },
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn cancel_provider_authorization(&mut self, cx: &mut Context<Self>) {
        self.auth_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Ok(slot) = self.auth_cancel.try_lock() {
            if let Some(attempt) = slot.as_ref() {
                attempt.cancel();
            }
        }
        if let Some(setup) = &mut self.setup {
            setup.error = Some("Cancelling sign-in. If save already started, its actual result will still be shown.".into());
        }
        cx.notify();
    }

    fn open_original_formatting(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let (Some(original), Some(row)) = (snapshot.selected_original, snapshot.selected_message)
        else {
            self.reader_session.notice = Some("Complete original HTML is unavailable for this selection. Native selectable formatting remains available.".into());
            cx.notify();
            return;
        };
        self.retire_reader_session(cx);
        if self.reader_session.exhausted {
            self.reader_session.notice = Some(
                "Reader generation space exhausted. Original viewing is disabled until restart."
                    .into(),
            );
            cx.notify();
            return;
        }
        let scope = reader_session::scope(
            &row.key,
            snapshot.selected_body_version,
            self.reader_session.generation,
        );
        self.reader_session.active = Some(scope.clone());
        let semantic: Arc<str> = Arc::from(snapshot.selected_html.as_ref());
        let plain: Arc<str> = Arc::from(row.message.body.as_str());
        let title: Arc<str> = Arc::from(row.message.subject.as_str());
        let worker_scope = scope.clone();
        let task = cx.background_spawn(async move {
            let path = reader_session::installed_helper()?;
            let document = original.document(worker_scope, semantic, plain, title)?;
            reader_host::ReaderHost::start(path, document).map_err(|_| {
                "Original reader could not start; native text remains available.".to_owned()
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.reader_session.accepts(&scope) {
                    match result {
                        Ok(host) => this.reader_session.host = Some(host),
                        Err(error) => {
                            this.reader_session.notice = Some(error);
                            this.retire_reader_session(cx);
                        }
                    }
                    cx.notify();
                } else if let Ok(host) = result {
                    host.cancel();
                }
            });
        })
        .detach();
        cx.notify();
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

    /// One background inventory load for startup and explicit storage Retry.
    /// Keep the original LiveMailbox and its event channel for the whole session.
    fn load_saved_accounts(&mut self, cx: &mut Context<Self>) {
        if self.live.read(cx).snapshot().thunderbird_loading {
            return;
        }
        self.loading_profiles = true;
        cx.spawn(async move |this, cx| {
            let inventory = cx
                .background_spawn(async move {
                    compose::prune_stale_draft_attachment_dirs();
                    account_session::StartupInventory::load()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.loading_profiles = false;
                let route = inventory.route();
                this.startup_error = inventory.direct_error.clone();
                // A cross-backend identity conflict can still restore known
                // direct workers. Retain their sender/alias configs, and never
                // replace a healthy roster with an error-derived empty list.
                let direct = (!inventory.direct.is_empty() || inventory.direct_error.is_none())
                    .then(|| inventory.direct.clone());
                this.live
                    .update(cx, |live, cx| live.install_inventory(inventory, cx));
                if let Some(direct) = direct {
                    this.profiles = direct
                        .into_iter()
                        .filter(|(id, _)| this.live.read(cx).is_direct_account(*id))
                        .collect();
                }
                if route != account_session::StartupRoute::FirstRun {
                    this.setup = None;
                    this.setup_email_subscription = None;
                    this.setup_return_view = View::Mailbox;
                    this.view = startup_view(route);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reader_text_identity(
        &self,
        snapshot: &MailboxSnapshot,
    ) -> Option<reader_session::SemanticTextIdentity> {
        let key = snapshot.selected_key.as_ref()?;
        // Exhausting the external helper's generation budget must not disable
        // the independently selectable native body.
        if self.collapsed_messages.contains(key)
            || snapshot.selected_message.as_ref().map(|row| &row.key) != Some(key)
        {
            return None;
        }
        self.reader_bodies
            .iter()
            .find(|body| &body.key == key && (!body.text.is_empty() || !body.html.is_empty()))?;
        Some(reader_session::SemanticTextIdentity {
            key: key.clone(),
            version: snapshot.selected_body_version,
            generation: self.reader_session.generation,
            plain: self.plain_messages.contains(key),
            quote_visible: (!self.preferences.hide_quoted_text
                || self.visible_quotes.contains(key))
                && !self.hidden_quotes.contains(key),
        })
    }

    /// Helper retirement also changes the current semantic identity. Update
    /// the real native consumer before notifying/rendering, even on refusal.
    fn retire_reader_session(&mut self, cx: &mut Context<Self>) {
        self.reader_session.retire();
        let snapshot = self.live.read(cx).snapshot();
        self.refresh_reader_text(&snapshot, cx);
    }

    /// Keep the actual selected TextView consumer outside the render path.
    fn refresh_reader_text(&mut self, snapshot: &MailboxSnapshot, cx: &mut Context<Self>) {
        self.refresh_semantic_reader_text(snapshot, cx);
    }
    fn refresh_semantic_reader_text(&mut self, snapshot: &MailboxSnapshot, cx: &mut Context<Self>) {
        let Some(identity) = self.reader_text_identity(snapshot) else {
            self.reader_text = None;
            self.reader_find.clear();
            self.attachment_preview.clear();
            self.rule_preview.clear();
            self.newsletter_status = None;
            self.rule_move_plan = None;
            self.reader_export_status = None;
            return;
        };
        let Some(body) = self
            .reader_bodies
            .iter()
            .find(|body| body.key == identity.key)
        else {
            self.reader_text = None;
            self.reader_find.clear();
            self.attachment_preview.clear();
            self.rule_preview.clear();
            self.newsletter_status = None;
            self.rule_move_plan = None;
            self.reader_export_status = None;
            return;
        };
        let html = body.markup(identity.plain, false);
        if let Some(target) = &mut self.reader_text {
            if target.rebind_unchanged(identity.clone(), &html) {
                return;
            }
        }
        // A replaced body/mode gets a new state, so its old selection cannot
        // survive into another message or an asynchronously parsed replacement.
        self.rule_preview.clear();
        self.newsletter_status = None;
        self.rule_move_plan = None;
        self.reader_export_status = None;
        self.reader_find.clear();
        self.attachment_preview.clear();
        let plain = identity.plain;
        self.reader_text = Some(reader_session::SemanticTextTarget {
            identity,
            html,
            state: if plain {
                body.plain_state.as_ref().unwrap().clone()
            } else {
                body.current_state.as_ref().unwrap().clone()
            },
        });
    }

    fn on_live_change(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        if let Some(row) = &snapshot.selected_message {
            self.load_sender_photo(&row.message.from_addr, cx);
            if snapshot.selected_sender_check.as_ref().is_some_and(|check| {
                check.trust == megamail_core::models::SenderTrust::Pass
            }) {
                let email = row.message.from_addr.clone();
                self.fetch_verified_logo(&email, cx);
            }
            if let Some(conversation) = snapshot
                .conversations
                .iter()
                .find(|thread| thread.members.iter().any(|member| member.key == row.key))
            {
                for member in conversation.members.iter().take(32) {
                    self.load_sender_photo(&member.message.from_addr, cx);
                }
            }
        }
        for card in &mut self.comfort.outgoing {
            if let Some(result) = self
                .live
                .read(cx)
                .send_result_for(card.account, card.request)
            {
                card.status = if result.uncertain {
                    "Outcome unknown · review Outbox before resending"
                } else {
                    match result.outcome {
                        SendOutcome::Sent => "Server accepted · waiting for Sent copy",
                        SendOutcome::Queued => "Saved to Outbox · not sent yet",
                        SendOutcome::Failed => "Send failed · your text remains in the draft",
                    }
                };
            }
        }
        self.comfort.outgoing.retain(|card| {
            !snapshot
                .conversations
                .iter()
                .flat_map(|t| t.members.iter())
                .any(|m| {
                    m.key.account_id == card.account
                        && comfort::normalized_id(&m.message.message_id) == card.message_id
                })
        });
        let retired = changed_mailbox_scopes(&self.reader_mailbox_epochs, &snapshot.mailbox_epochs);
        let same_source = match (&self.reader_source, &snapshot.selected_original) {
            (Some(previous), Some(current)) => Arc::ptr_eq(previous, current),
            (None, None) => true,
            _ => false,
        };
        if !same_source || self.reader_selected_key != snapshot.selected_key || !retired.is_empty()
        {
            self.reader_session.retire();
            self.reader_session.notice = None;
            self.reader_source = snapshot.selected_original.clone();
            self.invitation_preview = None;
            self.invitation_error = None;
            if let (Some(row), Some(version), Some(check)) = (
                &snapshot.selected_message,
                snapshot.selected_body_version,
                &snapshot.selected_sender_check,
            ) {
                let key = row.key.clone();
                let generation = self.reader_session.generation;
                let check = check.clone();
                let metadata_key = invitation::InvitationMessageKey {
                    scope: contacts::ContactScope {
                        backend: contacts::ContactBackend::Direct,
                        account_id: key.account_id,
                    },
                    folder_id: row.message.folder_id,
                    message_id: row.message.id,
                    session_epoch: version.source_generation,
                    body_generation: generation,
                };
                let task = cx.background_spawn(async move {
                    invitation::from_sender_check(metadata_key, &check).map_err(|e| e.to_string())
                });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    let _ = this.update(cx, |this, cx| {
                        let current = this.live.read(cx).snapshot();
                        if !this.reader_session.exhausted
                            && reader_session::selection_matches(
                                &key,
                                Some(version),
                                generation,
                                current.selected_key.as_ref(),
                                current.selected_body_version,
                                this.reader_session.generation,
                            )
                        {
                            match result {
                                Ok(preview) => this.invitation_preview = preview.map(Arc::new),
                                Err(error) => this.invitation_error = Some(error),
                            }
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        if !retired.is_empty() {
            retire_reader_body_cache(&mut self.reader_bodies, &retired);
            self.plain_messages
                .retain(|key| reader_scope_retained(key, &retired));
            self.visible_quotes
                .retain(|key| reader_scope_retained(key, &retired));
            self.hidden_quotes
                .retain(|key| reader_scope_retained(key, &retired));
            self.link_details
                .retain(|key| reader_scope_retained(key, &retired));
            self.expanded_messages
                .retain(|key| reader_scope_retained(key, &retired));
            self.collapsed_messages
                .retain(|key| reader_scope_retained(key, &retired));
            // Conversation identities may include replies from the retired folder.
            self.expanded_conversations
                .retain(|key| !retired.iter().any(|(id, _)| key.account_id == *id));
        }
        self.reader_mailbox_epochs = snapshot.mailbox_epochs.clone();
        self.rebuild_list_rows(&snapshot, cx);
        let selected_position = snapshot.selected_key.as_ref().and_then(|key| {
            snapshot
                .conversations
                .iter()
                .find_map(|thread| thread.members.iter().position(|member| &member.key == key))
        });
        if self.reader_selected_key != snapshot.selected_key
            || self.reader_selected_position != selected_position
        {
            if self.reader_selected_key != snapshot.selected_key {
                let members = snapshot.conversations.iter().find(|thread| {
                    thread
                        .members
                        .iter()
                        .any(|member| Some(&member.key) == snapshot.selected_key.as_ref())
                });
                self.reader_follow_selection = reader_follows_selection(
                    self.reader_selected_key.as_ref(),
                    snapshot.selected_key.as_ref(),
                    members
                        .into_iter()
                        .flat_map(|thread| thread.members.iter().map(|member| &member.key)),
                );
            }
            self.reader_selected_key = snapshot.selected_key.clone();
            self.reader_selected_position = selected_position;
            self.reader_should_focus = true;
            self.prefetch_around_selection(&snapshot, cx);
        }
        if let Some(row) = snapshot.selected_message.as_ref().filter(|row| {
            snapshot.body_loaded
                || !row.message.body.is_empty()
                || !snapshot.selected_html.is_empty()
        }) {
            if !self.reader_bodies.iter().any(|cached| {
                cached.key == row.key
                    && cached.text.as_ref() == row.message.body
                    && cached.source_html.as_ref() == snapshot.selected_html.as_ref()
            }) {
                let mut cached = ReaderBody::new(
                    row.key.clone(),
                    &row.message.body,
                    snapshot.selected_html.clone(),
                    snapshot.selected_html_without_quote.clone(),
                );
                cached.prepare(cx);
                self.reader_bodies.retain(|cached| cached.key != row.key);
                self.reader_bodies.push_front(cached);
                while self.reader_bodies.len() > 8
                    || self
                        .reader_bodies
                        .iter()
                        .map(|cached| cached.retained_bytes)
                        .sum::<usize>()
                        > 32 * 1024 * 1024
                {
                    if let Some(evicted) = self.reader_bodies.pop_back() {
                        self.plain_messages.remove(&evicted.key);
                    }
                }
            }
        }
        self.refresh_reader_text(&snapshot, cx);
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
                self.thunderbird_auto_open_pending = false;
                self.thunderbird_auto_open_seen_loading = false;
                if !snapshot.accounts.is_empty() && self.view == View::Setup {
                    self.setup = None;
                    self.setup_email_subscription = None;
                    self.view = View::Mailbox;
                }
            }
        }
        let mut close_composer = false;
        if let Some(composer) = self.composer.as_mut() {
            if let Some(ticket) = composer.draft_ticket {
                if self.live.update(cx, |live, _| {
                    live.take_draft_saved_for(composer.message.from_account_id, ticket)
                }) {
                    self.request_attachment_leases.finish(
                        false,
                        composer.message.from_account_id,
                        u64::from(ticket),
                    );
                    composer.draft_saved = composer.draft_pending.take();
                    composer.draft_ticket = None;
                    composer.draft_save_completed = true;
                    composer.error = None;
                } else if snapshot.action_error.is_some() || snapshot.error.is_some() {
                    // Legacy failures carry no ticket. Never unlock/drop leases
                    // on unrelated current-account status/error prose.
                    composer.error = Some("Draft save is awaiting its account/request acknowledgment. The legacy error is uncorrelated; keep this composition open and inspect Drafts before retrying.".into());
                }
            }
        }
        let composer_result = self.composer.as_ref().and_then(|composer| {
            composer.send_request_id.and_then(|request_id| {
                self.live
                    .read(cx)
                    .send_result_for(composer.message.from_account_id, request_id)
            })
        });
        if let (Some(composer), Some(result)) = (&mut self.composer, composer_result) {
            if composer.send_request_id == Some(result.request_id)
                && composer.message.from_account_id == result.account_id
            {
                if let (Some(service), Some(key), Some(generation)) = (
                    &self.recovery,
                    composer.recovery.key.clone(),
                    composer.send_generation,
                ) {
                    if result.outcome == SendOutcome::Sent && !result.uncertain {
                        composer.recovery.finished_send =
                            Some((result.account_id, result.request_id, generation));
                    }
                    let cleanup = service.submit(draft_recovery::WriterCommand::SendFinished {
                        key,
                        generation,
                        expected_request: result.request_id,
                        account_id: result.account_id,
                        request_id: result.request_id,
                        outcome: result.outcome,
                        uncertain: result.uncertain,
                    });
                    match cleanup {
                        Ok(()) => {
                            composer.recovery.cleanup_in_flight =
                                composer.recovery.finished_send.is_some()
                        }
                        Err(error) => {
                            self.recovery_error = Some(error.clone());
                            composer.recovery.status = error;
                        }
                    }
                }
                self.request_attachment_leases
                    .finish(true, result.account_id, result.request_id);
                composer.send_request_id = None;
                if result.uncertain {
                    composer.send_queued = false;
                    composer.send_uncertain = true;
                    composer.error = None;
                } else {
                    match result.outcome {
                        SendOutcome::Sent => {
                            composer.sent_complete = true;
                            close_composer = self.recovery.is_none()
                                || !composer.recovery.requires_ack
                                || composer.recovery.retired == Some(composer.recovery.generation);
                        }
                        SendOutcome::Queued if self.undo_send.as_ref().is_some_and(|u| u.request_id == result.request_id) => {
                            // Undo Send: the composer closes at once, like a send.
                            composer.send_queued = true;
                            close_composer = true;
                        }
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
        // Only the existing owning AccountConfig can authorize this choice.
        // Runtime-discovered aliases without that validator remain display-only.
        let Some((_, profile)) = self.profiles.iter().find(|(id, _)| *id == account_id) else {
            composer.error = Some(
                "Sender switching is unavailable for this adapter's unvalidated identities.".into(),
            );
            cx.notify();
            return;
        };
        let choice = choices[next]
            .alias
            .as_ref()
            .map_or(compose_productivity::SenderChoice::Primary, |identity| {
                compose_productivity::SenderChoice::Alias(identity.clone())
            });
        if let Err(error) =
            compose_productivity::choose_sender(&mut composer.message, account_id, profile, &choice)
        {
            composer.error = Some(format!("Sender choice rejected: {error:?}"));
            cx.notify();
            return;
        }
        composer.recovery.dirty_input = true;
        composer.draft_saved = None;
        composer.error = None;
        self.capture_recovery(cx);
        cx.notify();
    }

    fn cleanup_composer_attachments(&mut self, cx: &App) {
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        if let Some(lease) = composer.recovery.lease.take() {
            if let Some(service) = &self.recovery {
                if let Err(error) = service.release(lease) {
                    self.recovery_error = Some(error);
                }
            } else {
                std::mem::forget(lease);
            }
        }
        let paths = std::mem::take(&mut composer.temporary_attachment_paths);
        if !paths.is_empty() {
            cx.background_executor()
                .spawn(async move { compose::remove_staged_attachments(paths) })
                .detach();
        }
    }

    fn can_replace_composer(&mut self, cx: &App) -> bool {
        self.capture_recovery(cx);
        let Some(composer) = self.composer.as_mut() else {
            return true;
        };
        let error = if self.recovery.is_some()
            && composer.recovery.requires_ack
            && if composer.sent_complete {
                composer.recovery.retired != Some(composer.recovery.generation)
            } else {
                composer.recovery.acknowledged != composer.recovery.generation
            } {
            Some(
                "Local recovery has not acknowledged this composition. Keep it open or retry recovery before replacing it.",
            )
        } else if composer.send_uncertain {
            Some(
                "Inspect Sent/Outbox separately. Resend remains locked because durable send-ticket reconciliation is unavailable.",
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
        composer.error = Some("Resend remains locked: this adapter cannot reconcile the original durable send ticket. Inspect Sent/Outbox separately; a text confirmation cannot release a held intent.".into());
        cx.notify();
    }

    /// Panes are frosted glass over the wallpaper: the (already blurred and
    /// faded) image shows through in proportion to the wallpaper strength.
    fn pane_surface(&self, color: gpui_kit::Hsla, bleed: f32) -> gpui_kit::Hsla {
        if !self.appearance.has_wallpaper() {
            return gpui_kit::Hsla { a: 1.0, ..color };
        }
        let show = self.appearance.visual_wallpaper_opacity().clamp(0.0, 1.0);
        let alpha = 1.0 - show * (0.38 + bleed.clamp(0.0, 0.3));
        gpui_kit::Hsla { a: alpha.clamp(0.45, 1.0), ..color }
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
        if self.preferences.reduced_motion || cx.reduce_motion() {
            self.appearance
                .tick_transition(std::time::Instant::now() + Duration::from_secs(1));
            cx.notify();
            return;
        }
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

    fn change_preference(&mut self, change: PreferenceChange, cx: &mut Context<Self>) {
        if self.preferences_busy || self.preferences_loading {
            return;
        }
        let mut proposed = self.preferences.clone();
        match change {
            PreferenceChange::GroupConversations(value) => proposed.group_conversations = value,
            PreferenceChange::Density(value) => proposed.density = value,
            PreferenceChange::HideQuotedText(value) => proposed.hide_quoted_text = value,
            PreferenceChange::ReducedMotion(value) => proposed.reduced_motion = value,
            PreferenceChange::DefaultUnified(value) => proposed.default_unified = value,
            PreferenceChange::ListAvatars(value) => proposed.list_avatars = value,
            PreferenceChange::InternetAvatars(value) => proposed.internet_avatars = value,
            PreferenceChange::Gravatar(value) => proposed.gravatar = value,
            PreferenceChange::LoadRemote(value) => proposed.load_remote = value,
            PreferenceChange::Layout(value) => proposed.layout = value,
        }
        self.preferences_busy = true;
        self.preferences_error = None;
        let task = cx.background_spawn(async move { proposed.persist().map(|_| proposed) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.preferences_busy = false;
                match result {
                    Ok(preferences) => {
                        if (preferences.internet_avatars, preferences.gravatar)
                            != (this.preferences.internet_avatars, this.preferences.gravatar)
                        {
                            this.sender_photos.forget_domain(None);
                            this.logo_attempts.clear();
                        }
                        this.preferences = preferences;
                        this.list_scroll = UniformListScrollHandle::new();
                        let snapshot = this.live.read(cx).snapshot();
                        this.rebuild_list_rows(&snapshot, cx);
                        this.refresh_reader_text(&snapshot, cx);
                        if this.preferences.reduced_motion {
                            this.appearance.tick_transition(
                                std::time::Instant::now() + Duration::from_secs(1),
                            );
                        }
                    }
                    Err(error) => this.preferences_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_conversation(&mut self, key: ConversationKey, cx: &mut Context<Self>) {
        if !self.expanded_conversations.remove(&key) {
            self.expanded_conversations.insert(key.clone());
            self.live
                .update(cx, |live, cx| live.select_conversation(key, cx));
        }
        self.rebuild_list_rows(&self.live.read(cx).snapshot(), cx);
        cx.notify();
    }

    fn rebuild_list_rows(&mut self, snapshot: &MailboxSnapshot, cx: &mut Context<Self>) {
        let mut parents: Vec<ListRow> = if self.preferences.group_conversations {
            snapshot
                .conversations
                .iter()
                .map(|t| ListRow {
                    row: t.representative.clone(),
                    conversation: Some(t.clone()),
                    child: false,
                })
                .collect()
        } else {
            snapshot
                .page
                .iter()
                .map(|row| ListRow {
                    row: row.clone(),
                    conversation: None,
                    child: false,
                })
                .collect()
        };
        if snapshot.action_error.is_some() {
            self.hidden_rows.clear();
        }
        self.hidden_rows
            .retain(|key| snapshot.page.iter().any(|row| &row.key == key));
        parents.retain(|row| !self.hidden_rows.contains(&row.row.key));
        let mut split_unread = [0usize; 5];
        let classified: Vec<Option<splits::Split>> = parents
            .iter()
            .map(|row| self.split_of(row))
            .collect();
        for (row, split) in parents.iter().zip(&classified) {
            let unread = row
                .conversation
                .as_ref()
                .map_or(row.row.message.unread, |thread| thread.unread > 0);
            if unread {
                split_unread[0] += 1;
                if let Some(split) = split {
                    if let Some(index) = splits::Split::ALL.iter().position(|s| s == split) {
                        split_unread[index] += 1;
                    }
                }
            }
        }
        self.split_unread = split_unread;
        if self.split != splits::Split::All {
            let wanted = Some(self.split);
            let mut index = 0;
            parents.retain(|_| {
                let keep = classified[index] == wanted;
                index += 1;
                keep
            });
        }
        parents.retain(|row| {
            self.comfort.filter.is_none_or(|wanted| {
                self.work_status(row.conversation.as_ref(), &row.row, snapshot) == wanted
            })
        });
        parents.sort_by(|a, b| {
            let am = comfort::latest(a);
            let bm = comfort::latest(b);
            let order = match self.comfort.sort {
                comfort::Sort::Date => bm.timestamp.cmp(&am.timestamp),
                comfort::Sort::Sender => a
                    .row
                    .message
                    .from_addr
                    .to_ascii_lowercase()
                    .cmp(&b.row.message.from_addr.to_ascii_lowercase()),
                comfort::Sort::Subject => am
                    .subject
                    .to_ascii_lowercase()
                    .cmp(&bm.subject.to_ascii_lowercase()),
                comfort::Sort::Unread => b
                    .conversation
                    .as_ref()
                    .map_or(bm.unread, |t| t.unread > 0)
                    .cmp(&a.conversation.as_ref().map_or(am.unread, |t| t.unread > 0))
                    .then_with(|| bm.timestamp.cmp(&am.timestamp)),
            }
            .then_with(|| am.message_id.cmp(&bm.message_id))
            .then_with(|| a.row.key.account_id.cmp(&b.row.key.account_id))
            .then_with(|| a.row.key.folder_path.cmp(&b.row.key.folder_path))
            .then_with(|| a.row.key.uid.cmp(&b.row.key.uid));
            if self.comfort.reverse {
                order.reverse()
            } else {
                order
            }
        });
        let mut rows = Vec::new();
        for row in parents {
            let thread = row.conversation.clone();
            rows.push(row);
            if let Some(t) = thread.filter(|t| self.expanded_conversations.contains(&t.key)) {
                rows.extend(t.members.iter().map(|row| ListRow {
                    row: row.clone(),
                    conversation: None,
                    child: true,
                }));
            }
        }
        self.list_rows = Arc::new(rows);
        if self.preferences.list_avatars {
            let senders: Vec<String> = self
                .list_rows
                .iter()
                .filter(|row| !row.child)
                .take(120)
                .map(|row| row.row.message.from_addr.clone())
                .collect();
            for sender in senders {
                self.load_sender_photo(&sender, cx);
            }
        }
        self.refresh_triage(snapshot, cx);
        self.pump_triage(cx);
    }

    fn restore_wallpaper(&mut self, cx: &mut Context<Self>) {
        let light = !self.dark;
        let Some(request) = self.appearance.restore_request(light) else {
            return;
        };
        self.wallpaper_restore_generation = self.wallpaper_restore_generation.wrapping_add(1);
        let generation = self.wallpaper_restore_generation;
        self.appearance_busy = true;
        let task = cx.background_spawn(async move { appearance::begin_wallpaper_restore(request) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.wallpaper_restore_generation != generation {
                    return;
                }
                match result {
                    Ok(result) => {
                        this.appearance_error = None;
                        if !this.appearance.install_processed_wallpaper(result, !this.dark) {
                            this.appearance_error = Some(
                                "The saved wallpaper changed while it was loading. Reopen Settings to retry.".into(),
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
        if self.appearance_busy || self.appearance_loading || self.theme_busy {
            return;
        }
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
        if self.composer.is_some() {
            self.view = View::Compose;
            cx.notify();
            return;
        }
        if !self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::ComposeMessage)
        {
            return;
        }
        let Some(account_id) = self.live.read(cx).composer_account_id() else {
            return;
        };
        if !self.can_replace_composer(cx) {
            return;
        }
        let mut composer = Composer::new(compose::new_message(account_id), window, cx);
        composer.recovery.retention = draft_recovery::Retention::Ordinary;
        self.composer = Some(composer);
        self.view = View::Compose;
        cx.notify();
    }

    fn open_reply(&mut self, message: Message, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::ReplyMessage)
        {
            return;
        }
        if !self.can_replace_composer(cx) {
            return;
        }
        let account_id = message.account_id;
        let reply = compose::reply_for_account(
            account_id,
            &message,
            &self.own_addresses(account_id, cx),
            false,
        );
        self.open_reply_composer(reply, window, cx);
    }

    fn own_addresses(&self, account_id: u32, cx: &App) -> Vec<String> {
        let snapshot = self.live.read(cx).snapshot();
        let mut addresses = snapshot
            .accounts
            .iter()
            .filter(|account| account.id == account_id)
            .map(|account| account.email.clone())
            .collect::<Vec<_>>();
        addresses.extend(
            self.live
                .read(cx)
                .sender_identities(account_id)
                .into_iter()
                .map(|identity| identity.email),
        );
        if let Some((_, profile)) = self.profiles.iter().find(|(id, _)| *id == account_id) {
            addresses.extend(
                profile
                    .aliases
                    .iter()
                    .map(|alias| alias.address().to_owned()),
            );
        }
        addresses
    }

    fn open_reply_all(&mut self, message: Message, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::ReplyAll)
        {
            return;
        }
        if !self.can_replace_composer(cx) {
            return;
        }
        let account_id = message.account_id;
        let reply = compose::reply_for_account(
            account_id,
            &message,
            &self.own_addresses(account_id, cx),
            true,
        );
        self.open_reply_composer(reply, window, cx);
    }

    fn open_reply_composer(
        &mut self,
        mut reply: OutgoingMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let quoted = reply.body.trim_start().to_owned();
        reply.body.clear();
        let mut composer = Composer::new(reply, window, cx);
        composer.quoted = (!quoted.is_empty()).then_some(quoted);
        composer.body.read(cx).focus_handle(cx).focus(window, cx);
        self.composer = Some(composer);
        self.view = View::Compose;
        cx.notify();
    }

    fn open_forward(&mut self, message: Message, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_replace_composer(cx) {
            return;
        }
        let account_id = message.account_id;
        let prepared = compose::prepare_forward(account_id, &message);
        self.composer = Some(Composer::new_prepared_draft(
            prepared.message,
            prepared.temporary_attachment_paths,
            prepared.notice,
            prepared.attachment_lease,
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
                        let attachment_lease = prepared.attachment_lease.clone();
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
                        let opened = this.update_in(cx, |this, window, cx| {
                            let accepted =
                                this.live
                                    .read(cx)
                                    .snapshot()
                                    .draft_source
                                    .is_some_and(|source| {
                                        source.key == key
                                            && source.token == token
                                            && matches!(
                                                source.state,
                                                DraftSourceState::Prepared(result)
                                                    if result.is_ok()
                                            )
                                    });
                            if !accepted || this.composer.is_some() || this.view != View::Mailbox {
                                this.live.update(cx, |live, cx| {
                                    live.dismiss_draft_source(&key, token, cx)
                                });
                                return false;
                            }
                            this.composer = Some(Composer::new_prepared_draft(
                                message,
                                paths_for_open,
                                notice,
                                attachment_lease,
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
        if setup.thunderbird_loading
            || account_registry::mutation_blocked(setup.connecting, importing)
            || self.loading_profiles
            || self.startup_error.is_some()
        {
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

        // One-time migration: Thunderbird's settings and saved logins become
        // native accounts; Thunderbird itself is never started.
        setup.connecting = true;
        let live_accounts = self.live.read(cx).snapshot().accounts;
        let task = cx.background_spawn(async move {
            let mut converted = Vec::new();
            let mut failures = Vec::new();
            for (profile, ids) in selections {
                match megamail_core::thunderbird_migrate::convert_profile(&profile, &ids) {
                    Ok(results) => {
                        for (label, result) in results {
                            match result {
                                Ok(config) => converted.push(config),
                                Err(error) => failures.push(format!("{label}: {error}")),
                            }
                        }
                    }
                    Err(error) => failures.push(error),
                }
            }
            let saved = provider_setup::add_migrated(converted, &live_accounts);
            (saved, failures)
        });
        cx.spawn(async move |this, cx| {
            let (saved, mut failures) = task.await;
            let _ = this.update(cx, |this, cx| {
                match saved {
                    Ok(profiles) if !profiles.is_empty() => {
                        let known: HashSet<u32> = this.profiles.iter().map(|(id, _)| *id).collect();
                        this.profiles = profiles.clone();
                        for (id, profile) in
                            profiles.into_iter().filter(|(id, _)| !known.contains(id))
                        {
                            let email = profile.email.clone();
                            if let Err(error) = this
                                .live
                                .update(cx, |live, cx| live.add_profile(id, profile, cx))
                            {
                                failures.push(format!("{email}: {error}"));
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) => failures.push(error),
                }
                if failures.is_empty() {
                    this.setup = None;
                    this.setup_email_subscription = None;
                    this.view = this.setup_return_view;
                    this.startup_error = None;
                } else if let Some(setup) = this.setup.as_mut() {
                    setup.connecting = false;
                    setup.thunderbird_error = Some(failures.join("\n"));
                }
                cx.notify();
            });
        })
        .detach();
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
        let guide = app_passwords::guide(
            &setup.candidate().map(|c| c.incoming.host.clone()).unwrap_or_default(),
            &form.email,
        );
        form.password = app_passwords::clean(guide, &setup.inputs.password.read(cx).value());
        form.smtp_username = setup.inputs.smtp_username.read(cx).value().to_string();
        form.smtp_password = app_passwords::clean(guide, &setup.inputs.smtp_password.read(cx).value());
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
        if account_registry::mutation_blocked(
            self.setup.as_ref().is_some_and(|setup| setup.connecting),
            self.live.read(cx).snapshot().thunderbird_loading,
        ) {
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
        if let Some(reason) = self.live.read(cx).direct_add_block_reason() {
            if let Some(setup) = self.setup.as_mut() {
                setup.error = Some(reason.into());
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
        let known_accounts = self.live.read(cx).snapshot().accounts;
        if let Err(error) = provider_setup::check_new_account(
            &account.email,
            self.profiles
                .iter()
                .map(|(_, profile)| profile.email.as_str()),
            known_accounts.iter().map(|account| account.email.as_str()),
        ) {
            if let Some(setup) = self.setup.as_mut() {
                setup.error = Some(error.to_string());
            }
            cx.notify();
            return;
        }
        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        setup.connecting = true;
        setup.error = None;
        let task = cx
            .background_spawn(async move { provider_setup::add_password(account, known_accounts) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(added) => {
                        this.profiles = added.profiles;
                        let (account_id, profile) = added.added;
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

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view != View::Mailbox {
            return;
        }
        if self.command_palette.is_some() {
            self.close_palette(window, cx);
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type a command…"));
        let subscription = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(palette) = this.command_palette.as_mut() {
                    palette.index = 0;
                }
                cx.notify();
            }
        });
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.command_palette = Some(PaletteState { input, index: 0, _subscription: subscription });
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette = None;
        if self.list_rows.is_empty() {
            self.mailbox_focus.focus(window, cx);
        } else {
            self.message_focus.focus(window, cx);
        }
        cx.notify();
    }

    fn run_palette_command(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = self.command_palette.as_ref() else {
            return;
        };
        let query = palette.input.read(cx).value().to_string();
        let Some(command) = command_palette::filter(&query).into_iter().nth(index) else {
            return;
        };
        self.close_palette(window, cx);
        window.dispatch_action(command.action(), cx);
    }

    fn palette_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(palette) = self.command_palette.as_mut() else {
            return false;
        };
        let count = command_palette::filter(&palette.input.read(cx).value()).len();
        match key {
            "escape" => self.close_palette(window, cx),
            "enter" => {
                let index = palette.index;
                self.run_palette_command(index, window, cx);
            }
            "down" if count > 0 => palette.index = (palette.index + 1) % count,
            "up" if count > 0 => palette.index = (palette.index + count - 1) % count,
            _ => return false,
        }
        cx.notify();
        true
    }

    fn palette_view(&self, palette: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.command_palette.as_ref()?;
        let query = state.input.read(cx).value().to_string();
        let commands = command_palette::filter(&query);
        let selected = state.index.min(commands.len().saturating_sub(1));
        let rows = commands.into_iter().enumerate().map(|(index, command)| {
            let active = index == selected;
            h_flex()
                .id(SharedString::from(format!("palette-{index}")))
                .w_full()
                .h(px(36.))
                .px_3()
                .rounded(px(6.))
                .items_center()
                .justify_between()
                .cursor_pointer()
                .when(active, |row| row.bg(palette.selected))
                .hover(|row| row.bg(palette.hover))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.run_palette_command(index, window, cx)
                }))
                .child(div().text_size(px(13.)).child(command.label))
                .child(shortcut_keys(command.keys, palette))
        });
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .justify_center()
                .pt(relative(0.14))
                .bg(gpui_kit::black().opacity(0.35))
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
                )
                .child(
                    v_flex()
                        .id("command-palette")
                        .w(px(560.))
                        .h_auto()
                        .self_start()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(palette.border)
                        .bg(palette.surface)
                        .shadow_lg()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .capture_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                            if this.palette_key(&event.keystroke.key, window, cx) {
                                cx.stop_propagation();
                            }
                        }))
                        .child(
                            div()
                                .p_2()
                                .border_b_1()
                                .border_color(palette.border)
                                .child(Input::new(&state.input).appearance(false).text_size(px(15.))),
                        )
                        .child(
                            v_flex()
                                .id("palette-rows")
                                .max_h(px(380.))
                                .p_1()
                                .gap_px()
                                .overflow_y_scroll()
                                .children(rows),
                        )
                        .map(|card| self.reveal(card, "palette-reveal", 10., 150, cx)),
                )
                .into_any_element(),
        )
    }

    /// Fade in and rise into place; nothing when motion is reduced.
    fn reveal<E: IntoElement + gpui_kit::Styled + 'static>(
        &self,
        element: E,
        id: impl Into<SharedString>,
        rise: f32,
        millis: u64,
        cx: &App,
    ) -> AnyElement {
        if self.preferences.reduced_motion || cx.reduce_motion() {
            return element.into_any_element();
        }
        element
            .with_animation(
                gpui_kit::ElementId::Name(id.into()),
                Animation::new(Duration::from_millis(millis)).with_easing(|t| 1. - (1. - t).powi(4)),
                move |element, t| element.opacity(t).mt(px(rise * (1. - t))),
            )
            .into_any_element()
    }

    /// Archive or trash the selection and move on to the next conversation
    /// immediately, the way Superhuman's E and # feel.
    fn triage_selected(&mut self, kind: FolderKind, cx: &mut Context<Self>) {
        if !self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::NextMessage)
        {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(selected) = snapshot.selected_key.clone() else {
            return;
        };
        let position = self.list_rows.iter().position(|row| {
            row.row.key == selected
                || row.conversation.as_ref().is_some_and(|thread| {
                    thread.members.iter().any(|member| member.key == selected)
                })
        });
        let next = position.and_then(|index| {
            self.list_rows
                .iter()
                .skip(index + 1)
                .find(|row| !row.child)
                .or_else(|| self.list_rows[..index].iter().rev().find(|row| !row.child))
                .map(|row| row.row.key.clone())
        });
        let hidden = position.map(|index| self.list_rows[index].row.key.clone());
        let moved = self.live.update(cx, |live, cx| match kind {
            FolderKind::Trash => live.trash_selected(cx),
            _ => live.archive_selected(cx),
        });
        if !moved {
            return;
        }
        if let Some(key) = hidden {
            self.hidden_rows.insert(key);
        }
        if let Some(key) = next {
            self.live.update(cx, |live, cx| live.select_message(key, cx));
        }
        let snapshot = self.live.read(cx).snapshot();
        self.rebuild_list_rows(&snapshot, cx);
        cx.notify();
    }

    /// Superhuman-style speed: fetch the next few bodies before they are
    /// opened, so J/K and E land on an already-cached message.
    fn prefetch_around_selection(&mut self, snapshot: &MailboxSnapshot, cx: &mut Context<Self>) {
        let Some(selected) = snapshot.selected_key.as_ref() else {
            return;
        };
        let Some(index) = self.list_rows.iter().position(|row| {
            &row.row.key == selected
                || row.conversation.as_ref().is_some_and(|thread| {
                    thread.members.iter().any(|member| &member.key == selected)
                })
        }) else {
            return;
        };
        let keys: Vec<MessageKey> = self
            .list_rows
            .iter()
            .skip(index + 1)
            .filter(|row| !row.child)
            .take(4)
            .chain(self.list_rows[..index].iter().rev().filter(|row| !row.child).take(1))
            .map(|row| row.row.key.clone())
            .collect();
        if !keys.is_empty() {
            let live = self.live.clone();
            cx.defer(move |cx| {
                live.update(cx, |live, cx| live.prefetch_bodies(&keys, cx));
            });
        }
    }


    fn set_reader_plain(&mut self, plain: bool, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let Some(key) = snapshot.selected_key.clone() else {
            return;
        };
        if plain {
            self.plain_messages.insert(key);
        } else {
            self.plain_messages.remove(&key);
        }
        self.refresh_reader_text(&snapshot, cx);
        cx.notify();
    }

    /// Act on one row without opening it first (hover buttons, right-click,
    /// swipe). `unread` is the state the row showed when it was clicked.
    fn row_action(&mut self, key: MessageKey, action: RowAction, unread: bool, cx: &mut Context<Self>) {
        self.live.update(cx, |live, cx| {
            live.select_message(key, cx);
        });
        match action {
            RowAction::Archive => self.triage_selected(FolderKind::Archive, cx),
            RowAction::Trash => self.triage_selected(FolderKind::Trash, cx),
            RowAction::Star => {
                let starred = self.live.read(cx).snapshot().selected_message.is_some_and(|row| row.message.starred);
                self.live.update(cx, |live, cx| live.set_selected_starred(!starred, cx));
            }
            RowAction::Read => {
                self.live.update(cx, |live, cx| live.set_selected_unread(!unread, cx));
            }
        }
        cx.notify();
    }

    /// Two-finger horizontal swipe on a row: left archives, right toggles
    /// read. The row follows the fingers and snaps back if released early.
    fn swipe_row(&mut self, key: MessageKey, dx: f32, ended: bool, unread: bool, cx: &mut Context<Self>) {
        let travel = match &self.swipe {
            Some((current, travel)) if *current == key => *travel,
            _ => 0.,
        };
        let travel = (travel + dx).clamp(-180., 180.);
        if ended {
            self.finish_swipe(unread, cx);
            return;
        }
        self.swipe = Some((key, travel));
        self.swipe_generation += 1;
        let generation = self.swipe_generation;
        // Mice and X11 send no "fingers lifted" event: settle after a pause.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(220)).await;
            let _ = this.update(cx, |this, cx| {
                if this.swipe_generation == generation {
                    this.finish_swipe(unread, cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_swipe(&mut self, unread: bool, cx: &mut Context<Self>) {
        let Some((key, travel)) = self.swipe.take() else {
            return;
        };
        if travel <= -SWIPE_COMMIT {
            self.row_action(key, RowAction::Archive, unread, cx);
        } else if travel >= SWIPE_COMMIT {
            self.row_action(key, RowAction::Read, unread, cx);
        }
        cx.notify();
    }

    /// Repaint the Undo Send countdown until its window closes.
    fn tick_undo_send(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                let open = this
                    .update(cx, |this, cx| {
                        let open = this
                            .undo_send
                            .as_ref()
                            .is_some_and(|u| u.send_at >= megamail_core::datefmt::now());
                        if !open {
                            this.undo_send = None;
                        }
                        cx.notify();
                        open
                    })
                    .unwrap_or(false);
                if !open {
                    break;
                }
            }
        })
        .detach();
    }

    fn undo_last_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(undo) = self.undo_send.take() else {
            return;
        };
        let snapshot = self.live.read(cx).snapshot();
        let session = self.live.read(cx).schedule_session(undo.account);
        let item = snapshot.outbox.iter().find(|item| {
            item.account_id == undo.account
                && item.send_at == Some(undo.send_at)
                && item.phase == megamail_core::models::OutboxPhase::Ready
        });
        let canceled = match (item, session) {
            (Some(item), Some(session)) if undo.send_at > megamail_core::datefmt::now() => {
                let request = megamail_core::worker::QueuedCancelRequest {
                    id: item.id,
                    send_at: Some(undo.send_at),
                    session,
                };
                self.live
                    .update(cx, |live, cx| live.cancel_queued(undo.account, request, cx))
            }
            _ => false,
        };
        if !canceled {
            self.reader_session.notice = Some("Too late to undo: the message has already gone to the server.".into());
            cx.notify();
            return;
        }
        if self.composer.is_none() {
            let mut original = undo.original;
            original.client_message_id = None;
            let mut composer = Composer::new(original, window, cx);
            composer.notice = Some("Send undone. Your message is back here.".into());
            composer.body.read(cx).focus_handle(cx).focus(window, cx);
            self.composer = Some(composer);
            self.view = View::Compose;
        }
        cx.notify();
    }

    fn undo_toast(&self, palette: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let undo = self.undo_send.as_ref()?;
        let left = undo.send_at - megamail_core::datefmt::now();
        if left < 0 {
            return None;
        }
        let toast = div()
                .absolute()
                .bottom(px(24.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .px_4()
                        .py_2()
                        .rounded(px(10.))
                        .bg(palette.text)
                        .text_color(palette.background)
                        .shadow_lg()
                        .child(div().text_size(px(13.)).child(format!(
                            "Sending “{}” in {left}s",
                            if undo.original.subject.trim().is_empty() { "(No subject)" } else { undo.original.subject.trim() }
                        )))
                        .child(
                            Button::new("undo-send")
                                .ghost()
                                .small()
                                .label("Undo · Z")
                                .on_click(cx.listener(|this, _, window, cx| this.undo_last_send(window, cx))),
                        ),
                )
                ;
        Some(self.reveal(toast, "undo-toast", -12., 220, cx))
    }

    /// One key unsubscribe: RFC 8058 one-click when the sender offers it
    /// (an HTTPS POST, no browser), otherwise the reviewed email or webpage.
    fn unsubscribe_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let (Some(row), Some(check)) = (snapshot.selected_message.clone(), snapshot.selected_sender_check.clone()) else {
            return;
        };
        let Some(metadata) = check.unsubscribe.clone() else {
            self.newsletter_status = Some("No unsubscribe option was found in this message.".into());
            cx.notify();
            return;
        };
        let one_click = metadata
            .one_click
            .clone()
            .filter(|url| metadata.in_headers && url.starts_with("https://") && megamail_core::mail_text::safe_browser_url(url));
        if let Some(url) = one_click.filter(|_| !self.demo) {
            let sender = if row.message.from_name.is_empty() { row.message.from_addr.clone() } else { row.message.from_name.clone() };
            self.newsletter_status = Some(format!("Unsubscribing from {sender}…"));
            let task = cx.background_spawn(async move {
                ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(10))
                    .redirects(0)
                    .build()
                    .post(&url)
                    .set("Content-Type", "application/x-www-form-urlencoded")
                    .send_string("List-Unsubscribe=One-Click")
                    .map(|response| response.status() < 400)
                    .unwrap_or(false)
            });
            cx.spawn(async move |this, cx| {
                let ok = task.await;
                let _ = this.update(cx, |this, cx| {
                    this.newsletter_status = Some(if ok {
                        format!("Unsubscribed from {sender}.")
                    } else {
                        "One-click unsubscribe failed. Use the email or webpage option below.".into()
                    });
                    if ok {
                        this.triage_selected(FolderKind::Archive, cx);
                    }
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
            return;
        }
        if metadata.mailto.is_some() || metadata.reply.is_some() {
            self.open_unsubscribe_draft(&row.key, &metadata, window, cx);
        } else if let Some(url) = metadata.web.clone() {
            self.open_unsubscribe_page(&row.key, &url, cx);
        } else {
            self.newsletter_status = Some("No supported unsubscribe option was found.".into());
            cx.notify();
        }
    }

    fn split_of(&self, row: &ListRow) -> Option<splits::Split> {
        let message = &row.row.message;
        let known = self.known_senders.contains(&message.from_addr.to_ascii_lowercase())
            || row.conversation.as_ref().is_some_and(|thread| thread.replied);
        splits::classify(&message.from_addr, &message.from_name, known, &self.screener)
    }

    fn set_split(&mut self, split: splits::Split, cx: &mut Context<Self>) {
        self.split = split;
        self.list_scroll = UniformListScrollHandle::new();
        let snapshot = self.live.read(cx).snapshot();
        self.rebuild_list_rows(&snapshot, cx);
        cx.notify();
    }

    /// Screener decision for the selected message's sender.
    fn screen_sender(&mut self, allow: bool, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let Some(row) = snapshot.selected_message.as_ref() else {
            return;
        };
        let address = row.message.from_addr.clone();
        // Move on first, the way archive does, then hide or promote the sender.
        let next = self
            .list_rows
            .iter()
            .position(|r| r.row.key == row.key)
            .and_then(|index| {
                self.list_rows
                    .iter()
                    .skip(index + 1)
                    .chain(self.list_rows[..index].iter().rev())
                    .find(|r| !r.child && !r.row.message.from_addr.eq_ignore_ascii_case(&address))
                    .map(|r| r.row.key.clone())
            });
        self.screener.decide(&address, allow);
        let screener = self.screener.clone();
        cx.background_spawn(async move {
            if let Err(error) = screener.save() {
                tracing::warn!("{error}");
            }
        })
        .detach();
        if let Some(key) = next {
            self.live.update(cx, |live, cx| live.select_message(key, cx));
        }
        let snapshot = self.live.read(cx).snapshot();
        self.rebuild_list_rows(&snapshot, cx);
        cx.notify();
    }

    fn go_to_kind(&mut self, kind: FolderKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.view != View::Mailbox {
            return;
        }
        self.comfort.outbox_open = false;
        self.comfort.filter = None;
        self.live
            .update(cx, |live, cx| live.select_unified_kind(Some(kind), cx));
        self.list_scroll = UniformListScrollHandle::new();
        self.message_focus.focus(window, cx);
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::NextMessage)
        {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        if self.list_rows.is_empty() {
            return;
        }
        let current = snapshot
            .selected_key
            .as_ref()
            .and_then(|selected| {
                self.list_rows
                    .iter()
                    .position(|row| &row.row.key == selected)
            })
            .or_else(|| {
                snapshot.selected_key.as_ref().and_then(|key| {
                    self.list_rows.iter().position(|row| {
                        row.conversation.as_ref().is_some_and(|conversation| {
                            conversation.members.iter().any(|member| &member.key == key)
                        })
                    })
                })
            })
            .unwrap_or(0);
        let next = (current as isize + delta).clamp(0, self.list_rows.len() as isize - 1) as usize;
        let key = self.list_rows[next].row.key.clone();
        let row_count = self.list_rows.len();
        self.draft_open_error = None;
        self.live.update(cx, |live, cx| {
            live.select_message(key, cx);
            if next + 1 == row_count && snapshot.has_more && delta > 0 {
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
        let selected = snapshot.scope != MailboxScope::Unified
            && snapshot.current_folder_path.as_deref() == Some(folder.path.as_str());
        let path = folder.path.clone();
        let button = Button::new(SharedString::from(format!("folder-{}", folder.id)))
            .custom(sidebar_button_variant(cx, palette, dark, selected))
            .small()
            .selected(selected)
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
            .tooltip(format!("{} · {}", folder.name, folder.path))
            .accessibility_label(format!("{} folder", folder.name))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.mailbox_focus.focus(window, cx);
                this.live
                    .update(cx, |live, cx| live.select_folder(path.clone(), cx));
                this.list_scroll = UniformListScrollHandle::new();
                this.comfort.outbox_open = false;
                this.comfort.filter = None;
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
        // Keys work from the first frame: with nothing focused, the list
        // (or the open conversation) takes focus.
        if self.view == View::Mailbox && window.focused(cx).is_none() && !self.command_palette.is_some() {
            self.message_focus.focus(window, cx);
        }
        let dark = cx.theme().is_dark();
        let palette = self.palette;
        let snapshot = self.live.read(cx).snapshot();
        let available_width = f32::from(window.viewport_size().width);
        self.window_scale = window.scale_factor();
        self.ensure_original_render(&snapshot, cx);
        let pane_scale = ((available_width - 1000.) / 280.).clamp(0., 1.);
        let rail_width = px(192. + 40. * pane_scale);
        let list_width = px(380. + 120. * pane_scale);
        let client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        // Admission, not a refresh hook: run once after the mailbox root mounts,
        // and once on each Setup/Compose/Settings/modal -> Mailbox transition.
        let admit = self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::FocusMessages);
        if admit && !self.mailbox_focus_admitted {
            self.mailbox_focus_admitted = true;
            cx.on_next_frame(window, |this, window, cx| {
                if !this
                    .action_state(cx)
                    .allows(desktop_actions::MailboxAction::FocusMessages)
                {
                    this.mailbox_focus_admitted = false;
                    return;
                }
                // Keep any live search, native reader, or button focus. A retired
                // Setup field is outside the now-mounted mailbox focus subtree.
                let usable = window
                    .focused(cx)
                    .is_some_and(|focused| this.mailbox_focus.contains(&focused, window));
                if !usable {
                    let target = if this.list_rows.is_empty() {
                        &this.mailbox_focus
                    } else {
                        &this.message_focus
                    };
                    target.focus(window, cx);
                }
            });
        } else if !admit {
            self.mailbox_focus_admitted = false;
        }
        if self.reader_should_focus && self.view == View::Mailbox {
            self.reader_should_focus = false;
            if self.reader_follow_selection && snapshot.selected_key.is_some() {
                self.reader_anchor.scroll_to(window, cx);
            } else {
                self.reader_scroll
                    .set_offset(gpui_kit::point(px(0.), px(0.)));
            }
        }
        // No app header: the panes start at the top edge, like Superhuman.
        // Client-decorated windows still get a bare strip to drag and close.
        // The app bar: always there, above lists, conversations and settings
        // alike — folders, search, layout, compose, settings.
        let bar = self.app_bar(palette, window, cx);
        let chrome = if client_decorated {
            TitleBar::new().h(px(46.)).child(bar).into_any_element()
        } else {
            bar
        };
        let body = if self.view == View::Appearance {
            h_flex()
                .flex_1()
                .min_h_0()
                .child(self.sidebar(&snapshot, palette, cx, rail_width))
                .child(self.appearance_view(palette, cx))
                .into_any_element()
        } else if self.subscriptions.is_some() && self.view == View::Mailbox {
            h_flex()
                .flex_1()
                .min_h_0()
                .child(self.sidebar(&snapshot, palette, cx, rail_width))
                .child(self.subscriptions_view(palette, cx))
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
            let layout = self.preferences.layout;
            let main = if layout != preferences::PaneLayout::Full {
                use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
                let list = self.message_pane(&snapshot, palette, cx, px(0.)).into_any_element();
                let reader = if self.view == View::Compose {
                    self.composer_view(&snapshot, palette, cx)
                } else {
                    self.reader(&snapshot, palette, cx)
                };
                if layout == preferences::PaneLayout::Right {
                    h_resizable("split-right")
                        .with_state(&self.split_right_panes)
                        .child(resizable_panel().size(px(520.)).size_range(px(340.)..px(900.)).child(list))
                        .child(resizable_panel().size_range(px(360.)..gpui_kit::Pixels::MAX).child(
                            div().size_full().border_l_1().border_color(palette.border).child(reader),
                        ))
                        .into_any_element()
                } else {
                    v_resizable("split-below")
                        .with_state(&self.split_below_panes)
                        .child(resizable_panel().size(px(320.)).size_range(px(160.)..px(800.)).child(list))
                        .child(resizable_panel().size_range(px(200.)..gpui_kit::Pixels::MAX).child(
                            div().size_full().border_t_1().border_color(palette.border).child(reader),
                        ))
                        .into_any_element()
                }
            } else if self.view == View::Compose {
                self.composer_view(&snapshot, palette, cx)
            } else if self.comfort.outbox_open
                || (self.conversation_open && snapshot.selected_message.is_some())
            {
                h_flex()
                    .size_full()
                    .min_h_0()
                    .child(self.conversation_view(&snapshot, palette, cx))
                    .when(snapshot.selected_message.is_some() && !self.comfort.outbox_open, |el| {
                        el.child(self.contact_pane(&snapshot, palette, cx))
                    })
                    .into_any_element()
            } else {
                self.message_pane(&snapshot, palette, cx, px(0.)).into_any_element()
            };
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .when(!self.sidebar_hidden, |el| {
                    el.child(
                        div()
                            .w(px(232.))
                            .h_full()
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(palette.border)
                            .child(self.sidebar(&snapshot, palette, cx, px(0.))),
                    )
                })
                .child(div().flex_1().min_w_0().h_full().child(main))
                .into_any_element()
        };
        let palette_overlay = self.palette_view(palette, cx);
        let undo_toast = self.undo_toast(palette, cx);
        window_border().child(
            v_flex()
                .size_full()
                .when(self.view == View::Mailbox, |el| {
                    el.track_focus(&self.mailbox_focus)
                        .key_context(desktop_actions::MAILBOX_CONTEXT)
                })
                .on_action(cx.listener(|this, _: &ComposeMessage, window, cx| {
                    this.open_new_message(window, cx)
                }))
                .on_action(cx.listener(|this, _: &ReplyMessage, window, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    if this
                        .action_state(cx)
                        .allows(desktop_actions::MailboxAction::ReplyMessage)
                    {
                        if let Some(row) = snapshot.selected_message {
                            this.open_reply(row.message.clone(), window, cx);
                        }
                    }
                }))
                .on_action(cx.listener(|this, _: &ReplyAll, window, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    if this
                        .action_state(cx)
                        .allows(desktop_actions::MailboxAction::ReplyAll)
                    {
                        if let Some(row) = snapshot.selected_message {
                            this.open_reply_all(row.message.clone(), window, cx);
                        }
                    }
                }))
                .on_action(cx.listener(|this, _: &FindInMessage, window, cx| {
                    this.focus_reader_find(window, cx)
                }))
                .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                    if this
                        .action_state(cx)
                        .allows(desktop_actions::MailboxAction::FocusSearch)
                    {
                        this.search.read(cx).focus_handle(cx).focus(window, cx);
                    }
                }))
                .on_action(cx.listener(|this, _: &RefreshMail, _, cx| {
                    if this
                        .action_state(cx)
                        .allows(desktop_actions::MailboxAction::RefreshMail)
                    {
                        this.live.update(cx, |live, cx| live.refresh(cx));
                    }
                }))
                .on_action(cx.listener(|this, _: &FocusMessages, window, cx| {
                    if this.subscriptions.take().is_some() {
                        cx.notify();
                        return;
                    }
                    if this.view == View::Mailbox && (this.conversation_open || this.comfort.outbox_open) {
                        this.conversation_open = false;
                        this.comfort.outbox_open = false;
                        this.message_focus.focus(window, cx);
                        cx.notify();
                        return;
                    }
                    if this.view == View::Appearance {
                        this.view = this.appearance_return_view;
                        this.message_focus.focus(window, cx);
                        cx.notify();
                        return;
                    }
                    desktop_actions::focus_pane(
                        desktop_actions::MailboxAction::FocusMessages,
                        this.action_state(cx),
                        if this.list_rows.is_empty() {
                            &this.mailbox_focus
                        } else {
                            &this.message_focus
                        },
                        window,
                        cx,
                    );
                }))
                .on_action(cx.listener(|this, _: &FocusReader, window, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    if !this.conversation_open && snapshot.selected_message.is_some() {
                        this.conversation_open = true;
                        this.message_focus.focus(window, cx);
                        cx.notify();
                        return;
                    }
                    if let Some(identity) = this.reader_text_identity(&snapshot) {
                        if let Some(target) = this
                            .reader_text
                            .as_ref()
                            .filter(|target| target.matches(&identity))
                        {
                            let handle = target.state.read(cx).focus_handle().clone();
                            desktop_actions::focus_pane(
                                desktop_actions::MailboxAction::FocusReader,
                                this.action_state(cx),
                                &handle,
                                window,
                                cx,
                            );
                        }
                    }
                }))
                .on_action(cx.listener(|this, _: &AllSent, window, cx| {
                    if this.view == View::Mailbox {
                        this.mailbox_focus.focus(window, cx);
                        this.comfort.outbox_open = false;
                        this.comfort.filter = None;
                        this.live.update(cx, |live, cx| {
                            live.select_unified_kind(Some(FolderKind::Sent), cx)
                        });
                        cx.notify();
                    }
                }))
                .on_action(cx.listener(|this, _: &AllDrafts, window, cx| {
                    if this.view == View::Mailbox {
                        this.mailbox_focus.focus(window, cx);
                        this.comfort.outbox_open = false;
                        this.comfort.filter = None;
                        this.live.update(cx, |live, cx| {
                            live.select_unified_kind(Some(FolderKind::Drafts), cx)
                        });
                        cx.notify();
                    }
                }))
                .on_action(cx.listener(|this, _: &SortSender, window, cx| {
                    if this.view == View::Mailbox {
                        this.mailbox_focus.focus(window, cx);
                        this.comfort.sort = comfort::Sort::Sender;
                        this.apply_mail_sort(cx);
                        this.rebuild_list_rows(&this.live.read(cx).snapshot(), cx);
                        cx.notify();
                    }
                }))
                .on_action(cx.listener(|this, _: &ArchiveMessage, _, cx| {
                    this.triage_selected(FolderKind::Archive, cx)
                }))
                .on_action(cx.listener(|this, _: &TrashMessage, _, cx| {
                    this.triage_selected(FolderKind::Trash, cx)
                }))
                .on_action(cx.listener(|this, _: &ToggleStar, _, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    if let Some(row) = snapshot.selected_message {
                        let starred = row.message.starred;
                        this.live
                            .update(cx, |live, cx| live.set_selected_starred(!starred, cx));
                    }
                }))
                .on_action(cx.listener(|this, _: &ToggleUnread, _, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    if let Some(row) = snapshot.selected_message {
                        let unread = row.message.unread;
                        this.live
                            .update(cx, |live, cx| live.set_selected_unread(!unread, cx));
                    }
                }))
                .on_action(cx.listener(|this, _: &ForwardMessage, window, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    if this
                        .action_state(cx)
                        .allows(desktop_actions::MailboxAction::ReplyMessage)
                    {
                        if let Some(row) = snapshot.selected_message {
                            this.open_forward(row.message.clone(), window, cx);
                        }
                    }
                }))
                .on_action(cx.listener(|this, _: &GoInbox, window, cx| {
                    this.go_to_kind(FolderKind::Inbox, window, cx)
                }))
                .on_action(cx.listener(|this, _: &GoStarred, window, cx| {
                    this.go_to_kind(FolderKind::Starred, window, cx)
                }))
                .on_action(cx.listener(|this, _: &GoSent, window, cx| {
                    this.go_to_kind(FolderKind::Sent, window, cx)
                }))
                .on_action(cx.listener(|this, _: &GoDrafts, window, cx| {
                    this.go_to_kind(FolderKind::Drafts, window, cx)
                }))
                .on_action(cx.listener(|this, _: &GoArchive, window, cx| {
                    this.go_to_kind(FolderKind::Archive, window, cx)
                }))
                .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
                    this.open_palette(window, cx)
                }))
                .on_action(cx.listener(|this, _: &ShowShortcuts, window, cx| {
                    this.open_palette(window, cx)
                }))
                .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                    this.sidebar_hidden = !this.sidebar_hidden;
                    this.sidebar_collapsed = false;
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.open_appearance(cx)))
                .on_action(cx.listener(|this, _: &ManageSubscriptions, _, cx| this.open_subscriptions(cx)))
                .on_action(cx.listener(|this, _: &ToggleReaderMode, _, cx| {
                    let snapshot = this.live.read(cx).snapshot();
                    let has_original = snapshot
                        .selected_original
                        .as_ref()
                        .is_some_and(|o| o.source.is_some());
                    let next = match this.reader_mode(&snapshot) {
                        ReaderMode::Original => ReaderMode::Clean,
                        ReaderMode::Clean => ReaderMode::Text,
                        ReaderMode::Text if has_original => ReaderMode::Original,
                        ReaderMode::Text => ReaderMode::Clean,
                    };
                    this.set_reader_mode(next, cx);
                }))
                .on_action(cx.listener(|this, _: &NextSplit, _, cx| {
                    let next = this.split.next(1);
                    this.set_split(next, cx)
                }))
                .on_action(cx.listener(|this, _: &PreviousSplit, _, cx| {
                    let next = this.split.next(-1);
                    this.set_split(next, cx)
                }))
                .on_action(cx.listener(|this, _: &ScreenerAllow, _, cx| this.screen_sender(true, cx)))
                .on_action(cx.listener(|this, _: &ScreenerBlock, _, cx| this.screen_sender(false, cx)))
                .on_action(cx.listener(|this, _: &Unsubscribe, window, cx| this.unsubscribe_selected(window, cx)))
                .on_action(cx.listener(|this, _: &UndoSend, window, cx| this.undo_last_send(window, cx)))
                .on_action(cx.listener(|this, _: &CycleLayout, _, cx| {
                    let next = this.preferences.layout.next();
                    this.change_preference(PreferenceChange::Layout(next), cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleSenderColors, _, cx| {
                    let current = this.original.as_ref().is_some_and(|s| s.original_colors);
                    this.set_original_policy(None, Some(!current), cx)
                }))
                .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                    this.toggle_theme(window, cx)
                }))
                .on_action(cx.listener(|this, _: &UnifiedInbox, window, cx| {
                    if this
                        .action_state(cx)
                        .allows(desktop_actions::MailboxAction::UnifiedInbox)
                    {
                        this.mailbox_focus.focus(window, cx);
                        this.comfort.outbox_open = false;
                        this.comfort.filter = None;
                        this.live.update(cx, |live, cx| live.select_unified(cx));
                        this.view = View::Mailbox;
                    }
                }))
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
                .child(body)
                .children(undo_toast)
                .children(palette_overlay),
        )
    }
}

impl Drop for MailApp {
    fn drop(&mut self) {
        self.auth_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Ok(slot) = self.auth_cancel.try_lock() {
            if let Some(attempt) = slot.as_ref() {
                attempt.cancel();
            }
        }
        if let Some(composer) = &mut self.composer {
            if let Some(lease) = composer.recovery.lease.take() {
                if let Some(service) = &self.recovery {
                    let _ = service.release(lease);
                } else {
                    std::mem::forget(lease);
                }
            }
        }
        // Pending request leases retain staging; no fabricated worker retirement
        // or shutdown recovery acknowledgment. Session's Drop cancels its host.
    }
}

/// "Create an app password" card with the provider's page and steps.
fn app_password_card(guide: app_passwords::Guide, palette: Palette, cx: &mut Context<MailApp>) -> AnyElement {
    let url = guide.url;
    v_flex()
        .w_full()
        .gap_2()
        .p_3()
        .rounded(px(8.))
        .border_1()
        .border_color(palette.border)
        .bg(palette.surface)
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(if guide.blocked.is_some() {
                    format!("{} needs browser sign-in", guide.provider)
                } else {
                    format!("{} needs an app password", guide.provider)
                }),
        )
        .when_some(guide.blocked, |el, note| {
            el.child(div().text_size(px(11.)).line_height(relative(1.4)).text_color(palette.muted).child(note))
        })
        .children(guide.steps.iter().enumerate().map(|(index, step)| {
            h_flex()
                .gap_2()
                .items_start()
                .text_size(px(11.))
                .line_height(relative(1.4))
                .child(div().text_color(palette.faint).child(format!("{}.", index + 1)))
                .child(div().flex_1().text_color(palette.muted).child(*step))
        }))
        .when(guide.blocked.is_none(), |el| {
            el.child(
                Button::new("open-app-password-page")
                    .primary()
                    .small()
                    .self_start()
                    .label(format!("Create {} app password", guide.provider))
                    .tooltip(url)
                    .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url))),
            )
        })
        .into_any_element()
}

fn trust_colors(trust: megamail_core::models::SenderTrust) -> Option<(gpui_kit::Hsla, &'static str, &'static str)> {
    use megamail_core::models::SenderTrust;
    match trust {
        SenderTrust::Pass => Some((gpui_kit::hsla(145. / 360., 0.55, 0.42, 1.), "Verified", "The sender's domain was authenticated by your provider (DMARC / DKIM / SPF)")),
        SenderTrust::Suspicious => Some((gpui_kit::hsla(38. / 360., 0.9, 0.5, 1.), "Check sender", "Authenticated, but the addressing looks off")),
        SenderTrust::Fail => Some((gpui_kit::hsla(0., 0.72, 0.52, 1.), "Unverified sender", "Authentication failed: the From address is likely forged")),
        SenderTrust::Unverified => None,
    }
}

/// Small badge beside the sender address.
fn trust_badge(check: &megamail_core::models::SenderCheck, palette: Palette) -> Option<AnyElement> {
    let (color, label, tooltip) = trust_colors(check.trust)?;
    let _ = palette;
    Some(
        h_flex()
            .id("sender-trust-badge")
            .flex_shrink_0()
            .gap_1()
            .items_center()
            .px_1p5()
            .py_0p5()
            .rounded(px(4.))
            .bg(color.opacity(0.14))
            .text_color(color)
            .text_size(px(10.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(Icon::new(match check.trust {
                megamail_core::models::SenderTrust::Pass => IconName::Check,
                _ => IconName::TriangleAlert,
            }).size(px(11.)))
            .child(label)
            .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tooltip).build(window, cx))
            .into_any_element(),
    )
}

/// A plain-English warning above the body for forged or suspicious mail.
fn trust_warning(check: &megamail_core::models::SenderCheck, palette: Palette) -> Option<AnyElement> {
    use megamail_core::models::SenderTrust;
    if !matches!(check.trust, SenderTrust::Fail | SenderTrust::Suspicious) {
        return None;
    }
    let (color, ..) = trust_colors(check.trust)?;
    // One line, details on hover: a warning, not a wall of text.
    let details = std::iter::once(check.summary.clone())
        .chain(check.findings.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n");
    Some(
        h_flex()
            .id("trust-warning")
            .w_full()
            .gap_2()
            .items_center()
            .px_3()
            .py(px(6.))
            .rounded(px(6.))
            .bg(color.opacity(0.10))
            .text_size(px(12.))
            .text_color(color)
            .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(details.clone()).build(window, cx))
            .child(Icon::new(IconName::TriangleAlert).size(px(14.)))
            .child(div().font_weight(FontWeight::MEDIUM).child(if check.trust == SenderTrust::Fail {
                "Couldn't verify this sender."
            } else {
                "Something about this sender looks off."
            }))
            .child(div().text_color(palette.muted).child(if check.trust == SenderTrust::Fail {
                "Be careful with links, attachments and payment requests."
            } else {
                "Hover for details."
            }))
            .into_any_element(),
    )
}

fn quoted_html(quoted: &str) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut lines = quoted.lines();
    let attribution = lines.next().unwrap_or_default();
    let body = lines
        .map(|line| escape(line.strip_prefix("> ").or_else(|| line.strip_prefix('>')).unwrap_or(line)))
        .collect::<Vec<_>>()
        .join("<br>");
    format!(
        "<br><div>{}</div><blockquote style=\"margin:0 0 0 .8ex;border-left:1px solid #ccc;padding-left:1ex\">{}</blockquote>",
        escape(attribution),
        body
    )
}

fn shortcut_keys(keys: &'static str, palette: Palette) -> impl IntoElement {
    h_flex().gap_1().children(keys.split(' ').filter(|k| !k.is_empty()).map(move |key| {
        div()
            .min_w(px(18.))
            .h(px(18.))
            .px_1()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .border_1()
            .border_color(palette.border)
            .text_size(px(10.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.muted)
            .child(key)
    }))
}

fn message_row(
    message: &Message,
    two_line: bool,
    palette: Palette,
    // None: avatars off. Some(None): no picture, show initials.
    avatar: Option<Option<Arc<gpui_kit::RenderImage>>>,
    unread: bool,
    account_label: Option<String>,
) -> impl IntoElement {
    use gpui_kit::StyledImage as _;
    let lines = if two_line {
        message_lines_two(message, palette, unread, account_label).into_any_element()
    } else {
        message_lines(message, palette, unread, account_label).into_any_element()
    };
    match avatar {
        None => lines.into_any_element(),
        Some(picture) => h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .items_center()
            .child(match picture {
                Some(image) => gpui_kit::img(image)
                    .size(px(24.))
                    .flex_shrink_0()
                    .rounded_full()
                    .object_fit(gpui_kit::ObjectFit::Cover)
                    .into_any_element(),
                None => div()
                    .flex()
                    .flex_shrink_0()
                    .size(px(24.))
                    .rounded_full()
                    .items_center()
                    .justify_center()
                    .bg(comfort::identity_color(&message.from_addr, palette).opacity(0.14))
                    .text_color(comfort::identity_color(&message.from_addr, palette))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(initials(&message_sender(message)))
                    .into_any_element(),
            })
            .child(div().flex_1().min_w_0().child(lines))
            .into_any_element(),
    }
}

fn message_lines(
    message: &Message,
    palette: Palette,
    unread: bool,
    account_label: Option<String>,
) -> impl IntoElement {
    // Superhuman row: one line. Sender in a fixed column, subject and a
    // muted preview in the middle, date on the right. Unread = dot + weight.
    let subject = if message.subject.trim().is_empty() {
        "(No subject)".to_owned()
    } else {
        message.subject.trim().to_owned()
    };
    let preview = message.preview.trim().to_owned();
    h_flex()
        .w_full()
        .min_w_0()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_shrink_0()
                .size(px(6.))
                .rounded_full()
                .when(unread, |dot| dot.bg(palette.accent)),
        )
        .child(
            div()
                .w(px(190.))
                .flex_shrink_0()
                .truncate()
                .text_size(px(13.))
                .font_weight(if unread { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .text_color(if unread { palette.text } else { palette.muted })
                .child(message_sender(message)),
        )
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .text_size(px(13.))
                .child(
                    div()
                        .flex_shrink_0()
                        .max_w(relative(0.65))
                        .truncate()
                        .font_weight(if unread { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                        .text_color(if unread { palette.text } else { palette.muted })
                        .child(subject),
                )
                .when(!preview.is_empty(), |line| {
                    line.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(palette.faint)
                            .child(preview),
                    )
                }),
        )
        .when_some(account_label, |row, label| {
            row.child(
                div()
                    .flex_shrink_0()
                    .size(px(7.))
                    .rounded_full()
                    .bg(comfort::identity_color(&label, palette).opacity(0.8)),
            )
        })
        .when(message.starred, |row| {
            row.child(Icon::new(IconName::StarFill).size(px(12.)).text_color(palette.accent))
        })
        .when(message.has_attachment, |row| {
            row.child(Icon::new(IconName::Paperclip).size(px(12.)).text_color(palette.faint))
        })
        .child(
            div()
                .w(px(76.))
                .flex_shrink_0()
                .flex()
                .justify_end()
                .text_size(px(12.))
                .text_color(if unread { palette.text } else { palette.faint })
                .child(short_date(&message.date)),
        )
}

/// Narrow list (side-by-side layout): sender and date on top, subject and
/// preview underneath.
fn message_lines_two(
    message: &Message,
    palette: Palette,
    unread: bool,
    account_label: Option<String>,
) -> impl IntoElement {
    let subject = if message.subject.trim().is_empty() {
        "(No subject)".to_owned()
    } else {
        message.subject.trim().to_owned()
    };
    let preview = message.preview.trim().to_owned();
    v_flex()
        .w_full()
        .min_w_0()
        .gap(px(3.))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(div().flex_shrink_0().size(px(6.)).rounded_full().when(unread, |dot| dot.bg(palette.accent)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(if unread { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                        .text_color(if unread { palette.text } else { palette.muted })
                        .child(message_sender(message)),
                )
                .when_some(account_label, |row, label| {
                    row.child(div().flex_shrink_0().size(px(7.)).rounded_full().bg(comfort::identity_color(&label, palette).opacity(0.8)))
                })
                .when(message.starred, |row| row.child(Icon::new(IconName::StarFill).size(px(11.)).text_color(palette.accent)))
                .when(message.has_attachment, |row| row.child(Icon::new(IconName::Paperclip).size(px(11.)).text_color(palette.faint)))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(11.))
                        .text_color(if unread { palette.text } else { palette.faint })
                        .child(short_date(&message.date)),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .pl(px(14.))
                .gap_1()
                .text_size(px(12.))
                .child(
                    div()
                        .flex_shrink_0()
                        .max_w(relative(0.7))
                        .truncate()
                        .font_weight(if unread { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                        .text_color(if unread { palette.text } else { palette.muted })
                        .child(subject),
                )
                .when(!preview.is_empty(), |line| {
                    line.child(div().flex_1().min_w_0().truncate().text_color(palette.faint).child(preview))
                }),
        )
}

/// "Today, 11:26 AM" → "11:26 AM"; "Yesterday" etc. stay.
fn short_date(date: &str) -> String {
    date.strip_prefix("Today, ").unwrap_or(date).to_owned()
}

fn candidate_auth_label(
    incoming: discovery::AuthMethod,
    outgoing: discovery::AuthMethod,
) -> String {
    let label = |auth| match auth {
        discovery::AuthMethod::Password => "Password",
        discovery::AuthMethod::OAuth2 => "OAuth",
        discovery::AuthMethod::None => "None",
        discovery::AuthMethod::Unsupported => "Unsupported",
    };
    format!(
        "Incoming: {} · Outgoing: {}",
        label(incoming),
        label(outgoing)
    )
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
                let auth = candidate_auth_label(candidate.incoming.auth, candidate.outgoing.auth);
                let email = email.clone();
                Button::new(SharedString::from(format!("server-candidate-{index}")))
                    .ghost()
                    .small()
                    .selected(index == selected)
                    .w_full()
                    .px_2()
                    .accessibility_label(format!("Use {name} settings from {source}. {auth}"))
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
                                v_flex()
                                    .min_w_0()
                                    .flex_1()
                                    .items_start()
                                    .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(name))
                                    .child(div().text_size(px(10.)).text_color(cx.theme().muted_foreground).child(auth)),
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
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .items_start()
        .gap_2()
        .py_3()
        .border_b_1()
        .border_color(palette.border)
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(px(13.))
                .text_color(palette.accent),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(heading),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
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

/// "Alice Smith <a@x>, bob@y.com" → "Alice Smith, bob"; empty → "(no recipients)".
fn recipient_names(to: &str) -> String {
    let names: Vec<String> = to
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let name = part.split('<').next().unwrap_or("").trim().trim_matches('"').trim();
            if !name.is_empty() && !name.contains('@') {
                name.to_owned()
            } else {
                let address = part.trim_matches(|c| c == '<' || c == '>').trim();
                let address = address.rsplit('<').next().unwrap_or(address).trim_end_matches('>');
                address.split('@').next().unwrap_or(address).to_owned()
            }
        })
        .collect();
    match names.len() {
        0 => "(no recipients)".to_owned(),
        1..=2 => names.join(", "),
        n => format!("{}, {} +{}", names[0], names[1], n - 2),
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
    #[test]
    fn saved_storage_recovery_uses_mailbox_not_import_setup() {
        assert!(matches!(
            super::startup_view(super::account_session::StartupRoute::StorageRecovery),
            super::View::Mailbox
        ));
        assert!(matches!(
            super::startup_view(super::account_session::StartupRoute::Mailbox),
            super::View::Mailbox
        ));
        assert!(matches!(
            super::startup_view(super::account_session::StartupRoute::FirstRun),
            super::View::Setup
        ));
    }

    #[test]
    fn candidate_picker_distinguishes_explicit_auth_choices() {
        use megamail_core::discovery::AuthMethod::{None, OAuth2, Password, Unsupported};
        let labels = [
            super::candidate_auth_label(OAuth2, OAuth2),
            super::candidate_auth_label(OAuth2, Password),
            super::candidate_auth_label(Password, OAuth2),
            super::candidate_auth_label(Password, Password),
        ];
        assert_eq!(
            labels
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
        assert_eq!(labels[1], "Incoming: OAuth · Outgoing: Password");
        assert_eq!(
            super::candidate_auth_label(Password, None),
            "Incoming: Password · Outgoing: None"
        );
        assert_eq!(
            super::candidate_auth_label(Unsupported, Password),
            "Incoming: Unsupported · Outgoing: Password"
        );
    }

    #[test]
    fn opening_a_thread_starts_at_the_subject_and_only_member_navigation_follows() {
        let key = |uid| super::MessageKey {
            account_id: 1,
            folder_path: "INBOX".into(),
            uid,
        };
        let first = key(1);
        let reply = key(2);
        let unrelated = key(3);
        let members = [&first, &reply];
        assert!(!super::reader_follows_selection(
            None,
            Some(&reply),
            members.into_iter()
        ));
        assert!(!super::reader_follows_selection(
            Some(&unrelated),
            Some(&reply),
            members.into_iter()
        ));
        assert!(super::reader_follows_selection(
            Some(&first),
            Some(&reply),
            members.into_iter()
        ));
        assert!(!super::reader_follows_selection(
            Some(&reply),
            None,
            members.into_iter()
        ));
    }

    #[test]
    fn managed_reader_markup_preserves_exact_semantic_plain_and_quote_variants() {
        let cached = super::ReaderBody::new(
            super::MessageKey {
                account_id: 7,
                folder_path: "INBOX".into(),
                uid: 42,
            },
            "Current <literal>\n\n> Earlier",
            "<p><b>Current &lt;literal&gt;</b></p><blockquote>Earlier</blockquote>".into(),
            Some("<p><b>Current &lt;literal&gt;</b></p>".into()),
        );
        assert_eq!(cached.markup(false, true), cached.html);
        assert_eq!(cached.markup(false, false), cached.html_without_quote);
        assert_eq!(cached.markup(true, true), cached.plain_html);
        assert_eq!(cached.markup(true, false), cached.plain_html_without_quote);
        assert!(cached.markup(false, true).contains("<b>"));
        assert!(!cached.markup(true, true).contains("<b>"));
        assert!(cached.markup(true, true).contains("&lt;literal&gt;"));
        assert!(!cached.markup(false, false).contains("Earlier"));
    }

    #[test]
    fn reader_cache_keeps_formatted_quotes_and_counts_shared_markup_once() {
        let key = super::MessageKey {
            account_id: 1,
            folder_path: "INBOX".into(),
            uid: 42,
        };
        let rich = super::ReaderBody::new(
            key.clone(),
            "Current\n\n> Earlier",
            "<p><b>Current</b></p><blockquote>Earlier</blockquote>".into(),
            Some("<p><b>Current</b></p>".into()),
        );
        assert!(rich.has_quote);
        assert!(rich.html_without_quote.contains("<b>Current</b>"));
        assert!(!rich.html_without_quote.contains("Earlier"));
        assert!(rich.html.contains("Earlier"));
        assert!(rich.plain_html.contains("&gt; Earlier"));
        let plain = super::ReaderBody::new(key, "&".repeat(100).as_str(), "".into(), None);
        assert!(!plain.has_quote);
        assert_eq!(
            plain.retained_bytes,
            plain.text.len() + plain.plain_html.len()
        );
        assert_eq!(plain.html, plain.plain_html);
        assert_eq!(plain.html_without_quote, plain.plain_html);
    }

    use super::{composer_can_close, composer_can_edit, composer_operation_pending};

    #[test]
    fn actual_reader_cache_retirement_removes_only_the_changed_mailbox_scope() {
        use std::collections::{HashMap, VecDeque};
        let make = |account_id, folder_path: &str| {
            super::ReaderBody::new(
                super::MessageKey {
                    account_id,
                    folder_path: folder_path.into(),
                    uid: 7,
                },
                "synthetic cached text",
                "".into(),
                None,
            )
        };
        let mut cache = VecDeque::from([make(7, "INBOX"), make(7, "Archive"), make(8, "INBOX")]);
        let old = HashMap::from([(
            (7, "INBOX".into()),
            super::live::NativeMailboxEpoch {
                uid_validity: 10,
                generation: 1,
            },
        )]);
        let new = HashMap::from([(
            (7, "INBOX".into()),
            super::live::NativeMailboxEpoch {
                uid_validity: 11,
                generation: 2,
            },
        )]);
        let retired = super::changed_mailbox_scopes(&old, &new);
        super::retire_reader_body_cache(&mut cache, &retired);
        assert_eq!(cache.len(), 2);
        assert!(
            cache
                .iter()
                .all(|body| body.key.account_id != 7 || body.key.folder_path != "INBOX")
        );
    }

    #[test]
    fn direct_uncertain_worker_completion_locks_compose_send_save_and_close() {
        let mut live = super::LiveMailbox::synthetic_empty();
        live.synthetic_pending_send(4, 81);
        let compact = super::live::compact_worker_event(
            megamail_core::worker::WorkerEvent::SendUnknown {
                request_id: 81,
                reason: "opaque acknowledgement failure, not classified by text".into(),
            },
            false,
        )
        .unwrap();
        // Reject a different owner before consuming the matching pending send.
        assert!(live.record_send_completion(5, &compact).is_none());
        live.record_send_completion(4, &compact).unwrap();
        // No current account is selected: the production composer resolves its
        // owner/request result independently of snapshot's rail-account result.
        let result = live.send_result_for(4, 81).unwrap();
        assert!(live.send_result_for(5, 81).is_none());
        assert!(live.send_result_for(4, 82).is_none());
        assert!(result.uncertain);
        assert!(composer_operation_pending(
            false,
            false,
            false,
            result.uncertain
        ));
        assert!(!composer_can_edit(
            false,
            false,
            false,
            false,
            result.uncertain
        ));
        assert!(!composer_can_close(false, false, false, result.uncertain));
        assert!(live.record_send_completion(4, &compact).is_none());
    }

    #[test]
    fn direct_failed_completion_remains_editable_and_recoverable() {
        let mut live = super::LiveMailbox::synthetic_empty();
        live.synthetic_pending_send(4, 82);
        let compact = super::live::compact_worker_event(
            megamail_core::worker::WorkerEvent::SendFinished {
                request_id: 82,
                outcome: megamail_core::worker::SendOutcome::Failed,
            },
            false,
        )
        .unwrap();
        let result = live.record_send_completion(4, &compact).unwrap();
        assert!(!result.uncertain);
        assert!(composer_can_edit(
            false,
            false,
            false,
            false,
            result.uncertain
        ));
        assert!(composer_can_close(false, false, false, result.uncertain));
    }

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
    if std::env::args_os().skip(1).collect::<Vec<_>>() == [std::ffi::OsString::from("--version")] {
        println!(
            "MegaMail {} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("MEGAMAIL_SOURCE_REVISION").unwrap_or("source revision unavailable")
        );
        return;
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,megamail=info,megamail_core=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let mut arguments = std::env::args_os().skip(1).peekable();
    if arguments.peek().map(|arg| arg.as_os_str()) == Some(std::ffi::OsStr::new("mcp")) {
        arguments.next();
        let read_only = arguments.any(|arg| arg == "--read-only");
        std::process::exit(mcp::run(read_only));
    }
    if arguments.peek().map(|arg| arg.as_os_str())
        == Some(std::ffi::OsStr::new("--thunderbird-host"))
    {
        arguments.next();
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
    let startup = match feature_integration::startup(arguments.collect()) {
        Ok(startup) => startup,
        Err(error) => {
            eprintln!("Could not start MegaMail: {error}");
            std::process::exit(2);
        }
    };
    let demo = startup.demo;
    // Retained outside the app closure through setup, workers and event-loop
    // shutdown. Forwarded/error returns before GPUI or any worker is created.
    let activation = if demo {
        None
    } else {
        let Some(config_dir) = config::config_base() else {
            eprintln!("Could not create MegaMail's private configuration directory.");
            std::process::exit(1);
        };
        match instance::start_activation(&config_dir, startup.uri.as_deref()) {
            Ok(instance::ActivationStartup::Primary(inbox)) => Some(Arc::new(inbox)),
            Ok(instance::ActivationStartup::Forwarded) => return,
            Err(error) => {
                eprintln!(
                    "Could not start MegaMail: another instance may be using its mail account data ({error})."
                );
                std::process::exit(1);
            }
        }
    };
    let activation_for_app = activation.clone();
    gpui_kit::application()
        .with_assets(MailIcons)
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
                cx.new(|cx| MailApp::new(window, cx, demo, activation_for_app.clone()))
            })
            .expect("Failed to open MegaMail window");
            cx.activate(true);
        });
    // Workers have no joined retirement acknowledgment. Keep the primary lock
    // until process teardown, rather than unlock while an admitted send may
    // still consume attachments/Outbox. The next start verifies stale sockets.
    // This is not graceful worker/socket shutdown or a recovery flush receipt.
    std::mem::forget(activation);
}
