## Standards

**0 confirmed hard breaches; 0 new concrete P2 findings.**

Read-only R8 against HEAD `d4dbd2bba14f474a33870269cc15bd98376ff360`, including relevant working-tree/untracked files. Graft returned no matching nodes. Reviewed repository instructions, the **full** `docs/research/batch2-helper-engine-runtime.md`, observer/control harnesses, receipts, traces, and relevant policy/launcher/protocol consumers.

Read-only hash verification found:
- All **15 receipt-covered helper/fixture inputs** match the 575-input Correction 5 freeze; README remains separately hashed.
- Both compiled helpers and extensions match `summary.json`.
- All **70 manifest-listed artifacts**, including the summary and traces, match `artifact-sha256.json`.

Correction 5’s build log records production and isolated no-CSP builds. No reruns, compilation, edits, application launches, provider/profile operations, installation, or network probes were performed in this review.

## Spec

Original brief retained: responsive, faithful HTML using a real isolated engine; preserve Zeron and existing mail/profile boundaries; prevent active content/network access and preserve encrypted-cache exclusions; no fake OAuth or feature claims inferred from declarations.

**SOURCE/EVIDENCE-ACCEPTED FOR NAMED SCENARIOS:**

- **Hook → acknowledgement → load:** `extension.c` installs cancellation before sending acknowledgement; `main.c` consumes the matching acknowledgement before loading untrusted content. Actual hostile/parser ltraces show this ordering at **40 → 41 → 43**. No-CSP traces record five hostile denials and one parser-confusion denial by actual `mm_request_allowed = 0`; the source callback returns its inverse. Authored tracing records allowed document and CID `/part/0`. Production fixtures produce no remote hook requests—not evidence of extension rejection.
- **Containment/egress:** The compiled launcher retains namespace isolation, capability drop, private empty read-only `/sys`, and fd-3 verification. Production strace records original/current namespace identities differing. All eight helper traces contain **zero AF_INET/AF_INET6 occurrences**, covering descendants and distinguishing allowed AF_UNIX traffic. The independent private-network control records successful IPv4/IPv6 TCP/UDP sends; EPERM calibration is excluded.
- **Lifecycle:** Receipts support owned-window `closed`/exit 0, missing-extension refusal/exit 2 without WebKit execution, and pinned WebProcess crash → termination error → owned close/exit 2. Inventories show no live descendants before forced cleanup.

**UNRESOLVED, accurately deferred:** observer cancellation lacks final-interval syscall coverage and does not prove native ReaderHost cancellation. Redirects, image-set dispatch, actual file requests, user-click handoff, hangs, native lifecycle, clipboard, accessibility and pixels remain unaccepted. Readiness does not establish responsive/fidelity acceptance.

The native protocol rejects the mutation readiness policy at source; this run has **no native consumer proof**. Default original-formatting access remains closed. No blanket security guarantee or report overclaim identified.

**Summary:** Standards 0 findings; Spec 0 new concrete P2 findings. Read-only review released; parent retains report ownership.
