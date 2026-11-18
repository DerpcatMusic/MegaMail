//! Composer screen: layout, send, drafts and attachments.
use crate::*;

impl MailApp {
    pub(crate) fn composer_view(
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
            .capture_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                let keys = &event.keystroke;
                let ctrl = keys.modifiers.control || keys.modifiers.platform;
                match (ctrl, keys.key.as_str()) {
                    (true, "enter") => this.send_current_message(cx),
                    (true, "s") => this.save_current_draft(cx),
                    (true, "b") => this.format_body(compose_markdown::Mark::Bold, window, cx),
                    (true, "i") => this.format_body(compose_markdown::Mark::Italic, window, cx),
                    (true, "k") => this.format_body(compose_markdown::Mark::Link, window, cx),
                    (false, "escape") => {
                        this.capture_recovery(cx);
                        this.view = View::Mailbox;
                        if this.list_rows.is_empty() {
                            this.mailbox_focus.focus(window, cx);
                        } else {
                            this.message_focus.focus(window, cx);
                        }
                        cx.notify();
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
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
                    .child(h_flex().items_center().gap_1().child(Button::new("browse-while-composing").ghost().small().label("Back to mail").tooltip("Keep this draft open and return to mail · Esc").on_click(cx.listener(|this, _, window, cx| { this.capture_recovery(cx); this.mailbox_focus.focus(window, cx); this.view = View::Mailbox; cx.notify(); })))
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
                    )),
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
                            .gap_1()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_end()
                                    .gap_2()
                                    .child(div().flex_1().min_w_0().child(inline_field("To", Input::new(&composer.to).appearance(false).aria_label("To").disabled(!composer.can_edit()), palette)))
                                    .when(!composer.show_cc, |row| {
                                        row.child(
                                            Button::new("show-cc-bcc")
                                                .ghost()
                                                .xsmall()
                                                .label("Cc Bcc")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(c) = this.composer.as_mut() {
                                                        c.show_cc = true;
                                                    }
                                                    cx.notify();
                                                })),
                                        )
                                    }),
                            )
                            .when(composer.show_cc, |el| {
                                el.child(inline_field("Cc", Input::new(&composer.cc).appearance(false).aria_label("Cc").disabled(!composer.can_edit()), palette))
                                .child(inline_field("Bcc", Input::new(&composer.bcc).appearance(false).aria_label("Bcc").disabled(!composer.can_edit()), palette))
                            })
                            .when(composer.show_more, |el| el.child(self.compose_tools_view(palette, cx)))
                            .when(composer.show_more && self.live.read(cx).schedule_session(composer.message.from_account_id).is_some(), |el| el
                                .child(labeled_input("Send Later", Input::new(&composer.schedule).aria_label("Scheduled date, time and numeric UTC offset").disabled(!composer.can_edit()), palette))
                                .child(h_flex().flex_wrap().gap_2()
                                    .child(Button::new("apply-send-time").ghost().small().label("Set time").disabled(!composer.can_edit()).on_click(cx.listener(|this, _, window, cx| this.set_compose_schedule(false, window, cx))))
                                    .child(Button::new("clear-send-time").ghost().small().label("Send now instead").disabled(!composer.can_edit() || composer.message.send_at.is_none()).on_click(cx.listener(|this, _, window, cx| this.set_compose_schedule(true, window, cx)))))
                                .child(div().text_size(px(11.)).text_color(palette.muted).child("Local scheduling requires MegaMail running and connected. When closed, delivery resumes after the next launch.")))
                            .child(inline_field("Subject", Input::new(&composer.subject).appearance(false).aria_label("Subject").disabled(!composer.can_edit()), palette))
                            .child(self.format_toolbar(palette, cx))
                            .child(
                                Textarea::new(&composer.body)
                                    .h(px(320.))
                                    .appearance(false)
                                    .aria_label("Message body")
                                    .disabled(!composer.can_edit()),
                            )
                            .children(self.format_preview(palette))
                            .when_some(composer.quoted.clone(), |el, quoted| {
                                el.child(
                                    Button::new("toggle-quoted")
                                        .ghost()
                                        .xsmall()
                                        .self_start()
                                        .label(if composer.show_quoted { "Hide quoted text" } else { "•••" })
                                        .tooltip("Quoted original, sent below your reply")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(c) = this.composer.as_mut() {
                                                c.show_quoted = !c.show_quoted;
                                            }
                                            cx.notify();
                                        })),
                                )
                                .when(composer.show_quoted, |el| {
                                    el.child(
                                        div()
                                            .w_full()
                                            .pl_3()
                                            .border_l_2()
                                            .border_color(palette.border)
                                            .text_size(px(12.))
                                            .line_height(relative(1.5))
                                            .text_color(palette.muted)
                                            .child(quoted),
                                    )
                                })
                            })
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
                            .when(composer.show_more, |el| el.child(div().text_size(px(11.)).text_color(palette.faint).child(composer.recovery.status.clone())))
                            .when_some(self.recovery_error.clone(), |el, error| el.child(error_banner("Local recovery", &error, cx, palette)))
                            .when(self.recovery_error.is_some(), |el| el.child(Button::new("retry-local-recovery").ghost().small().label(if composer.sent_complete { "Retry recovery cleanup" } else { "Retry local recovery" }).disabled(composer.operation_pending() || if composer.sent_complete { composer.recovery.cleanup_in_flight || composer.recovery.retired == Some(composer.recovery.generation) } else { composer.recovery.in_flight.is_some() || composer.recovery.acknowledged == composer.recovery.generation }).on_click(cx.listener(|this, _, _, cx| this.retry_native_recovery(cx)))))
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
                                h_flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new("add-attachments")
                                            .ghost()
                                            .small()
                                            .icon(Icon::new(IconName::Paperclip))
                                            .tooltip(if composer.selecting_attachments { "Choosing files…" } else { "Attach files" })
                                            .disabled(!composer.can_edit())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.choose_attachments(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("composer-more")
                                            .ghost()
                                            .small()
                                            .label(if composer.show_more { "Less" } else { "More" })
                                            .tooltip("Formatting, contacts, snippets and Send Later")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(c) = this.composer.as_mut() {
                                                    c.show_more = !c.show_more;
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(palette.faint)
                                            .child(if composer.markup.enabled { "" } else if !composer.message.html.is_empty() { "Original HTML" } else { "" }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("save-draft")
                                            .ghost()
                                            .small()
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
                                        Button::new("send-individually")
                                            .ghost()
                                            .small()
                                            .label("Individually")
                                            .selected(composer.individually)
                                            .tooltip("One private copy per To recipient. Use {first_name}, {name} or {email} in the subject and body. Paste a list one per line.")
                                            .disabled(!composer.can_edit())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(c) = this.composer.as_mut() {
                                                    c.individually = !c.individually;
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("send-message")
                                            .primary()
                                            .small()
                                            .icon(Icon::new(IconName::Send))
                                            .tooltip("Send · Ctrl+Enter")
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

    pub(crate) fn send_current_message(&mut self, cx: &mut Context<Self>) {
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
        if composer.markup.enabled && composer.markup.ready_source != composer.body.read(cx).value().as_ref() {
            // Typed faster than the debounced preview: convert now (fast, bounded).
            let source = composer.body.read(cx).value().to_string();
            if source.len() > 2 * 1024 * 1024 {
                composer.error = Some("Formatted messages are limited to 2 MiB. Switch to Plain text.".into());
                cx.notify();
                return;
            }
            composer.markup.html = megamail_core::markdown::to_html(&source);
            composer.markup.ready_source = source;
            composer.markup.busy = false;
            composer.markup.error = None;
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
        if composer.individually {
            let copies = batch_send::split(&message);
            if copies.is_empty() {
                composer.error = Some("No valid recipient addresses in To.".into());
                cx.notify();
                return;
            }
            // ponytail: no Undo for batches and the composer tracks only the last
            // request; earlier failures surface in the Outbox and account errors.
            let mut last = None;
            for copy in copies {
                last = self.live.update(cx, |live, cx| live.send_message(copy, cx)).or(last);
            }
            if let (Some(request_id), Some(composer)) = (last, self.composer.as_mut()) {
                composer.send_request_id = Some(request_id);
                composer.send_generation = Some(composer.recovery.generation);
            }
            cx.notify();
            return;
        }
        // Undo Send: an immediate send waits a few seconds in the Outbox,
        // where Undo can still cancel it (direct IMAP accounts).
        let mut message = message;
        let undo_original = (message.send_at.is_none()
            && self.live.read(cx).schedule_session(message.from_account_id).is_some())
        .then(|| {
            let original = message.clone();
            message.send_at = Some(megamail_core::datefmt::now() + UNDO_SEND_SECONDS);
            original
        });
        let undo_at = message.send_at;
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
        if let (Some(request_id), Some(original), Some(send_at)) = (request_id, undo_original, undo_at) {
            self.undo_send = Some(PendingUndo {
                account: original.from_account_id,
                send_at,
                request_id,
                original,
            });
            self.tick_undo_send(cx);
        }
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

    pub(crate) fn save_current_draft(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn choose_attachments(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn save_reader_attachment(
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

/// One Superhuman-style header row: a quiet label, a borderless input, a hairline.
fn inline_field(label: &'static str, input: impl IntoElement, palette: Palette) -> impl IntoElement {
    h_flex()
        .w_full()
        .min_w_0()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(palette.border)
        .child(div().w(px(56.)).flex_shrink_0().text_size(px(12.)).text_color(palette.muted).child(label))
        .child(div().flex_1().min_w_0().child(input))
}
