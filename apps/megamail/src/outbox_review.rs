//! Read-only delivery review; recorded uncertainty never grants resend permission.
use crate::*;
use megamail_core::models::OutboxPhase;

fn guidance(phase: OutboxPhase) -> (&'static str, &'static str) {
    match phase {
        OutboxPhase::Ready => (
            "Waiting for delivery",
            "MegaMail will send this copy when due and connected. You can cancel a future schedule or discard an unclaimed unscheduled copy.",
        ),
        OutboxPhase::InFlight => (
            "Transmission started",
            "Delivery has not been settled. Check Sent and the recipient before taking further action. This retained copy cannot be canceled or automatically retried.",
        ),
        OutboxPhase::Accepted => (
            "Server accepted this message",
            "Do not resend. A local copy is retained because recording acceptance or storing the Sent copy did not finish. Check your Sent folder and keep this copy for recovery.",
        ),
        OutboxPhase::Refused => (
            "Server refused delivery",
            "Review the server response and correct the recipient or account problem. This copy is held and will not retry automatically. Creating another message requires a separate send decision.",
        ),
        OutboxPhase::Uncertain => (
            "Delivery outcome unknown",
            "The server may have accepted this message even though its acknowledgement was lost. Check Sent and ask the recipient before creating another copy; resending can deliver duplicates. This copy is held.",
        ),
    }
}

impl MailApp {
    pub(crate) fn outbox_review_panel(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some((account, id)) = self.outbox_review else {
            return div().into_any_element();
        };
        let Some(item) = snapshot
            .outbox
            .iter()
            .find(|item| item.id == id && item.account_id == account)
        else {
            return div().into_any_element();
        };
        let (title, explanation) = guidance(item.phase);
        let account_label = snapshot
            .accounts
            .iter()
            .find(|a| a.id == account)
            .map(|a| a.label.clone())
            .unwrap_or_else(|| "Current account".into());
        v_flex()
            .w_full()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(palette.border)
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        Button::new("close-delivery-review")
                            .ghost()
                            .small()
                            .label("Close details")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.outbox_review = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().text_size(px(12.)).child(explanation))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(palette.muted)
                    .child(format!(
                        "Account: {account_label} · Queued {} · {} delivery attempts",
                        megamail_core::datefmt::date_time(item.queued_at),
                        item.attempts
                    )),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .child(format!("To: {}", item.recipients)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .child(format!("Subject: {}", item.subject)),
            )
            .when(!item.last_error.is_empty(), |el| {
                el.child(
                    div()
                        .id("delivery-server-response")
                        .max_h(px(120.))
                        .overflow_y_scroll()
                        .text_size(px(11.))
                        .child(item.last_error.clone()),
                )
            })
            // shortcut: show only the cache's encryption-safe preview, add full MIME review when qualified raw export exists.
            .when(!item.preview.is_empty(), |el| {
                el.child(
                    div()
                        .id("delivery-message-preview")
                        .max_h(px(140.))
                        .overflow_y_scroll()
                        .text_size(px(12.))
                        .child(item.preview.clone()),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_delivery_phases_have_distinct_recovery_guidance() {
        let phases = [
            OutboxPhase::Ready,
            OutboxPhase::InFlight,
            OutboxPhase::Accepted,
            OutboxPhase::Refused,
            OutboxPhase::Uncertain,
        ];
        let titles: HashSet<_> = phases.into_iter().map(|phase| guidance(phase).0).collect();
        assert_eq!(titles.len(), 5);
        assert!(guidance(OutboxPhase::Accepted).1.contains("Do not resend"));
        assert!(guidance(OutboxPhase::Uncertain).1.contains("duplicates"));
        assert!(
            guidance(OutboxPhase::InFlight)
                .1
                .contains("cannot be canceled")
        );
    }
}
