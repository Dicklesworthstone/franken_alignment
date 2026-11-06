# Durable publication through the Tower

On Unix, `fa_reference::tower::publication::PublicationTower` connects the
registered `propose`, `get`, `precheck`, `capabilities` and separately granted
operator `situation` operations to an original
`FileActorPort<FileOversight>`. It uses the same `TowerRequest`, verb vocabulary
and sixteen-field response envelope as the [snapshot Tower](TOWER_REFERENCE.md).
The original file owner supplies durable intake, exact proposal binding,
request-local observations and recovery. This adapter owns no authority journal.

## Mount one actor session

The host creates or opens its original `FileOversight`, obtains its actor port
and supervisor, and mounts that port with `PublicationTower::new`. The mount is
an explicitly versioned `Run` address. Its tenant and run are the canonical
decimal values of the original native scope. A branch address or a different
tenant/run refuses installation. The generation identifies the host's route
registration; it is not a policy epoch, request generation or control sequence.

Each Tower instance belongs to one host-authenticated actor session. The host
must provision `TowerAccess` and must not accept grants from actor request bytes
or share the session's tickets with another principal. This Rust adapter checks
those grants; it does not implement transport authentication.

`exchange(access, document)` accepts the same operations through the closed
[`fa.tower_request/1` codec](TOWER_REQUEST.md). It validates the complete bounded
document before dispatch, so malformed input cannot consume an admission
snapshot or disclosure query. `dispatch` remains available for typed callers.

The supervisor remains outside the Tower. Only it supplies a fresh one-use
admission snapshot, performs original helper commitment/reveal review, obtains
the original automatic and human keys, and invokes dispatch and publication.
The Tower retains a weak actor port, so dropping the supervisor releases the
original authority lock and makes further observations explicitly unavailable.

## Acquire fresh evidence at native intake

For a source-backed live owner, use
`FileActorSupervisor<FileOversight>::exchange_tower_from_file` or
`FileSupervisedDriver::exchange_tower_from_file`. The host supplies the existing
Tower session, authenticated access, complete request document, registered
`EvidenceFile` and its original elapsed-clock function. The typed counterpart
is `dispatch_tower_from_file`. These calls bind the mounted port to the exact
supervisor allocation before proceeding; equal numeric scopes are insufficient.

Complete Tower decoding, caller grants, route generation, purpose, request key,
native proposal validation and ticket capacity all precede source acquisition.
Only a valid **new** submission enters the original
`prepare_wire_submission` / `prepare_file_intake` path. That path records the
original bounded source and clock observations and installs their one-use
admission snapshot immediately before the original durable submission. It does
not substitute caller declarations for source evidence or create another ledger.

Polls, precheck, capabilities, malformed or unauthorized documents, full ticket
sessions, and both exact and conflicting retries acquire no new source or time.
Retries still compare the entire proposal in the original durable owner. A
source acquisition failure preserves its original invalidation and accounting;
the actor receives only the native redacted unavailable/capacity result.

`FileTowerExchange` separates the actor-visible `PublicationResponse` from the
host-only optional `FileEvidenceReport`. The latter describes source preparation,
not successful request admission or execution. Preparation and submission remain
distinct journal transactions. A source failure, lost reply or encoding refusal
cannot roll back their committed records or refund work.

An ordinary host selecting one request uses
`exchange_tower_request_from_file`. A different proposal key is withheld before
source acquisition or consumption of any previously installed snapshot. During
review or recovery, `exchange_tower_request_observe` accepts no source or clock
and admits only exact retries of an already recorded selected request, while
retaining normal ticket-scoped reads. An unused snapshot cannot turn this
observation phase into new admission. Driver wrappers always perform the
original helper maintenance after the operation, including failures.

## Inspect exact policy before proposing

Precheck defaults to unavailable. The supervisor explicitly issues an original
`FileActorPrecheckPort` and retains its `FilePrecheckDisclosure` custodian. The
host installs that port with `set_precheck(Some(port))`. The adapter checks the
original owner allocation, not only numeric scope equality. A port from another
owner refuses even when its tenant, run and complete native Scope are identical.
Every replacement first withdraws the old installation, including an invalid
replacement. `set_precheck(None)` withdraws this facade's copy without revoking
other copies or changing the native grant's lifetime.

