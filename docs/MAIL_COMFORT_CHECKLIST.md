# MegaMail comfort and trust work

Requested 2026-10-16: make the entire daily mail experience comfortable and dependable, with clear authorship and usable folders, accounts, sorting, lifecycle, search, composition and recovery. Existing native-migration work is included in this branch; no real email is sent during development.

## Implementation and verification

- [x] Folder/account navigation stays usable while a draft is retained; return to the same draft without losing text.
- [x] All-account Inbox, Sent, Drafts, Archive and searchable local mail are discoverable.
- [x] Date, sender/email, subject and unread sorting, with direction; direct accounts sort the entire local folder cache before paging, legacy adapters sort loaded headers.
- [x] Distinct You / correspondent identity, visible addresses, sender-specific avatars and sent-message treatment.
- [x] Opaque reading/list surfaces; readable metadata and clear selection/focus in light and dark themes.
- [x] Restore the full original message; collapse recognized quote history, including Outlook-style reply headers.
- [x] Newest relevant message opens clearly; expand/collapse the conversation without losing selection.
- [x] Outgoing reply remains visible with accurate sending/queued/accepted/uncertain/refused status until reconciliation.
- [x] Durable Needs reply / Waiting / Done / Snoozed state, independent of unread, with new inbound reopening and explicit scope.
- [x] Search scope and cached-body limitations visible; all-folder local search and useful field hints.
- [x] Persistent per-account sync/connection health and visible recovery actions.
- [x] Compose identity, recipients, draft retention, attachment warnings and send-state recovery remain truthful.
- [x] Relevant Hylki correctness changes assessed; preserve shared GTK callers and profile separation.
- [x] Native/core tests, formatting, bridge/helper checks, production build and native demo interaction evidence.
- [x] Fresh independent finish review, accurate docs, reviewed PR, verified install and source receipt.

## Release boundaries

A checked item records implementation with the source/test/capture evidence in the qualification receipt; it does not extend beyond the release boundaries below. Provider-wide delivery, Windows/macOS support, rich HTML editing and provider-global search require separate evidence and must never be implied by local fixtures. Controlled test mail uses fictional loopback fixtures only. Unknown delivery never authorizes an automatic retry.

## Daily-use guide

- Click any account or folder to browse it. A retained composer no longer blocks navigation. Use **Resume draft** to return; **Back to mail · keep draft** leaves it intact.
- The shared rail opens **All inboxes**, **All Sent**, **All Drafts**, **Archived**, **All local mail** and **Outbox**. Local mail excludes Trash, Spam and Templates. It is cached mail, not a provider-wide completeness claim.
- Use **Sort** for date, sender email, subject or unread and the direction button to reverse. Direct IMAP queries sort SQLite before limiting each page. The list combines bounded pages; legacy Thunderbird sources sort loaded headers.
- **You · sent**, the full sender address and correspondent-colored avatars identify authors. Conversation controls expand all or collapse history. **Details** exposes transport metadata. **Show quoted history** restores the original text; original-format view remains available when the isolated helper is installed.
- The conversation work menu saves **Needs reply**, **Waiting**, **Snoozed** (one day) or **Done** to this device. These are independent of read/unread and provider labels. A newer inbound message reopens Waiting/Done; expired snoozes reappear. Unsaved or corrupt state is reported explicitly.
- Sending creates an immediate provisional reply card with its actual state. Server acceptance waits for the Sent copy; uncertain outcomes require reviewing Outbox before resending. Exact account and Message-ID reconcile the card. Durable Outbox and draft recovery remain the restart authority.
- The sync panel shows per-account connection errors and observed folder-sync times. It does not prove all provider folders or history are synchronized.
- Compose supports opt-in Markdown and a formatted preview. Plain text remains the default. Existing recovered HTML is retained. This is not a WYSIWYG rich-text editor.

## Hylki integration decisions

MegaMail already uses the Hylki worker/cache/protocol implementation. This change ports the intent of upstream [immediate sent-reply cards](https://github.com/hyprlab/hylki/commit/3ffbb8dfd080cb8300177db193b47423a1ce1e73) into GPUI, using explicit per-send Message-IDs for exact reconciliation. It also supplies an explicit UTF-8 plain-text SMTP content type. It preserves GTK callers and separate MegaMail profile/credential storage. The upstream gap and candidates are recorded in the external assessment; merging every upstream commit would include reverted features and is not a qualification strategy.

## Remaining qualification and feature backlog

These are not silently marked complete by a local test pass:

- Real-provider send/receive, OAuth variants, long offline/reconnect sessions and accessibility assistive-technology qualification need controlled account-specific evidence. No real messages are sent during this work.
- Provider-global search, every historical thread member, WYSIWYG editing, calendar/contact synchronization, Windows/macOS, scheduled work reminders beyond one-day snooze and cloud synchronization of work states remain outside the verified native feature set.
- Work filters and conversation grouping operate on the currently loaded, bounded results; separate account/folder cache queries provide paging and explicit partial-result warnings.
- Provisional reply cards are session-local. Durable outbox tickets and recovered drafts, rather than the visual card, carry restart/retry authority.

## Qualification receipt

Final test counts, visual artifacts, build revision, installation hashes and independent review disposition are recorded in `docs/research/mail-comfort-qualification.md` when qualification completes.
