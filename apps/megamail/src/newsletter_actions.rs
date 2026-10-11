//! User-reviewed newsletter actions from the existing detector's message metadata.
use crate::*;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::{Sizable as _, button::ButtonVariants as _};
use gpui_kit::{
    AppContext as _, IntoElement as _, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};
use megamail_core::{desktop::MailtoIntent, models::Unsubscribe};

fn unsubscribe_intent(metadata: &Unsubscribe) -> Result<MailtoIntent, String> {
    let intent = if let Some(uri) = &metadata.mailto {
        desktop_actions::external_compose(uri)
            .map_err(|_| "This newsletter's email link is invalid.".to_string())?
    } else if let Some(reply) = &metadata.reply {
        if reply.to.len() > 320
            || reply.to.chars().any(char::is_control)
            || reply.subject.len() > 256
            || reply.subject.chars().any(char::is_control)
        {
            return Err("This newsletter's reply instruction is invalid.".into());
        }
        let mut intent = desktop_actions::external_compose(&format!("mailto:{}", reply.to))
            .map_err(|_| "This newsletter's reply address is invalid.".to_string())?;
        intent.subject = reply.subject.clone();
        intent.body.clear();
        intent
    } else {
        return Err("This newsletter has no supported email unsubscribe route.".into());
    };
    if intent.to.is_empty() || !intent.cc.is_empty() || !intent.bcc.is_empty() {
        return Err("Unsubscribe links with copied or hidden recipients are not supported. Review the newsletter's website instead.".into());
    }
    Ok(intent)
}
fn selected_metadata<'a>(
    snapshot: &'a MailboxSnapshot,
    key: &MessageKey,
) -> Option<&'a Unsubscribe> {
    if snapshot.selected_key.as_ref() != Some(key)
        || snapshot
            .selected_message
            .as_ref()
            .is_none_or(|row| &row.key != key)
    {
        return None;
    }
    snapshot
        .selected_sender_check
        .as_ref()?
        .unsubscribe
        .as_ref()
}

impl MailApp {
    fn open_unsubscribe_draft(
        &mut self,
        key: &MessageKey,
        expected: &Unsubscribe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.demo || self.view != View::Mailbox || self.composer.is_some() {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(metadata) = selected_metadata(&snapshot, key) else {
            return;
        };
        if metadata != expected {
            return;
        }
        if !self.live.read(cx).can_use_account(key.account_id) {
            return;
        }
        let intent = match unsubscribe_intent(metadata) {
            Ok(intent) => intent,
            Err(error) => {
                self.newsletter_status = Some(error);
                cx.notify();
                return;
            }
        };
        if !self.can_replace_composer(cx) {
            cx.notify();
            return;
        }
        let message = desktop_actions::compose_message(intent, key.account_id);
        let mut composer = Composer::new(message, window, cx);
        composer.notice = Some("Review this newsletter's unsubscribe request, recipients and sender. Send it only when ready.".into());
        composer.recovery.retention = draft_recovery::Retention::Ordinary;
        self.composer = Some(composer);
        self.view = View::Compose;
        cx.notify();
    }
    fn open_unsubscribe_page(&mut self, key: &MessageKey, url: &str, cx: &mut Context<Self>) {
        if self.view != View::Mailbox || self.composer.is_some() {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        if selected_metadata(&snapshot, key).and_then(|u| u.web.as_deref()) != Some(url)
            || !megamail_core::mail_text::safe_browser_url(url)
        {
            return;
        }
        cx.open_url(url);
    }
    pub(crate) fn newsletter_panel(
        &self,
        snapshot: &MailboxSnapshot,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = snapshot.selected_message.as_ref() else {
            return div().into_any_element();
        };
        let Some(metadata) = selected_metadata(snapshot, &row.key) else {
            return div().into_any_element();
        };
        let email = metadata.mailto.is_some() || metadata.reply.is_some();
        let web = metadata
            .web
            .as_deref()
            .filter(|url| megamail_core::mail_text::safe_browser_url(url))
            .map(str::to_owned);
        let expected = metadata.clone();
        let key = row.key.clone();
        let key_web = key.clone();
        v_flex().w_full().gap_2()
            .child(div().text_size(px(11.)).text_color(palette.muted).child(if metadata.in_headers { "Newsletter unsubscribe options from this message" } else { "Possible unsubscribe instructions found in this message" }))
            .when(email, |el| el.child(Button::new("review-unsubscribe-email").ghost().small().label("Review unsubscribe email")
                .disabled(self.demo || self.composer.is_some() || !self.live.read(cx).can_use_account(key.account_id))
                .on_click(cx.listener(move |this, _, window, cx| this.open_unsubscribe_draft(&key, &expected, window, cx)))))
            .when_some(web, |el, url| el
                .child(div().w_full().min_w_0().truncate().text_size(px(11.)).child(url.clone()))
                .child(Button::new("review-unsubscribe-page").ghost().small().label("Review unsubscribe webpage").tooltip(url.clone()).disabled(self.composer.is_some())
                    .on_click(cx.listener(move |this, _, _, cx| this.open_unsubscribe_page(&key_web, &url, cx)))))
            .when(metadata.one_click.is_some(), |el| el.child(div().text_size(px(11.)).text_color(palette.muted).child("Automatic one-click requests are unavailable. Use a reviewed email or webpage option when present.")))
            .when_some(self.newsletter_status.clone(), |el, text| el.child(div().text_size(px(11.)).text_color(palette.muted).child(text)))
            .into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn email_unsubscribe_is_only_a_reviewable_intent_with_no_hidden_recipients() {
        let mut metadata = Unsubscribe {
            mailto: Some("mailto:list@example.test?subject=unsubscribe&body=Remove%20me".into()),
            ..Default::default()
        };
        let intent = unsubscribe_intent(&metadata).unwrap();
        assert_eq!(intent.to, vec!["list@example.test"]);
        assert_eq!(intent.subject, "unsubscribe");
        assert_eq!(intent.body, "Remove me");
        metadata.mailto = Some("mailto:list@example.test?bcc=other@example.test".into());
        assert!(unsubscribe_intent(&metadata).is_err());
        metadata.mailto =
            Some("mailto:list@example.test?subject=%0D%0ABcc:other@example.test".into());
        assert!(unsubscribe_intent(&metadata).is_err());
        metadata.mailto = None;
        metadata.reply = Some(megamail_core::models::ReplyRoute {
            to: "list@example.test".into(),
            subject: "STOP".into(),
        });
        assert_eq!(unsubscribe_intent(&metadata).unwrap().subject, "STOP");
        metadata.reply.as_mut().unwrap().subject = "STOP\nBcc:other@example.test".into();
        assert!(unsubscribe_intent(&metadata).is_err());
    }
}
