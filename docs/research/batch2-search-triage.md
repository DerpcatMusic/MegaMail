# Batch 2 — bounded search and triage handoff

Status: **owned files frozen for parent integration; native integration not shipped**. This owner may write only `apps/megamail/src/triage.rs`, `crates/mail-core/src/search.rs`, and this report. No module declarations, live state, worker, cache, query, manifest or GTK callers were edited. Parent owns the single later batch build/test window. No Cargo/build/check/clippy/rustc/native/helper compilation, provider calls, private-profile access, credential access, mail mutation, commit or push occurred.

## Actual current seam, not a replacement engine

Queried graft first; the graph returned no hits. Read AGENTS, PRODUCT, DESIGN, architecture, the feature matrix, foundation mail-engine/account/auth/reader/latency findings, cache/delivery/reader owner handoffs, current live state, production Message model, query service, shared worker requests/results and Thunderbird request dispatch. Applicable Rust and deep-module guidance was loaded.

`live::MessageKey` currently owns account ID, folder path and UID. `triage::ScopedMessageKey` **wraps this existing key**, adding nonzero session and mailbox generations. It does not introduce a second worker. Direct accounts must supply persisted mailbox generation from the cache owner plus current session generation; Thunderbird must supply its actual session/message-map generation, not an IMAP UIDVALIDITY guess. Unknown generations/epochs must fail closed for mutations.

Production request mapping is executable in `Batch::submit_next`, through `Dispatch`:

| Mutation | Existing request | Account routing |
| --- | --- | --- |
| `SetUnread(unread)` | `MailRequest::SetSeen { path, uid, seen: !unread }` | `dispatch.key.message.account_id` |
| `SetStarred(starred)` | `MailRequest::SetFlagged { path, uid, flagged: starred }` | same |
| `MoveFolder { destination }` | `MailRequest::MoveMessage { path, uid, dest: destination }` | same; destination must be discovered in this account |
| `Label { name, add }` | **No request; UnsupportedLabel** | no dispatch |

Both current direct IMAP and Thunderbird handlers support the first three single-item requests. Thunderbird rejects `MoveMessages`, keyword editing, purge, spam and various bulk requests. The module therefore uses bounded single-item FIFO admission with **one in-flight item**, not a fake Thunderbird bulk method or a misleading shared bulk completion. All folder moves require explicit user confirmation, including archive. Labels are never translated to moves or `SetKeyword`: IMAP keywords/local tags, Outlook categories and Gmail label membership are different semantics.

### Parent/native integration obligations (blocking dispatch)

