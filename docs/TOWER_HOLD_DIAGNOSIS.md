# Diagnose recorded publication conditions through Tower

The native publication Tower connects the registered `why-held` verb to a
separately granted read of the original durable owner. It answers which recorded
conditions stand in the way of dispatch for one exact external request at one
acknowledged cut. It does not replay an earlier admission explanation or infer
fresh permission from a recorded approval.

This is a scoped implementation of FA-137 under plan sections 17.3, 17.8 and 25.1.
The full dispatch predicate tree, evidence-address graph, causal explanation of
historical terminal decisions and ranked `next` affordances remain open.

## Issue and retain the original native capability

The trusted host already holds the original actor gateway and supervisor:

```rust
let (diagnostics, custodian) = supervisor.hold_diagnosis_port(32)?;
tower.set_hold_diagnosis(Some(diagnostics))?;
```

Retain `custodian` for the disclosure lifetime. The allowance is between one
and 1,024 queries; clones share it. Dropping or consuming the custodian revokes
every port. Ports retain only weak ownership and cannot keep an authority lock
alive, adopt a reopened owner, replenish their allowance, obtain approval keys
or provide a mutable host.

An installation must belong to the same owner allocation as the mounted actor
port. Matching numeric scope is insufficient. Every replacement first withdraws
the previous installation, including a rejected foreign replacement. It does
not revoke or charge another original grant.

The caller also needs the operator role and authorized access to the registered
control-ledger request address. Actor submission, precheck, situation inspection,
admission explanation and completed-review disclosure remain separate grants.

## Select the original external request

