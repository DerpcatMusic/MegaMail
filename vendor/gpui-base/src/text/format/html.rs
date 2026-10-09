use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{DefiniteLength, Hsla, SharedString, px, relative};
use html5ever::tendril::TendrilSink;
use html5ever::{LocalName, ParseOpts, local_name, parse_document};
use markup5ever_rcdom::{Node, NodeData, RcDom};

use crate::text::document::ParsedDocument;
use crate::text::node::{
    self, BlockNode, CodeBlock, ImageNode, InlineNode, LinkMark, NodeContext, Paragraph,
    ParagraphDirection, Table, TableRow, TextMark,
};

const BLOCK_ELEMENTS: [&str; 35] = [
    "html",
    "body",
    "head",
    "address",
    "article",
    "aside",
    "blockquote",
    "details",
    "summary",
    "dialog",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
    "style",
    "script",
];

/// Parse HTML into AST Node.
pub(crate) fn parse(source: &str, cx: &mut NodeContext) -> Result<ParsedDocument, SharedString> {
    let opts = ParseOpts {
        ..Default::default()
    };

    let bytes = cleanup_html(&source);
    let mut cursor = std::io::Cursor::new(bytes);
    // Ref
    // https://github.com/servo/html5ever/blob/main/rcdom/examples/print-rcdom.rs
    let dom = parse_document(RcDom::default(), opts)
        .from_utf8()
        .read_from(&mut cursor)
        .map_err(|e| SharedString::from(format!("{:?}", e)))?;

    let mut paragraph = Paragraph::default();
    // NOTE: The outer paragraph is not used.
    let node: BlockNode = parse_node(&dom.document, &mut paragraph, cx, ParagraphDirection::Auto)
        .unwrap_or(BlockNode::Unknown);
    let node = node.compact();

    Ok(ParsedDocument {
        source: source.to_string().into(),
        blocks: Arc::new(vec![node]),
    })
}

fn cleanup_html(source: &str) -> Vec<u8> {
    let mut w = std::io::Cursor::new(vec![]);
    let mut r = std::io::Cursor::new(source);
    let mut minify = super::html5minify::Minifier::new(&mut w);
    // Reparse minified fragments in no-quirks mode. The minifier can omit
    // optional end tags (such as `</p>` before a table), and quirks mode can
    // then give following siblings the wrong semantic parent and `dir`.
    if let Ok(()) = minify.minify(&mut r) {
        w.into_inner()
    } else {
        source.bytes().collect()
    }
}

fn attr_value(attrs: &RefCell<Vec<html5ever::Attribute>>, name: LocalName) -> Option<String> {
    attrs.borrow().iter().find_map(|attr| {
        if attr.name.local == name {
            Some(attr.value.to_string())
        } else {
            None
        }
    })
}

fn direction_from_attrs(
    attrs: &RefCell<Vec<html5ever::Attribute>>,
    inherited: ParagraphDirection,
) -> ParagraphDirection {
    attr_value(attrs, local_name!("dir"))
        .and_then(|value| ParagraphDirection::from_html_attribute(&value))
        .unwrap_or(inherited)
}

fn node_direction(node: &Node, inherited: ParagraphDirection) -> ParagraphDirection {
    match &node.data {
        NodeData::Element { name, attrs, .. }
            if BLOCK_ELEMENTS.contains(&name.local.trim())
                || name.local == local_name!("li")
                || name.local == local_name!("tr")
                || name.local == local_name!("td")
                || name.local == local_name!("th") =>
        {
            direction_from_attrs(attrs, inherited)
        }
        _ => inherited,
    }
}

/// Get the highlight background color for a `<mark>` element.
///
/// Reads the `color` attribute first, then the `background-color` declaration
/// from the `style` attribute. Base accepts CSS hex plus common named colors.
fn mark_color(attrs: &RefCell<Vec<html5ever::Attribute>>) -> Option<Hsla> {
    let color_attr = attrs.borrow().iter().find_map(|attr| {
        if &*attr.name.local == "color" {
            Some(attr.value.to_string())
        } else {
            None
        }
    });

    if let Some(value) = color_attr
        && let Some(color) = parse_mark_color(value.trim())
    {
        return Some(color);
    }

    style_attrs(attrs)
        .get("background-color")
        .and_then(|v| parse_mark_color(v.trim()))
}

