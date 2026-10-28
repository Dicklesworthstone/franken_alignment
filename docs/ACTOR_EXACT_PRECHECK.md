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

The capability is also available through a separate bounded byte adapter,
`requests::actor::precheck_wire::FilePrecheckWire`. Existing Submit/Poll/Cancel
commands and process transports do not silently gain policy access. An
integration must explicitly bind the separately granted port to its chosen
actor. A bounded Unix connection and Linux kernel-credential peer session now
carry this same capability across a process transport. They do not create a
listener, CLI service, deployment profile or provider identity; in-process role
separation and message correlation are not OS isolation.

## Bounded actor byte interface

```rust
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    FilePrecheckWire, PrecheckRequest, encode_precheck_request, decode_precheck_response,
};

let request = PrecheckRequest { query: 9001, proposal };
let bytes = encode_precheck_request(&request)?;
let mut wire = FilePrecheckWire::new(precheck, 16)?;
let response = wire.exchange(&bytes).encode()?;
let observation = decode_precheck_response(
    &response, request.query, request.proposal.expected_policy_epoch,
)?;
```

Keep the original disclosure custodian alive on the supervisor side. The adapter
owns only its weak actor capability and a finite exchange allowance; it cannot
obtain the host, refresh evidence, observe time, install policy or submit an
effect. The optional transport below supplies framing, output backpressure and
kernel-peer admission. Grant provisioning, listener ownership, scheduling and
whole-session deadlines remain the integrating host's responsibility.

Requests use the closed `fa.actor-precheck/1` schema and responses use
`fa.actor-precheck-response/1`. Decimal-string identifiers retain all u64 bits;
payloads use bounded lowercase hexadecimal. The strict parser rejects duplicate
or extra fields, unsupported versions, invalid numeric spellings, oversized
payloads and authority-bearing additions. An ordinary actor command is not a
precheck request. Response encoding has fixed fields and enumerated outcomes;
no diagnostic string, evidence byte, helper identity or policy tree is included.

Every response is marked `precheck: true`; a passed result must explicitly say
`review_required: true`. The response decoder checks the original query and
expected policy epoch, including the nested known basis and stale-generation
binding. It accepts only the precheck's closed Knowledge states and coarse
errors, never an effect outcome, permit, pending submission or fabricated
absence. Decoding verifies representation and correlation, not authenticity
or truth. Retrying a query is new work, not an idempotent effect operation.

`FilePrecheckWire::new` requires a positive bounded exchange allowance.
`exchange` charges an entered frame before parsing, including malformed frames.
Exhaustion performs no further parsing or evaluation. Reconstructing an adapter
does not reset the native grant's shared evaluation allowance; creating another
adapter's parser allowance is an explicit host decision, not a global network
rate limiter. The original grant's conservative invalidation/revocation rules
are unchanged.

### Declared disclosure boundary

| State | Actor-visible information | Still excluded |
| --- | --- | --- |
| No granted capability | No precheck adapter is provisioned | Policy, source and review data |
| Known pass or denial | Coarse exact outcome; disclosure/query labels, policy epoch and captured clock cut | Predicate trace, literals, source values, helpers, votes and authority |
| Wrong expected epoch | Supplied and captured public policy epochs | Internal controller sequence and review data |
| Owner/source unavailable or invalidated | Coarse unknown state | Private failure detail and current internal history |
| Revoked or dropped custodian | Withheld requiring exact-policy disclosure | Former disclosure data |
| Malformed or exhausted exchange | Closed coarse error | Partial decoded input and internal diagnostics |

Repeated allowed queries can infer exact-policy properties. This leakage is
intentional and bounded by the explicit grant, not eliminated by the byte
protocol. The full deployment profile options and witness-sensitive caching
remain separate unfinished FA-142 work; this adapter does not claim them.

## Authenticated precheck process transport

