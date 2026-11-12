# Scoped ordinary authority narrowing

An independently authorized supervisor can restrict the original ordinary
publication run to an exact set of targets. The original consequence authority
intersects that set with its current ceiling. A later larger selection cannot
restore a target that an earlier restriction removed.

This implements the ordinary `NarrowAuthority` law in plan sections 9.8 and
25.2, under FA-014, FA-104 and the FA-138 Tower work. It serves FI-A02, FI-A09
and FI-A13: external control, typed consequences and useful restrictions that
preserve permitted work. The [existing consequence contract](CONSEQUENCE_REFERENCE.md)
continues to govern accounting and exact target identity.

## Scope and target identity

Each target binds adapter, object, contract version, expected resource version
and generation. All five coordinates must be nonzero. Duplicate targets refuse.
The native API canonicalizes a target set; canonical journal input must already
use that ordering. The native target limit is the original 1,024-attempt bound.
An empty selection retains no targets. An absent initial ceiling means the
original unrestricted reference ceiling and is distinct from an empty ceiling.

Expected resource versions are exact permissions. Retaining version 2 can
permit a publication that advances the endpoint to version 3, but does not
admit a later version-3 proposal. A sequence that needs several versions must
retain those versions when making its initial restriction; a later larger
selection still intersects the current ceiling and cannot restore omissions.

This operation supports ordinary artifact publication, including its existing
mandatory evidence source, checked publication, credential and credibility
guards. Decoder, learned, predictor/consistency, mediated and all stream owners
are unsupported by this ordinary operation and refuse before replay. The
existing learned prompt-checkpoint narrowing operation remains a separate
contract, with its checkpoint binding and deliberate epoch revocation.

## Original control and accounting

One accepted operation appends one original `NarrowAuthority` journal event and
advances the control sequence once. The original authority epoch and recorded
clock remain unchanged. Policy, actor state, incident count, endpoint payload
and execution count are not replaced by this restriction.

Only outside-ceiling work that has not dispatched is cancelled. An existing
reservation for such work is refunded once through the original rights ledger.
Proposed or reviewing work without a reservation supplies no refund. Retained
targets keep their original reservations. Dispatched, Unknown and
IrrecoverablyUnknown attempts retain their original states and charges.
Narrowing does not establish whether an earlier effect executed.

The acknowledgment records the previous ceiling, requested target set, actual
effective intersection, cancelled native attempts and refunded reservation
units. The requested set is never presented as the effective ceiling unless
that is the actual intersection. This is a domain restriction, not a synthetic
congress decision about one selected attempt.

Every human approval binds a control predecessor. Advancing that predecessor
withdraws the original human requests and keys, including those for retained
targets; their consumed work is not refunded. An allowed automatic reservation
does not make an old human key current. Fresh independent human approval is
required for later dispatch. Pending reviews and driver readiness keep their
existing predecessor checks as well; this operation does not silently renew
them. A stopped or suspended owner refuses a fresh narrowing operation.

The same bounded control-record allowance accounts for review, retirement and
narrowing receipts. The original durable owner separately retains at most 128
authority-narrowing operation identities. Native narrowing uses ordinary
journal capacity and does not consume protected terminal recovery capacity.

## Trusted durable API

`FileAuthorityNarrowRequest::new(operation, expected_control_sequence,
expected_authority_epoch, retained_targets)` constructs the explicit request.
The types live in `persistent::observed::narrowing`.

`FileOversight::narrow_authority(journal_revision, request)` checks the exact
original predecessor and enters the canonical native transaction. It obtains
no source evidence, fresh clock observation, helper judgment or human approval.
It does not reset the actor, replace policy, resend an effect or alter a pending
stream.

An exact healthy retry returns the original native receipt. A changed request
under the same operation conflicts. Reopening through an applicable existing
recovery path preserves that durable namespace and receipt; retrying the
restriction adds no narrowing event or second refund. The opening path's own
fencing and recovery obligations remain in force.

The new event has a distinct canonical tag. Existing event encodings remain
unchanged, and a reader that does not implement the new tag cannot accept a
history containing it. Replay applies the same original gate intersection,
accounting and key-withdrawal transition.

## Independent live capability

The original `FileActorSupervisor<FileOversight>` exposes
`narrow_port(queries, operations, max_replay_events)`. It returns a weak
`FileSupervisorNarrowPort` and a nonclone `FileSupervisorNarrowGrant`.
Dropping or revoking the grant withdraws all port clones. Ports cannot keep the
original owner alive or follow a reopened owner. A numerically equal scope
does not establish shared owner identity.

The query allowance is 1–1,024, mutation allowance 1–128, and per-operation
replay ceiling 1–4,096 events. Clones share all allowances and the same exact
result cache. These are finite native work reservations, not CPU measurements
or estimates of model safety.

