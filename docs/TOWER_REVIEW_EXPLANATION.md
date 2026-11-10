# Explain an original completed congress through Tower

The separately granted completed-review profile connects the original native
publication owner's congress history to the existing `explain` verb. It serves
an operator investigating a held or published request: which exact policy and
committee applied, what each member actually revealed, which reducer rules
selected the consequence, whether that consequence applied, and what the
original publication journal subsequently observed.

This implements a scoped part of FA-137, supporting FA-136, under plan §§9.2–9.4, 9.8, 17.3,
17.8 and 22.1, serving FI-A06, FI-A13 and FI-A14. It uses the original replay
machine and effect audit; there is no second evaluator, congress, ledger or
publication path. It does not claim a complete current `why-held` tree.

## Native custody and selection

An original `FileActorSupervisor<FileOversight>` issues
`review_explanation_port_with_budget(queries, max_replay_events)`. The host
retains the noncloneable `FileSupervisorReviewExplanationGrant` and installs its
port with `PublicationTower::set_review_explanation`. Queries range from 1
through 1,024; each complete acknowledged history must fit the explicitly
selected positive event bound, at most `MAX_REVIEW_EXPLANATION_EVENTS`.

This grant is independent of actor intake, precheck, situation inspection,
admission explanation and candidate policy replay. The port retains weak owner
and grant references. A foreign owner cannot be installed even when all numeric
scope fields match; a rejected replacement withdraws the old installation.
Revocation/drop suppresses future queries through every clone. Owner loss,
active mutable borrowing or storage failure cannot expose a previously captured
answer. There is no read cache or refund after encoding failure.

`explain_review(request, round)` first resolves the external request through its
original durable disposition. It then uses the original completed-review cursor
to validate the complete acknowledged suffix once, including the selected
review's actual application and selected-attempt effect audit. The native action,
attempt and round must all bind to the request; equality of numeric request and
attempt IDs is insufficient. Missing/unadmitted requests, unfinished rounds,
unsupported numerical histories, insufficient replay work and failed
reconstruction remain `Unknown`, never proof of absence.

Every entered valid query spends one shared allowance, including repeated and
unknown requests. Malformed zero identities, invalid Tower syntax, unauthorized
callers and stale mount generations refuse before native expenditure. Reads do
not refresh source evidence, sample time, consume admission snapshots, acquire
either publication key or write journal events.

## Native completed refinement explanations

The same original port also exposes
`explain_review_completion(request, round)`. A Known result explicitly contains
`FileCompletedReviewExplanation::Applied` or `::Refined`. The existing
`explain_review` method retains its application-only type and behavior, including
Unknown when its selected round completed an evidence refinement.

The completion selector enables the existing native cursor's refinement mode
before the first event and finishes one complete replay with its original effect
audit. It uses the same query custody and numerical reservations; it performs no
second replay, archive copy, helper judgment or replacement reduction.

`FileRefinementExplanation` is a sealed shared projection of that original audit.
It exposes the policy evaluation, frozen congress and ballots, tally, decision,
three historical cuts, states and actual effect observations. Its `group()` names
the exact retained K/V residual acquired by the refinement.
`reviewed_input_revision()` and the common `input_revision()` identify the input
the congress judged; `refined_input_revision()` identifies the resulting input.
The latter requires a fresh review at the refinement cut. A later review may
already have happened, so this is not a current eligibility judgment.

The refinement projection has no `application()` method, permit conversion,
whole-archive accessor or payload/helper-input disclosure through Debug. A later
Applied review and independent human approval keep their own effect lineage;
they never make the selected refinement an authorizing review. The Tower round
selector uses this completion method and returns the distinct refinement
response below only for an original Known refinement. Unknown, Withheld and
capacity results retain the existing review response because the completion
kind has not been observed.

## Explicit numerical history allowance

An original supervisor can select
`review_explanation_port_with_numerical_budget(queries, max_replay_events,
numerical_replay_events)`. This enables reconstruction of its retained decoder
or action-predictor history, including learned text and raw-frame predictors.
The existing constructors keep numerical reconstruction disabled.

The additional allowance is cumulative across every clone and query. It must be
positive and at most `queries * max_replay_events`, with checked multiplication.
`remaining_numerical_replay_events()` returns `None` when it was not selected,
or the original remaining reservation count, including `Some(0)` after spending.

One unit reserves one original journal transition. Before advancing the private
replay cursor, the implementation reserves the entire frozen acknowledged cut,
including nonnumerical records: bootstrap, congress and later transitions can
also invoke retained prediction, refinement or checkpoint work. Each transition
still uses its immutable native numerical limits. These units do not measure
arithmetic operations, CPU time, memory or wall time.

