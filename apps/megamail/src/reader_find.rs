//! Explicit bounded search of the current selectable reader's displayed text.
use crate::*;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::{Sizable as _, button::ButtonVariants as _};
use gpui_kit::{
    AppContext as _, IntoElement as _, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};
use std::ops::Range;
const MAX_MATCHES: usize = 500;

struct ResultSet {
    identity: reader_session::SemanticTextIdentity,
    text: String,
    current_text: String,
    quoted_text: Option<String>,
    needle: String,
    matches: Vec<Range<usize>>,
    limited: bool,
}
pub(crate) struct State {
    query: Entity<InputState>,
    open: bool,
    busy: bool,
    result: Option<ResultSet>,
    current: usize,
    status: String,
    revision: u64,
}
impl State {
    pub fn new(window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        Self {
            query: new_input(window, cx, "Exact text, case-sensitive"),
            open: false,
            busy: false,
            result: None,
            current: 0,
            status: String::new(),
            revision: 0,
        }
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn clear(&mut self) {
        self.open = false;
        self.result = None;
        self.current = 0;
        self.status.clear();
        // Retain admission until an existing background scan actually finishes.
        self.revision = self.revision.saturating_add(1);
    }
}
fn join_displayed_text(current: &str, quoted: Option<&str>) -> String {
    match quoted.filter(|text| !text.is_empty()) {
        Some(quote) => format!("{current}\n{quote}"),
        None => current.to_owned(),
    }
}
fn matches(text: &str, needle: &str) -> Result<(Vec<Range<usize>>, bool), String> {
    if text.is_empty() || text.len() > reader_document::SOURCE_MAX {
        return Err("The displayed text is empty, still parsing, or too large to search.".into());
    }
    if needle.is_empty() || needle.len() > 256 || needle.chars().count() > 64 {
        return Err("Enter a phrase of up to 64 characters.".into());
    }
    let mut found: Vec<_> = text
        .match_indices(needle)
        .take(MAX_MATCHES + 1)
        .map(|(offset, matched)| offset..offset + matched.len())
        .collect();
    let limited = found.len() > MAX_MATCHES;
    found.truncate(MAX_MATCHES);
    Ok((found, limited))
}
fn excerpt<'a>(text: &'a str, range: &Range<usize>) -> (&'a str, &'a str, &'a str) {
    let start = text[..range.start]
        .char_indices()
        .rev()
        .nth(60)
        .map_or(0, |(i, _)| i);
    let end = text[range.end..]
        .char_indices()
        .nth(60)
        .map_or(text.len(), |(i, _)| range.end + i);
    (
        &text[start..range.start],
        &text[range.clone()],
        &text[range.end..end],
    )
}
impl MailApp {
    pub(crate) fn focus_reader_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .action_state(cx)
            .allows(desktop_actions::MailboxAction::FocusReader)
        {
            self.reader_find.open = true;
            cx.notify();
            self.reader_find
                .query
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        }
    }
    fn reader_text_segments(
        &self,
        identity: &reader_session::SemanticTextIdentity,
        cx: &App,
    ) -> Option<(
        gpui_kit::component::text::RenderedText,
        Option<gpui_kit::component::text::RenderedText>,
    )> {
        let target = self
            .reader_text
            .as_ref()
            .filter(|target| target.matches(identity))?;
        let current = target.state.read(cx).rendered_text();
        let quoted = if identity.quote_visible {
            self.reader_bodies
                .iter()
                .find(|body| body.key == identity.key && body.has_quote)
                .and_then(|body| {
                    if identity.plain {
                        body.plain_quote_state.as_ref()
                    } else {
                        body.quote_state.as_ref()
                    }
                })
                .map(|state| state.read(cx).rendered_text())
        } else {
            None
        };
        Some((current, quoted))
    }
    fn find_result_matches(&self, result: &ResultSet, cx: &App) -> bool {
        self.reader_text_segments(&result.identity, cx)
            .is_some_and(|(current, quote)| {
                current.as_str() == result.current_text
                    && quote.as_ref().map(|text| text.as_str()) == result.quoted_text.as_deref()
            })
    }
    pub(crate) fn find_in_displayed_message(&mut self, cx: &mut Context<Self>) {
        if self.reader_find.busy || self.view != View::Mailbox {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(identity) = self.reader_text_identity(&snapshot) else {
            return;
        };
        let Some((current, quoted)) = self.reader_text_segments(&identity, cx) else {
            return;
        };
        let current_text = current.as_str().to_owned();
        let quoted_text = quoted.map(|text| text.as_str().to_owned());
        let text = join_displayed_text(&current_text, quoted_text.as_deref());
        let needle = self.reader_find.query.read(cx).value().to_string();
        if text.len() > reader_document::SOURCE_MAX
            || needle.is_empty()
            || needle.len() > 256
            || needle.chars().count() > 64
        {
            self.reader_find.status =
                "Enter an exact phrase of up to 64 characters after the message loads.".into();
            cx.notify();
            return;
        }
        let Some(revision) = self.reader_find.revision.checked_add(1) else {
            self.reader_find.status =
                "Search revision exhausted. Reopen the application before searching again.".into();
            cx.notify();
            return;
        };
        self.reader_find.revision = revision;
        self.reader_find.busy = true;
        self.reader_find.result = None;
        self.reader_find.status = "Searching displayed text…".into();
        let task = cx.background_spawn(async move {
            matches(&text, &needle).map(|(matches, limited)| ResultSet {
                identity,
                text,
                current_text,
                quoted_text,
                needle,
                matches,
                limited,
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.reader_find.busy = false;
                if this.reader_find.revision != revision {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(result) => {
                        let snapshot = this.live.read(cx).snapshot();
                        let still_current = this.view == View::Mailbox
                            && this.reader_text_identity(&snapshot).as_ref()
                                == Some(&result.identity)
                            && this.find_result_matches(&result, cx);
                        if still_current {
                            this.reader_find.current = 0;
                            this.reader_find.status = if result.matches.is_empty() {
                                "No exact matches in the displayed text.".into()
                            } else {
                                format!(
                                    "{}{} matches for “{}”.",
                                    if result.limited { "First " } else { "" },
                                    result.matches.len(),
                                    result.needle
                                )
                            };
                            this.reader_find.result = Some(result);
                        } else {
                            this.reader_find.clear();
                        }
                    }
                    Err(error) => this.reader_find.status = error,
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn reader_find_panel(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.reader_find;
        let result = state
            .result
            .as_ref()
            .filter(|result| self.find_result_matches(result, cx));
        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Input::new(&state.query)
                            .aria_label("Find exact text in displayed message")
                            .disabled(state.busy)
                            .flex_1()
                            .min_w_0(),
                    )
                    .child(
                        Button::new("find-displayed-message")
                            .ghost()
                            .small()
                            .tooltip("Find exact text in this displayed message · Ctrl+Shift+F")
                            .label(if state.busy {
                                "Searching…"
                            } else {
                                "Find in message"
                            })
                            .disabled(state.busy)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.find_in_displayed_message(cx)),
                            ),
                    ),
            )
            .child(
                Button::new("close-reader-find")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::Close).size(px(14.)))
                    .tooltip("Close find")
                    .accessibility_label("Close find")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.reader_find.clear();
                        cx.notify();
                    })),
            )
            .when(!state.status.is_empty(), |el| {
                el.child(div().text_size(px(11.)).text_color(palette.muted).child(
                    if state.result.is_some() && result.is_none() {
                        "The displayed text changed. Search again.".to_owned()
                    } else {
                        state.status.clone()
                    },
                ))
            })
            .when_some(
                result.and_then(|r| r.matches.get(state.current).map(|range| (r, range))),
                |el, (result, range)| {
                    let (before, matched, after) = excerpt(&result.text, range);
                    el.child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("previous-reader-match")
                                    .ghost()
                                    .small()
                                    .label("Previous match")
                                    .disabled(state.current == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reader_find.current =
                                            this.reader_find.current.saturating_sub(1);
                                        cx.notify();
                                    })),
                            )
                            .child(div().text_size(px(11.)).child(format!(
                                "{} of {}",
                                state.current + 1,
                                result.matches.len()
                            )))
                            .child(
                                Button::new("next-reader-match")
                                    .ghost()
                                    .small()
                                    .label("Next match")
                                    .disabled(state.current + 1 >= result.matches.len())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(r) = &this.reader_find.result {
                                            this.reader_find.current = (this.reader_find.current
                                                + 1)
                                            .min(r.matches.len().saturating_sub(1));
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .child(before.to_owned())
                            .child(
                                div()
                                    .font_weight(gpui_kit::FontWeight::BOLD)
                                    .child(matched.to_owned()),
                            )
                            .child(after.to_owned()),
                    )
                },
            )
            .into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_history_is_searchable_but_collapsed_history_is_excluded() {
        let current = "Current reply";
        let quote = "Earlier quoted phrase";
        assert_eq!(
            super::matches(&super::join_displayed_text(current, Some(quote)), "quoted")
                .unwrap()
                .0
                .len(),
            1
        );
        assert!(
            super::matches(&super::join_displayed_text(current, None), "quoted")
                .unwrap()
                .0
                .is_empty()
        );
    }
    #[test]
    fn literal_search_and_unicode_excerpts_are_bounded() {
        let text = format!("{}Needle é[]{}", "界".repeat(80), "🙂".repeat(80));
        let (found, limited) = matches(&text, "é[]").unwrap();
        assert!(!limited);
        let (before, matched, after) = excerpt(&text, &found[0]);
        assert_eq!(matched, "é[]");
        assert!(before.chars().count() <= 61 && after.chars().count() <= 60);
        assert!(matches(&text, "needle").unwrap().0.is_empty());
        let (found, limited) = matches(&"a".repeat(501), "a").unwrap();
        assert_eq!(found.len(), 500);
        assert!(limited);
        assert!(matches("a", "").is_err());
        assert!(matches("a", &"a".repeat(257)).is_err());
    }
}
