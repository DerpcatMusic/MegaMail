# MegaMail license bundle

These read-only notices accompany the Linux MegaMail binary. The inventory was generated
from the locked dependency graph for `apps/megamail`.

- [`inventory.md`](inventory.md) lists every resolved registry/Git package, version, Cargo
  SPDX license declaration, and copied package or upstream license or notice file.
- [`inventory.json`](inventory.json) records the same data, hashes, lockfile identity, and
  any missing or skipped package files.
- [`project/`](project/) contains MegaMail's AGPL license, the complete third-party notice
  file (including Zeron's MIT text), Geist's OFL text, and the bundled Lucide/Feather notices.
- [`dependencies/`](dependencies/) contains package license and notice files found in the
  locked Cargo sources; GPUI Kit's omitted Apache text is copied from its matching official tag.

Packages with missing or skipped license-file issues: **92**.
Read the issue list in `inventory.md` before redistributing. Cargo SPDX declarations are
included even when a crate source did not provide a matching text file.

Regenerate after dependency or lockfile changes with `python3 tools/license-inventory.py`.
The generator uses Python's standard library and `cargo metadata --locked`; it does not
build the application. The published GPUI Kit crate omits its Apache text, so generation
also reads the matching `v<version>/LICENSE-APACHE` from the official Longbridge GitHub tag.
