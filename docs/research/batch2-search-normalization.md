# Batch 2 — search normalization repair and frozen handoff

Status: **owned source frozen; Rust compilation/execution and native integration remain pending**. This follow-on repairs the prepared `SearchQuery` SQL/model text seam from the frozen spec review. It does not establish a shipped all-local/body/provider-search feature or repair every legacy literal-search path.

## Ownership and constraints

Changed only:

- `crates/mail-core/src/search.rs`: shared bounded text comparison, explicit SQLite function registration, SQL predicate calls, and six additional regression tests.
- `docs/research/batch2-search-normalization.md`: this new report.

The parent approved and reports adding `functions` to the existing core rusqlite 0.32 dependency, retaining `bundled`. This owner did not edit any manifest, module export, shared cache, QueryService, native UI, model, worker, or prior owner report. The parent owns those integration files and the single grouped build/test window. No Cargo/build/check/clippy/rustc or native/helper compilation, commit/push, private-profile/credential access, provider/TB login/send/mail mutation, or background process occurred here.

Queried graft first; its graph returned no hits. Read the project instructions, product/design/architecture contracts, prior search/triage and cache-integrity handoffs, relevant foundation findings, and feature-matrix S1/S01–S09 requirements. Loaded Rust, find-docs and diagnosis guidance. The diagnosis runtime gate is deliberately not claimed: the restriction forbids running the actual Rust path. A Python SQLite reproduction establishes the underlying LIKE discrepancy, not end-to-end Rust acceptance.

## Exact defect and normalization

Source fact: `src/models.rs::Message::scrub_nuls` replaces **each NUL with one ASCII space**. Its introductory prose says “strip,” but its executable implementation is `s.replace('\0', " ")`. Removing NULs would introduce another disagreement.

The old `sql_predicate` used SQLite LIKE. The production cache loader scrubs legacy retained headers after SQL retrieval, whereas LIKE stops at embedded NUL. Executed Python reproduction:

| Stored header | Needle | Old SQL LIKE | Loaded model comparison |
| --- | --- | --- | --- |
| `before\0secret` | `secret` | false | true |
| `sec\0ret` | `secret` | false | false |
| `sec\0ret` | `sec ret` | false | true |

Negation reverses known true/false discrepancies. This repair does not remove NUL-bearing rows, remove characters from the query, truncate at NUL, use SQLite replace/hex tricks, or classify unknown text as an empty successful exclusion.

Both loaded matching and registered SQL text predicates now use the same private `contains` helper:

- Each source NUL compares as one ASCII space. `sec\0ret` does **not** match `secret`; it does match the phrase `sec ret`. Adjacent NULs remain adjacent spaces.
- ASCII letters are case-insensitive; non-ASCII case folding is not claimed. Multibyte UTF-8 is retained unchanged.
- Literal backslash-u0000 is not decoded. `%`, `_`, `!`, quotes and injection-shaped search text remain literal substring data. Bound needles are not LIKE patterns.
- Body text uses the same NUL-to-space rule explicitly. This agrees for raw eligible text and already-scrubbed text. It does not imply rendered HTML is an eligible plain-text body.
- Query bounds remain 4,096 UTF-8 bytes and 32 conjunctive terms. Body bounds remain 2 MiB. **New explicit header/preview bound:** 2 MiB per field. Length is measured before normalization; replacing NUL with space preserves byte length. Over-limit fields are **Unknown**, never truncated or silently omitted as known nonmatches.
- SQL NULL, non-text values, invalid UTF-8 and over-limit text yield SQL NULL (Unknown). Body NotLoaded/Excluded yields model Unknown. Rust `String`/`&str` cannot represent invalid UTF-8: an adapter must mark invalid/unavailable body text Excluded/NotLoaded, not lossy-decode it and claim equivalence. This does not change the shared cache loader's separate legacy error-reporting behavior for corrupt rows.
- A known hit in any searched header alternative dominates an unknown alternative (SQL `true OR NULL`). With no known hit, an unknown alternative stays Unknown. A known false conjunct dominates Unknown (SQL `false AND NULL`). NOT preserves Unknown.