fn parse_mark_color(value: &str) -> Option<Hsla> {
    if value.starts_with('#') {
        return gpui::Rgba::try_from(value).ok().map(Into::into);
    }
    match value.to_ascii_lowercase().as_str() {
        "black" => Some(gpui::rgb(0x000000).into()),
        "white" => Some(gpui::rgb(0xffffff).into()),
        "blue" => Some(gpui::rgb(0x3b82f6).into()),
        "yellow" => Some(gpui::rgb(0xfacc15).into()),
        _ => None,
    }
}

/// Get style properties to HashMap
/// TODO: Use cssparser to parse style attribute.
fn style_attrs(attrs: &RefCell<Vec<html5ever::Attribute>>) -> HashMap<String, String> {
    let mut styles = HashMap::new();
    let Some(css_text) = attr_value(attrs, local_name!("style")) else {
        return styles;
    };

    for decl in css_text.split(';') {
        let mut parts = decl.splitn(2, ':');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            styles.insert(
                key.trim().to_lowercase().to_string(),
                value.trim().to_string(),
            );
        }
    }

    styles
}

/// Parse length value from style attribute.
///
/// When is percentage, it will be converted to relative length.
/// Else, it will be converted to pixels.
fn value_to_length(value: &str) -> Option<DefiniteLength> {
    if value.ends_with("%") {
        value
            .trim_end_matches("%")
            .parse::<f32>()
            .ok()
            .map(|v| relative(v / 100.))
    } else {
        value
            .trim_end_matches("px")
            .parse()
            .ok()
            .map(|v| px(v).into())
    }
}

/// Get width, height from attributes or parse them from style attribute.
fn attr_width_height(
    attrs: &RefCell<Vec<html5ever::Attribute>>,
) -> (Option<DefiniteLength>, Option<DefiniteLength>) {
    let mut width = None;
    let mut height = None;

    if let Some(value) = attr_value(attrs, local_name!("width")) {
        width = value_to_length(&value);
    }

    if let Some(value) = attr_value(attrs, local_name!("height")) {
        height = value_to_length(&value);
    }

    if width.is_none() || height.is_none() {
        let styles = style_attrs(attrs);
        if width.is_none() {
            width = styles.get("width").and_then(|v| value_to_length(&v));
        }
        if height.is_none() {
            height = styles.get("height").and_then(|v| value_to_length(&v));
        }
    }

    (width, height)
}

fn parse_table_row(table: &mut Table, node: &Rc<Node>, inherited_direction: ParagraphDirection) {
    let direction = node_direction(node, inherited_direction);
    let mut row = TableRow::default();
    let mut count = 0;
    for child in node.children.borrow().iter() {
        match child.data {
            NodeData::Element {
                ref name,
                ref attrs,
                ..
            } if name.local == local_name!("td") || name.local == local_name!("th") => {
                if child.children.borrow().is_empty() {
                    continue;
                }

                count += 1;
                parse_table_cell(&mut row, child, attrs, direction);
            }
            _ => {}
        }
    }

    if count > 0 {
        table.children.push(row);
    }
}

fn parse_table_cell(
    row: &mut node::TableRow,
    node: &Rc<Node>,
    attrs: &RefCell<Vec<html5ever::Attribute>>,
    inherited_direction: ParagraphDirection,
) {
    let mut paragraph = Paragraph::default();
    let direction = direction_from_attrs(attrs, inherited_direction);
    paragraph.set_direction(direction);
    for child in node.children.borrow().iter() {
        parse_paragraph(&mut paragraph, child, direction);
    }
    let width = attr_width_height(attrs).0;
    let table_cell = node::TableCell {
        children: paragraph,
        width,
    };
    row.children.push(table_cell);
}

/// Trim text but leave at least one space.
///
/// - Before: " \r\n Hello world \t "
/// - After: " Hello world "
#[allow(dead_code)]
fn trim_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for (i, c) in text.chars().enumerate() {
        if c.is_whitespace() {
            if i > 0 && out.ends_with(' ') {
                continue;
            }
        }
        out.push(c);
    }

    out
}

