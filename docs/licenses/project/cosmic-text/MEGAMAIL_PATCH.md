# MegaMail patch

This is cosmic-text 0.19.0 with its upstream normalized manifest, original
manifest, README, source, tests, and licenses preserved. It adds
`ShapeLine::new_with_base_level` and `ShapeLine::build_with_base_level`, which
accept an optional `unicode_bidi::Level` for explicit paragraph direction.
The existing `new` and `build` methods remain automatic-direction wrappers
that pass `None`; the original text and all byte indices stay unchanged.

It also fixes unwrapped `Wrap::None` layout dropping the first incongruent
directional span when no continuation start was supplied. A zero-valued
traversal origin is no longer mistaken for a clipped span boundary. The
regression covers forced RTL Latin-first and forced LTR Hebrew-first text and
checks that every original source byte remains covered by a glyph range.

The focused regression tests are
`explicit_base_level_overrides_automatic_first_strong_direction` and
`unwrapped_explicit_direction_preserves_all_source_ranges`. Run them with:

```sh
cargo test --manifest-path vendor/cosmic-text/Cargo.toml --lib
```

The crate retains its upstream dual MIT/Apache-2.0 license. See `LICENSE-MIT`
and `LICENSE-APACHE`.
