# Durable evidence-source recovery

## Contract and caller

The trusted owner of `FileOversight` can now recover an exhausted or poisoned file-observation capture without abandoning the publication journal. The API is `source::FileSourceReplacement`, `FileOversight::replace_file_source`, and `FileOversight::file_source_replacement`. This extends the existing L2 observation source through the original L6 broker replacement operation; it is not a new actor verb, approval mechanism or policy implementation.

Construct a request with a nonzero operation ID, the current capture generation, the current authority epoch, and a strictly greater next capture generation. Submit it with the current journal revision. Keep the request identity for retries. An identical retry returns its original historical `PolicySourceChange`, even after reopening or later work, without writing another event. Reusing its operation ID for different fields refuses. Unknown operations still require the exact current revision and predecessor.

Replacement keeps the original source ID and scope, fixed freshness age, per-generation capture limits and journal budget. The native lifetime bound remains sixteen capture generations, including bootstrap, regardless of gaps in the numeric generation labels or intervening reopens. Producer generation is a separate identity and is never reset. Retained producer bytes, the semantic floor and native observation/consumer clock floors survive. Replacement does not make retained evidence current: a fresh complete file observation is required before admission resumes.

Undispatched proposals, reviews and authorizations are cancelled through the original broker and their reservations are refunded there. Their helper sessions and automatic/human keys are retired. Already dispatched effects keep their charged liability and retained envelopes. Guarded publication can resolve nonexecution; an existing terminal endpoint receipt remains authoritative. Only the original reconciliation path settles the broker. Recovery never retries an effect or refunds an unknown outcome.

A new replacement is permitted under a locally interrupted observation latch because it durably retires that observation source and leaves the replacement unavailable. An old replacement retry does not clear a newer latch. A terminally stopped host does not accept a new replacement; historical inspection/retry remains available. Journal capacity and storage failures are not repaired by source replacement.

The journal adds source subevent tag `3` containing the original replacement request fields. Earlier source tags and the outer journal profile remain unchanged. New readers replay older histories; older readers reject the new tag. Receipts are reconstructed from original transitions, never accepted as serialized authority or asserted balances.

## Implementation status and change log

2026-09-13: added bounded durable source replacement and replay-retained request identities. Added public tests pairing capture exhaustion/poisoning with fresh two-key publication, preserved producer/semantic floors, unchanged lease expiry, exact retries through reopen, newer-work preservation, and the lifetime generation bound. Source freshness/authentication and operator storage/clock assumptions remain unchanged. This is a reference-profile extension, not a production qualification.

The attempted verification command was `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`; it failed before execution because `rch` was not installed (exit 127). Rust compilation, formatting, Clippy and test execution for this change remain unverified. No Beads or roadmap packet is closed by these changes.