fn parse_paragraph(
    paragraph: &mut Paragraph,
    node: &Rc<Node>,
    inherited_direction: ParagraphDirection,
) {
    let direction = node_direction(node, inherited_direction);
    if paragraph.is_empty() {
        paragraph.set_direction(direction);
    }

    fn push_merged(
        paragraph: &mut Paragraph,
        text: String,
        marks: Vec<(Range<usize>, TextMark)>,
        new_mark: Option<TextMark>,
    ) {
        if text.is_empty() {
            return;
        }
        let mut node = InlineNode::new(text).marks(marks);
        if let Some(new_mark) = new_mark {
            let len = node.text.len();
            if let Some(last) = node.marks.last_mut()
                && last.0.start == 0
                && last.0.end == len
            {
                last.1.merge(new_mark);
            } else {
                node.marks.push((0..node.text.len(), new_mark));
            }
        }
        paragraph.push(node);
    }

    fn merge_children_with_mark(
        node: &Node,
        paragraph: &mut Paragraph,
        new_mark: Option<TextMark>,
        direction: ParagraphDirection,
    ) {
        let mut merged_text = String::new();
        let mut merged_marks = Vec::new();

        for child in node.children.borrow().iter() {
            let mut child_paragraph = Paragraph::default();
            parse_paragraph(&mut child_paragraph, &child, direction);

            for node in child_paragraph.children {
                let offset = merged_text.len();
                merged_text.push_str(&node.text);
                for (range, child_mark) in node.marks {
                    merged_marks.push((range.start + offset..range.end + offset, child_mark));
                }

                if let Some(mut image) = node.image {
                    if let Some(link_mark) = new_mark.as_ref().and_then(|mark| mark.link.clone()) {
                        image.link = Some(link_mark);
                    }

                    push_merged(
                        paragraph,
                        std::mem::take(&mut merged_text),
                        std::mem::take(&mut merged_marks),
                        new_mark.clone(),
                    );

                    paragraph.push(InlineNode::image(image));
                }
            }
        }

        push_merged(paragraph, merged_text, merged_marks, new_mark.clone());
    }

    match &node.data {
        NodeData::Text { contents } => {
            let part = &contents.borrow();
            paragraph.push_str(&part);
        }
        NodeData::Element { name, attrs, .. } => match name.local {
            local_name!("br") => paragraph.push_str("\n"),
            local_name!("em") | local_name!("i") => {
                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().italic()),
                    direction,
                );
            }
            local_name!("strong") | local_name!("b") => {
                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().bold()),
                    direction,
                );
            }
            local_name!("del") | local_name!("s") => {
                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().strikethrough()),
                    direction,
                );
            }
            local_name!("u") => {
                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().underline()),
                    direction,
                );
            }
            local_name!("code") => {
                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().code()),
                    direction,
                );
            }
            local_name!("mark") => {
                let color = mark_color(&attrs).unwrap_or_else(|| gpui::rgb(0xfef08a).into());
                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().highlight(color)),
                    direction,
                );
            }
            local_name!("a") => {
                let link_mark = LinkMark {
                    url: attr_value(&attrs, local_name!("href"))
                        .unwrap_or_default()
                        .into(),
                    title: attr_value(&attrs, local_name!("title")).map(Into::into),
                    ..Default::default()
                };

                merge_children_with_mark(
                    node,
                    paragraph,
                    Some(TextMark::default().link(link_mark)),
                    direction,
                );
            }
            local_name!("img") => {
                let Some(src) = attr_value(attrs, local_name!("src")) else {
                    if cfg!(debug_assertions) {
                        tracing::warn!("Image node missing src attribute");
                    }
                    return;
                };

                let alt = attr_value(attrs, local_name!("alt"));
                let title = attr_value(attrs, local_name!("title"));
                let (width, height) = attr_width_height(attrs);

                paragraph.push_image(ImageNode {
                    url: src.into(),
                    alt: alt.map(Into::into),
                    width,
                    height,
                    title: title.map(Into::into),
                    ..Default::default()
                });
            }
            _ => {
                merge_children_with_mark(node, paragraph, None, direction);
            }
        },
        _ => {
            merge_children_with_mark(node, paragraph, None, direction);
        }
    }
}

