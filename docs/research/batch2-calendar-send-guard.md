# Batch 2: calendar MIME intent guard

## Confirmed source defects

The encrypted-Outbox preview correction identified a separate pre-existing MIME defect: build_message returns calendar MIME before OpenPGP wrapping. An outgoing calendar composition with encrypt=true therefore could produce plaintext MIME; sign=true could produce unsigned MIME. Its early return also bypassed ordinary attachment assembly and silently omitted extra user files.

## Parent correction

The calendar branch now refuses either signing/encryption intent or extra attachments before producing MIME. It returns a fixed explanatory build error. It does not drop fields from the original composition, clear security flags, silently downgrade protection, or claim to implement calendar encryption. Ordinary unprotected calendar messages retain their existing MIME shape.

Two Rust regressions are authored, not compiled or executed: all three sign/encrypt combinations must fail through actual build_email before plaintext calendar MIME escapes; send and draft builders must reject extra files rather than silently omit them. The existing ordinary calendar test remains intact. No test opens the fictional file or uses private GPG keys.

This bounded safety refusal is not a calendar/crypto implementation. Native RSVP remains unavailable, and other provider-specific crypto paths require their own capability checks. It does not erase previously persisted data or repair old deliveries. Public signatures and shared GTK callers remain unchanged; unsupported intent now yields an honest error instead of a false protection/file-preservation promise. Source syntax and whitespace checks pass; full review and the grouped Rust window remain pending.
