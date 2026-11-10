# Plan and reconcile an original publication outcome

The native publication Tower connects `next` and `resolve` to a separately
granted, bounded receipt-reconciliation operation on the original ordinary
owner. An uncertain dispatch can have a real executed or nonexecuted receipt
that its ledger has not yet accepted. This path lets an authorized operator
request that original reconciliation without obtaining a publication key,
resending a message or inventing a nonexecution result.

This is a scoped FA-138 increment under plan sections 17.3, 17.8 and 25.1.
It returns at most one connected action. Complete dispatch explanations,
additional native actions, measured cost/information gain, ranked selection,
rehearsal and the planner's execution decision card remain open.

## Grant the original operation explicitly

The trusted host retains the original actor gateway and supervisor:

```rust
let (resolution, custodian) = supervisor.resolution_port(32, 8, 4096)?;
let mut tower = PublicationTower::new_reconciliation_operator(
    mount, actor, PublicationLimits::default(),
)?;
tower.set_resolution(Some(resolution))?;
```

The three bounds are independent: entered `next` reads, entered `resolve`
operations and the maximum retained history including the new Reconcile event.
The maximums are 1,024 queries, 128 operations and the existing native journal
event ceiling. The selected event ceiling does not supply a numerical replay
budget. Decoder and learned-consistency histories are refused before mutation.

Retain the noncopyable custodian. Every port clone shares the original counters
and operation-result cache. Dropping or revoking the custodian withdraws all
ports; weak handles cannot keep the original authority lock alive or attach to
a reopened owner. A matching numeric scope does not substitute for the same
owner allocation. A rejected foreign replacement withdraws the previously
installed resolution port.

The dedicated reconciliation constructor has a closed operator-only verb set:
`situation`, `explain`, `why-held`, `next`, `resolve` and `policy-replay`.
Each still requires its original independently selected capability and purpose.
Both native `next` and `resolve` require the operator role and Permit purpose.
That purpose cannot enable `propose`, `hold`, actor ticket polling, precheck
or arbitrary publication through this constructor. These checks precede source
admission, ticket work and native calls.

The ordinary broad Rust constructor retains its existing explicit authority
contract. The learned observer retains its existing read-only verb set and
does not accept these new commands. No existing observation grant creates a
resolution grant.

## Ask for a concrete action

Use the original external-request address, not a ledger attempt number.
For the same scope and registration used in the diagnosis examples:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "plan-reconciliation-701",
  "address": "fa://1/attempt/request-3-2-4-5-701@41",
  "verb": "next",
  "arguments": {"operation": "9001"},
  "idempotency_key": null
}
```

The caller chooses a nonzero prospective resolution operation number.
Planning does not reserve that number. A native `next` request is distinct from
the reference snapshot's `{"max_cost":"8"}` ranking profile. The native route
refuses that profile instead of inventing a price or treating an unknown cost
as zero.

Each entered read resolves the original request-to-attempt map and captures
one immutable acknowledged cut. Missing and unadmitted requests remain Unknown.
Request generation zero is valid and remains distinct from journal revision,
control sequence and Tower registration generation.

A plan can offer reconciliation only for an actual Dispatching or Unknown
request, with a ready original clock, supported ordinary history, unused
operation identity and sufficient selected operation/replay allowance.
Other recorded states and limits return no action with a typed block reason.
This is coverage of one connected native operation, not proof that no other
useful action exists.

The result kind is `resolution-next`, schema
`fa.publication-resolution-next/1`. Its Known value contains the actual stage,
`can_reconcile`, a nullable `block_reason`, and a nullable complete Tower request
in `action`. Cost estimate, expected information gain, ranking and current
publication eligibility are all explicitly Unknown.

When present, `action` is an executable six-field request, with:

- verb `resolve` and operation ID `resolve-9001`;
- the original request address at the exact nonzero registration generation;
- arguments containing `operation` and the complete captured `expected` basis;
- idempotency key equal to the canonical operation number.

The host does not execute that action while answering `next`. A caller must
select it explicitly. Any intervening owner progress can make it stale.

## Execute against the exact predecessor

`resolve` accepts no payload, clock tick, alleged receipt, replacement scope,
human key or endpoint choice. Its full expected basis contains scope, original
external request, request-local generation, actual attempt, journal revision,
control sequence and recorded tick. The native owner checks all of them before
entering work. A stale new operation changes neither the journal nor an unused
admission snapshot.

On entry, the grant spends one operation, binds its complete input to the
operation number and invalidates the old one-use admission snapshot. It calls
only the original `FileOversight::reconcile(revision, attempt)` transition.
The original canonical transaction and endpoint gates remain in force.

No new time is sampled. The original live clock must already be ready.
A reopened historical clock is not promoted into a current observation.
The operation uses the acknowledged tick without claiming freshness.

| Native result | Acknowledged state and accounting |
|---|---|
| Existing Executed receipt | Confirmed; the original effect charge remains |
| Existing NotExecuted receipt | ConfirmedNotExecuted; the original receipt may release the charge exactly once |
| No retained receipt | Unknown; the original liability remains charged |
| Retention expired | Unknown; the original liability remains charged |

The operation never sends, publishes, seals, resolves expiry on a missing
receipt, abandons uncertainty or manufactures a refund. Passing a deadline
does not turn a missing receipt into nonexecution. Accepting a real receipt
does write the original authority journal and can settle its existing
liability; this is an explicitly granted mutation.

## Retry identity and acknowledgment boundaries

Every entered operation number is bound to its full original request and
expected basis. Under the same live grant, an exact repeat returns its original
acknowledgment or refusal without another native call or operation charge,
even when the owner has subsequently advanced. Changing any bound input under
that number returns an idempotency conflict.

The live-grant cache is finite and shared across reconnects and port clones.
It is not a durable command registry. Revoked custody, a lost owner, a faulted
owner or a conflicting borrow cannot use it as a substitute for the original
authority. An entered but unacknowledged operation, including one interrupted
by an unwind, cannot repeat native work through that key.

A replacement grant or reopened owner cannot retrieve the old grant's cached
result. An old predecessor is refused after acknowledged owner progress.
Transport loss, an ambiguous canonical replacement or an output-size refusal
does not prove that the operation failed to commit. No automatic retry,
reconnection, new operation number or effect resend follows those outcomes.

The `resolution` result uses schema `fa.publication-resolution/1`, names
`idempotency_scope: "live_resolution_grant"` and declares
`may_write_journal: true`, `publication_attempted: false` and
`seal_attempted: false`. A Known result retains separate before/after native
bases and the original endpoint outcome. An exact cached result remains an
acknowledgment of its historical cut.

The sealed response binds query operation ID, native request, registration,
operation identity and expected basis for known, unavailable and refused
outcomes. Relabeling the public outer envelope cannot repurpose it.
The whole original response-size bound applies after native work, without
refund or rollback on encoding refusal.

## Validation boundary

Eighteen native, two decoder and eight public integration regression functions
cover original congress/human approval and publication, actual retained
nonexecution receipts, missing/expired receipt liabilities, full predecessor
checks, query/operation limits, original-result retries, custodian and owner
loss, snapshot invalidation, numerical history refusal, wire correlation and
the restricted operator constructor.

The changes receive independent source review. The execution environment is
unavailable and the restored environment had no RCH runner or Rust toolchain.
No compilation, formatter, Clippy, doctest, runtime test or full RCH gate pass
is claimed for this increment. Source-manifest hashes identify submitted bytes.
No full FA-138, production-activation or Bead acceptance gate is closed.
