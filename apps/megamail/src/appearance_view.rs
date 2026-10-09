//! Flat mail and appearance settings in Zeron's ordinary-row language.

use std::rc::Rc;

use gpui_kit::base::{Disableable as _, Selectable as _};
use gpui_kit::component::{
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{
    Anchor, AnyElement, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px, relative, rgb,
};

use crate::preferences::Density;
use crate::theme::{AccentPreset, AccentSelection, SurfacePreference, ThemeAppearance, ThemeMode};
use crate::{
    AppearanceChange, MailApp, Palette, PreferenceChange, ThemeChange, appearance, zeron_background,
};

pub(crate) fn render(app: &MailApp, palette: Palette, cx: &mut Context<MailApp>) -> AnyElement {
    let dark = app.dark;
    let can_edit = !app.appearance_busy && !app.appearance_loading && !app.theme_busy;
    let theme_controls_disabled = app.theme_busy || app.appearance_loading || app.appearance_busy;
    let can_edit_effect = can_edit && app.appearance.has_cached_image();
    let can_edit_effect_strength =
        can_edit_effect && app.appearance.effect() != appearance::WallpaperEffect::None;
    let preferences = app.preferences;
    let preferences_disabled = app.preferences_busy || app.preferences_loading;
    let bool_choices = |current| {
        [("On", true), ("Off", false)]
            .into_iter()
            .map(|(label, value)| (label.to_owned(), current == value, value))
            .collect::<Vec<_>>()
    };
    // ponytail: ten-point steps keep the native menu compact; use a SliderState if continuous adjustment is needed.
    let fine_steps = (0..=10).map(|step| step as f32 / 10.0).collect::<Vec<_>>();
    let wallpaper_detail = app
        .appearance
        .wallpaper_name()
        .map(|name| format!("{name} · stored privately; source path not retained"))
        .unwrap_or_else(|| "No image selected".to_owned());

    let mail_controls = v_flex()
        .w_full()
        .child(section_heading("Mail", palette))
        .child(choice_row(
            "Message density",
            Some("Choose the height of each message row."),
            preferences.density.label().to_owned(),
            [Density::Comfortable, Density::Compact]
                .into_iter()
                .map(|density| {
                    (
                        density.label().to_owned(),
                        preferences.density == density,
                        density,
                    )
                })
                .collect(),
            !preferences_disabled,
            "mail-density",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_preference(PreferenceChange::Density(value), cx),
        ))
        .child(choice_row(
            "Group conversations",
            Some("Show replies together in conversation threads."),
            (if preferences.group_conversations {
                "On"
            } else {
                "Off"
            })
            .to_owned(),
            bool_choices(preferences.group_conversations),
            !preferences_disabled,
            "mail-group-conversations",
            palette,
            dark,
            cx,
            |this, value, cx| {
                this.change_preference(PreferenceChange::GroupConversations(value), cx)
            },
        ))
        .child(choice_row(
            "Hide quoted text",
            Some("Collapse a trailing quoted reply beneath the new message."),
            (if preferences.hide_quoted_text {
                "On"
            } else {
                "Off"
            })
            .to_owned(),
            bool_choices(preferences.hide_quoted_text),
            !preferences_disabled,
            "mail-hide-quoted-text",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_preference(PreferenceChange::HideQuotedText(value), cx),
        ))
        .child(choice_row(
            "Reduced motion",
            Some("Reduce motion in menus and transitions."),
            (if preferences.reduced_motion {
                "On"
            } else {
                "Off"
            })
            .to_owned(),
            bool_choices(preferences.reduced_motion),
            !preferences_disabled,
            "mail-reduced-motion",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_preference(PreferenceChange::ReducedMotion(value), cx),
        ))
        .child(choice_row(
            "Starting inbox",
            Some("Choose which inbox opens when MegaMail starts."),
            if preferences.default_unified {
                "Unified inbox".to_owned()
            } else {
                "Account inbox".to_owned()
            },
            [("Unified inbox", true), ("Account inbox", false)]
                .into_iter()
                .map(|(label, value)| {
                    (
                        label.to_owned(),
                        preferences.default_unified == value,
                        value,
                    )
                })
                .collect(),
            !preferences_disabled,
            "mail-default-unified",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_preference(PreferenceChange::DefaultUnified(value), cx),
        ));

    let theme_controls = v_flex()
        .w_full()
        .child(section_heading("Theme", palette))
        .child(settings_row(
            "Appearance",
            Some("Choose how MegaMail follows light and dark changes."),
            mode_control(app.theme.mode(), theme_controls_disabled, palette, dark, cx),
            palette,
        ))
        .child(theme_row(
            app,
            ThemeAppearance::Light,
            theme_controls_disabled,
            palette,
            dark,
            cx,
        ))
        .child(theme_row(
            app,
            ThemeAppearance::Dark,
            theme_controls_disabled,
            palette,
            dark,
            cx,
        ))
        .child(accent_row(app, theme_controls_disabled, palette, dark, cx))
        .child(surface_row(
            app.theme.surface(),
            theme_controls_disabled,
            palette,
            dark,
            cx,
        ))
        .child(theme_library_row(
            app,
            theme_controls_disabled,
            palette,
            dark,
            cx,
        ))
        .children(app.theme.library_entries().into_iter().map(|entry| {
            let id = entry.id.clone();
            settings_row(
                "Imported theme",
                Some(&format!(
                    "{} · {} variants{}",
                    entry.name,
                    entry.variant_count,
                    entry
                        .warning
                        .map(|warning| format!(" · {warning}"))
                        .unwrap_or_default()
                )),
                Button::new(SharedString::from(format!("remove-theme-{}", entry.id)))
                    .custom(super::floating_button_variant(cx, palette, dark))
                    .small()
                    .label("Remove")
                    .accessibility_label(format!("Remove imported theme {}", entry.name))
                    .disabled(theme_controls_disabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.change_theme(ThemeChange::Remove(id.clone()), window, cx)
                    })),
                palette,
            )
        }));

    let background_controls = v_flex()
        .w_full()
        .child(section_heading("Background", palette))
        .child(background_row(app, can_edit, palette, dark, cx))
        .child(settings_row(
            "Wallpaper image",
            Some(&wallpaper_detail),
            Button::new("choose-wallpaper")
                .custom(super::floating_button_variant(cx, palette, dark))
                .small()
                .icon(gpui_kit::component::Icon::new(
                    gpui_kit::assets::IconName::Image,
                ))
                .label(if app.appearance_busy {
                    "Working…"
                } else {
                    "Choose image"
                })
                .disabled(!can_edit)
                .on_click(cx.listener(|this, _, _, cx| this.choose_wallpaper(cx))),
            palette,
        ))
        .child(choice_row(
            "Image treatment",
            Some(if !app.appearance.has_cached_image() {
                "Choose a background or image to enable treatments."
            } else if can_edit_effect {
                "Dither, ASCII, halftone, or scanlines."
            } else {
                "Wait for the current background change to finish."
            }),
            app.appearance.effect().label().to_owned(),
            appearance::WallpaperEffect::ALL
                .into_iter()
                .map(|effect| {
                    (
                        effect.label().to_owned(),
                        effect == app.appearance.effect(),
                        effect,
                    )
                })
                .collect(),
            can_edit_effect,
            "appearance-effect",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_appearance(AppearanceChange::Effect(value), cx),
        ))
        .child(percent_row(
            "Treatment strength",
            "Blend the treatment with the original background.",
            app.appearance.effect_strength(),
            &fine_steps,
            can_edit_effect_strength,
            "effect-strength",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_appearance(AppearanceChange::EffectStrength(value), cx),
        ))
        .child(percent_row(
            "Wallpaper strength",
            "Set how strongly the wallpaper shows through.",
            app.appearance.opacity(),
            &fine_steps,
            can_edit,
            "wallpaper-opacity",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_appearance(AppearanceChange::Opacity(value), cx),
        ))
        .child(choice_row(
            "Wallpaper blur",
            Some("Soften the image beneath the mail panes."),
            blur_label(app.appearance.blur_sigma()),
            [0.0_f32, 10.0, 16.0]
                .into_iter()
                .map(|sigma| {
                    (
                        blur_label(sigma),
                        (app.appearance.blur_sigma() - sigma).abs() < 0.1,
                        sigma,
                    )
                })
                .collect(),
            can_edit_effect,
            "wallpaper-blur",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_appearance(AppearanceChange::Blur(value), cx),
        ))
        .child(percent_row(
            "Bottom fade",
            "Blend the wallpaper into the reading canvas at the bottom edge.",
            app.appearance.fade(),
            &fine_steps,
            can_edit,
            "background-fade",
            palette,
            dark,
            cx,
            |this, value, cx| this.change_appearance(AppearanceChange::Fade(value), cx),
        ));

    let snapshot = app.live.read(cx).snapshot();
    let accounts = v_flex()
        .w_full()
        .child(section_heading("Accounts", palette))
        .children(snapshot.accounts.iter().map(|account| {
            let id = account.id;
            settings_row(
                account.name.clone(),
                Some(&account.email),
                Button::new(SharedString::from(format!("settings-account-{id}")))
                    .custom(super::floating_button_variant(cx, palette, dark))
                    .small()
                    .label("Open inbox")
                    .disabled(app.composer.is_some())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.live.update(cx, |live, cx| live.switch_account(id, cx));
                        this.view = super::View::Mailbox;
                        cx.notify();
                    })),
                palette,
            )
        }))
        .child(settings_row(
            "Add accounts",
            Some("Import Thunderbird mailboxes or connect Gmail and IMAP."),
            Button::new("settings-add-accounts")
                .custom(super::floating_button_variant(cx, palette, dark))
                .small()
                .label("Add or import")
                .on_click(cx.listener(|this, _, window, cx| this.open_setup(window, cx))),
            palette,
        ));
    let shortcuts = v_flex()
        .w_full()
        .child(section_heading("Keyboard shortcuts", palette))
        .children(
            [
                ("New message", "Ctrl+N"),
                ("Reply", "Ctrl+R"),
                ("Reply all", "Ctrl+Shift+R"),
                ("Search mail", "Ctrl+F"),
                ("All inboxes", "Ctrl+Shift+L"),
                ("Refresh mail", "F5"),
                ("Move through the message list", "↑ / ↓"),
            ]
            .into_iter()
            .map(|(name, key)| {
                settings_row(
                    name,
                    None,
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child(key),
                    palette,
                )
            }),
        );

    let content = v_flex()
        .w_full()
        .max_w(px(920.))
        .gap_5()
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Settings"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(relative(1.4))
                        .text_color(palette.muted)
                        .child("Mail, themes, backgrounds, and surface materials."),
                ),
        )
        .child(mail_controls)
        .when_some(app.preferences_error.clone(), |column, error| {
            column.child(super::error_banner("Mail preferences", &error, cx, palette))
        })
        .when(app.preferences_busy || app.preferences_loading, |column| {
            column.child(
                div()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(palette.muted)
                    .child(if app.preferences_loading {
                        "Loading saved mail preferences…"
                    } else {
                        "Saving mail preferences…"
                    }),
            )
        })
        .child(theme_controls)
        .child(background_controls)
        .child(accounts)
        .child(shortcuts)
        .when_some(app.theme_error.clone(), |column, error| {
            column.child(super::error_banner("Theme update", &error, cx, palette))
        })
        .when_some(app.theme_status.clone(), |column, status| {
            column.child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(palette.muted)
                    .child(gpui_kit::component::Icon::new(
                        gpui_kit::assets::IconName::Check,
                    ))
                    .child(status),
            )
        })
        .when_some(app.appearance_error.clone(), |column, error| {
            column.child(super::error_banner(
                "Background update",
                &error,
                cx,
                palette,
            ))
        })
        .when(
            app.theme_busy || app.appearance_busy || app.appearance_loading,
            |column| {
                column.child(
                    div()
                        .py_2()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .child(if app.appearance_loading {
                            "Loading saved appearance…"
                        } else if app.theme_busy {
                            "Saving theme preferences…"
                        } else if app.appearance_busy {
                            "Processing wallpaper…"
                        } else {
                            "Saving appearance…"
                        }),
                )
            },
        );

    v_flex()
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
        .bg(app.pane_surface(palette.background, 0.05))
        .child(
            v_flex()
                .id("appearance-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .items_center()
                .px_6()
                .py_6()
                .child(content),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .justify_end()
                .items_center()
                .border_t_1()
                .border_color(palette.border)
                .px_6()
                .py_3()
                .child(
                    Button::new("appearance-done")
                        .custom(super::floating_button_variant(cx, palette, dark))
                        .small()
                        .label("Back to mail")
                        .disabled(app.appearance_busy || app.appearance_loading || app.theme_busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.view = this.appearance_return_view;
                            cx.notify();
                        })),
                ),
        )
        .into_any_element()
}

