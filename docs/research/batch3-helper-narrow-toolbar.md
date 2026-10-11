# Batch 3: helper narrow chrome (source only)

**PATCHED, NOT PIXEL-VERIFIED.** Exclusive files: `tools/megamail-html-reader/main.c`, new `viewport_contract.py`, this report. All released to parent. No build/compiler, helper runtime, display, network, installation, device or profile action occurred.

Read AGENTS, DESIGN, root/helper READMEs, architecture and prior fixture report. Graft query returned no nodes. Impeccable context ran; its native-Linux classification warning was not repaired outside ownership. Adapt/craft guidance and existing 500px capture informed the change; source detector returned `[]` (not native visual validation).

## Requisition contributors

Installed primary source: `/usr/share/gir-1.0/Gtk-3.0.gir`, Widget geometry-management/get_preferred_width, Box, Label text-layout, Stack and ScrolledWindow documentation; GTK headers 3.24.52, WebKitGTK headers 2.54.1.

GTK recursively propagates child preferred widths to the toplevel, then requests height at that width. Default 680×600 is not a minimum override.

| Contributor in production main.c | Source assessment/change |
| --- | --- |
| Window / vertical root box | Default dimensions unchanged; vertical siblings share width. Decorations remain toolkit/WM-owned. |
| Toolbar / buttons | Old horizontal row added four full-label requests, 3×8px spacing and 2×12px border: consistent with observed 455px clamp, not individually measured. Now two rows: Plain text/Close, then Zoom out/Reset/Zoom in. |
| Subject and every notice state | Wrapping alone was insufficient evidence. Both now use minimum `width_chars=1`, natural `max_width_chars=60`, WORD_CHAR wrapping and horizontal expansion. Installed Label documentation explicitly says wrapped labels rewrap to all available width. No text truncation/ellipsis or hidden warning. |
| Stack / fallback scroller / TextView / WebView | Homogeneous stack can include both pages' requisitions. Existing scrollable, word/character-wrapped selectable fallback and direct WebView remain unchanged. Their actual minimums are unmeasured; no evidence justifies a document-widget workaround. |

Plain/Close/zoom have mnemonics, native button focus; Close also Ctrl+W. Reset retains a full tooltip/ATK name. AT-SPI acceptance remains blocked.

## Checks and parent acceptance

Source-only commands `viewport_contract.py` and existing `tests.py` passed: 44 chrome checks, 8 unchanged-source fingerprints, 43 GTK/1 ATK/63 WebKit declarations; existing 15 checks/63 WebKit symbols. No authored runtime test executed. Header presence is not compilation or ABI proof.

Parent must measure preferred widths and actual **window, stack and WebView** allocations, then verify unchanged fixture at 430/340 and wide sizes. Do not equate window width with CSS viewport. Prior exact backgrounds/CID-pixel oracle and receipts remain in `batch2-helper-fixture-visual-runtime.md`; ≤440 stacking remains unverified. No breakpoint/CSS/resource mutation.

Non-chrome main bytes and policy/extension/sandbox/fixture fingerprints are unchanged: token ACK-before-load, CSP, disabled active content and containment preserved. No gate reopening/security proof. Inline hosting, real MIME/CID transport, remote resources, GPUI hosting, accessibility and native-host cancellation acceptance remain blocked.