1. Register `mod triage;` in native main and `pub mod search;` in core lib. No new dependencies: search uses existing chrono, rusqlite and serde; triage uses std and current core/live imports.
2. Retain `Selection` on the actual live mailbox. Call `update_view(view_generation, visible_scoped_keys)` using the actual filtered loaded rows, not conversation representative keys mistaken for all conversation members. Increase generation on account/folder/query/filter change. Same-generation refresh prunes vanished keys; pagination does **not** implicitly select new pages. Key membership uses exact account/path/UID/session/mailbox scope. Invalid/duplicate/oversize input leaves state unchanged. Restore one-reader selection separately; do not make checkboxes trigger provider read mutations.
3. Wire click toggle, inclusive anchored Shift range, select-all-loaded and clear through `Selection::edit`. Render selected count and **loaded-only** scope. Action cap is 500, visible cap 5,000. Over-cap range/select-all is rejected atomically, not silently truncated. “All provider history” and “all indexed matching messages” selection are NOT implemented. A server-total count is not an operation key set.
4. Freeze exact `SelectedItem` snapshots for flag inverse/no-op checks. Prepare through `Batch::prepare`. Capabilities bind to the **exact scoped key**, contain discovered same-account destinations, and must derive online/write permission, validated epoch and correlation from actual backend state. Graph/JMAP/POP3/unknown adapters remain Unsupported. Saved configuration/open channel is not online/write permission. Parent must retain one active batch and monotonic non-reused batch IDs (checked overflow), not one batch per click.
5. Native adapter must carry `{OperationToken { batch_id, item_index }, ScopedMessageKey}` to execution and echo an exact terminal `Completion::{Succeeded,Failed,Uncertain}`. Check current account/session/mailbox generations immediately before mutation and before admitting refreshed rows. Use operation result, not list-row disappearance, to report success. **Current legacy worker events cannot satisfy this contract:** SeenSettled and MovesSettled occur on success *or failure*; BulkComplete has neither item nor success; star/single-move lack a typed terminal success. Generic Error cannot be arbitrarily attributed to the latest action. No wrapper around a bool sender can manufacture these guarantees. Coordinate an additive native envelope/result seam with worker/TB owners; preserve GTK MailRequest/event callers. Until wired, set `correlated_results=false`; actions reject explicitly, not pretend to work.
6. Call `submit_next` only from the actual native sender lane. Route Dispatch to the owning account's sender, preserving token/key sideband through the additive native adapter. Admission false is `Rejected(AdmissionFailed)`, not success. Re-read exact-key current capabilities each time. After terminal result, submit next pending item; if an item was capability-rejected, continue driving remaining pending items without falsely finishing the whole batch. No automatic mutation retry.
7. `Batch::cancel` stops unsent items only. On scope change it cancels pending work; in-flight stays in-flight. Account removal/session loss/timeout must settle the admitted item **Uncertain** unless a definite execution failure is known. Ignoring a late event is not cancellation and not a successful mutation. Parent sets a bounded timeout; never dispatch a duplicate on timeout. Workers must still validate mailbox epoch, because local admission cannot prevent a concurrent server UID reset.
8. Undo receipts exist only for confirmed successful flag changes. `UndoReceipt::prepare` checks exact scoped key and fresh flag state, then uses the same Batch/Dispatch/request mapping for the inverse. Capabilities and epoch are revalidated. Moves/purge/labels have no undo receipt; no speculative Message-ID/old-UID restore. Limit UI history to 20 receipts and expire on account/session/mailbox change; this module returns at most the action cap. An external flag race after snapshot remains possible without a provider conditional mutation.

## Search implementation and semantics

`SearchQuery::parse` is the sole grammar for both `matches(&Message, BodyText)` and `sql_predicate(allow_body)`. Tests exercise those callable interfaces, not a disconnected alternate parser. Bounds: 4,096 UTF-8 bytes, 32 AND terms, 2 MiB eligible body. Oversize query/term sets are rejected, not truncated. ASCII case-insensitive literal substrings are deliberate to match SQLite LIKE; non-ASCII case folding is not claimed. `%`, `_`, `!` are literals, never user-controlled SQL wildcards.

Implemented grammar:

- Bare words and quoted phrases: sender name/address, subject and header preview only. They do not implicitly search bodies.
- `from:`, `to:`, `cc:`, `subject:`, `body:`; phrases such as `from:"A Sender"`.
- `date:YYYY-MM-DD` is that UTC day `[start, next day)`; `before:` is strictly before that day; `after:` is strictly after that day (next-day start inclusive). This does not infer a user's time zone. Unknown timestamp zero remains Unknown under both positive and negative date terms.
- `unread:true|false`, `starred:true|false`, `attachments:true|false`, `is:unread|read|starred`, `has:attachment`.
- Leading `-` negates a term. All terms are AND. Unquoted AND/OR/NOT, grouping, unknown operators, invalid dates/booleans and incomplete quoting/escaping reject. OR, relevance ranking, attachment filename/type/content and natural-language dates are deferred.
- Body absence/exclusion/oversize is **Unknown**, including `-body:...`, not empty-text success. Supply explicit body eligibility: decrypted/encrypted/signed-or-unknown content remains Excluded under existing cache policy. Preview never substitutes for body. This module never fetches bodies/resources. It does not persist body text.