`FilePrecheckChannel` wraps the original wire in bounded newline framing.
`UnixPrecheckConnection` drives an already connected Unix socket without
creating a task, runtime or listener. On Linux, use `FilePrecheckPeerSession`
when the host needs kernel-credential admission before any request is read:

```rust
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    FilePrecheckPeerSession, FilePrecheckWire, MAX_PRECHECK_FRAME_BYTES,
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;

// trusted_peer_policy is independently provisioned, never learned from a request.
// Keep the original disclosure custodian with the supervisor.
let wire = FilePrecheckWire::new(precheck, 16)?;
let mut session = FilePrecheckPeerSession::new(
    trusted_peer_policy, wire, MAX_PRECHECK_FRAME_BYTES, 4,
)?;
session.attach(accepted_socket)?;
let report = session.drive(DriveBudget::default())?;
// The host schedules the next bounded drive from report.status.interest().
```

`PeerPolicy` requires both the supplied UID and GID and can additionally require
a PID. It verifies this socket's kernel-observed connection credentials before
reading bytes or moving the original channel. A refused candidate is dropped
without a response, an evaluation, a parser charge, or disruption of an active
connection. These credentials do not attest executable code, a remote principal,
or the later recipient of a transferred descriptor. UID/GID-only policies admit
all connecting processes with those credentials. The policy must be trusted
configuration, not an identity inferred from the candidate being checked.

The channel retains at most one bounded input document and one response. A
newline is mandatory; EOF does not complete a fragment. A one-byte frame excess
closes the channel before parsing. Pipelined bytes after the first newline stay
unprocessed until that response is fully written **and flushed**. Native async
integrations can use `feed`, `pending_output`, `acknowledge_written` and
`acknowledge_flushed` with the same barrier.

Each `drive` requires positive read-byte, write-byte, frame and I/O-attempt
allowances within the original transport maxima. It reports actual progress;
`Interrupted` spends an I/O attempt, `WouldBlock` yields, and terminal socket
failures latch. Scheduler interest distinguishes buffered local work from
readability, writability and closure. The borrowed descriptor is a capability
for the **trusted scheduler**, not an isolation boundary against a hostile host
that can otherwise use or duplicate it. Socket work is bounded per drive;
the synchronous exact evaluation is not preempted mid-call, and the host must
enforce its whole-session deadline and bound rejected candidate attempts.

The session owns one original channel across explicit `disconnect`/`attach`
cycles. Its frame allowance and the native grant's shared query allowance never
reset on reconnect. Only transport fragments and unsent replies are discarded;
an evaluated query stays charged even when its reply is lost. Connection
admissions have a separate finite limit; rejected candidates do not consume it.
Exhausting the original wire closes further intake after the final reply drains.
Repeating a query, including its old label, is new work, not an idempotent retry.

`session.revoke()` irreversibly closes that ingress but does not revoke other
native port copies. Revoking or dropping the original disclosure instead makes
every such port return `Withheld`; the session does not renew it. Any original
owner revision change still makes the captured cut unavailable. No precheck
frame, reconnect or socket teardown observes time, refreshes a source, issues a
new grant, consumes a submission ID or changes the journal/effect ledger.

This is a native integrating-host surface, not a new `supervise_publication`
service. That operator currently acquires its fresh source on a new Submit;
initial precheck provisioning requires a separately explicit supervisor-owned
source/clock observation phase. The transport deliberately does not introduce
one implicitly. Generated, learned, predictive and cumulative-stream histories
retain the native disclosure refusal rather than falling back to raw proposals.

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

The byte-interface regression source adds request/response round trips, actual
source-backed pass/deny/stale/unknown/withheld exchanges, malformed/cross-protocol
input, full-width identifiers, response correlation, strict review-required
labeling, parser/evaluation budget separation and unchanged journal/request
state. Targeted and full remote-check attempts exit 127 because `rch` is absent;
these new tests and compilation have not executed.