Use the address issued by `PublicationTower::request_address`, or the
original request link returned by an opted-in native situation page. An allocated
ledger attempt number is a different identity. For tenant 1, principal 2, run 3,
branch 4, authority 5, external request 701 and registration generation 41:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "diagnose-request-701",
  "address": "fa://1/attempt/request-3-2-4-5-701@41",
  "verb": "why-held",
  "arguments": null,
  "idempotency_key": null
}
```

Malformed documents, caller/grant boundaries and stale registrations are checked
before native lookup. An omitted address generation is permitted for an explicit
read; the response still names the original nonzero mount. A request number
cannot reconstruct an actor ticket or select another owner.

Each entered read resolves the retained request-to-attempt mapping and inspects
the same owner under one immutable borrow. No partially enumerated situation,
separate historical capture, canonical-file fallback or numerical replay is
consulted. Reads do not refresh source evidence, observe a new clock, run a
helper, consume the actor's one-use admission snapshot or append journal events.

A missing request or a request rejected before attempt admission returns
Unknown in this profile. It does not fabricate an attempt or claim closed-domain
absence. An admitted policy denial remains an actual Denied stage.

## Read the ten conditions with their basis

The sealed result preserves the native scope, external request, request-local
generation, actual attempt, journal revision, authority control sequence and
recorded clock observation. Request generation zero is valid. These coordinates
are distinct and must not be substituted for one another.

| Condition | Observation at the acknowledged cut |
|---|---|
| `recorded_admission_open` | No Stop is recorded |
| `recorded_not_suspended` | The control state is not suspended |
| `recorded_policy_epoch_matches` | The action's original epoch equals the recorded current policy epoch |
| `recorded_deadline_open` | The last recorded clock is strictly before the action deadline; equality is expired. Unknown if no clock observation exists |
| `recorded_stage_authorized` | The actual recorded stage is Authorized |
| `dispatch_not_recorded` | The recorded lifecycle has not admitted dispatch; this does not prove external nonexecution |
| `current_source_evidence` | Unknown |
| `current_approvals` | Unknown |
| `current_clock` | Unknown |
| `final_publication_eligibility` | Unknown |

Every known boolean carries the identical native basis. A false recorded
condition identifies a recorded obstacle, not the historical cause of a terminal
decision. Proposed, Reviewing, Denied and Cancelled requests are never presented
as Authorized. Confirmed, Dispatching, Unknown, ConfirmedNotExecuted and
IrrecoverablyUnknown retain their actual lifecycle semantics.

The four live conditions stay Unknown even when every recorded condition is
true. Their `controller_unavailable` reason means that this read does not
obtain the necessary controller observation; it does not imply that the original
owner has died. Actual publication must still satisfy its original independent
source, clock, approval, rights, target, credential and endpoint checks.

Unknown and explicitly abandoned effects retain their original charge. This
diagnosis cannot resend them, refund them, clear Stop or advance a stream.

## Wire and work boundaries

The result kind is `hold-diagnosis`, with schema
`fa.publication-hold-diagnosis/1`. The existing sixteen-field Tower envelope
is unchanged. The result declares:

- `scope: "recorded_publication_state"` and the exact external request and mount;
- `historical: true`;
- `blocker_completeness: "recorded_conditions_only"`;
- `current_eligibility: "not_observed"`;
- no live evidence or clock read, payload disclosure or effect authority.

All native integer identities are canonical decimal strings. The sealed result
binds the original query address, registration and request even for Unknown,
Withheld and error results; replacing public outer envelope fields cannot
relabel another report. Known predicate bases must match the captured report.

Every entered lookup spends once, including missing requests, unknown results
and repeated requests through a clone or another authenticated connection.
There is no diagnosis cache or refund for encoding failure. A later read
captures the then-acknowledged state instead of returning an earlier answer.
A malformed zero request, revoked grant, lost owner, conflicting mutable borrow
or storage fault does not enter lookup. Exhausted healthy grants return Capacity.

The existing response limit applies to the whole report. A live native route
remains reserved after refusal or owner loss; the shared router never falls back
to imported snapshot evidence.

## Observe a learned stream's original publication

The Rust `PublicationTower::new_learned_observer` facade accepts this same
separately issued grant for its original learned owner. Its operator-only
`why-held` reads the actual source-derived request and publication machine.
It preserves numerical generation, source snapshots, review keys and journal
bytes; ordinary payload submission, ticket polling and mutations remain outside
that observer's closed verb set. The four live conditions remain Unknown.

This route needs no numerical replay. Historical admission and completed-review
explanation grants retain their original replay limits and may still return
Unknown for learned histories. The ordinary authenticated command line service
below retains its existing ordinary-owner profile.

## Use the authenticated ordinary operator endpoint

The existing `--operator-profile` supports the optional explicit selection:

```json
"hold_diagnosis": {
  "queries": 32,
  "disclosure": "recorded_publication_state"
}
```

Omission or null preserves existing profiles. The selected service provisions
the same original grant and retains its noncopyable custodian across reconnects,
scheduled handoffs and terminal reply grace. A diagnosis-only endpoint needs no
inspection, policy, congress or experiment disclosure. Send the exact request
above with `supervise_publication tower-operator-exchange operator.json why-held.json`.

Before output or any later explicit request, the client validates the closed
profile, original request/mount/scope, distinct native coordinates, all ten
ordered conditions and every Known predicate basis. It also checks consistency
of recorded stage, epoch and deadline predicates and refuses any replacement of
the four Unknown live conditions with invented permission. Malformed or unbound
responses terminate without automatic retry or another request. Existing byte,
depth, item, I/O and elapsed limits are unchanged.

See [operator setup and examples](LIVE_TOWER_OPERATOR.md#diagnose-an-original-requests-recorded-publication-conditions).

## Validation and remaining qualification

Native unit and public integration regression sources exercise real original
requests, congress and independent human approval, actual publication, clock
and policy changes, Stop, revocation, source/snapshot conservation, storage and
owner failures, charged Unknown abandonment and wire correlation.

Four operator profile/service and three authenticated-client regression functions
add actual socket coverage for explicit selection, preserved admission, live
acknowledged advancement, shared reconnect budgets and corrupted-reply stopping.
One additional learned-observer regression follows actual numerical generation,
source-derived Message intake, original two-key publication and reconciliation
through Reviewing, Authorized and Confirmed, including revocation and owner loss.

These changes received independent source review. The execution environment
became unavailable before implementation. No Rust compilation, runtime test,
doctest, formatter, Clippy or RCH gate pass is claimed for this increment.
Source hashes identify the submitted code only. No Bead, full packet,
production activation or complete eligibility contract is closed.
