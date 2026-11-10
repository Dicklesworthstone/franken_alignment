# Live operator inspection and control through the original Tower owner

On Linux, ordinary Tower publication services can expose a
separate authenticated operator endpoint. It connects the existing native
`situation`, historical admission or completed-congress `explain`, recorded
`why-held` diagnosis and explicitly selected `policy-replay` capabilities to the
running supervisor. A separately selected `resolution` grant connects native
`next` and `resolve` to original endpoint receipt reconciliation. A separate
`fence` selection permanently stops admission in that original owner. Policy replay
compares a candidate against the original owner's retained policy corpus
without changing policy or issuing authority. The service keeps its original journal lock, source
admission, helper/human review, publication keys, independent Stop and cleanup path.

## Select the operator endpoint

Using an already qualified executable:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --operator-profile OPERATOR_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --operator-profile OPERATOR_PROFILE --tower-generation 41
```

All four ordinary Tower serve modes support the option for one request or an
explicit finite request schedule. The generation, operator and selected
schedule/precheck pairs may appear in any order after the positional files.
The two create modes can select the existing topology profile alongside the
operator endpoint, including a request schedule. Both open modes can compose
explicit guarded mediated recovery with an exact v2 source checkpoint, a request
schedule and the operator endpoint. An operator profile requires an explicit
Tower generation.
Live learned actor and resident sequence commands also accept the explicitly
paired operator/generation selection through their own closed grammar; see
[live learned observation](LIVE_LEARNED_OPERATOR.md). Their facade preserves
source-derived intake and the original numerical-replay limits. Multi-peer and
ordinary native-text service modes do not consume it. Unsupported combinations
fail before reading configuration.

An optional `--precheck-profile PRECHECK_PROFILE` retains its separate actor
disclosure and socket. The operator, actor, precheck, human-review and Stop
paths must be distinct. Directory ownership, the selected audience and socket
collisions are checked before a clock read, source acquisition or store creation.
Existing socket paths are never unlinked to make setup succeed.

Creation with `--topology-profile TOPOLOGY_PROFILE` keeps the original mediated
bootstrap and final topology refresh, including plain checked publication.
Operator reads cannot certify the cut or replace either publication key. A
version 2 topology checkpoint destination must not alias the operator socket;
this is checked before clock, source or persistent setup. Topology schedules
retain cumulative source work and recheck each new dispatch. Precheck remains a
separate incompatible admission profile. See the
[Tower topology contract](TOWER_PEER.md#preserve-topology-enforcement-through-tower-admission).

Explicit `--guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE`
on the Tower open modes preserves the original owner and source-work custody
before installing operator grants. It supports fresh checked publication and
recorded observations without letting operator traffic refresh source evidence
or start a new request. See [live mediated recovery](LIVE_TOWER_RECOVERY.md) for
exact paired custody, independent Stop and failure-retention boundaries.

## Observe a finite publication sequence

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --requests 7,8 --operator-profile OPERATOR_PROFILE --tower-generation 41
```

The original bounded schedule retains one operator session and its native grant
custodians for the entire owner lifetime. Selecting the next request does not
reissue inspection, explanation or policy-replay work, clear a pending continuation, reset
transport allowances or extend elapsed deadlines. The operator can inspect an
earlier confirmed request while a later request waits for its independent human
approval. A continued situation still reads its original immutable capture;
an explicit fresh situation captures the advanced owner. A repeated policy
campaign retains its original report and cut, while a new campaign compares the
advanced owner's retained corpus within the same remaining replay allowance.

Preflight checks the operator listener against **every scheduled** human-review
and Stop path before the first clock read or store/socket creation. A late key
cannot collide with a listener that was valid only for the initial request.
All original source, review, publication, settlement and confirmed helper-reaping
conditions still govern handoff. Operator reads cannot advance the schedule.

Read limits spent on an earlier request remain spent during later requests,
reconnects and final reply grace. A fully recorded schedule can expose its
historical observations without source files or helper programs, using the
original recovery path. Request schedules still cannot select the single-request
precheck sidecar; it has a different admission-time disclosure lifetime.

## Public operator profile

The profile fixes the peer identity and finite selected capability allowances. It contains no
private supervisor configuration, source path, helper recipe or publication key.
For example, replace the scope, paths and kernel identities with the actual
deployment's values:

