# MegaMail `gpui-pre-wgpu` patch

This directory vendors `gpui-pre-wgpu` 0.3.8 from Zed revision
`279fe070bb389b79652e52065b2f001edcc0b11b`. The upstream source, normalized
and original manifests, and Apache-2.0 license are retained. This WGPU backend
is paired with the local `gpui-pre` and `cosmic-text` path patches.

The Linux Cosmic Text path shapes bidi paragraphs in one native context,
performs platform-native wrapped-row layout, and carries original UTF-8
cluster ranges, visual caret stops, and glyph paint offsets into GPUI. Explicit
`TextDirection::Ltr` and `TextDirection::Rtl` are passed to Cosmic Text as
Unicode bidi base levels; automatic direction remains first-strong. The source
string is not rewritten, so logical selection and copy preserve email content.
Automatic pure-LTR wrapping continues to use GPUI's existing fast path.
Automatic-direction text containing Unicode paragraph separators uses GPUI's
paragraph-segmented fallback, because Cosmic Text's single `ShapeLine` API
requires one base direction across all bidi paragraphs. Explicit LTR/RTL uses
one forced base level for every paragraph.

The implementation is exercised by a font-backed mixed English/Hebrew
regression. Run the focused test with:

```sh
cargo test --manifest-path vendor/gpui-pre-wgpu/Cargo.toml --lib --features test-support explicit_rtl_base_changes_bidi_order_without_rewriting_source -- --nocapture
```

Additional focused regressions are `mixed_bidi_paragraph_separators_do_not_panic_in_auto_or_explicit_direction`
and `native_wrapping_consumes_global_line_clamp_across_hard_lines`.

Run the full backend unit suite serially with:

```sh
cargo test --manifest-path vendor/gpui-pre-wgpu/Cargo.toml --lib --features test-support
```

The standalone font-backed tests reuse the matching GPUI package's Zed test
fonts under `../gpui-pre/test-fonts`, including their retained OFL licenses.
