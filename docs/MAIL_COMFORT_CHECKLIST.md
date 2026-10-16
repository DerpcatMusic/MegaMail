# MegaMail comfort and trust work

Requested 2026-10-16: make the entire daily mail experience comfortable and dependable, with clear authorship and usable folders, accounts, sorting, lifecycle, search, composition and recovery. Existing native-migration work is included in this branch; no real email is sent during development.

## Implementation and verification

- [ ] Folder/account navigation stays usable while a draft is retained; return to the same draft without losing text.
- [ ] All-account Inbox, Sent, Drafts, Archive and searchable local mail are discoverable.
- [ ] Date, sender/email, subject and unread sorting, with direction and honest loaded-result scope.
- [ ] Distinct You / correspondent identity, visible addresses, sender-specific avatars and sent-message treatment.
- [ ] Opaque reading/list surfaces; readable metadata and clear selection/focus in light and dark themes.
- [ ] Restore the full original message; collapse recognized quote history, including Outlook-style reply headers.
- [ ] Newest relevant message opens clearly; expand/collapse the conversation without losing selection.
- [ ] Outgoing reply remains visible with accurate sending/queued/accepted/uncertain/refused status until reconciliation.
- [ ] Durable Needs reply / Waiting / Done / Snoozed state, independent of unread, with new inbound reopening and explicit scope.
- [ ] Search scope and cached-body limitations visible; all-folder local search and useful field hints.
- [ ] Persistent per-account sync/connection health and visible recovery actions.
- [ ] Compose identity, recipients, draft retention, attachment warnings and send-state recovery remain truthful.
- [ ] Relevant Hylki correctness changes assessed; preserve shared GTK callers and profile separation.
- [ ] Native/core tests, formatting, bridge/helper checks, production build and native demo interaction evidence.
- [ ] Fresh independent finish review, accurate docs, reviewed PR, verified install and source receipt.

## Release boundaries

A checklist item is checked only with implemented behavior and named verification. Provider-wide delivery, Windows/macOS support, rich HTML editing and provider-global search require separate evidence and must never be implied by local fixtures. Controlled test mail uses fictional loopback fixtures only. Unknown delivery never authorizes an automatic retry.
