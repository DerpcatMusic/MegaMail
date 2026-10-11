//! Local durable Send Later; the account worker must be running for timely delivery.
use crate::*;
use megamail_core::worker::QueuedCancelRequest;
impl MailApp {
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
