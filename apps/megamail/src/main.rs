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
mod appearance_view;
mod attachment_preview;
mod comfort;
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
mod theme;
mod thunderbird_adapter;
mod triage;
mod triage_ui;
mod workflow;
mod zeron_background;
mod zeron_style;

use action::{
    AllDrafts, AllSent, ComposeMessage, FindInMessage, FocusMessages, FocusReader, FocusSearch,
    NextMessage, PreviousMessage, RefreshMail, ReplyAll, ReplyMessage, SortSender, UnifiedInbox,
};

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
        ArrowLeft,
        ArrowRight,
        Check,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        ChevronUp,
        Close,
        Copy,
        CornerUpLeft,
        CornerUpRight,
        Download,
        FileText,
        Folder,
        FolderOpen,
        Image,
        Inbox,
        Info,
        ListFilter,
        LoaderCircle,
        Mail,
        Moon,
        Palette,
        PanelLeftClose,
        PanelLeftOpen,
        Paperclip,
        PenLine,
        Plus,
        RefreshCw,
        Search,
        Send,
        Settings2,
        Star,
        StarFill,
        Sun,
        Trash,
        TriangleAlert,
        X
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
    _input_subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct SenderChoice {
    alias: Option<String>,
    label: String,
}

impl Composer {
    fn new(message: OutgoingMessage, window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        let mut composer = Self {
            markup: Default::default(),
            to: new_input_value(window, cx, "recipient@example.com", message.to.clone()),
            cc: new_input_value(window, cx, "Copy to", message.cc.clone()),
            bcc: new_input_value(window, cx, "Blind copy to", message.bcc.clone()),
            subject: new_input_value(window, cx, "Subject", message.subject.clone()),
            body: cx.new(|cx| TextareaState::new(window, cx).default_value(message.body.clone())),
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
            _input_subscriptions: Vec::new(),
        };
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

struct MailApp {
    live: Entity<LiveMailbox>,
    sender_photos: sender_photos::State,
    mail_panes: Entity<gpui_kit::component::resizable::ResizableState>,
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
                this.mailbox_focus.focus(window, cx);
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
            compact_mail_panes: cx.new(|_| Default::default()),
            sender_photos: Default::default(),
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
                    )
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.appearance = loaded.0;
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

    fn pane_surface(&self, color: gpui_kit::Hsla, _bleed: f32) -> gpui_kit::Hsla {
        gpui_kit::Hsla { a: 1.0, ..color }
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
        self.composer = Some(Composer::new(
            compose::reply_for_account(
                account_id,
                &message,
                &self.own_addresses(account_id, cx),
                false,
            ),
            window,
            cx,
        ));
        self.view = View::Compose;
        cx.notify();
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
        self.composer = Some(Composer::new(
            compose::reply_for_account(
                account_id,
                &message,
                &self.own_addresses(account_id, cx),
                true,
            ),
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
            .w_full()
            .h(px(
                zeron_style::ACCOUNT_TRIGGER_HEIGHT + zeron_style::ACCOUNT_DETAIL_LINE_HEIGHT
            ))
            .rounded(px(4.))
            .px_2()
            .when(collapsed, |button| {
                button.w(px(42.)).px_0().tooltip(account.label.clone())
            })
            .accessibility_label(format!("Switch to {}", account.label))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.mailbox_focus.focus(window, cx);
                this.live
                    .update(cx, |live, cx| live.switch_account(account_id, cx));
                this.list_scroll = UniformListScrollHandle::new();
                this.comfort.outbox_open = false;
                this.comfort.filter = None;
                this.view = View::Mailbox;
                cx.notify();
            }))
            .when(collapsed, |button| {
                button.child(
                    div()
                        .flex()
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
                                .flex()
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
        width: gpui_kit::Pixels,
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
            .map(|el| {
                if width == px(0.) {
                    el.w_full()
                } else {
                    el.w(width)
                }
            })
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
                                    .icon(
                                        Icon::new(if self.accounts_expanded {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        })
                                        .size(px(15.)),
                                    )
                                    .label(
                                        account
                                            .map(|account| account.email.clone())
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
                                snapshot.scope == MailboxScope::Account
                                    && Some(account.id) == current,
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
                            .label(if self.composer.is_some() {
                                "Resume draft"
                            } else {
                                "New message"
                            })
                            .disabled(
                                !self
                                    .action_state(cx)
                                    .allows(desktop_actions::MailboxAction::ComposeMessage),
                            )
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
                    .child(self.recovery_panel(palette, cx))
                    .child(self.unified_button(snapshot, palette, dark, false, cx))
                    .child(self.shared_folders(snapshot, palette, cx))
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
                            .label("Settings")
                            .selected(self.view == View::Appearance)
                            .on_click(cx.listener(|this, _, _, cx| this.open_appearance(cx))),
                    ),
            )
            .into_any_element()
    }

    fn unified_button(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        dark: bool,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = snapshot.scope == MailboxScope::Unified
            && snapshot.unified_kind == Some(FolderKind::Inbox);
        Button::new(if collapsed {
            "unified-compact"
        } else {
            "unified-inbox"
        })
        .custom(sidebar_button_variant(cx, palette, dark, selected))
        .small()
        .selected(selected)
        .w_full()
        .h(px(32.))
        .rounded(px(4.))
        .px_2()
        .icon(Icon::new(IconName::Inbox).size(px(14.)))
        .when(!collapsed, |el| el.label("All inboxes"))
        .tooltip("Inbox mail from every account")
        .accessibility_label("All inboxes from every account")
        .on_click(cx.listener(|this, _, window, cx| {
            this.mailbox_focus.focus(window, cx);
            this.comfort.outbox_open = false;
            this.comfort.filter = None;
            this.live.update(cx, |live, cx| live.select_unified(cx));
            this.list_scroll = UniformListScrollHandle::new();
            this.view = View::Mailbox;
            cx.notify();
        }))
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
                    .child(self.unified_button(snapshot, palette, dark, true, cx))
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
                            .tooltip("Settings")
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
        if self.list_rows.is_empty() {
            let recovery = snapshot.current_folder_path.is_none()
                && (snapshot.error.is_some()
                    || self.startup_error.is_some()
                    || snapshot.thunderbird_error.is_some());
            let message = if snapshot.thunderbird_loading {
                "Reconnecting saved accounts…"
            } else if recovery {
                "Saved accounts need attention"
            } else if snapshot.loading {
                "Loading messages…"
            } else if self.comfort.filter.is_some() {
                "No conversations in this work state"
            } else if snapshot.filter != MessageFilter::All {
                "No messages match this filter"
            } else if snapshot.search_query.is_empty() {
                "This folder is empty"
            } else {
                "No matches in this search scope"
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
                    if snapshot.thunderbird_loading {
                        "Your saved accounts are being reconnected. No import is needed."
                    } else if recovery {
                        "Use Retry above after resolving the reported problem. Account storage is kept."
                    } else if snapshot.filter != MessageFilter::All {
                        "Choose All or load more messages to broaden this view."
                    } else if snapshot.search_query.is_empty() {
                        if self.comfort.filter.is_some() { "Choose All work states or load more results to broaden this view." } else { "New mail will appear here when this account syncs." }
                    } else {
                        "Try another search or choose All local mail in Search scope."
                    },
                ))
                .into_any_element();
        }

        let selected = snapshot.selected_key.clone();
        let rows = self.list_rows.clone();
        let row_height = match self.preferences.density {
            Density::Compact => 64.,
            Density::Comfortable => 78.,
        };
        let expanded = self.expanded_conversations.clone();
        let labels = snapshot.account_labels.clone();
        let own_addresses: Vec<_> = snapshot
            .accounts
            .iter()
            .map(|a| a.email.to_lowercase())
            .chain(
                self.profiles
                    .iter()
                    .flat_map(|(_, p)| p.aliases.iter().map(|a| a.address().to_lowercase())),
            )
            .collect();
        let unified = snapshot.scope == MailboxScope::Unified;
        let owner = cx.entity().downgrade();
        let bulk_open = self.triage.open;
        let bulk_busy = self.triage.busy();
        let bulk_selected: HashSet<_> = self
            .triage
            .selection
            .keys()
            .map(|k| k.message.clone())
            .collect();
        let bulk_available = self.triage.available.clone();
        uniform_list("message-rows", rows.len(), move |range, _, _| {
            range
                .map(|index| {
                    let ListRow {
                        row,
                        conversation,
                        child,
                    } = rows[index].clone();
                    let mut display_message = if bulk_open {
                        row.message.clone()
                    } else {
                        conversation
                            .as_ref()
                            .and_then(|thread| thread.latest_sent.as_ref())
                            .filter(|sent| sent.message.timestamp > row.message.timestamp)
                            .map_or_else(
                                || row.message.clone(),
                                |sent| {
                                    let mut message = sent.message.clone();
                                    message.from_name = "You".to_owned();
                                    message
                                },
                            )
                    };
                    if own_addresses
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(&display_message.from_addr))
                    {
                        display_message.from_name = format!(
                            "You → {}",
                            if display_message.to.is_empty() {
                                "Recipients"
                            } else {
                                &display_message.to
                            }
                        );
                    }
                    if let Some(thread) = conversation.as_ref().filter(|_| !bulk_open) {
                        display_message.starred =
                            thread.members.iter().any(|member| member.message.starred);
                        display_message.has_attachment = thread
                            .members
                            .iter()
                            .any(|member| member.message.has_attachment);
                    }
                    let active = if let Some(thread) = &conversation {
                        !expanded.contains(&thread.key)
                            && thread
                                .members
                                .iter()
                                .any(|member| selected.as_ref() == Some(&member.key))
                    } else {
                        selected.as_ref() == Some(&row.key)
                    };
                    let owner_select = owner.clone();
                    let key = row.key.clone();
                    let unread = if bulk_open {
                        row.message.unread
                    } else {
                        conversation
                            .as_ref()
                            .map_or(row.message.unread, |thread| thread.unread > 0)
                    };
                    let mut element = h_flex()
                        .w_full()
                        .h(px(row_height))
                        .items_center()
                        .rounded(px(4.))
                        .when(active, |el| el.bg(palette.selected))
                        .when(child, |el| el.pl_4())
                        .child(
                            Button::new(SharedString::from(format!(
                                "message-{}-{}-{}-{}",
                                key.account_id, key.folder_path, key.uid, child
                            )))
                            .ghost()
                            .small()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .rounded(px(4.))
                            .px_3()
                            .tab_stop(true)
                            .accessibility_label(format!(
                                "{}: {}",
                                message_sender(&row.message),
                                row.message.subject
                            ))
                            .on_click(move |_, _, cx| {
                                let _ = owner_select.update(cx, |this, cx| {
                                    this.draft_open_error = None;
                                    this.live.update(cx, |live, cx| {
                                        live.select_message(key.clone(), cx)
                                    });
                                    this.view = View::Mailbox;
                                    cx.notify();
                                });
                            })
                            .child(message_row(
                                &display_message,
                                palette,
                                unread,
                                (unified && !child)
                                    .then(|| labels.get(&row.key.account_id).cloned())
                                    .flatten(),
                            )),
                        );
                    if bulk_open {
                        let bulk_key = row.key.clone();
                        let selected = bulk_selected.contains(&bulk_key);
                        let available = bulk_available.contains(&bulk_key);
                        debug_assert!(
                            !available
                                || (display_message.uid == bulk_key.uid
                                    && display_message.account_id == bulk_key.account_id)
                        );
                        let owner_bulk = owner.clone();
                        element = element.child(
                            Button::new(SharedString::from(format!(
                                "bulk-select-{}-{}-{}-{child}",
                                bulk_key.account_id, bulk_key.folder_path, bulk_key.uid
                            )))
                            .ghost()
                            .small()
                            .label(if selected { "Selected" } else { "Select" })
                            .selected(selected)
                            .disabled(!available || bulk_busy)
                            .accessibility_label(format!(
                                "{} message {}",
                                if selected { "Deselect" } else { "Select" },
                                row.message.subject
                            ))
                            .on_click(move |_, _, cx| {
                                let _ = owner_bulk.update(cx, |this, cx| {
                                    this.edit_triage_selection(
                                        Some(bulk_key.clone()),
                                        false,
                                        false,
                                        cx,
                                    )
                                });
                            }),
                        );
                    }
                    if let Some(thread) = conversation
                        .filter(|thread| thread.count > 1 || thread.replied || thread.forwarded)
                    {
                        let key = thread.key.clone();
                        let open = expanded.contains(&key);
                        let owner_expand = owner.clone();
                        element = element.child(
                            v_flex()
                                .items_center()
                                .gap_1()
                                .pr_2()
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "thread-toggle-{}-{}",
                                        key.account_id, key.anchor
                                    )))
                                    .ghost()
                                    .small()
                                    .h(px(24.))
                                    .px_1()
                                    .icon(Icon::new(IconName::Mail).size(px(12.)))
                                    .label(thread.count.to_string())
                                    .tooltip(if open {
                                        "Collapse replies"
                                    } else {
                                        "Expand conversation replies"
                                    })
                                    .accessibility_label(format!(
                                        "{} {} conversation messages",
                                        if open { "Collapse" } else { "Expand" },
                                        thread.count
                                    ))
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = owner_expand.update(cx, |this, cx| {
                                                this.toggle_conversation(key.clone(), cx)
                                            });
                                        },
                                    ),
                                )
                                .when(thread.replied, |el| {
                                    el.child(
                                        Icon::new(IconName::CornerUpLeft)
                                            .size(px(12.))
                                            .text_color(palette.muted),
                                    )
                                })
                                .when(thread.forwarded, |el| {
                                    el.child(
                                        Icon::new(IconName::CornerUpRight)
                                            .size(px(12.))
                                            .text_color(palette.muted),
                                    )
                                }),
                        );
                    }
                    element.into_any_element()
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
            .tooltip(format!("Mailbox options · {}", selected.label()))
            .accessibility_label("Mailbox filters and options")
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
                let bulk_owner = owner.clone();
                let rules_owner = owner.clone();
                let recovery_owner = owner.clone();
                menu.separator()
                    .item(
                        PopupMenuItem::element(|_, _| div().child("Select messages")).on_click(
                            move |_, _, cx| {
                                let _ = bulk_owner.update(cx, |this, cx| {
                                    this.triage.open = !this.triage.open;
                                    cx.notify();
                                });
                            },
                        ),
                    )
                    .item(
                        PopupMenuItem::element(|_, _| div().child("Mail rules…")).on_click(
                            move |_, _, cx| {
                                let _ =
                                    rules_owner.update(cx, |this, cx| this.load_rule_editor(cx));
                            },
                        ),
                    )
                    .item(
                        PopupMenuItem::element(|_, _| div().child("Recover local drafts…"))
                            .on_click(move |_, _, cx| {
                                let _ = recovery_owner
                                    .update(cx, |this, cx| this.list_local_recovery(cx));
                            }),
                    )
            })
            .into_any_element()
    }

    fn message_pane(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
        width: gpui_kit::Pixels,
    ) -> impl IntoElement {
        let title = snapshot.title.clone();
        let unavailable = snapshot.account_errors.len();
        v_flex()
            .map(|el| {
                if width == px(0.) {
                    el.w_full()
                } else {
                    el.w(width)
                }
            })
            .h_full()
            .min_w_0()
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
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        Button::new("compose-new")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Mail))
                            .disabled(
                                !self
                                    .action_state(cx)
                                    .allows(desktop_actions::MailboxAction::ComposeMessage),
                            )
                            .accessibility_label("Compose a new message")
                            .tooltip("Compose a new message")
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.open_new_message(window, cx)
                                }),
                            ),
                    ),
            )
            .when(
                snapshot.scope == MailboxScope::Unified && unavailable > 0,
                |el| {
                    el.child(
                        Button::new("unified-partial-error")
                            .ghost()
                            .small()
                            .w_full()
                            .h(px(28.))
                            .icon(Icon::new(IconName::Info).size(px(14.)))
                            .label(format!("{unavailable} accounts unavailable · Retry"))
                            .tooltip(
                                snapshot
                                    .account_errors
                                    .iter()
                                    .map(|(id, error)| {
                                        format!(
                                            "{}: {}",
                                            snapshot
                                                .account_labels
                                                .get(id)
                                                .cloned()
                                                .unwrap_or_else(|| id.to_string()),
                                            error
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            )
                            .disabled(
                                !self
                                    .action_state(cx)
                                    .allows(desktop_actions::MailboxAction::RefreshMail),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this
                                    .action_state(cx)
                                    .allows(desktop_actions::MailboxAction::RefreshMail)
                                {
                                    this.live.update(cx, |live, cx| live.refresh(cx));
                                }
                            })),
                    )
                },
            )
            .child(
                v_flex()
                    .id("mailbox-tools")
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .flex_shrink_0()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
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
                                    .aria_label("Search mail with field, date and status operators")
                                    .flex_1()
                                    .min_w_0(),
                            )
                            .child(self.filter_menu(snapshot, palette, cx)),
                    )
                    .child(self.mail_controls(snapshot, palette, cx))
                    .when_some(snapshot.search_error.clone(), |row, error| {
                        row.child(error_banner("Search", &error, cx, palette))
                    })
                    .when_some(self.startup_error.clone(), |row, error| {
                        row.child(error_banner("Saved account storage", &error, cx, palette))
                            .child(
                                Button::new("retry-account-storage")
                                    .ghost()
                                    .small()
                                    .label("Retry saved accounts")
                                    .disabled(self.loading_profiles || snapshot.thunderbird_loading)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.load_saved_accounts(cx)),
                                    ),
                            )
                    })
                    .when_some(snapshot.thunderbird_error.clone(), |row, error| {
                        row.child(error_banner("Thunderbird connection", &error, cx, palette))
                            .child(
                                Button::new("reconnect-saved-thunderbird")
                                    .ghost()
                                    .small()
                                    .label("Retry saved accounts")
                                    .disabled(snapshot.thunderbird_loading)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.load_saved_accounts(cx);
                                    })),
                            )
                    })
                    .when_some(snapshot.error.clone(), |row, error| {
                        row.child(error_banner(
                            "Mail connection needs attention",
                            &error,
                            cx,
                            palette,
                        ))
                        .when(snapshot.is_thunderbird_account, |row| {
                            let target = snapshot.current_account_id;
                            row.child(
                                Button::new("retry-selected-saved-account")
                                    .ghost()
                                    .small()
                                    .label("Retry this saved account")
                                    .disabled(snapshot.thunderbird_loading)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.live.update(cx, |live, cx| {
                                            live.retry_thunderbird(target, cx);
                                        });
                                    })),
                            )
                        })
                    })
                    .when_some(snapshot.action_error.clone(), |row, error| {
                        row.child(error_banner("Mail action failed", &error, cx, palette))
                    })
                    .when(self.triage.open, |el| {
                        el.child(self.triage_panel(palette, cx))
                    })
                    .child(self.rule_editor_panel(palette, cx)),
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
                        self.list_rows.iter().filter(|r| !r.child).count(),
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
            .id("native-outbox-panel")
            .max_h(px(360.))
            .overflow_y_scroll()
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
                            .child("Outbox"),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(palette.muted)
                            .child(format!("{} retained · all accounts", snapshot.outbox.len())),
                    ),
            )
            .children(snapshot.outbox.iter().map(|item| {
                let id = item.id;
                let owning_account = item.account_id;
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
                            .child(div().text_size(px(11.)).text_color(palette.muted).child(snapshot.accounts.iter().find(|a| a.id == owning_account).map_or_else(|| "Saved account".into(), |a| a.email.clone())),
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
                    .when_some(item.send_at.filter(|at| *at > megamail_core::datefmt::now()), |el, at| el
                        .child(div().text_size(px(11.)).child(format!("Scheduled {}", megamail_core::datefmt::date_time(at))))
                        .when_some(self.live.read(cx).schedule_session(owning_account).map(|session| (owning_account, session)), |el, (account, session)| {
                            let request = worker::QueuedCancelRequest { id, send_at: Some(at), session };
                            el.child(Button::new(SharedString::from(format!("cancel-schedule-{owning_account}-{id}"))).ghost().small()
                                .label(if self.queue_cancel_confirmation == Some((account, request)) { "Confirm cancellation" } else { "Cancel scheduled send" })
                                .disabled(item.phase != megamail_core::models::OutboxPhase::Ready || !snapshot.pending_queue_cancels.is_empty())
                                .tooltip("Cancel only if this message is still scheduled and transmission has not started.")
                                .on_click(cx.listener(move |this, _, _, cx| this.cancel_queued_item(account, request, cx))))
                        }))
                    .when_some(Some(owning_account), |el, account| el.child(
                        Button::new(SharedString::from(format!("review-outbox-{owning_account}-{id}")))
                            .secondary().small().label("Delivery details")
                            .accessibility_label(format!("Review delivery of {subject_for_accessibility}"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.outbox_review = Some((account, id));
                                cx.notify();
                            }))
                    ))
                    .when(item.send_at.is_none(), |el| el.when_some(self.live.read(cx).schedule_session(owning_account).map(|session| (owning_account, session)), |el, (account, session)| {
                        let request = worker::QueuedCancelRequest { id, send_at: None, session };
                        el.child(Button::new(SharedString::from(format!("discard-ready-outbox-{owning_account}-{id}"))).ghost().small().icon(Icon::new(IconName::Trash))
                            .label(if self.queue_cancel_confirmation == Some((account, request)) { "Confirm discard" } else { "Discard unsent copy" })
                            .disabled(item.phase != megamail_core::models::OutboxPhase::Ready || !snapshot.pending_queue_cancels.is_empty())
                            .tooltip("Remove only an unclaimed local Outbox copy. Held or already submitted mail requires manual review.")
                            .accessibility_label(format!("Discard unsubmitted copy of {subject_for_accessibility}"))
                            .on_click(cx.listener(move |this, _, _, cx| this.cancel_queued_item(account, request, cx))))
                    }))
            }))
            .child(self.outbox_review_panel(snapshot, palette, cx))
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
        let Some(key) = snapshot.selected_key.clone() else {
            return div().into_any_element();
        };
        let expanded = self.link_details.contains(&key);
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .pt_3()
            .child(
                Button::new("reader-link-details")
                    .ghost()
                    .small()
                    .self_start()
                    .icon(Icon::new(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    }))
                    .label(format!("Link details · {}", snapshot.links.len()))
                    .accessibility_label(if expanded {
                        "Hide link details"
                    } else {
                        "Show link details"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.link_details.remove(&key) {
                            this.link_details.insert(key.clone());
                        }
                        cx.notify();
                    })),
            )
            .when(expanded, |el| {
                el.children(snapshot.links.iter().enumerate().map(|(index, link)| {
                    let url = link.url.clone();
                    let label = if link.label.trim().is_empty() {
                        link.url.clone()
                    } else {
                        link.label.clone()
                    };
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .gap_1()
                        .child(
                            Button::new(SharedString::from(format!("message-link-{index}")))
                                .ghost()
                                .small()
                                .w_full()
                                .min_w_0()
                                .justify_start()
                                .child(div().w_full().min_w_0().truncate().child(label.clone()))
                                .accessibility_label(format!("Open {label} in your browser"))
                                .tooltip(format!("Open {} in your browser", link.url))
                                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .whitespace_normal()
                                .pl_2()
                                .text_size(px(10.))
                                .text_color(palette.faint)
                                .child(link.url.clone()),
                        )
                }))
            })
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
            .min_w_0()
            .gap_2()
            .pt_6()
            .border_t_1()
            .border_color(palette.border)
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
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
                        .min_w_0()
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
                        .child(attachment_preview::preview_button(
                            name.clone(),
                            content.clone(),
                            index,
                            self.attachment_preview.busy(),
                            cx,
                        ))
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
            .child(self.attachment_preview_panel(snapshot, palette, cx))
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

    fn message_actions(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let selected = snapshot.selected_message.clone();
        let demo = snapshot.is_demo;
        let plain = snapshot
            .selected_key
            .as_ref()
            .is_some_and(|key| self.plain_messages.contains(key));
        let body_loaded = snapshot.body_loaded;
        let export_busy = self.reader_export_busy;
        let rules_busy = self.rule_preview_busy;
        Button::new("message-actions")
            .custom(floating_button_variant(cx, palette, self.dark))
            .small()
            .icon(Icon::new(IconName::ChevronDown).size(px(14.)))
            .tooltip("More message actions")
            .accessibility_label("More message actions")
            .disabled(selected.is_none())
            .dropdown_menu(move |mut menu, _, _| {
                let Some(row) = &selected else {
                    return menu;
                };
                let unread = row.message.unread;
                let starred = row.message.starred;
                let body = row.message.body.clone();
                for (index, label) in [
                    if unread {
                        "Mark as read"
                    } else {
                        "Mark as unread"
                    },
                    if starred {
                        "Remove star"
                    } else {
                        "Star message"
                    },
                    "Move to Trash",
                    "Copy message text",
                    "Find in message",
                    if plain {
                        "Use native formatting"
                    } else {
                        "Read as text"
                    },
                    "Save message text…",
                    "Save for printing…",
                    "Preview mail rules",
                    "Open original formatting…",
                ]
                .into_iter()
                .enumerate()
                {
                    let owner = owner.clone();
                    let body = body.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| div().child(label))
                            .disabled(match index {
                                0..=2 => demo,
                                3 => body.is_empty(),
                                4..=5 => !body_loaded,
                                6..=7 => !body_loaded || export_busy,
                                8 => demo || !body_loaded || rules_busy,
                                _ => !body_loaded,
                            })
                            .on_click(move |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| match index {
                                    0 => {
                                        this.live.update(cx, |live, cx| {
                                            live.set_selected_unread(!unread, cx)
                                        });
                                    }
                                    1 => {
                                        this.live.update(cx, |live, cx| {
                                            live.set_selected_starred(!starred, cx)
                                        });
                                    }
                                    2 => {
                                        this.live.update(cx, |live, cx| live.trash_selected(cx));
                                    }
                                    3 => cx.write_to_clipboard(ClipboardItem::new_string(
                                        body.clone(),
                                    )),
                                    4 => this.focus_reader_find(window, cx),
                                    5 => {
                                        if let Some(key) =
                                            this.live.read(cx).snapshot().selected_key
                                        {
                                            if !this.plain_messages.remove(&key) {
                                                this.plain_messages.insert(key);
                                            }
                                            let snapshot = this.live.read(cx).snapshot();
                                            this.refresh_reader_text(&snapshot, cx);
                                            cx.notify();
                                        }
                                    }
                                    6 => this.save_readable_text(cx),
                                    7 => this.save_printable_html(cx),
                                    8 => this.preview_saved_rules(cx),
                                    _ => this.open_original_formatting(cx),
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    fn reader(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.comfort.outbox_open {
            return v_flex().h_full().flex_1().min_w_0().bg(palette.background).p_5().gap_3()
                .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child("Outbox · all accounts"))
                .child(div().text_size(px(13.)).text_color(palette.muted).child("Queued, scheduled and held messages. Server acceptance is separate from delivery; uncertain outcomes need review."))
                 .when(!snapshot.outbox.is_empty(), |el| el.child(self.outbox_panel(snapshot, palette, cx)))
                .when(snapshot.outbox.is_empty(), |el| el.child(div().child("No messages are queued or held.")))
                .child(Button::new("close-outbox-view").secondary().small().self_start().label("Back to messages").on_click(cx.listener(|this, _, _, cx| { this.comfort.outbox_open = false; cx.notify(); })))
                .into_any_element();
        }
        let Some(row) = snapshot.selected_message.clone() else {
            return v_flex()
                .h_full()
                .flex_1()
                .min_w_0()
                .bg(self.pane_surface(palette.background, 0.05))
                .when(!snapshot.outbox.is_empty(), |el| {
                    el.child(self.outbox_panel(snapshot, palette, cx))
                })
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
            .find(|folder| folder.path == row.key.folder_path);
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
        let reader_subject = snapshot
            .conversations
            .iter()
            .find(|thread| thread.members.iter().any(|member| member.key == row.key))
            .and_then(|thread| {
                thread
                    .members
                    .iter()
                    .find(|member| !member.message.subject.is_empty())
            })
            .map_or(row.message.subject.as_str(), |member| {
                member.message.subject.as_str()
            });
        let row_for_draft = row.clone();
        let reply_message = row.clone();
        let forward_message = row.clone();
        let reply_all_message = row.clone();
        v_flex()
            .h_full()
            .flex_1()
            .min_w_0()
            .bg(self.pane_surface(palette.background, 0.05))
            .child(
                h_flex()
                    .min_h(px(44.))
                    .py_2()
                    .flex_wrap()
                    .gap_2()
                    .min_w_0()
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
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
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
                            .child(if is_draft { "Draft" } else { "Conversation" }),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .min_w_0()
                            .items_center()
                            .gap_1()
                            .when(!is_draft, |row| {
                                row.child(
                                    Button::new("reply-message")
                                        .secondary()
                                        .small()
                                        .icon(Icon::new(IconName::CornerUpLeft))
                                        .label("Reply")
                                        .disabled(!self.action_state(cx).allows(desktop_actions::MailboxAction::ReplyMessage))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_reply(
                                                reply_message.message.clone(),
                                                window,
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    Button::new("reply-all-message")
                                        .ghost()
                                        .small()
                                        .label("All")
                                        .disabled(!self.action_state(cx).allows(desktop_actions::MailboxAction::ReplyAll))
                                        .tooltip("Reply all · Ctrl+Shift+R")
                                        .accessibility_label("Reply to all recipients")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_reply_all(
                                                reply_all_message.message.clone(),
                                                window,
                                                cx,
                                            )
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
                                            this.open_forward(
                                                forward_message.message.clone(),
                                                window,
                                                cx,
                                            )
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
                                            this.open_draft(
                                                row_for_draft.as_ref().clone(),
                                                cx,
                                                window,
                                            )
                                        })),
                                )
                            })
                            .child(self.message_actions(snapshot, palette, cx))
                            .child(
                                Button::new("archive-message")
                                    .secondary()
                                    .small()
                                    .icon(Icon::new(archive_icon))
                                    .tooltip(archive_label)
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
             .when(!snapshot.outbox.is_empty(), |el| el.child(self.outbox_panel(snapshot, palette, cx)))
            .when_some(self.draft_open_error.clone(), |row, error| {
                row.child(error_banner("Could not open draft", &error, cx, palette))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .id("reader-scroll")
                    .track_scroll(&self.reader_scroll)
                    .overflow_y_scroll()
                    .min_w_0()
                    .overflow_x_hidden()
                    .px_6()
                    .pt_5()
                    .pb_6()
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .max_w(px(680.))
                            .gap_4()
                            .child(h_flex().gap_2()
                                .child(Button::new("open-original-formatting").ghost().small().label("Original formatting…")
                                    .disabled(!snapshot.body_loaded || snapshot.selected_original.as_ref().is_none_or(|body| body.completeness != reader_document::Completeness::Complete))
                                    .tooltip("Open the sender’s original layout in a separate window")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_original_formatting(cx))))
                                .when(self.reader_session.active.is_some(), |el| el.child(Button::new("close-original-formatting").ghost().small().label("Close original")
                                    .on_click(cx.listener(|this, _, _, cx| { this.retire_reader_session(cx); this.reader_session.notice = None; cx.notify(); })))))
                            .when_some(self.reader_session.notice.clone(), |el, notice| el.child(div().text_size(px(11.)).text_color(palette.muted).child(notice)))
                            .when_some(self.invitation_error.clone(), |el, error| el.child(div().text_size(px(11.)).text_color(palette.muted).child(error)))
                            .when_some(self.invitation_preview.clone(), |el, invitation| {
                                el.child(v_flex().gap_1().text_size(px(12.))
                                    .child(format!("Invitation: {}", invitation.summary))
                                    .child(format!("Starts: {} · {}", invitation.start.as_ref().map_or_else(|| "unknown".to_owned(), |stamp| format!("{} ({:?})", stamp.value, stamp.zone)), invitation.location))
                                    .child(match invitation.rsvp {
                                        invitation::RsvpCapability::NotRequested => "Read-only preview. No response was requested.",
                                        invitation::RsvpCapability::Cancelled => "Cancelled invitation. No response is sent.",
                                        invitation::RsvpCapability::CommandTransportUnavailable => "Read-only preview. RSVP transport is unavailable; no acceptance or email is sent.",
                                    }))
                            })
                            .child(
                                gpui_kit::component::text::TextView::html(
                                    "reader-subject",
                                    preferences::plain_text_html(if reader_subject.is_empty() {
                                        "(No subject)"
                                    } else {
                                        reader_subject
                                    }),
                                )
                                .selectable(true)
                                .w_full()
                                .min_w_0()
                                .text_size(px(24.))
                                .line_height(relative(1.25))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(palette.text),
                            )
                            .child(self.work_controls(snapshot, palette, cx))
                            .child(self.conversation_content(snapshot, &row, palette, cx))
                            .child(self.outgoing_cards(snapshot, palette)),
                    ),
            )
            .into_any_element()
    }

    fn conversation_content(
        &self,
        snapshot: &MailboxSnapshot,
        selected: &Arc<MessageRow>,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let conversation = snapshot.conversations.iter().find(|thread| {
            thread
                .members
                .iter()
                .any(|member| member.key == selected.key)
        });
        let members = conversation
            .map(|thread| thread.members.clone())
            .unwrap_or_else(|| Arc::new(vec![selected.clone()]));
        let content = v_flex()
            .w_full()
            .min_w_0()
            .gap_4()
            .when(members.len() > 1, |el| {
                el.child(
                    h_flex()
                        .flex_wrap()
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child(Icon::new(IconName::Mail).size(px(14.)))
                        .child(format!("{} messages · oldest first", members.len()))
                        .when(conversation.is_some_and(|thread| thread.replied), |el| {
                            el.child(Icon::new(IconName::CornerUpLeft).size(px(14.)))
                                .child("You replied")
                        })
                        .when(conversation.is_some_and(|thread| thread.forwarded), |el| {
                            el.child(Icon::new(IconName::CornerUpRight).size(px(14.)))
                                .child("Forwarded")
                        }),
                )
            })
            .when(snapshot.conversation_loading, |el| {
                el.child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child("Finding messages across this account…"),
                )
            })
            .when_some(snapshot.conversation_error.clone(), |el, error| {
                el.child(error_banner("Conversation", &error, cx, palette))
            })
            .when_some(snapshot.conversation_warning.clone(), |el, warning| {
                el.child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child(warning),
                )
            })
            .children(members.iter().map(|row| {
                let key = row.key.clone();
                let newest = members.last().is_some_and(|m| m.key == key);
                let newest_sent = conversation
                    .and_then(|t| t.latest_sent.as_ref())
                    .is_some_and(|m| m.key == key);
                let open = (key == selected.key
                    || newest
                    || newest_sent
                    || self.expanded_messages.contains(&key))
                    && !self.collapsed_messages.contains(&key);
                let selected_here = key == selected.key;
                let cached = self.reader_bodies.iter().find(|cached| cached.key == key);
                let quote_visible =
                    (!self.preferences.hide_quoted_text || self.visible_quotes.contains(&key)) && !self.hidden_quotes.contains(&key);
                let plain = self.plain_messages.contains(&key);
                let has_quote = cached.is_some_and(|cached| {
                    if plain {
                        preferences::split_quoted_text(&cached.text).1.is_some()
                    } else {
                        cached.has_quote
                    }
                });
                let body: SharedString = cached
                    .map(|cached| {
                        if quote_visible {
                            cached.text.clone()
                        } else {
                            cached.without_quote.clone()
                        }
                    })
                    .unwrap_or_else(|| {
                        if selected_here {
                            selected.message.body.clone().into()
                        } else {
                            row.message.body.clone().into()
                        }
                    });
                let body_element: AnyElement = if let Some(cached) =
                    cached.filter(|body| !body.text.is_empty() || !body.html.is_empty())
                {
                    let target = self.reader_text_identity(snapshot).and_then(|identity| {
                        self.reader_text
                            .as_ref()
                            .filter(|target| selected_here && target.matches(&identity))
                    });
                    let text_view = if let Some(target) = target {
                        gpui_kit::component::text::TextView::new(&target.state)
                    } else {
                        gpui_kit::component::text::TextView::new(if plain {
                            cached.plain_state.as_ref().unwrap()
                        } else { cached.current_state.as_ref().unwrap() })
                    };
                    text_view
                        .selectable(true)
                        .w_full()
                        .min_w_0()
                        .on_link_click(|url, _, _, cx| {
                            if megamail_core::mail_text::safe_browser_url(url) {
                                cx.open_url(url);
                            }
                        })
                        .style(gpui_kit::component::text::TextViewStyle {
                            paragraph_gap: gpui_kit::rems(
                                if plain || cached.source_html.is_empty() {
                                    0.
                                } else {
                                    0.75
                                },
                            ),
                            heading_font_size: Some(Arc::new(|level, _| {
                                px(match level {
                                    1 => 22.,
                                    2 => 19.,
                                    3 => 16.,
                                    _ => 14.,
                                })
                            })),
                            table: {
                                let mut style =
                                    gpui_kit::StyleRefinement::default().bg(palette.surface);
                                style.overflow.x = Some(gpui_kit::Overflow::Scroll);
                                style
                            },
                            is_dark: cx.theme().is_dark(),
                            ..Default::default()
                        })
                        .text_size(px(14.))
                        .line_height(relative(1.6))
                        .text_color(palette.text)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(14.))
                        .line_height(relative(1.6))
                        .text_color(palette.text)
                        .child(if body.is_empty() {
                            if cached.is_some() {
                                "This message has no body.".into()
                            } else if snapshot.body_loading && selected_here {
                                "Loading message body…".into()
                            } else {
                                "Open this message to load its body.".into()
                            }
                        } else {
                            body
                        })
                        .into_any_element()
                };
                let own = self.own_message(&row.message, snapshot);
                let sender = if own {
                    comfort::own_label(
                        snapshot
                            .folder_kinds
                            .get(&(row.key.account_id, row.message.folder_id))
                            .copied(),
                    )
                    .to_owned()
                } else {
                    message_sender(&row.message)
                };
                let sender_color = if own {
                    palette.accent
                } else {
                    comfort::identity_color(&row.message.from_addr, palette)
                };
                let disclosure = Button::new(SharedString::from(format!(
                    "reader-message-{}-{}-{}",
                    key.account_id, key.folder_path, key.uid
                )))
                .ghost()
                .small()
                .size(px(32.))
                .h_auto()
                .px_0()
                .rounded(px(4.))
                .accessibility_label(format!(
                    "{} message from {sender}",
                    if open { "Collapse" } else { "Expand" }
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if open {
                        this.expanded_messages.remove(&key);
                        this.collapsed_messages.insert(key.clone());
                    } else {
                        this.collapsed_messages.remove(&key);
                        this.expanded_messages.insert(key.clone());
                        this.live
                            .update(cx, |live, cx| live.select_message(key.clone(), cx));
                    }
                    let snapshot = this.live.read(cx).snapshot();
                    this.refresh_reader_text(&snapshot, cx);
                    cx.notify();
                }))
                .icon(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(14.)))
                .tooltip(if open { "Collapse message" } else { "Expand message" });
                let header = h_flex().w_full().min_w_0().items_center().gap_3()
                    .child(self.sender_avatar(&row.message.from_addr, &sender, sender_color, cx))
                    .child(v_flex().flex_1().min_w_0().gap_1()
                        .child(gpui_kit::component::text::TextView::html(
                            SharedString::from(format!("sender-name-{}-{}-{}", row.key.account_id, row.key.folder_path, row.key.uid)),
                            preferences::plain_text_html(&sender))
                            .selectable(true).w_full().min_w_0().text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD).text_color(sender_color))
                        .child(gpui_kit::component::text::TextView::html(
                            SharedString::from(format!("sender-address-{}-{}-{}", row.key.account_id, row.key.folder_path, row.key.uid)),
                            preferences::plain_text_html(&row.message.from_addr))
                            .selectable(true).w_full().min_w_0().text_size(px(12.)).text_color(palette.muted))
                        .when(!open, |el| el.child(div().text_size(px(12.)).text_color(palette.muted).child(row.message.preview.clone()))))
                    .child(div().max_w(px(110.)).whitespace_normal().text_size(px(11.)).text_color(palette.muted).child(row.message.date.clone()))
                    .child(disclosure);
                v_flex()
                    .id(SharedString::from(format!(
                        "reader-anchor-{}-{}-{}",
                        row.key.account_id, row.key.folder_path, row.key.uid
                    )))
                    .w_full()
                    .min_w_0()
                    .border_t_1()
                    .border_color(palette.border)
                    .p_4()
                    .rounded(px(8.))
                    .bg(if own {
                        palette.accent_wash
                    } else {
                        palette.surface
                    })
                    .gap_3()
                    .when(selected_here, |el| {
                        el.anchor_scroll(Some(self.reader_anchor.clone()))
                    })
                    .child(header)
                    .when(open, |el| {
                        el.child(
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2()
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "message-details-{}-{}-{}",
                                        row.key.account_id, row.key.folder_path, row.key.uid
                                    )))
                                    .ghost()
                                    .small()
                                    .self_start()
                                    .label(if self.comfort.details.contains(&row.key) {
                                        "Hide details"
                                    } else {
                                        "Message details"
                                    })
                                    .on_click({
                                        let key = row.key.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            if !this.comfort.details.remove(&key) {
                                                this.comfort.details.insert(key.clone());
                                            }
                                            cx.notify();
                                        })
                                    }),
                                )
                                .when(self.comfort.details.contains(&row.key), |el| {
                                    el.child(reader_recipient(
                                        &row.key,
                                        "From",
                                        &if row.message.from_name.is_empty() {
                                            row.message.from_addr.clone()
                                        } else {
                                            format!(
                                                "{} <{}>",
                                                row.message.from_name, row.message.from_addr
                                            )
                                        },
                                        palette,
                                    ))
                                    .child(reader_recipient(&row.key, "Auth", &if selected_here {
                                        snapshot.selected_sender_check.as_ref().map(|check| {
                                            std::iter::once(check.summary.as_str()).chain(check.findings.iter().map(String::as_str)).collect::<Vec<_>>().join("\n")
                                        }).unwrap_or_else(|| "Authentication results unavailable for this message.".to_owned())
                                    } else { "Open this message to view its authentication results.".to_owned() }, palette))
                                    .child(Button::new(SharedString::from(format!("sender-photo-{}-{}-{}", row.key.account_id, row.key.folder_path, row.key.uid)))
                                        .ghost().small().self_start().label(if self.sender_photos.busy() { "Choosing photo…" } else { "Choose local photo…" }).disabled(self.sender_photos.busy())
                                        .on_click({ let email = row.message.from_addr.clone(); cx.listener(move |this, _, _, cx| this.choose_sender_photo(email.clone(), cx)) }))
                                    .child(reader_recipient(
                                        &row.key,
                                        "To",
                                        &row.message.to,
                                        palette,
                                    ))
                                    .child(reader_recipient(
                                        &row.key,
                                        "Date",
                                        &row.message.datetime_full(),
                                        palette,
                                    ))
                                    .when(
                                        !row.message.reply_to.is_empty()
                                            && row.message.reply_to != row.message.from_addr,
                                        |el| {
                                            el.child(reader_recipient(
                                                &row.key,
                                                "Reply to",
                                                &row.message.reply_to,
                                                palette,
                                            ))
                                        },
                                    )
                                    .when(
                                        !row.message.cc.is_empty(),
                                        |el| {
                                            el.child(reader_recipient(
                                                &row.key,
                                                "Cc",
                                                &row.message.cc,
                                                palette,
                                            ))
                                        },
                                    )
                                }),
                        )
                        .when(selected_here, |el| {
                            el.child(self.newsletter_panel(snapshot, palette, cx))
                                .when(self.reader_find.is_open(), |el| {
                                    el.child(self.reader_find_panel(palette, cx))
                                })
                        })
                        .child(body_element)
                        .when(selected_here, |el| {
                            el.when_some(self.reader_export_status.clone(), |el, status| {
                                el.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(status),
                                )
                            })
                            .children(self.rule_preview.iter().cloned().map(|line| {
                                div()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child(line)
                            }))
                            .when(
                                self.rule_move_plan.is_some(),
                                |el| {
                                    el.child(
                                        Button::new("prepare-previewed-rule-move")
                                            .ghost()
                                            .small()
                                            .label("Review proposed move")
                                            .disabled(self.rule_preview_busy || self.triage.busy())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.apply_previewed_rule_move(cx)
                                            })),
                                    )
                                },
                            )
                        })
                        .when(cached.is_none() && !selected_here, |el| {
                            let key = row.key.clone();
                            el.child(
                                Button::new(SharedString::from(format!(
                                    "load-thread-body-{}-{}-{}",
                                    key.account_id, key.folder_path, key.uid
                                )))
                                .ghost()
                                .small()
                                .self_start()
                                .label("Load message")
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.live.update(cx, |live, cx| {
                                            live.select_message(key.clone(), cx)
                                        });
                                    },
                                )),
                            )
                        })
                        .when(has_quote, |el| {
                            let key = row.key.clone();
                            el.child(
                                Button::new(SharedString::from(format!(
                                    "show-quote-{}-{}-{}",
                                    key.account_id, key.folder_path, key.uid
                                )))
                                .ghost()
                                .small()
                                .self_start()
                                .label(if quote_visible {
                                    "Hide quoted history"
                                } else {
                                    "Show quoted history"
                                })
                                .accessibility_label(if quote_visible {
                                    "Hide the original quoted message text"
                                } else {
                                    "Show the original quoted message text"
                                })
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if quote_visible {
                                            this.visible_quotes.remove(&key);
                                            this.hidden_quotes.insert(key.clone());
                                        } else {
                                            this.visible_quotes.insert(key.clone());
                                            this.hidden_quotes.remove(&key);
                                        }
                                        let snapshot = this.live.read(cx).snapshot();
                                        this.refresh_reader_text(&snapshot, cx);
                                        cx.notify();
                                    },
                                )),
                            )
                        })
                        .when(has_quote && quote_visible, |el| {
                            if let Some(cached) = cached {
                                el.child(div().w_full().min_w_0().pt_2().border_t_1().border_color(palette.border)
                                    .child(gpui_kit::component::text::TextView::new(if plain { cached.plain_quote_state.as_ref().unwrap() } else { cached.quote_state.as_ref().unwrap() })
                                        .selectable(true).w_full().min_w_0().text_size(px(14.)).line_height(relative(1.6))
                                        .text_color(palette.muted)
                                        .on_link_click(|url, _, _, cx| { if megamail_core::mail_text::safe_browser_url(url) { cx.open_url(url); } })))
                            } else { el }
                        })
                        .when(
                            members.len() > 1
                                && cached.is_some()
                                && !snapshot.folders.iter().any(|folder| {
                                    folder.path == row.key.folder_path
                                        && folder.kind == FolderKind::Drafts
                                }),
                            |el| {
                                let reply = row.clone();
                                let body = cached.unwrap().text.clone();
                                let reply_all = reply.clone();
                                let all_body = body.clone();
                                let key = row.key.clone();
                                let all_key = key.clone();
                                el.child(
                                    h_flex()
                                        .w_full()
                                        .min_w_0()
                                        .flex_wrap()
                                        .gap_1()
                                        .pt_2()
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "thread-reply-{}-{}-{}",
                                                key.account_id, key.folder_path, key.uid
                                            )))
                                            .ghost()
                                            .small()
                                            .icon(Icon::new(IconName::CornerUpLeft))
                                            .label("Reply")
                                            .accessibility_label("Reply to this message")
                                            .disabled(snapshot.pending_send)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.live.update(cx, |live, cx| {
                                                    live.select_message(key.clone(), cx)
                                                });
                                                let mut message = reply.message.clone();
                                                message.body = body.to_string();
                                                this.open_reply(message, window, cx);
                                            })),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "thread-reply-all-{}-{}-{}",
                                                all_key.account_id,
                                                all_key.folder_path,
                                                all_key.uid
                                            )))
                                            .ghost()
                                            .small()
                                            .label("Reply all")
                                            .accessibility_label(
                                                "Reply to all recipients of this message",
                                            )
                                            .disabled(snapshot.pending_send)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.live.update(cx, |live, cx| {
                                                    live.select_message(all_key.clone(), cx)
                                                });
                                                let mut message = reply_all.message.clone();
                                                message.body = all_body.to_string();
                                                this.open_reply_all(message, window, cx);
                                            })),
                                        ),
                                )
                            },
                        )
                        .when(selected_here, |el| {
                            el.child(self.links_panel(snapshot, palette, cx)).child(
                                self.attachments_panel(snapshot, row.key.clone(), palette, cx),
                            )
                        })
                    })
                    .into_any_element()
            }));
        if self.preferences.reduced_motion || cx.reduce_motion() {
            content.into_any_element()
        } else {
            content
                .with_animation(
                    SharedString::from(format!(
                        "conversation-reveal-{}-{}-{}",
                        selected.key.account_id, selected.key.folder_path, selected.key.uid
                    )),
                    Animation::new(Duration::from_millis(160))
                        .with_easing(|t| 1. - (1. - t).powi(3)),
                    |element, t| element.opacity(0.88 + 0.12 * t),
                )
                .into_any_element()
        }
    }

    fn provider_controls(
        &self,
        setup: &SetupState,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use megamail_core::provider_auth::{MicrosoftAudience as Audience, Provider};
        let admission_reason = self.live.read(cx).direct_add_block_reason();
        let busy = self.demo
            || self.loading_profiles
            || self.startup_error.is_some()
            || admission_reason.is_some()
            || setup.connecting
            || setup.discovering
            || setup.thunderbird_loading
            || self.live.read(cx).snapshot().thunderbird_loading;
        let google = self
            .auth_availability
            .as_ref()
            .is_some_and(|a| a.google.is_ok());
        let microsoft = self
            .auth_availability
            .as_ref()
            .is_some_and(|a| a.microsoft.is_ok());
        v_flex().gap_2()
            .child(h_flex().flex_wrap().gap_2()
                .child(Button::new("google-browser-signin").secondary().small().label("Google · browser sign-in (IMAP/SMTP)").disabled(busy || !google)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_provider_authorization(Provider::GoogleImapSmtp, cx))))
                .child(Button::new("microsoft-browser-signin").secondary().small().label("Microsoft · browser sign-in (Graph)").disabled(busy || !microsoft)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_provider_authorization(Provider::MicrosoftGraph(this.microsoft_audience), cx)))))
            .child(Button::new("microsoft-audience").ghost().small().label(match self.microsoft_audience {
                Audience::Personal => "Microsoft audience: personal", Audience::WorkOrSchool => "Microsoft audience: work/school", Audience::PersonalAndWork => "Microsoft audience: personal + work/school",
            }).disabled(busy).on_click(cx.listener(|this, _, _, cx| {
                this.microsoft_audience = match this.microsoft_audience { Audience::Personal => Audience::WorkOrSchool, Audience::WorkOrSchool => Audience::PersonalAndWork, Audience::PersonalAndWork => Audience::Personal }; cx.notify();
            })))
            .when(!google || !microsoft, |el| el.child(div().text_size(px(11.)).text_color(palette.muted).child("Browser sign-in is disabled without a configured MegaMail-owned desktop registration. App-password/manual IMAP and the separate Thunderbird runtime remain alternatives. Graph is not Exchange IMAP.")))
            .when_some(admission_reason, |el, reason| el.child(div().text_size(px(11.)).text_color(palette.muted).child(reason)))
            .when(self.auth_inflight, |el| el.child(Button::new("cancel-provider-signin").ghost().small().label("Cancel browser sign-in").on_click(cx.listener(|this, _, _, cx| this.cancel_provider_authorization(cx)))))
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
                            .child(self.provider_controls(setup, palette, cx))
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
                                    .child(if self.auth_inflight {
                                        "Finishing browser sign-in: validation, mailbox probe, then private save. Saved is not connected."
                                    } else if setup.connecting {
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
                                        .label(if self.auth_inflight {
                                            "Signing in…"
                                        } else if setup.connecting {
                                            "Testing…"
                                        } else {
                                            "Test and add"
                                        })
                                        .disabled(
                                            setup.connecting
                                                || setup.discovering
                                                || setup.thunderbird_loading
                                                || self.live.read(cx).direct_add_block_reason().is_some()
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
                .accessibility_label("Switch sender address")
                .disabled(!composer.can_edit() || profile.is_none())
                .tooltip(if profile.is_some() { "Choose an existing configured sender identity; provider send-as authorization is not proven" } else { "Sender switching needs an owning account identity validator for this adapter" })
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
                                    .child(if !composer.message.in_reply_to.is_empty() { "Reply" } else if composer.message.draft_origin.is_some() { "Edit draft" } else { "New message" }),
                            )
                            .child(from_control),
                    )
                    .child(Button::new("browse-while-composing").ghost().small().label("Back to mail · keep draft").on_click(cx.listener(|this, _, window, cx| { this.capture_recovery(cx); this.mailbox_focus.focus(window, cx); this.view = View::Mailbox; cx.notify(); })))
                    .child(
                        Button::new("close-composer")
                            .ghost()
                            .small()
                            .icon(Icon::new(if composer_done {
                                IconName::Check
                            } else {
                                IconName::ArrowLeft
                            }))
                            .label(if composer_done { "Done" } else { "Close" })
                            .disabled(!composer.can_close())
                            .accessibility_label(if composer.send_queued {
                                "Close composer; queued message remains in Outbox"
                            } else if composer_saved {
                                "Close composer; saved draft remains in Drafts"
                            } else {
                                "Close composer; local recovery may remain, but session-only text will be lost"
                            })
                            .tooltip(if composer.send_queued {
                                "Close composer; queued message remains in Outbox"
                            } else if composer_saved {
                                "Close composer; saved draft remains in Drafts"
                            } else {
                                "Close composer; local recovery may remain, but session-only text will be lost"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.capture_recovery(cx);
                                let acknowledged = this.composer.as_ref().is_some_and(|c| this.recovery.is_none() || !c.recovery.requires_ack || if c.sent_complete { c.recovery.retired == Some(c.recovery.generation) } else { c.recovery.acknowledged == c.recovery.generation });
                                if !acknowledged {
                                    if let Some(composer) = &mut this.composer { composer.error = Some("Local recovery is pending or failed. Keep this composer open and retry; close/shutdown durability is not acknowledged.".into()); }
                                } else if this.composer.as_ref().is_some_and(Composer::can_close) {
                                    this.cleanup_composer_attachments(cx); this.composer = None; this.view = View::Mailbox;
                                }
                                cx.notify();
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
                            .child(self.compose_tools_view(palette, cx))
                            .when(self.live.read(cx).schedule_session(composer.message.from_account_id).is_some(), |el| el
                                .child(labeled_input("Send Later", Input::new(&composer.schedule).aria_label("Scheduled date, time and numeric UTC offset").disabled(!composer.can_edit()), palette))
                                .child(h_flex().flex_wrap().gap_2()
                                    .child(Button::new("apply-send-time").ghost().small().label("Set time").disabled(!composer.can_edit()).on_click(cx.listener(|this, _, window, cx| this.set_compose_schedule(false, window, cx))))
                                    .child(Button::new("clear-send-time").ghost().small().label("Send now instead").disabled(!composer.can_edit() || composer.message.send_at.is_none()).on_click(cx.listener(|this, _, window, cx| this.set_compose_schedule(true, window, cx)))))
                                .child(div().text_size(px(11.)).text_color(palette.muted).child("Local scheduling requires MegaMail running and connected. When closed, delivery resumes after the next launch.")))
                            .child(labeled_input(
                                "Subject",
                                Input::new(&composer.subject)
                                    .aria_label("Subject")
                                    .disabled(!composer.can_edit()),
                                palette,
                            ))
                            .child(self.markup_controls(palette, cx))
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
                                                        composer.recovery.dirty_input = true;
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
                            .child(div().text_size(px(11.)).text_color(palette.muted).child(composer.recovery.status.clone()))
                            .when_some(self.recovery_error.clone(), |el, error| el.child(error_banner("Local recovery", &error, cx, palette)))
                            .child(Button::new("retry-local-recovery").ghost().small().label(if composer.sent_complete { "Retry recovery cleanup" } else { "Retry local recovery" }).disabled(composer.operation_pending() || if composer.sent_complete { composer.recovery.cleanup_in_flight || composer.recovery.retired == Some(composer.recovery.generation) } else { composer.recovery.in_flight.is_some() || composer.recovery.acknowledged == composer.recovery.generation }).on_click(cx.listener(|this, _, _, cx| this.retry_native_recovery(cx))))
                            .when(!composer.message.html.is_empty(), |el| el.child(Button::new("convert-recovered-html").ghost().small().label("Discard HTML alternative and edit plain text").disabled(composer.operation_pending()).on_click(cx.listener(|this, _, _, cx| {
                                if let Some(composer) = &mut this.composer { if !composer.operation_pending() { composer.message.html.clear(); composer.recovery.dirty_input = true; composer.notice = Some("HTML was discarded explicitly. Only the recovered plain-text alternative will be edited/sent.".into()); } }
                                this.capture_recovery(cx); cx.notify();
                            }))))
                            .when(composer.message.draft_origin.is_some() && self.live.read(cx).server_draft_retirement_unavailable(composer.message.from_account_id), |row| {
                                row.child(div().text_size(px(11.)).text_color(palette.muted).child("IMAP Send/Save draft uses a separate copy. The original server draft is retained for manual review: generation-qualified retirement is unavailable. No automatic deletion or EXPUNGE is authorized by this composer."))
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
                                                .child("The adapter did not confirm whether the server accepted this message. Inspect Sent and Outbox. Resend remains locked because ticket reconciliation is not available."),
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
                                                        .label("Resend review unavailable")
                                                        .disabled(true)
                                                        .tooltip("A typed durable send-ticket reconciliation is required; a text confirmation cannot unlock resend")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.confirm_send_not_delivered(cx)
                                                        })),
                                                ),
                                        ),
                                )
                            })
                            .when_some(composer.error.clone(), |row, error| {
                                row.child(error_banner("Composer", &error, cx, palette))
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
                                    .child(if composer.markup.enabled { "Formatted Markdown · HTML + plain text" } else if !composer.message.html.is_empty() { "Original HTML retained" } else { "Plain-text message" }),
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
                                                self.demo || !composer.can_edit() || composer_saved || composer.recovery.blocks_provider_origin(&composer.message),
                                            )
                                            .tooltip(if composer.recovery.blocks_provider_origin(&composer.message) { "Recovered provider Draft/Outbox origins are unversioned; safe replacement/reconciliation is unavailable" } else { "Save to provider Drafts; this is separate from private local recovery" })
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
                                            } else if composer.message.send_at.is_some() {
                                                "Schedule"
                                            } else {
                                                "Send"
                                            })
                                            .disabled(self.demo || !composer.can_edit() || composer.recovery.restored_hold)
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
        if let Some(composer) = self
            .composer
            .as_mut()
            .filter(|c| c.can_edit() && !c.recovery.restored_hold)
        {
            match megamail_core::rng::token(24) {
                Ok(token) => {
                    composer.message.client_message_id = Some(format!(
                        "megamail-{token}@{}",
                        self.profiles
                            .iter()
                            .find(|(id, _)| *id == composer.message.from_account_id)
                            .and_then(|(_, p)| p.email.rsplit_once('@').map(|(_, domain)| domain))
                            .unwrap_or("megamail.local")
                    ))
                }
                Err(_) => {
                    composer.error =
                        Some("Could not create a safe send identity. Nothing was sent.".into());
                    cx.notify();
                    return;
                }
            }
        }
        self.capture_recovery(cx);
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
        if composer.markup.enabled
            && (composer.markup.busy
                || composer.markup.error.is_some()
                || composer.markup.ready_source != composer.body.read(cx).value().as_ref())
        {
            composer.error = Some(
                "Wait for the formatting preview to finish before sending, or choose Plain text."
                    .into(),
            );
            cx.notify();
            return;
        }
        if composer.recovery.restored_hold {
            composer.error = Some("Recovered content is not permission to resend. Reconciliation with held/queued Outbox is unavailable; sending remains blocked. You can edit and save a provider draft.".into());
            cx.notify();
            return;
        }
        let desired_schedule = megamail_core::datefmt::parse_send_time(
            &composer.schedule.read(cx).value(),
            megamail_core::datefmt::now(),
        );
        match desired_schedule {
            Err(error) => {
                composer.error = Some(error.into());
                cx.notify();
                return;
            }
            Ok(at) if at != composer.message.send_at => {
                composer.error = Some(
                    "Use Set time to review this schedule, or Send now instead to clear it.".into(),
                );
                cx.notify();
                return;
            }
            _ => {}
        }
        if composer.message.send_at.is_some()
            && self
                .live
                .read(cx)
                .schedule_session(composer.message.from_account_id)
                .is_none()
        {
            composer.error = Some("This account cannot schedule mail in MegaMail. Clear the schedule or choose a direct IMAP account.".into());
            cx.notify();
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
        let outgoing_card = (!message.in_reply_to.is_empty()
            && self
                .live
                .read(cx)
                .schedule_session(message.from_account_id)
                .is_some())
        .then(|| comfort::OutgoingCard {
            account: message.from_account_id,
            request: 0,
            parent: comfort::normalized_id(&message.in_reply_to),
            message_id: message.client_message_id.clone().unwrap_or_default(),
            body: message.body.clone(),
            to: message.to.clone(),
            status: "Sending…",
        });
        let request_id = self
            .live
            .update(cx, |live, cx| live.send_message(message, cx));
        if let Some(request_id) = request_id {
            if let Some(mut card) = outgoing_card {
                card.request = request_id;
                self.comfort.outgoing.push(card);
            }
            if let Some(composer) = self.composer.as_mut() {
                composer.send_request_id = Some(request_id);
                composer.send_generation = Some(composer.recovery.generation);
                if let Some(service) = &self.recovery {
                    self.request_attachment_leases.hold(
                        true,
                        composer.message.from_account_id,
                        request_id,
                        composer.recovery.lease.clone(),
                        service.reaper(),
                    );
                }
            }
        }
        cx.notify();
    }

    fn save_current_draft(&mut self, cx: &mut Context<Self>) {
        self.capture_recovery(cx);
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
        if composer.recovery.blocks_provider_origin(&composer.message) {
            if let Some(composer) = &mut self.composer {
                composer.error = Some("Recovered provider Draft/Outbox origins are not replacement authority. A versioned origin or durable ticket reconciliation is required; no provider mutation was dispatched.".into());
            }
            cx.notify();
            return;
        }
        if composer.draft_save_completed {
            if let Some(composer) = &mut self.composer {
                composer.error = Some("Reopen the saved draft from Drafts before saving a replacement: this adapter does not return a versioned remote draft origin.".into());
            }
            cx.notify();
            return;
        }
        let Some(ticket) = self.next_draft_ticket.checked_add(1) else {
            return;
        };
        self.next_draft_ticket = ticket;
        let message = composer.outgoing(cx);
        let pending_message = message.clone();
        let accepted = self
            .live
            .update(cx, |live, cx| live.save_draft(message, Some(ticket), cx));
        if accepted {
            if let Some(composer) = self.composer.as_mut() {
                composer.draft_pending = Some(pending_message);
                composer.draft_ticket = Some(ticket);
                if let Some(service) = &self.recovery {
                    self.request_attachment_leases.hold(
                        false,
                        composer.message.from_account_id,
                        u64::from(ticket),
                        composer.recovery.lease.clone(),
                        service.reaper(),
                    );
                }
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
                            composer.recovery.dirty_input = true;
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
        let available_width = f32::from(window.viewport_size().width);
        let pane_scale = ((available_width - 1000.) / 280.).clamp(0., 1.);
        let rail_width = px(192. + 40. * pane_scale);
        let list_width = px(280. + 52. * pane_scale);
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
            "Settings".to_owned()
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
                                    .disabled(
                                        !self
                                            .action_state(cx)
                                            .allows(desktop_actions::MailboxAction::RefreshMail),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this
                                            .action_state(cx)
                                            .allows(desktop_actions::MailboxAction::RefreshMail)
                                        {
                                            this.live.update(cx, |live, cx| live.refresh(cx));
                                        }
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
                                "Return from settings"
                            } else {
                                "Open settings"
                            })
                            .tooltip("Settings")
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
                .child(self.sidebar(&snapshot, palette, cx, rail_width))
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
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    gpui_kit::component::resizable::h_resizable(if self.sidebar_collapsed {
                        "mail-panes-compact"
                    } else {
                        "mail-panes"
                    })
                    .with_state(if self.sidebar_collapsed {
                        &self.compact_mail_panes
                    } else {
                        &self.mail_panes
                    })
                    .child(
                        gpui_kit::component::resizable::resizable_panel()
                            .size(if self.sidebar_collapsed {
                                px(48.)
                            } else {
                                rail_width
                            })
                            .size_range(if self.sidebar_collapsed {
                                px(48.)..px(49.)
                            } else {
                                px(176.)..px(420.)
                            })
                            .child(self.sidebar(&snapshot, palette, cx, px(0.))),
                    )
                    .child(
                        gpui_kit::component::resizable::resizable_panel()
                            .size(list_width)
                            .size_range(px(240.)..px(680.))
                            .child(self.message_pane(&snapshot, palette, cx, px(0.))),
                    )
                    .child(
                        gpui_kit::component::resizable::resizable_panel()
                            .size_range(px(300.)..gpui_kit::Pixels::MAX)
                            .child(if self.view == View::Compose {
                                self.composer_view(&snapshot, palette, cx)
                            } else {
                                self.reader(&snapshot, palette, cx)
                            }),
                    ),
                )
                .into_any_element()
        };
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
                .child(body),
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

fn message_row(
    message: &Message,
    palette: Palette,
    unread: bool,
    account_label: Option<String>,
) -> impl IntoElement {
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
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .size(px(20.))
                                .rounded_full()
                                .items_center()
                                .justify_center()
                                .text_size(px(9.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .bg(comfort::identity_color(&message.from_addr, palette)
                                    .opacity(0.12))
                                .text_color(comfort::identity_color(&message.from_addr, palette))
                                .child(initials(&message_sender(message))),
                        )
                        .when(unread, |row| {
                            row.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(palette.accent))
                        })
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .font_weight(if unread {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(comfort::identity_color(&message.from_addr, palette))
                                .child(message_sender(message)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(12.))
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
                        .font_weight(if unread {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if unread { palette.text } else { palette.muted })
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
            h_flex()
                .w_full()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child(message.preview.clone()),
                )
                .when_some(account_label, |row, label| {
                    row.child(
                        div()
                            .max_w(px(88.))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(palette.faint)
                            .child(label),
                    )
                }),
        )
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