fn parse_node(
    node: &Rc<Node>,
    paragraph: &mut Paragraph,
    cx: &mut NodeContext,
    inherited_direction: ParagraphDirection,
) -> Option<BlockNode> {
    match node.data {
        NodeData::Text { ref contents } => {
            let text = contents.borrow().to_string();
            if text.len() > 0 {
                if paragraph.is_empty() {
                    paragraph.set_direction(inherited_direction);
                }
                paragraph.push_str(&text);
            }

            None
        }
        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => {
            let direction = node_direction(node, inherited_direction);
            match name.local {
                local_name!("br") => {
                    if paragraph.is_empty() {
                        paragraph.set_direction(direction);
                    }
                    paragraph.push_str("\n");
                    None
                }
                local_name!("pre") => {
                    let mut children = vec![];
                    consume_paragraph(&mut children, paragraph);
                    let mut code = html_text(node);
                    if code.starts_with("\r\n") {
                        code.drain(..2);
                    } else if code.starts_with('\n') {
                        code.remove(0);
                    }
                    let code = BlockNode::CodeBlock(
                        CodeBlock::new(code.into(), None, None::<node::Span>)
                            .with_direction(direction),
                    );
                    if children.is_empty() {
                        Some(code)
                    } else {
                        children.push(code);
                        Some(BlockNode::Root {
                            children,
                            span: None,
                        })
                    }
                }
                local_name!("h1")
                | local_name!("h2")
                | local_name!("h3")
                | local_name!("h4")
                | local_name!("h5")
                | local_name!("h6") => {
                    let mut children = vec![];
                    consume_paragraph(&mut children, paragraph);

                    let level = name
                        .local
                        .chars()
                        .last()
                        .unwrap_or('6')
                        .to_digit(10)
                        .unwrap_or(6) as u8;

                    let mut paragraph = Paragraph::default();
                    paragraph.set_direction(direction);
                    for child in node.children.borrow().iter() {
                        parse_paragraph(&mut paragraph, child, direction);
                    }

                    let heading = BlockNode::Heading {
                        level,
                        children: paragraph,
                        span: None,
                    };
                    if children.len() > 0 {
                        children.push(heading);

                        Some(BlockNode::Root {
                            children,
                            span: None,
                        })
                    } else {
                        Some(heading)
                    }
                }
                local_name!("img") => {
                    let mut children = vec![];
                    consume_paragraph(&mut children, paragraph);

                    let Some(src) = attr_value(attrs, local_name!("src")) else {
                        if cfg!(debug_assertions) {
                            tracing::warn!("image node missing src attribute");
                        }
                        return None;
                    };

                    let alt = attr_value(&attrs, local_name!("alt"));
                    let title = attr_value(&attrs, local_name!("title"));
                    let (width, height) = attr_width_height(&attrs);

                    let mut paragraph = Paragraph::default();
                    paragraph.set_direction(direction);
                    paragraph.push_image(ImageNode {
                        url: src.into(),
                        title: title.map(Into::into),
                        alt: alt.map(Into::into),
                        width,
                        height,
                        ..Default::default()
                    });

                    if children.len() > 0 {
                        children.push(BlockNode::Paragraph(paragraph));
                        Some(BlockNode::Root {
                            children,
                            span: None,
                        })
                    } else {
                        Some(BlockNode::Paragraph(paragraph))
                    }
                }
                local_name!("ul") | local_name!("ol") => {
                    let ordered = name.local == local_name!("ol");
                    let children = consume_children_nodes(node, paragraph, cx, direction);
                    Some(BlockNode::List {
                        children,
                        ordered,
                        start: None,
                        span: None,
                    })
                }
                local_name!("li") => {
                    let mut children = vec![];
                    consume_paragraph(&mut children, paragraph);

                    for child in node.children.borrow().iter() {
                        let mut child_paragraph = Paragraph::default();
                        if let Some(child_node) =
                            parse_node(child, &mut child_paragraph, cx, direction)
                        {
                            children.push(child_node);
                        }
                        if child_paragraph.text_len() > 0 {
                            // If last child is paragraph, merge child
                            if let Some(last_child) = children.last_mut() {
                                if let BlockNode::Paragraph(last_paragraph) = last_child {
                                    last_paragraph.merge(child_paragraph);
                                    continue;
                                }
                            }

                            children.push(BlockNode::Paragraph(child_paragraph));
                        }
                    }

                    consume_paragraph(&mut children, paragraph);

                    Some(BlockNode::ListItem {
                        children,
                        spread: false,
                        checked: None,
                        span: None,
                    })
                }
                local_name!("table") => {
                    let mut children = vec![];
                    consume_paragraph(&mut children, paragraph);

                    let mut table = Table {
                        direction,
                        ..Default::default()
                    };
                    for child in node.children.borrow().iter() {
                        match child.data {
                            NodeData::Element {
                                ref name,
                                ref attrs,
                                ..
                            } if name.local == local_name!("tbody")
                                || name.local == local_name!("thead") =>
                            {
                                let section_direction = direction_from_attrs(attrs, direction);
                                for sub_child in child.children.borrow().iter() {
                                    parse_table_row(&mut table, &sub_child, section_direction);
                                }
                            }
                            _ => {
                                parse_table_row(&mut table, &child, direction);
                            }
                        }
                    }
                    consume_paragraph(&mut children, paragraph);

                    let table = BlockNode::Table(table);
                    if children.len() > 0 {
                        children.push(table);
                        Some(BlockNode::Root {
                            children,
                            span: None,
                        })
                    } else {
                        Some(table)
                    }
                }
                local_name!("blockquote") => {
                    let children = consume_children_nodes(node, paragraph, cx, direction);
                    Some(BlockNode::Blockquote {
                        children,
                        span: None,
                    })
                }
                local_name!("style") | local_name!("script") => None,
                _ => {
                    if BLOCK_ELEMENTS.contains(&name.local.trim()) {
                        let mut children: Vec<BlockNode> = vec![];

                        // Case:
                        //
                        // Hello <p>Inner text of block element</p> World

                        // Insert before text as a node -- The "Hello"
                        consume_paragraph(&mut children, paragraph);
                        paragraph.set_direction(direction);

                        // Inner of the block element -- The "Inner text of block element"
                        for child in node.children.borrow().iter() {
                            if let Some(child_node) = parse_node(child, paragraph, cx, direction) {
                                children.push(child_node);
                            }
                        }
                        consume_paragraph(&mut children, paragraph);

                        if children.is_empty() {
                            None
                        } else {
                            Some(BlockNode::Root {
                                children,
                                span: None,
                            })
                        }
                    } else {
                        // Others to as Inline
                        parse_paragraph(paragraph, node, direction);

                        if paragraph.is_image() {
                            Some(BlockNode::Paragraph(paragraph.take()))
                        } else {
                            None
                        }
                    }
                }
            }
        }
        NodeData::Document => {
            let children = consume_children_nodes(node, paragraph, cx, inherited_direction);
            Some(BlockNode::Root {
                children,
                span: None,
            })
        }
        NodeData::Doctype { .. }
        | NodeData::Comment { .. }
        | NodeData::ProcessingInstruction { .. } => None,
    }
}

