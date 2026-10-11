//! Read-only rule preview over the selected, generation-qualified message.
use crate::*;
use megamail_core::config::{FilterInput, FilterRule};
use rules_preview::{ActionOutcome, MatchOutcome, PreviewAction, PreviewContext, PreviewSample};

fn describe(
    rules: &[FilterRule],
    context: &PreviewContext<'_>,
    sample: &PreviewSample<'_>,
) -> Result<Vec<String>, String> {
    let reports = rules_preview::preview_rules(rules, context, sample)
        .map_err(|_| "The rule preview rejected this message or its scope.".to_string())?;
    if reports.is_empty() {
        return Ok(vec!["No saved rules for this profile.".into()]);
    }
    let mut result = vec!["Preview only. No message was changed.".into()];
    for report in reports {
        let rule = &rules[report.rule_index];
        let name = if rule.name.is_empty() {
            format!("Rule {}", report.rule_index + 1)
        } else {
            rule.name.clone()
        };
        let status = match report.matched {
            MatchOutcome::Matched => "matches",
            MatchOutcome::NotMatched => "does not match",
            MatchOutcome::NeedsBody => "needs a complete body; actions are provisional",
            MatchOutcome::OtherAccount => "belongs to another account",
        };
        result.push(format!("{name}: {status}"));
        for action in report.actions {
            let (label, outcome) = match action {
                PreviewAction::Tag { keyword, outcome } => (format!("Tag {keyword}"), outcome),
                PreviewAction::Move {
                    destination,
                    outcome,
                    ..
                } => (format!("Move to {destination}"), outcome),
            };
            let status = match outcome {
                ActionOutcome::WouldApply => "would apply",
                ActionOutcome::NoCapability => "unavailable for this account",
                ActionOutcome::UnknownDestination => "destination not discovered",
                ActionOutcome::UnknownTag => "tag not discovered",
                ActionOutcome::AlreadyPresent => "already present",
                ActionOutcome::SameFolder => "already in this folder",
                ActionOutcome::DuplicateTag => "already proposed by an earlier rule",
                ActionOutcome::LaterMoveIgnored => "an earlier move takes precedence",
            };
            result.push(format!("{label}: {status}"));
        }
    }
    Ok(result)
}

pub(crate) struct MovePlan {
    identity: reader_session::SemanticTextIdentity,
    scope: worker::NativeMutationScope,
    destination: String,
}
fn planned_move(
    rules: &[FilterRule],
    context: &PreviewContext<'_>,
    sample: &PreviewSample<'_>,
) -> Option<String> {
    let reports = rules_preview::preview_rules(rules, context, sample).ok()?;
    let mut destination = None;
    for report in reports {
        if report.matched == MatchOutcome::NeedsBody {
            return None;
        }
        if report.matched != MatchOutcome::Matched {
            continue;
        }
        for action in report.actions {
            match action {
                PreviewAction::Move {
                    destination: path,
                    count_unread: true,
                    outcome: ActionOutcome::WouldApply,
                } => destination = Some(path.to_owned()),
                PreviewAction::Move {
                    outcome: ActionOutcome::LaterMoveIgnored | ActionOutcome::SameFolder,
                    ..
                } => {}
                _ => return None,
            }
        }
    }
    destination
}

