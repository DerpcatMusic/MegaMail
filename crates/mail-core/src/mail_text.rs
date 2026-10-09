//! Safe formatting and explicit browser links for the native reader; never fetches mail resources.
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashSet;
use std::rc::Rc;

const MAX_READER_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_READER_DOM_NODES: usize = 65_536;
const MAX_READER_DOM_DEPTH: usize = 128;
// ponytail: larger messages use a readable fallback until the native renderer can stream them.
const READER_HTML_LIMIT_MESSAGE: &str = "<p>This message is too large to display safely.</p>";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextDirection {
    Ltr,
    Rtl,
    Auto,
}

impl TextDirection {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ltr" => Some(Self::Ltr),
            "rtl" => Some(Self::Rtl),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
            Self::Auto => "auto",
        }
    }
}

enum ReaderWalk {
    Enter {
        node: Handle,
        depth: usize,
        direction: Option<TextDirection>,
        layout_table: bool,
    },
    Exit(&'static str),
}

/// Returns a bounded, formatting-preserving HTML fragment for the native mail reader.
///
/// Only semantic text markup and explicit HTTP(S) links survive. Message CSS,
/// alignment, dimensions, remote resources, active content, and form controls do not.
pub fn reader_html(html: &str) -> String {
    render_reader_html(html, false)
        .ok()
        .flatten()
        .unwrap_or_else(|| READER_HTML_LIMIT_MESSAGE.to_owned())
}

/// Renders a sanitized fragment without its final blockquote, if one is present.
pub fn reader_html_without_quote(safe_html: &str) -> Option<String> {
    render_reader_html(safe_html, true).ok().flatten()
}

fn render_reader_html(html: &str, hide_trailing_quote: bool) -> Result<Option<String>, ()> {
    if html.len() > MAX_READER_HTML_BYTES {
        return Err(());
    }

    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(html);
    let hidden_quotes = if hide_trailing_quote {
        let quotes = trailing_blockquotes(&dom.document)?;
        if quotes.is_empty() {
            return Ok(None);
        }
        quotes
    } else {
        HashSet::new()
    };

    let mut output = String::new();
    let mut pending = vec![ReaderWalk::Enter {
        // RcDom's drop clears descendants, even when their handles are queued below.
        node: dom.document.clone(),
        depth: 0,
        direction: None,
        layout_table: false,
    }];
    let mut visited = 0usize;

    while let Some(task) = pending.pop() {
        match task {
            ReaderWalk::Exit(tag) => push_reader(&mut output, &format!("</{tag}>"))?,
            ReaderWalk::Enter {
                node,
                depth,
                direction,
                layout_table,
            } => {
                visited += 1;
                if visited > MAX_READER_DOM_NODES || depth > MAX_READER_DOM_DEPTH {
                    return Err(());
                }
                if hidden_quotes.contains(&(Rc::as_ptr(&node) as usize)) {
                    continue;
                }

                let (direction, tag, layout_table, drop_subtree, image_alt, link) = match &node.data
                {
                    NodeData::Text { contents } => {
                        push_reader_text(&mut output, &contents.borrow())?;
                        continue;
                    }
                    NodeData::Element { name, attrs, .. } => {
                        let name = name.local.as_ref();
                        let attrs = attrs.borrow();
                        let node_direction = attrs
                            .iter()
                            .find(|attribute| attribute.name.local.as_ref() == "dir")
                            .and_then(|attribute| TextDirection::parse(&attribute.value));
                        let direction = node_direction.or(direction);

                        if drops_reader_subtree(name) {
                            (direction, None, layout_table, true, None, None)
                        } else if name == "img" {
                            let alt = attrs
                                .iter()
                                .find(|attribute| attribute.name.local.as_ref() == "alt")
                                .map(|attribute| attribute.value.to_string());
                            (direction, None, layout_table, true, alt, None)
                        } else {
                            let table_layout = if name == "table" {
                                !is_data_table(&node)
                            } else {
                                layout_table
                            };
                            let link = if name == "a" {
                                attrs
                                    .iter()
                                    .find(|attribute| attribute.name.local.as_ref() == "href")
                                    .map(|attribute| attribute.value.to_string())
                                    .filter(|url| safe_browser_url(url))
                            } else {
                                None
                            };
                            let tag = match name {
                                // The document wrapper carries children; its direction is inherited.
                                "html" => None,
                                "body" if direction.is_some() => Some("div"),
                                "body" => None,
                                "a" if link.is_none() => None,
                                "a" => Some("a"),
                                "table" | "tr" if table_layout => None,
                                "td" | "th" if layout_table => Some("div"),
                                "tbody" | "thead" | "tfoot" if layout_table => None,
                                "thead" | "tfoot" => Some("tbody"),
                                "hr" | "wbr" => Some("br"),
                                "strike" => Some("s"),
                                "p" => Some("p"),
                                "div" => Some("div"),
                                "h1" => Some("h1"),
                                "h2" => Some("h2"),
                                "h3" => Some("h3"),
                                "h4" => Some("h4"),
                                "h5" => Some("h5"),
                                "h6" => Some("h6"),
                                "br" => Some("br"),
                                "strong" => Some("strong"),
                                "b" => Some("b"),
                                "em" => Some("em"),
                                "i" => Some("i"),
                                "u" => Some("u"),
                                "s" => Some("s"),
                                "del" => Some("del"),
                                "code" => Some("code"),
                                "pre" => Some("pre"),
                                "blockquote" => Some("blockquote"),
                                "ul" => Some("ul"),
                                "ol" => Some("ol"),
                                "li" => Some("li"),
                                "table" => Some("table"),
                                "tbody" => Some("tbody"),
                                "tr" => Some("tr"),
                                "td" => Some("td"),
                                "th" => Some("th"),
                                "span" => Some("span"),
                                "caption" => Some("caption"),
                                _ => None,
                            };
                            (direction, tag, table_layout, false, None, link)
                        }
                    }
                    _ => (direction, None, layout_table, false, None, None),
                };

                if let Some(alt) = image_alt {
                    push_reader_text(&mut output, &alt)?;
                }
                if drop_subtree {
                    continue;
                }

                let mut emitted_tag = tag;
                if let Some(tag) = tag {
                    if matches!(tag, "br") {
                        push_reader(&mut output, "<br>")?;
                    } else {
                        push_reader(&mut output, "<")?;
                        push_reader(&mut output, tag)?;
                        if let Some(direction) = direction {
                            push_reader(&mut output, " dir=\"")?;
                            push_reader(&mut output, direction.as_str())?;
                            push_reader(&mut output, "\"")?;
                        }
                        if tag == "a" {
                            if let Some(url) = link.as_deref() {
                                push_reader(&mut output, " href=\"")?;
                                push_reader_attr(&mut output, url)?;
                                push_reader(&mut output, "\"")?;
                            }
                        }
                        push_reader(&mut output, ">")?;
                        if matches!(tag, "hr" | "img" | "input" | "meta" | "link") {
                            emitted_tag = None;
                        }
                    }
                }

                if let Some(tag) = emitted_tag.filter(|tag| *tag != "br") {
                    pending.push(ReaderWalk::Exit(tag));
                }
                let children = node.children.borrow();
                if pending.len().saturating_add(children.len()) > MAX_READER_DOM_NODES * 2 {
                    return Err(());
                }
                pending.extend(children.iter().rev().map(|child| ReaderWalk::Enter {
                    node: child.clone(),
                    depth: depth + 1,
                    direction,
                    layout_table,
                }));
            }
        }
    }

    Ok(Some(output))
}

fn drops_reader_subtree(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "style"
            | "template"
            | "head"
            | "title"
            | "meta"
            | "link"
            | "base"
            | "form"
            | "input"
            | "button"
            | "select"
            | "textarea"
            | "option"
            | "optgroup"
            | "datalist"
            | "iframe"
            | "frame"
            | "frameset"
            | "object"
            | "embed"
            | "applet"
            | "svg"
            | "math"
            | "video"
            | "audio"
            | "source"
            | "track"
            | "canvas"
    )
}