fn html_text(node: &Node) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        NodeData::Document | NodeData::Element { .. } => {
            let mut text = String::new();
            for child in node.children.borrow().iter() {
                text.push_str(&html_text(child));
            }
            text
        }
        NodeData::Doctype { .. }
        | NodeData::Comment { .. }
        | NodeData::ProcessingInstruction { .. } => String::new(),
    }
}

fn consume_children_nodes(
    node: &Node,
    paragraph: &mut Paragraph,
    cx: &mut NodeContext,
    direction: ParagraphDirection,
) -> Vec<BlockNode> {
    let mut children = vec![];
    consume_paragraph(&mut children, paragraph);
    for child in node.children.borrow().iter() {
        if let Some(child_node) = parse_node(child, paragraph, cx, direction) {
            children.push(child_node);
        }
        consume_paragraph(&mut children, paragraph);
    }

    children
}

fn consume_paragraph(children: &mut Vec<BlockNode>, paragraph: &mut Paragraph) {
    if paragraph.is_empty() {
        return;
    }

    children.push(BlockNode::Paragraph(paragraph.take()));
}

#[cfg(test)]
mod tests {
    use gpui::{px, relative};
    use std::sync::Arc;

    use crate::text::{
        document::ParsedDocument,
        node::{BlockNode, ImageNode, InlineNode, NodeContext, Paragraph, ParagraphDirection},
    };

    use super::trim_text;

