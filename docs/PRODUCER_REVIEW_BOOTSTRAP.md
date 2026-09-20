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
replacement, caught clock unwind, and empty-recipe whole-input requirements. They are authored, not executed. Rust, Cargo
and RCH are unavailable in this editing environment; exact-revision remote
compilation, formatting, Clippy and tests remain required. FA-061/062 stay open.
