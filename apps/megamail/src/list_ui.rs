//! Sidebar and message list.
use crate::*;

impl MailApp {
    pub(crate) fn account_button(
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

    pub(crate) fn sidebar(
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
                            .child(nav_content(Icon::new(IconName::PenLine).size(px(14.)), if self.composer.is_some() {
                                "Resume draft"
                            } else {
                                "New message"
                            }))
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
                            .child(nav_content(Icon::new(IconName::Settings2).size(px(14.)), "Settings"))
                            .selected(self.view == View::Appearance)
                            .on_click(cx.listener(|this, _, _, cx| this.open_appearance(cx))),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn unified_button(
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
        .when(collapsed, |el| el.icon(Icon::new(IconName::Inbox).size(px(14.))))
        .when(!collapsed, |el| el.child(nav_content(Icon::new(IconName::Inbox).size(px(14.)), "All inboxes")))
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

    pub(crate) fn collapsed_sidebar(
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
                            .icon(Icon::new(IconName::Settings))
                            .tooltip("Settings · Ctrl+,")
                            .accessibility_label("Settings")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.composer.is_none() {
                                    this.open_appearance(cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn message_list(
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
                "All clear"
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
                        if self.comfort.filter.is_some() { "Choose All work states or load more results to broaden this view." } else { "You're caught up. New mail lands here as it arrives." }
                    } else {
                        "Try another search or choose All local mail in Search scope."
                    },
                ))
                .into_any_element();
        }

        let selected = snapshot.selected_key.clone();
        let rows = self.list_rows.clone();
        let two_line = self.preferences.layout == preferences::PaneLayout::Right;
        let row_height = match (self.preferences.density, two_line) {
            (Density::Compact, false) => 36.,
            (Density::Comfortable, false) => 44.,
            (Density::Compact, true) => 50.,
            (Density::Comfortable, true) => 60.,
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
        let photos = self.preferences.list_avatars.then(|| self.sender_photos.images());
        let swipe = self.swipe.clone();
        uniform_list("message-rows", rows.len(), move |range, _, cx| {
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
                        // Mail you sent is listed by who it went to.
                        display_message.from_name = format!("To {}", recipient_names(&display_message.to));
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
                    let travel = swipe
                        .as_ref()
                        .filter(|(swiped, _)| *swiped == row.key)
                        .map_or(0., |(_, travel)| *travel);
                    let row_id = format!("{}-{}-{}-{}", key.account_id, key.folder_path, key.uid, child);
                    let has_toggle = conversation
                        .as_ref()
                        .is_some_and(|thread| thread.count > 1 || thread.replied || thread.forwarded);
                    let hover_actions = (!bulk_open && !child && travel == 0.).then(|| {
                        let button = |name: &'static str, icon: IconName, tip: &'static str, action: RowAction| {
                            let owner = owner.clone();
                            let key = key.clone();
                            div()
                                .id(SharedString::from(format!("row-{name}-{row_id}")))
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .text_color(palette.muted)
                                .hover(|s| s.bg(palette.selected).text_color(palette.text))
                                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx))
                                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let key = key.clone();
                                    let _ = owner.update(cx, |this, cx| this.row_action(key, action, unread, cx));
                                })
                                .child(Icon::new(icon).size(px(15.)))
                        };
                        h_flex()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(px(if has_toggle { 52. } else { 6. }))
                            .items_center()
                            .gap(px(2.))
                            .pl_2()
                            .bg(palette.background.blend(if active { palette.selected } else { palette.hover }))
                            .invisible()
                            .group_hover("message-row", |s| s.visible())
                            .child(button("archive", IconName::Archive, "Done · E", RowAction::Archive))
                            .child(button("star", IconName::Star, "Star · S", RowAction::Star))
                            .child(button(
                                "read",
                                if unread { IconName::MailOpen } else { IconName::Mail },
                                if unread { "Mark read · U" } else { "Mark unread · U" },
                                RowAction::Read,
                            ))
                            .child(button("trash", IconName::Trash, "Trash · #", RowAction::Trash))
                    });
                    let owner_swipe = owner.clone();
                    let owner_menu = owner.clone();
                    let key_swipe = key.clone();
                    let key_menu = key.clone();
                    let mut element = h_flex()
                        .id(SharedString::from(format!("row-{row_id}")))
                        .group("message-row")
                        .relative()
                        .w_full()
                        .h(px(row_height))
                        .items_center()
                        .rounded(px(4.))
                        .when(active, |el| el.bg(palette.selected))
                        .when(child, |el| el.pl_4())
                        .on_scroll_wheel(move |event, _, cx| {
                            let delta = event.delta.pixel_delta(px(16.));
                            let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                            let ended = matches!(event.touch_phase, gpui_kit::TouchPhase::Ended);
                            if !ended && dx.abs() <= dy.abs() * 1.5 {
                                return;
                            }
                            cx.stop_propagation();
                            let key = key_swipe.clone();
                            let _ = owner_swipe.update(cx, |this, cx| this.swipe_row(key, dx, ended, unread, cx));
                        })
                        .on_mouse_down(gpui_kit::MouseButton::Right, move |_, _, cx| {
                            let key = key_menu.clone();
                            let _ = owner_menu.update(cx, |this, cx| {
                                this.live.update(cx, |live, cx| {
                                    live.select_message(key, cx);
                                });
                                cx.notify();
                            });
                        })
                        .when(travel != 0., |el| {
                            let archive = travel < 0.;
                            let armed = travel.abs() >= SWIPE_COMMIT;
                            el.child(
                                h_flex()
                                    .absolute()
                                    .inset_0()
                                    .rounded(px(4.))
                                    .px_5()
                                    .items_center()
                                    .when(archive, |el| el.justify_end())
                                    .bg(if armed { palette.accent } else { palette.selected })
                                    .text_color(if armed { palette.background } else { palette.muted })
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .gap_2()
                                    .child(Icon::new(if archive { IconName::Archive } else if unread { IconName::MailOpen } else { IconName::Mail }).size(px(15.)))
                                    .child(if archive { "Done" } else if unread { "Mark read" } else { "Mark unread" }),
                            )
                        })
                        .child(
                            // The row slides under the fingers on an opaque card.
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .rounded(px(4.))
                                .when(travel != 0., |el| {
                                    el.relative().left(px(travel)).bg(palette.background.blend(if active {
                                        palette.selected
                                    } else {
                                        palette.hover
                                    }))
                                })
                                .child(
                            Button::new(SharedString::from(format!(
                                "message-{}-{}-{}-{}",
                                key.account_id, key.folder_path, key.uid, child
                            )))
                            .custom({
                                // Hover is lighter than selection, so the two never look alike.
                                let fill = if active { palette.selected } else { palette.hover };
                                ButtonCustomVariant::new(cx)
                                    .color(cx.theme().transparent)
                                    .foreground(palette.text)
                                    .hover(fill)
                                    .active(fill)
                            })
                            .small()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .rounded(px(4.))
                            .px_3()
                            .w_full()
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
                                    this.conversation_open = true;
                                    cx.notify();
                                });
                            })
                            .child(message_row(
                                &display_message,
                                two_line,
                                palette,
                                photos.as_ref().map(|photos| {
                                    sender_photos::email_key(&row.message.from_addr)
                                        .and_then(|key| photos.get(&key).cloned())
                                }),
                                unread,
                                (unified && !child)
                                    .then(|| labels.get(&row.key.account_id).cloned())
                                    .flatten(),
                            ))),
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
                                .when(travel != 0., |el| el.invisible())
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
                    let owner_menu = owner.clone();
                    let key_menu = row.key.clone();
                    use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
                    element
                        .children(hover_actions)
                        .context_menu(move |menu, _, _| {
                            let item = |label: &'static str, action: RowAction| {
                                let owner = owner_menu.clone();
                                let key = key_menu.clone();
                                PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                    let key = key.clone();
                                    let _ = owner.update(cx, |this, cx| this.row_action(key, action, unread, cx));
                                })
                            };
                            menu.item(item("Done    E", RowAction::Archive))
                                .item(item("Star    S", RowAction::Star))
                                .item(item(if unread { "Mark read    U" } else { "Mark unread    U" }, RowAction::Read))
                                .separator()
                                .item(item("Move to Trash    #", RowAction::Trash))
                        })
                        .into_any_element()
                })
                .collect()
        })
        .size_full()
        .track_scroll(&self.list_scroll)
        .into_any_element()
    }

    pub(crate) fn filter_menu(
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

    /// All · People · Screener · Updates · Newsletters, underlined when active.
    pub(crate) fn split_tabs(&self, palette: Palette, cx: &mut Context<Self>, height: f32) -> AnyElement {
        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_4()
                            .children(splits::Split::ALL.iter().enumerate().map(|(index, split)| {
                                let split = *split;
                                let unread = self.split_unread[index];
                                let active = self.split == split;
                                h_flex()
                                    .id(SharedString::from(format!("split-{index}")))
                                    .h(px(height))
                                    .gap_1()
                                    .items_center()
                                    .border_b_2()
                                    .border_color(if active { palette.accent } else { gpui_kit::transparent_black() })
                                    .cursor_pointer()
                                    .text_size(px(13.))
                                    .text_color(if active { palette.text } else { palette.muted })
                                    .when(active, |el| el.font_weight(FontWeight::MEDIUM))
                                    .hover(|s| s.text_color(palette.text))
                                    .on_click(cx.listener(move |this, _, _, cx| this.set_split(split, cx)))
                                    .child(split.label())
                                    .when(unread > 0 && split != splits::Split::All, |el| {
                                        el.child(div().text_size(px(11.)).text_color(palette.faint).child(unread.to_string()))
                                    })
                            }))
                            .when(self.split == splits::Split::Newsletters, |el| {
                                el.child(div().flex_1()).child(
                                    Button::new("newsletters-manage")
                                        .ghost()
                                        .small()
                                        .label("Unsubscribe in bulk")
                                        .on_click(cx.listener(|this, _, _, cx| this.open_subscriptions(cx))),
                                )
                            })
        .into_any_element()
    }
}
