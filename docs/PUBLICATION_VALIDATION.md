# Final-cut publication validation (FA-062 reference profile)

## Consumer and status

The existing `DeliveryBroker` consumes this gate immediately before authorizing
an attempt and again in its common dispatch path, before the controller consumes
the effect permit. `OversightBroker` exposes the same configuration and input
operations without exposing its inner delivery owner. Ordinary and human-key
dispatch share the check. The policy, committee, human, deadline, revocation,
mediation, fleet and endpoint checks remain independent requirements.

This is an opt-in, in-memory reference capability, not activation of a production
feature. The source and regression tests are implemented; Rust compilation,
formatting, Clippy and execution have NOT been performed in this editing
environment, which has neither the Rust toolchain nor RCH. FA-062 is not closed.
The verifier must run `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask
-- check` on the exact integrated revision. No previous execution evidence is
claimed for this code.

## Host integration

Before the first proposal, call `enable_publication_validation(PublicationLimits)`
on the delivery or oversight broker. The configured profile applies to every
subsequent proposal and cannot be disabled or widened. The lifetime binding
capacity is at most `MAX_PUBLICATION_BINDINGS` (16); rejection occurs before a
new proposal is inserted, and cancellation does not reopen that capacity.

After `propose`, use its **controller-produced frozen action**, including the
policy-derived dependencies, to bind a `PublicationJudgment`. The binding holds
one structured `WitnessJudgment`, one opaque `OpaqueJudgment`, or both. Binding
requires at least one lane and must happen while the attempt is Reviewing,
before authorization. It is write-once: changing the judgment, dropping a lane,
or rebinding the action requires a new reviewed attempt, not editing this one.

Call `record_publication_inputs(attempt, expected_revision, Some(inputs))` with
actual current host observations before authorization and publication. Call it
with `None` when those observations are unavailable. This changes observations,
not reviewed requirements. The exact predecessor prevents stale writers from
overwriting a newer observation. Structured revision, control-cut and semantic
epoch high-water marks survive unavailability and actor reset.

The current input source remains a trusted host assertion. This interface does
not capture a provider request, authenticate an adapter, prove that the host
recorded every remote change, or establish wall-clock freshness. Existing live
policy-state capture and whole-committee checks are not replaced by it. The gate
is not yet represented in the persistent journal protocol; an in-memory enabled
profile must not be reconstructed as a durable enabled profile by inference.

## Final-cut semantics

The broker recomputes validation, using the immutable retained action and the
latest recorded inputs. It never accepts an external `PublicationReport` as
permission. Structured validation uses the existing bounded FA-060 exact
refinement engine: value/version dependencies, absent keys, empty ranges,
complete range membership, source/domain semantics and closing frontiers are
rechecked. Unrelated updates at a newer cut can still pass.

Opaque equality includes the entire submitted byte stream, input profile,
tokenizer/model/policy epochs, ordered parts and omissions. An explanation such
as “only these bytes mattered” does not narrow it. Both lanes share one fixed
step/byte budget. Exhaustion is Incomplete, not validity; there is no unbudgeted
fallback or carry-over of partial results across changed cuts. A standalone
FA-060 cursor remains the API for resumable work against one immutable basis.

`publication_validation(attempt)` returns the last comparison report, including
work spent before a refusal. It is historical diagnostics, not a claim that the
current inputs still match it. `publication_input_revision` exposes the current
host-observation revision. Neither result grants authority.

## Failure behavior and regression coverage

A refusal returns no sendable envelope and does not consume the effect permit,
refund an existing reservation, or consume a human key. A previously admitted
or unknown effect remains reconcilable even when both the publication input and
the committee input become unavailable. Endpoint receipts still determine
execution and nonexecution; missing evidence is never a refund proof.

`oversight/publication/tests.rs` covers the two-lane validator, action/lane
binding, structured and opaque invalidation, frontier gaps and shared budgets.
`tests/publication_delivery.rs` exercises actual committee review, authorization,
delivery and endpoint transitions, including successful newer-cut publication,
negative dependencies, missing evidence after approval, fixed-budget refusal,
input-update compare-and-swap, high-water marks, two-key dispatch without a
fallback, receipt reconciliation during evidence loss and atomic capacity
rejection. These tests are authored but awaiting the verifier execution above.
