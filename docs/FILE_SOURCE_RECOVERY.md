# Durable evidence-source recovery

## Contract and caller

The trusted owner of `FileOversight` can now recover an exhausted or poisoned file-observation capture without abandoning the publication journal. The API is `source::FileSourceReplacement`, `FileOversight::replace_file_source`, and `FileOversight::file_source_replacement`. This extends the existing L2 observation source through the original L6 broker replacement operation; it is not a new actor verb, approval mechanism or policy implementation.

Construct a request with a nonzero operation ID, the current capture generation, the current authority epoch, and a strictly greater next capture generation. Submit it with the current journal revision. Keep the request identity for retries. An identical retry returns its original historical `PolicySourceChange`, even after reopening or later work, without writing another event. Reusing its operation ID for different fields refuses. Unknown operations still require the exact current revision and predecessor.

Replacement keeps the original source ID and scope, fixed freshness age, per-generation capture limits and journal budget. The native lifetime bound remains sixteen capture generations, including bootstrap, regardless of gaps in the numeric generation labels or intervening reopens. Producer generation is a separate identity and is never reset. Retained producer bytes, the semantic floor and native observation/consumer clock floors survive. Replacement does not make retained evidence current: a fresh complete file observation is required before admission resumes.

Undispatched proposals, reviews and authorizations are cancelled through the original broker and their reservations are refunded there. Their helper sessions and automatic/human keys are retired. Already dispatched effects keep their charged liability and retained envelopes. Guarded publication can resolve nonexecution; an existing terminal endpoint receipt remains authoritative. Only the original reconciliation path settles the broker. Recovery never retries an effect or refunds an unknown outcome.

A new replacement is permitted under a locally interrupted observation latch because it durably retires that observation source and leaves the replacement unavailable. An old replacement retry does not clear a newer latch. A terminally stopped host does not accept a new replacement; historical inspection/retry remains available. Journal capacity and storage failures are not repaired by source replacement.

The journal adds source subevent tag `3` containing the original replacement request fields. Earlier source tags and the outer journal profile remain unchanged. New readers replay older histories; older readers reject the new tag. Receipts are reconstructed from original transitions, never accepted as serialized authority or asserted balances.

## Supervised jobs and interrupted storage

`FileSupervisedDriver::replace_file_source` invokes that same durable host operation and then runs existing helper maintenance. Current ledger dispositions decide which cohort retires. A stale predecessor refusal leaves a healthy review intact, and a historical replacement retry cannot terminate a newer cohort. A cancelled job retains its original next `Stopped` event. Sent work keeps its publication/reconciliation phase and original charge. The method performs no file read, clock sample or approval operation.

Publication and source replacement do not conflate two different cases. An unresolved dispatched effect must go through guarded nonexecution and a separate reconciliation before its charge is released. An already executed effect retains its endpoint receipt even when the new source is unavailable. Neither case resends the effect.

A failed journal replacement returns no candidate source receipt, cancellation or refund. The failed live owner is unavailable; reopening replays the actual canonical bytes and fences before admission. A replacement whose canonical rename became visible may be retried by its original operation identity. One that did not become visible remains absent; its old authority predecessor is stale after the independent recovery fence. Both paths still require a new complete observation.

Source replacement is ordinary journal work. It cannot consume the emergency tail installed by `RecoveryReserve`; original fence/stop/drain operations retain that allowance. Repairing a source does not extend the journal, reserve disk space or promise unbounded restarts.

## Implementation status and change log

2026-09-13, first increment: added bounded durable source replacement and replay-retained request identities. Added four public tests pairing capture exhaustion/poisoning with fresh two-key publication, preserved producer/semantic floors, unchanged lease expiry, exact retries through reopen, newer-work preservation, and the lifetime generation bound.

2026-09-13, second increment: integrated source replacement with the existing supervised-driver lifecycle. Added four socket-worker driver tests for healthy/stale reviews, historical retries with a newer cohort, pending publication and already executed receipts. Added three unit test functions: a campaign over all five original storage barriers, an actual pre-write byte-capacity interruption and repeated historical retry, and the emergency-tail boundary. Eleven new test functions across both increments are present; they have not executed in this environment.

Source freshness/authentication and operator storage/clock assumptions remain unchanged. These tests use reference-profile endpoints, real operator files/socket workers and the existing deterministic I/O fault hooks; they are not hardware power-loss or production qualification evidence.

The attempted verification command was `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`; it failed before execution because `rch` was not installed (exit 127). Rust compilation, formatting, Clippy and test execution for this change remain unverified. No Beads or roadmap packet is closed by these changes.
