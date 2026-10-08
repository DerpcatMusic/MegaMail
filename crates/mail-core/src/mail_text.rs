//! Explicit browser links for the plain-text reader; never fetches mail resources.
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

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
}