Call `port.next(operation, retained_targets)` to inspect the selected
intersection. A known `FileNarrowPlan` carries the original basis, requested
targets, current ceiling, effective targets and any blocking reason. Its
`request()` returns an explicit `FileNarrowRequest` when that plan permits an
attempt. Next spends one query; it reserves no operation identity, performs no
replay, consumes no admission snapshot and executes no restriction.

The `FileNarrowBasis` binds full scope, journal revision, control sequence,
authority epoch and recorded time. `port.narrow(request)` compares all of it
against the same original healthy owner. A stale cut, unsupported history,
foreign operation identity or insufficient selected replay allowance refuses
before entering a new native mutation.

On entry, the port retains the operation identity, spends one mutation and
invalidates the original admission snapshot before calling the durable owner.
Storage failure or interruption does not replenish that work or turn an
unacknowledged candidate into a receipt. A faulted or unavailable owner cannot
export a cached acknowledgment as current evidence.

An exact retry under the same live grant returns the original cached result
without another event, replay, refund or snapshot invalidation. Changed input
under that key conflicts. The cache has live-grant lifetime; a new grant cannot
reuse an operation already recorded in the durable native namespace.

## Continue an already completed ordinary review

A completed `Continue` review may remain useful when an independent operator
narrows the run to a set that still includes its exact target. The native driver
provides a separate, explicit continuation step for that case:

```rust,ignore
let operation = driver.pending_narrowing_continuation(request)?;
if let Some(operation) = operation {
    driver.continue_after_narrowing(request, operation)?;
}
```

Both methods operate on the original process-local driver. The first reports
only a provable pending continuation; the second checks it again before changing
the driver's retained control predecessor. They require the same healthy locked
owner, a completed ordinary review in its Ready phase, the original exact action
and complete input revision and bytes, the original authority epoch, a retained
target and the original native Continue decision. A Ready job may still be
native Reviewing while it waits for its first human key. If it already owns an
automatic permit, its native reservation must still be Authorized.

Every intervening control sequence must be accounted for by an acknowledged
ordinary target-narrowing receipt held by this owner. Each restriction must
retain the selected target, and the selected final operation must end at the
current sequence. A source replacement, policy update, another review, fencing,
cancellation, suspended owner, excluded target or other unaccounted control
transition cannot be treated as narrowing. An in-progress helper review, a
dispatched or uncertain effect, and a learned, stream or mediated history cannot
use this continuation path.

The returned `FileDriverNarrowingContinuation` records the request, attempt,
selected operation, previous and current control sequences, authority epoch,
ordered narrowing operation identities and whether the original automatic permit
was retained. An exact retry returns that acknowledgment only while the same
Ready job and current native custody still agree; it cannot revive a terminal,
replaced or faulted owner.

Continuation changes only the driver's local readiness predecessor. It does not
append an event, sample time, acquire source evidence, run helpers, refund rights,
recreate a permit or issue a human approval. Subsequent ordinary approval and
dispatch still acquire fresh evidence through their original paths. Earlier
human requests and approvals remain withdrawn; a new independent human review
must bind the new control context. Missing or changed source evidence still
prevents publication.

## Tower request and response

Install the original capability with `PublicationTower::set_narrow(Some(port))`
on a restricted control-operator mount. Operator role and Permit purpose remain
required. Equal numeric scope cannot install another owner's port. Actor,
legacy reconciliation, learned observer, imported-snapshot and branch surfaces
cannot use this route. A learned control mount can report an explicitly blocked
plan but cannot enter the ordinary narrowing mutation.

