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
    /// Wrap the selection (or the caret) in Markdown markers, or prefix each
    /// selected line when `close` is empty. Only used in Formatted mode.
    pub(crate) fn format_body(&mut self, open: &'static str, close: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.composer.as_mut().filter(|c| c.markup.enabled && c.can_edit()) else { return; };
        let body = c.body.clone();
        let range = body.read(cx).selected_range();
        let text = body.read(cx).value().to_string();
        let (start, end) = (range.start.min(text.len()), range.end.min(text.len()));
        let Some(selected) = text.get(start..end) else { return; };
        let wrapped = if close.is_empty() {
            let lines: Vec<String> = selected.split('\n').map(|l| format!("{open}{l}")).collect();
            lines.join("\n")
        } else {
            format!("{open}{selected}{close}")
        };
        let caret = start + open.len() + if selected.is_empty() { 0 } else { selected.len() + close.len() };
        body.update(cx, |body, cx| {
            body.insert(wrapped, window, cx);
            if selected.is_empty() { body.set_selected_range(caret..caret, cx); }
        });
        c.recovery.dirty_input = true;
        cx.notify();
    }

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
    /// Formatting strip and preview; only rendered while Formatted is on.
    pub(crate) fn markup_controls(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = &self.composer else {
            return div().into_any_element();
        };
        v_flex().gap_2()
            .child(h_flex().flex_wrap().items_center().gap_1()
                .child(Button::new("fmt-bold").ghost().small().icon(Icon::new(IconName::Bold).size(px(16.))).accessibility_label("Bold").tooltip("Bold · wraps the selection in **").on_click(cx.listener(|this, _, w, cx| this.format_body("**", "**", w, cx))))
                .child(Button::new("fmt-italic").ghost().small().icon(Icon::new(IconName::Italic).size(px(16.))).accessibility_label("Italic").tooltip("Italic").on_click(cx.listener(|this, _, w, cx| this.format_body("*", "*", w, cx))))
                .child(Button::new("fmt-link").ghost().small().icon(Icon::new(IconName::Link).size(px(16.))).accessibility_label("Link").tooltip("Turns the selection into a link; paste the address over “url”").on_click(cx.listener(|this, _, w, cx| this.format_body("[", "](url)", w, cx))))
                .child(Button::new("fmt-list").ghost().small().icon(Icon::new(IconName::List).size(px(16.))).accessibility_label("Bulleted list").tooltip("Bulleted list").on_click(cx.listener(|this, _, w, cx| this.format_body("- ", "", w, cx))))
                .child(Button::new("fmt-quote").ghost().small().icon(Icon::new(IconName::Quote).size(px(16.))).accessibility_label("Quote").tooltip("Quote").on_click(cx.listener(|this, _, w, cx| this.format_body("> ", "", w, cx))))
                .child(div().flex_1())
                .child(Button::new("preview-compose-markdown").ghost().small().label(if c.markup.preview { "Hide preview" } else { "Show preview" }).on_click(cx.listener(|this, _, _, cx| { if let Some(c) = &mut this.composer { c.markup.preview = !c.markup.preview; } cx.notify(); }))))
            .when_some(c.markup.error.clone(), |el, error| el.child(div().text_size(px(12.)).text_color(palette.muted).child(error)))
            .when(c.markup.preview && !c.markup.semantic.is_empty(), |el| el.child(div().p_4().rounded(px(8.)).border_1().border_color(palette.border).bg(palette.surface).child(gpui_kit::component::text::TextView::html("compose-markdown-preview", c.markup.semantic.clone()).selectable(true).w_full().text_size(px(14.)).line_height(relative(1.6)))))
            .into_any_element()
    }
}
