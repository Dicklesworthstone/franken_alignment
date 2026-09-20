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

## Runnable bounded waiting

The existing `create-checked` and `submit-checked` commands accept an explicit
`fa.supervised-witnesses/4` profile. It has the same original/current capture and
feed fields as version two plus REQUIRED `max_retries` in 1..=64. It retains the
nonempty structured recipe and requires a cut-bound original capture. All older
schemas, whole-input mode and matched producer mode keep their prior behavior;
an unknown retry field in an older schema is rejected rather than ignored.

The workflow calls `step_or_wait` on its prepared profile. This returns no driver
event only for the acknowledged waiting diagnostic above. It yields to the SAME
outer loop, so the original logical/wall deadline and independent stop-control
checkpoint run before another acquisition. The existing poll interval bounds
frequency; `max_retries` bounds the number of extra wait decisions for this
prepared request. A perpetually lagging source therefore produces at most
`max_retries + 1` deferred steps before the original stop/drain path is invoked.
Even a frozen logical clock cannot create unlimited retries. No deadline, source
lease, human approval or budget is refreshed by waiting itself.

This retry count is orchestration state, not an additional authority condition or
persistent grant. It never resets within the prepared request. Fenced restart
withdraws the old job and keys rather than restarting a retry allowance for them.
Receipt-only resume remains source-free. Prepared profiles contain no positive
observation cache. The low-level `step` method remains strict; existing callers
only gain waiting by choosing the new profile and its bounded orchestration path.

Six runnable regressions cover schema admission and legacy controls, caught-up
publication versus changed negative evidence, exact one/two-retry exhaustion,
missing original cuts, source-free receipt recovery and strict post-dispatch lag.
One budget unit test checks every admitted limit and its overflow/zero neighbors.
These seven tests plus nine driver tests are authored but UNEXECUTED. The required
RCH xtask verifier cannot start because rch is unavailable; no test pass, compiler,
formatting, Clippy, production activation or Bead-closure claim is made.
