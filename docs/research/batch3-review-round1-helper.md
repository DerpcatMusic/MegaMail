## Standards

**0 confirmed hard violations; 0 new concrete P2 findings.**

Read-only review against HEAD `d4dbd2bba14f474a33870269cc15bd98376ff360`, limited to `tools/megamail-html-reader/main.c`, `viewport_contract.py`, `docs/research/batch3-helper-narrow-toolbar.md`, and the added parent-driver step. Read AGENTS, architecture/design requirements, Impeccable/code-review guidance, R7/R8 and prior fixture evidence. Graft was available but reported no graph; no graph was built.

Released hashes matched before and after inspection:
- `main.c`: `cd2a4473a53afc268c57f121d45e8b143d689fc2f05e28cd4bce8f17ac156616`
- `viewport_contract.py`: `9c8bf99bc487106ef122467196a7649ff1f597b2fd169648ca2b071afced9325`

The read-only contract passed **44 source checks and 8 fingerprints**, with **43 GTK / 1 ATK / 63 WebKit declarations** found. These establish neither compilation nor runtime acceptance. The report and `helper-viewport-source` driver step correctly preserve that distinction; the step precedes compilation without enabling the release gate.

## Spec

**SOURCE_ACCEPTED — runtime pending.**

`main.c:260–323` reasonably addresses the toolbar clamp: two horizontal action rows replace the single row; subject and notice use WORD_CHAR wrapping, minimum width one character and natural width sixty characters. Installed GTK documentation supports this interpretation. Default **680×600**, direct WebView placement, fallback scroller/TextView and protocol remain unchanged. No warning ellipsis, line limit, forced geometry or document-style workaround was introduced.

**Keyboard/root scope:** GTK declares `clicked` an action signal, so the accelerator target is valid. The window-associated accelerator group and swapped `gtk_window_close` callback show no new lifetime defect. Close requests normal helper-window closure through the existing destroy/main-loop/`closed` path—not native ReaderHost cancellation. Mnemonics are distinct in current English labels; Reset has the full tooltip/ATK name. Vertical GtkBox packing with `FALSE, FALSE` does **not** withhold horizontal allocation. Translated labels, actual focus behavior and AT-SPI remain unverified.

**Evidence correction:** the subject is selectable; the notice is wrapped but **not** made selectable. The original selectable plain-text fallback remains intact.

Window/root box, buttons, labels, homogeneous stack, fallback scroller/TextView and WebView requisitions remain unmeasured. Source does not prove a minimum above 440px, so that is a pending gate, not a confirmed defect.

After **all owners release, full review and a new receipt**, parent must measure preferred widths and actual **window, stack and WebView allocations at 430/340**, repeat unchanged wide/narrow fixture color/table/**1,024 CID-pixel** comparisons, and exercise keyboard/Close teardown.

No pixels, blanket security, inline hosting, real MIME/CID, remote consent, accessibility, native cancellation or clipboard acceptance is granted.

**Explicit read-only release to parent. No edits, builds, application launches or real operations performed; other active owners’ source was not reviewed.**