`SearchScope` distinguishes `LoadedHeaders`, `AllLocalIndexed`, and `ProviderHistory` with accurate labels. `validate_scope` rejects unavailable scopes, and rejects body search under LoadedHeaders even if a selected body happens to be available. Provider scope remains false on current integrations: there is **no provider-search implementation** here. A future adapter must report its actual provider coverage/errors; merely toggling a boolean does not implement provider search.

`SavedQueryRecord` is the untrusted serde persistence form; `SavedQuery::validate` checks version 1, bounded nonempty name, bounded folder, nonzero account, folder→account binding, grammar and backend capability. Validated SavedQuery has no unchecked deserialize. Parent must revalidate on execution, ensure account still exists, persist privately/atomically off UI and cap stored queries to 64. Persistence/chooser UI are not implemented.

### Cache/query owner obligations — not edited by this owner

Current direct `QueryService -> Cache::message_page` already scans cached **current-folder header/preview columns** using a single literal query before its page limit. It is not merely a post-filter of 100 loaded rows, but neither does it prove complete server history or all-folder/body search. Thunderbird has separate runtime queries; do not infer the direct SQLite scope applies to Thunderbird. The matrix/PRODUCT loaded-only wording needs reconciliation by parent based on actual adapter routes, not an inflated “full history” claim.

For parsed full-local search, the cache/query owner needs an additive paged background hook consuming `SearchQuery::sql_predicate`, not passing `from:...` into the legacy literal PageRequest query. Root shared cache cannot unconditionally import `crate::search` because GTK does not declare that core-only module. Suitable hook: a GTK-compatible bound predicate/page method in shared cache (schema identifiers fixed by trusted core code), called only by a core QueryService parsed-search branch. Keep the old message_page signature untouched. Do not change schema or add a second SQLite connection on GPUI.

Required contract:

- `SqlPredicate.clause` uses fixed `m` message alias and optional `b` eligible plain-body alias, anonymous `?` parameters, owned `Vec<rusqlite::types::Value>` and `requires_body`. Bind values with params_from_iter; **never** interpolate search values.
- Add mandatory bound account ID and folder set outside the predicate. Reject empty/unknown account scope, validate folder ownership, bound account/folder fanout. No global unscoped SQLite search as fallback. Return account/folder/UID/**mailbox generation** with every row; old folder IDs alone are unsafe.
- Preserve keyset pages and a maximum 500 returned rows, caller generation, coalescing/queue limits and epoch-aware stale-result rejection. For multi-folder/account pages define a deterministic compound cursor (e.g. timestamp/account/folder/UID); a single before_uid is not a valid cross-folder cursor.
- Alias `m.has_attachment` must be the same **effective attachment flag** current message_page/model uses (attachments_checked correction), not an uncorrected raw bit. Matcher consumes that production model value. Header columns are existing from_name/from_addr/recipients/cc/subject/preview/ts/unread/starred; null/unknown date remains SQL NULL under negative date terms.
- Only join `b` when required. Join on all account/folder/UID identity fields, plus validated epoch handling. `b.body` must be bounded, complete, cache-eligible **normalized plain text**, not the current rendered MIME HTML string treated as plain text. Missing/oversize/excluded body must remain NULL/Unknown under negation. SQL predicate explicitly refuses oversize body matches. Existing encrypted-body cache exclusions are mandatory; no new decrypted index. Plain projection/schema and eligibility live with cache owner, not this module.
- Report loaded headers vs all **locally indexed** matching headers vs provider coverage separately. Full local indexed means every qualifying local row was queried, not every provider message was synchronized. Body exclusions/missing bodies and index/backfill completeness require explicit unknown/partial coverage counts/warnings. A bounded result cap or missing cursor must not imply full provider-history completion.
- Do not route provider/TB full-search requests to Hylki SQLite. Saved scope cannot override adapter capability. If hooks are not ready, wire parser only to actual loaded model filtering with LoadedHeaders and reject body/local/provider scope honestly.

## Matrix accounting

The implementations below are reusable prepared modules, **not reachable native UI features** until parent wiring and batch validation.

| Matrix rows | Implemented inside owned module | Prepared integration / deferred |
| --- | --- | --- |
| T01/T02, T05 | scoped bounded selection, inclusive ranges, loaded select-all, read/star request mapping | UI state/controls and typed terminal results prepared; all-history selection deferred |
| T03/T04/T06/T08 | confirmed same-account single-item move planning; distinct outcomes/admission/cancel | archive/trash/arbitrary-destination UI + results prepared; true backend bulk, copy and permanent purge deferred |
| T07 | bounded loaded-set flag changes use existing single-item requests | folder-wide MarkAllRead/provider-total semantics deferred; do not label loaded selection “whole folder” |
| T11/T12/T13 | explicit unsupported label gate distinct from folder moves | Gmail label/category/tag editing deferred; SetKeyword is not label parity |
| T15 | confirmed flag-only inverse receipts, exact epoch/fresh-state check | flag undo UI prepared; move undo deferred pending unambiguous new keys and adapter support |
| S01/S05/S06 | parsed header/address/date/flags/attachment/phrase/exclusion grammar and matcher | native search integration prepared; AND only, OR/ranking deferred |
| S02/S03/S04/S09 | scope gates/labels, body unknown/exclusion semantics, parameterized SQL predicate | cache/query hooks and real coverage reporting prepared; full provider history deferred; all-folder compound paging deferred |
| S08 | saved record validation and capability revalidation interface | private bounded persistence/chooser UI deferred |
| T09/T10/T14/T16–T19, S07/S10–S14 | none claimed | out of this coherent selection/search slice |

## Validation and honest assessment

Rust synthetic tests are colocated in both modules and **not executed or type-checked** in this owner window. They use fictional Message models, in-memory SQLite, bounded selection and injected sender closures; no profile/provider access. Parent must declare modules then execute them in its one coordinated batch validation window. Parent must also test the actual worker/native envelope seam after wiring, not just these policy tests.

Executed validation: direct rustfmt plus rustfmt --check on both owned Rust files (syntax parsing/formatting only); whitespace checks of all three new files using git diff --no-index --check; Python static checks of bounds, no production unwrap/expect, exact-key capability binding, absence of MoveMessages/SetKeyword mapping, existing dependency declarations, and the fictional UTC timestamp. All passed. Eleven fictional Python SQLite cases executed SQL string templates extracted from the owned source: wildcard/escape literal handling, bound injection-shaped text, ASCII/non-ASCII case semantics, absent body, negated body matches, and oversized UTF-8 body exclusion. All passed. This executes SQL templates, **not the Rust parser/builder or native interface**. Thirty-seven authored Rust tests (16 search, 21 triage) remain **unexecuted and untypechecked**. The three owned files are frozen; no background processes or build slots were started.

**Good:** one grammar for model/SQL use; exact scoped identity and bounded atomic selection; no admission-as-success; no partial search called provider history; label/move distinction; cancel cannot lie about an admitted mutation; no decrypted body caching or network work introduced.

**Bad/limits:** no native feature becomes reachable from these three allowed files alone. The existing worker event seam lacks typed per-item outcomes; claiming native bulk/undo today would be false. Body SQL needs eligible plain projection and search scope completeness from cache owner; raw HTML body LIKE is not faithful body search. All moves ask confirmation, which is conservative but may need later UX refinement. Item-at-a-time dispatch sacrifices bulk protocol throughput; no benchmark or performance improvement is claimed.

**Risks/blockers:** parent module/event/UI wiring, actual mailbox/session generations and write ACLs, body projection/index eligibility, cross-folder cursor design, backend completeness reporting, undo races and timeout uncertainty. Preserve GTK callers, durable Outbox, account ownership, TLS/OAuth validation and default remote-resource blocking. No provider compatibility or production-readiness certification follows from authored synthetic tests.
