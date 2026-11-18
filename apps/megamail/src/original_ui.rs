//! The Original reader: Blitz render lifecycle, remote-image policy, link clicks.
use crate::*;

impl MailApp {
    /// Whether the selected message shows in its original layout.
    pub(crate) fn original_mode(&self, snapshot: &MailboxSnapshot) -> bool {
        let Some(key) = snapshot.selected_key.as_ref() else {
            return false;
        };
        !self.prefer_clean
            && !self.plain_messages.contains(key)
            && snapshot.body_loaded
            && snapshot
                .selected_original
                .as_ref()
                .is_some_and(|original| original.source.is_some())
    }

    pub(crate) fn original_theme(&self) -> original_view::Theme {
        let rgb = |color: gpui_kit::Hsla| {
            let c = color.to_rgb();
            [(c.r * 255.) as u8, (c.g * 255.) as u8, (c.b * 255.) as u8]
        };
        original_view::Theme {
            dark: self.dark,
            background: rgb(self.palette.background),
            text: rgb(self.palette.text),
            link: rgb(self.palette.accent),
        }
    }

    /// Start (or refresh) the Blitz render of the selected message when its
    /// source, width, scale, theme or image policy changed. Never blocks.
    pub(crate) fn ensure_original_render(&mut self, snapshot: &MailboxSnapshot, cx: &mut Context<Self>) {
        if self.view != View::Mailbox || !self.original_mode(snapshot) {
            return;
        }
        let (Some(key), Some(source)) = (
            snapshot.selected_key.clone(),
            snapshot
                .selected_original
                .as_ref()
                .and_then(|original| original.source.clone()),
        ) else {
            return;
        };
        let measured = self.original_measure.get();
        let width = if measured > 100. { measured.min(920.) } else { 640. };
        let same = self.original.as_ref().filter(|state| state.key == key);
        // Images load on their own, like Gmail, unless the sender failed
        // authentication or the message sits in Junk/Spam.
        let untrusted = snapshot.selected_sender_check.as_ref().is_some_and(|check| {
            matches!(
                check.trust,
                megamail_core::models::SenderTrust::Fail | megamail_core::models::SenderTrust::Suspicious
            )
        }) && !snapshot
            .selected_message
            .as_ref()
            .is_some_and(|m| self.image_senders.contains(&m.message.from_addr.to_ascii_lowercase()))
            || {
            let folder = key.folder_path.to_ascii_lowercase();
            folder.contains("spam") || folder.contains("junk")
        };
        let default_remote = self.preferences.load_remote && !untrusted;
        let remote_forced = same.is_some_and(|s| s.remote_forced);
        let (allow_remote, original_colors) = same.map_or((default_remote, false), |s| {
            ((s.allow_remote && !untrusted) || s.remote_forced, s.original_colors)
        });
        let hide_history = (self.preferences.hide_quoted_text || self.hidden_quotes.contains(&key))
            && !self.visible_quotes.contains(&key);
        let params = original_view::Params {
            width,
            scale: self.window_scale,
            theme: self.original_theme(),
            original_colors,
            allow_remote,
            hide_history,
        };
        if let Some(state) = same {
            let unchanged = Arc::ptr_eq(&state.source, &source)
                && (state.params.width - params.width).abs() < 6.
                && state.params.scale == params.scale
                && state.params.theme == params.theme
                && state.params.original_colors == params.original_colors
                && state.params.allow_remote == params.allow_remote
                && state.params.hide_history == params.hide_history;
            if unchanged || state.pending {
                return;
            }
        }
        self.original_generation += 1;
        let generation = self.original_generation;
        let previous = self
            .original
            .take()
            .filter(|state| state.key == key)
            .and_then(|state| state.result.map(|result| (result, state.doc_id)));
        let (result, doc_id) = match previous {
            Some((result, doc_id)) => (Some(result), doc_id),
            None => (None, 0),
        };
        self.original = Some(OriginalState {
            key,
            source: source.clone(),
            params: params.clone(),
            allow_remote,
            remote_forced,
            original_colors,
            generation,
            pending: true,
            doc_id,
            result,
        });
        let rx = original_view::render(generation, source, params);
        cx.spawn(async move |this, cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("The original layout renderer stopped.".into()));
            let mut result = result;
            // Remote images and styles arrive after the first paint; draw
            // again once they are in the cache. The current image stays up.
            let remote = result.as_mut().ok().and_then(|rendered| rendered.remote_done.take());
            let _ = this.update(cx, |this, cx| {
                if let Some(state) = this.original.as_mut().filter(|s| s.generation == generation) {
                    state.pending = false;
                    state.doc_id = generation;
                    state.result = Some(result);
                    cx.notify();
                }
            });
            if let Some(remote) = remote {
                if remote.await.is_ok() {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(state) = this.original.as_mut().filter(|s| s.generation == generation) {
                            state.params.width = -1.;
                            let snapshot = this.live.read(cx).snapshot();
                            this.ensure_original_render(&snapshot, cx);
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
    }

    /// The rendered original, or None to fall back to the Clean view while
    /// the first render runs (no blank pane).
    pub(crate) fn original_element(&self, key: &MessageKey, palette: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let measure = self.original_measure.clone();
        let measure_canvas = gpui_kit::canvas(
            move |bounds, _, _| measure.set(f32::from(bounds.size.width)),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(1.));
        let state = self.original.as_ref().filter(|state| &state.key == key);
        let Some(Ok(rendered)) = state.and_then(|state| state.result.as_ref()) else {
            if let Some(Err(error)) = state.and_then(|state| state.result.as_ref()) {
                return Some(
                    v_flex()
                        .w_full()
                        .relative()
                        .gap_2()
                        .child(measure_canvas)
                        .child(div().text_size(px(12.)).text_color(palette.muted).child(error.clone()))
                        .into_any_element(),
                );
            }
            // While the first render runs (tens of ms), hold the space with
            // the measuring strip instead of flashing the Clean text first.
            if state.is_some_and(|state| state.pending) {
                return Some(
                    div()
                        .w_full()
                        .relative()
                        .min_h(px(240.))
                        .child(measure_canvas)
                        .into_any_element(),
                );
            }
            let _ = measure_canvas;
            return None;
        };
        let state = state.unwrap();
        let bounds_cell = self.original_bounds.clone();
        let bounds_canvas = gpui_kit::canvas(
            move |bounds, _, _| bounds_cell.set(bounds),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let dark = self.dark;
        Some(
            v_flex()
                .w_full()
                .relative()
                .gap_2()
                .child(measure_canvas)
                .when(rendered.blocked_remote > 0, |el| {
                    el.child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_size(px(11.))
                            .text_color(palette.muted)
                            .when(rendered.blocked_remote > 0, |row| {
                                row.child(format!(
                                    "{} remote image{} blocked",
                                    rendered.blocked_remote,
                                    if rendered.blocked_remote == 1 { "" } else { "s" }
                                ))
                                .child(
                                    Button::new("original-load-images")
                                        .ghost()
                                        .xsmall()
                                        .label("Load images")
                                        .tooltip("Fetch this message's remote images. The sender can see that you opened it.")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_original_policy(Some(true), None, cx)
                                        })),
                                )
                                .child(
                                    Button::new("original-always-load-images")
                                        .ghost()
                                        .xsmall()
                                        .label("Always from this sender")
                                        .tooltip("Load this sender's images automatically, even when their mail fails sender checks. Junk is never loaded.")
                                        .on_click(cx.listener(|this, _, _, cx| this.trust_sender_images(cx))),
                                )
                            })
,
                    )
                })
                .child(
                    div()
                        .id("original-view")
                        .relative()
                        .w(px(rendered.width))
                        .h(px(rendered.height))
                        .rounded(px(6.))
                        .overflow_hidden()
                        .on_click(cx.listener(|this, event: &gpui_kit::ClickEvent, _, cx| {
                            this.original_click(event.position(), cx)
                        }))
                        .child(
                            gpui_kit::img(gpui_kit::ImageSource::Render(rendered.image.clone()))
                                .w(px(rendered.width))
                                .h(px(rendered.height)),
                        )
                        .child(bounds_canvas),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn original_click(&mut self, position: gpui_kit::Point<gpui_kit::Pixels>, cx: &mut Context<Self>) {
        let Some(state) = self.original.as_ref() else {
            return;
        };
        let bounds = self.original_bounds.get();
        let x = f32::from(position.x - bounds.origin.x);
        let y = f32::from(position.y - bounds.origin.y);
        if x < 0. || y < 0. {
            return;
        }
        let rx = original_view::link_at(state.doc_id, x, y);
        cx.spawn(async move |_, cx| {
            if let Ok(Some(href)) = rx.await {
                let _ = cx.update(|cx| {
                    if megamail_core::mail_text::safe_browser_url(&href) {
                        cx.open_url(&href);
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn trust_sender_images(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let Some(sender) = snapshot.selected_message.as_ref().map(|m| m.message.from_addr.to_ascii_lowercase()) else {
            return;
        };
        self.image_senders.insert(sender);
        let senders = self.image_senders.clone();
        cx.background_spawn(async move { let _ = image_trust::save(&senders); }).detach();
        self.set_original_policy(Some(true), None, cx);
    }

    pub(crate) fn set_original_policy(&mut self, remote: Option<bool>, colors: Option<bool>, cx: &mut Context<Self>) {
        if let Some(state) = self.original.as_mut() {
            if let Some(remote) = remote {
                state.allow_remote = remote;
                state.remote_forced = remote;
            }
            if let Some(colors) = colors {
                state.original_colors = colors;
            }
        }
        let snapshot = self.live.read(cx).snapshot();
        self.ensure_original_render(&snapshot, cx);
        cx.notify();
    }

    /// Reader mode for the selected message: Original, Clean or Text.
    pub(crate) fn set_reader_mode(&mut self, mode: ReaderMode, cx: &mut Context<Self>) {
        let snapshot = self.live.read(cx).snapshot();
        let Some(key) = snapshot.selected_key.clone() else {
            return;
        };
        match mode {
            ReaderMode::Original => {
                self.prefer_clean = false;
                self.plain_messages.remove(&key);
            }
            ReaderMode::Clean => {
                self.prefer_clean = true;
                self.plain_messages.remove(&key);
            }
            ReaderMode::Text => {
                self.plain_messages.insert(key);
            }
        }
        self.refresh_reader_text(&snapshot, cx);
        self.ensure_original_render(&snapshot, cx);
        cx.notify();
    }

    pub(crate) fn reader_mode(&self, snapshot: &MailboxSnapshot) -> ReaderMode {
        match snapshot.selected_key.as_ref() {
            Some(key) if self.plain_messages.contains(key) => ReaderMode::Text,
            _ if self.original_mode(snapshot) => ReaderMode::Original,
            _ => ReaderMode::Clean,
        }
    }
}
