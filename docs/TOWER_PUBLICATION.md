# Durable publication through the Tower

On Unix, `fa_reference::tower::publication::PublicationTower` connects fourteen
registered operations to an original `FileActorPort<FileOversight>`: `propose`,
`get`, `precheck`, `capabilities`, and separately granted operator `situation`,
historical admission or separately granted completed-review `explain`, candidate `policy-replay`,
separately granted recorded-state `why-held`, and bounded original receipt
planning/reconciliation, independently granted undispatched-request cancellation
or explicit Unknown disposition through `next` and `resolve`, independently
granted ordinary target-ceiling restriction through `next` and `narrow`, separately granted
original journal `tail`, request-to-review `related`, and original-domain `fence` through its restricted
control mount.
It uses the same `TowerRequest`, verb vocabulary
and sixteen-field response envelope as the [snapshot Tower](TOWER_REFERENCE.md).
The original file owner supplies durable intake, exact proposal binding,
request-local observations and recovery. This adapter owns no authority journal.

The [native journal-tail contract](TOWER_JOURNAL_TAIL.md) supplies coherent
historical metadata pages from the same live owner. An explicit original
journal revision selects a fixed capture; finite supervisor disclosure and
single-use continuations bound enumeration without replaying numerical work.

The [request-history contract](TOWER_REQUEST_HISTORY.md) connects an original
request directly to all of its completed reviews at a bounded acknowledged
cut. Explicit Explain links preserve the separate replay/disclosure grant and
distinguish a learned refinement from an attempted consequence application.

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

The [policy-replay contract](TOWER_POLICY_REPLAY.md) describes the native
route: complete current-policy corpus comparison, distinct disclosure custody,
exact campaign retries and a sealed response registration. It changes no policy
or publication authority.

## Plan and reconcile an original outcome

The separately granted [native reconciliation contract](TOWER_RESOLUTION.md)
connects an exact original request's `next` plan to an executable `resolve`
request. The dedicated `new_reconciliation_operator` constructor limits its
Permit purpose to receipt reconciliation and independently selected observation
or comparison routes. It cannot submit or cancel actor requests.

A retained original receipt can settle an uncertain dispatch. A miss remains
charged Unknown; the operation never publishes, resends, seals or observes a
caller-provided clock. Full predecessor checks and finite per-grant retry custody
apply. Costs, information gain and publication eligibility remain Unknown.
Numerical history requires a different replay contract, and the learned observer
below keeps its existing closed read-only profile.

## Cancel one original undispatched request

The independent [scoped cancellation contract](SCOPED_REQUEST_CANCELLATION.md)
connects the same existing `next` and `resolve` verbs using the explicit
`action: "cancel_undispatched"` argument. It requires its own original
`FileSupervisorCancellationPort`, installed through `set_cancellation`,
and a restricted control operator with Permit purpose. The legacy
reconciliation-only mount does not acquire this authority.

Next offers an exact full-predecessor request for Proposed, Prepared, Reviewing
or Authorized work under finite read, operation and replay allowances. Cancel
invokes the original native transition, releases only the selected undispatched
reservation and retains its original dependent-buffer cleanup. Dispatching,
Unknown and terminal work refuse. This does not Fence the domain or prevent
another original request from publishing.

Learned control requires explicit cumulative numerical replay authority,
including for raw-frame predictor histories. Grant custody, exact retries and
spent work survive different facades on the same original owner. New response
kinds `cancellation-next` and `cancellation` carry separate closed schemas;
omitting the action discriminator retains the original reconciliation format.
An action remains data until explicitly selected.

## Restrict the original ordinary target ceiling

The independent [ordinary narrowing contract](SCOPED_AUTHORITY_NARROWING.md)
connects `next` with explicit `action: "narrow_authority"` to the registered
`narrow` verb. Install the original `FileSupervisorNarrowPort` through
`set_narrow`; only a restricted control-operator mount with Permit purpose can
use it. Actor, legacy reconciliation, learned observer, imported-snapshot and
branch routes do not acquire this capability.

The operator selects at most 64 canonical complete target identities at the
original Run address. A known plan reports both the requested set and its
actual intersection with the current ceiling. The returned Narrow document
binds the concrete route generation, operation identity and full native
predecessor. It remains data until explicitly submitted. Plans reserve no
mutation or admission snapshot; costs, information gain, ranking and current
publication eligibility remain Unknown.