The comparison allocates no normalized text copy. It checks input byte limits before UTF-8 validation/scanning in the scalar functions. Normal text uses the existing byte-window ASCII comparison; NUL-bearing text compares mapped bytes. Work remains bounded by field size, needle size and term count, but this is still substring scanning, **not an index or a measured speed improvement**.

## Exact public API and dependency

```rust
pub fn register_sql_functions(connection: &rusqlite::Connection) -> rusqlite::Result<()>;
```

The function registers these two connection-local scalar functions, each with arity two `(field, bound_needle)`:

- `megamail_search_header_contains`: header/preview limit.
- `megamail_search_body_contains`: eligible plain-body limit.

Flags: `SQLITE_UTF8 | SQLITE_DETERMINISTIC | SQLITE_DIRECTONLY`. Schema views/triggers/generated expressions are not permitted to invoke them. Registration changes no schema, stored header or body. It returns a registration error rather than falling back to LIKE. Invalid/empty/over-4,096-byte needle arguments error; the query parser already produces valid bounded needles. Repeated registration is supported by the connection API and covered by an authored test.

Required core dependency: existing `rusqlite = { version = "0.32", features = ["bundled", "functions"] }`, approved/added by the parent. This is the existing rusqlite scalar-function feature, not a new library. Context7 documented `create_scalar_function` and its feature requirement; installed **0.32.1** source was checked for the exact closure signature, `Context::get_raw`, `ValueRef::Text` explicit byte lengths, output/error types and flag definitions. No dependency resolution/build was run by this owner.

References:

