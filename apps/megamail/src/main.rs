use std::borrow::Cow;

use gpui_kit::base::Selectable as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Decorations, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, KeyBinding, MouseButton, ParentElement as _, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    UniformListScrollHandle, Window, WindowBounds, WindowDecorations, WindowOptions, div,
    prelude::FluentBuilder as _, px, relative, rgb, size, uniform_list,
};
use gpui_kit::{
    assets::IconName,
    component::{
        ActiveTheme as _, Icon, Sizable as _, Theme, ThemeMode, TitleBar,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputEvent, InputState},
        v_flex, window_border,
    },
};

use megamail::{Folder, MESSAGES, Mailbox, Message};

mod action {
    gpui_kit::actions!(megamail, [NextMessage, PreviousMessage]);
}

use action::{NextMessage, PreviousMessage};

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

fn apply_theme(mode: ThemeMode, window: &mut Window, cx: &mut App) {
    Theme::change(mode, Some(window), cx);
    let palette = Palette::new(mode.is_dark());
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
        theme.radius = px(8.);
        theme.radius_lg = px(16.);
        theme.font_family = "Geist".into();
    });
}

struct MailApp {
    mailbox: Mailbox,
    search: gpui_kit::Entity<InputState>,
    message_focus: FocusHandle,
    list_scroll: UniformListScrollHandle,
    notice: Option<SharedString>,
    dark: bool,
    _subscriptions: Vec<Subscription>,
}

impl MailApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("MegaMail");
        apply_theme(ThemeMode::Dark, window, cx);

        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search sample mail"));
        let message_focus = cx.focus_handle().tab_stop(true);
        let _subscriptions = vec![cx.subscribe_in(&search, window, {
            let search = search.clone();
            move |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.mailbox.set_query(search.read(cx).value().to_string());
                    this.list_scroll = UniformListScrollHandle::new();
                    this.notice = None;
                    cx.notify();
                }
            }
        })];

        Self {
            mailbox: Mailbox::default(),
            search,
            message_focus,
            list_scroll: UniformListScrollHandle::new(),
            notice: None,
            dark: true,
            _subscriptions,
        }
    }

    fn set_folder(&mut self, folder: Folder, cx: &mut Context<Self>) {
        self.mailbox.set_folder(folder);
        self.list_scroll = UniformListScrollHandle::new();
        self.notice = None;
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.mailbox.move_selection(delta);
        if let Some(row) = self
            .mailbox
            .visible_ids()
            .iter()
            .position(|id| Some(*id) == self.mailbox.selected())
        {
            self.list_scroll
                .scroll_to_item(row, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn move_message(&mut self, restore: bool, cx: &mut Context<Self>) {
        let changed = if restore {
            self.mailbox.restore_selected()
        } else {
            self.mailbox.archive_selected()
        };
        if changed {
            self.notice = Some(if restore {
                "Sample message restored · local demo".into()
            } else {
                "Sample message archived · local demo".into()
            });
            self.list_scroll = UniformListScrollHandle::new();
            cx.notify();
        }
    }

    fn folder_button(
        &self,
        folder: Folder,
        label: &'static str,
        icon: IconName,
        cx: &mut Context<Self>,
    ) -> Button {
        let selected = self.mailbox.folder() == folder;
        let count = match folder {
            Folder::Inbox => self.mailbox.unread_count(),
            Folder::Drafts | Folder::Starred | Folder::Archive => self.mailbox.count(folder),
            Folder::Sent => 0,
        };
        Button::new(SharedString::from(format!("folder-{label}")))
            .ghost()
            .small()
            .selected(selected)
            .w_full()
            .h(px(36.))
            .rounded(px(8.))
            .px_2()
            .icon(Icon::new(icon).small())
            .accessibility_label(label)
            .on_click(cx.listener(move |this, _, _, cx| this.set_folder(folder, cx)))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(13.)).child(label))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(if selected {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(if count == 0 {
                                String::new()
                            } else {
                                count.to_string()
                            }),
                    ),
            )
    }

    fn message_list(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let visible_ids = self.mailbox.visible_ids();
        if visible_ids.is_empty() {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .px_5()
                .text_center()
                .child(
                    Icon::new(IconName::Inbox)
                        .size(px(24.))
                        .text_color(palette.faint),
                )
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("No messages here"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child("Try another folder or change your search."),
                )
                .into_any_element();
        }

        let selected = self.mailbox.selected();
        let owner = cx.entity().downgrade();
        uniform_list("message-rows", visible_ids.len(), move |range, _, _| {
            range
                .map(|row| {
                    let id = visible_ids[row];
                    let message = MESSAGES[id];
                    let active = selected == Some(id);
                    let owner = owner.clone();
                    Button::new(SharedString::from(format!("message-{id}")))
                        .ghost()
                        .small()
                        .selected(active)
                        .tab_stop(true)
                        .w_full()
                        .h(px(86.))
                        .rounded(px(10.))
                        .px_3()
                        .accessibility_label(format!("{}: {}", message.sender, message.subject))
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.mailbox.select(id);
                                this.notice = None;
                                cx.notify();
                            });
                        })
                        .child(message_row(message, palette))
                        .into_any_element()
                })
                .collect()
        })
        .size_full()
        .track_scroll(&self.list_scroll)
        .into_any_element()
    }

    fn reader(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(id) = self.mailbox.selected() else {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    Icon::new(IconName::Mail)
                        .size(px(28.))
                        .text_color(palette.faint),
                )
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Select a message"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child("Your sample message preview will appear here."),
                )
                .into_any_element();
        };

        let message = MESSAGES[id];
        let restoring = self.mailbox.folder() == Folder::Archive;
        let action_label = if restoring { "Restore" } else { "Archive" };
        let action_icon = if restoring {
            IconName::ArchiveRestore
        } else {
            IconName::Archive
        };

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
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .child(folder_label(self.mailbox.folder()))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(px(12.))
                                    .text_color(palette.faint),
                            )
                            .child("Sample message"),
                    )
                    .child(
                        Button::new("archive-message")
                            .secondary()
                            .small()
                            .icon(Icon::new(action_icon))
                            .label(action_label)
                            .accessibility_label(format!("{action_label} this sample message"))
                            .tooltip(format!("{action_label} this message in the local demo"))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.move_message(restoring, cx)),
                            ),
                    ),
            )
            .child(
                div()
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
                            .child(message_content(message, palette)),
                    ),
            )
            .into_any_element()
    }
}