fn is_data_table(table: &Handle) -> bool {
    let NodeData::Element { attrs, .. } = &table.data else {
        return false;
    };
    if attrs.borrow().iter().any(|attribute| {
        attribute.name.local.as_ref() == "role"
            && matches!(
                attribute.value.trim().to_ascii_lowercase().as_str(),
                "presentation" | "none"
            )
    }) {
        return false;
    }

    let mut pending = table
        .children
        .borrow()
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let mut rows = 0usize;
    let mut widest_row = 0usize;
    let mut has_header = false;
    let mut has_caption = false;
    let mut visited = 0usize;
    while let Some(node) = pending.pop() {
        visited += 1;
        if visited > MAX_READER_DOM_NODES {
            return false;
        }
        let NodeData::Element { name, .. } = &node.data else {
            continue;
        };
        match name.local.as_ref() {
            "table" => continue,
            "caption" => has_caption = true,
            "tr" => {
                rows += 1;
                let mut cells = 0usize;
                for child in node.children.borrow().iter() {
                    if let NodeData::Element { name, .. } = &child.data {
                        match name.local.as_ref() {
                            "td" => cells += 1,
                            "th" => {
                                cells += 1;
                                has_header = true;
                            }
                            _ => {}
                        }
                    }
                }
                widest_row = widest_row.max(cells);
            }
            _ => pending.extend(node.children.borrow().iter().rev().cloned()),
        }
    }

    has_header || has_caption || (rows >= 2 && widest_row >= 2)
}