```json
{
  "schema": "fa.tower-operator-service/1",
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "socket": "/run/fa/operator.sock",
  "supervisor": {"uid": 1000, "gid": 1000, "pid": null},
  "operator": {"uid": 1001, "gid": 1000, "pid": null},
  "candidate_limit": 8,
  "connection_limit": 4,
  "exchange_limit": 1024,
  "response_bytes": 1048576,
  "runtime_ms": 60000,
  "poll_ms": 5,
  "inspection_queries": 64,
  "explanation_queries": 8,
  "explanation_replay_events": 4096,
  "explanation_disclosure": "policy_and_selected_witnesses",
  "hold_diagnosis": {
    "queries": 32,
    "disclosure": "recorded_publication_state"
  },
  "review_explanation": {
    "queries": 8,
    "max_replay_events": 4096,
    "disclosure": "policy_and_congress_trace"
  },
  "policy_replay": {
    "queries": 2,
    "page_queries": 512,
    "cases": 5120,
    "input_bytes": 1048576,
    "disclosure": "policy_and_case_traces"
  }
}
```

The schema is closed: missing required fields, duplicate fields and unknown
fields fail. `hold_diagnosis`, `policy_replay`, `review_explanation` and
`resolution` and `fence` are independently optional; omitting one or setting it to `null` keeps that capability
disabled, so existing inspection/explanation profiles retain their behavior. Profile integers are unsigned JSON integers, as in the existing actor
profiles. Tower request integers retain their separate canonical decimal-string
format. The optional numeric `response_bytes` selects the complete response
allowance for this operator endpoint. Omitting it preserves the original
16,384-byte default; an explicit selection must be an integer from 16,384 through
1,048,576, using the native `PublicationLimits` bounds. Null, strings, fractions,
duplicates and unknown fields refuse. Validation happens before grants or a
socket are created. A larger output allowance adds no disclosure or purpose
grant and does not enable an otherwise disabled operation.

Inspection and explanation have separate allowances, each bounded by the
original native maximum of 1,024 queries. At least one of inspection, admission
explanation, recorded-state diagnosis, completed-review explanation, policy
replay, receipt reconciliation or original admission fencing must be enabled. For inspection only, set
`explanation_queries` and `explanation_replay_events` to
zero and `explanation_disclosure` to `"none"`. Enabling explanation explicitly
selects policy-literal and selected-witness disclosure, with a positive bounded
replay-event allowance. It does not disclose the entire source, helper input
or action payload.

Numerical admission reconstruction requires its own optional top-level
`explanation_numerical_replay_events` integer. It must be positive, admission
explanation must be enabled, and the total must not exceed the checked product
of `explanation_queries * explanation_replay_events`. Omission preserves the
default numerical Unknown; explicit null, zero, malformed or excessive values
refuse before startup. For example, two queries with a 4,096-event ceiling can
select `"explanation_numerical_replay_events": 8192`.

The shared service provisions this exact original admission grant. Every entered
numerical reconstruction reserves the entire acknowledged cut before replay,
under the retained immutable numerical limits. A short allowance spends the
query without replay, and admitted work is not refunded on failure or output
refusal. The same custody survives reconnects and resident handoffs; retirement
reports its remaining reservations. Plain histories do not spend this numerical
allowance. Completed-review and receipt-reconciliation work have independent
grants and cannot fund admission reconstruction.

The independent `hold_diagnosis` object requires exactly `queries` (1–1,024)
and `disclosure: "recorded_publication_state"`. Unknown fields, duplicate
fields, zero/excessive bounds, noninteger values and other disclosure markers
refuse before setup. A diagnosis-only endpoint sets all existing inspection
and admission-explanation counts to zero, keeps `explanation_disclosure` as
`"none"`, and omits other optional disclosures. It acquires no policy-literal,
witness, congress, source or publication authority. The original finite grant
persists across reconnects, scheduled handoffs and terminal reply grace.

Receipt reconciliation requires a separate explicit `resolution` member:

```json
"resolution": {
  "queries": 32,
  "operations": 8,
  "max_replay_events": 4096,
  "authority": "reconcile_existing_receipts"
}
```

This optional object is closed. `queries` must be 1–1,024, `operations` 1–128,
and `max_replay_events` 1–4,096, including the appended Reconcile event. All are unsigned JSON integers. The exact authority marker is required.
Omission or `null` disables the grant. A resolution-only profile sets inspection
and admission-explanation counts to zero, keeps `explanation_disclosure` as
`"none"`, and omits the other optional capabilities. This explicitly permits
recording an existing endpoint receipt in the original journal; it provides no
publication key, fresh clock or evidence source.

For original decoder or action-predictor histories, the object additionally
accepts a positive integer `numerical_replay_events`, bounded by
`operations * max_replay_events`. Each entered numerical Resolve reserves
the complete original journal cut plus its Reconcile event before native work,
even if the original warm replay cache can do less. This counts logical
transitions under retained native model limits, not CPU time or scalar products.
Omission keeps numerical histories unsupported; explicit null, zero, malformed,
duplicate or excessive values refuse before setup. Review explanation's
numerical allowance is separate and cannot pay for a resolution.

