# Plan and reconcile an original publication outcome

The native publication Tower connects `next` and `resolve` to a separately
granted, bounded receipt-reconciliation operation on the original owner.
Numerical histories additionally require an explicit cumulative replay allowance. An uncertain dispatch can have a real executed or nonexecuted receipt
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
budget. Decoder and all action-predictor histories, including raw-frame
predictors, are refused before mutation. The original reconciliation transaction
can replay retained predictor events even when no decoder or learned-predictor
cache exists, so an ordinary event ceiling cannot authorize that numerical work.

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

## Reserve numerical history work explicitly

The default `resolution_port` retains the ordinary-only work contract. To
reconcile an original decoder, learned predictor or raw-frame predictor history,
the trusted host must instead select:

```rust
let (resolution, custodian) =
    supervisor.resolution_port_with_numerical_budget(32, 8, 4096, 8192)?;
```

The final parameter is a positive cumulative allowance bounded by
`operations * max_replay_events`. It reserves original journal transitions under
their retained native numerical limits. It is not measured scalar products,
CPU time, memory, or a new live generation budget.

A first numerical Resolve reserves the entire acknowledged history plus its
single proposed Reconcile event: `journal_revision + 1`. The selected per-operation
event ceiling must also contain that cut. The reservation is conservative even
when the original owner's private replay continuation can avoid repeating part
of the history. It is spent before entering the existing native reconciliation
transaction and is not refunded by failure, cheaper cached replay or lost output.

Next checks the original allowance without reserving it or invoking replay.
An absent numerical grant produces `numerical_replay_unsupported`; an insufficient
selected cumulative allowance produces `numerical_replay_limit`. A direct Resolve
in the latter case returns Capacity before changing the operation cache,
operation counter, journal or admission snapshot. An exact already-entered retry
returns its original cached result without another numerical reservation.

Every clone shares the same optional numerical counter, available through
`custodian.remaining_numerical_replay_events()`. Ordinary histories do not spend
it. Revocation, owner failure and the finite live-grant retry boundary remain
unchanged.

For a learned owner, the trusted host can explicitly select
`PublicationTower::new_learned_reconciliation_operator(mount, learned_actor, limits)`
and install that owner's independent resolution port. This constructor uses the
existing private learned owner bridge and the restricted operator vocabulary.
It grants no work by itself. The original `new_learned_observer` remains read-only,
even if a resolution port is installed, and neither constructor exposes
ordinary payload intake or publication keys.

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
request, with a ready original clock, unused operation identity and sufficient
selected operation/replay allowance. Numerical histories additionally require
the separate complete-cut reservation described above.
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

## Select the authenticated ordinary operator service

The ordinary Tower service exposes this same original grant when its public
`fa.tower-operator-service/1` profile explicitly selects:

```json
"resolution": {
  "queries": 32,
  "operations": 8,
  "max_replay_events": 4096,
  "authority": "reconcile_existing_receipts"
}
```

This is an optional member of the complete operator profile, not a standalone
profile. Omission or `null` leaves reconciliation disabled. The closed object
requires all four fields, the exact authority marker and positive bounded
integers (at most 1,024 queries, 128 operations and 4,096 replay events);
unknown fields and malformed selections fail before setup. A
resolution-only service may set inspection and admission-explanation counts to
zero, retain `explanation_disclosure: "none"`, and omit the other optional grants.

The service retains one original custodian across healthy reconnects, request
handoffs and terminal reply grace. Its selected Permit purpose is confined by
the restricted operator constructor. Neither an actor request nor a different
socket can acquire that grant. A learned actor or resident learned sequence
rejects a selected resolution profile before clock/source acquisition, store
creation or listener setup; its observer remains read-only.

Start an ordinary service with both `--operator-profile OPERATOR_PROFILE` and
`--tower-generation 41`, following the existing
[live operator setup](LIVE_TOWER_OPERATOR.md#select-the-operator-endpoint).
Save the explicit `next` request above as `NEXT_JSON`, then run:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE NEXT_JSON
```

When the returned Known plan has `can_reconcile: true`, select the complete
`result.value.knowledge.value.action` object as the contents of `RESOLVE_JSON`
and submit it explicitly:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE RESOLVE_JSON
```

Keep its address, operation number, idempotency key and full expected basis
unchanged. Unknown/refused plans and null actions cannot supply a guessed
predecessor. The two calls consume two connections; retain sufficient connection,
exchange and lifetime allowance, and ensure both client processes match the
selected operator UID/GID/PID policy. The command freezes its request files before connecting, so an action
returned by one exchange must be selected for a later invocation. Neither the
client nor `next` automatically extracts, executes, refreshes or retries it.

The client validates the complete native subject, registration, operation,
Knowledge basis and closed action profile before output. For Resolve it also
checks the exact expected predecessor, native before/after relationship and
endpoint outcome/stage. It preserves explicit Unknown cost, ranking and
eligibility. An altered action, receipt or response is rejected before a later
explicit request can run. The existing finite parser, response, elapsed, byte
and I/O-attempt bounds still apply.

## Validation boundary

Eighteen native, two decoder and eight public integration regression functions
cover original congress/human approval and publication, actual retained
nonexecution receipts, missing/expired receipt liabilities, full predecessor
checks, query/operation limits, original-result retries, custodian and owner
loss, snapshot invalidation, numerical history refusal, wire correlation and
the restricted operator constructor.

A further paired regression covers raw-predictor and plain owners in both
Dispatching and Unknown states after real original congress and two-key dispatch.
Raw histories report `NumericalReplayUnsupported` and refuse direct/cloned Resolve
before operation spending or admission-snapshot invalidation. Journal, forecast,
authority and existing liability remain unchanged. The plain controls still
reconcile a missing receipt to charged Unknown and preserve exact retry custody.
This regression is authored and source-reviewed, with new execution pending.

Four service/profile, three authenticated-client and one learned-startup
regression functions additionally cover the explicit optional selection, a real
next-to-resolve acknowledgment and exact retry, stale/missing-receipt outcomes,
rejected actor submission, preserved one-use admission custody, strict response
correlation and refusal before learned startup. The socket cases use actual
Unix peer authentication and original native receipts.

Six additional native and seven public learned regressions exercise actual generated
Messages, original congress and independent human approval, executed and missing
receipts, exact/short/default numerical allowances, shared retry custody and
the unchanged observer boundary. A successful reconciliation advances only
the original receipt transition; it does not generate another token, publish
another message or sample new evidence.

The changes receive independent source review. The workspace was unavailable during part of implementation;
the restored environment has no RCH runner or Rust toolchain.
No compilation, formatter, Clippy, doctest, runtime test or full RCH gate pass
is claimed for this increment. Source-manifest hashes identify submitted bytes.
No full FA-138, production-activation or Bead acceptance gate is closed.