fn section_heading(label: &'static str, palette: Palette) -> impl IntoElement {
    div()
        .pt_3()
        .pb_1()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.text)
        .child(label)
}

fn settings_row(
    title: impl Into<SharedString>,
    detail: Option<&str>,
    control: impl IntoElement,
    palette: Palette,
) -> gpui_kit::Div {
    h_flex()
        .w_full()
        .min_h(px(56.))
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .border_b_1()
        .border_color(palette.border)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text)
                        .child(title.into()),
                )
                .when_some(detail, |column, detail| {
                    column.child(
                        div()
                            .text_size(px(12.))
                            .line_height(relative(1.35))
                            .text_color(palette.muted)
                            .child(detail.to_owned()),
                    )
                }),
        )
        .child(div().flex_none().max_w(px(410.)).child(control))
}

fn mode_control(
    current: ThemeMode,
    disabled: bool,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    h_flex().gap_1().children(
        [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark].map(|mode| {
            Button::new(SharedString::from(format!(
                "appearance-mode-{}",
                mode.label()
            )))
            .custom(super::floating_button_variant(cx, palette, dark))
            .small()
            .selected(current == mode)
            .label(mode.label())
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.change_theme(ThemeChange::Mode(mode), window, cx)
            }))
        }),
    )
}

