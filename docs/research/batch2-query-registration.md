# Batch 2: background search connection registration

## Source integration

Parent enabled `functions` beside `bundled` on the existing core rusqlite 0.32 dependency. Installed rusqlite 0.32.1 declares `functions = []`; no extra dependency edges were introduced and no Cargo resolution was run.

Shared `Cache::read_connection_with` opens a separate read-only WAL connection, applies its existing busy timeout, invokes a caller-supplied setup callback, then publishes the reader. Setup failure closes that new connection without migrating or resetting cached data. Existing `read_connection` keeps its public signature and forwards a no-op setup callback. The shared cache and GTK have no dependency on core search and no raw connection getter was added.

Core `QueryService::run_queries` creates that configured reader with `search::register_sql_functions` at its common production/test initialization seam. Cache opening, registration, reads and error handling remain on the background query thread. A failed registration produces Failed page callbacks, not successful empty results. Both header and body scalar predicates are available on every published query reader.

## Prepared, not shipped search semantics

Registration is not parsed-search routing. Existing PageRequest/message_page literal-search behavior is unchanged. Core parsed predicates and model matching share bounded NUL-to-one-space comparison; oversized, missing, non-text or invalid UTF-8 fields remain Unknown. Native routing must report scope and incomplete coverage honestly and must not search ineligible/encrypted raw bodies. Scalar evaluation is not an index or measured speedup.

## Acceptance status

Two Rust regressions are authored, not compiled or executed:

- A real synthetic Cache::open_at legacy row is loaded through scrub_nuls and checked through registered header/body predicates on the configured read-only connection. A write attempt remains forbidden.
- A setup failure preserves synthetic rows and produces exactly one Failed callback through the real query loop rather than empty success.

Rustfmt parsing and whitespace checks passed. Current Context7 documentation plus installed 0.32.1 source were consulted for Connection/read-only/function APIs; installed source remains authoritative for the locked version. No private profiles, credentials, provider operations, Cargo, compiler, installer or mail mutations were used. Parent will execute Rust acceptance with the frozen large batch.
