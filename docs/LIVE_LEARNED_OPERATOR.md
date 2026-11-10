# Observe a live learned publication through the original Tower owner

The Linux learned publication commands can serve a separately authenticated
operator endpoint while the original owner performs numerical generation,
source-derived actor admission, native or process helper review, independent
human review and publication. A resident sequence retains that same endpoint
across separately approved Messages and an optional final Finish.

The endpoint uses the existing `fa.tower_request/1` requests and sixteen-field
response envelope. Its native `situation` response comes from the actual
acknowledged owner and preserves the original ledger, liabilities, request links,
capture basis and pagination rules. The observer creates no additional
supervisor, journal, source lease or effect path.

## Select the live endpoint

For one live learned actor, select the actor profile and both operator options:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE \
  --actor-profile ACTOR_PROFILE \
  --operator-profile OPERATOR_PROFILE --tower-generation 41
```

The same selection composes with the existing live continuation modes
`--continue-generation`, `--after-message PREVIOUS_REQUEST` and
`--finish-after-message PREVIOUS_REQUEST`. Each retains its original numerical
recipe, source freshness, recovery requirements and two independent approval
keys. Optional actor precheck retains its separate disclosure and socket.

A resident sequence already selects the actors in its manifest:

```text
supervise_publication serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --operator-profile OPERATOR_PROFILE --tower-generation 41
```

This composes with the resident command's explicit live recovery and
retained-history options. The operator pair can appear among the other supported
options. The complete grammar is validated before an option is removed: missing
partners, duplicate flags, zero or malformed generations, conflicting
continuations and incompatible offline modes are rejected before startup.

Historical `--resume`, receipt inspection, review audits, Stop/recovery commands,
policy promotion/replay and checkpoint/reset maintenance do not start this live
endpoint. Their existing independent commands retain their own capabilities.

## Choose a public operator profile

Use the existing closed `fa.tower-operator-service/1` schema described in
[the live operator contract](LIVE_TOWER_OPERATOR.md#public-operator-profile).
For live learned inspection, select positive finite `inspection_queries`, set
admission explanation fields to zero/`"none"`, and omit the optional historical
review and policy-comparison grants unless deliberately needed. Replace the
following identities and paths with the actual deployment's values:

```json
{
  "schema": "fa.tower-operator-service/1",
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "socket": "/run/fa/operator.sock",
  "supervisor": {"uid": 1000, "gid": 1000, "pid": null},
  "operator": {"uid": 1001, "gid": 1000, "pid": null},
  "candidate_limit": 8,
  "connection_limit": 4,
  "exchange_limit": 64,
  "response_bytes": 16384,
  "runtime_ms": 60000,
  "poll_ms": 5,
  "inspection_queries": 32,
  "explanation_queries": 0,
  "explanation_replay_events": 0,
  "explanation_disclosure": "none"
}
```

The ordinary operator's optional `resolution` mutation grant is not accepted
by learned observation. Omit that member or set it to `null`. Selecting it,
including in a programmatically supplied profile, fails before clock/source
acquisition, authority-store creation or listener setup. The learned facade
retains its closed read-only verb set and cannot execute a `next` reconciliation
action or `resolve` request.

The complete scope and selected supervisor identity must match every selected
actor. Operator identity remains independently configured. Actual Linux UID/GID
and an optional PID are authenticated before reading protocol bytes. These are
kernel process identities, not executable identities or human authentication.
Naming the same identity for multiple roles gives those processes the
corresponding explicitly selected access.

The socket directory must be supervisor-owned and not group/other writable.
Admission compares the canonical path with the authority store subtree, selected
policy source, actor/reviewer/Stop/precheck sockets, retained history output and
topology checkpoint output. Checking the source pathname does not read evidence
or consume source work. A resident sequence checks every scheduled endpoint
before the first clock sample, store or listener is created. Existing socket
paths are never unlinked to make startup succeed.

## Read the original live situation

The existing clients need only the public profile and explicit requests:

```text
supervise_publication tower-operator-exchange OPERATOR_PROFILE SITUATION_JSON
supervise_publication tower-situation OPERATOR_PROFILE fa://1/run/3@41 --max-pages 64
```

For the example scope and registration:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-learned-run",
  "address": "fa://1/run/3@41",
  "verb": "situation",
  "arguments": null,
  "idempotency_key": null
}
```

An initial read can show zero admitted requests while generation or actor intake
is pending. After source-derived submission it can show the original reviewing
attempt, and after checked two-key publication it can show the confirmed
execution and charged units. These stages are observations of the recorded
ledger. They do not supply current eligibility, complete hold reasons, helper
verdicts, source values, model activations or an authorization decision.

