//! Local durable Send Later; the account worker must be running for timely delivery.
use crate::*;
use megamail_core::worker::QueuedCancelRequest;

/// Superhuman-style "later" choices in local time: (label, unix seconds).
pub(crate) fn later_presets(now: i64) -> Vec<(&'static str, i64)> {
    use chrono::{Datelike, Duration, Local, TimeZone, Timelike};
    let Some(now) = Local.timestamp_opt(now, 0).single() else { return Vec::new() };
    let at = |days: i64, hour: u32| {
        let day = now.date_naive() + Duration::days(days);
        day.and_hms_opt(hour, 0, 0).and_then(|t| Local.from_local_datetime(&t).earliest()).map(|t| t.timestamp())
    };
    let to_monday = (7 - now.weekday().num_days_from_monday() as i64) % 7;
    let mut out = Vec::new();
    if now.hour() < 17 {
        out.extend(at(0, 18).map(|t| ("This evening", t)));
    }
    out.extend(at(1, 8).map(|t| ("Tomorrow morning", t)));
    out.extend(at(if to_monday == 0 { 7 } else { to_monday }, 8).map(|t| ("Next week", t)));
    out.push(("In 3 days", now.timestamp() + 3 * 86400));
    out
}

impl MailApp {
    /// Pick a preset send time: fills the schedule field and applies it.
    pub(crate) fn schedule_at(&mut self, at: i64, window: &mut Window, cx: &mut Context<Self>) {
        use chrono::TimeZone;
        let Some(text) = chrono::Local.timestamp_opt(at, 0).single().map(|t| t.format("%Y-%m-%d %H:%M %:z").to_string()) else { return };
        if let Some(c) = self.composer.as_ref() {
            c.schedule.update(cx, |input, cx| input.set_value(text, window, cx));
        }
        self.set_compose_schedule(false, window, cx);
    }

    pub(crate) fn set_compose_schedule(
        &mut self,
        clear: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(composer) = self.composer.as_mut() else {
            return;
        };
        if !composer.can_edit() || self.demo {
            return;
        }
        if self
            .live
            .read(cx)
            .schedule_session(composer.message.from_account_id)
            .is_none()
        {
            composer.error =
                Some("Choose an available direct IMAP account to schedule mail.".into());
            cx.notify();
            return;
        }
        let value = if clear {
            String::new()
        } else {
            composer.schedule.read(cx).value().to_string()
        };
        match megamail_core::datefmt::parse_send_time(&value, megamail_core::datefmt::now()) {
            Ok(at) => {
                composer.message.send_at = at;
                if clear {
                    composer
                        .schedule
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                composer.notice = Some(at.map_or_else(|| "Send now selected.".into(), |at| format!("Scheduled for {}. Keep MegaMail running and connected; if closed, it will send after the next launch. Press Schedule to save it to Outbox.", megamail_core::datefmt::date_time(at))));
                composer.error = None;
                self.capture_recovery(cx);
            }
            Err(error) => composer.error = Some(error.into()),
        }
        cx.notify();
    }
    pub(crate) fn cancel_queued_item(
        &mut self,
        account: u32,
        request: QueuedCancelRequest,
        cx: &mut Context<Self>,
    ) {
        if self.queue_cancel_confirmation != Some((account, request)) {
            self.queue_cancel_confirmation = Some((account, request));
            cx.notify();
            return;
        }
        self.queue_cancel_confirmation = None;
        self.live
            .update(cx, |live, cx| live.cancel_queued(account, request, cx));
        cx.notify();
    }
}
