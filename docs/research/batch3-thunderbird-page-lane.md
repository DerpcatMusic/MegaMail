# Batch 3: Thunderbird page-read lane — source-only release

## Concrete seam

The inspected producer is `live.rs::PageSender → page_sender → Command::Page`. Before this change, `ActorState::handle_page → page → Runtime::call("list")` waited on the actor. That prevented the same actor from admitting queued `LoadBody` or consuming `BodyFinished`. Native direct-account `QueryService` is a different, already-background reader; it is not Thunderbird's page executor.

Production now uses `handle_page → prepare_page → PageLane → QueryClient::Runtime → Runtime::call("list")`; `run → poll_page → project_page → MailboxEvent::Page` consumes completion. The private QueryClient test variant replaces only the external call boundary. No third engine, socket, shell RPC, dependency, or public interface was added.

## Bounds and lifetime

One persistent, joined page thread belongs to each profile actor. Its job and completion channels each have capacity one; the actor retains exactly one physical flight, including logically retired work. Each list therefore occupies at most one of the core transport's existing 64 pending slots. Existing list deadlines remain 15 seconds. Retirement never clears that gate early. Completion JSON is capped at 2 MiB, rows at 100, and request framing at the existing 1 MiB limit. The core's parsed transport frame can still reach its existing 16 MiB bound before the page-specific byte check.

Actor commands precede page projection. A 10 ms poll and a maximum 64-command burst prevent a completed flight from remaining busy indefinitely under queued commands. Selected body/cache/conversation code and metadata are unchanged. Stateful operations stay serial and invalidate overlapping pages. Preloaded pages need no worker admission. Overlapping remote pages receive explicit typed Busy failures and require retry; this is not an automatic queued-retry design.

The immutable envelope captures account, source profile path, actor session, unique request ticket, folder/remote identities, cursor, query, and caller generation before dispatch. Supersession, rebinding, folder-tree replacement, mutation, and shutdown retire pending status. Only current envelopes allocate native IDs, rows, unread projection, or cursor tokens. No Thunderbird UIDVALIDITY is invented or old generation restamped.

Shutdown retires the flight and bounded queued pages, closes command/completion delivery, then joins the owned reader. Its runtime clone retains existing process/lock cleanup until the deadline/return. New page threads are never detached.

## Evidence and remaining contracts

Thirteen Rust tests were authored **before/alongside the fix, NOT COMPILED OR RUN**. The real actor dispatcher and mailbox event consumer use fictional query callbacks and the actual BodyCache. Coverage includes warm/cold selected-body events before page release, original synchronous-await negative control, folder/generation retirement, flight bound, serial mutation order, timeout versus login, panic recovery, disconnect, sender-close shutdown, source/session rejection, bytes, and actor cursor allocation. Existing body/protocol tests remain.

Executed: rustfmt and scoped Python source assertions only. An initial whitespace-sensitive assertion was corrected; final structural assertions passed. No Rust execution, native benchmark, build, provider/profile/keyring access, installation, or mail mutation occurred. The earlier 300.8→5.46 ms result remains production-JS fixture evidence, not native acceptance.

Immediate physical cancellation needs a minimal additive core cancellable-call/token contract. Silent UI deselection without another actor request additionally needs a scoped page-retirement notification. Neither frozen interface was changed. Existing body/conversation/refresh thread counts and the public unbounded UI event channel remain parent-owned residuals.

Adapter release SHA-256: `2313fd22c2806d388c7d21f7cd2fc5217b0c24204bae61a9a9c5343c71c3f63d`.
