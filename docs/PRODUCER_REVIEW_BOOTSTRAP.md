# Starting review against an already-running producer

`FileOversight::bind_publication_from_producer` captures a new attempt's original
witness recipe and the matching change-feed cut from one actual bundle read. It
serves the existing checked supervisor, whose former preparation path read the
snapshot alone and therefore could not bind an image ahead of its bootstrap feed.
The FA-061/062 requirement that original cut coverage be complete is unchanged.

The caller supplies the original owner's attempt, explicitly matched producer
readers, bounded witness requests, and a trusted clock sampled after acquisition.
Preflight checks ownership/action/profile, the Reviewing stage, absence of any
existing witness recipe, absence of an active review session, and the configured
feed/freshness gates. Duplicate or foreign requests return before I/O or mutation.
This is an operator API, not an actor command or a new authority constructor.

The owner durably withdraws feed eligibility before opening the bundle. There is
no bound witness to withdraw yet: the mandatory empty slot cannot authorize work.
After reading, the existing feed ingester checks every retained overlap and stages
the contiguous suffix and producer heartbeat. It then stages the ORIGINAL source
binding at that resulting cut. One final canonical replacement acknowledges both
catch-up and the immutable recipe. The initial withdrawal is a separate write.
All existing current-observation callers still use the same ingestion routine.
No new journal event or wire format is introduced. An empty structured recipe
requires an opaque input lane; it cannot produce a metadata-only review binding.

The original image is not installed as a fresh current input. Helpers must still
review the action, and authorization, dispatch and first publication still require
independent acquisitions plus the existing policy, human, deadline and exact
witness checks. Later data never changes the original recipe. A producer update
after the read cannot rewrite that image; it is seen at the next boundary.

A missing/malformed bundle returns an inner read error, with feed eligibility
withdrawn and no original recipe bound. A new actual read can retry that case.
After successful decoding, a failed clock, invalid recipe, missing retained
prefix, conflict, allocation/capacity/replay failure, or failed storage replacement
quarantines the owner. Neither a binding nor an installed-feed report escapes a
failed final write. The original recovery path fences old keys; no effect outcome
or refund is inferred from a failure. A snapshot ahead of an evicted feed prefix
is still refused rather than treated as proof of the missing records.

The producer remains an operator-controlled observation source. This operation
neither authenticates external truth nor snapshots independent policy sources or
remote effects atomically. Full bundle access requires full-input authorization.

Eight tests in `tests/producer_initial_binding.rs` cover an advanced producer's
successful review/publication, no initial-capture freshness credit, post-read
replacement with unrelated/forbidden controls, read-loss retry, duplicate/foreign/
active-review refusal, invalid recipes, exact retained-window neighbors, failed
replacement, caught clock unwind, and empty-recipe whole-input requirements.
They are authored, not executed. Rust, Cargo and RCH are unavailable in this editing environment; exact-revision remote
compilation, formatting, Clippy and tests remain required. FA-061/062 stay open.

## Runnable supervisor integration

The existing workflow now calls `PublicationProfile::prepare_with_clock` before
helper launch. Producer-backed profiles pass their matched readers, original recipe
and actual workflow clock to the native combined binder. The prepared value holds
only readers; subsequent authorization, dispatch and first publication still
acquire independently. No new schema or command is needed. New submissions to
retained stores share this same execute routine. Exact retries and receipt-only
recovery do not enter preparation and still need no producer or helper access.

Raw version-one/version-two profiles keep their former original-capture behavior
and do not call the new clock argument. The lower-level `prepare` method remains
available for those profiles and already-matching cuts; it still refuses an
advanced unmatched snapshot. There is no fallback from failed coupled preparation
to that method for a producer-backed running workflow. The newer explicit
whole-input profile retains its mandatory opaque lane even with an empty recipe.

Four further tests in the example's `publication/bootstrap_tests.rs` cover the
original startup failure beside its caught-up control, full process/reviewer
publication with an already-advanced producer, source-free receipt recovery,
later forbidden/unrelated changes, and failure to renew an expired heartbeat by
reading it during startup. Existing tests and their assertions are unchanged.
These are twelve authored Rust tests across the two commits, not executed results:
the original eleven plus one whole-input compatibility regression added at integration.
The preliminary withdrawal remains separately durable; failures never acknowledge
only a permitting half of the original binding/catch-up transaction.
