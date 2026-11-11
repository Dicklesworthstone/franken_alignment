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

## Validation boundary

Fourteen native regression functions cover actual retained publication with a
fresh human key, excluded reservation refunds, charged Unknown conservation,
monotone and empty restrictions, canonical journal refusal, durable and live
retries, finite work, owner replacement and revocation, unsupported histories,
all five original storage failure barriers, and interrupted source acquisition.
The positive publication fixture retains the original automatic reservation;
it does not manufacture a replacement permit after narrowing.

Implementation and regression sources are subject to the repository's required
remote gate. In this session, `RCH_REQUIRE_REMOTE=1 rch exec -- cargo
+nightly-2026-09-08 run --locked -p xtask -- check` returned exit 127 because
`rch` was unavailable. No Rust toolchain was available for formatting or syntax
checking either. Source inspection is not compilation, runtime-test, Clippy or
full-gate evidence. Existing packet and production qualification states remain
unchanged until revision-bound execution is available.