Use `Verb::Precheck` at the mounted run with
`ReadArguments::Precheck { query, proposal }`. The query label is nonzero; the
proposal retains the same exact fields used by real submission. The mounted
generation may be omitted for this read or must match exactly when supplied.
Subject scope is required, but an operator role, complete-domain grants and a
Permit purpose are not prerequisites for using an installed disclosure.

Malformed or unauthorized Tower requests refuse before entering the native
query allowance. Once entered, the original port checks current disclosure
custody, owner availability, exact revision and original source eligibility.
It evaluates only disclosed exact constraints: target/ceiling, deadline,
available resources and the actual retained policy predicates. Native denial
remains a Known precheck result, with common reason `DeniedByPolicy`; a pass
retains `precheck: true` and `review_required: true`.

The adapter has no precheck outcome cache. A supervisor-provisioned native
witness cache remains effective, including its original source invalidation.
Every entered query still consumes the same finite native allowance, even a
cache hit, repeated query label or new facade using a clone. An optional envelope
idempotency key neither deduplicates a precheck nor acquires a request ticket.

Precheck does not supply an admission snapshot, reserve rights, append a journal
event or call an endpoint. Actual submission still requires its original fresh
snapshot and review. A subsequent owner transition makes the old disclosure
ineligible; it cannot reuse an earlier pass. The host may install a newly issued
same-owner disclosure without discarding the session's existing request tickets.

## Discover the actual disclosed boundary

Use `Verb::Capabilities` at the mounted run with
`ReadArguments::Capabilities { query, expected_policy_epoch }`. Discovery requires
the same explicitly installed original precheck port. It neither grants that
port nor requests broader disclosure. The query must be nonzero; epoch zero is
valid syntax. Subject scope and an exact supplied route generation are checked
before native work. An operator role or Permit purpose is not required.

The adapter calls the original
`FileActorPrecheckPort::capabilities(query, expected_policy_epoch)`. A Known
result describes this granted exact-policy endpoint: whether its original
witness cache is enabled, the remaining shared query allowance **after this
read**, and the original endpoint's information and authority limitations.
It explicitly retains `informational: true`, `signed_profile: false`,
`effect_authority: false` and `review_required: true`. Policy contents, source
values and helper feedback remain withheld; source refresh remains unavailable.

Discovery, exact evaluation, cloned ports, repeated labels and newly constructed
facades share the same finite allowance and custodian lifetime. The returned
count is an observation, not a reservation for a later read. An admitted query
with a stale expected policy epoch spends work and returns the native Stale
result. Revoked custody remains Withheld; owner borrow, changed owner revision,
ineligible source and owner loss preserve their original unavailable outcomes.
No previously Known discovery is cached or reused after these changes.

Discovery does not inspect an admission snapshot, evaluate a policy, populate
the witness cache, read source files, observe a new clock or write a journal
event. Ordinary proposal admission, independent review and both publication
keys remain separate. The native
[discovery contract](ACTOR_PRECHECK_DISCOVERY.md) defines the exact response.
This grant-local observation does not implement the full signed deployment
assurance profile or `doctor` contract.

## Inspect the original live owner's situation

The host can separately grant a privileged read capability with
`FileActorSupervisor<FileOversight>::inspection_port(queries)`, receiving a
`FileSupervisorInspectionPort` and its `FileSupervisorInspectionGrant` custodian.
The allowance must be between one and 1,024 queries. The host retains custody
and installs the port with `PublicationTower::set_inspection(Some(port))`.
Installation requires the exact original owner allocation. Every replacement
withdraws the old installation first, including a rejected foreign port.

