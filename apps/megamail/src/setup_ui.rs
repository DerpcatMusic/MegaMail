//! Account setup and provider controls.
use crate::*;

impl MailApp {
    pub(crate) fn provider_controls(
        &self,
        setup: &SetupState,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use megamail_core::provider_auth::{MicrosoftAudience as Audience, Provider};
        let admission_reason = self.live.read(cx).direct_add_block_reason();
        let busy = self.demo
            || self.loading_profiles
            || self.startup_error.is_some()
            || admission_reason.is_some()
            || setup.connecting
            || setup.discovering
            || setup.thunderbird_loading
            || self.live.read(cx).snapshot().thunderbird_loading;
        let google = self
            .auth_availability
            .as_ref()
            .is_some_and(|a| a.google.is_ok());
        let microsoft = self
            .auth_availability
            .as_ref()
            .is_some_and(|a| a.microsoft.is_ok());
        v_flex().gap_2()
            .child(h_flex().flex_wrap().gap_2()
                .child(Button::new("google-browser-signin").secondary().small().label("Google · browser sign-in (IMAP/SMTP)").disabled(busy || !google)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_provider_authorization(Provider::GoogleImapSmtp, cx))))
                .child(Button::new("microsoft-browser-signin").secondary().small().label("Microsoft · browser sign-in (Graph)").disabled(busy || !microsoft)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_provider_authorization(Provider::MicrosoftGraph(this.microsoft_audience), cx)))))
            .child(Button::new("microsoft-audience").ghost().small().label(match self.microsoft_audience {
                Audience::Personal => "Microsoft audience: personal", Audience::WorkOrSchool => "Microsoft audience: work/school", Audience::PersonalAndWork => "Microsoft audience: personal + work/school",
            }).disabled(busy).on_click(cx.listener(|this, _, _, cx| {
                this.microsoft_audience = match this.microsoft_audience { Audience::Personal => Audience::WorkOrSchool, Audience::WorkOrSchool => Audience::PersonalAndWork, Audience::PersonalAndWork => Audience::Personal }; cx.notify();
            })))
            .when(!google || !microsoft, |el| el.child(div().text_size(px(11.)).text_color(palette.muted).child("Browser sign-in is disabled without a configured MegaMail-owned desktop registration. App-password/manual IMAP and the separate Thunderbird runtime remain alternatives. Graph is not Exchange IMAP.")))
            .when_some(admission_reason, |el, reason| el.child(div().text_size(px(11.)).text_color(palette.muted).child(reason)))
            .when(self.auth_inflight, |el| el.child(Button::new("cancel-provider-signin").ghost().small().label("Cancel browser sign-in").on_click(cx.listener(|this, _, _, cx| this.cancel_provider_authorization(cx)))))
            .into_any_element()
    }

    pub(crate) fn setup_view(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(setup) = self.setup.as_ref() else {
            return v_flex().into_any_element();
        };
        let snapshot = self.live.read(cx).snapshot();
        let has_mail_accounts = !snapshot.accounts.is_empty();
        let thunderbird_waiting_for_mail = !snapshot.thunderbird_results.is_empty()
            && snapshot
                .thunderbird_results
                .iter()
                .all(|result| result.state == ThunderbirdAccountState::Connected)
            && !snapshot.accounts.is_empty()
            && snapshot.folders.is_empty()
            && snapshot.page.is_empty();
        let existing_emails = snapshot
            .accounts
            .iter()
            .map(|account| account.email.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let email = setup.inputs.email.read(cx).value().to_string();
        let candidate = setup.candidate();
        let settings_need_review = setup.manual
            || candidate.is_none()
            || candidate.is_some_and(|candidate| candidate.requires_review());
        let settings_reviewed = setup.review_is_current(cx);
        let guide = app_passwords::guide(
            &candidate.map(|c| c.incoming.host.clone()).unwrap_or_default(),
            &email,
        );
        let password_label: &'static str = match guide.filter(|g| g.blocked.is_none()).map(|g| g.provider) {
            Some("Google") => "Google app password",
            Some("Yahoo") => "Yahoo app password",
            Some("AOL") => "AOL app password",
            Some("iCloud") => "iCloud app-specific password",
            Some("Fastmail") => "Fastmail app password",
            Some("Zoho") => "Zoho app password",
            _ => "Password or app password",
        };
        let password_accessible_label = password_label.to_owned();
        let username_suggestion = candidate
            .map(|candidate| candidate.incoming.username.default_for(&email))
            .unwrap_or_else(|| email.clone());
        let incoming_placeholder = candidate.map_or("imap.example.com".to_owned(), |candidate| {
            candidate.incoming.host.clone()
        });
        let incoming_port_placeholder = candidate.map_or("993".to_owned(), |candidate| {
            candidate.incoming.port.to_string()
        });
        let outgoing_placeholder = candidate.map_or("smtp.example.com".to_owned(), |candidate| {
            candidate.outgoing.host.clone()
        });
        let outgoing_port_placeholder = candidate.map_or("587".to_owned(), |candidate| {
            candidate.outgoing.port.to_string()
        });
        v_flex()
            .flex_1()
            .min_h_0()
            .bg(self.pane_surface(palette.background, 0.05))
            .child(
                v_flex()
                    .id("setup-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .items_center()
                    .px_6()
                    .py_7()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(600.))
                            .gap_5()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Icon::new(IconName::Mail)
                                            .size(px(22.))
                                            .text_color(palette.accent),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(22.))
                                            .line_height(relative(1.2))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(if has_mail_accounts {
                                                "Add another mailbox"
                                            } else {
                                                "Bring your mail"
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .max_w(px(520.))
                                    .text_size(px(13.))
                                    .line_height(relative(1.45))
                                    .text_color(palette.muted)
                                    .child(if has_mail_accounts {
                                        "Import another Thunderbird mailbox or connect a provider with IMAP and SMTP settings."
                                    } else {
                                        "Import your Thunderbird mailboxes in one pass, or connect another provider with IMAP and SMTP settings."
                                    }),
                            )
                            .when_some(self.startup_error.clone(), |row, error| {
                                row.child(error_banner("Profile storage", &error, cx, palette))
                            })
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(field_label("From Thunderbird", palette))
                                    .child(
                                        Button::new("import-thunderbird-settings")
                                            .ghost()
                                            .small()
                                            .icon(Icon::new(if setup.thunderbird_loading || snapshot.thunderbird_loading { IconName::LoaderCircle } else { IconName::Download }))
                                            .label(if setup.thunderbird_loading { "Reading Thunderbird…" } else if snapshot.thunderbird_loading { "Connecting Thunderbird…" } else { "Find mail accounts" })
                                            .disabled(setup.thunderbird_loading || setup.discovering || setup.connecting || snapshot.thunderbird_loading)
                                            .on_click(cx.listener(|this, _, _, cx| this.import_thunderbird_settings(cx))),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(540.))
                                            .text_size(px(11.))
                                            .line_height(relative(1.4))
                                            .text_color(palette.muted)
                                            .child("Use the sign-in already stored in Thunderbird. MegaMail imports selected mailboxes through an isolated profile and never displays or logs passwords or OAuth tokens."),
                                    ),
                            )
                            .when(!setup.thunderbird_profiles.is_empty(), |row| {
                                row.child(thunderbird_picker(
                                    &setup.thunderbird_profiles,
                                    &setup.selected_thunderbird,
                                    &snapshot.thunderbird_results,
                                    &existing_emails,
                                    setup.thunderbird_loading
                                        || setup.connecting
                                        || snapshot.thunderbird_loading,
                                    palette,
                                    cx,
                                ))
                            })
                            .when_some(snapshot.thunderbird_error.clone(), |row, error| {
                                row.child(error_banner(
                                    "Thunderbird access",
                                    &error,
                                    cx,
                                    palette,
                                ))
                            })
                            .when_some(snapshot.thunderbird_notice.clone(), |row, notice| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(relative(1.35))
                                        .text_color(palette.muted)
                                        .child(notice),
                                )
                            })
                            .when(thunderbird_waiting_for_mail, |row| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(relative(1.35))
                                        .text_color(palette.muted)
                                        .child("Thunderbird sign-in is ready. Loading folders and messages…"),
                                )
                            })
                            .when(
                                setup.thunderbird_profiles.is_empty()
                                    && !snapshot.thunderbird_results.is_empty(),
                                |row| {
                                    row.child(thunderbird_results_panel(
                                        &snapshot.thunderbird_results,
                                        palette,
                                    ))
                                },
                            )
                            .when_some(setup.thunderbird_error.clone(), |row, error| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(error),
                                )
                            })
                            .child(div().h(px(1.)).w_full().bg(palette.border))
                            .child(field_label("Add another mailbox", palette))
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(field_label("Email address", palette))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Input::new(&setup.inputs.email)
                                                    .content_type(InputContentType::EmailAddress)
                                                    .aria_label("Email address")
                                                    .disabled(setup.discovering || setup.connecting)
                                                    .flex_1(),
                                            )
                                            .child(
                                                Button::new("discover-account")
                                                    .secondary()
                                                    .small()
                                                    .icon(Icon::new(IconName::Search))
                                                    .label(if setup.discovering { "Looking…" } else { "Find settings" })
                                                    .disabled(setup.discovering || setup.connecting)
                                                    .on_click(cx.listener(|this, _, _, cx| this.start_discovery(cx))),
                                            ),
                                    ),
                            )
                            .when_some(setup.discovery_error.clone(), |row, error| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(palette.muted)
                                        .child(error),
                                )
                            })
                            .child(self.provider_controls(setup, palette, cx))
                            .when_some(setup.report.as_ref(), |row, report| {
                                row.child(candidate_picker(report, setup.selected_candidate, cx))
                            })
                            .when(!setup.manual && setup.report.is_none(), |row| {
                                row.child(
                                    Button::new("show-manual-settings")
                                        .ghost()
                                        .small()
                                        .label("Enter server settings manually")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(setup) = this.setup.as_mut() {
                                                setup.manual = true;
                                                setup.warning_reviewed = false;
                                                setup.reviewed_settings = None;
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when_some(candidate, |row, candidate| {
                                row.child(candidate_summary(candidate, palette))
                                    .when_some(guide, |row, guide| {
                                        row.child(app_password_card(guide, palette, cx))
                                    })
                                    .when(guide.is_none() && candidate.oauth_advertised, |row| {
                                        row.child(
                                            div()
                                                .text_size(px(11.))
                                                .line_height(relative(1.4))
                                                .text_color(palette.muted)
                                                .child("This provider prefers browser sign-in. If it also allows app passwords, create one in its security settings and paste it below."),
                                        )
                                    })
                            })
                            .when(setup.candidate().is_some(), |row| {
                                row.child(
                                    Button::new("toggle-manual-settings")
                                        .ghost()
                                        .small()
                                        .label(if setup.manual { "Use discovered settings" } else { "Edit server settings" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(setup) = this.setup.as_mut() {
                                                setup.manual = !setup.manual;
                                                setup.reviewed_settings = None;
                                                setup.warning_reviewed = if setup.manual {
                                                    false
                                                } else {
                                                    !setup.candidate().is_some_and(|candidate| candidate.requires_review())
                                                };
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when(setup.manual, |row| {
                                row.child(
                                    v_flex()
                                        .gap_4()
                                        .border_t_1()
                                        .border_color(palette.border)
                                        .pt_4()
                                        .child(
                                            div()
                                                .text_size(px(14.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Server settings"),
                                        )
                                        .child(
                                            h_flex()
                                                .items_start()
                                                .gap_4()
                                                .child(server_fields(
                                                    "Incoming · IMAP",
                                                    &setup.inputs.imap_host,
                                                    &setup.inputs.imap_port,
                                                    &incoming_placeholder,
                                                    &incoming_port_placeholder,
                                                    setup.imap_tls,
                                                    true,
                                                    palette,
                                                    cx,
                                                ))
                                                .child(server_fields(
                                                    "Outgoing · SMTP",
                                                    &setup.inputs.smtp_host,
                                                    &setup.inputs.smtp_port,
                                                    &outgoing_placeholder,
                                                    &outgoing_port_placeholder,
                                                    setup.smtp_tls,
                                                    false,
                                                    palette,
                                                    cx,
                                                )),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(field_label("SMTP authentication", palette))
                                                .child(
                                                    Button::new("smtp-auth-password")
                                                        .ghost()
                                                        .small()
                                                        .selected(setup.smtp_auth == megamail_core::discovery::AuthMethod::Password)
                                                        .label("Password")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(setup) = this.setup.as_mut() {
                                                                setup.smtp_auth = megamail_core::discovery::AuthMethod::Password;
                                                                setup.reviewed_settings = None;
                                                                cx.notify();
                                                            }
                                                        })),
                                                )
                                                .child(
                                                    Button::new("smtp-auth-none")
                                                        .ghost()
                                                        .small()
                                                        .selected(setup.smtp_auth == megamail_core::discovery::AuthMethod::None)
                                                        .label("No login")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(setup) = this.setup.as_mut() {
                                                                setup.smtp_auth = megamail_core::discovery::AuthMethod::None;
                                                                setup.smtp_credentials_expanded = false;
                                                                setup.reviewed_settings = None;
                                                                cx.notify();
                                                            }
                                                        })),
                                                ),
                                        ),
                                )
                            })
                            .when(setup.manual || candidate.is_some(), |row| row.child(
                                v_flex()
                                    .gap_3()
                                    .border_t_1()
                                    .border_color(palette.border)
                                    .pt_4()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Sign in"),
                                    )
                                    .child(labeled_input(
                                        "Display name",
                                        Input::new(&setup.inputs.name)
                                            .content_type(InputContentType::Name)
                                            .aria_label("Display name"),
                                        palette,
                                    ))
                                    .child(labeled_input(
                                        "Username",
                                        Input::new(&setup.inputs.username)
                                            .content_type(InputContentType::Username)
                                            .aria_label("Mail server username"),
                                        palette,
                                    ))
                                    .child(labeled_input(
                                        "SMTP username",
                                        Input::new(&setup.inputs.smtp_username)
                                            .content_type(InputContentType::Username)
                                            .aria_label("SMTP username; leave blank to use the IMAP username"),
                                        palette,
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(palette.faint)
                                            .child(format!("Suggested username: {username_suggestion}")),
                                    )
                                    .child(labeled_input(
                                        password_label,
                                            Input::new(&setup.inputs.password)
                                            .content_type(InputContentType::Password)
                                            .mask_toggle()
                                            .aria_label(password_accessible_label),
                                        palette,
                                    ))
                                    .when(setup.smtp_auth == megamail_core::discovery::AuthMethod::Password, |column| {
                                        column
                                            .child(
                                                Button::new("toggle-smtp-password")
                                                    .ghost()
                                                    .small()
                                                    .label(if setup.smtp_credentials_expanded { "Use the same password for SMTP" } else { "Use a different SMTP password" })
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        if let Some(setup) = this.setup.as_mut() {
                                                            setup.smtp_credentials_expanded = !setup.smtp_credentials_expanded;
                                                            if !setup.smtp_credentials_expanded {
                                                                setup.inputs.smtp_password.update(cx, |input, cx| input.set_value("", window, cx));
                                                            }
                                                            cx.notify();
                                                        }
                                                    })),
                                            )
                                            .when(setup.smtp_credentials_expanded, |column| {
                                                column.child(labeled_input(
                                                    "SMTP password",
                                                    Input::new(&setup.inputs.smtp_password)
                                                        .content_type(InputContentType::Password)
                                                        .mask_toggle()
                                                        .aria_label("Separate SMTP password"),
                                                    palette,
                                                ))
                                            })
                                    }),
                            ))
                            .when((setup.manual || candidate.is_some()) && settings_need_review, |row| {
                                row.child(
                                    Button::new("review-server-settings")
                                        .ghost()
                                        .small()
                                        .selected(settings_reviewed)
                                        .icon(Icon::new(if settings_reviewed { IconName::Check } else { IconName::Info }))
                                        .label("I reviewed these server settings")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(setup) = this.setup.as_mut() {
                                                if setup.review_is_current(cx) {
                                                    setup.warning_reviewed = false;
                                                    setup.reviewed_settings = None;
                                                } else {
                                                    setup.warning_reviewed = true;
                                                    setup.reviewed_settings = Some(setup.settings_fingerprint(cx));
                                                }
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when_some(setup.error.clone(), |row, error| {
                                row.child(error_banner("Could not add account", &error, cx, palette))
                            })
                            ,
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .border_t_1()
                    .border_color(palette.border)
                    .px_6()
                    .py_3()
                    .child(
                        h_flex()
                            .w_full()
                            .max_w(px(600.))
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(if setup.manual || candidate.is_some() {
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child(if self.auth_inflight {
                                        "Finishing browser sign-in: validation, mailbox probe, then private save. Saved is not connected."
                                    } else if setup.connecting {
                                        "Testing both servers…"
                                    } else if self.loading_profiles {
                                        "Loading saved accounts…"
                                    } else {
                                        "Credentials stay in the OS keyring."
                                    })
                            } else {
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .text_color(palette.muted)
                                    .child("Find provider settings or import Thunderbird to continue.")
                            })
                            .when(has_mail_accounts, |row| {
                                row.child(
                                    Button::new("setup-back")
                                        .ghost()
                                        .small()
                                        .label(if self.profiles.is_empty() {
                                            "Open mail"
                                        } else {
                                            "Return"
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.view = this.setup_return_view;
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(setup.manual || candidate.is_some(), |row| {
                                row.child(
                                    Button::new("connect-account")
                                        .primary()
                                        .small()
                                        .icon(Icon::new(if setup.connecting {
                                            IconName::LoaderCircle
                                        } else {
                                            IconName::ArrowRight
                                        }))
                                        .label(if self.auth_inflight {
                                            "Signing in…"
                                        } else if setup.connecting {
                                            "Testing…"
                                        } else {
                                            "Test and add"
                                        })
                                        .disabled(
                                            setup.connecting
                                                || setup.discovering
                                                || setup.thunderbird_loading
                                                || self.live.read(cx).direct_add_block_reason().is_some()
                                                || self.loading_profiles
                                                || self.startup_error.is_some()
                                                || (settings_need_review && !settings_reviewed),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| this.connect_account(cx))),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn appearance_view(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        appearance_view::render(self, palette, cx)
    }
}