Narrow advances the original control sequence once, cancels only excluded
undispatched attempts and refunds only their actual reservations. It preserves
retained automatic authorizations, authority epoch and dispatched/Unknown
charges. Human keys retain their predecessor checks and require fresh approval.
An empty target set retains nothing; a wider later selection cannot restore
earlier exclusions. The ordinary operation refuses decoder, predictor, mediated
and every stream owner, without borrowing their separate replay authority.

Sealed `narrowing-next` and `narrowing` results correlate the exact requested,
prior and effective sets and the original run cut. They retain the original
finite grant and exact retry rules; a facade creates no new native budget.

## Observe an original learned stream owner

`PublicationTower::new_learned_observer(mount, learned_port, limits)` mounts the
original `FileLearnedTextStreamActorPort` for privileged observation. The
supervisor separately issues and retains the same native inspection, recorded
hold-diagnosis, explanation or policy-comparison grants used by the ordinary
facade. A grant from another
owner is rejected even when every numeric scope field matches.

This constructor has a closed, read-only dispatch profile: an authenticated
operator may request `situation`, recorded-state `why-held`, historical `explain`,
or `policy-replay` under
their independently selected native grants. Ordinary `propose`, request `get`,
`precheck`, `capabilities` and every other verb are unavailable through this
profile, including when a host supplies Permit or Experiment tags. Caller
payloads cannot become learned release bytes. These checks precede the shared
source-admission callback, ticket acquisition and native query work.

Live situation observations come from the actual learned owner's acknowledged
journal. They can show an empty ledger before intake, the source-derived Message
awaiting review, and the original confirmed release with its charged units.
They do not refresh evidence, advance numerical generation, consume an admission
snapshot, write a journal event or approve a release. Captured pages retain their
original cut when subsequent generation or publication advances the owner.

The separately granted [recorded-state `why-held`](TOWER_HOLD_DIAGNOSIS.md)
reads the exact original learned request and its acknowledged publication
conditions. It reports actual Reviewing, Authorized and Confirmed stages without
advancing numerical generation or publication. Source freshness, current
approvals, current time and final eligibility remain Unknown. This observation
requires no numerical replay. The separately authenticated
[live learned operator](LIVE_LEARNED_OPERATOR.md) uses the same grant for single
releases and resident sequences without acquiring publication control.

The constructor does not expand any numerical-replay contract. Admission and
completed-review explanation grants without numerical work allowances continue
to return charged Unknown for learned histories. An operator must not interpret
that response as a missing congress, a current hold explanation or permission.
Explicit numerical grants now enable original admission and completed
Applied/Refined congress explanations through the same live owner. The existing
separately budgeted offline learned audit retains its independent history-anchor
contract.

All port clones and facades share their original finite grants. Revocation
removes disclosure, owner loss makes reads unavailable, and the weak observer
does not keep the authority lock alive. Reopening the same journal creates a
different owner and requires its own explicitly issued grants. This connects the
existing L4 situation projection and L8 observation contracts; it adds no new
verb, authority type or execution guarantee.

## Operate an original native generated-text stream

Native monitored generation uses the original `FileGeneratedTextActorPort` and
its immutable native-text-only bootstrap contract. Three explicit constructors
connect that owner to the same separately granted operator operations:

| Constructor | Available profile |
|---|---|
| `new_generated_observer` | Original inspection, journal tail, request history, admission/completed-review explanation, hold diagnosis and policy comparison |
| `new_generated_reconciliation_operator` | The same reads and explicitly granted original receipt reconciliation through `next` / `resolve` |
| `new_generated_control_operator` | The same operations plus separately granted undispatched cancellation, Unknown abandonment and admission fencing |

Each takes the original generated-text port, an exact versioned Run address and
`PublicationLimits`. The constructor itself grants no disclosure, replay work
or mutation. The host must independently issue, install and retain the original
native grants; all clones and facades share their finite work and exact retries.
Numerical history still requires explicit numerical replay allowances for the
operations that replay it. Ordinary target-ceiling narrowing remains unsupported
for this stream contract.

