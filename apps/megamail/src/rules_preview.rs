//! Read-only bounded preview of the actual mail-core FilterRule/FilterInput seam.
//! Incoming text is data, never an authority to dispatch actions. No worker, provider,
//! config persistence, HTML renderer, or execution backend is called from this module.

use megamail_core::config::{FilterField, FilterInput, FilterRule};

const MAX_RULES: usize = 256;
const MAX_CONDITIONS: usize = 16;
const MAX_HEADER: usize = 64 * 1024;
const MAX_BODY: usize = 2 * 1024 * 1024;
const MAX_CONTEXT_ITEMS: usize = 1024;

/// Whether the supplied body and body_hits are enough to answer every body condition.
/// HeadersOnly includes a snippet/partial body: absence of a match is not a negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyCoverage {
    Complete,
    HeadersOnly,
}

/// Trusted account/folder metadata supplied by the UI's selected account registry.
/// Booleans describe existing mutation capability, not authorization to use it.
pub struct PreviewContext<'a> {
    pub account_id: u32,
    pub account_email: &'a str,
    pub source_folder: &'a str,
    pub known_folders: &'a [&'a str],
    pub known_tags: &'a [&'a str],
    pub existing_tags: &'a [&'a str],
    pub can_move: bool,
    pub can_tag: bool,
}

/// Read-only untrusted message data, with ownership from its retained scoped key.
/// Do not persist/log body text, especially decrypted/encrypted-mail content.
pub struct PreviewSample<'a> {
    pub account_id: u32,
    pub input: FilterInput<'a>,
    pub body_coverage: BodyCoverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewError {
    WrongAccount,
    LimitExceeded,
    InvalidContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchOutcome {
    Matched,
    NotMatched,
    NeedsBody,
    OtherAccount,
}

/// WouldApply is a DRY RUN result, not completion or a mail mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionOutcome {
    WouldApply,
    NoCapability,
    UnknownDestination,
    UnknownTag,
    AlreadyPresent,
    SameFolder,
    DuplicateTag,
    LaterMoveIgnored,
}

/// Actions encoded by existing FilterRule; deliberately no arbitrary commands/AI tools.
/// count_unread is retained as the existing filing policy, not a mark-read command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewAction<'a> {
    Tag {
        keyword: &'a str,
        outcome: ActionOutcome,
    },
    Move {
        destination: &'a str,
        count_unread: bool,
        outcome: ActionOutcome,
    },
}

/// Rule index ties UI results to its immutable rule-list snapshot. References borrow
/// config only: no message contents are copied into the report.
pub struct RulePreview<'a> {
    pub rule_index: usize,
    pub matched: MatchOutcome,
    pub actions: Vec<PreviewAction<'a>>,
    /// Existing core Body semantics use substring/hits, ignoring the matcher enum.
    pub body_uses_contains_semantics: bool,
}

fn bounded_strings(values: &[&str]) -> bool {
    values.len() <= MAX_CONTEXT_ITEMS
        && values.iter().all(|value| value.len() <= MAX_HEADER)
        && values.iter().map(|value| value.len()).sum::<usize>() <= MAX_BODY
}

fn validate(
    context: &PreviewContext<'_>,
    sample: &PreviewSample<'_>,
    rules: &[FilterRule],
) -> Result<(), PreviewError> {
    if context.account_id == 0 || sample.account_id != context.account_id {
        return Err(PreviewError::WrongAccount);
    }
    if context.account_email.trim().is_empty()
        || context.source_folder.is_empty()
        || context.account_email.chars().any(char::is_control)
    {
        return Err(PreviewError::InvalidContext);
    }
    let input = &sample.input;
    if rules.len() > MAX_RULES
        || input.body.len() > MAX_BODY
        || !bounded_strings(&[
            context.account_email,
            context.source_folder,
            input.from_addr,
            input.from_name,
            input.subject,
            input.recipients,
            input.reply_to,
        ])
        || !bounded_strings(context.known_folders)
        || !bounded_strings(context.known_tags)
        || !bounded_strings(context.existing_tags)
        || input.body_hits.len() > 128
        || input.body_hits.iter().any(|hit| hit.len() > MAX_HEADER)
        || input.body_hits.iter().map(String::len).sum::<usize>() > MAX_BODY
    {
        return Err(PreviewError::LimitExceeded);
    }
    let mut rule_bytes = 0usize;
    for rule in rules {
        if rule.more.len() >= MAX_CONDITIONS {
            return Err(PreviewError::LimitExceeded);
        }
        for value in [
            &rule.name,
            &rule.account_email,
            &rule.value,
            &rule.dest_path,
            &rule.tag,
        ]
        .into_iter()
        .chain(rule.more.iter().map(|condition| &condition.value))
        {
            if value.len() > MAX_HEADER {
                return Err(PreviewError::LimitExceeded);
            }
            rule_bytes += value.len();
        }
        if rule_bytes > MAX_BODY {
            return Err(PreviewError::LimitExceeded);
        }
    }
    Ok(())
}

