//! Opt-in formatted composition through Hylki's Markdown engine, off the UI lane.
use crate::*;
#[derive(Default)]
pub(crate) struct Markup {
    pub enabled: bool,
    pub preview: bool,
    pub generation: u64,
    pub ready_source: String,
    pub html: String,
    pub semantic: SharedString,
    pub busy: bool,
    pub error: Option<String>,
}
impl MailApp {
    pub(crate) fn refresh_markdown(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.composer.as_mut().filter(|c| c.markup.enabled) else {
            return;
        };
        let source = c.body.read(cx).value().to_string();
        c.markup.generation = c.markup.generation.wrapping_add(1);
        let generation = c.markup.generation;
        c.markup.busy = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(180)).await;
            let current = this.update(cx, |this, cx| this.composer.as_ref().is_some_and(|c| c.markup.enabled && c.markup.generation == generation && c.body.read(cx).value().as_ref() == source)).unwrap_or(false);
            if !current { return; }
            let result = cx.background_spawn(async move {
                if source.len() > 2 * 1024 * 1024 { return Err("Formatted messages are limited to 2 MiB. Use plain text for this draft.".to_owned()); }
                let html = megamail_core::markdown::to_html(&source);
                let semantic = megamail_core::mail_text::reader_html(&html);
                Ok((source, html, semantic))
            }).await;
            let _ = this.update(cx, |this, cx| {
                let Some(c) = this.composer.as_mut().filter(|c| c.markup.enabled && c.markup.generation == generation) else { return; };
                c.markup.busy = false;
                match result {
                    Ok((source, html, semantic)) if c.body.read(cx).value().as_ref() == source => { c.markup.ready_source = source; c.markup.html = html; c.markup.semantic = semantic.into(); c.markup.error = None; c.recovery.dirty_input = true; }
                    Ok(_) => return,
                    Err(error) => c.markup.error = Some(error),
                }
                this.capture_recovery(cx);
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    pub(crate) fn markup_controls(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = &self.composer else {
            return div().into_any_element();
        };
        v_flex().gap_2()
            .child(h_flex().flex_wrap().gap_2()
                .child(Button::new("compose-plain-text").ghost().small().label("Plain text").selected(!c.markup.enabled).disabled(!c.can_edit())
                    .on_click(cx.listener(|this, _, _, cx| { if let Some(c) = &mut this.composer { c.markup.enabled = false; c.markup.preview = false; c.markup.generation = c.markup.generation.wrapping_add(1); c.recovery.dirty_input = true; } this.capture_recovery(cx); cx.notify(); })))
                .child(Button::new("compose-markdown").ghost().small().label("Formatted · Markdown").selected(c.markup.enabled).disabled(!c.can_edit())
                    .on_click(cx.listener(|this, _, _, cx| { if let Some(c) = &mut this.composer { c.markup.enabled = true; } this.refresh_markdown(cx); })))
                .when(c.markup.enabled, |el| el.child(Button::new("preview-compose-markdown").secondary().small().label(if c.markup.preview { "Hide preview" } else { "Show preview" }).on_click(cx.listener(|this, _, _, cx| { if let Some(c) = &mut this.composer { c.markup.preview = !c.markup.preview; } cx.notify(); })))))
            .when(c.markup.enabled, |el| el.child(div().text_size(px(12.)).text_color(palette.muted).child(if c.markup.busy { "Updating formatting…" } else { "Use **bold**, *italic*, lists, headings and links. Sends HTML with a plain-text fallback." })))
            .when_some(c.markup.error.clone(), |el, error| el.child(div().child(error)))
            .when(c.markup.enabled && c.markup.preview && !c.markup.semantic.is_empty(), |el| el.child(div().p_4().rounded(px(8.)).bg(palette.surface).child(gpui_kit::component::text::TextView::html("compose-markdown-preview", c.markup.semantic.clone()).selectable(true).w_full().text_size(px(14.)).line_height(relative(1.6)))))
            .into_any_element()
    }
}
