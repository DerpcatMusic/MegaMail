//! Mass unsubscriber: every sender that mails on repeat, loudest first, with
//! one button per row and one for the whole selection. One-click (RFC 8058)
//! is an HTTPS POST, email routes send a short message from the account the
//! mail arrived in, web routes open the page. Kept / done / deferred senders
//! persist in `subscriptions.v1.json` beside the screener file.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::*;
use megamail_core::models::Unsubscribe;

/// How far back the sender scan looks; rates are per month over this window.
const WINDOW_DAYS: i64 = 90;
const DEFER_DAYS: i64 = 30;
const FILE: &str = "subscriptions.v1.json";
const MAX_BYTES: u64 = 2 * 1024 * 1024;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// What the user decided per sender address (lowercase).
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Decisions {
    pub kept: HashSet<String>,
    /// Unsubscribed, with the time it happened.
    pub done: HashMap<String, i64>,
    /// Hidden until this time.
    pub deferred: HashMap<String, i64>,
}

impl Decisions {
    pub fn load() -> Self {
        use std::io::Read as _;
        let Some(path) = megamail_core::config::config_base().map(|dir| dir.join(FILE)) else {
            return Self::default();
        };
        let mut bytes = Vec::new();
        match std::fs::File::open(path).and_then(|file| file.take(MAX_BYTES).read_to_end(&mut bytes)) {
            Ok(_) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = megamail_core::config::config_base().ok_or("Subscription storage is unavailable.")?;
        let json = serde_json::to_string(self).map_err(|_| "Subscription decisions could not be encoded.")?;
        megamail_core::config::write_private_file(&dir.join(FILE), &json)
            .map_err(|_| "Subscription decisions could not be saved.".into())
    }

    fn hidden(&self, address: &str, now: i64) -> bool {
        self.kept.contains(address)
            || self.done.contains_key(address)
            || self.deferred.get(address).is_some_and(|until| *until > now)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Method {
    OneClick,
    Email,
    Web,
    None,
}

impl Method {
    fn label(self) -> &'static str {
        match self {
            Method::OneClick => "One-click",
            Method::Email => "Email",
            Method::Web => "Web",
            Method::None => "None",
        }
    }
}

#[derive(Clone)]
pub(crate) struct Sender {
    pub address: String,
    pub name: String,
    pub account_id: u32,
    pub per_month: f32,
    pub last_ts: i64,
    pub route: Option<Unsubscribe>,
}

impl Sender {
    fn one_click_url(&self) -> Option<&str> {
        let route = self.route.as_ref()?;
        route
            .one_click
            .as_deref()
            .filter(|url| route.in_headers && url.starts_with("https://") && megamail_core::mail_text::safe_browser_url(url))
    }

    fn web_url(&self) -> Option<&str> {
        self.route.as_ref()?.web.as_deref().filter(|url| megamail_core::mail_text::safe_browser_url(url))
    }

    pub fn method(&self) -> Method {
        let Some(route) = &self.route else { return Method::None };
        if self.one_click_url().is_some() {
            Method::OneClick
        } else if route.mailto.is_some() || route.reply.is_some() {
            Method::Email
        } else if self.web_url().is_some() {
            Method::Web
        } else {
            Method::None
        }
    }

    fn display(&self) -> &str {
        if self.name.is_empty() { &self.address } else { &self.name }
    }
}

fn senders_from(rows: Vec<megamail_core::cache::SubscriptionSender>) -> Vec<Sender> {
    let months = WINDOW_DAYS as f32 / 30.0;
    let mut senders: Vec<Sender> = rows
        .into_iter()
        .map(|row| Sender {
            address: row.address,
            name: row.name,
            account_id: row.account_id,
            per_month: row.count as f32 / months,
            last_ts: row.last_ts,
            route: row.unsubscribe.and_then(|json| serde_json::from_str(&json).ok()),
        })
        .collect();
    // Unsubscribable senders first, then loudest.
    senders.sort_by(|a, b| {
        (a.method() == Method::None)
            .cmp(&(b.method() == Method::None))
            .then(b.per_month.total_cmp(&a.per_month))
    });
    senders
}

#[derive(Default)]
pub(crate) struct Screen {
    pub loading: bool,
    pub senders: Vec<Sender>,
    pub decisions: Decisions,
    pub selected: HashSet<String>,
    pub working: HashSet<String>,
    pub status: Option<String>,
}

impl Screen {
    fn visible(&self) -> Vec<&Sender> {
        let now = now();
        self.senders.iter().filter(|s| !self.decisions.hidden(&s.address, now)).collect()
    }
}

impl MailApp {
    pub(crate) fn open_subscriptions(&mut self, cx: &mut Context<Self>) {
        if self.subscriptions.is_some() {
            self.subscriptions = None;
            cx.notify();
            return;
        }
        self.subscriptions = Some(Screen { loading: true, ..Default::default() });
        cx.notify();
        let since = now() - WINDOW_DAYS * 86_400;
        cx.spawn(async move |this, cx| {
            let (senders, decisions) = cx
                .background_spawn(async move {
                    let rows = megamail_core::cache::Cache::open()
                        .map(|cache| cache.subscription_senders(since))
                        .unwrap_or_default();
                    (senders_from(rows), Decisions::load())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(screen) = this.subscriptions.as_mut() {
                    screen.loading = false;
                    screen.senders = senders;
                    screen.decisions = decisions;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_subscription_decisions(&mut self) {
        if let Some(screen) = self.subscriptions.as_mut()
            && let Err(error) = screen.decisions.save()
        {
            screen.status = Some(error);
        }
    }

    fn decide_subscription(&mut self, address: &str, keep: bool, cx: &mut Context<Self>) {
        let Some(screen) = self.subscriptions.as_mut() else { return };
        screen.selected.remove(address);
        if keep {
            screen.decisions.kept.insert(address.to_owned());
        } else {
            screen.decisions.deferred.insert(address.to_owned(), now() + DEFER_DAYS * 86_400);
        }
        self.save_subscription_decisions();
        cx.notify();
    }

    fn finish_unsubscribe(&mut self, address: &str, ok: bool, cx: &mut Context<Self>) {
        let Some(screen) = self.subscriptions.as_mut() else { return };
        screen.working.remove(address);
        if ok {
            screen.selected.remove(address);
            screen.decisions.done.insert(address.to_owned(), now());
            self.save_subscription_decisions();
        } else if let Some(screen) = self.subscriptions.as_mut() {
            screen.status = Some(format!("Could not unsubscribe from {address}. Try its webpage."));
        }
        cx.notify();
    }

    /// Unsubscribe from every listed address. Web-only senders open their
    /// page when there are at most three of them, so a batch never sprays tabs.
    pub(crate) fn unsubscribe_senders(&mut self, addresses: Vec<String>, cx: &mut Context<Self>) {
        let Some(screen) = self.subscriptions.as_ref() else { return };
        let targets: Vec<Sender> = screen
            .senders
            .iter()
            .filter(|s| addresses.contains(&s.address) && !screen.working.contains(&s.address))
            .cloned()
            .collect();
        let web_count = targets.iter().filter(|s| s.method() == Method::Web).count();
        let (mut started, mut skipped) = (0, 0);
        for sender in targets {
            match sender.method() {
                Method::OneClick if !self.demo => {
                    let url = sender.one_click_url().unwrap_or_default().to_owned();
                    let address = sender.address.clone();
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
                        let _ = this.update(cx, |this, cx| this.finish_unsubscribe(&address, ok, cx));
                    })
                    .detach();
                    if let Some(screen) = self.subscriptions.as_mut() {
                        screen.working.insert(sender.address.clone());
                    }
                    started += 1;
                }
                Method::Email if !self.demo => {
                    let route = sender.route.clone().unwrap_or_default();
                    let sent = newsletter_actions::unsubscribe_intent(&route).ok().and_then(|mut intent| {
                        if intent.subject.trim().is_empty() {
                            intent.subject = "unsubscribe".into();
                        }
                        let message = desktop_actions::compose_message(intent, sender.account_id);
                        self.live.update(cx, |live, cx| live.send_message(message, cx))
                    });
                    self.finish_unsubscribe(&sender.address, sent.is_some(), cx);
                    started += 1;
                }
                Method::Web if web_count <= 3 => {
                    if let Some(url) = sender.web_url() {
                        cx.open_url(url);
                    }
                    self.finish_unsubscribe(&sender.address, true, cx);
                    started += 1;
                }
                _ => skipped += 1,
            }
        }
        if let Some(screen) = self.subscriptions.as_mut() {
            screen.status = Some(match (started, skipped, self.demo) {
                (_, _, true) => "Unsubscribing is disabled in the local demo.".into(),
                (n, 0, _) => format!("Unsubscribing from {n}…"),
                (n, s, _) => format!("Unsubscribing from {n}. {s} need their webpage or have no route; open them one at a time."),
            });
        }
        cx.notify();
    }

    pub(crate) fn subscriptions_view(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(screen) = self.subscriptions.as_ref() else { return div().into_any_element() };
        let visible = screen.visible();
        let selected: Vec<String> =
            visible.iter().filter(|s| screen.selected.contains(&s.address)).map(|s| s.address.clone()).collect();
        let all_selected = !visible.is_empty() && selected.len() == visible.len();
        let all: Vec<String> = visible.iter().map(|s| s.address.clone()).collect();
        let total_per_month: f32 = visible.iter().map(|s| s.per_month).sum();

        let header = h_flex()
            .h(px(52.))
            .flex_shrink_0()
            .items_center()
            .gap_3()
            .px_6()
            .child(
                Button::new("subs-back")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::ArrowLeft))
                    .tooltip("Back to mail · Esc")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.subscriptions = None;
                        cx.notify();
                    })),
            )
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("Subscriptions"))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.muted)
                    .child(format!("{} senders · ~{:.0} emails a month", visible.len(), total_per_month)),
            )
            .child(div().flex_1())
            .when_some(screen.status.clone(), |el, status| {
                el.child(div().max_w(px(420.)).truncate().text_size(px(12.)).text_color(palette.muted).child(status))
            })
            .child(
                Button::new("subs-select-all")
                    .ghost()
                    .small()
                    .label(if all_selected { "Select none" } else { "Select all" })
                    .disabled(visible.is_empty())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(screen) = this.subscriptions.as_mut() {
                            if all_selected {
                                screen.selected.clear();
                            } else {
                                screen.selected.extend(all.iter().cloned());
                            }
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("subs-batch")
                    .primary()
                    .small()
                    .label(format!("Unsubscribe from {}", selected.len()))
                    .disabled(selected.is_empty())
                    .on_click(cx.listener(move |this, _, _, cx| this.unsubscribe_senders(selected.clone(), cx))),
            );

        let body: AnyElement = if screen.loading {
            div().p_6().text_color(palette.muted).child("Finding your subscriptions…").into_any_element()
        } else if visible.is_empty() {
            div()
                .p_6()
                .text_color(palette.muted)
                .child("Nothing left to review. Senders you keep, defer or leave are hidden here.")
                .into_any_element()
        } else {
            v_flex()
                .id("subs-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(visible.iter().enumerate().map(|(i, sender)| {
                    self.subscription_row(i, sender, screen, palette, cx)
                }))
                .into_any_element()
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(self.pane_surface(palette.background, 0.05))
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn subscription_row(
        &self,
        index: usize,
        sender: &Sender,
        screen: &Screen,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let address = sender.address.clone();
        let checked = screen.selected.contains(&address);
        let working = screen.working.contains(&address);
        let method = sender.method();
        let toggle = address.clone();
        let (unsub, keep, defer) = (address.clone(), address.clone(), address.clone());
        let badge_color = match method {
            Method::OneClick => palette.accent,
            Method::None => palette.faint,
            _ => palette.muted,
        };
        h_flex()
            .id(SharedString::from(format!("subs-row-{index}")))
            .h(px(44.))
            .flex_shrink_0()
            .items_center()
            .gap_3()
            .px_6()
            .border_b_1()
            .border_color(palette.border)
            .when(checked, |el| el.bg(palette.selected))
            .hover(|s| s.bg(palette.hover))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(screen) = this.subscriptions.as_mut()
                    && !screen.selected.remove(&toggle)
                {
                    screen.selected.insert(toggle.clone());
                }
                cx.notify();
            }))
            .child(
                div()
                    .size(px(16.))
                    .flex_shrink_0()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if checked { palette.accent } else { palette.border })
                    .when(checked, |el| {
                        el.bg(palette.accent).child(Icon::new(IconName::Check).size(px(12.)).text_color(palette.on_accent))
                    }),
            )
            .child(div().w(px(220.)).flex_shrink_0().truncate().font_weight(FontWeight::MEDIUM).child(sender.display().to_owned()))
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(palette.muted).child(sender.address.clone()))
            .child(
                div()
                    .w(px(90.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(palette.muted)
                    .child(format!("{:.1}/month", sender.per_month)),
            )
            .child(
                div()
                    .w(px(78.))
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(badge_color)
                    .child(method.label()),
            )
            .child(
                Button::new(SharedString::from(format!("subs-unsub-{index}")))
                    .small()
                    .label(if working { "Working…" } else { "Unsubscribe" })
                    .disabled(working || method == Method::None)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.unsubscribe_senders(vec![unsub.clone()], cx)
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("subs-keep-{index}")))
                    .ghost()
                    .small()
                    .label("Keep")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.decide_subscription(&keep, true, cx)
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("subs-defer-{index}")))
                    .ghost()
                    .small()
                    .label("Later")
                    .tooltip("Hide for 30 days")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.decide_subscription(&defer, false, cx)
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sender(route: Option<Unsubscribe>, per_month: f32) -> Sender {
        Sender { address: "a@b.test".into(), name: String::new(), account_id: 1, per_month, last_ts: 0, route }
    }

    #[test]
    fn methods_prefer_one_click_then_email_then_web() {
        let mut route = Unsubscribe {
            one_click: Some("https://list.test/u".into()),
            mailto: Some("mailto:u@list.test".into()),
            web: Some("https://list.test/page".into()),
            in_headers: true,
            ..Default::default()
        };
        assert_eq!(sender(Some(route.clone()), 1.).method(), Method::OneClick);
        route.in_headers = false; // a body guess never gets a blind POST
        assert_eq!(sender(Some(route.clone()), 1.).method(), Method::Email);
        route.mailto = None;
        assert_eq!(sender(Some(route), 1.).method(), Method::Web);
        assert_eq!(sender(None, 1.).method(), Method::None);
    }

    #[test]
    fn decisions_hide_kept_done_and_deferred_until_expiry() {
        let mut d = Decisions::default();
        d.kept.insert("k@x".into());
        d.done.insert("d@x".into(), 5);
        d.deferred.insert("l@x".into(), 100);
        assert!(d.hidden("k@x", 0) && d.hidden("d@x", 0) && d.hidden("l@x", 99));
        assert!(!d.hidden("l@x", 100) && !d.hidden("other@x", 0));
    }
}