The existing explicit request-link and continuation profiles remain available.
Each page retains its original capture even if the live owner advances. A new
unpaged situation obtains a fresh capture and spends another read. Responses
preserve native registration and journal coordinates; a Tower generation is
never substituted for a control sequence or policy epoch.

## Diagnose recorded publication conditions

An explicit optional profile selection also exposes the original learned request's
recorded publication conditions:

```json
"hold_diagnosis": {
  "queries": 16,
  "disclosure": "recorded_publication_state"
}
```

Send an explicit `why-held` request for the original request address through
`tower-operator-exchange`, as described in
[the authenticated diagnosis contract](LIVE_TOWER_OPERATOR.md#diagnose-an-original-requests-recorded-publication-conditions).
The report preserves the acknowledged publication stage and native predicate
bases. Current evidence freshness, approvals, time and final eligibility remain
Unknown. The read does not run generation, replay a numerical history, consume
an admission snapshot or acquire a publication key.

This grant has its own finite query allowance. The common service retains the
same custodian across resident handoffs and reconnects; inspection exhaustion
does not replenish diagnosis work or the reverse. Omission or `null` keeps this
disclosure disabled. Diagnosis-only profiles retain the existing closed schema
and set the other unselected read allowances to zero.

## Explain the original learned congress

The same endpoint can reconstruct a completed learned congress when the public
operator profile explicitly selects both the historical disclosure and its
independent numerical allowance:

```json
"review_explanation": {
  "queries": 8,
  "max_replay_events": 4096,
  "numerical_replay_events": 8192,
  "disclosure": "policy_and_congress_trace"
}
```

Send an `explain` request with an explicit original round at the original request
address through `tower-operator-exchange`; the full request and response contract
is in [completed-review explanations](TOWER_REVIEW_EXPLANATION.md#explicit-request).
The unchanged client checks the selected round, request, scope, registration and
native historical cuts before emitting the closed response.

The allowance counts cumulative original journal transitions under their
retained model limits. Each numerical read reserves its entire acknowledged cut
before replay begins, so 8,192 units do not necessarily fund all eight queries.
Omitting the field preserves charged Unknown for numerical history; an
insufficient positive allowance also returns Unknown without advancing the
cursor. An admitted failure retains its reservation. Null, zero, malformed and
excessive totals fail before service startup.

The original grant is installed by the common live service and survives both
reconnects and resident Message/Finish handoffs. Historical reconstruction
observes the original policy, committee verdicts, reducer application and later
effect linkage. It supplies neither current eligibility nor the still-required
independent human publication key.

## Preserve cumulative work and independent control

One service owns the socket, transport counters and native read custodians for
the selected lifetime. Reconnects and resident handoffs do not reissue grants,
reset query/transport allowances, refresh pending captures, extend the original
deadline or replenish numerical/source work. Exhausted reads return their
original refusal while independent reviewed publication can continue. Revocation
withdraws weak grants; an observer cannot keep the authority lock alive after
the actual owner exits.

Independent Stop has priority before each operator turn. The workflow services
bounded operator work during generation, actor waiting, native/process review,
human review, publication and the actor's existing terminal reply grace. A
partial frame cannot block Stop. Truncated or oversized frames, transport failure,
expiry or exhausted connection/exchange limits retire only this observer. They do not retry
an effect, approve a request, stop a healthy publication or suppress the original
actor response.

The learned observer facade permanently restricts dispatch to operator
observations. Arbitrary `propose` payloads, request `get` tickets, `precheck`,
`capabilities` and other unsupported verbs remain unavailable even with broader
host purpose tags. The separate learned actor port remains the selected intake
path and continues to derive release bytes from its original numerical owner.

## Historical and qualification boundaries

Explicit historical explanation and policy-comparison selections keep their
native work contracts. Completed-review explanations require the separate
numerical allowance above for decoder or action-predictor histories. Admission
explanation has no numerical grant and remains charged Unknown for those
histories. Repeated Unknown queries still spend their original query allowance.
Refinement-specific reports and independent retained-history anchoring remain
available through the existing learned audit command.

This integrates the registered situation and supervision capabilities toward
FA-134/FA-136 and the learned publication workflow. Recorded-state `why-held`
retains its defined predicate scope; complete current eligibility and ranked
`next` remain unavailable. Detector quality, physical containment and production
qualification are not established. Executed checks and remaining full-gate limits are
recorded in [implementation status](../IMPLEMENTATION_STATUS.md).