Learned actor and resident commands require this numerical allowance whenever
resolution is selected; unbudgeted resolution fails before startup. Omitted/null
resolution keeps their original observer. A positive explicit selection installs
the restricted learned reconciliation operator and original numerical grant.

The independent `review_explanation` object requires `queries` (1–1,024),
`max_replay_events` (1–4,096), and `disclosure: "policy_and_congress_trace"`.
It additionally accepts the optional positive integer `numerical_replay_events`,
bounded by `queries * max_replay_events`. This cumulative allowance reserves
the complete original journal cut before numerical reconstruction; each transition
retains its native model limits. Omission keeps numerical histories Unknown.
Explicit null, zero, malformed or excessive numerical totals refuse.

This grants original policy/congress/reducer and actual application/effect
observations; ordinary admission explanation does not grant it. Unknown nested
fields, duplicate fields, zero/excessive bounds, malformed numeric types and a
different disclosure marker refuse before setup. A review-only endpoint may
leave inspection and admission explanation disabled and omit policy replay.
The original grant and spent work persist across reconnects, scheduled handoffs
and terminal reply grace, including live learned and resident services.
See [completed-review explanations](TOWER_REVIEW_EXPLANATION.md).

The same round selector also serves original completed refinements with a
distinct `refinement-explanation` response. It reports the original residual
group and input revisions with application `not_attempted`; any later dispatch
keeps its separate authorizing review. The existing client validates both
completion kinds under the same bounded parser and independently selected grant.

Selecting `policy_replay` requires `queries`, `cases`, `input_bytes` and the exact
disclosure marker `"policy_and_case_traces"`. This separately grants disclosure
of the previous and candidate policies, including their literal values, and the
complete retained case comparisons and predicate traces. Action payload bytes,
retained source witness values and helper inputs are not included in the Tower
report. Inspection or explanation alone cannot provide this grant. The optional
`page_queries` field separately enables paged delivery. Omitting it preserves
the complete-report-only grant; an explicit value must be a JSON integer from
1 through 1,024. Zero, null, strings, fractions, duplicates and unknown nested
fields refuse before setup. Selecting pages does not change campaign admission
or grant another purpose.

| Replay allowance | Permitted selection | Scope |
|---|---:|---|
| `queries` | 1–64 | Distinct admitted campaign identities over the grant's lifetime |
| `page_queries` | Absent, or 1–1,024 | Separately admitted headers and case pages across every campaign on the same live grant |
| `cases` | 1–5,120 | Complete retained proposal/review cases in one campaign |
| `input_bytes` | 1–33,554,432 | Native logical replay-input accounting in one campaign |
| Retained campaign data | Fixed 8 MiB maximum | Shared candidate and report cache over the grant's lifetime |

Replay input accounting counts action payloads and retained witness occurrences,
including repeated occurrences. It does not measure elapsed time, allocator
memory or fresh source reads. The existing request, transport and response
bounds remain independent. A default v1 request uses the selected
`response_bytes` allowance for the entire report and refuses if it cannot fit.
For example, an explicit 128 KiB selection can deliver a retained policy literal
larger than the default 16 KiB reply. Raising a case/input allowance does not
enlarge the response or retained-cache limit. Even the maximum 1 MiB response
cannot hold every native report; explicitly selected v2 pages deliver that
complete cached corpus in bounded pieces, as described below. Each header or
case page must still fit in one complete response. An output refusal never
substitutes truncated policies, traces or cases.

The client retains its original 1 MiB frame ceiling, depth and string bounds,
correlated closed envelope, and cumulative elapsed/byte/I/O allowances. Only
an explicitly requested `policy-replay` or completed-review `explain` reply uses
its separately selected finite JSON work allowance of 262,144 items,
accommodating complete case, congress and predicate traces.
Other operations retain the original 4,096-item parser limit. A reply exceeding
any bound is rejected before output or a later request.

A replay-only profile sets `inspection_queries`, `explanation_queries` and
`explanation_replay_events` to zero, keeps `explanation_disclosure` as `"none"`,
and supplies a valid `policy_replay` object. It acquires neither inspection nor
admission-explanation custody.

The configured supervisor identity and complete scope must match the actor
service. The socket directory must be supervisor-owned and not group/other
writable; the owned socket receives mode `0660`. The operator identity is
independent of the actor and reviewer profiles. Each endpoint checks actual
Linux UID/GID and optional PID before reading a byte. These are process kernel
identities, not executable or human authentication. If profiles name the same
kernel identity, those processes share the explicitly granted access.