fn match_outcome(rule: &FilterRule, sample: &PreviewSample<'_>) -> MatchOutcome {
    if sample.body_coverage == BodyCoverage::Complete {
        return if rule.matches(&sample.input) {
            MatchOutcome::Matched
        } else {
            MatchOutcome::NotMatched
        };
    }
    // Do not make absent body a false negative or a snippet a complete body. Known
    // conditions still use the existing matching implementation (including Reply-To
    // fallback, recipient parsing, comma alternatives and body matcher quirks).
    let conditions = rule.conditions();
    let mut unknown = false;
    for condition in conditions {
        if condition.field == FilterField::Body {
            unknown = true;
        } else {
            let matched = condition.matches(&sample.input);
            if rule.any && matched {
                return MatchOutcome::Matched;
            }
            if !rule.any && !matched {
                return MatchOutcome::NotMatched;
            }
        }
    }
    if unknown {
        MatchOutcome::NeedsBody
    } else if rule.matches(&sample.input) {
        MatchOutcome::Matched
    } else {
        MatchOutcome::NotMatched
    }
}

/// Preview every rule in list order. Like the existing GTK filter pass, collect tags
/// from every matching rule and choose the FIRST known, non-source move destination.
/// Unknown destinations do not consume that first-move slot. This is predicate/action
/// preview only: not sync's Sent-copy exclusions, provider epochs, or mutation execution.
/// If any rule needs body data, list-level tag/move precedence is provisional; UI must
/// label the entire preview incomplete and must not offer execution from these results.
pub fn preview_rules<'a>(
    rules: &'a [FilterRule],
    context: &PreviewContext<'_>,
    sample: &PreviewSample<'_>,
) -> Result<Vec<RulePreview<'a>>, PreviewError> {
    validate(context, sample, rules)?;
    let mut report = Vec::with_capacity(rules.len());
    let mut wanted_tags: Vec<&str> = Vec::new();
    let mut move_selected = false;
    for (rule_index, rule) in rules.iter().enumerate() {
        let matched = if !rule
            .account_email
            .eq_ignore_ascii_case(context.account_email)
        {
            MatchOutcome::OtherAccount
        } else {
            match_outcome(rule, sample)
        };
        let mut actions = Vec::new();
        if matched == MatchOutcome::Matched {
            if !rule.tag.is_empty() {
                let outcome = if !context
                    .known_tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case(&rule.tag))
                {
                    ActionOutcome::UnknownTag
                } else if context
                    .existing_tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case(&rule.tag))
                {
                    ActionOutcome::AlreadyPresent
                } else if wanted_tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case(&rule.tag))
                {
                    ActionOutcome::DuplicateTag
                } else {
                    wanted_tags.push(&rule.tag);
                    if context.can_tag {
                        ActionOutcome::WouldApply
                    } else {
                        ActionOutcome::NoCapability
                    }
                };
                actions.push(PreviewAction::Tag {
                    keyword: &rule.tag,
                    outcome,
                });
            }
            if !rule.dest_path.is_empty() {
                let outcome = if !context.known_folders.contains(&rule.dest_path.as_str()) {
                    ActionOutcome::UnknownDestination
                } else if context.source_folder == rule.dest_path {
                    ActionOutcome::SameFolder
                } else if move_selected {
                    ActionOutcome::LaterMoveIgnored
                } else {
                    move_selected = true;
                    if context.can_move {
                        ActionOutcome::WouldApply
                    } else {
                        ActionOutcome::NoCapability
                    }
                };
                actions.push(PreviewAction::Move {
                    destination: &rule.dest_path,
                    count_unread: rule.count_unread,
                    outcome,
                });
            }
        }
        report.push(RulePreview {
            rule_index,
            matched,
            actions,
            body_uses_contains_semantics: rule.field == FilterField::Body
                || rule
                    .more
                    .iter()
                    .any(|condition| condition.field == FilterField::Body),
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use megamail_core::config::{FilterCondition, FilterMatch};

    fn rule() -> FilterRule {
        FilterRule {
            name: "Fictional".into(),
            account_email: "self@example.invalid".into(),
            field: FilterField::Subject,
            matcher: FilterMatch::Contains,
            value: "invoice, receipt".into(),
            more: vec![],
            any: false,
            dest_path: "Archive".into(),
            tag: "finance".into(),
            count_unread: true,
        }
    }
    fn context() -> PreviewContext<'static> {
        PreviewContext {
            account_id: 7,
            account_email: "SELF@example.invalid",
            source_folder: "INBOX",
            known_folders: &["INBOX", "Archive", "Other"],
            known_tags: &["finance"],
            existing_tags: &[],
            can_move: true,
            can_tag: true,
        }
    }
    fn sample() -> PreviewSample<'static> {
        PreviewSample {
            account_id: 7,
            input: FilterInput {
                subject: "Your RECEIPT",
                ..Default::default()
            },
            body_coverage: BodyCoverage::Complete,
        }
    }

    #[test]
    fn preview_uses_core_matches_and_preserves_input() {
        let rules = [rule()];
        let input = sample();
        let before = format!("{:?}", input.input);
        let report = preview_rules(&rules, &context(), &input).unwrap();
        assert!(rules[0].matches(&input.input));
        assert_eq!(report[0].matched, MatchOutcome::Matched);
        assert_eq!(
            report[0].actions,
            [
                PreviewAction::Tag {
                    keyword: "finance",
                    outcome: ActionOutcome::WouldApply
                },
                PreviewAction::Move {
                    destination: "Archive",
                    count_unread: true,
                    outcome: ActionOutcome::WouldApply
                }
            ]
        );
        assert_eq!(before, format!("{:?}", input.input));
    }

    #[test]
    fn scope_is_trusted_metadata_not_incoming_sender() {
        let mut foreign = rule();
        foreign.account_email = "other@example.invalid".into();
        let rules = [foreign];
        let report = preview_rules(&rules, &context(), &sample()).unwrap();
        assert_eq!(report[0].matched, MatchOutcome::OtherAccount);
        assert!(report[0].actions.is_empty());
        let mut input = sample();
        input.account_id = 8;
        assert!(matches!(
            preview_rules(&rules, &context(), &input),
            Err(PreviewError::WrongAccount)
        ));
    }

    #[test]
    fn missing_body_is_indeterminate_not_false_negative() {
        let mut r = rule();
        r.more.push(FilterCondition {
            field: FilterField::Body,
            matcher: FilterMatch::Equals,
            value: "confirmed".into(),
        });
        let mut input = sample();
        input.body_coverage = BodyCoverage::HeadersOnly;
        input.input.body = "confirmed"; // Even a positive snippet is not claimed complete.
        let rules = [r.clone()];
        let report = preview_rules(&rules, &context(), &input).unwrap();
        assert_eq!(report[0].matched, MatchOutcome::NeedsBody);
        r.any = true;
        assert_eq!(
            preview_rules(&[r], &context(), &input).unwrap()[0].matched,
            MatchOutcome::Matched
        );
    }

    #[test]
    fn first_valid_move_and_all_unique_tags_follow_existing_pass_order() {
        let mut unknown = rule();
        unknown.dest_path = "Vanished".into();
        let mut later = rule();
        later.dest_path = "Other".into();
        let rules = [unknown, rule(), later];
        let report = preview_rules(&rules, &context(), &sample()).unwrap();
        assert!(matches!(
            report[0].actions[1],
            PreviewAction::Move {
                outcome: ActionOutcome::UnknownDestination,
                ..
            }
        ));
        assert!(matches!(
            report[1].actions[0],
            PreviewAction::Tag {
                outcome: ActionOutcome::DuplicateTag,
                ..
            }
        ));
        assert!(matches!(
            report[1].actions[1],
            PreviewAction::Move {
                outcome: ActionOutcome::WouldApply,
                ..
            }
        ));
        assert!(matches!(
            report[2].actions[1],
            PreviewAction::Move {
                outcome: ActionOutcome::LaterMoveIgnored,
                ..
            }
        ));
    }

    #[test]
    fn unsupported_adapter_is_not_presented_as_success() {
        let mut ctx = context();
        ctx.can_move = false;
        ctx.can_tag = false;
        let rules = [rule()];
        let report = preview_rules(&rules, &ctx, &sample()).unwrap();
        assert!(report[0].actions.iter().all(|action| matches!(
            action,
            PreviewAction::Tag {
                outcome: ActionOutcome::NoCapability,
                ..
            } | PreviewAction::Move {
                outcome: ActionOutcome::NoCapability,
                ..
            }
        )));
    }

    #[test]
    fn body_matcher_quirk_and_reply_to_fallback_come_from_core() {
        let mut r = rule();
        r.field = FilterField::Body;
        r.matcher = FilterMatch::Equals;
        r.value = "receipt".into();
        let mut input = sample();
        input.input.body = "not equal but contains receipt";
        let rules = [r];
        let report = preview_rules(&rules, &context(), &input).unwrap();
        assert_eq!(report[0].matched, MatchOutcome::Matched);
        assert!(report[0].body_uses_contains_semantics);
        let mut r = rule();
        r.field = FilterField::ReplyTo;
        r.matcher = FilterMatch::Equals;
        r.value = "sender@example.invalid".into();
        input.input.from_addr = "sender@example.invalid";
        assert_eq!(
            preview_rules(&[r], &context(), &input).unwrap()[0].matched,
            MatchOutcome::Matched
        );
    }

    #[test]
    fn oversized_rules_are_rejected_before_matching_or_partial_report() {
        assert!(matches!(
            preview_rules(&vec![rule(); MAX_RULES + 1], &context(), &sample()),
            Err(PreviewError::LimitExceeded)
        ));
    }
}