fn theme_row(
    app: &MailApp,
    appearance: ThemeAppearance,
    disabled: bool,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let (label, id, selected_id) = match appearance {
        ThemeAppearance::Light => ("Light theme", "light", app.theme.selection().light.as_str()),
        ThemeAppearance::Dark => ("Dark theme", "dark", app.theme.selection().dark.as_str()),
    };
    let choices = app.theme.variants(appearance);
    let selected = choices.iter().find(|choice| choice.id == selected_id);
    let preview = selected.map(|choice| {
        palette_preview(choice.background, choice.text, choice.accent).into_any_element()
    });
    let value_label = selected
        .map(|choice| choice.name.clone())
        .unwrap_or_else(|| "Choose a theme".to_owned());
    let weak = cx.entity().downgrade();
    let choices_for_menu = choices.clone();
    let selected_id = selected_id.to_owned();
    let row = h_flex()
        .items_center()
        .gap_1()
        .when_some(preview, |row, preview| row.child(preview))
        .child(
            Button::new(SharedString::from(format!("theme-select-{id}")))
                .custom(super::floating_button_variant(cx, palette, dark))
                .small()
                .icon(gpui_kit::component::Icon::new(
                    gpui_kit::assets::IconName::ChevronDown,
                ))
                .label(value_label)
                .accessibility_label(format!("Select {label}"))
                .disabled(disabled)
                .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
                    choices_for_menu.iter().fold(
                        menu.min_w(px(270.)).max_h(px(380.)).scrollable(true),
                        |menu, choice| {
                            let id = choice.id.clone();
                            let name = choice.name.clone();
                            let family = choice.family_name.clone();
                            let accessible_name = format!("{name}, {family}");
                            let bg = choice.background;
                            let text = choice.text;
                            let accent = choice.accent;
                            let callback = weak.clone();
                            menu.item(
                                PopupMenuItem::element(move |_, _| {
                                    h_flex()
                                        .id(SharedString::from(format!(
                                            "theme-option-{accessible_name}"
                                        )))
                                        .w_full()
                                        .items_center()
                                        .gap_2()
                                        .aria_label(accessible_name.clone())
                                        .child(palette_preview(bg, text, accent))
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap(px(1.))
                                                .child(div().text_size(px(12.)).child(name.clone()))
                                                .child(
                                                    div()
                                                        .text_size(px(9.5))
                                                        .text_color(palette.muted)
                                                        .child(family.clone()),
                                                ),
                                        )
                                })
                                .checked(id == selected_id)
                                .on_click(
                                    move |_, window, app| {
                                        let change = ThemeChange::Variant(appearance, id.clone());
                                        let _ = callback.update(app, |this, cx| {
                                            this.change_theme(change, window, cx)
                                        });
                                    },
                                ),
                            )
                        },
                    )
                }),
        );
    settings_row(
        label,
        Some("Choose independently for this appearance."),
        row,
        palette,
    )
}

