//! Explicit local exports of the currently rendered, generation-qualified reader.
use crate::*;

fn exportable(text: &str) -> bool {
    !text.is_empty() && text.len() <= reader_document::SOURCE_MAX
}

fn printable_html(subject: &str, from: &str, to: &str, text: &str) -> Result<String, String> {
    if !exportable(text)
        || [subject, from, to]
            .iter()
            .any(|value| value.len() > 16 * 1024)
    {
        return Err("The displayed message exceeds printable-export limits.".into());
    }
    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }
    Ok(format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'">
<meta name="viewport" content="width=device-width, initial-scale=1"><title>{subject}</title>
<style>@page{{margin:18mm}}body{{font:15px/1.5 system-ui,sans-serif;color:#161616;max-width:900px;margin:32px auto;padding:0 20px}}h1{{font-size:24px;line-height:1.3;overflow-wrap:anywhere}}dl{{display:grid;grid-template-columns:45px 1fr;gap:6px 14px;border-bottom:1px solid currentColor;padding-bottom:20px}}dt{{font-weight:600}}dd{{margin:0;overflow-wrap:anywhere}}pre{{font:inherit;white-space:pre-wrap;overflow-wrap:anywhere}}@media print{{body{{max-width:none;margin:0;padding:0}}}}</style></head>
<body><h1>{subject}</h1><dl><dt>From</dt><dd>{from}</dd><dt>To</dt><dd>{to}</dd></dl><pre>{text}</pre></body></html>"#,
        subject = escape(subject),
        from = escape(from),
        to = escape(to),
        text = escape(text)
    ))
}

impl MailApp {
    pub(crate) fn save_readable_text(&mut self, cx: &mut Context<Self>) {
        self.export_displayed_text(false, cx);
    }
    /// A fetched message source arrived: save it, or attach it to a new forward.
    pub(crate) fn finish_eml(&mut self, export: live::EmlExport, window: &mut Window, cx: &mut Context<Self>) {
        let raw = match export.raw {
            Ok(raw) => raw,
            Err(error) => {
                self.reader_export_status = Some(format!("Could not fetch the message source: {error}"));
                cx.notify();
                return;
            }
        };
        let subject = self
            .live
            .read(cx)
            .snapshot()
            .selected_message
            .as_ref()
            .filter(|row| row.key == export.key)
            .map(|row| row.message.subject.clone())
            .unwrap_or_default();
        if export.forward {
            if !self.can_replace_composer(cx) {
                self.reader_export_status = Some("Finish or close the open draft first.".into());
                cx.notify();
                return;
            }
            let (attachments, temporary) = match compose::stage_eml(&subject, raw.as_ref().clone()) {
                Ok(staged) => staged,
                Err(error) => {
                    self.reader_export_status = Some(error);
                    cx.notify();
                    return;
                }
            };
            let mut message = compose::new_message(export.key.account_id);
            message.subject = format!("Fwd: {subject}");
            message.attachments = attachments;
            self.composer = Some(Composer::new_prepared_draft(message, temporary, None, None, window, cx));
            self.view = View::Compose;
            cx.notify();
            return;
        }
        let directory = std::env::var_os("XDG_DOWNLOAD_DIR")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let picker = cx.prompt_for_new_path(&directory, Some("message.eml"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picker.await else { return };
            let result = cx
                .background_spawn(async move { desktop_actions::write_new_private_file(&path, &raw) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.reader_export_status = Some(match result {
                    Ok(()) => "Saved the message as an .eml file.".into(),
                    Err(error) => error,
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn save_printable_html(&mut self, cx: &mut Context<Self>) {
        self.export_displayed_text(true, cx);
    }
    fn export_displayed_text(&mut self, printable: bool, cx: &mut Context<Self>) {
        if self.reader_export_busy || self.view != View::Mailbox {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        if !snapshot.body_loaded {
            return;
        }
        let Some(identity) = self.reader_text_identity(&snapshot) else {
            return;
        };
        let Some(target) = self.reader_text.as_ref().filter(|t| t.matches(&identity)) else {
            return;
        };
        let text = target.state.read(cx).rendered_text().as_str().to_owned();
        if !exportable(&text) {
            self.reader_export_status = Some("The displayed text is empty, still parsing, or exceeds 2 MiB. Try again after the message loads.".into());
            cx.notify();
            return;
        }
        let Some(row) = snapshot
            .selected_message
            .as_ref()
            .filter(|r| r.key == identity.key)
        else {
            return;
        };
        let headers = (
            row.message.subject.clone(),
            row.message.from_addr.clone(),
            row.message.to.clone(),
        );
        let directory = std::env::var_os("XDG_DOWNLOAD_DIR")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let picker = cx.prompt_for_new_path(
            &directory,
            Some(if printable {
                "megamail-message.html"
            } else {
                "megamail-message.txt"
            }),
        );
        self.reader_export_busy = true;
        self.reader_export_status = None;
        cx.spawn(async move |this, cx| {
            let chosen = picker.await;
            let task = this.update(cx, |this, cx| {
                this.reader_export_busy = false;
                let current = this.live.read(cx).snapshot();
                if this.view != View::Mailbox
                    || this.reader_text_identity(&current).as_ref() != Some(&identity)
                {
                    cx.notify();
                    return None;
                }
                let path = match chosen {
                    Ok(Ok(Some(path))) => path,
                    Ok(Err(_)) => {
                        this.reader_export_status =
                            Some("The save dialog could not open. Try saving again.".into());
                        cx.notify();
                        return None;
                    }
                    _ => {
                        cx.notify();
                        return None;
                    }
                };
                this.reader_export_busy = true;
                cx.notify();
                Some(cx.background_spawn(async move {
                    let bytes = if printable { printable_html(&headers.0, &headers.1, &headers.2, &text)?.into_bytes() } else { text.into_bytes() };
                    desktop_actions::write_new_private_file(&path, &bytes)
                }))
            });
            let Ok(Some(task)) = task else {
                return;
            };
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.reader_export_busy = false;
                let current = this.live.read(cx).snapshot();
                if this.view == View::Mailbox
                    && this.reader_text_identity(&current).as_ref() == Some(&identity)
                {
                    this.reader_export_status = Some(match result {
                        Ok(()) if printable => "Saved printable HTML. Open the file in a browser and use Print or Save as PDF.".into(),
                        Ok(()) => "Saved the displayed message text.".into(),
                        Err(error) => error,
                    });
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn printable_export_escapes_message_content_and_disables_external_requests() {
        let html = printable_html(
            "<img src=x onerror=alert(1)>",
            "owner@example.test",
            "recipient@example.test",
            "<script>alert('x')</script>&\nمرحبا",
        )
        .unwrap();
        assert!(!html.contains("<img"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&amp;\nمرحبا"));
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains("white-space:pre-wrap"));
        assert!(printable_html(&"x".repeat(16 * 1024 + 1), "from", "to", "body").is_err());
    }

    #[test]
    fn readable_export_bounds_utf8_bytes_without_trimming_or_rewriting() {
        assert!(!exportable(""));
        assert!(exportable("  مرحبا 世界\n"));
        assert!(exportable(&"x".repeat(reader_document::SOURCE_MAX)));
        assert!(!exportable(&"x".repeat(reader_document::SOURCE_MAX + 1)));
        assert!(!exportable(
            &"界".repeat(reader_document::SOURCE_MAX / 3 + 1)
        ));
    }
}