fn trailing_blockquotes(root: &Handle) -> Result<HashSet<usize>, ()> {
    let mut container = root.clone();
    let mut visited = 0usize;
    for _ in 0..MAX_READER_DOM_DEPTH {
        let child = container
            .children
            .borrow()
            .iter()
            .rev()
            .find(|child| match &child.data {
                NodeData::Text { contents } => !contents.borrow().trim().is_empty(),
                NodeData::Comment { .. } | NodeData::Doctype { .. } => false,
                _ => true,
            })
            .cloned();
        let Some(child) = child else {
            return Ok(HashSet::new());
        };
        visited += 1;
        if visited > MAX_READER_DOM_NODES {
            return Err(());
        }
        let NodeData::Element { name, .. } = &child.data else {
            return Ok(HashSet::new());
        };
        if name.local.as_ref() == "blockquote" {
            let mut quotes = HashSet::new();
            quotes.insert(Rc::as_ptr(&child) as usize);
            let children = container.children.borrow();
            if let Some(index) = children.iter().position(|node| Rc::ptr_eq(node, &child)) {
                for sibling in children[..index].iter().rev() {
                    match &sibling.data {
                        NodeData::Text { contents } if contents.borrow().trim().is_empty() => {
                            continue
                        }
                        NodeData::Comment { .. } => continue,
                        NodeData::Element { name, .. } if name.local.as_ref() == "blockquote" => {
                            quotes.insert(Rc::as_ptr(sibling) as usize);
                        }
                        _ => break,
                    }
                }
            }
            return Ok(quotes);
        }
        let wrapper = matches!(
            name.local.as_ref(),
            "html" | "body" | "div" | "section" | "article" | "main" | "aside"
        );
        if !wrapper {
            return Ok(HashSet::new());
        }
        container = child;
    }
    Err(())
}

fn push_reader(output: &mut String, value: &str) -> Result<(), ()> {
    if output.len().saturating_add(value.len()) > MAX_READER_HTML_BYTES {
        return Err(());
    }
    output.push_str(value);
    Ok(())
}

fn push_reader_text(output: &mut String, value: &str) -> Result<(), ()> {
    push_reader_escaped(output, value, false)
}

fn push_reader_attr(output: &mut String, value: &str) -> Result<(), ()> {
    push_reader_escaped(output, value, true)
}