impl MailApp {
    pub(crate) fn apply_previewed_rule_move(&mut self, cx: &mut Context<Self>) {
        if self.demo
            || self.rule_preview_busy
            || self.triage.busy()
            || self.composer.is_some()
            || self.view != View::Mailbox
        {
            return;
        }
        let Some(plan) = self.rule_move_plan.take() else {
            return;
        };
        let current = self.live.read(cx).snapshot();
        if self.reader_text_identity(&current).as_ref() != Some(&plan.identity)
            || self.live.read(cx).native_mutation_scope(&plan.identity.key) != Some(plan.scope)
        {
            self.rule_preview =
                vec!["The message changed since preview. Preview its saved rules again.".into()];
            cx.notify();
            return;
        }
        self.prepare_single_rule_move(plan.identity.key, plan.scope, plan.destination, cx);
    }
    pub(crate) fn preview_saved_rules(&mut self, cx: &mut Context<Self>) {
        if self.demo || self.rule_preview_busy || self.view != View::Mailbox {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(identity) = self.reader_text_identity(&snapshot) else {
            return;
        };
        let Some(row) = snapshot.selected_message.clone() else {
            return;
        };
        let Some(account) = snapshot
            .accounts
            .iter()
            .find(|a| a.id == row.key.account_id)
        else {
            return;
        };
        let address = account.email.clone();
        let folders: Vec<_> = snapshot
            .folders
            .iter()
            .filter(|f| f.account_id == row.key.account_id)
            .map(|f| f.path.clone())
            .collect();
        let native_scope = self.live.read(cx).native_mutation_scope(&row.key);
        let can_move = native_scope.is_some();
        self.rule_move_plan = None;
        let complete = snapshot
            .selected_original
            .as_ref()
            .is_some_and(|o| o.completeness == reader_document::Completeness::Complete);
        let html = snapshot
            .selected_original
            .as_ref()
            .and_then(|o| o.source.clone())
            .unwrap_or_else(|| Arc::from(snapshot.selected_html.as_ref()));
        self.rule_preview_busy = true;
        self.rule_preview = vec!["Reading saved rules…".into()];
        let task = cx.background_spawn(async move {
            let rules = config::load_filters_checked()?;
            let body = megamail_core::markdown::plain_text(&html);
            let folders: Vec<_> = folders.iter().map(String::as_str).collect();
            let tags: Vec<_> = row.message.keywords.iter().map(String::as_str).collect();
            let context = PreviewContext {
                account_id: row.key.account_id,
                account_email: &address,
                source_folder: &row.key.folder_path,
                known_folders: &folders,
                known_tags: &tags,
                existing_tags: &tags,
                can_move,
                can_tag: false,
            };
            let sample = PreviewSample {
                account_id: row.key.account_id,
                input: FilterInput {
                    from_addr: &row.message.from_addr,
                    from_name: &row.message.from_name,
                    subject: &row.message.subject,
                    recipients: &row.message.to,
                    reply_to: &row.message.reply_to,
                    body: &body,
                    body_hits: &[],
                },
                body_coverage: if complete {
                    rules_preview::BodyCoverage::Complete
                } else {
                    rules_preview::BodyCoverage::HeadersOnly
                },
            };
            describe(&rules, &context, &sample)
                .map(|lines| (lines, planned_move(&rules, &context, &sample)))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.rule_preview_busy = false;
                let current = this.live.read(cx).snapshot();
                if this.view == View::Mailbox
                    && this.reader_text_identity(&current).as_ref() == Some(&identity)
                {
                    match result {
                        Ok((lines, destination)) => {
                            this.rule_preview = lines;
                            this.rule_move_plan =
                                destination.zip(native_scope).map(|(destination, scope)| {
                                    MovePlan {
                                        identity,
                                        scope,
                                        destination,
                                    }
                                });
                        }
                        Err(error) => {
                            this.rule_preview = vec![error];
                            this.rule_move_plan = None;
                        }
                    }
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
    fn only_supported_complete_move_plans_can_be_applied() {
        use megamail_core::config::{FilterField, FilterMatch};
        let mut rule = FilterRule {
            name: "Fixture".into(),
            account_email: "owner@example.test".into(),
            field: FilterField::Subject,
            matcher: FilterMatch::Contains,
            value: "needle".into(),
            more: vec![],
            any: false,
            dest_path: "Archive".into(),
            tag: String::new(),
            count_unread: true,
        };
        let context = PreviewContext {
            account_id: 7,
            account_email: "owner@example.test",
            source_folder: "INBOX",
            known_folders: &["Archive"],
            known_tags: &[],
            existing_tags: &[],
            can_move: true,
            can_tag: false,
        };
        let sample = PreviewSample {
            account_id: 7,
            input: FilterInput {
                subject: "needle",
                ..FilterInput::default()
            },
            body_coverage: rules_preview::BodyCoverage::HeadersOnly,
        };
        assert_eq!(
            planned_move(&[rule.clone()], &context, &sample),
            Some("Archive".into())
        );
        rule.count_unread = false;
        assert!(planned_move(&[rule.clone()], &context, &sample).is_none());
        rule.count_unread = true;
        rule.tag = "Work".into();
        assert!(planned_move(&[rule.clone()], &context, &sample).is_none());
        rule.tag.clear();
        rule.field = FilterField::Body;
        assert!(planned_move(&[rule], &context, &sample).is_none());
    }

    #[test]
    fn real_rules_show_other_accounts_and_incomplete_bodies_without_dispatch() {
        use megamail_core::config::{FilterField, FilterMatch};
        let rule = FilterRule {
            name: "Fictional filing".into(),
            account_email: "owner@example.test".into(),
            field: FilterField::Body,
            matcher: FilterMatch::Contains,
            value: "needle".into(),
            more: vec![],
            any: false,
            dest_path: "Archive".into(),
            tag: String::new(),
            count_unread: true,
        };
        let context = PreviewContext {
            account_id: 7,
            account_email: "owner@example.test",
            source_folder: "INBOX",
            known_folders: &["Archive"],
            known_tags: &[],
            existing_tags: &[],
            can_move: true,
            can_tag: false,
        };
        let sample = PreviewSample {
            account_id: 7,
            input: FilterInput::default(),
            body_coverage: rules_preview::BodyCoverage::HeadersOnly,
        };
        let rows = describe(&[rule.clone()], &context, &sample).unwrap();
        assert!(rows.iter().any(|r| r.contains("needs a complete body")));
        let context = PreviewContext {
            account_email: "other@example.test",
            ..context
        };
        assert!(
            describe(&[rule], &context, &sample)
                .unwrap()
                .iter()
                .any(|r| r.contains("belongs to another account"))
        );
    }
}