impl Render for MailApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let palette = Palette::new(dark);
        let folder = self.mailbox.folder();
        let visible_count = self.mailbox.visible_ids().len();
        let title = folder_label(folder);
        let unread = self.mailbox.unread_count();

        let client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let toolbar = h_flex()
            .w_full()
            .h(px(38.))
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
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(5.)).h(px(5.)).rounded_full().bg(palette.accent))
                    .child(
                        div()
                            .text_color(palette.muted)
                            .text_size(px(10.))
                            .child("Sample mail · Local demo"),
                    ),
            )
            .child(
                Button::new("toggle-theme")
                    .ghost()
                    .small()
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
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dark = !this.dark;
                        apply_theme(
                            if this.dark {
                                ThemeMode::Dark
                            } else {
                                ThemeMode::Light
                            },
                            window,
                            cx,
                        );
                        cx.notify();
                    })),
            );
        let chrome = if client_decorated {
            TitleBar::new().h(px(38.)).child(toolbar).into_any_element()
        } else {
            // A compositor-owned caption supplies the title and window controls.
            toolbar.into_any_element()
        };

        window_border().child(
            v_flex()
                .size_full()
                .bg(palette.background)
                .text_color(palette.text)
                .child(chrome)
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .border_t_1()
                        .border_color(palette.border)
                        .child(self.sidebar(palette, cx))
                        .child(self.message_pane(palette, title, visible_count, unread, cx))
                        .child(self.reader(palette, cx)),
                ),
        )
    }
}