All three mounts require operator access. They cannot accept ordinary caller
payloads or generated-source Submit documents, obtain actor tickets, run
`precheck`, advance generation or issue either publication key. Permit and
Experiment purpose tags do not expand their closed vocabulary. The internal
weak-port conversion stays crate-private, so actors cannot obtain an ordinary
arbitrary-payload gateway from a generated-text port. Observations contain their
original metadata and epistemic status, never an unapproved text release.

Mounting and reading do not consume the actor's admission snapshot, change
numerical state or write journal records. A grant from another owner is refused,
including an owner with identical numeric scope. Losing the original supervisor
or revoking a grant makes the corresponding access unavailable; the facade
cannot keep the original authority lock alive or adopt a reopened owner.

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

Polls, precheck, capabilities, policy replay, malformed or unauthorized
documents, full ticket sessions, and both exact and conflicting retries acquire
no new source or time.
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

The authenticated peer also supports `drive_tower_sequence_from_file` and
`drive_tower_sequence_observe` for an explicit schedule of at most 64 distinct
keys. Only the current new key can acquire evidence; completed predecessors must
be terminal in the original journal before socket I/O. Tickets and all finite
session allowances survive the handoff. The [live Tower service](TOWER_PEER.md#publish-an-explicit-request-sequence)
connects this ingress to the original sequential review/publication lifecycle.

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

An initial `situation` uses the mounted run with no arguments. The request requires
tenant, ControlLedger/Verify authority and matching optional registration
generation. It additionally requires the host-authenticated operator role.
Neither ordinary actor intake nor a precheck disclosure grants that role or
inspection port. Actor requests cannot mint, install or widen the capability.
The [ordinary live actor service](TOWER_PEER.md) keeps operator access disabled
on its actor connection. An explicit
[operator sidecar](LIVE_TOWER_OPERATOR.md) provisions a separate authenticated
session with its own native read custody and no Permit purpose.

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

The initial query captures a fresh original `FileOversight::situation()`. It
returns the original `FileSituationBasis`, whole-capture accounting summary,
and at most the first 16 native attempt rows. Native ordering places unresolved
outcomes before pending work and terminal outcomes. The summary retains counts
and balances for **all** attempts, even when the selected rows are incomplete.
`enumeration_complete: false` explicitly reports remaining rows.

When more rows and query allowance remain, `knowledge.value.continuation` in
the native result body contains an opaque nonzero decimal string. Copy that
value into the next request's sole argument. For example, if the returned value
is `"17"`:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-2",
  "address": "fa://1/run/3@41",
  "verb": "situation",
  "arguments": {"cursor": "17"},
  "idempotency_key": null
}
```

The typed equivalent is `ReadArguments::SituationPage { cursor: 17 }`.
The facade matches this transport value to its retained original
`FileInspectionCursor`; a scalar cannot construct a native cursor or grant
inspection. Each continued page uses the same immutable native capture, exact
basis, ordering and whole-capture summary even if the healthy live owner has
advanced. It never applies an offset to a new live report. Direct native callers
can use `FileSupervisorInspectionPort::situation_page(Some(cursor))` with the
cursor returned by `FileInspectionPage::continuation()`.

Each accepted continuation consumes its page identity and returns the next one
when possible. Read the returned value after every page. A fresh `arguments:
null` query replaces the shared capture and invalidates earlier progress, also
when issued through another clone of the same port. Foreign, replaced, consumed
or otherwise unissued cursors fail without spending a query or displacing a
different valid continuation. Cursor IDs are ephemeral within this
process/service lifetime; they are not persisted resume credentials and must
not be reused after a service restart.

The final available query returns no continuation and releases the capture,
even when more rows exist. Always inspect `enumeration_complete`: a null
continuation alone does not prove complete enumeration. An encodable response
is required before the facade retains its next cursor; an output refusal does
not refund the query.

The continuation field is an additive Tower extension to the existing native
result body; the standalone situation CLI keeps its existing output shape.

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

Cloned ports and newly constructed facades share the same finite allowance and
at most one retained native capture. Each entered capture or accepted page
spends one query, including repeated operation labels or envelope keys. An
output or fallible native failure does not refund it. Missing custody returns
native Withheld; lost, mutably borrowed or faulted owners return native Unknown
and discard retained capture data. Every page checks the original owner again.
Neither unavailability nor exhausted work reuses a previously Known report.
Dropping or consuming the noncloneable custodian revokes every weak port clone;
reconnecting or reinstalling a clone cannot renew it. Neither the grant, the port
nor a cursor retains the authority lock after the original supervisor closes.
A reopened owner requires its own separately issued grant.

The result profile is `PublicationResult::Situation(FileInspectionResult)`.
The same native result passes through the shared router and bounded peer without
a response cache. `tower-exchange` can carry it for an independently provisioned
operator connection while preserving its original observational meaning.

## Follow captured attempts to original requests

Use `ReadArguments::SituationLinks { cursor: None }`, or JSON arguments
`{"request_links":true}`, to receive `fa.publication-situation/2`. Continue the
same capture with `{"request_links":true,"cursor":"N"}`. The default null and
cursor-only requests retain the exact eight-field v1 row shape and field order.
The common envelope and outer result kind `situation` are unchanged.

Each v2 attempt retains its original numeric `address` and adds:

```json
{
  "external_request": "7001",
  "request_address": "fa://1/attempt/request-3-2-4-5-7001@41"
}
```

These two fields are both null for a direct native attempt without an admitted
external request. The original request book supplies the mapping, captured once
alongside the ledger and copied into immutable rows. External keys can differ
from attempt IDs and retain all 64 bits. Refused allocations never become links
to a later direct attempt that happens to reuse the allocation. Ambiguous,
rebound or orphaned admitted mappings fail closed. The reverse projection is
bounded by the original request-book limit and built once per capture; subsequent
pages do not scan a newer live request book.

The request address contains the original run, principal, branch, authority and
external key, plus the mounted registration generation. An original query may
omit its generation; the returned links still name the registered generation.
`PublicationResult::SituationLinks(LinkedSituation)` seals both that registration
and the original query address, so later edits to public envelope fields cannot
rebind the result. `request_address_for(scope, registration, key)` exposes only
the validated canonical spelling for consumers; it proves neither existence nor
authority.

An operator can use a non-null link in a separately authorized `explain` query
to inspect the original exact-policy admission. Inspection and link possession
do not confer explanation disclosure, an actor request ticket, Permit authority
or either publication key. `get` retains its original ticket requirements. Links
describe the captured request identity, not current eligibility, later holds or
fresh source evidence. Native cursor, revocation, owner health, query and output
limits apply equally to both projections, with no refund on encoding refusal.

Thirteen authored native, Tower integration and codec regression functions cover
original mapping and reopen, full-width/nonmatching keys, direct and refused
allocations, immutable pagination, separate explanation custody, generationless
queries, envelope rebinding, strict opt-in syntax and bounded output. An existing
reference-Tower test now also checks no-charge refusal of this native profile.
Independent source review and syntax/format checks do not replace the pending
remote compilation and runtime gate.

## Explain the original exact-policy admission

A distinct `FileSupervisorExplanationPort` discloses the original policy and its
actual admission trace. It is issued only by the original supervisor through
`explanation_port(queries)`, `explanation_port_with_budget(queries,
max_replay_events)` or the explicit numerical selection described below.
Retain the noncloneable `FileSupervisorExplanationGrant`
custodian and install its weak port with `PublicationTower::set_explanation`.
Existing actor, precheck and situation-inspection grants do not confer this
stronger disclosure. Replacement withdraws the previous installation even when
a foreign owner's replacement is rejected.

Use `explain` with no arguments at the **request address** derived by the facade.
The independent [completed-congress profile](TOWER_REVIEW_EXPLANATION.md) instead
uses `{"round":"801"}` and a separately installed review disclosure grant;
admission explanation never implicitly enables that stronger disclosure.

For the original admission:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "explain-admission-701",
  "address": "fa://1/attempt/request-3-2-4-5-701@41",
  "verb": "explain",
  "arguments": null,
  "idempotency_key": null
}
```