## Exchange original Tower documents

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE SITUATION_JSON EXPLAIN_JSON
```

This client reads only the public operator profile and one to 64 explicit Tower
request files. It freezes and validates the whole request sequence before
connecting, authenticates the server before sending bytes, and shares the
original Tower client's elapsed, byte and I/O-attempt limits. Replies remain
their original complete correlated JSON observations, one per line. There is
no automatic poll, reconnect, retry or implicit submission.

An initial situation request for the example scope and generation is:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "operator-situation-1",
  "address": "fa://1/run/3@41",
  "verb": "situation",
  "arguments": null,
  "idempotency_key": null
}
```

Further pages use the returned opaque continuation according to the
[native inspection contract](TOWER_PUBLICATION.md#inspect-the-original-live-owners-situation).
They preserve the original frozen capture even when the live owner advances.
An explicit request for `explain` uses the original request address printed by
the actor service, for example `fa://1/attempt/request-3-2-4-5-7@41`, with
`arguments: null`. It describes the original policy admission and its distinct
ending cut; it does not establish current eligibility or explain a later hold.

To obtain those original request addresses from the situation itself, explicitly
select `{"request_links":true}` and use the same flag alongside each returned
cursor. The resulting `fa.publication-situation/2` rows include paired nullable
`external_request` and `request_address` fields. They are frozen with the original
capture and preserve full request scope and registration generation. Default
situation requests keep their existing v1 shape. A link can be supplied in a
separate explanation request only when that endpoint already has the independent
explanation grant. See [the request-link contract](TOWER_PUBLICATION.md#follow-captured-attempts-to-original-requests).

## Diagnose an original request's recorded publication conditions

Enable `hold_diagnosis` in the operator profile and submit an explicit
`why-held` request through the existing authenticated client:

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

```sh
supervise_publication tower-operator-exchange operator.json why-held.json
```

Replace the sample scope, original external request and registration with the
actual native request address. The result carries the real acknowledged stage
and recorded Stop/suspension, policy-epoch and deadline conditions. The four
live-evidence/approval/time/permission conditions remain Unknown, even at an
Authorized stage. Each entered query observes the then-acknowledged owner;
repeating a document does not replay a free cached response or renew an allowance.
No source read, new clock, helper execution, admission-snapshot consumption or
journal mutation is performed.

The client validates the separate `fa.publication-hold-diagnosis/1` profile,
request namespace, full native scope, mount generation, request-local generation,
all ten ordered conditions and their original bases before emitting a response
or sending another explicit document. It preserves zero request generation,
the deadline equality rule and all four Unknown live conditions. These checks
establish response correlation and internal consistency, not independent
verification of history or current permission. The ordinary 4,096-item parser
limit and original frame, elapsed, I/O and output limits apply unchanged.

An unexpected subject, changed predicate basis, invented permission or malformed
reply ends the client exchange before output or a later request. Native Unknown
and Capacity responses remain explicit native observations. The client performs
no automatic poll, retry, recovery or follow-up action.
See [the native diagnosis contract](TOWER_HOLD_DIAGNOSIS.md).

## Plan and reconcile an uncertain publication

With `resolution` explicitly selected, send native `next` to the original
external-request address using a prospective operation number:

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

Submit this file with `tower-operator-exchange`. A Known plan either names a
typed block reason or returns one complete executable `resolve` request in
`action`. Select that object unchanged into a new request file and submit it
with a later explicit `tower-operator-exchange` invocation. The client validates
the native subject, original basis and action before output; it does not execute
a returned action automatically. An intervening owner transition can make the
action stale, in which case a new operation is refused before native work.

Resolve queries only the existing original endpoint receipt using the
already-ready acknowledged clock. A real receipt can settle the original
liability. Missing or expired receipts remain Unknown and charged; no send,
seal, fabricated nonexecution receipt or deadline-based refund is introduced.
Entered work invalidates the old one-use admission snapshot.

Within the same live grant, an exact repeated Resolve returns its original
historical result without another native call. A changed input under that
operation number conflicts. That finite cache survives healthy reconnects,
but is not durable across grant replacement or owner reopening. The client
also validates the exact expected predecessor, before/after bases and native
outcome before displaying an acknowledgment or sending any later request.
Transport/output failure never proves that mutation did not commit.

Costs, information gain, ranking and current publication eligibility remain
Unknown. This exposes one connected operation and does not implement a general
planner. Numerical histories require the separate cumulative allowance above.
Next checks it without reserving work; insufficient selected work returns
`numerical_replay_limit` with no action. Exact entered retries consume no further
reservation, and ordinary histories do not spend it.
See [the native next and resolution contract](TOWER_RESOLUTION.md)
for exact fields, authority, accounting, retry and qualification boundaries.

## Compare a candidate against the original retained policy corpus

After explicitly enabling replay in the operator profile, send an ordinary
Tower request through the same authenticated operator client:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE POLICY_REPLAY_JSON
```

For the example run and route registration:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "operator-policy-71",
  "address": "fa://1/run/3@41",
  "verb": "policy-replay",
  "arguments": {
    "campaign": "71",
    "expected_policy_generation": "1",
    "candidate": {
      "generation": "2",
      "nodes": [{"op": "payload_at_most", "maximum": "256"}]
    }
  },
  "idempotency_key": "71"
}
```

The campaign is a nonzero canonical decimal u64, and its string must equal the
idempotency key. The expected generation names the original owner's effective
policy; the candidate generation must be newer. These policy generations are
separate from the route registration `41`, the revocation epoch and the journal
revision. Candidate graphs use the original bounded exact-policy predicates,
with canonical decimal strings for every integer. The existing 2,048-item
request bound also applies, so some graphs at the native policy maximum cannot
fit this transport. Invalid graph or request syntax cannot enter native replay.

The native owner selects all retained complete proposal and applied-review
cases under its current policy, including exact denials. The caller cannot
choose a favorable subset or replace history with supplied archives. Candidate
reads outside the retained observations preserve `RequiresShadow`; a missing
observation is not invented from the current live source. The original evaluator
produces the case comparisons without rerunning helpers or transferring their
old approvals to the candidate.

The result has kind `policy-replay` and schema
`fa.publication-policy-replay/1`. A Known result includes the complete previous
and candidate policies, case count, input charge, comparisons, predicate traces,
missing observations, newly blocked/reviewable cases and original replay limits.
Its acknowledged-owner basis identifies the campaign, full scope, journal
revision, control sequence, policy epoch/generation and previously recorded time.
`remaining_campaigns_at_capture` is the allowance observed when that campaign
first ran, not a current reservation. The result explicitly remains historical
rehearsal evidence with no effect or promotion authority and no policy mutation.

Each admitted new campaign spends one query. Its charge and first result remain
retained even if the comparison fails. An exact retry of the same campaign,
expected policy generation and complete candidate returns that original result
and cut without another replay charge, including after a later publication
advances the owner. Changing the candidate or expected generation while reusing
the campaign produces an idempotency conflict. A fresh campaign sees the later
retained corpus and spends the same grant's remaining allowance. Transport
exchanges and connections remain charged independently, including on exact retries.

The campaign cache belongs to the live grant, survives healthy disconnects and
request-schedule handoffs, and is not a durable restart cache. Every query and
cached retry rechecks original owner health and grant custody. Dropping or
reopening the owner cannot transfer an old grant or keep its report available
through a stale endpoint. Retirement revokes access to the cache with the grant.

Replay has no source callback: it does not acquire or refresh evidence, certify
topology, spend or refund source-refresh allowance, or create a new actor
admission snapshot. The publication source, helper/human, accounting and key
requirements still apply to every actual effect. The operator session receives
`Experiment` purpose only when this native replay grant is explicitly selected;
that replay grant supplies no `Permit`, policy governor or promotion key.
Separately selected resolution/fence grants retain their own restricted authority. See the
[native publication contract](TOWER_PUBLICATION.md) for the original capability
and response boundaries.

## Enumerate a complete policy report through bounded pages

Enable `policy_replay.page_queries` in the service profile, retain a sufficient
`exchange_limit`, and select `response_bytes` for one whole page. The 1 MiB
selection in the example accommodates the bounded pair of policy graphs and
case traces without repeating those policy graphs on every response. Existing
v1 requests and profiles without `page_queries` keep their original behavior.

Use an explicit initial request with `"paged": true`:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "operator-policy-pages-71",
  "address": "fa://1/run/3@41",
  "verb": "policy-replay",
  "arguments": {
    "campaign": "71",
    "expected_policy_generation": "1",
    "candidate": {
      "generation": "2",
      "nodes": [{"op": "payload_at_most", "maximum": "256"}]
    },
    "paged": true
  },
  "idempotency_key": "71"
}
```

Then run the bounded Linux client:

```sh
supervise_publication tower-policy-replay OPERATOR_PROFILE POLICY_REPLAY_PAGED_JSON --max-pages 512
```

The command accepts one initial paged request file and a canonical page allowance
from 1 through 1,024. Legacy complete-report requests and continuation documents
cannot silently begin an enumeration. The request must name the profile's Run,
complete scope and explicit nonzero route generation. The client validates the
local selection before connecting and authenticates the supervisor using the
same kernel credential checks as `tower-operator-exchange`. The server separately
authenticates the operator and enforces its original grants.

The server performs the complete native comparison once. A Known v2 header
contains the previous and candidate policies, original acknowledged-owner basis,
full summary and an issued continuation, with no case rows. Later responses
contain contiguous pages of at most 16 whole cases; policies are null on those
pages. The last case page explicitly marks `enumeration_complete: true`.
The maximum 5,120-case corpus therefore needs 321 responses including the header,
provided every selected native, response and transport bound permits it. No page
request selects a favorable subset or asks the native engine to rerun a slice.

All v2 responses retain kind `policy-replay` and use schema
`fa.publication-policy-replay/2`. The shared summary contains complete case/input
counts, original replay limits, `requires_shadow`, and global delta counts.
`newly_reviewable` and `newly_blocked` are page-local ID lists in native row order.
The header contains empty local lists. `remaining_campaigns_at_capture` remains
historical; `remaining_pages_at_capture` describes the shared page allowance when
that particular page was first admitted. Other readers can spend the allowance
between pages, so a later page may show more than one page of decline.

Every newly admitted header or continuation spends one page query. A new
campaign also spends its existing campaign query and retains the complete report
under the original 8 MiB logical cache bound. Pages share that original report;
they do not make a deep copy of its corpus for each cursor. Their own fixed
metadata cache is bounded separately by the maximum 1,024 admitted page results.
Page work is shared across reconnects, schedule transitions, campaigns and any
clones of the same grant. Exact original header/cursor retries preserve the same
result, next cursor and historical remaining counts without replaying or spending
another page query. Failed output restores neither campaign nor page allowance.

A continuation request uses only the original campaign and exact issued cursor:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "operator-policy-page-2",
  "address": "fa://1/run/3@41",
  "verb": "policy-replay",
  "arguments": {"campaign": "71", "cursor": "123"},
  "idempotency_key": "71"
}
```

Here `123` must be replaced with the actual returned cursor; inventing a number
cannot construct native custody. The native cursor binds the original owner,
grant, campaign, immutable report and next offset. The Tower mount retains only
issued weak handles. Wrong campaigns, foreign or revoked handles, owner loss
and replacement registrations cannot transfer access to that original report.
The automatic client follows each accepted cursor on the same connection and
uses the same campaign and idempotency key. It never follows stale-mount advice
to another endpoint, reconnects, retries or starts a new campaign.

Before output, the public `tower::policy_replay_client::PolicyReplayCapture`
validator checks the closed correlated envelope, v2 schema and flags, original
scope and basis, exact requested candidate and expected previous generation,
unchanged summary, cursor chain, contiguous offsets and native 16-case chunks.
It requires unique case IDs in native proposal-then-review order, exact trace
node coverage and witness-index shape, valid disclosed action fields, consistent
Known/Unknown outcomes and shadow classification, correct page-local delta IDs,
and final agreement with every global delta count. Pure predicates and Boolean
trace results are checked against the disclosed projection. Hidden payload and
witness values remain hidden: structural checks do not reconstruct those values
or independently prove their truth. Authentication and original native custody
remain necessary.

The validator retains two bounded policies, one basis/summary, at most 5,120
case IDs and 1,024 cursor IDs. It does not accumulate the full corpus, traces or
all page bytes. Its parser has an explicit 65,536-item allowance for one header
or at most 16 complete cases, plus the unchanged 1 MiB, depth-16 and 131,072-byte
string limits. Rejected pages change no accepted state and are never emitted or
followed. Other CLI response parsers keep their existing independent limits.

Accepted original replies are emitted unchanged as JSON lines. Success requires
complete, exact case coverage; it confers neither current eligibility nor policy
promotion authority. The smaller of `--max-pages`, the profile's page allowance
and its original exchange limit bounds the automatic loop. The existing elapsed,
64 MiB cumulative byte and finite I/O-call limits also apply, so a sufficiently
large corpus can exhaust transport even when page counts permit it. There is no
implicit increase or fresh connection to evade those limits.

An accepted native refusal, Unknown, Withheld, error, exhausted continuation or
local page cap preserves already emitted valid pages and returns a nonzero exit.
A malformed page is excluded from output. Output failure stops before requesting
the next cursor and can leave a partial final line; consumers must check framing
and process status. A native output-size refusal preserves the original cache
and spent work but does not mean enumeration completed. Configure a suitable
per-page response bound before starting the live service; this client never
reconfigures the service or retries automatically.

## Enumerate one original situation capture

```sh
supervise_publication tower-situation OPERATOR_PROFILE fa://1/run/3@41 --max-pages 64
supervise_publication tower-situation OPERATOR_PROFILE fa://1/run/3@41 --max-pages 64 --request-links
```

This Linux command follows the native situation continuation for you. The run
address must be canonical, match the operator profile's tenant and run, and
include the exact nonzero registration generation. The explicit page allowance
is a canonical integer from 1 through 64. Grammar and address checks precede
profile reads; profile/run checks precede connection. The server authenticates
the operator and the client authenticates the server before protocol bytes.

The default first request has null arguments. Later default requests use only
the exact cursor returned by the previous accepted page, on the same connection and within one
transport budget. The smaller of `--max-pages` and the profile's original
exchange allowance limits requests. Native query exhaustion remains independent
and can end enumeration earlier. No reconnect, retry or new capture occurs.

The optional final `--request-links` flag explicitly selects v2 on every page:
`{"request_links":true}` initially, then the same flag with the exact returned
cursor. Default mode requires the original v1 response; linked mode requires
v2. An unexpected profile or a schema relabeling refuses before output or
continuation, with no automatic fallback.

Linked rows have exactly ten fields, including paired nullable
`external_request` and `request_address`. The client verifies each canonical
nonzero full-width key against the exact address derived from the original
scope and selected registration generation. Tenant, run, principal, branch,
authority, generation and key must all match. Direct attempts require both
fields to be null. Duplicate request identities within/across pages and more
than the original 128 request keys refuse; direct attempts do not consume this
key limit. The entire page is checked before retaining any new IDs or capture
progress. Returned links remain data for separately authorized explicit reads;
the client does not issue Explain or Get calls for them.

Before emitting or following a Known page, the client checks the original
acknowledged-owner basis, complete scope, unchanged journal/control/time cut and
whole-ledger summary. It also checks contiguous offsets, the native 16-row page
bound, unique attempt IDs and continuation IDs, canonical full-width integers,
accounting totals and explicit completion. A page whose capture basis or summary
changes, or whose sequence is inconsistent, is rejected before its bytes are
output. The server's original branded native continuation preserves the actual
capture identity; live owner changes do not replace that capture.

Original accepted replies are written as JSON lines. Exit success means the
Known enumeration explicitly completed; it does not mean the observation is
current eligibility or publication authority. If a page, exchange or native
grant limit ends an incomplete enumeration, already emitted valid pages remain
available and the command exits with an error. Correlated native Unknown,
Withheld, refusal and error replies are also preserved before failure. Output
failure stops further requests without restoring spent native work; a failed
write can leave partial output, so consumers must check both framing and exit
status.

The client retains one basis and summary plus at most 1,024 attempt IDs and 64
cursor IDs, with at most 128 external keys in linked mode, rather than accumulating the entire ledger or all page bytes. A
larger reported total is valid but does not enlarge this work allowance. The
existing client checks elapsed time and finite byte/I/O work at checkpoints;
synchronous OS connection and output calls are not preempted by those checks.

Nine additional authored client regressions cover linked native captures across
live owner changes, full-width/direct identities, explicit v1/v2 selection,
scope/generation/key substitution, nullable pairs and row closure, duplicate
keys with atomic rejected-page handling, no follow-up after a damaged reply,
independent limits, the actual 128-request plus direct-attempt boundary and
closed final-flag grammar. Independent source review and syntax/format checks
remain separate from the required runtime and full repository gates.

## Fence original admission

An explicit optional `fence` profile enables one permanent admission Stop:

```json
"fence": {
  "max_replay_events": 4096,
  "numerical_replay_events": 4096,
  "authority": "stop_original_admission"
}
```

The numerical member is optional for an ordinary history and required for
learned services. The original terminal recovery reserve is mandatory.
Omitted/null `fence` leaves the capability disabled. This selection issues its
own native custodian and never borrows inspection, reconciliation or explanation
allowances. It remains one operation across every reconnect and handoff.

Use `tower-operator-exchange OPERATOR_PROFILE FENCE_REQUEST_JSON` with the exact
current registration, operation/idempotency key and complete original journal,
control, scope and authority-epoch predecessor. Stale observation refuses
without spending the operation. The exact predecessor can race with ongoing
generation/review, so this operation does not promise acceptance within a fixed
time under continuous journal advancement.

The Known result preserves the original local Stop, cancellation/refund and
outstanding charged liabilities. It does not acknowledge endpoint drain, remote
actor termination or fleet fencing. Original host cleanup is a separate step;
the historical response remains unchanged after cleanup. See the
[complete admission-fence contract](TOWER_FENCE.md) for the request, situation
coordinate mapping, explicit retry rules, client validation and cleanup limits.

## Lifetime and authority

The service provisions the original selected native grants once and retains their
noncloneable custodians. One Tower session and its continuation state survive
healthy reconnects. Reconnection, repeated operation labels, review transitions
and reply grace cannot replenish query, resolution-operation, fence-operation, candidate,
connection or exchange work.
The operator's elapsed lifetime is bounded by its profile, actor lifetime and
the remaining wall lifetime of the original supervisor deadline.

The host pumps at most one complete operator frame per cooperative checkpoint.
Independent Stop runs first at active intake and execution checkpoints. Selected
operator work remains available during helper/human review and terminal reply
grace. Resolution requires its separate mutation grant and an eligible original
request. Fence requires its own one-operation grant and exact original
predecessor. After an acknowledged Stop, the workflow invokes its original
driver cleanup before further source, generation or helper work; the separately
retained operator can finish its bounded historical reply grace. An actor
transport failure cannot revoke that independent reply opportunity.

Truncated, oversized or failed operator transport retires that sidecar without
implicitly resubmitting work or entering Stop. A response failure does not roll
back an already entered resolution or Fence. Final retirement revokes all
selected operator custodians. The native grants retain no strong owner or
authority lock, and original health/custody checks apply to every operation.

Only the separate operator session receives operator access and its explicitly
selected grants. Policy replay additionally selects `Experiment`; it does not
grant `Permit`, a source callback, precheck disclosure, helper/human keys or a
supervisor mutation capability. Resolution and Fence independently select
`Permit` through a restricted operator constructor whose closed verb set refuses actor submission,
cancellation, polling and precheck before native work. A proposal sent to an
operator profile cannot acquire source evidence or create an action. The actor endpoint keeps its original grants and
cannot obtain these reads by adding fields to a request. Opening an existing
recorded request can provide historical operator reads without reacquiring
source, launching helpers or repeating approval.

These are cooperative operations on the original acknowledged state. Native
synchronous source, journal and replay work is not preempted by this endpoint.
The feature does not supply the full prospective `fa` CLI, a complete live
publication-eligibility conjunction, ranked planning, general mutation commands
or deployment qualification. Recorded-state `why-held` exposes only the explicitly described
acknowledged facts. The policy-replay
operator addition has nine authored regressions covering closed optional profile
selection, authenticated reports, exact retries after owner advancement,
cumulative reconnect work, kernel-identity rejection, absent disclosure,
separation from publication permission, foreign-owner refusal and owner loss,
explicit response bounds, complete large policy literals and multi-case traces.
They use the original file owner and real Unix sockets. The live-service
client adds five further regressions carrying genuine native reports through
the strict response validator and actual operator command: original Known,
cached-retry and exhausted replies are preserved, while wrong verbs, damaged
envelopes and a corrupted first reply cannot trigger the next campaign. Larger
complete reports preserve all cases, literals and traces across exact retries;
the distinct parser-work limits and unchanged structural bounds remain enforced.
The page addition has three further authored service tests for closed opt-in
page allowances, original grants across reconnects and cached header retries,
and legacy-grant refusal without campaign spending. Five authored client tests
exercise real authenticated 20-case page traversal, local/native exhaustion,
damaged-page suppression, output failure and preconnection request selection.
The public consumer has separate native-response coverage for stable captures,
atomic rejection, exact enumeration and strict trace/Knowledge semantics.
The live-service executable's runtime qualification and full canonical gate remain
separate from targeted public-library policy-replay verification.

The receipt-reconciliation addition has four service/profile, three client and
one learned-startup regression functions covering real original receipts,
explicit action selection, exact retries, stale and unknown outcomes, restricted
authority and rejection of altered responses before later work. These new tests
received source review; compilation and runtime execution remain unverified.

The numerical resolution profile adds two authored profile/service regressions
and a real learned generation/congress/human/client regression spanning both an
executed original receipt and a charged missing receipt. Exact/short budgets,
reconnects, cached results and the driver's settled-state handling are covered.
Independent source review is complete; new compilation and execution are pending.

The admission-fence integration adds four profile/service, three client, one
learned socket and four original cleanup regression functions. Independent
source review is complete; these new tests have not compiled or executed in
this session because the execution workspace became unavailable after two
baseline SIGKILLs. The [fence contract](TOWER_FENCE.md#regression-scope-and-qualification)
records this qualification boundary.