fn push_reader_escaped(output: &mut String, value: &str, attribute: bool) -> Result<(), ()> {
    let mut start = 0usize;
    for (index, character) in value.char_indices() {
        let entity = match character {
            '&' => Some("&amp;"),
            '<' => Some("&lt;"),
            '>' => Some("&gt;"),
            '"' if attribute => Some("&quot;"),
            _ => None,
        };
        if let Some(entity) = entity {
            push_reader(output, &value[start..index])?;
            push_reader(output, entity)?;
            start = index + character.len_utf8();
        }
    }
    push_reader(output, &value[start..])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailLink {
    pub label: String,
    pub url: String,
}

pub fn extract_links(html: &str) -> Vec<MailLink> {
    // ponytail: oversized mail remains readable as text; parse links up to 2 MiB.
    if html.len() > 2 * 1024 * 1024 {
        return Vec::new();
    }
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(html);
    let mut pending = vec![dom.document.clone()];
    let mut links = Vec::new();
    while let Some(node) = pending.pop() {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            if matches!(name.local.as_ref(), "script" | "style" | "template") {
                continue;
            }
            if name.local.as_ref() == "a" {
                if let Some(attr) = attrs
                    .borrow()
                    .iter()
                    .find(|a| a.name.local.as_ref() == "href")
                {
                    let url = attr.value.trim().to_string();
                    if safe_browser_url(&url)
                        && !links.iter().any(|link: &MailLink| link.url == url)
                    {
                        let label = anchor_text(&node);
                        links.push(MailLink {
                            label: if label.is_empty() { url.clone() } else { label },
                            url,
                        });
                        if links.len() == 32 {
                            break;
                        }
                    }
                }
            }
        }
        pending.extend(node.children.borrow().iter().rev().cloned());
    }
    links
}

pub fn safe_browser_url(url: &str) -> bool {
    url.len() <= 4096
        && !url.chars().any(char::is_control)
        && glib::Uri::parse(url, glib::UriFlags::NONE).is_ok_and(|uri| {
            matches!(uri.scheme().as_str(), "http" | "https")
                && uri.host().is_some_and(|host| !host.is_empty())
                && uri.userinfo().is_none()
        })
}

