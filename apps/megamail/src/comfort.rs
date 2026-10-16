//! Native mail ergonomics. Identity, work status and delivery are separate signals.
use crate::workflow::{Ledger, Record, Status};
use crate::*;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) enum Sort {
    #[default]
    Date,
    Sender,
    Subject,
    Unread,
}
impl Sort {
    pub const ALL: [Self; 4] = [Self::Date, Self::Sender, Self::Subject, Self::Unread];
    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Sender => "Sender / email",
            Self::Subject => "Subject",
            Self::Unread => "Unread first",
        }
    }
}
#[derive(Default)]
pub(crate) struct State {
    pub ledger: Ledger,
    pub loaded: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub filter: Option<Status>,
    pub sort: Sort,
    pub reverse: bool,
    pub health_open: bool,
    pub last_minute: i64,
    pub outbox_open: bool,
    pub details: HashSet<MessageKey>,
    pub outgoing: Vec<OutgoingCard>,
}
#[derive(Clone)]
pub(crate) struct OutgoingCard {
    pub account: u32,
    pub request: u64,
    pub parent: String,
    pub message_id: String,
    pub body: String,
    pub to: String,
    pub status: &'static str,
}
pub(crate) fn normalized_id(id: &str) -> String {
    id.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_owned()
}
pub(crate) fn ids<'a>(messages: impl Iterator<Item = &'a Message>) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    for m in messages {
        for id in std::iter::once(m.message_id.as_str()).chain(m.references.split_whitespace()) {
            let id = normalized_id(id);
            if !id.is_empty()
                && id.len() <= 512
                && !id.chars().any(char::is_control)
                && seen.insert(id.clone())
            {
                ids.push(id);
            }
            if ids.len() == 128 {
                return ids;
            }
        }
    }
    ids
}
/// Stable colors distinguish participants; explicit labels carry the same meaning.
fn luminance(color: gpui_kit::Hsla, base: gpui_kit::Hsla) -> f32 {
    let color = color.to_rgb();
    let base = base.to_rgb();
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r * color.a + base.r * (1. - color.a))
        + 0.7152 * linear(color.g * color.a + base.g * (1. - color.a))
        + 0.0722 * linear(color.b * color.a + base.b * (1. - color.a))
}
fn identity_contrast(color: gpui_kit::Hsla, palette: Palette) -> f32 {
    let ink = luminance(color, palette.background);
    [
        palette.list,
        palette.surface,
        palette.selected,
        palette.hover,
        palette.accent_wash,
    ]
    .into_iter()
    .flat_map(|surface| {
        let background = luminance(surface, palette.background);
        let avatar = luminance(color.opacity(0.12), surface);
        [background, avatar]
    })
    .map(|background| (ink.max(background) + 0.05) / (ink.min(background) + 0.05))
    .fold(f32::INFINITY, f32::min)
}
pub(crate) fn identity_color(address: &str, palette: Palette) -> gpui_kit::Hsla {
    let hash = address
        .trim()
        .to_lowercase()
        .bytes()
        .fold(2166136261u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(16777619)
        });
    let dark = palette.background.l < 0.5;
    let mut color = gpui_kit::hsla(
        (hash % 360) as f32 / 360.,
        if dark { 0.63 } else { 0.72 },
        if dark { 0.76 } else { 0.33 },
        1.,
    );
    // Match the actual selected/hover/reader surfaces, including avatar tint.
    for _ in 0..=100 {
        if identity_contrast(color, palette) >= 4.6 {
            break;
        }
        color.l = (color.l + if dark { 0.01 } else { -0.01 }).clamp(0., 1.);
    }
    color
}
pub(crate) fn own_label(role: Option<FolderKind>) -> &'static str {
    match role {
        Some(FolderKind::Drafts) => "You · draft",
        Some(FolderKind::Sent) => "You · sent",
        _ => "You",
    }
}
pub(crate) fn inferred_work(own: bool, role: Option<FolderKind>) -> Status {
    if own && role == Some(FolderKind::Sent) {
        Status::Waiting
    } else {
        Status::Review
    }
}
pub(crate) fn latest(row: &ListRow) -> &Message {
    row.conversation
        .as_ref()
        .and_then(|t| t.members.last())
        .map_or(&row.row.message, |r| &r.message)
}

