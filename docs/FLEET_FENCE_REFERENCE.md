# Fleet admission fences and acknowledgment frontiers

## Consumer and qualification

`delivery::fleet::FleetCoordinator` coordinates already constructed
`DeliveryBroker` authorities. The original proposal, positive review,
authorization and dispatch paths enforce the enrolled domain's lease and stop.
There is no second budget ledger and enrollment never constructs, copies or
transfers effect rights. Distinct domains retain their independent funding.

This is std-only, in-memory reference progress toward FA-120 and plan 8.11,
15.3 and 15.4, grounded in FI-A02. It does not close the production packet or its
FA-034/FA-041 prerequisites. Rust compilation, formatting, Clippy and tests have
not run in this session: Cargo, rustc, rustfmt and RCH are unavailable. Eight
public integration tests are supplied in `tests/fleet_fences.rs`; they are
source, not passing execution evidence. Beads remain open.

## Ordered issue, local enforcement, acknowledgment

Enrollment requires the expected coordinator revision, a unique nonzero domain
and authority scope, an explicit finite lease, and an authority with no admitted
attempts. The roster freezes when the first fence is issued. There is no removal,
rejoin, lease extension, resume or history-clearing operation. This bounded
profile is not a long-running renewal or distributed membership service.

A fence selects all registered domains, one tenant, one principal within a tenant,
an adapter/contract pair, or an explicit domain set. Unknown or empty selections
refuse rather than silently becoming All or omitting an unknown domain. Each
new command advances a fleet floor. Retrying the same identity and exact scope
returns the original command; reusing the identity with different scope refuses.

Issuance alone changes no recipient authority. A domain can still dispatch
before receiving the command, within its existing lease and all original checks.
`install_fleet_fence` validates the exact coordinator/registration and expected
local sequence/epoch. It stages the existing Rights accounting, advances the
local revocation floor to at least both the command floor and old epoch plus one,
cancels undispatched attempts, and suspends the original scope. Only Authorized
reservations are refunded. Dispatched, unknown, irrecoverably unknown and terminal
effects retain their real states. A successful acknowledgment names the actual
control sequence, old/new epoch, cancelled attempts and refund. A duplicate
installation returns that acknowledgment without repeating the transaction.

The coordinator must separately receive the acknowledgment. Until then its
frontier remains AwaitingAcknowledgment or LeaseExpired, never an optimistic
broadcast-success state. A later, stronger acknowledged fence can cover an older
command, but the report retains the actual later installation order.

## Reference ordering and lease assumptions

This model uses one explicit shared scheduler order for command issue, successful
dispatch admission, fence installation and domain observation. It is NOT any
domain's control sequence. Local control sequences are never compared across
domains as if they were globally ordered. The scheduler is a single-threaded
reference oracle, not a distributed sequencer, causal-clock protocol or new
authoritative journal. No distributed latency or synchronization cost is hidden
inside a production-performance claim.

Enrolled domains also share the declared logical lease-clock floor. A local
clock older than that floor cannot admit work. Observing the lease boundary
blocks admission even if the domain has not received a fence; copying an old
local time cannot reopen it. LeaseExpired therefore denotes the separately
enforced time condition, not a received acknowledgment or proof that reservations
were cancelled. Real partition leases need qualified clocks, ownership and
anti-rollback contracts. This implementation supplies none of those mechanisms.

## Effects and observation gaps

Successful dispatch records its scheduler position in the existing owning broker.
A refused dispatch records no admission and leaves its permit accounting intact.
`fleet_observation` produces an opaque complete snapshot of retained admitted
attempts, original ledger dispositions and accepted endpoint outcomes. It copies
neither payloads nor policy/helper evidence into fleet telemetry. The coordinator
accepts only its registered domain's observation; stale observations refuse.

`FleetDeliveryKnowledge::Unobserved` is distinct from an observed empty prefix.
The report lists known admissions after command issue and unresolved admissions,
including those before issue. A post-issue list becomes complete only after a
known admission stop AND an observation at or after that stop. Acknowledgment
alone does not close the telemetry gap. Closed admissions do not imply known
external outcomes: unresolved effects stay visible until receipt reconciliation.

A fleet acknowledgment is an ADMISSION stop, not an endpoint-drain receipt.
A previously admitted message can execute later, including after acknowledgment.
Endpoint incarnation fencing, status queries and nonexecution sealing remain the
separate existing protocol. A fleet fence never proves nonexecution, clears a
pending stream slot or manufactures a refund. Dispatcher restart, actor reset,
policy changes and endpoint fence acknowledgments cannot clear the fleet stop.

## Bounds and remaining work

The coordinator admits at most 64 domains and 128 commands, subject to smaller
configured limits. Each domain's observations are bounded by its original
128-delivery cap; cancellation lists by its original attempt cap. Latest domain
observations replace older snapshots but cannot remove an earlier admitted
attempt from the snapshot because the underlying broker retains all keys.
No new capacity is needed to acknowledge an already issued command or reconcile
an existing endpoint obligation. Counts are logical bounds, not measured memory
or throughput. External copies of evidence confer no authority.

The implementation assumes trusted deployment enrollment and supplied capture/
clock facts. It does not prove complete fleet inventory, provide authenticated
network messages, tolerate an OS crash, implement durable distributed grants,
renew leases or enable governed resumption. The old decision-archive profile
does not thereby acquire a fleet proof. Those production obligations remain open.
