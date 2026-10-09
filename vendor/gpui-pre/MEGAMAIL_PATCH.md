# MegaMail `gpui-pre` patch

This directory vendors `gpui-pre` 0.3.8 from Zed revision
`279fe070bb389b79652e52065b2f001edcc0b11b`. The upstream source, normalized
and original manifests, README, and Apache-2.0 license are retained. The app
uses a crates.io path patch to this copy.

The patch adds a default-compatible platform hook for explicit text direction,
`TextDirection::{Auto,Ltr,Rtl}`, and `Styled::text_direction`. Text shaping can
use `TextSystem::shape_text_with_direction`; the original `shape_text` API
continues to use automatic direction. Direction and line-clamp values are part
of the wrapped-layout cache key. Native rows carry logical source byte ranges,
caret stops, and exact shaped glyph cluster geometry so painting, hit testing,
copy, and selection can follow visual bidi order without changing source text.
Text layout cache reuse also accounts for direction. Native styled painting indexes decoration byte
ranges once per row and binary-searches them for each visual glyph, avoiding a full style-run scan
for every glyph.

Only a backend that overrides `PlatformTextSystem::layout_wrapped_line_with_direction`
can honor an explicit base direction. The default hook delegates to the older
automatic-direction hook for compatibility with other platform implementations.
MegaMail's matching Linux WGPU backend provides the native bidi implementation.

Run the vendored crate tests serially with:

```sh
cargo test --manifest-path vendor/gpui-pre/Cargo.toml --lib --features test-support
```

Relevant focused tests include `wrapped_layout_cache_key_includes_line_clamp`,
`wrapped_layout_cache_key_includes_base_direction`,
`native_rows_map_aligned_positions_to_visual_edge_offsets`,
`native_hit_test_returns_glyph_index_while_closest_returns_nearest_caret`, and
`legacy_wrapped_rows_use_alignment_for_hit_and_caret_mapping`.

Standalone test portability: retain the EXIF fixture used by the upstream unit
suite and the IBM Plex Sans/Lilex test fonts from the same Zed revision, with
their OFL licenses. Font paths are local to this package. The registry snapshot's
property-test macro fails to expand the randomized-tree fixture; its four
seed cases are run through the existing GPUI test harness instead.
