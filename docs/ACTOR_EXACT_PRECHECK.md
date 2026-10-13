# Revocable, source-backed actor precheck

## Capability and integration

The original `FileActorSupervisor<FileOversight>` can explicitly disclose a
bounded exact-predicate precheck. The existing actor submission port does not
receive this capability automatically. This implements a scoped Rust integration
of the plan's `exact_predicates` disclosure/precheck contract (§17.8, FA-142),
using the existing effect owner and native `Policy::evaluate`.

After the supervisor has observed time and acknowledged a complete, fresh file
policy source, it can provision the separate capability:

```rust
let revision = supervisor.host()?.revision();
let (precheck, disclosure) = supervisor.disclose_exact_precheck(revision, 81, 16)?;
// Give only precheck (or a clone) to the actor; retain disclosure separately.
let observation = precheck.check(9001, &proposal);
// Later, or when this scope ends:
disclosure.revoke();
```

The same method is accessible through `FileSupervisedDriver::supervisor()`.
`FileActorPrecheckPort::check` accepts the existing `ActorProposal`; it never
accepts actor-supplied scope, witnesses, source data, time or a policy. It returns
`Result<FilePrecheckObservation, ActorError>`. The observation uses the existing
`Knowledge` wrapper with a distinct `FilePrecheckBasis`, not an admitted request's
`ActorBasis`. Existing actor observations and wire formats keep `ActorBasis` as
the default generic parameter and retain their previous variants and meaning.

This increment exposes an in-process Rust capability, not a new CLI command or
wire operation. Existing process transports do not silently gain policy access.
An integration must decide explicitly which actor may receive this capability.
In-process role separation is not OS isolation or provider authentication.

## What a result says

`Known(PrecheckPassed)` means the candidate passed the disclosed exact policy
and the captured owner's target, target ceiling, policy epoch, strict deadline
and available-unit screens. It is NOT admission, helper approval, human consent,
a reservation, a prediction of all gate outcomes, or permission to dispatch.
Credential, identity, empirical review, storage capacity and other subsequent
requirements still apply. For example, an otherwise valid request can pass
precheck while its required helper is unavailable; normal admission/review must
still refuse or hold it. No code consumes a precheck result as authority.

`Known(DeniedByPolicy)` is a coarse exact refusal. It does not disclose which
helper voted, any empirical threshold, source value, policy literal, internal
attempt, controller sequence, or detailed reason for a restriction. Repeated
queries can nevertheless infer aspects of exact predicates: that is a declared
consequence of this explicit disclosure capability, not a secrecy guarantee.

The basis names the supervisor's disclosure label, the query label, policy epoch
and already-observed elapsed-clock tick. The labels are provenance annotations,
not unique durable IDs or signatures. No request ID is reserved by a query.
A passed query numbered 9001 does not prevent a later real submission using 9001.
Read-query results cannot be passed where `Knowledge<ActorOutcome>` or a
`FilePermit` is required.

An actor-supplied wrong policy epoch produces `Stale` for that public epoch.
Changed owner history, unavailable source, a busy owner borrow, or a dropped
owner instead produces `Unknown(ControllerUnavailable)` without revealing a
new internal sequence. Missing or revoked disclosure produces `Withheld`.
Malformed inputs and evaluated-query budget exhaustion use `ActorError`.

## Original source and conservative invalidation

Issuance calls the original `capture_file_policy_state` and copies only the
acknowledged exact source/policy data and deterministic screens. An arbitrary
one-shot admission snapshot cannot stand in for this source. Every query checks
source eligibility again without refreshing it. Known source acquisition loss
therefore refuses even when the acknowledged journal revision did not advance.

A grant is bound privately to its precise issuing owner and journal revision.
ANY acknowledged transition invalidates it, including a clock-only transition.
This is deliberately conservative, not fine-grained witness caching. A new
source observation can enable a newly issued grant; it cannot revive old ports.
The precheck never observes time itself: results describe the clock cut stated
in the basis, not the current wall clock or the future dispatch boundary. The
supervisor remains responsible for timely observation and fresh issuance.

Only ordinary, source-backed publication is supported here. A raw proposal is
not a valid substitute for a numerical generation, predictive observation or
cumulative stream release. Histories containing decoder, consistency or stream
operations refuse disclosure instead of approximating those lanes. Reopened
owners need a newly acknowledged source before a new disclosure, and all old
ports remain attached to the old gateway.

## Lifetime, bounded work and unchanged effects

A non-cloneable `FilePrecheckDisclosure` owns the immutable disclosure data.
Ports hold only weak references to that custodian and the original gateway.
Dropping/revoking the custodian closes every port clone. Neither the custodian
nor a port can keep the authority's file lock alive after its supervisor drops.
Revoking disclosure does not cancel real requests or disable the separate
ordinary actor submission port.

Each grant admits 1 through 128 evaluated queries. All clones share one counter;
repeated labels are real new work, not a free replay. A current, available query
is charged before fallible freezing or policy evaluation. Malformed proposals,
policy errors and unwinds do not restore its allowance. Revoked or ineligible
grants perform no evaluation. Input payloads are bounded before copying. The
native policy, witness and source limits remain unchanged.

The bound counts entered evaluated queries, not CPU time, caller invocations,
network frames or allocator memory. A host may explicitly grant another budget;
there is no automatic renewal or claim of a global rate limiter. Source/policy
snapshot copying and the bounded history scan occur once per grant. There is no
history replay, inference, file or clock read, journal replacement, helper call,
human offer, reservation, dispatch or receipt acknowledgment in a precheck.

The existing one-shot admission snapshot is neither read nor consumed by the
precheck. Real `submit`, `poll`, `cancel`, review, two-key authorization and
publication retain their original implementations.

## Regression and qualification scope

The regression source uses real file-backed `FileOversight` owners and original
source observations. It compares passing/denying prechecks with subsequent real
actor admissions, verifies unchanged journal bytes and effect counters, and
checks that a query consumes neither a request ID nor the admission snapshot.
It covers revocation across clones, shared exhausted budgets, malformed and
maximum-sized payloads, exact target/deadline/epoch/unit boundaries, missing,
expired and failed source reads, successful fresh disclosure after source repair,
borrow contention, owner drop/reopen, and a source-backed stream refusal with an
ordinary-publication positive control. Compile-fail cases protect the custodian,
owner and effect-key boundaries.

This is authored reference source, not a completed production FA-142 claim.
Compilation, runtime tests and the full unchanged gate require execution on the
configured remote builder. Missing launcher/toolchain is not qualification.
No admission/dispatch semantics, dependency inventory, journal format, policy
relaxation or production feature gate is changed by enabling these source APIs.
