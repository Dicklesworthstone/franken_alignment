# Supervised producer catch-up

This opt-in acquisition path connects the original FA-061/062 durable
capture-or-defer transition to the existing supervised driver. Independent
producer snapshots can lag a complete notification feed without forcing a
new review and fenced recovery for every ordinary propagation delay.

`step_with_publication_deferral` accepts the existing committee callback.
`step_from_files_with_publication_deferral` uses the existing concrete policy
reader and its independent native source lease. Both take the existing witness
reader and an optional feed reader. They use the original job state machine;
there is no internal polling, replacement key, review rebase, or second ledger.

Before dispatch, a cut-bound snapshot may be retained as Deferred only when its
identity, generation, bytes, structured floors and included cut are consistent
and monotonic, but its cut lags selected invalidations in a COMPLETE feed. Each
acquisition durably withdraws witness eligibility before committee/file reads.
A deferred observation installs no current witness, is not fresh, and consumes
its acquisition cycle. A later step must withdraw and read again. A caught-up
snapshot must still pass the original exact validation and all authority gates.

Reports preserve the original driver Result, including Incomplete. A successful
read is not an installation receipt; `captures` contains only acknowledged native
capture outcomes. `waiting_for_producer()` identifies a final acknowledged
Deferred observation only while the same job remains ready, its committee/control
basis is intact, and its matching human key and frozen action are unexpired.
It does not claim that every other gate will pass after catch-up. Callers must
bound retries and service stop/cancellation between calls. Historical reports
cannot be passed back as authority or used instead of current observations.

One call performs at most the original two acquisitions. Waiting before initial
authorization reserves nothing. Waiting after successful authorization retains
the SAME permit and reservation; it neither charges twice nor requests another
human approval. Committee loss/drift is not renamed producer lag. Missing feed
records, rollback, conflicting contents, foreign identity and installation/storage
failure retain their original refusals and quarantine rules.

After dispatch this API delegates to the unchanged strict publication path.
It never adds a wait window or extends execution eligibility. Known endpoint
receipts remain authoritative and source-free reconciliation stays available;
unknown charges cannot be refunded through deferral. Matching coupled-producer
readers also delegate to their existing coherent one-read acquisition, since an
image cannot legitimately lag the complete feed decoded from the same bundle.
Legacy source bindings must use the original strict APIs; opting into deferral
requires an already bound version-two capture, not an on-the-fly upgrade.

Nine authored helper-socket tests cover initial lag and successful catch-up,
lag after reservation, unchanged approvals, exact negative evidence, equivocation,
missing feed records, key expiry/revocation, independent committee/source failures,
post-dispatch strictness, cancellation, and interruption. They have not executed:
RCH and Rust are unavailable in this editing environment. No build/test result,
production activation, source-authentication claim or FA-061/062 closure follows.
