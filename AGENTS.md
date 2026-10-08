# MegaMail

MegaMail is a history-preserving fork of Hylki. The root Rust package and
`src/` remain the upstream GTK application during the GPUI migration.
The new native preview is the independent package at `apps/megamail/`.
Do not mistake a successful preview build for a successful mail-engine port.

## Read first

- `docs/MEGAMAIL_ARCHITECTURE.md`: boundaries, migration order, acceptance gates.
- `docs/research/`: source-linked Hylki, Zeron, GPUI Kit and performance findings.
- `DESIGN.md`: the Zeron-derived visual contract; preserve it across changes.

If a `graft/` directory exists, query it before searching or opening source.
Use `graft ask "<question>" --source` for ranked context, `graft grep` for all
literal occurrences, and `graft callers` before renames or multi-file changes.
Otherwise use `rg` normally.

## Builds

- Never set `CARGO_TARGET_DIR` or `RUSTC_WRAPPER`; use the machine Cargo config.
- Run at most one Cargo build, test, check or clippy at a time across agents.
- Native commands use `--manifest-path apps/megamail/Cargo.toml`.
- Stop any dev servers, virtual displays or demo processes you started.

## Mail and provenance

- Preserve the AGPL license and upstream attribution. Retain Zeron's MIT
  notice with any copied code, tokens or assets.
- Keep the preview local and label sample data. No fake sync, send or OAuth.
- MegaMail must get a separate app ID, config/cache path and credential
  namespace before real accounts are connected. Never implicitly migrate or
  mutate an existing Hylki profile.
- Mail, database queries, parsing and thumbnail decoding belong off the UI
  thread. Rendering must not perform I/O or rebuild a whole mailbox.
- Preserve remote-content blocking, TLS/OAuth validation, secret handling,
  encrypted-mail cache exclusions and durable outbox behavior during porting.