fn palette_preview(background: Hsla, text: Hsla, accent: Hsla) -> impl IntoElement {
    h_flex()
        .flex_none()
        .w(px(36.))
        .h(px(18.))
        .overflow_hidden()
        .rounded(px(4.))
        .border_1()
        .border_color(text.opacity(0.18))
        .child(div().flex_1().h_full().bg(background))
        .child(div().flex_1().h_full().bg(text))
        .child(div().flex_1().h_full().bg(accent))
}

fn accent_row(
    app: &MailApp,
    disabled: bool,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let current = app.theme.accent();
    let model_appearance = if app.dark {
        zeron_theme::Appearance::Dark
    } else {
        zeron_theme::Appearance::Light
    };
    let theme_default = app
        .theme
        .variants(if app.dark {
            ThemeAppearance::Dark
        } else {
            ThemeAppearance::Light
        })
        .into_iter()
        .find(|choice| {
            let selected_id = if app.dark {
                &app.theme.selection().dark
            } else {
                &app.theme.selection().light
            };
            &choice.id == selected_id
        })
        .map(|choice| choice.accent)
        .unwrap_or(palette.accent);
    let mut swatches = vec![(AccentSelection::ThemeDefault, theme_default)];
    swatches.extend(AccentPreset::ALL.into_iter().map(|preset| {
        let color = preset.color(model_appearance);
        (
            AccentSelection::Preset(preset),
            rgb((u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)).into(),
        )
    }));
    settings_row(
        "Accent color",
        Some("Theme default keeps each palette's authored accent."),
        h_flex()
            .items_center()
            .gap_1()
            .children(swatches.into_iter().map(|(selection, color)| {
                let selected = current == selection;
                let sample = if selection == AccentSelection::ThemeDefault {
                    h_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .gap(px(2.))
                        .child(
                            div()
                                .w(px(3.))
                                .h(px(10.))
                                .rounded_full()
                                .bg(color.opacity(0.7)),
                        )
                        .child(div().w(px(3.)).h(px(14.)).rounded_full().bg(color))
                        .child(
                            div()
                                .w(px(3.))
                                .h(px(8.))
                                .rounded_full()
                                .bg(color.opacity(0.55)),
                        )
                        .into_any_element()
                } else {
                    div()
                        .size_full()
                        .rounded(px(5.))
                        .bg(color)
                        .into_any_element()
                };
                Button::new(SharedString::from(format!(
                    "accent-{}",
                    selection.label().to_lowercase().replace(' ', "-")
                )))
                .custom(super::floating_button_variant(cx, palette, dark))
                .small()
                .w(px(26.))
                .h(px(26.))
                .selected(selected)
                .when(selected, |button| {
                    button.shadow(vec![crate::zeron_style::floating_selection_ring(dark)])
                })
                .accessibility_label(selection.label())
                .tooltip(selection.label())
                .disabled(disabled)
                .child(sample)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.change_theme(ThemeChange::Accent(selection), window, cx)
                }))
            })),
        palette,
    )
}