`situation` uses the mounted run with no arguments. The request still requires
tenant, ControlLedger/Verify authority and matching optional registration
generation. It additionally requires the host-authenticated operator role.
Neither ordinary actor intake nor a precheck disclosure grants that role or
inspection port. Actor requests cannot mint, install or widen the capability.
The [ordinary live actor service](TOWER_PEER.md) keeps operator access disabled.

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-1",
  "address": "fa://1/run/3@41",
  "verb": "situation",
  "arguments": null,
  "idempotency_key": null
}
```

Each permitted query captures a fresh original `FileOversight::situation()`.
It returns the original `FileSituationBasis`, whole-capture accounting summary,
and at most the first 16 native attempt rows. Native ordering places unresolved
outcomes before pending work and terminal outcomes. The summary retains counts
and balances for **all** attempts, even when the selected rows are incomplete.
`enumeration_complete: false` explicitly reports remaining rows. No live offset
or caller-supplied continuation can be applied to a different capture; further
pages remain available through the original supervisor situation API.

The original native basis retains scope, journal revision, control sequence,
recorded elapsed tick, origin and any prefix anchor. This port's origin is the
healthy owner's acknowledged state. It does not read a potentially newer disk
image or claim current source eligibility. The body retains
`fa.publication-situation/1`, `historical: true` and
`current_eligibility: "not_observed"`. The common envelope's global coordinates
remain Unknown; no mount generation, elapsed tick or native revision is coerced
into another coordinate. Sealed capture data rejects a reconstructed response
whose public basis has been substituted.

Inspection reads no source or clock, consumes no admission snapshot, writes no
journal event, acquires no request ticket and provides no publication key.
Native `Unknown` and `IrrecoverablyUnknown` attempts remain unresolved and
charged according to the original ledger. The read does not infer nonexecution,
refund liabilities or fabricate a `why-held` explanation from action stages.

Cloned ports and newly constructed facades share the same finite allowance.
Each entered capture spends one query, including repeated operation labels or
envelope keys. An output or fallible capture failure does not refund it. Missing
custody returns native Withheld; lost, mutably borrowed or faulted owners return
native Unknown before capture. Neither unavailability nor exhausted work reuses
a previously Known report. Dropping or consuming the noncloneable custodian
revokes every weak port clone; reconnecting or reinstalling a clone cannot renew
it. Neither the grant nor the port retains the authority lock after the original
supervisor closes. A reopened owner requires its own separately issued grant.

The result profile is `PublicationResult::Situation(FileInspectionResult)`.
The same native result passes through the shared router and bounded peer without
a response cache. `tower-exchange` can carry it for an independently provisioned
operator connection while preserving its original observational meaning.

## Submit an actual proposal

Use the mounted run address with its exact registration generation, `Verb::Propose`,
and `ReadArguments::Propose { request, proposal }`. The nonzero native request
identity must match `idempotency_key` as a canonical decimal string. Arbitrary
string keys are not hashed into native identities. The proposal contains the
exact target, payload, units, deadline and expected policy epoch.

The caller needs the subject's tenant/plane/authority grants and an explicitly
allowed `Permit` purpose. This is permission to use the already provisioned
actor intake; it is not a `PermitCx`, effect permit or supervisory role.
Operator status and global-domain visibility are not required for an actor to
submit its own proposal. Original native checks still decide whether admission
is possible under the supplied policy epoch and current one-use snapshot.

The adapter transfers the owned proposal directly to a validated typed
`ActorWire::execute` path. Byte parsing, byte encoding and typed execution share
the same command validation. The existing bounded session ticket map and native
durable owner remain the only request owners. No payload-to-hex-to-JSON round
trip, duplicate proposal ledger or cached outcome is introduced.

A successful submission can return native `Pending` or `NotAdmitted`; it does
not report a completed effect. The normal supervisor workflow can subsequently
produce `Denied`, `CancelledBeforeDispatch`, `Executed`,
`ConfirmedNotExecuted` or `OutcomeUnknown` through the original owner.

## Observe and recover the original request

`request_address(request)` returns a namespaced `Attempt` address for the actor's
intake-request projection, including the original run/principal/branch/authority.
It is deliberately distinct from a native ledger attempt number. `get` at that
address polls a ticket acquired by this session. Knowing or guessing a numeric
request identity does not acquire another session's ticket.

Each admitted `get` and each exact `propose` retry polls the original owner.
Pending results can therefore change to explicit uncertainty or a terminal
outcome. The adapter never caches a response that would conceal that change.

Reusing the same native key with the same complete proposal reacquires its
original ticket without another durable request or execution. Changing its
proposal conflicts even if the new proposal would otherwise be admissible.
`operation_id` is envelope correlation metadata, not a second effect identity.

After the owner is reopened through its original recovery path, a fresh Tower
session has no old tickets. An exact submit retry reacquires only the original
request's observation. It does not require a fresh admission snapshot for an
already retained request, resurrect old publication keys or resubmit an effect.
Native recovery and liability accounting remain authoritative.

## Preserve the native epistemic boundary

`PublicationResponse` specializes the common `TowerResponse` with
`PublicationResult` and `PublicationLimits`. Its structured `publication-request`
body contains the original actor `WireResponse`, including every native
`Knowledge` state and the original request-local `ActorBasis`. Common admission
refusals use the shared `Knowledge` encoder. There is no synthetic outer basis
claiming that a native request label is a control cut.

The `precheck` body embeds the original `PrecheckWireResponse`. Its disclosure
ID, query label, policy epoch and observed elapsed tick remain in the original
`FilePrecheckBasis`; they never become a global generation or control sequence.
Revoked disclosure stays Withheld, unavailable/stale source eligibility stays
Unknown, and a mismatched policy epoch stays a native Stale observation.

The `capabilities` body similarly embeds the unchanged original
`PrecheckCapabilitiesResponse`, including its own schema and informational
marker. It retains `FilePrecheckBasis` and the original remaining-query count;
neither becomes a global Tower budget or control sequence. An inconsistent
publicly reconstructed response fails its original encoder's correlation checks.

The envelope keeps all sixteen common fields. Global generation, control
sequence, source frontiers, budgets, realized cost and uncomputed affordances
remain `Unknown`, or `Withheld` on an unauthorized request. Neither a native
request generation nor a mount generation is substituted into those fields.
The address retains the explicit mount generation; a stale route points to the
current mounted address in `would_change_this`.

Native errors remain present in the structured body while the common envelope
maps them to registered reason codes. An unavailable controller is distinct
from an unknown external outcome. Exact keyed proposals are retry-safe in the
native idempotency sense; an unknown outcome is never converted to a refund or
permission to create a new key.

## Bounds and remaining scope

The native session retains at most 128 tickets. Each native precheck disclosure
allows at most 128 shared evaluation/discovery queries, potentially fewer when
issued by the supervisor. Only a successful explicitly granted discovery exposes
the remaining native count. Native proposals retain their
existing target, payload, unit and deadline checks, and the durable owner retains
its own request/byte limits. Envelope operation IDs and keys are bounded to
128 bytes and addresses to 256 bytes. Responses have a configurable 16 KiB to
1 MiB bound, with 16 KiB by default. A post-mutation encoding refusal preserves
the exact retry key and never implies that durable admission was rolled back.

Supervisor inspection has its own maximum of 1,024 shared capture queries and
16 returned attempt rows. The host chooses the actual grant allowance, which
is distinct from native precheck work, request tickets and global Tower budgets.

The Linux [live Tower peer and client](TOWER_PEER.md) now integrate this profile
into the existing ordinary `supervise_publication` service. Fresh source
admission, peer credentials, selected-request scheduling, independent Stop and
the original review/publication path remain native. This profile does not expose
supervisor mutations, arbitrary affordance execution, a planner's decision card,
learned generation, branch rehearsal or independent receipt verification.
Unsupported verbs refuse before touching the native owner.
The complete dispatcher and deployment qualification contracts remain open.

Earlier publication qualification is recorded in the
[publication receipt](../artifacts/execution/2026-11-06-tower-publication-receipt.json)
and subsequent [precheck receipt](../artifacts/execution/2026-11-06-tower-precheck-receipt.json).
Publication's library and selected test targets passed type checking before the
precheck extension. Runtime execution and the final extension's compiler check
remain blocked by the recorded shared-memory limit; those earlier checks do not
qualify the later source.

The capabilities increment adds eight real-source regression functions for
permitted discovery, shared allowances, unchanged source/journal state,
generation/scope/refusal admission, owner identity/lifetime, revocation and exact
native encoding. It updates the exhaustive router test to recognize the newly
connected verb. Independent source review and qualified nightly syntax/format
checks passed. RCH is unavailable in the current environment, so compilation,
Clippy, test execution and the canonical gate remain pending for this revision.