An insufficient reservation consumes the entered query but executes no cursor
transitions and leaves the numerical allowance untouched. An admitted replay
spends its complete reservation even if reconstruction, final request/round
attribution or output later fails. Missing request/round selection before replay
also spends the entered query without a numerical reservation. A later query
must pay for the then-acknowledged complete cut; no earlier answer is a free cache.

The replay uses the original owner's retained bindings and journal, with no
caller-supplied recipe, file reopening, source refresh or live-budget transfer.
It recomputes historical numerical transitions while preserving the original
committee's commitments and verdicts; it does not ask helpers for fresh judgments.
The sealed historical response and its current-eligibility Unknowns are unchanged.

Raw-frame action predictors also perform numerical work. Completed-review reads
require this explicit selection for those histories as well. Original admission
explanations have their own independent
`explanation_port_with_numerical_budget` selection, described in the
[admission contract](TOWER_PUBLICATION.md#explain-the-original-exact-policy-admission).
A completed-review grant never funds admission reconstruction, and the default
admission constructors still return charged Unknown for decoder or
action-predictor histories. Plain histories retain their existing known
explanations without spending a selected numerical allowance.

## Explicit request

Select a nonzero original round at the facade's external **request address**:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "explain-review-701-801",
  "address": "fa://1/attempt/request-3-2-4-5-701@41",
  "verb": "explain",
  "arguments": {"round": "801"},
  "idempotency_key": null
}
```

The exact round is an explicit historical selector, not a capability or an
offset into a replacement archive. The field is a canonical unsigned decimal
string; missing, duplicate, extra, zero, fractional, signed, numeric and
overflowing values refuse. `arguments: null` retains the original admission
explanation and its existing wire schema. An imported `ReferenceTower` refuses
this native selector before spending its unrelated logical read allowance.

## The historical Applied response

The unchanged sixteen-field envelope carries result kind `review-explanation`
and schema `fa.publication-review-explanation/1`. The projection binds its
address, registration and selected round, including unknown/refused results.
The public response cannot be relabeled to another request or registration by
rebuilding outer fields.

Known results contain three distinct native cuts: the original pre-vote anchor,
the completed application, and the end of the acknowledged journal. Native
control sequences and recorded ticks stay in that basis; no synthetic global
Tower clock or current eligibility is constructed.

The body includes:

- The original policy graph and exact evaluation trace, with witness references
  and count. Policy literals are part of the explicit disclosure; retained
  source witness values and action payload bytes are omitted.
- The complete frozen congress roster, cohorts, declared and admitted weights,
  influence caps, thresholds, accepted commit/reveal times and original verdicts.
  Missing members and explicit abstentions stay separate, visible outcomes.
- The original capped tally, consequence, authority floor and reducer rule
  trace. A helper `Deny` remains an empirical recommendation rather than being
  promoted to an exact policy disqualifier.
- The actual application: an applied control sequence/input revision/consequence
  or the original refusal code. A replayable `Continue` that failed with `Stale`
  remains a refused application.
- Historical effect observations from the same replay: stage after review,
  stage at the ending cut, dispatched automatic-review identity and consumed
  human approval metadata, terminal endpoint outcome and reconciliation. A later
  review of the same attempt is identified as a different dispatch origin.

An unobserved dispatch/outcome/reconciliation is `Unknown{not_observed}`. It is
never a receipt for nonexecution. Effect claims apply to the original reference
publication sink; they do not authenticate an arbitrary remote service. Hidden
helper input, source snapshots, action payload and reveal salts are not exposed.
The wire says that helpers were not rerun and live evidence was not read.

The original durable replay checks consistency under its existing host, clock,
source and reference commitment assumptions. This projection is not an
independent cryptographic verification, proof that helpers were honest, or proof
that a chosen policy was sound. No returned data can mint a permit.

## The historical refinement response

The same explicit round selector can identify an original completed evidence
refinement. Its unchanged sixteen-field envelope then carries result kind
`refinement-explanation`, schema `fa.publication-refinement-explanation/1` and
scope `historical_completed_refinement`. It remains a historical read under the
same completed-review disclosure and numerical allowances; no new verb or
operator authority is introduced.

The sealed Known body preserves the original policy, evaluation, frozen
congress, verdicts, tally, decision, timing, evidence root and effect audit. Its
three native cuts refer to the pre-vote anchor, the completed refinement and the
end of the acknowledged journal. The additional `refinement` member identifies
the exact original residual acquisition. This is a body excerpt:

```json
{
  "application": {"state": "not_attempted"},
  "input_revision": "1",
  "refinement": {
    "kind": "kv_exact_residual",
    "group": {"layer": "1", "side": "value", "position": "3", "head": "0"},
    "reviewed_input_revision": "1",
    "refined_input_revision": "2",
    "requires_fresh_review": true
  }
}
```

All coordinates and revisions use canonical unsigned decimal strings. The
common `input_revision` equals `reviewed_input_revision`; the refined revision
must be greater. `application` is exactly `not_attempted`, rather than an
invented refusal or successful application. The fresh-review requirement
describes the selected refinement cut. A later review may already have approved
the new input, so current eligibility and the current hold reason stay Unknown.

Unobserved effects remain Unknown. If the acknowledged suffix includes a later
dispatch, its `selected_review` flag is false: the automatic authorization must
name a different original round and an input revision at least as new as the
refinement. Consumed human approval and endpoint observations retain their own
original lineage. Re-encoding or rebuilding outer response fields cannot move
the sealed refinement to another request, scope or registration.

The authenticated client accepts the new kind only for an explicit completed
round selector. It checks the existing historical/disclosure flags and native
basis, the closed refinement shape, exact `not_attempted` application, increasing
input revisions, canonical group coordinates and independent dispatch lineage
before output or another explicit request. The original Applied schema and
Unknown/Withheld/error responses remain compatible. These checks do not rerun a
reducer or independently authenticate the original congress.

## Use the authenticated operator endpoint

In the existing `fa.tower-operator-service/1` profile, explicitly select:

```json
"review_explanation": {
  "queries": 8,
  "max_replay_events": 4096,
  "disclosure": "policy_and_congress_trace"
}
```

This is an optional object within the full [operator profile](LIVE_TOWER_OPERATOR.md),
not a standalone connection profile. Absence or null keeps the capability
disabled. It has its own finite grant even when admission explanation, situation
inspection and policy replay share the same authenticated endpoint. A service
may select review explanation alone. Its original custodian survives reconnects
and scheduled request handoffs without replenishing work.

For an original decoder or action-predictor history, explicitly add the optional
positive integer `numerical_replay_events` inside this object. For example:

```json
"review_explanation": {
  "queries": 8,
  "max_replay_events": 4096,
  "numerical_replay_events": 8192,
  "disclosure": "policy_and_congress_trace"
}
```

This selects at most 8,192 cumulative original journal transitions, with each
complete cut still limited to 4,096. It does not promise eight successful reads:
each admitted numerical read reserves its full cut, including ordinary events.
The total must not exceed `queries * max_replay_events`. Omission retains the
default numerical Unknown; explicit null, zero, nonintegers, unknown or duplicate
fields refuse during profile admission.

Start an ordinary Tower service or a [live learned actor/resident sequence](LIVE_LEARNED_OPERATOR.md)
with its existing `--operator-profile` selection and send the explicit request above through:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE REVIEW_REQUEST_JSON
```