Common tenant/plane/Verify checks, the host-authenticated operator role, exact
optional registration generation, canonical request identity and empty
arguments precede disclosure work. The original request book resolves the
external key to its admitted attempt. An equal numeric attempt ID is never a
substitute, and this privileged read need not acquire an actor ticket.

The original investigation reconstructs the complete acknowledged journal in
memory. It returns the original admission policy, every predicate's original
result and witness index, its semantic and policy coordinates, and separate
admission and ending states. Both journal revisions, control sequences and
recorded ticks retain their native meanings. A later cancellation, policy
replacement, clock observation or completed publication cannot rewrite the
original admission trace.

The response is `PublicationResult::Explanation(FileExplanationResult)` with
native schema `fa.publication-explanation/1`. Its scope is explicitly
`historical_exact_policy_admission`; current eligibility is not observed and
current hold reason remains Unknown. This explains an exact-policy denial even
when no congress occurred. A satisfied original admission does not explain a
later hold or establish permission to publish. The complete dispatch `why-held` conjunction and ranked `next` still need
their own complete current predicates and are not inferred from these stages.

The canonical body encodes the original predicate graph and retains full-width
identities as decimal strings. It discloses policy literals, target and payload
length, but does not serialize the action payload, helper inputs, a complete
source snapshot or observed witness values. Witness references remain explicit
alongside `witness_values_disclosed: false`. The privileged Rust result retains
the policy's selected witnesses. The encoder checks the sealed native basis
and matching request subject before serializing a Known report. Global Tower
coordinates, costs and affordances remain Unknown.

