# Historical policy diagnosis for learned publication

An admitted learned request can be denied by the exact policy before a helper
round exists. Receipt inspection preserves that disposition, while review audit
has no completed round to inspect. The learned recovery families can now retain
the original admission during their existing numerical replay and return the
same immutable `FileProposalInvestigation` used by ordinary proposal diagnosis.

This is historical **admission-policy diagnosis**. It is not a reconstruction of
every dispatch condition, a new helper judgment, a numerical counterfactual, or
permission to reopen a denied action. It serves the existing experiment and
predicate-tree contracts in plan sections 11.3, 12.5 and 17.8.

## One original replay, one selected admission

The base `FileLearnedRecovery` and all five independently guarded forms expose:

```rust,ignore
let mut recovery = recovery
    .require_request_proposal_investigation(request, scope)?;

loop {
    check_operator_deadline()?;
    let progress = recovery.progress();
    if progress.status == FileLearnedRecoveryStatus::Ready {
        break;
    }
    recovery.advance(progress.replayed_events, 1)?;
}

let investigation = recovery.finish_proposal_investigation()?;
let diagnosis = investigation.run(&interventions)?;
```

Select the same independent model, tokenizer, learned codec, stream recipe,
predictor and original guards that the existing recovery requires. In the
guarded forms these include the effective ending policy, identity registration,
current credential epoch, source/stream obligations and independent authority,
control and journal floors. The mediated forms retain the original topology
history, current graph and availability checks. The operator additionally
requires an independent history anchor and the original exact terminal reserve.

The five guarded forms are `FileGuardedLearnedRecovery`,
`FilePredictiveLearnedRecovery`, `FileOwnedPredictiveRecovery`,
`FileMediatedLearnedRecovery` and `FilePredictiveMediatedLearnedRecovery`.
Together they cover plain, raw-predictive and owned-predictive histories, with
and without topology mediation. The base learned cursor retains its original
recipe contract; the guarded forms add their existing independent requirements.

Registration must precede every replay event. Zero request IDs, duplicate
registration and registration after replay has started refuse. The selection
uses the first original `SubmitRequest` through the actual request book.
An external request ID is not assumed to equal its internal attempt number.
A different direct proposal with that number, a later retry or another request
cannot substitute for the selected admission. A retained NotAdmitted request
has no frozen proposal and yields no fictional diagnosis.

Learned message and finish submissions already use that same native event.
The original selector captures the exact historical policy input, compiled
witnesses and initial Reviewing/Denied state immediately after the original
reducer applies the selected admission. The entire remaining numerical and
authority history must still verify. Later policy changes, cancellation,
execution or withdrawal do not rewrite the original question.

The selected path executes the same original reducer once per event. It does
not replay the numerical prefix again to reconstruct an experiment, reopen live
sources or regenerate output. Replay failures and caught unwinds remain terminal.
Guarded adapters repeat their complete Ready checks before finalization.

## Offline custody and read-only finalization

This path uses the original learned recovery cursor and its exclusive Store
lock. It is an **offline** diagnostic: a running owner must release the lock
before it can open. The older ordinary `FileProposalRead` remains an immutable
image reader that can run beside a live owner; its behavior is unchanged.

The learned finalizer requires complete successful replay, rereads canonical
bytes under that same lock, and refuses if the image changed. It then releases
the private machine and lock and returns only the immutable investigation.
There is no recovery Fence, journal append/replacement, durability confirmation,
staging cleanup, logical clock observation, helper call or publication. A full
journal remains inspectable. Dropping an unfinished cursor also preserves the
canonical journal and staged evidence.

The result records two cuts: the selected admission snapshot and the ending
verified journal snapshot. Its ending state may already be cancelled, dispatched,
executed or otherwise terminal. Another authorized writer can advance the
journal after the lock is released; that does not update this historical result.
The result is not a new durability acknowledgment and cannot produce a successor
archive as though a mutation had been acknowledged.

Independent anchors/floors and exact input binding remain necessary custody
assumptions. This feature does not authenticate supplied archives or prevent
joint rollback. Access to the private operator configuration, policy literals
and historical evidence remains privileged.

## Existing counterfactual evaluator and bounded search

`FileProposalInvestigation::run` uses the existing `ProposalExperiment`.
Payload, target, units and exact-preimage key interventions remain limited by
the registered `InterventionScope`. Every branch starts at the same original
admission. Hypothetical edits never write to an evidence source, alter numerical
state or inherit the old request's approval.

`begin_repair_search` uses the existing cooperative subset-search cursor and
its complete case/edit-byte admission. Unknown or refused branches stay in the
record, cancelled searches retain unreported masks, and minimality is established
only within a completed registered menu. No new evaluator or search solver is
introduced.

A satisfied counterfactual still requires fresh independent review of a new
proposal. It does not predict helper votes or validate a changed payload against
the original numerical forecast. Every returned diagnosis has Experiment
purpose and no permission. The immutable investigation has no owner conversion
and cannot be used as a `FilePermit`.

See [the original proposal experiment](PROPOSAL_POLICY_DIAGNOSIS.md),
[operator predicate-tree diagnosis](OPERATOR_PROPOSAL_DIAGNOSIS.md) and
[bounded proposal search](OPERATOR_PROPOSAL_SEARCH.md) for the shared semantics.

## Work and interruption

Original numerical replay may be expensive. An event quantum limits original
transition attempts; it does not bound synchronous decoding, a numerical step,
canonical reads or policy evaluation in wall-clock time. Callers should check one
invocation budget before opening, between events, at Ready, after finalization
and before output. Historical replay accounting is separate from the subsequent
bounded experiment/search accounting.

## Native verification checkpoint, 2026-10-28

Eight new `proposal_diagnosis` functions are part of the original
`mediated_learned_receipts` target, which now contains 26 functions. They
exercise all five guarded forms across six plain/raw/owned and topology
configurations, actual denied and successful admissions, durable nonadmission
without a frozen action, independent message/finish selection, original request
identity, registration/drop boundaries, full guard/floor/credential checks,
canonical and semantic-suffix failures, full capacity and untouched staging.

The full real library and complete native integration target pass metadata
checking on the available nightly-2026-08-31 compiler. The two forbidden
owner/permit snippets fail with the expected E0599 and E0308, respectively.
The production-library source manifest is
`a29149b5729e2c229dc628907c770ebb65edf21dfad35880b93dafcde9af82fd`.

The initial metadata/link attempts failed on a formatter filename banner
accidentally prepended to the new module. Restoring the saved source and using
normal file formatting resolved that integration error. A subsequent full
Cranelift library build was SIGKILLed by shared-host OOM pressure after 31.69
seconds, before a usable linked library existed. The eight new functions remain
unexecuted at this checkpoint. No original source body, assertion or libtest
registration was removed; prior receipt passes are historical evidence for
their earlier source. Canonical workspace/RCH qualification and Beads remain open.