    fn collect_paragraphs(block: &BlockNode, paragraphs: &mut Vec<(String, ParagraphDirection)>) {
        match block {
            BlockNode::Root { children, .. }
            | BlockNode::Blockquote { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => {
                for child in children {
                    collect_paragraphs(child, paragraphs);
                }
            }
            BlockNode::Paragraph(paragraph) => {
                paragraphs.push((paragraph.text(), paragraph.direction));
            }
            BlockNode::Heading { children, .. } => {
                paragraphs.push((children.text(), children.direction));
            }
            BlockNode::Table(table) => {
                for row in &table.children {
                    for cell in &row.children {
                        paragraphs.push((cell.children.text(), cell.children.direction));
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_code(block: &BlockNode, code: &mut Vec<String>) {
        match block {
            BlockNode::Root { children, .. }
            | BlockNode::Blockquote { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => {
                for child in children {
                    collect_code(child, code);
                }
            }
            BlockNode::CodeBlock(block) => code.push(block.code().to_string()),
            _ => {}
        }
    }

    #[test]
    fn test_cleanup_html() {
        let html = r#"<p>
            and
            <code>code</code>
            text
        </p>"#;
        let cleaned = super::cleanup_html(html);
        assert_eq!(
            String::from_utf8(cleaned).unwrap(),
            "<!doctype html><p>and <code>code</code> text"
        );

        let html = r#"<p>
            and
            <em>   <code>code</code>   <i>italic</i>   </em>
            text
        </p>"#;
        let cleaned = super::cleanup_html(html);
        assert_eq!(
            String::from_utf8(cleaned).unwrap(),
            "<!doctype html><p>and <em><code>code</code> <i>italic</i></em> text"
        );
    }

    #[test]
    fn test_trim_text() {
        assert_eq!(trim_text("  \n\tHello world \t\r "), " Hello world ",);
    }

    #[test]
    fn test_mark() {
        let mut cx = NodeContext::default();

        // `<mark>` is rendered as a highlight, kept as `==...==` in markdown.
        let html = r#"<p>Hello <mark>world</mark></p>"#;
        let node = super::parse(html, &mut cx).unwrap();
        assert_eq!(node.to_markdown(), "Hello ==world==");

        let html = r#"<p><mark color="blue">blue</mark> and <mark style="background-color: #336699">hex</mark></p>"#;
        let node = super::parse(html, &mut cx).unwrap();
        assert_eq!(node.to_markdown(), "==blue== and ==hex==");
    }

    #[test]
    fn test_keep_spaces() {
        let html = r#"<p>and <code>code</code> text</p>"#;
        let mut cx = NodeContext::default();
        let node = super::parse(html, &mut cx).unwrap();
        assert_eq!(node.to_markdown(), "and `code` text");

        let html = r#"
            <div>
            <p>
                and
                <em>   <code>code</code>   <i>italic</i>   </em>
                text
            </p>
            <p>
                <img src="https://example.com/image.png" alt="Example" width="100" height="200" title="Example Image" />
            </p>
            <ul>
                <li>Item 1</li>
                <li>Item 2
                </li>
            </ul>
            </div>
        "#;
        let node = super::parse(html, &mut cx).unwrap();
        assert_eq!(
            node.to_markdown(),
            indoc::indoc! {r#"
            and *code italic* text

            ![Example](https://example.com/image.png "Example Image")

            - Item 1
            - Item 2
            "#}
            .trim()
        );
    }

    #[test]
    fn test_value_to_length() {
        assert_eq!(super::value_to_length("100px"), Some(px(100.).into()));
        assert_eq!(super::value_to_length("100%"), Some(relative(1.)));
        assert_eq!(super::value_to_length("56%"), Some(relative(0.56)));
        assert_eq!(super::value_to_length("240"), Some(px(240.).into()));
    }

    #[test]
    fn test_image() {
        let html = r#"<img src="https://example.com/image.png" alt="Example" width="100" height="200" title="Example Image" />"#;
        let mut cx = NodeContext::default();
        let node = super::parse(html, &mut cx).unwrap();
        assert_eq!(
            node,
            ParsedDocument {
                source: html.to_string().into(),
                blocks: Arc::new(vec![BlockNode::Paragraph(Paragraph {
                    span: None,
                    children: vec![InlineNode::image(ImageNode {
                        url: "https://example.com/image.png".to_string().into(),
                        alt: Some("Example".to_string().into()),
                        width: Some(px(100.).into()),
                        height: Some(px(200.).into()),
                        title: Some("Example Image".to_string().into()),
                        ..Default::default()
                    })],
                    ..Default::default()
                })])
            }
        );

        let html = r#"<img src="https://example.com/image.png" alt="Example" style="width: 80%" title="Example Image" />"#;
        let node = super::parse(html, &mut cx).unwrap();
        assert_eq!(
            node,
            ParsedDocument {
                source: html.to_string().into(),
                blocks: Arc::new(vec![BlockNode::Paragraph(Paragraph {
                    span: None,
                    children: vec![InlineNode::image(ImageNode {
                        url: "https://example.com/image.png".to_string().into(),
                        alt: Some("Example".to_string().into()),
                        width: Some(relative(0.8)),
                        height: None,
                        title: Some("Example Image".to_string().into()),
                        ..Default::default()
                    })],
                    ..Default::default()
                })])
            }
        );
    }

    #[test]
    fn html_direction_inherits_and_can_be_overridden_in_paragraphs_and_cells() {
        let html = r#"<html dir="rtl"><body><div><p>שלום</p><p dir="ltr">English introduction שלום</p><table><tr><td>عربي</td><td dir="ltr">English</td></tr></table></div></body></html>"#;
        let mut cx = NodeContext::default();
        let document = super::parse(html, &mut cx).unwrap();
        let mut paragraphs = Vec::new();
        for block in document.blocks.iter() {
            collect_paragraphs(block, &mut paragraphs);
        }

        assert_eq!(
            paragraphs
                .iter()
                .map(|(_, direction)| *direction)
                .collect::<Vec<_>>(),
            vec![
                ParagraphDirection::Rtl,
                ParagraphDirection::Ltr,
                ParagraphDirection::Rtl,
                ParagraphDirection::Ltr,
            ]
        );
    }

    #[test]
    fn html_table_direction_is_independent_of_first_cell_override() {
        let mut cx = NodeContext::default();
        let document = super::parse(
            r#"<table dir="rtl"><tr><td dir="ltr">English</td><td>שלום</td></tr></table>"#,
            &mut cx,
        )
        .unwrap();
        let table = document
            .blocks
            .iter()
            .find_map(|block| {
                if let BlockNode::Table(table) = block {
                    Some(table)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(table.resolved_rtl());
        assert_eq!(
            table.children[0].children[0].children.direction,
            ParagraphDirection::Ltr
        );
    }

    #[test]
    fn html_direction_scopes_nested_siblings_and_table_rows() {
        let html = r#"<html dir="rtl"><body><div><p dir="ltr">English introduction שלום</p><table><tr dir="rtl"><td>عربي row</td><td dir="ltr">English cell</td></tr><tr dir="ltr"><td>English row</td><td>English cell</td></tr></table><p>שלום after table</p></div></body></html>"#;
        let mut cx = NodeContext::default();
        let document = super::parse(html, &mut cx).unwrap();
        let mut paragraphs = Vec::new();
        for block in document.blocks.iter() {
            collect_paragraphs(block, &mut paragraphs);
        }

        assert_eq!(
            paragraphs
                .iter()
                .map(|(_, direction)| *direction)
                .collect::<Vec<_>>(),
            vec![
                ParagraphDirection::Ltr,
                ParagraphDirection::Rtl,
                ParagraphDirection::Ltr,
                ParagraphDirection::Ltr,
                ParagraphDirection::Ltr,
                ParagraphDirection::Rtl,
            ]
        );
    }

    #[test]
    fn html_break_preserves_logical_line_order() {
        let html = "<p>before<br>after</p>";
        let mut cx = NodeContext::default();
        let document = super::parse(html, &mut cx).unwrap();
        let mut paragraphs = Vec::new();
        for block in document.blocks.iter() {
            collect_paragraphs(block, &mut paragraphs);
        }

        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].0, "before\nafter");
    }

    #[test]
    fn html_pre_keeps_indentation_lines_and_final_newline() {
        let html = "<pre>  first\n    second\n</pre>";
        let mut cx = NodeContext::default();
        let document = super::parse(html, &mut cx).unwrap();
        let mut code = Vec::new();
        for block in document.blocks.iter() {
            collect_code(block, &mut code);
        }

        assert_eq!(code, vec!["  first\n    second\n"]);
    }
}