The client checks the connected supervisor's kernel credentials before any wire
exchange. It verifies the closed response schema, historical/disclosure flags,
requested round in every result, and the known basis's native request/scope/cuts
before output. A mismatched response stops before any later request or output;
there is no automatic retry or reconnect. These are correlation and bounded
shape checks, not independent verification of the congress decision.

Both selected completed-review kinds have their own finite 262,144-item parser
allowance within the original 1 MiB frame ceiling, depth, string and cumulative
I/O/deadline limits. Ordinary admission explanation keeps its previous 4,096-item
parser limit. A large policy/committee requires an explicitly sufficient server
`response_bytes` allowance; increasing that allowance grants no disclosure.

## Bounded delivery and qualification

The selected `PublicationLimits.max_response_bytes` remains the complete-output
bound: 16 KiB by default and at most 1 MiB. Member/trace arrays are encoded with
incremental bounds. An oversized answer becomes an explicit budget refusal; it
does not truncate the roster, erase a dissent, retry replay or repay work.
Cloned native results share one immutable original replay. Wire projection
indexes the accepted member events once without recomputing the reducer.

The public integration targets are `tower_review_explanation` and
`tower_learned_observation`; native custody and storage-failure tests accompany
the port. Exact executed validation and remaining
repository qualification are recorded in `IMPLEMENTATION_STATUS.md`. No Bead,
full roadmap packet or production profile is closed by this increment.
