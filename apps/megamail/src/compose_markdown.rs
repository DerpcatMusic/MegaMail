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
}

/// Toolbar actions; each wraps or prefixes the body selection in Markdown.
#[derive(Clone, Copy)]
pub(crate) enum Mark {
    Bold,
    Italic,
    Link,
    Bullets,
    Numbers,
    Quote,
    Heading,
}

/// The replacement for `selected`, and the sub-range of it to select after.
pub(crate) fn apply(mark: Mark, selected: &str) -> (String, std::ops::Range<usize>) {
    let wrap = |open: &str, close: &str, placeholder: &str| {
        let inner = if selected.is_empty() { placeholder } else { selected };
        (format!("{open}{inner}{close}"), open.len()..open.len() + inner.len())
    };
    let prefix = |each: &dyn Fn(usize) -> String| {
        let text = if selected.is_empty() { "item" } else { selected };
        let out = text.lines().enumerate().map(|(i, l)| format!("{}{l}", each(i))).collect::<Vec<_>>().join("\n");
        let len = out.len();
        (out, len..len)
    };
    match mark {
        Mark::Bold => wrap("**", "**", "bold"),
        Mark::Italic => wrap("*", "*", "italic"),
        Mark::Link => {
            let is_url = selected.starts_with("http://") || selected.starts_with("https://");
            if is_url {
                (format!("[link]({selected})"), 1..5)
            } else {
                let text = if selected.is_empty() { "link" } else { selected };
                let out = format!("[{text}](https://)");
                let at = out.len() - 1;
                (out, at - 8..at)
            }
        }
        Mark::Bullets => prefix(&|_| "- ".into()),
        Mark::Numbers => prefix(&|i| format!("{}. ", i + 1)),
        Mark::Quote => prefix(&|_| "> ".into()),
        Mark::Heading => prefix(&|_| "## ".into()),
    }
}

impl MailApp {
    pub(crate) fn format_body(&mut self, mark: Mark, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.composer.as_mut().filter(|c| c.can_edit()) else { return };
        c.markup.enabled = true;
        let body = c.body.clone();
        body.update(cx, |input, cx| {
            let range = input.selected_range();
            let value = input.value();
            let selected = value.get(range.clone()).unwrap_or_default().to_owned();
            let (text, select) = apply(mark, &selected);
            input.insert(text, window, cx);
            input.set_selected_range(range.start + select.start..range.start + select.end, cx);
        });
        body.read(cx).focus_handle(cx).focus(window, cx);
        self.refresh_markdown(cx);
    }

    pub(crate) fn format_toolbar(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = &self.composer else { return div().into_any_element() };
        let enabled = c.can_edit();
        let tool = |id: &'static str, label: &'static str, tip: &'static str, mark: Mark| {
            Button::new(id).ghost().xsmall().label(label).tooltip(tip).disabled(!enabled)
                .on_click(cx.listener(move |this, _, window, cx| this.format_body(mark, window, cx)))
        };
        h_flex()
            .w_full()
            .gap_1()
            .items_center()
            .py_1()
            .border_b_1()
            .border_color(palette.border)
            .child(tool("fmt-bold", "B", "Bold · Ctrl+B", Mark::Bold).font_weight(FontWeight::BOLD))
            .child(tool("fmt-italic", "I", "Italic · Ctrl+I", Mark::Italic).italic())
            .child(tool("fmt-link", "Link", "Insert link · Ctrl+K", Mark::Link))
            .child(div().w(px(1.)).h(px(14.)).mx_1().bg(palette.border))
            .child(tool("fmt-bullets", "• List", "Bulleted list", Mark::Bullets))
            .child(tool("fmt-numbers", "1. List", "Numbered list", Mark::Numbers))
            .child(tool("fmt-quote", "Quote", "Quote", Mark::Quote))
            .child(tool("fmt-heading", "Heading", "Heading", Mark::Heading))
            .child(div().flex_1())
            .child(
                Button::new("fmt-plain").ghost().xsmall()
                    .label(if c.markup.enabled { "Formatted" } else { "Plain text" })
                    .selected(!c.markup.enabled)
                    .tooltip("Formatted sends HTML with a plain-text copy. Click to switch.")
                    .disabled(!enabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let Some(c) = &mut this.composer else { return };
                        c.markup.enabled = !c.markup.enabled;
                        c.markup.generation = c.markup.generation.wrapping_add(1);
                        c.recovery.dirty_input = true;
                        if c.markup.enabled { this.refresh_markdown(cx) } else { cx.notify() }
                    })),
            )
            .when(c.markup.enabled, |row| row.child(
                Button::new("fmt-preview").ghost().xsmall()
                    .label("Preview")
                    .selected(c.markup.preview)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(c) = &mut this.composer { c.markup.preview = !c.markup.preview; }
                        cx.notify();
                    })),
            ))
            .into_any_element()
    }

    /// The formatted preview under the editor, when asked for.
    pub(crate) fn format_preview(&self, palette: Palette) -> Option<AnyElement> {
        let c = self.composer.as_ref()?;
        (c.markup.enabled && c.markup.preview && !c.markup.semantic.is_empty()).then(|| {
            div().p_4().rounded(px(8.)).bg(palette.surface)
                .child(gpui_kit::component::text::TextView::html("compose-markdown-preview", c.markup.semantic.clone()).selectable(true).w_full().text_size(px(14.)).line_height(relative(1.6)))
                .into_any_element()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_wrap_and_prefix_the_selection() {
        assert_eq!(apply(Mark::Bold, "hi"), ("**hi**".into(), 2..4));
        assert_eq!(apply(Mark::Link, "https://x.io").0, "[link](https://x.io)");
        assert_eq!(apply(Mark::Link, "docs"), ("[docs](https://)".into(), 7..15));
        assert_eq!(apply(Mark::Numbers, "a\nb").0, "1. a\n2. b");
    }
}
