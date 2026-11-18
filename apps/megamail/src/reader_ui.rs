//! Reader pane: toolbar, conversation, panels, app bar.
use crate::*;

impl MailApp {
    pub(crate) fn message_pane(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
        width: gpui_kit::Pixels,
    ) -> impl IntoElement {
        let title = snapshot.title.clone();
        let unavailable = snapshot.account_errors.len();
        // Side-by-side: the tabs get their own row under the title.
        let narrow = self.preferences.layout == preferences::PaneLayout::Right;
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
            .bg(self.pane_surface(palette.background, 0.05))
            .child(
                h_flex()
                    .h(px(48.))
                    .flex_shrink_0()
                    .items_center()
                    .gap_4()
                    .px_6()
                    .child(
                        div()
                            .flex_shrink_0()
                            .max_w(px(if narrow { 140. } else { 220. }))
                            .truncate()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(if narrow { div().flex_1().into_any_element() } else { self.split_tabs(palette, cx, 48.) })
                    .child(self.filter_menu(snapshot, palette, cx))
                    .child(
                        Button::new("list-options")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Settings2))
                            .selected(self.list_controls_open)
                            .tooltip("Sort, work states, search scope, sync")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.list_controls_open = !this.list_controls_open;
                                cx.notify();
                            })),
                    )
            )
            .when(narrow, |el| {
                el.child(div().h(px(36.)).px_6().flex().items_center().child(self.split_tabs(palette, cx, 36.)))
            })
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
                    .px_6()
                    .when(
                        self.list_controls_open || !snapshot.account_errors.is_empty(),
                        |el| el.child(self.mail_controls(snapshot, palette, cx)),
                    )
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
            .child(div().h(px(1.)).flex_shrink_0().bg(palette.border))
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
                            .px_3()
                            .py_1()
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
                    } else if self.demo {
                        format!("Demo mail · sending off · {} unread", snapshot.unread_count)
                    } else {
                        format!("{} unread", snapshot.unread_count)
                    })
                    .child(
                        h_flex()
                            .gap_3()
                            .text_color(palette.faint)
                            .child(h_flex().gap_1().child(shortcut_keys("E", palette)).child("Done"))
                            .child(h_flex().gap_1().child(shortcut_keys("R", palette)).child("Reply"))
                            .child(h_flex().gap_1().child(shortcut_keys("Ctrl K", palette)).child("Commands")),
                    )
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

    pub(crate) fn outbox_panel(
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

    pub(crate) fn links_panel(
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

    pub(crate) fn attachments_panel(
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

    pub(crate) fn message_actions(
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

    pub(crate) fn reader(
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
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .when(self.preferences.layout == preferences::PaneLayout::Full, |el| el.child(
                                Button::new("conversation-back")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(IconName::ArrowLeft))
                                    .tooltip("Back to the list · Esc")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.conversation_open = false;
                                        this.message_focus.focus(window, cx);
                                        cx.notify();
                                    })),
                            ))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(palette.muted)
                                    .child(if is_draft {
                                        "Draft".to_owned()
                                    } else {
                                        folder
                                            .map(|folder| folder.name.clone())
                                            .unwrap_or_default()
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(palette.faint)
                                    .child(self.position_label(snapshot)),
                            ),
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
                                        .tooltip("Reply · R")
                                        .accessibility_label("Reply")
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
                                        .icon(Icon::new(IconName::CornerUpLeft))
                                        .label("All")
                                        .disabled(!self.action_state(cx).allows(desktop_actions::MailboxAction::ReplyAll))
                                        .tooltip("Reply all · A")
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
                                        .tooltip("Forward · F")
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
                            .child(self.reader_mode_switch(snapshot, palette, cx))
                            .child(self.message_actions(snapshot, palette, cx))
                            .child(
                                Button::new("archive-message")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(archive_icon))
                                    .tooltip(if is_archived { "Restore to Inbox" } else { "Done · E" })
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
                                        if is_archived {
                                            this.live.update(cx, |live, cx| {
                                                live.restore_selected(cx);
                                            });
                                        } else {
                                            this.triage_selected(FolderKind::Archive, cx);
                                        }
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
                    .px_8()
                    .pt_6()
                    .pb_8()
                    .items_center()
                    .child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .max_w(px(820.))
                            .gap_4()
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
                                .text_size(px(22.))
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

    /// The conversation fills the window; it owns the same keyboard focus as
    /// the list so J/K/E/R/… keep working, and Esc goes back.
    pub(crate) fn conversation_view(&self, snapshot: &MailboxSnapshot, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("conversation-view")
            .flex_1()
            .min_w_0()
            .h_full()
            .track_focus(&self.message_focus)
            .key_context("MegaMailMessages")
            .on_action(cx.listener(|this, _: &NextMessage, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousMessage, _, cx| this.move_selection(-1, cx)))
            .child({
                let id = snapshot.selected_key.as_ref().map_or_else(
                    || "conversation".to_owned(),
                    |key| format!("conversation-{}-{}-{}", key.account_id, key.folder_path, key.uid),
                );
                self.reveal(div().size_full().child(self.reader(snapshot, palette, cx)), id, 12., 200, cx)
            })
            .into_any_element()
    }

    pub(crate) fn app_bar(&self, palette: Palette, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let layout = self.preferences.layout;
        h_flex()
            .id("app-bar")
            .w_full()
            .h(px(46.))
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_3()
            .border_b_1()
            .border_color(palette.border)
            .bg(self.pane_surface(palette.background, 0.04))
            .child(
                Button::new("toggle-folders")
                    .ghost()
                    .small()
                    .icon(Icon::new(if self.sidebar_hidden { IconName::PanelLeftOpen } else { IconName::PanelLeftClose }))
                    .tooltip("Accounts and folders · Ctrl+B")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_hidden = !this.sidebar_hidden;
                        this.sidebar_collapsed = false;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(
                Input::new(&self.search)
                    .prefix(Icon::new(IconName::Search).size(px(14.)).text_color(palette.faint))
                    .cleanable(true)
                    .aria_label("Search mail · /")
                    .w(px(460.))
                    .flex_shrink_1()
                    .min_w(px(160.)),
            )
            .child(div().flex_1())
            .child(
                Button::new("cycle-layout")
                    .ghost()
                    .small()
                    .icon(Icon::new(match layout {
                        preferences::PaneLayout::Full => IconName::PanelLeftClose,
                        preferences::PaneLayout::Right => IconName::Columns2,
                        preferences::PaneLayout::Below => IconName::Rows2,
                    }))
                    .tooltip(format!("Layout: {} · F8", layout.label()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let next = this.preferences.layout.next();
                        this.change_preference(PreferenceChange::Layout(next), cx);
                    })),
            )
            .child(
                Button::new("app-bar-subscriptions")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::MailOpen))
                    .selected(self.subscriptions.is_some())
                    .tooltip("Subscriptions: mass unsubscribe")
                    .on_click(cx.listener(|this, _, _, cx| this.open_subscriptions(cx))),
            )
            .child(
                Button::new("app-bar-settings")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::Settings))
                    .tooltip("Settings · Ctrl+,")
                    .on_click(cx.listener(|this, _, window, cx| this.open_appearance(cx))),
            )
            .child(
                Button::new("compose-new")
                    .primary()
                    .small()
                    .icon(Icon::new(IconName::PenLine))
                    .label("Compose")
                    .disabled(!self.action_state(cx).allows(desktop_actions::MailboxAction::ComposeMessage))
                    .tooltip("New message · C")
                    .on_click(cx.listener(|this, _, window, cx| this.open_new_message(window, cx))),
            )
            .into_any_element()
    }

    /// "3 of 120" for the open conversation within the list.
    pub(crate) fn position_label(&self, snapshot: &MailboxSnapshot) -> String {
        let rows: Vec<_> = self.list_rows.iter().filter(|row| !row.child).collect();
        snapshot
            .selected_key
            .as_ref()
            .and_then(|key| rows.iter().position(|row| &row.row.key == key))
            .map(|index| format!("{} of {}", index + 1, rows.len()))
            .unwrap_or_default()
    }

    /// Original / Clean / Text as one small segmented control in the toolbar.
    pub(crate) fn reader_mode_switch(&self, snapshot: &MailboxSnapshot, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let has_original = snapshot.selected_original.as_ref().is_some_and(|o| o.source.is_some());
        let current = self.reader_mode(snapshot);
        let modes: &[(ReaderMode, &str, &str)] = &[
            (ReaderMode::Original, "Original", "The sender's layout in your theme · V"),
            (ReaderMode::Clean, "Clean", "Text and structure only · V"),
            (ReaderMode::Text, "Text", "Plain text · V"),
        ];
        h_flex()
            .p(px(2.))
            .gap(px(2.))
            .mr_2()
            .rounded(px(7.))
            .bg(palette.hover)
            .children(modes.iter().filter(|(mode, _, _)| has_original || *mode != ReaderMode::Original).map(|(mode, label, tip)| {
                let mode = *mode;
                let tip = *tip;
                let selected = current == mode;
                div()
                    .id(SharedString::from(format!("reader-mode-{label}")))
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .text_size(px(11.))
                    .cursor_pointer()
                    .when(selected, |el| el.bg(palette.background).shadow_sm().text_color(palette.text))
                    .when(!selected, |el| el.text_color(palette.muted))
                    .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_reader_mode(mode, cx)))
                    .child(*label)
            }))
            .into_any_element()
    }

    /// Right-hand sender panel, like Superhuman's: who this is, whether
    /// they are who they claim, and what else they sent you.
    pub(crate) fn contact_pane(&self, snapshot: &MailboxSnapshot, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::StyledImage as _;
        let Some(row) = snapshot.selected_message.clone() else {
            return div().into_any_element();
        };
        let message = &row.message;
        let name = if message.from_name.trim().is_empty() {
            message.from_addr.clone()
        } else {
            message.from_name.trim().to_owned()
        };
        let address = message.from_addr.to_lowercase();
        let photo = sender_photos::email_key(&message.from_addr)
            .and_then(|key| self.sender_photos.images().get(&key).cloned());
        let initials: String = name
            .split_whitespace()
            .filter_map(|word| word.chars().find(|c| c.is_alphanumeric()))
            .take(2)
            .flat_map(char::to_uppercase)
            .collect();
        let domain = address.rsplit('@').next().unwrap_or("").to_owned();
        let recent: Vec<_> = self
            .list_rows
            .iter()
            .filter(|other| !other.child && other.row.message.from_addr.to_lowercase() == address)
            .filter(|other| other.row.key != row.key)
            .take(8)
            .map(|other| other.row.clone())
            .collect();
        let check = snapshot.selected_sender_check.clone();
        v_flex()
            .id("contact-pane")
            .w(px(288.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .bg(self.pane_surface(palette.background, 0.08))
            .border_l_1()
            .border_color(palette.border)
            .px_5()
            .pt_6()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .child(match photo {
                        Some(image) => gpui_kit::img(image)
                            .size(px(56.))
                            .rounded_full()
                            .object_fit(gpui_kit::ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .size(px(56.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(palette.hover)
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.muted)
                            .child(if initials.is_empty() { "?".to_owned() } else { initials })
                            .into_any_element(),
                    })
                    .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(name))
                    .child(div().text_size(px(12.)).text_color(palette.muted).child(address.clone()))
                    .when_some(check.as_ref().and_then(|check| trust_badge(check, palette)), |el, badge| {
                        el.child(div().pt_1().child(badge))
                    }),
            )
            .when(!domain.is_empty(), |el| {
                el.child(
                    v_flex()
                        .gap_1()
                        .child(div().text_size(px(11.)).text_color(palette.faint).child("DOMAIN"))
                        .child(div().text_size(px(12.)).text_color(palette.text).child(domain)),
                )
            })
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_size(px(11.)).text_color(palette.faint).child(if recent.is_empty() {
                        "NO OTHER MAIL IN THIS VIEW"
                    } else {
                        "RECENT"
                    }))
                    .children(recent.into_iter().enumerate().map(|(index, other)| {
                        let key = other.key.clone();
                        v_flex()
                            .id(("contact-recent", index))
                            .py_1p5()
                            .px_2()
                            .mx(px(-8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(palette.hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.live.update(cx, |live, cx| live.select_message(key.clone(), cx));
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(palette.text)
                                    .child(if other.message.subject.trim().is_empty() {
                                        "(No subject)".to_owned()
                                    } else {
                                        other.message.subject.clone()
                                    }),
                            )
                            .child(div().text_size(px(11.)).text_color(palette.faint).child(other.message.date.clone()))
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn conversation_content(
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
            .gap_0()
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
                let selected_here = key == selected.key;
                let cached = self.reader_bodies.iter().find(|cached| cached.key == key);
                // Only messages whose text is here open by themselves; the rest
                // stay one-line rows that load when clicked. Never a "Load" button.
                let open = (selected_here
                    || ((newest || newest_sent) && cached.is_some())
                    || self.expanded_messages.contains(&key))
                    && !self.collapsed_messages.contains(&key);
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
                let original_element = (selected_here && self.reader_mode(snapshot) == ReaderMode::Original)
                    .then(|| self.original_element(&key, palette, cx))
                    .flatten();
                let body_element: AnyElement = if let Some(element) = original_element {
                    element
                } else if let Some(cached) =
                    cached.filter(|body| !body.text.is_empty() || !body.html.is_empty())
                {
                    let target = self.reader_text_identity(snapshot).and_then(|identity| {
                        self.reader_text
                            .as_ref()
                            .filter(|target| selected_here && target.matches(&identity))
                    });
                    let state = if let Some(target) = target {
                        target.state.clone()
                    } else if plain {
                        cached.plain_state.clone().unwrap()
                    } else {
                        cached.current_state.clone().unwrap()
                    };
                    if !state.read(cx).is_parsed() {
                        // Large HTML parses in the background. Show the text at
                        // once in the same typography instead of an empty pane.
                        div()
                            .w_full()
                            .min_w_0()
                            .text_size(px(14.))
                            .line_height(relative(1.6))
                            .text_color(palette.text)
                            .child(if quote_visible {
                                cached.text.clone()
                            } else {
                                cached.without_quote.clone()
                            })
                            .into_any_element()
                    } else {
                    gpui_kit::component::text::TextView::new(&state)
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
                    }
                } else {
                    div()
                        .text_size(px(14.))
                        .line_height(relative(1.6))
                        .text_color(palette.text)
                        .child(if body.is_empty() {
                            if cached.is_some() {
                                "This message has no body.".into()
                            } else if snapshot.body_loading && selected_here {
                                if row.message.preview.trim().is_empty() {
                                    "Loading…".into()
                                } else {
                                    format!("{} …", row.message.preview.trim()).into()
                                }
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
                let sender_color = if own { palette.accent } else { palette.text };
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
                let expand_key = row.key.clone();
                let header = if !open {
                    h_flex()
                        .id(SharedString::from(format!("reader-collapsed-{}-{}-{}", row.key.account_id, row.key.folder_path, row.key.uid)))
                        .w_full()
                        .min_w_0()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.collapsed_messages.remove(&expand_key);
                            this.expanded_messages.insert(expand_key.clone());
                            this.live.update(cx, |live, cx| live.select_message(expand_key.clone(), cx));
                            let snapshot = this.live.read(cx).snapshot();
                            this.refresh_reader_text(&snapshot, cx);
                            cx.notify();
                        }))
                        .child(div().w(px(170.)).flex_shrink_0().truncate().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(sender_color).child(sender.clone()))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(palette.faint).child(row.message.preview.clone()))
                        .child(div().flex_shrink_0().text_size(px(12.)).text_color(palette.faint).child(short_date(&row.message.date)))
                        .into_any_element()
                } else {
                h_flex().w_full().min_w_0().items_center().gap_3()
                    .child(self.sender_avatar(&row.message.from_addr, &sender, sender_color, cx))
                    .child(v_flex().flex_1().min_w_0().gap_1()
                        .child(gpui_kit::component::text::TextView::html(
                            SharedString::from(format!("sender-name-{}-{}-{}", row.key.account_id, row.key.folder_path, row.key.uid)),
                            preferences::plain_text_html(&sender))
                            .selectable(true).w_full().min_w_0().text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD).text_color(sender_color))
                        .child(h_flex().w_full().min_w_0().gap_2().items_center()
                            .child(div().flex_1().min_w_0().child(gpui_kit::component::text::TextView::html(
                                SharedString::from(format!("sender-address-{}-{}-{}", row.key.account_id, row.key.folder_path, row.key.uid)),
                                preferences::plain_text_html(&row.message.from_addr))
                                .selectable(true).w_full().min_w_0().text_size(px(12.)).text_color(palette.muted)))
                            .when_some(
                                selected_here.then(|| snapshot.selected_sender_check.as_ref()).flatten().and_then(|check| trust_badge(check, palette)),
                                |el, badge| el.child(badge),
                            ))
                        )
                    .child(div().max_w(px(140.)).whitespace_normal().text_size(px(12.)).text_color(palette.muted).child(row.message.date.clone()))
                    .child(disclosure)
                    .into_any_element()
                };
                v_flex()
                    .id(SharedString::from(format!(
                        "reader-anchor-{}-{}-{}",
                        row.key.account_id, row.key.folder_path, row.key.uid
                    )))
                    .w_full()
                    .min_w_0()
                    .border_t_1()
                    .border_color(palette.border)
                    .py(px(if open { 18. } else { 10. }))
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
                                    .xsmall()
                                    // Gmail-style "to me ▾": who it went to, a click for the rest.
                                    .text_color(palette.muted)
                                    .icon(Icon::new(if self.comfort.details.contains(&row.key) {
                                        IconName::ChevronUp
                                    } else {
                                        IconName::ChevronDown
                                    }))
                                    .label({
                                        let to = row.message.to.trim();
                                        let first = to.split(',').next().unwrap_or("").trim();
                                        let first = first
                                            .split('<')
                                            .next()
                                            .map(str::trim)
                                            .filter(|name| !name.is_empty())
                                            .unwrap_or(first)
                                            .trim_matches('"');
                                        let more = to.split(',').count().saturating_sub(1);
                                        match (first.is_empty(), more) {
                                            (true, _) => "Details".to_owned(),
                                            (false, 0) => format!("to {first}"),
                                            (false, n) => format!("to {first} +{n}"),
                                        }
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
                        .when(
                            selected_here
                                && self.list_rows.iter().find(|r| r.row.key == row.key || r.conversation.as_ref().is_some_and(|t| t.members.iter().any(|m| m.key == row.key)))
                                    .is_some_and(|r| self.split_of(r) == Some(splits::Split::Screener)),
                            |el| {
                                el.child(
                                    h_flex()
                                        .w_full()
                                        .gap_2()
                                        .items_center()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(6.))
                                        .bg(palette.accent_wash)
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(px(12.))
                                                .child(format!("First email from {}. Let them into People?", row.message.from_addr)),
                                        )
                                        .child(
                                            Button::new("screener-allow")
                                                .primary()
                                                .xsmall()
                                                .label("Allow")
                                                .tooltip("Y")
                                                .on_click(cx.listener(|this, _, _, cx| this.screen_sender(true, cx))),
                                        )
                                        .child(
                                            Button::new("screener-block")
                                                .outline()
                                                .xsmall()
                                                .label("Block")
                                                .tooltip("Keep this sender out of every split · N")
                                                .on_click(cx.listener(|this, _, _, cx| this.screen_sender(false, cx))),
                                        ),
                                )
                            },
                        )
                        .when_some(
                            selected_here
                                .then(|| snapshot.selected_sender_check.as_ref())
                                .flatten()
                                .and_then(|check| trust_warning(check, palette)),
                            |el, warning| el.child(warning),
                        )
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
                            // Expanded but not downloaded yet: fetch it now and show
                            // the preview meanwhile.
                            let key = row.key.clone();
                            el.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "thread-preview-{}-{}-{}",
                                        key.account_id, key.folder_path, key.uid
                                    )))
                                    .text_size(px(13.))
                                    .text_color(palette.muted)
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.live.update(cx, |live, cx| live.select_message(key.clone(), cx));
                                    }))
                                    .child(row.message.preview.clone()),
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
}