impl MailApp {
    pub(crate) fn apply_mail_sort(&mut self, cx: &mut Context<Self>) {
        use megamail_core::cache::{PageOrder, SortKey};
        let key = match self.comfort.sort {
            Sort::Date => SortKey::Date,
            Sort::Sender => SortKey::Sender,
            Sort::Subject => SortKey::Subject,
            Sort::Unread => SortKey::Unread,
        };
        let order = PageOrder {
            key,
            reverse: self.comfort.reverse,
        };
        self.live.update(cx, |live, cx| live.set_order(order, cx));
    }

    pub(crate) fn own_message(&self, message: &Message, snapshot: &MailboxSnapshot) -> bool {
        let email = message.from_addr.trim();
        snapshot
            .accounts
            .iter()
            .any(|a| a.id == message.account_id && a.email.eq_ignore_ascii_case(email))
            || self.profiles.iter().any(|(id, p)| {
                *id == message.account_id
                    && (p.email.eq_ignore_ascii_case(email)
                        || p.aliases
                            .iter()
                            .any(|a| a.address().eq_ignore_ascii_case(email)))
            })
    }
    pub(crate) fn work_identity(
        &self,
        thread: Option<&ConversationRow>,
        row: &MessageRow,
        snapshot: &MailboxSnapshot,
    ) -> (Vec<String>, i64, bool) {
        let rows: Vec<&MessageRow> = thread.map_or_else(
            || vec![row],
            |t| t.members.iter().map(|m| m.as_ref()).collect(),
        );
        let role = |row: &MessageRow| {
            snapshot
                .folder_kinds
                .get(&(row.key.account_id, row.message.folder_id))
                .copied()
        };
        let active = || {
            rows.iter().copied().filter(|row| {
                !matches!(role(row), Some(FolderKind::Drafts | FolderKind::Templates))
            })
        };
        let incoming = active()
            .filter(|row| !self.own_message(&row.message, snapshot))
            .map(|row| row.message.timestamp)
            .max()
            .unwrap_or(0);
        let own_sent_latest = active()
            .max_by_key(|row| row.message.timestamp)
            .is_some_and(|row| {
                inferred_work(self.own_message(&row.message, snapshot), role(row))
                    == Status::Waiting
            });
        (
            ids(rows.iter().map(|row| &row.message)),
            incoming,
            own_sent_latest,
        )
    }
    pub(crate) fn work_status(
        &self,
        thread: Option<&ConversationRow>,
        row: &MessageRow,
        snapshot: &MailboxSnapshot,
    ) -> Status {
        let (ids, incoming, own_latest) = self.work_identity(thread, row, snapshot);
        self.comfort.ledger.find(row.key.account_id, &ids).map_or(
            if own_latest {
                Status::Waiting
            } else {
                Status::Review
            },
            |r| r.effective(incoming, megamail_core::datefmt::now()),
        )
    }
    pub(crate) fn mark_work(&mut self, status: Status, cx: &mut Context<Self>) {
        if self.comfort.busy || !self.comfort.loaded {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(row) = snapshot.selected_message.as_ref() else {
            return;
        };
        let thread = snapshot
            .conversations
            .iter()
            .find(|t| t.members.iter().any(|m| m.key == row.key));
        let (ids, incoming, _) = self.work_identity(thread, row, &snapshot);
        let now = megamail_core::datefmt::now();
        let record = Record {
            account: row.key.account_id,
            ids,
            status,
            observed_inbound: incoming,
            snooze_until: (status == Status::Snoozed).then_some(now + 86400),
            updated: now,
        };
        let next = match self.comfort.ledger.with_record(record) {
            Ok(next) => next,
            Err(error) => {
                self.comfort.error = Some(error);
                cx.notify();
                return;
            }
        };
        if self.demo {
            self.comfort.ledger = next;
            self.rebuild_list_rows(&snapshot, cx);
            cx.notify();
            return;
        }
        self.comfort.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { next.save().map(|_| next) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.comfort.busy = false;
                match result {
                    Ok(next) => {
                        this.comfort.ledger = next;
                        this.comfort.error = None;
                    }
                    Err(error) => this.comfort.error = Some(error),
                }
                let snapshot = this.live.read(cx).snapshot();
                this.rebuild_list_rows(&snapshot, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn work_controls(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = &snapshot.selected_message else {
            return div().into_any_element();
        };
        let thread = snapshot
            .conversations
            .iter()
            .find(|t| t.members.iter().any(|m| m.key == row.key));
        let status = self.work_status(thread, row, snapshot);
        let (ids, _, suggested_waiting) = self.work_identity(thread, row, snapshot);
        let saved = self.comfort.ledger.find(row.key.account_id, &ids).is_some();
        let feedback = if self.comfort.busy {
            "Saving on this device…"
        } else if self.comfort.error.is_some() {
            "Work state unavailable · see details below"
        } else if !self.comfort.loaded {
            "Loading local work states…"
        } else if self.demo {
            "Demo · session only"
        } else if saved {
            "Your work · saved on this device"
        } else if suggested_waiting {
            "Suggested from Sent · choose a work state"
        } else {
            "Choose a work state · independent of unread"
        };
        let owner = cx.entity().downgrade();
        let label = if self.comfort.busy {
            "Saving…"
        } else {
            status.label()
        };
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("conversation-work-state")
                            .secondary()
                            .small()
                            .label(label)
                            .disabled(!self.comfort.loaded || self.comfort.busy)
                            .tooltip("Local work state, independent of read/unread and delivery")
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu;
                                for state in Status::ALL {
                                    let owner = owner.clone();
                                    menu =
                                        menu.item(
                                            PopupMenuItem::element(move |_, _| {
                                                div().child(if state == Status::Snoozed {
                                                    "Snooze for 1 day"
                                                } else {
                                                    state.label()
                                                })
                                            })
                                            .on_click(move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.mark_work(state, cx)
                                                });
                                            }),
                                        );
                                }
                                menu
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .child(feedback),
                    )
                    .child(
                        Button::new("expand-conversation")
                            .ghost()
                            .small()
                            .label("Expand all")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let s = this.live.read(cx).snapshot();
                                for t in s.conversations.iter().filter(|t| {
                                    t.members
                                        .iter()
                                        .any(|r| Some(&r.key) == s.selected_key.as_ref())
                                }) {
                                    for r in t.members.iter() {
                                        this.collapsed_messages.remove(&r.key);
                                        this.expanded_messages.insert(r.key.clone());
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("collapse-conversation")
                            .ghost()
                            .small()
                            .label("Collapse history")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.expanded_messages.clear();
                                this.collapsed_messages.clear();
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.comfort.error.clone(), |el, error| {
                el.child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.text)
                        .child(error),
                )
            })
            .into_any_element()
    }
    pub(crate) fn mail_controls(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let work_owner = owner.clone();
        let scope_owner = owner.clone();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("mail-sort")
                            .ghost()
                            .small()
                            .label(format!("Sort: {}", self.comfort.sort.label()))
                            .tooltip(
                                "Direct accounts sort the local cache before paging. Legacy adapters sort loaded headers.",
                            )
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu;
                                for sort in Sort::ALL {
                                    let owner = owner.clone();
                                    menu =
                                        menu.item(
                                            PopupMenuItem::element(move |_, _| {
                                                div().child(sort.label())
                                            })
                                            .on_click(move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.comfort.sort = sort;
                                                    this.apply_mail_sort(cx);
                                                    this.rebuild_list_rows(
                                                        &this.live.read(cx).snapshot(),
                                                        cx,
                                                    );
                                                    cx.notify();
                                                });
                                            }),
                                        );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("reverse-sort")
                            .ghost()
                            .small()
                            .label(if self.comfort.reverse {
                                "Reverse"
                            } else {
                                "Default order"
                            })
                            .tooltip("Reverse the current sort order")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.comfort.reverse = !this.comfort.reverse;
                                this.apply_mail_sort(cx);
                                this.rebuild_list_rows(&this.live.read(cx).snapshot(), cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("work-filter")
                            .ghost()
                            .small()
                            .label(self.comfort.filter.map_or("All work states", Status::label))
                            .dropdown_menu(move |menu, _, _| {
                                let clear = work_owner.clone();
                                let mut menu = menu.item(
                                    PopupMenuItem::element(|_, _| div().child("All work states"))
                                        .on_click(move |_, _, cx| {
                                            let _ = clear.update(cx, |this, cx| {
                                                this.comfort.filter = None;
                                                this.rebuild_list_rows(
                                                    &this.live.read(cx).snapshot(),
                                                    cx,
                                                );
                                                cx.notify();
                                            });
                                        }),
                                );
                                for status in Status::ALL {
                                    let owner = work_owner.clone();
                                    menu =
                                        menu.item(
                                            PopupMenuItem::element(move |_, _| {
                                                div().child(status.label())
                                            })
                                            .on_click(move |_, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.comfort.filter = Some(status);
                                                    this.rebuild_list_rows(
                                                        &this.live.read(cx).snapshot(),
                                                        cx,
                                                    );
                                                    cx.notify();
                                                });
                                            }),
                                        );
                                }
                                menu
                            }),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("search-scope")
                            .ghost()
                            .small()
                            .label("Search scope")
                            .dropdown_menu(move |menu, _, _| {
                                let owner = scope_owner.clone();
                                let other = scope_owner.clone();
                                menu.item(
                                    PopupMenuItem::element(|_, _| {
                                        div().child("All inboxes · every account")
                                    })
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.comfort.filter = None;
                                                this.live
                                                    .update(cx, |live, cx| live.select_unified(cx));
                                            });
                                        },
                                    ),
                                )
                                .item(
                                    PopupMenuItem::element(|_, _| {
                                        div().child("All local mail · every account")
                                    })
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = other.update(cx, |this, cx| {
                                                this.comfort.filter = None;
                                                this.live.update(cx, |live, cx| {
                                                    live.select_unified_kind(None, cx)
                                                });
                                            });
                                        },
                                    ),
                                )
                            }),
                    )
                    .child(
                        Button::new("connection-health")
                            .ghost()
                            .small()
                            .label(if snapshot.account_errors.is_empty() {
                                "Sync details"
                            } else {
                                "Connection needs attention"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.comfort.health_open = !this.comfort.health_open;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().text_size(px(11.)).text_color(palette.muted).child(
                if snapshot.is_thunderbird_account {
                    "Search: loaded headers. Try from: or subject:"
                } else {
                    "Search: cached mail in this view. body: covers downloaded bodies only."
                },
            ))
            .when(self.comfort.health_open, |el| {
                el.child(self.sync_details(snapshot, palette, cx))
            })
            .into_any_element()
    }
    fn sync_details(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex().gap_2().text_size(px(11.)).text_color(palette.muted)
            .children(snapshot.accounts.iter().map(|account| {
                let state = if let Some(error) = snapshot.account_errors.get(&account.id) { format!("Needs attention: {error}") } else if snapshot.account_loading.get(&account.id).copied().unwrap_or(false) { "Loading this view…".into() } else { snapshot.last_synced.get(&account.id).map_or_else(|| "No completed folder sync this session".into(), |at| format!("Folder sync: {}", megamail_core::datefmt::editable_send_time(*at).unwrap_or_default())) };
                v_flex().child(div().font_weight(FontWeight::MEDIUM).text_color(palette.text).child(account.email.clone())).child(state)
            }))
            .child("Cached pages can remain readable while an account reconnects. Sync time is not a delivery receipt.")
            .child(Button::new("health-refresh").secondary().small().label("Refresh mail").on_click(cx.listener(|this, _, _, cx| { this.live.update(cx, |live, cx| live.refresh(cx)); })))
            .into_any_element()
    }
    pub(crate) fn shared_folders(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .gap_1()
            .children(
                [
                    (Some(FolderKind::Sent), "All sent", IconName::Mail),
                    (Some(FolderKind::Drafts), "All drafts", IconName::FileText),
                    (Some(FolderKind::Archive), "All archived", IconName::Archive),
                    (None, "All local mail", IconName::Search),
                ]
                .into_iter()
                .map(|(kind, label, icon)| {
                    Button::new(SharedString::from(format!("shared-folder-{label}")))
                        .ghost()
                        .small()
                        .w_full()
                        .justify_start()
                        .icon(Icon::new(icon).size(px(14.)))
                        .label(label)
                        .selected(
                            snapshot.scope == MailboxScope::Unified
                                && snapshot.unified_kind == kind,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.mailbox_focus.focus(window, cx);
                            this.comfort.filter = None;
                            this.comfort.outbox_open = false;
                            this.live
                                .update(cx, |live, cx| live.select_unified_kind(kind, cx));
                            this.view = View::Mailbox;
                            this.list_scroll = UniformListScrollHandle::new();
                            cx.notify();
                        }))
                }),
            )
            .child(
                Button::new("all-outbox")
                    .ghost()
                    .small()
                    .w_full()
                    .justify_start()
                    .icon(Icon::new(IconName::Mail).size(px(14.)))
                    .label(format!("Outbox · {}", snapshot.outbox.len()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.mailbox_focus.focus(window, cx);
                        this.comfort.outbox_open = true;
                        this.view = View::Mailbox;
                        cx.notify();
                    })),
            )
            .when(self.composer.is_some(), |el| {
                el.child(
                    Button::new("resume-retained-draft")
                        .secondary()
                        .small()
                        .label("Resume your draft")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.view = View::Compose;
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
    pub(crate) fn outgoing_cards(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
    ) -> AnyElement {
        let Some(selected) = &snapshot.selected_message else {
            return div().into_any_element();
        };
        let thread = snapshot
            .conversations
            .iter()
            .find(|t| t.members.iter().any(|m| m.key == selected.key));
        let (thread_ids, _, _) = self.work_identity(thread, selected, snapshot);
        v_flex()
            .gap_3()
            .children(
                self.comfort
                    .outgoing
                    .iter()
                    .filter(|o| {
                        o.account == selected.key.account_id && thread_ids.contains(&o.parent)
                    })
                    .map(|o| {
                        v_flex()
                            .gap_2()
                            .p_4()
                            .rounded(px(8.))
                            .bg(palette.accent_wash)
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(palette.accent)
                                            .child("You"),
                                    )
                                    .child(div().text_size(px(12.)).child(o.status)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(palette.muted)
                                    .child(format!("To {}", o.to)),
                            )
                            .child(
                                gpui_kit::component::text::TextView::html(
                                    SharedString::from(format!(
                                        "outgoing-{}-{}",
                                        o.account, o.request
                                    )),
                                    preferences::plain_text_html(&o.body),
                                )
                                .selectable(true)
                                .text_size(px(14.))
                                .line_height(relative(1.6))
                                .w_full(),
                            )
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod comfort_tests {
    use super::*;
    #[test]
    fn ownership_never_turns_an_unsent_draft_into_a_sent_or_waiting_message() {
        assert_eq!(own_label(Some(FolderKind::Drafts)), "You · draft");
        assert_eq!(
            inferred_work(true, Some(FolderKind::Drafts)),
            Status::Review
        );
        assert_eq!(
            inferred_work(true, Some(FolderKind::Templates)),
            Status::Review
        );
        assert_eq!(inferred_work(true, None), Status::Review);
        assert_eq!(inferred_work(true, Some(FolderKind::Inbox)), Status::Review);
        assert_eq!(inferred_work(true, Some(FolderKind::Sent)), Status::Waiting);
        assert_eq!(inferred_work(false, Some(FolderKind::Sent)), Status::Review);
    }
    #[test]
    fn participant_colors_remain_readable_on_reader_selected_and_avatar_surfaces() {
        for dark in [false, true] {
            let mut palette = Palette::new(dark);
            palette.surface = gpui_kit::rgb(if dark { 0x343437 } else { 0xececee }).into();
            for i in 0..1500 {
                let color = identity_color(&format!("participant{i}@example.test"), palette);
                assert!(
                    identity_contrast(color, palette) >= 4.5,
                    "participant {i}, dark={dark}"
                );
            }
        }
    }
}