fn surface_row(
    current: SurfacePreference,
    disabled: bool,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    settings_row(
        "Surface",
        Some("Keep the theme recommendation or choose a material."),
        h_flex()
            .gap_1()
            .children(SurfacePreference::ALL.into_iter().map(|surface| {
                Button::new(SharedString::from(format!(
                    "surface-{}",
                    surface.label().to_lowercase().replace(' ', "-")
                )))
                .custom(super::floating_button_variant(cx, palette, dark))
                .small()
                .selected(current == surface)
                .label(surface.label())
                .disabled(disabled)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.change_theme(ThemeChange::Surface(surface), window, cx)
                }))
            })),
        palette,
    )
}

fn theme_library_row(
    app: &MailApp,
    disabled: bool,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let entry_count = app.theme.library_entries().len();
    h_flex()
        .w_full()
        .min_h(px(56.))
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .border_b_1()
        .border_color(palette.border)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("Theme library"),
                )
                .child(div().text_size(px(12.)).text_color(palette.muted).child(
                    if entry_count == 0 {
                        "Import a local VS Code theme or extension.".to_owned()
                    } else {
                        format!("{entry_count} imported theme source(s)")
                    },
                )),
        )
        .child(
            h_flex()
                .gap_1()
                .child(
                    Button::new("import-local-theme")
                        .custom(super::floating_button_variant(cx, palette, dark))
                        .small()
                        .icon(gpui_kit::component::Icon::new(
                            gpui_kit::assets::IconName::FolderOpen,
                        ))
                        .label(if app.theme_busy {
                            "Importing…"
                        } else {
                            "Import theme"
                        })
                        .disabled(disabled)
                        .on_click(cx.listener(|this, _, _, cx| this.import_theme(cx))),
                )
                .child(
                    Button::new("reset-theme")
                        .custom(super::floating_button_variant(cx, palette, dark))
                        .small()
                        .label("Reset")
                        .disabled(disabled)
                        .on_click(cx.listener(|this, _, window, cx| this.reset_theme(window, cx))),
                ),
        )
}

