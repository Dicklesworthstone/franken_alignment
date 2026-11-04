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

## Learned operator command

The explicit command loads the original numerical recipe and requires independent
history custody:

```sh
supervise_publication create-learned-generated CONFIG.json LEARNED_RECIPE.json \
  --resume --explain-proposal \
  --history-anchor /operator/custody/history.anchor
```

The external request comes from `LEARNED_RECIPE.request`. Add
`--finish-after-message PREVIOUS_REQUEST` for an original finish; its exact
confirmed predecessor is checked on the same Ready cursor before finalization.
A finish cannot be selected as a message by omitting this option.

For topology-mediated histories also supply
`--topology-profile /operator/custody/historical-v2.json`. This new diagnostic
route requires the original version-2 source checkpoint at the exact journal
cut, whose nested history anchor matches the independent input. Version-1 work
minima, mixed pairs and stale complete source cuts refuse. The historical profile
omits `retain_checkpoint`; its `next` live source files are not read. There is
no checkpoint-repair mode inside diagnosis.

Optional investigation modes are mutually exclusive:

```text
... --payload /operator/investigation/replacement-frame.bin
... --search /operator/investigation/search-menu.json
```

`--payload` supplies hypothetical original `ActionSpec` payload bytes. In a
learned stream this payload contains the encoded release frame; it is not simply
replacement generated text. The command does not rerun the model, recompute a
forecast, validate a new release protocol or submit those bytes. Even exact-policy
sufficiency proves none of those other conditions. Empty files are explicit
empty-payload interventions, not omitted input.

`--search` uses the existing closed `fa.proposal-search/1` menu and the original
bounded subset-search implementation. It preserves every control, refused and
unknown branch, original budget accounting, explicit cancellation and missing
case denominator. Completed search and hypothetical sufficiency confer no
permission. See [the menu and result contract](OPERATOR_PROPOSAL_SEARCH.md).

The parser validates the complete original argument vector before any option is
stripped or configuration is opened. Duplicate selectors, flag-valued/missing
paths, malformed finish IDs, payload/search mixtures, retained-output flags,
receipt/reconciliation/repair modes, live continuation, reviewer options and
credibility activation refuse. This command cannot silently select a weaker
recovery or turn an explanation into an effect-bearing operation.

Recipe loading retains the original historical model/tokenizer/codec and
prediction inputs and complete guard requirements. It does not require current
policy-source files, native helper executables, reviewer sockets or next live
topology files. The original recovery lock is required; unlike the ordinary
source-only image reader, this route cannot run while another owner holds it.

The same invocation budget covers configuration and asset intake, independent
custody, pre-open, every replay predecessor, Ready, canonical reread, experiment/
search evaluation, rendering and the final pre-output boundary. The one-shot
response is encoded by the existing predicate-tree renderer or search renderer.
A deadline before output publishes no partial JSON. Write/flush failure may
leave partial stdout but preserves canonical history, staging, actual denial,
charges and effects. No new custody acknowledgment or archive is produced.

Successful exit means the requested historical report was delivered. It does
not mean that the controlled request was approved, that a later helper denial
was explained, or that a counterfactual is a valid learned publication.

## Operator verification checkpoint

Eight additional operator regression functions cover six actual learned modes,
unreviewed denial and hypothetical repair, complete/cancelled subset search,
independent guards and exact request selection, closed original-argv admission,
all elapsed boundaries, real message/finish selection, output failures,
malformed suffixes and exact v2 source/history custody. The existing fixture
bodies, assertions and test registrations are retained.

The final real-source metadata checks pass for the full native library
(15.04 seconds), complete 26-function native integration target, production
publication executable (2.21 seconds), and complete unchanged publication test
target (10.22 seconds). Both forbidden owner/permit snippets produce their
expected E0599/E0308. These results establish parsing, type and borrow checking;
they do not execute tests. The operator target check also includes the earlier
receipt-service tests whose preceding full-target checks were interrupted.

A single lower-peak alternative full library build with 256 code-generation
units was stopped after 47.18 seconds by another shared-cgroup OOM event. Along
with the earlier 16-unit attempt, it produced no usable linked library. All
sixteen new native/operator test functions therefore remain unexecuted.
The available compiler is nightly-2026-08-31 with its matching official Cranelift
backend for attempted linking; this is not canonical workspace/RCH qualification.
No failed attempt is reclassified, no original test is removed, and no roadmap
packet or Bead is closed.
