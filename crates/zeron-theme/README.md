# Zeron theme model

This crate is a source-preserving copy of Zeron's standalone theme crate at
commit `0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9` (version `0.2.107`). It
retains the MIT license in [`LICENSE-MIT`](LICENSE-MIT) and the upstream
theme model, built-in catalog, custom theme library, and offline VS Code theme
importer. The command-line importer binary is intentionally excluded; MegaMail
uses the library API and imports files only after an explicit user action.

Upstream: <https://github.com/zeronsh/zeron/tree/0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9/crates/theme>

MegaMail's GPUI adapter is in `apps/megamail/src/theme.rs`. It stores device
preferences below MegaMail's private XDG config directory and the custom theme
library below its private XDG data directory. Import compilation and all file
I/O must run away from GPUI rendering and input callbacks.