A grant allows 1–1,024 shared queries and a maximum of 1–4,096 retained events
per query. The complete history must fit; no prefix explanation is returned.
The existing constructors leave numerical replay unselected and return charged
Unknown for decoder and action-predictor histories, including raw-frame
predictors. The original supervisor can instead explicitly select
`explanation_port_with_numerical_budget(queries, max_replay_events,
numerical_replay_events)`. Its positive cumulative allowance must not exceed
the checked product of the query count and per-query event ceiling.

For a numerical admission, the port reserves the entire acknowledged event
count before entering the original eager investigator. One unit reserves one
original journal transition, including nonnumerical transitions; numerical
events retain their original immutable recipe limits. This is not a CPU,
arithmetic-product, memory or elapsed-time measurement. The same immutable owner
borrow pins the cut, and replay uses retained bindings without reopening the
journal or reading a live source. The resulting explanation includes genuine
exact-policy denial even when no congress was opened.

Missing or unadmitted requests, unreconstructable histories and exceeded replay
bounds remain Unknown, never proved absence. Missing/unadmitted requests and
insufficient event or numerical allowances spend the entered query without
reserving or executing numerical replay. Once admitted, the complete reservation
remains spent after replay, attribution, unwind or output failure. Plain
histories do not consume a selected numerical allowance. Clones, reconnects and
repeated operation/idempotency labels share the same finite counters; the
custodian exposes `remaining_numerical_replay_events()` as None for unselected
work and Some(0) for an exhausted selection. This allowance is independent of
completed-review explanation, reconciliation and live numerical execution.

Reads acquire no source or clock, consume no admission snapshot, write no
journal event, run no counterfactual and issue no review or publication key.
Revoked custody returns Withheld; unavailable, borrowed or faulted owners
return Unknown without cached-result or disk fallback. A reopened owner needs
its own separately issued capability. The ordinary actor service still grants
no operator role or explanation port. An independently provisioned operator
connection can carry this profile through the same bounded peer and client.

## Diagnose the original request's recorded publication conditions

A separate `hold_diagnosis_port` and `set_hold_diagnosis` installation connect
operator `why-held` at the exact native request address. One immutable owner
borrow supplies the current acknowledged lifecycle, Stop/suspension, policy
epoch and deadline relative to recorded time. The ten typed conditions retain
one basis; fresh source, approvals, clock and final publication eligibility
remain Unknown. Query work and revocation are independent of inspection and
historical explanation. See [the native diagnosis contract](TOWER_HOLD_DIAGNOSIS.md).

## Compare a candidate with the original retained policy corpus

`policy-replay` uses the mounted run with an explicit exact registration
generation and `ReadArguments::PolicyReplay { campaign,
expected_policy_generation, candidate }`. The nonzero campaign is also the
canonical envelope idempotency key. The caller needs the host-authenticated
operator role and `Experiment` purpose in addition to subject access.

The original supervisor separately issues
`policy_replay_port(queries, ReplayLimits { cases, input_bytes })` and retains
its noncloneable `FileSupervisorPolicyReplayGrant`. The host installs the weak
port with `set_policy_replay(Some(port))`; foreign owner replacement clears the
previous installation before refusing. Existing actor, precheck, situation and
explanation capabilities do not grant this comparison.