fn background_row(
    app: &MailApp,
    enabled: bool,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
) -> impl IntoElement {
    let current = app.appearance.preset();
    let choices = {
        let mut presets = appearance::AppearancePreset::ALL.to_vec();
        if app.appearance.has_wallpaper() {
            presets.push(appearance::AppearancePreset::Custom);
        }
        presets
    };
    let weak = cx.entity().downgrade();
    let dropdown = h_flex()
        .items_center()
        .gap_2()
        .child(background_strip(app, palette, dark))
        .child(
            Button::new("appearance-background")
                .custom(super::floating_button_variant(cx, palette, dark))
                .small()
                .icon(gpui_kit::component::Icon::new(
                    gpui_kit::assets::IconName::ChevronDown,
                ))
                .label(current.label())
                .accessibility_label("Select background")
                .disabled(!enabled)
                .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
                    choices.iter().fold(menu, |menu, preset| {
                        let preset = *preset;
                        let callback = weak.clone();
                        menu.item(
                            PopupMenuItem::new(preset.label())
                                .checked(preset == current)
                                .on_click(move |_, _, app| {
                                    let _ = callback.update(app, |this, cx| {
                                        this.change_appearance(AppearanceChange::Preset(preset), cx)
                                    });
                                }),
                        )
                    })
                }),
        );
    settings_row(
        "Background",
        Some("Select a built-in gradient or your custom image."),
        dropdown,
        palette,
    )
}

fn choice_row<T: Copy + 'static>(
    title: &'static str,
    detail: Option<&'static str>,
    current_label: String,
    choices: Vec<(String, bool, T)>,
    enabled: bool,
    id: &'static str,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
    apply: impl Fn(&mut MailApp, T, &mut Context<MailApp>) + 'static,
) -> impl IntoElement {
    let weak = cx.entity().downgrade();
    let apply = Rc::new(apply);
    let choices = choices
        .into_iter()
        .map(|(label, selected, value)| (label, selected, value, weak.clone()))
        .collect::<Vec<_>>();
    let control = Button::new(SharedString::from(id))
        .custom(super::floating_button_variant(cx, palette, dark))
        .small()
        .icon(gpui_kit::component::Icon::new(
            gpui_kit::assets::IconName::ChevronDown,
        ))
        .label(current_label)
        .disabled(!enabled)
        .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
            choices
                .iter()
                .fold(menu, |menu, (label, selected, value, callback)| {
                    let value = *value;
                    let callback = callback.clone();
                    let apply = apply.clone();
                    menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(*selected)
                            .on_click(move |_, _, app| {
                                let _ = callback.update(app, |this, cx| apply(this, value, cx));
                            }),
                    )
                })
        });
    settings_row(title, detail, control, palette)
}

fn percent_row(
    title: &'static str,
    detail: &'static str,
    current: f32,
    values: &[f32],
    enabled: bool,
    id: &'static str,
    palette: Palette,
    dark: bool,
    cx: &mut Context<MailApp>,
    apply: impl Fn(&mut MailApp, f32, &mut Context<MailApp>) + 'static,
) -> impl IntoElement {
    let choices = values
        .iter()
        .copied()
        .map(|value| {
            let label = if value == 0.0 {
                "Off".to_owned()
            } else {
                format!("{}%", (value * 100.0) as u32)
            };
            (label, (value - current).abs() < 0.01, value)
        })
        .collect();
    let current_label = if current <= 0.001 {
        "Off".to_owned()
    } else {
        format!("{}%", (current * 100.0).round() as u32)
    };
    choice_row(
        title,
        Some(detail),
        current_label,
        choices,
        enabled,
        id,
        palette,
        dark,
        cx,
        apply,
    )
}

fn blur_label(sigma: f32) -> String {
    if sigma == 0.0 {
        "Sharp".to_owned()
    } else {
        format!("Blur {}", sigma as u32)
    }
}

fn background_strip(app: &MailApp, palette: Palette, dark: bool) -> impl IntoElement {
    div()
        .relative()
        .flex_none()
        .w(px(88.))
        .h(px(36.))
        .overflow_hidden()
        .rounded(px(5.))
        .border_1()
        .border_color(palette.border)
        .child(zeron_background::background_element(
            &app.appearance,
            dark,
            palette.accent,
            palette.background,
        ))
}
