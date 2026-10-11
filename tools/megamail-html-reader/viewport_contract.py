#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Read-only production-source/header checks. No compiler, display or helper.

This checks chrome wiring and freezes the pre-patch security/document sources.
It does NOT measure GTK requisitions, WebView allocation, pixels or accessibility.
"""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FROZEN = {
    "policy.c": "db56a025ca3254ae67b165f620073e43cf18f68a58816689e349b2ace223a646",
    "policy.h": "d631acfcd2fba802e19dfca1e70f787735ce20e90e2d5bf68fa13f22531dce30",
    "request_policy.c": "d92ef76352341b28ade3dfb091d176fa203de76870f634e6a7860e590110ff30",
    "extension.c": "7507ffc00e46d5338229e62031e98a656185150736c9d5aeb07f1022380528cd",
    "sandbox.c": "d888a7fb744fa7c0c72aa5ac954f6bb05b88d124d9b892ac2cafc9481e965725",
    "sandbox.h": "a81c6516c1fc75c066acfdd88f27b01f8769287b22b6528c18c5a0a821df60dd",
    "fixture_request.py": "e6f24c10562b8aac21b4b5889c73a12208a41610d5e8c57f50c8f2f4b70027c4",
}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def audit():
    main = (ROOT / "main.c").read_text()
    begin = main.index("    r.window = gtk_window_new(")
    end = main.index('    g_signal_connect(r.window, "destroy"')
    chrome = main[begin:end]
    # Everything before/after widget construction is byte-for-byte unchanged:
    # ACK-before-load, scheme/CSP, engine settings, denied actions and teardown.
    require(hashlib.sha256((main[:begin] + main[end:]).encode()).hexdigest()
            == "d102fbaa22114a52d82565496889c42a9e7d47a836ea2eefd11af0af6495c831",
            "non-chrome main.c changed; separately review before updating baseline")
    for name, expected in FROZEN.items():
        require(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected,
                name + " changed; separately review before updating baseline")

    snippets = [
        'gtk_window_set_default_size(GTK_WINDOW(r.window), 680, 600);',
        'toolbar = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);',
        'view_actions = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);',
        'zoom_actions = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);',
        'gtk_box_pack_start(GTK_BOX(toolbar), view_actions, FALSE, FALSE, 0);',
        'gtk_box_pack_start(GTK_BOX(toolbar), zoom_actions, FALSE, FALSE, 0);',
        'gtk_toggle_button_new_with_mnemonic("_Plain text")',
        'r.plain_button = plain;',
        'gtk_box_pack_start(GTK_BOX(view_actions), plain, FALSE, FALSE, 0);',
        'g_signal_connect(plain, "toggled", G_CALLBACK(plain_toggled), &r);',
        'gtk_button_new_with_mnemonic("_Close")',
        'gtk_box_pack_end(GTK_BOX(view_actions), close, FALSE, FALSE, 0);',
        'g_signal_connect_swapped(close, "clicked", G_CALLBACK(gtk_window_close), r.window);',
        'gtk_window_add_accel_group(GTK_WINDOW(r.window), accelerators);',
        'gtk_widget_add_accelerator(close, "clicked", accelerators, GDK_KEY_w, GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);',
        'const char *actions[] = {"out", "reset", "in"};',
        'const char *labels[] = {"Zoom _out", "_Reset", "Zoom _in"};',
        'gtk_button_new_with_mnemonic(labels[i])',
        'const char *zoom_names[] = {"Zoom out", "Reset zoom to 100%", "Zoom in"};',
        'gtk_widget_set_tooltip_text(button, zoom_names[i]);',
        'atk_object_set_name(gtk_widget_get_accessible(button), zoom_names[i]);',
        'gtk_box_pack_start(GTK_BOX(zoom_actions), button, FALSE, FALSE, 0);',
        'g_signal_connect(button, "clicked", G_CALLBACK(zoom), &r);',
        'subject = gtk_label_new(doc->title);',
        'gtk_label_set_selectable(GTK_LABEL(subject), TRUE);',
        'gtk_text_view_set_editable(GTK_TEXT_VIEW(text), FALSE);',
        'gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(text), GTK_WRAP_WORD_CHAR);',
        'gtk_text_buffer_set_text(gtk_text_view_get_buffer(GTK_TEXT_VIEW(text)),',
        '*doc->plain ? doc->plain : "No plain-text alternative was supplied."',
        'gtk_stack_add_named(GTK_STACK(r.stack), scroll, "plain");',
        'gtk_stack_add_named(GTK_STACK(r.stack), GTK_WIDGET(r.view), "original");',
        'gtk_stack_set_visible_child_name(GTK_STACK(r.stack), "plain");',
    ]
    for label in ("subject", "notice"):
        snippets.extend([
            f'gtk_label_set_line_wrap(GTK_LABEL({label}), TRUE);',
            f'gtk_label_set_line_wrap_mode(GTK_LABEL({label}), PANGO_WRAP_WORD_CHAR);',
            f'gtk_label_set_width_chars(GTK_LABEL({label}), 1);',
            f'gtk_label_set_max_width_chars(GTK_LABEL({label}), 60);',
            f'gtk_widget_set_hexpand({label}, TRUE);',
        ])
    for snippet in snippets:
        require(snippet in chrome, "production chrome contract missing: " + snippet)
    require(not any(term in chrome for term in (
        "gtk_label_set_ellipsize", "gtk_label_set_lines", "gtk_widget_set_size_request",
        "gtk_window_set_geometry_hints", "webkit_web_view_set_zoom_level",
        "webkit_user_content", "gtk_scrolled_window_set_policy",
    )), "chrome must not truncate metadata or force document sizing/styles/zoom")

    # Installed declarations only. No compilation, library loading or display.
    gtk_symbols = set(re.findall(r'\b(gtk_[a-z0-9_]+)\s*\(', main))
    atk_symbols = set(re.findall(r'\b(atk_[a-z0-9_]+)\s*\(', main))
    webkit_symbols = set(re.findall(r'\b(webkit_[a-z0-9_]+)\s*\(',
                                  main + (ROOT / "extension.c").read_text()))
    webkit_symbols.discard("webkit_web_extension_initialize_with_user_data")
    for directory, symbols in [
        ("/usr/include/gtk-3.0", gtk_symbols),
        ("/usr/include/atk-1.0", atk_symbols),
        ("/usr/include/webkitgtk-4.1", webkit_symbols),
    ]:
        declarations = "\n".join(p.read_text(errors="replace")
                                  for p in Path(directory).rglob("*.h"))
        missing = sorted(s for s in symbols if not re.search(r'\b' + s + r'\s*\(', declarations))
        require(not missing, "installed declarations absent: " + ", ".join(missing))
    require("PANGO_WRAP_WORD_CHAR" in Path("/usr/include/pango-1.0/pango/pango-layout.h").read_text(),
            "installed Pango word/character wrapping absent")
    print(json.dumps({
        "production_chrome_source_checks": len(snippets) + 2,
        "unchanged_nonchrome_and_frozen_sources": len(FROZEN) + 1,
        "installed_gtk_declarations": len(gtk_symbols),
        "installed_atk_declarations": len(atk_symbols),
        "installed_webkit_declarations": len(webkit_symbols),
        "source_only": True, "compiled": False, "runtime_executed": False,
        "viewport_or_pixels_proven": False, "security_or_a11y_proven": False,
    }, sort_keys=True))


if __name__ == "__main__":
    audit()