- [Connection scalar function API](https://docs.rs/rusqlite/0.32.1/rusqlite/struct.Connection.html#method.create_scalar_function)
- [Function flags](https://docs.rs/rusqlite/0.32.1/rusqlite/functions/struct.FunctionFlags.html)
- Installed source: `rusqlite-0.32.1/src/functions.rs` and `src/types/value_ref.rs` in the Cargo registry.

`SearchQuery::sql_predicate(allow_body)` retains its existing return shape (`SqlPredicate { clause, values, requires_body }`) and fixed `m`/`b` aliases. It now emits the registered function calls rather than LIKE. `values` holds exact owned literal needles. No account/folder scope is implicitly added or widened.

## Production-seam regression tests authored, not executed

Search now has **22 Rust tests**, up from 16. Six additions:

1. `legacy_cache_loaded_headers_and_registered_sql_have_identical_nul_semantics`: creates a randomly named private synthetic directory, opens the actual production `Cache::open_at` upgrade seam, inserts legacy NUL-bearing rows through another SQLite connection to that same synthetic DB, loads the actual model via `Cache::load_messages`, and compares it with the actual generated predicate using the actual registered Rust functions. It also compares a raw unsanitized model with the loaded scrubbed model. Positive and negative bare/from/to/cc/subject queries cover before-NUL suffixes, cross-NUL phrases, adjacent/leading/trailing NULs, literal escapes, multibyte/case sensitivity, wildcard literals and injection-shaped text. Same-UID sibling accounts/folders remain outside the explicit test scope. No configuration/profile discovery is used.
2. `body_nuls_use_the_same_space_normalization_and_unknown_rules`: compares actual registered SQL/model raw and scrubbed body matching, including NotLoaded/Excluded under positive/negative queries.
3. `text_bounds_are_byte_exact_before_nul_normalization`: exact-bound and over-bound UTF-8/NUL fields; compares full SQL tri-state with model tri-state for header and body queries.
4. `invalid_or_absent_sql_text_is_unknown_including_under_negation`: actual SQLite NULL, invalid-UTF-8 TEXT, BLOB and integer projections remain Unknown. Invalid bytes cannot be a Rust `String`; unavailable body eligibility is checked separately.
5. `header_alternatives_preserve_sql_three_valued_logic`: true/unknown OR and false/unknown AND equivalence, including negative queries.
6. `sql_text_functions_are_explicit_connection_local_and_not_like_dependent`: unregistered SQL rejects; registration/re-registration and connection-locality; ASCII matching independent of `case_sensitive_like`; bound literal values; invalid needle lengths reject.

The new `sql_match` test helper selects the predicate expression into `Option<bool>` and maps it to `SearchMatch`; it does not only compare WHERE counts. This prevents NULL and false from being accidentally conflated. The prior two SQLite tests were updated to register the same production functions before use. None of the 22 search tests has been executed or type-checked here. The earlier owner's 37-test total comprised 16 search and 21 triage tests; this owner changed no triage tests.

## Executed validation

- `rustfmt --edition 2021 crates/mail-core/src/search.rs` and subsequent `rustfmt --edition 2021 --check`: final pass. Formatter parsing is not Rust type checking. A missing let-else semicolon was caught and corrected during this loop.
- `git diff --no-index --check /dev/null` for each owned file: no whitespace errors. These files are untracked in the parent's batch; no-index's ordinary “files differ” exit is not a failure of the whitespace check.
- Static Python assertions: exact public API, production cache open/load test seam, scalar function flags, literal bound values, no residual `like_pattern`, no raw NUL in Rust source, and 22 authored search tests.
- **1,063 Python SQLite contract/template probes passed.** They used the fixed scalar-call SQL template extracted from `search.rs`, with explicitly Python-implemented scalar functions. Cases cover NUL normalization, negation, literal escapes/wildcards/injection-shaped data, ASCII/non-ASCII behavior, missing/non-text/oversize inputs, exact UTF-8 byte limits, three-valued logic, and seeded differential inputs. These execute SQLite with a Python contract implementation; **they do not execute the Rust parser/builder/registration/cache loader**, and do not validate Rust handling of corrupt UTF-8. The actual Rust production-seam tests remain the acceptance proof requested by the review.

## Parent integration obligations and assessment

1. Register the functions on **every actual QueryService search connection**, off UI, including read-only and replacement/reopened readers. Handle registration errors as query failure. The shared `Cache` connection is private; any neutral connection-initialization callback/accessor needed to do this is parent-owned. Root shared Cache/GTK must not import the core-only search module.
2. Route prepared SQL/model searches through these actual predicates. Do not replace registration with another Python/SQL normalizer or pretend that an isolated regression fixture wires native search. Existing literal PageRequest SQL and Thunderbird provider search are separate paths and unchanged by this owner.
3. Preserve bound account/folder scope, effective attachment flags, stable paging/cursors, account/mailbox epochs and stale-response rejection from the prior handoff. Keep body joins fully scoped; project only complete eligible plain text, with missing/excluded/invalid bodies as NULL. Preserve encrypted-body cache exclusions and remote-resource privacy.
4. Report Unknown/partial coverage honestly. A WHERE predicate returns only known matches; its omission of unknown/corrupt/oversize rows is not proof of complete negative coverage. The new 2 MiB per-header field limit must not become an undocumented full-search truncation claim. Exclusions must not turn missing bodies into matches.
5. Run the actual 22 search tests and relevant integrated cache/query/GTK/native acceptance in the parent's single batch window after freeze. Verify registration on the real read-only connection seam, not just a separately opened fixture connection.

**Good:** a small repair uses the actual Message and SQLite predicate seam, respects the executable scrubber's space replacement, handles positive/negative/unknown results consistently, preserves safe bound needles and isolation, and adds a real cache-load regression rather than only a substitute model test.

**Bad/limits:** the prepared SQL API now requires connection registration, and the header matcher introduces a documented per-field cap with Unknown results. Scalar substring scans do not supply indexing, full provider history, retention completeness or a native UX. Corrupt UTF-8 cannot be represented as an actual loaded Rust Message; current shared loader error behavior is not redesigned here.

**Risks/blockers:** unexecuted Rust compilation/tests; parent registration and search routing; unknown/partial coverage accounting; eligible body projection and existing cache/paging/epoch integration. Scalar execution may have different cost than SQLite LIKE; no throughput/latency measurement or improvement is claimed. No provider compatibility, production readiness, mail mutation, or end-to-end feature completion claim follows from this freeze.