fn anchor_text(node: &Handle) -> String {
    let mut pending = vec![node.clone()];
    let mut text = String::new();
    while let Some(node) = pending.pop() {
        match &node.data {
            NodeData::Text { contents } => text.extend(
                contents
                    .borrow()
                    .chars()
                    .take(120usize.saturating_sub(text.chars().count())),
            ),
            NodeData::Element { name, .. }
                if matches!(name.local.as_ref(), "script" | "style" | "template") =>
            {
                continue
            }
            _ => {}
        }
        if text.chars().count() >= 120 {
            break;
        }
        pending.extend(node.children.borrow().iter().rev().cloned());
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_cta_destination_and_decodes_entities_without_fetching_images() {
        assert!(safe_browser_url("https://example.com/login?a=1&b=2"));
        let links = extract_links(
            r#"<img src="https://tracker.example/pixel"><a href="https://example.com/login?a=1&amp;b=2">Sign <b>in</b></a>"#,
        );
        assert_eq!(
            links,
            vec![MailLink {
                label: "Sign in".into(),
                url: "https://example.com/login?a=1&b=2".into()
            }]
        );
    }

    #[test]
    fn rejects_executable_relative_and_credential_urls() {
        for url in [
            "javascript:alert(1)",
            "file:///tmp/private",
            "data:text/html,test",
            "//example.com",
            "/login",
            "https://u:p@example.com",
            "https://example.com/\n",
        ] {
            assert!(!safe_browser_url(url), "{url}");
        }
        assert!(extract_links(r#"<script><a href="https://example.com">hidden</a></script><a href="javascript:alert(1)">bad</a>"#).is_empty());
    }

    #[test]
    fn deduplicates_and_bounds_link_count_and_labels() {
        let html = (0..50)
            .map(|i| format!("<a href='https://example.com/{i}'>{}</a>", "a".repeat(500)))
            .collect::<String>();
        let links = extract_links(&html);
        assert_eq!(links.len(), 32);
        assert!(links.iter().all(|link| link.label.len() <= 120));
        assert_eq!(
            extract_links("<a href='https://example.com'>a</a><a href='https://example.com'>b</a>")
                .len(),
            1
        );
        assert!(extract_links(&" ".repeat(2 * 1024 * 1024 + 1)).is_empty());
    }

    #[test]
    fn reader_keeps_semantic_formatting_and_safe_links_but_drops_email_styles() {
        let html = reader_html(
            r#"<html dir="RTL"><head><title>hidden title</title><style>.x{color:red}</style></head><body><h2 align="center" style="font-size:90px;color:red">שלום</h2><p dir="ltr">Hi <strong>there</strong>, <em>friend</em>. <a href="https://example.com/path?a=1&amp;b=2" title="ignored" style="color:red">Open</a> <a href="javascript:alert(1)">bad link</a><img src="https://tracker.example/pixel" alt="Logo &amp; label"></p><script>secret()</script></body></html>"#,
        );
        assert!(html.contains("<h2 dir=\"rtl\">שלום</h2>"));
        assert!(html.contains("<p dir=\"ltr\">Hi "));
        assert!(html.contains("<strong dir=\"ltr\">there</strong>"));
        assert!(html.contains("<em dir=\"ltr\">friend</em>"));
        assert!(
            html.contains("<a dir=\"ltr\" href=\"https://example.com/path?a=1&amp;b=2\">Open</a>")
        );
        assert!(html.contains("Logo &amp; label"));
        for removed in [
            "font-size",
            "color:",
            "align=",
            "javascript:",
            "tracker.example",
            "hidden title",
            "secret()",
            "title=",
        ] {
            assert!(!html.contains(removed), "unexpected {removed} in {html}");
        }
    }

    #[test]
    fn reader_preserves_lists_quotes_preformatted_text_and_real_tables() {
        let html = reader_html(
            "<ol><li>First</li><li>Second</li></ol><blockquote><p>Quoted</p></blockquote><pre>  one\n\ttwo &amp; &lt;</pre><table role='presentation'><tr><td>left</td><td>right</td></tr></table><table><tr><td>single cell</td></tr></table><table><tr><th>Name</th><th>Value</th></tr><tr><td>A</td><td>B</td></tr></table>",
        );
        assert!(html.contains("<ol><li>First</li><li>Second</li></ol>"));
        assert!(html.contains("<blockquote><p>Quoted</p></blockquote>"));
        assert!(html.contains("<pre>  one\n\ttwo &amp; &lt;</pre>"));
        assert!(!html.contains("role="));
        assert_eq!(html.matches("<table>").count(), 1);
        assert!(html.contains("left"));
        assert!(html.contains("right"));
        assert!(html.contains("<div>single cell</div>"));
    }

    #[test]
    fn reader_quote_toggle_removes_only_a_trailing_blockquote() {
        let html = reader_html("<p>My reply</p><blockquote><p>Earlier message</p></blockquote>");
        let without_quote = reader_html_without_quote(&html).unwrap();
        assert!(without_quote.contains("My reply"));
        assert!(!without_quote.contains("Earlier message"));
        assert!(reader_html_without_quote("<p>No quote</p>").is_none());

        let html = reader_html(
            "<p>Reply</p><blockquote>First quote</blockquote><blockquote>Older quote</blockquote>",
        );
        let without_quotes = reader_html_without_quote(&html).unwrap();
        assert!(without_quotes.contains("Reply"));
        assert!(!without_quotes.contains("First quote"));
        assert!(!without_quotes.contains("Older quote"));
    }

    #[test]
    fn reader_returns_bounded_fallback_instead_of_truncated_markup() {
        let expanded = reader_html(&"&".repeat(600_000));
        assert_eq!(expanded, READER_HTML_LIMIT_MESSAGE);
        assert_eq!(
            reader_html(&"<b>x</b>".repeat(MAX_READER_DOM_NODES / 2 + 100)),
            READER_HTML_LIMIT_MESSAGE
        );
        assert_eq!(
            reader_html(&" ".repeat(MAX_READER_HTML_BYTES + 1)),
            READER_HTML_LIMIT_MESSAGE
        );
    }
}