The original native engine evaluates all retained proposals and applied reviews
bound to the current policy, including exact denials. A caller cannot choose a
favorable subset. Missing candidate witnesses remain `RequiresShadow`; an empty
or unreconstructable corpus is Unknown. A newly reviewable case inherits no
helper approval, and the original policy, journal, source, clock, request tickets
and publication keys remain unchanged.

Every admitted campaign retains its complete expected generation, candidate and
original result. Exact retries across port clones and facades preserve the first
owner cut without another charge, even after later journal changes. Conflicting
contents refuse. Custody and original owner health are checked before cache
access; a reopened owner needs a new grant.

The result is `PublicationResult::PolicyReplay(LinkedPolicyReplay)`. Its private
fields seal the native result to the exact mount registration and campaign.
`native()`, `registration()` and `campaign()` expose observations only. Encoding
rejects attempts to rebind the envelope's subject, generation or key, including
native error results. The `fa.publication-policy-replay/1` wire body discloses
policy literals and complete case traces, with explicit historical/rehearsal
markers; it omits action payloads and actual witness values and grants no effect
or promotion authority.

See [TOWER_POLICY_REPLAY.md](TOWER_POLICY_REPLAY.md) for the full API, JSON policy
syntax, resource accounting, epistemic outcomes and tested end-to-end examples.

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

The `policy-replay` body preserves its sealed original policy comparison and
native journal revision, control sequence, policy generation/epoch and recorded
tick. Its `remaining_campaigns_at_capture` is historical grant-local accounting.
Neither the candidate generation nor a cached result becomes a current global
Tower observation or permission to change policy.

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

Supervisor inspection has its own maximum of 1,024 shared capture/page queries,
one retained native capture per grant and 16 returned attempt rows per page.
The host chooses the actual grant allowance, which is distinct from native
precheck work, request tickets and global Tower budgets.

Policy replay has a separate allowance of 1–64 admitted campaigns and a shared
8 MiB logical request/result cache. Each complete corpus must fit the host's
1–5,120 case limit and logical input limit of one byte through 32 MiB. A response
limit refuses the whole encoding without refunding native work; an exact retry
through a larger same-owner facade can encode the retained complete result.
The original grant and owner must still be available.

The Linux [live Tower peer and client](TOWER_PEER.md) now integrate this profile
into the existing ordinary `supervise_publication` service. Fresh source
admission, peer credentials, selected-request scheduling, independent Stop and
the original review/publication path remain native. Supervisor mutations are
limited to independently selected original-domain Fence, original-receipt
reconciliation and undispatched-request cancellation. Arbitrary affordance
execution, a planner's decision card, learned generation, branch rehearsal and
independent receipt verification remain outside this profile.
Unsupported verbs refuse before touching the native owner.
The complete dispatcher and deployment qualification contracts remain open.

Earlier publication qualification is recorded in the
[publication receipt](../artifacts/execution/2026-11-06-tower-publication-receipt.json)
and subsequent [precheck receipt](../artifacts/execution/2026-11-06-tower-precheck-receipt.json).
Publication's library and selected test targets passed type checking before the
precheck extension. Runtime execution and that extension's compiler check were
blocked by the recorded shared-memory limit; those earlier checks do not qualify
the later source.

The capabilities increment adds eight real-source regression functions for
permitted discovery, shared allowances, unchanged source/journal state,
generation/scope/refusal admission, owner identity/lifetime, revocation and exact
native encoding. It updates the exhaustive router test to recognize the newly
connected verb. Independent source review and qualified nightly syntax/format
checks passed. That increment recorded RCH as unavailable and left compilation,
Clippy, test execution and the canonical gate pending.

For the later policy-replay extension, the qualified `nightly-2026-09-08`
compiler with Cranelift executed all **12 public policy-replay integration
tests** successfully on 2026-11-09. They cover actual complete-corpus evaluation,
unchanged authority followed by real publication, exact retries, custody,
uncertainty, strict encoding and routing. This scoped local result does not
qualify the full repository, remote deployment or remaining dispatcher verbs;
the [policy-replay verification section](TOWER_POLICY_REPLAY.md#executed-coverage-and-remaining-qualification)
records its scope and corrected fixture attempt.
