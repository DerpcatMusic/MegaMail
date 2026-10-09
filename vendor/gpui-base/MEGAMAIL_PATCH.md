# MegaMail GPUI Base patch

This directory vendors `gpui-base` 0.7.1 from crates.io so MegaMail can make a
narrow native-reader patch without forking GPUI Kit. The upstream crate source,
`Cargo.toml.orig`, README, tests, and `LICENSE-APACHE` are retained. Registry
build markers, `.cargo/`, and the registry crate's unrelated `Cargo.lock` are
excluded. `apps/megamail/Cargo.toml` applies the path patch only to the
standalone MegaMail app.

The patch honors semantic HTML `dir="ltr|rtl|auto"` through inherited block
direction, aligns paragraphs and code blocks, places list markers and quote
rails on the RTL leading edge, reverses RTL table columns, treats `<br>` as an
inline newline, and displays `<pre>` as a whitespace-preserving monospace code
block. HTML minification retains the canonical doctype so compressed
paragraph end tags cannot leak direction into sibling tables. Paragraph direction uses Unicode Bidi first-strong detection. Any text
paragraph containing RTL characters, RTL numbers, or bidi formatting controls
uses one GPUI `Inline` with all styled runs, so Cosmic Text receives the entire
logical paragraph for shaping and its byte-based selection remains in original
logical order. Pure-LTR inline-code paragraphs retain Base's smaller padded
code `InlineFlow`.

Keyboard select-all clears the existing window selection before selecting the
focused `TextView` alone. This prevents Base's window-level copy projection
from joining a stale local select-all in another reader field; mouse drag
selection can still span multiple views.

The matching local GPUI/Cosmic patches now pass explicit paragraph base levels
through shaping and wrapped-layout caches, including all-Latin RTL paragraphs.
Block `dir` therefore controls both alignment and Unicode bidi shaping. Inline
fragment directional isolates remain unsupported. RTL inline-code runs retain
monospace and code background in the whole-paragraph path, but GPUI `TextRun`
has no per-run font size, so the 0.875 code scale is not applied on that path.
Original logical text and copied content remain unchanged.

The app must pass email bodies through `mail_core::mail_text::reader_html` and
handle clicked links through its URL policy. This Base patch does not sanitize
HTML or make raw email HTML safe to render; in particular, Base's generic HTML
parser still supports image nodes for other callers.

Run the vendored crate's unit suite serially with:

```sh
cargo test --manifest-path vendor/gpui-base/Cargo.toml --lib
```

The focused new regression tests are named `automatic_bidi_uses_first_strong_after_digits_and_punctuation`,
`mixed_ltr_rtl_code_uses_one_logical_inline_and_copies_original_order`,
`rtl_code_mark_uses_whole_paragraph_layout_and_preserves_logical_copy`,
`html_direction_inherits_and_can_be_overridden_in_paragraphs_and_cells`,
`html_direction_scopes_nested_siblings_and_table_rows`,
`html_break_preserves_logical_line_order`, and
`html_pre_keeps_indentation_lines_and_final_newline`.

The native wrapped-row API from the matching local GPUI patch is consumed by
Base's row extents, glyph background geometry, touch selection bounds, and
selection painting. Source byte ranges are preserved while each visual row is
aligned independently, so RTL selection and inline-code backgrounds follow the
painted glyphs. `native_rtl_row_geometry_keeps_source_ranges_and_right_alignment`
covers the source-range and alignment projection.