impl MailApp {
    fn sidebar(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(232.))
            .h_full()
            .flex_shrink_0()
            .bg(palette.sidebar)
            .border_r_1()
            .border_color(palette.border)
            .px_3()
            .py_4()
            .gap_5()
            .child(
                v_flex()
                    .gap_1()
                    .px_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text)
                            .child("Maya's sample mailbox"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .child("Demo profile · local only"),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(self.folder_button(Folder::Inbox, "Inbox", IconName::Inbox, cx))
                    .child(self.folder_button(Folder::Starred, "Starred", IconName::Star, cx))
                    .child(self.folder_button(Folder::Drafts, "Drafts", IconName::FileText, cx))
                    .child(self.folder_button(Folder::Sent, "Sent", IconName::Send, cx))
                    .child(self.folder_button(Folder::Archive, "Archive", IconName::Archive, cx)),
            )
            .child(div().flex_1())
            .child(
                v_flex()
                    .gap_3()
                    .border_t_1()
                    .border_color(palette.border)
                    .pt_4()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size_9()
                                    .rounded_full()
                                    .bg(palette.accent_wash)
                                    .text_color(palette.accent)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(11.))
                                    .child("MV"),
                            )
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Maya Vertex"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(palette.muted)
                                            .truncate()
                                            .child("SAMPLE PROFILE"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .line_height(relative(1.35))
                            .text_color(palette.faint)
                            .child("No account connected. All actions stay in this local preview."),
                    ),
            )
    }

    fn message_pane(
        &self,
        palette: Palette,
        title: &'static str,
        visible_count: usize,
        unread: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .w(px(360.))
            .h_full()
            .flex_shrink_0()
            .min_h_0()
            .bg(palette.list)
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
                        div()
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .child(format!(
                                "{visible_count} {}",
                                if visible_count == 1 {
                                    "message"
                                } else {
                                    "messages"
                                }
                            )),
                    ),
            )
            .child(
                v_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .child(
                        Input::new(&self.search)
                            .prefix(
                                Icon::new(IconName::Search)
                                    .size(px(14.))
                                    .text_color(palette.faint),
                            )
                            .cleanable(true)
                            .aria_label("Search sample messages"),
                    )
                    .when_some(self.notice.clone(), |this, notice| {
                        this.child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded(px(6.))
                                .bg(palette.accent_wash)
                                .text_size(px(10.))
                                .text_color(palette.accent)
                                .child(notice),
                        )
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
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.faint)
                    .child("RECENT")
                    .child(if visible_count == 1 {
                        "1 MESSAGE".to_owned()
                    } else {
                        format!("{visible_count} MESSAGES")
                    }),
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
                            .child(self.message_list(palette, cx)),
                    ),
            )
            .child(
                h_flex()
                    .h(px(34.))
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .border_t_1()
                    .border_color(palette.border)
                    .text_size(px(10.))
                    .text_color(palette.muted)
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .size(px(12.))
                            .text_color(palette.accent),
                    )
                    .child(format!("{unread} unread · sample data only")),
            )
    }
}

fn message_row(message: Message, palette: Palette) -> impl IntoElement {
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
                        .when(message.unread, |this| {
                            this.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(palette.accent))
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
                                .child(message.sender),
                        ),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(palette.faint)
                        .child(message.time),
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
                        .child(message.subject),
                )
                .when(message.starred, |this| {
                    this.child(
                        Icon::new(IconName::StarFill)
                            .size(px(12.))
                            .text_color(palette.accent),
                    )
                }),
        )
        .child(
            div()
                .w_full()
                .truncate()
                .text_size(px(11.))
                .text_color(palette.muted)
                .child(message.preview),
        )
}

fn message_content(message: Message, palette: Palette) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_5()
        .child(
            div()
                .text_size(px(22.))
                .line_height(relative(1.2))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text)
                .child(message.subject),
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
                        .child(message.initials),
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
                                .child(message.sender),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(palette.muted)
                                .truncate()
                                .child(message.address),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(palette.faint)
                        .child(message.time),
                ),
        )
        .child(div().h(px(1.)).bg(palette.border))
        .children(message.body.iter().map(|paragraph| {
            div()
                .text_size(px(14.))
                .line_height(relative(1.5))
                .text_color(palette.text)
                .child(*paragraph)
        }))
        .child(
            div()
                .pt_2()
                .text_size(px(11.))
                .text_color(palette.faint)
                .child("Fictional sample content · no mailbox or network action is active."),
        )
}

fn folder_label(folder: Folder) -> &'static str {
    match folder {
        Folder::Inbox => "Inbox",
        Folder::Starred => "Starred",
        Folder::Drafts => "Drafts",
        Folder::Sent => "Sent",
        Folder::Archive => "Archive",
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
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
                app_id: Some("com.megamail.preview".into()),
                window_decorations: Some(WindowDecorations::Client),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| MailApp::new(window, cx))
            })
            .expect("Failed to open MegaMail window");
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use gpui_kit::AssetSource;
    use gpui_kit::assets::AllAssets;

    #[test]
    fn mail_actions_have_bundled_icons() {
        for icon in [
            super::IconName::Mail,
            super::IconName::Send,
            super::IconName::Archive,
            super::IconName::ArchiveRestore,
        ] {
            assert!(AllAssets.load(&icon.path()).unwrap().is_some());
        }
    }
}