Select the registered `next` verb at the original Run address with an explicit
`narrow_authority` action. For example, substitute the actual route and target:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-target-restriction-9001",
  "address": "fa://1/run/3@41",
  "verb": "next",
  "arguments": {
    "action": "narrow_authority",
    "operation": "9001",
    "retained_targets": [{
      "adapter": "1",
      "object": "1",
      "contract_version": "1",
      "expected_version": "2",
      "generation": "1"
    }]
  },
  "idempotency_key": null
}
```

Tower inputs contain at most 64 unique targets, ordered by adapter, object,
contract version, expected version and generation. Their coordinates and
operation use canonical nonzero decimal strings. Unsorted input, duplicates,
unknown fields and noncanonical numeric spelling refuse before native work.
The trusted native ceiling retains its separate 1,024-target bound.

A known eligible plan supplies the complete `action` document through
`LinkedNarrowingNext::action()`. Its verb is `narrow`; arguments contain
`operation`, `expected` and `retained_targets`. `expected` binds the full native
effect scope, journal revision, control sequence, authority epoch and nullable
recorded time. The action binds the concrete original route generation and an
idempotency key equal to the operation's decimal spelling. Next may omit route
generation; mutation may not. Selecting a plan does not execute its action or
guarantee the predecessor will still be current when it is submitted.

The result kinds are `narrowing-next` and `narrowing`, with schemas
`fa.publication-narrowing-next/1` and `fa.publication-narrowing/1`. Both identify
`original_run_target_ceiling` scope and `live_narrowing_grant` retry custody.
They distinguish requested `retained_targets` from the actual
`effective_targets`. A plan carries `current_ceiling`, blocking reason and its
explicit action; cost, information gain, ranking and current eligibility remain
Unknown. A receipt carries the predecessor, previous ceiling, cancelled native
attempts and refunded units, with the acknowledged successor as its Knowledge
basis. Neither result discloses publication payloads or supplies effect keys.

The encoder checks the original sealed response against its request and requires
the exact requested/prior intersection, canonical attempt identities, a single
journal/control advance and unchanged epoch and recorded time. Relabelling a
native response cannot authorize another operation or turn the requested set
into a wider effective ceiling. Run-level journal Tail metadata identifies the
new event without inventing an attempt, external request or review round.

## Authenticated ordinary operator service

An ordinary Tower service can select this independent optional member of its
existing `fa.tower-operator-service/1` profile:

```json
"narrowing": {
  "queries": 32,
  "operations": 8,
  "max_replay_events": 4096,
  "authority": "narrow_original_targets"
}
```

Omission or null leaves narrowing disabled. The object is closed: duplicate or
unknown fields, noninteger allowances, out-of-bound values or a different
authority marker refuse before setup. A narrowing-only endpoint can disable
inspection and admission explanation. It does not obtain cancellation, Unknown
disposition, reconciliation or Fence merely by selecting narrowing.

Learned actor and resident service setup refuses this selection. The ordinary
profile has no numerical replay allowance; adding one does not enable learned
or predictor history. An otherwise supported operator endpoint over a mediated
owner still receives the native unsupported-history boundary.

The service installs the same-owner port on its restricted control mount and
retains one original grant across authenticated reconnects, review transitions
and ordinary schedule handoffs. None of those events renews queries, mutations
or exact-result custody. Retirement drops the grant. Existing kernel peer
authentication, transport bounds, source intake, checked publication, helper and
human review, independent Stop and cleanup continue to govern the workflow.

Use the existing client to send one explicit plan document:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE NARROW_PLAN_JSON
```

Inspect the plan, save its complete returned `action` as a separate request
document, and explicitly send that document with the same client. The client
does not follow a plan, reconnect, retry or construct a fresh predecessor by
itself. It validates the exact request/response, configured full audience,
target intersection, predecessor, epoch and receipt accounting before emitting
the response or sending a later selected document. A forged widening receipt
cannot be passed on as an acknowledgment.

Narrow before new intake when the next retained request should follow the
complete original helper/human publication workflow. If narrowing instead
excludes a current undispatched request, the existing driver retires that
cancelled job and the schedule may try its next independent request under the
remaining ceiling. An empty ceiling therefore denies later admission. A request
waiting for human approval can still be native Reviewing without a reservation;
cancelling it refunds zero.

For an already active retained driver job, the existing review/readiness
predecessor can become stale. This implementation does not refresh that job
automatically. Original containment remains available, while native retained
automatic authorization can publish only with fresh independent human approval
and all other current gates. Service evidence must not be read as automatic
continuation of every active retained workflow.

## Validation boundary

Fourteen native regression functions cover actual retained publication with a
fresh human key, excluded reservation refunds, charged Unknown conservation,
monotone and empty restrictions, canonical journal refusal, durable and live
retries, finite work, owner replacement and revocation, unsupported histories,
all five original storage failure barriers, and interrupted source acquisition.
The positive publication fixture retains the original automatic reservation;
it does not manufacture a replacement permit after narrowing.

Nine public Tower regression functions cover actual retained publication and
Unknown conservation, exact predecessor and scope checks, canonical bounded
input, finite custody and retry, sealed output tampering, imported snapshot and
branch refusal, and a real learned-history negative control that preserves
numerical, stream and admission state.

Twelve operator/client/service regression functions cover strict profile
selection, authenticated original-owner mutations, retained publication,
reconnect/exhaustion behavior, explicit client action and retry documents,
forged output refusal, and both plain and checked service workflows. The two
full-workflow functions each exercise both publication modes: restriction before
intake followed by actual helper/human publication, and active excluded review
retirement followed by admission refusal under the retained empty ceiling.

Implementation and regression sources are subject to the repository's required
remote gate. In this session, `RCH_REQUIRE_REMOTE=1 rch exec -- cargo
+nightly-2026-09-08 run --locked -p xtask -- check` returned exit 127 because
`rch` was unavailable. No Rust toolchain was available for formatting or syntax
checking either. Source inspection is not compilation, runtime-test, Clippy or
full-gate evidence. Existing packet and production qualification states remain
unchanged until revision-bound execution is available.
