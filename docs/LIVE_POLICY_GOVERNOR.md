# Independently approve policy changes in a running publication service

The Linux reference executable can provision a separate governor endpoint for a
new or independently recovered ordinary Tower publication run. The governor
requests a replay of the
complete original policy corpus and explicitly approves the exact returned
report. That approval delegates one original opaque permit to the independently
selected operator. The operator then uses the existing `next` / `promote` path.
Approval alone changes neither policy nor effect authority.

This implements the live ordinary reference path for plan §22.7 / FA-130. The
existing [native campaign gate](DURABLE_POLICY_CAMPAIGN_GUARD.md),
[Tower promotion contract](TOWER_POLICY_PROMOTION.md) and original two-key effect
workflow remain the decision and authority implementations.

## Start with separate role custody

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --tower-generation 41 --requests 7,8 \
  --operator-profile OPERATOR_PROFILE \
  --governor-profile GOVERNOR_PROFILE

supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE \
  --tower-generation 41 --requests 7,8 \
  --operator-profile OPERATOR_PROFILE \
  --governor-profile GOVERNOR_PROFILE
```

The closed selection requires ordinary `serve-create` or `serve-create-checked`,
an explicit schedule of at least two requests, the Tower generation, and both
independent profiles. Checked creation requires the existing plain publication
witness profile. Feed, wait, producer, snapshot fallback, whole-domain and joint
profiles remain explicit refusals; a stronger selected verifier is never reduced
to plain witness limits. The selection refuses topology, precheck and
generated/learned modes before reading their configuration. Governed open uses
the separate explicit recovery selection below. Existing journals are never
upgraded to campaign governance by an actor or connection.

The complete first canonical image contains the selected publication guard,
configured file-source guard, campaign requirement and original terminal
recovery reserve. Only its acknowledged creation returns the independent human
and governor roles. The human role follows the original effect-review workflow;
the governor role stays in the governor service.

For checked creation the original publication witness limits are part of that
same first image. Policy promotion preserves them: each later effect still binds
its own original capture and passes final witness validation under its current
policy epoch. An old human key or a changed required value, absence or range
cannot be excused by the governor's policy approval.

The governor profile is a closed JSON document. Profile counters use ordinary
unsigned JSON numbers. This example's identities and paths must be replaced by
the independently provisioned local processes:

```json
{
  "schema": "fa.policy-governor-service/1",
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "socket": "/run/franken-alignment/governor.sock",
  "supervisor": {"uid": 1000, "gid": 1000, "pid": 1100},
  "governor": {"uid": 1000, "gid": 1000, "pid": 5100},
  "candidate_limit": 8,
  "connection_limit": 4,
  "exchange_limit": 16,
  "runtime_ms": 60000,
  "poll_ms": 5,
  "campaigns": {"cases": 512, "input_bytes": 1048576, "max_campaigns": 8},
  "campaign_limit": 4,
  "report_bytes": 1048576
}
```

Governor credentials must be disjoint from the actor, operator and human
reviewer credentials. Different UID/GID admissions are disjoint; equal UID/GID
admissions require both roles to pin different nonzero PIDs. A wildcard PID
intersecting another role refuses. The audience and supervisor identity must
match the original service and every other selected profile.

The supervisor-owned socket directory must not be group/other writable. The
governor socket must be outside the authority store and distinct from the
actor, operator and every scheduled human/Stop endpoint, including canonical
ancestor aliases. Existing socket paths are never replaced. Role and pathname
checks precede clock, source, socket and store work.

Add this independent optional object to the existing operator profile:

```json
"policy_promotion": {
  "queries": 16,
  "max_replay_events": 4096,
  "authority": "promote_approved_policy"
}
```

It selects a restricted control mount and explicit Permit purpose, but issues no
key. The ordinary command requires the governor and this operator selection
together. Native/learned numerical operator profiles refuse this ordinary
promotion selection. The operator can receive only one approved campaign during
its original lifetime; reconnects and request-schedule handoffs do not renew it.

## Recover the governed owner with fresh independent custody

Both ordinary and simple-witness checked services can continue an already
campaign-guarded owner through its original anchored recovery:

```sh
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --tower-generation 42 --requests 8,9 \
  --guarded GUARDED_SUBMIT_V2_PROFILE \
  --operator-profile OPERATOR_PROFILE --governor-profile GOVERNOR_PROFILE

supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE \
  --tower-generation 42 --requests 8,9 \
  --guarded GUARDED_SUBMIT_V2_PROFILE \
  --operator-profile OPERATOR_PROFILE --governor-profile GOVERNOR_PROFILE
```

The recovery document has the closed `fa.guarded-submit/2` schema. Retain the
actual current **complete policy**, exact campaign guard limits, independent
minimum floors and history anchor before selecting this command. Values below
are examples and must match that separately trusted expectation:

```json
{
  "schema_version": "fa.guarded-submit/2",
  "minimum": {
    "journal_revision": 70,
    "control_sequence": 10,
    "authority_epoch": 1
  },
  "history_anchor": "/operator/custody/previous.anchor",
  "retain_history_anchor": "/operator/custody/next.anchor",
  "effective_policy": {
    "generation": 2,
    "nodes": [{"op": "payload_at_most", "maximum": 512}]
  },
  "campaigns": {"cases": 512, "input_bytes": 1048576, "max_campaigns": 8}
}
```

The governor profile must select exactly those campaign limits. A legacy `/1`
profile, missing campaigns, changed bounds or stronger publication profile
refuses before the clock or live-resource acquisition. The original locked
cursor then verifies the complete history, independent anchor and floors, exact
full policy contents, source guard, terminal reserve and selected publication
limits before recovery acknowledgment. A different policy with the same
generation is still a mismatch. No source observation, policy approval or new
actor input is reconstructed from retained metadata.

Replay yields at every original event and at Ready. The independently
authenticated Stop listener remains available throughout replay. A pending peer
selects the original sealed recovery Stop exchange before ordinary completion;
complete replay and all requirements remain mandatory. Stop can acknowledge its
original admission Stop/Fence pair without an effect-clock observation, fresh
source, reconciliation sweep or live governor. Unknown/dispatched effects retain
their charges. A lost response or late deadline cannot erase the acknowledged
native Stop or suppress requested successor-anchor retention. Startup then ends
without admitting work.

With no pending Stop, the same cursor completes the original `Fence → Time →
Sweep` transaction once. All unresolved Dispatching, Unknown and irrecoverably
unknown deliveries block new intake. The fresh owner preserves policy history,
confirmed effects, charges, native campaign spending and original reserve, while
old source eligibility, old roles and old promotion keys stay revoked. The
service transfers only the newly returned governor into its independently
selected endpoint. A later policy change needs a fresh full-corpus campaign,
explicit governor approval and separate operator promotion; no old report or
cached response restores approval.

The original Stop control continues through preparation and first intake. New
publication still requires current source evidence, helper review, fresh human
approval and the selected final witness check. Actor proposals must carry the
actual new epoch and target; the command never rewrites them. The selected
successor anchor is retained from that same acknowledged owner on normal success
and returned failure paths, including failures before intake. A retention or
response error cannot undo a known effect or authorize a resend. Keep the old
independent anchor until its successor is acknowledged.

## Discover the current cut through the independent governor endpoint

The governor can obtain the exact predecessor and originally selected campaign
bounds using its own authenticated endpoint. It needs no operator credential or
read access to the authority store. Send one explicit inspection document:

```sh
supervise_publication policy-governor-exchange GOVERNOR_PROFILE GET_REQUEST_JSON
```

```json
{
  "schema": "fa.policy-governor-request/1",
  "scope": {
    "tenant": "1", "principal": "2", "run": "3",
    "branch": "4", "authority": "5", "purpose": "effect"
  },
  "verb": "get",
  "query": "501"
}
```

The response uses the existing `fa.policy-governor-response/1` schema with
`verb: "get"`, `phase: "inspect"`, and the same nonzero canonical decimal `query`.
Its `Known` value contains `journal_revision`, `control_sequence`,
`authority_epoch`, `policy_generation`, the originally selected
`campaigns: {cases,input_bytes,max_campaigns}`, `campaign_limit`, `report_bytes`,
`stopped`, and `source_interrupted`. All counters are canonical decimal strings;
`stopped` is a Boolean, while `source_interrupted` is the original latch Boolean
or `null` when the owner has no file-source guard. The acknowledgment basis binds
the full scope, query and journal revision to `origin: "acknowledged_owner"`.
The client checks these fields and the exact limits against its independently
selected connection profile before displaying the document.

Copy the returned four cut fields into the explicit replay request's `expected`
object, and use the selected campaign bounds. The response records one immutable
native cut. Intervening work can stale that predecessor; the subsequent native
replay gate still checks it. Inspection performs no clock observation, source
read, campaign request, approval or key issuance, and does not renew any limit.

An exact query retry returns its original cached historical result. Select a new
explicit query identity to observe a later cut. Query and campaign identities are
separate even when their numbers match. Inspection spends the original finite
transport/exchange/cache allowances but no campaign allowance. A discovered
stopped or interrupted state is evidence of that state and grants no new work.

## Request and examine the complete original report

The governor uses one explicit document per client invocation:

```sh
supervise_publication policy-governor-exchange GOVERNOR_PROFILE REVIEW_REQUEST_JSON
```

The request uses registered verb `policy-replay` and the same candidate format as
the [existing replay command](POLICY_REPLAY_OPERATOR.md). Custodian wire counters
and scope identifiers use canonical unsigned decimal strings. Replace every
expected counter below with the original current cut; these values are examples:

```json
{
  "schema": "fa.policy-governor-request/1",
  "scope": {
    "tenant": "1", "principal": "2", "run": "3",
    "branch": "4", "authority": "5", "purpose": "effect"
  },
  "verb": "policy-replay",
  "operation": "801",
  "expected": {
    "journal_revision": "57", "control_sequence": "8",
    "authority_epoch": "0", "policy_generation": "1"
  },
  "candidate": {
    "schema_version": "fa.policy-replay-request/1",
    "expected_policy_generation": 1,
    "limits": {"cases": 512, "input_bytes": 1048576, "output_bytes": 1048576},
    "candidate": {
      "generation": 2,
      "nodes": [{"op": "payload_at_most", "maximum": 512}]
    }
  }
}
```

The candidate is the complete replacement policy, not an additional predicate.
Its replay cases/input limits must exactly match the startup campaign
requirement, and its selected output bound must fit `report_bytes`. The native
corpus must be nonempty; a completed first request can supply both proposal and
review cases while the service waits for the next scheduled request.

The sidecar captures the exact pre-request snapshot and invokes the original
`request_policy_campaign`. It renders the complete returned native report with
the existing canonical encoder. A known response has schema
`fa.policy-governor-response/1`, the same scope, verb, campaign ID and phase
`review`. Its `knowledge.value` contains:

- `journal_revision`: the acknowledged Request revision, exactly the selected
  predecessor plus one.
- `reviewed_report`: the complete canonical `fa.policy-replay/1` document as a
  JSON string, including its final newline, original pre-request cut, both
  policies, every case and trace, changed sets and shadow requirement.

Successful acknowledgments use the original `Known { value, basis }` shape.
The basis records `origin: "acknowledged_owner"`, the complete scope, campaign
and acknowledged journal revision; the client checks them against both the
original request and the returned value. A command error instead carries a
sibling `error: {"code": ...}` and no `knowledge` field. In particular,
`unconfirmed` does not claim that an entered approval was rolled back.

No new source read, helper run or native time observation is made to improve the
report. Review is historical; missing observations remain unknown. If the
candidate requires shadow, explicit acceptance cannot waive it.

## Approve those exact bytes, then explicitly promote

After independently reviewing the full report, send a separate document:

```json
{
  "schema": "fa.policy-governor-request/1",
  "scope": {
    "tenant": "1", "principal": "2", "run": "3",
    "branch": "4", "authority": "5", "purpose": "effect"
  },
  "verb": "promote",
  "phase": "approve",
  "campaign": "801",
  "expected_journal_revision": "58",
  "reviewed_report": "EXACT STRING FROM THE REVIEW RESPONSE",
  "accept_newly_reviewable": false
}
```

The placeholder is not accepted: copy the complete string value without
reformatting its embedded JSON or dropping the newline. `true` is an explicit
acceptance of the report's newly reviewable set; it still grants no effect key.
Use the same single-document client command with this approval file.

Approval compares the complete report bytes and current revision, then calls
the original nonclone governor. The original complete-corpus stamp, source
interruption latch, Stop and native journal limits remain mandatory. Intervening
proposals or reviews can stale a campaign, including exact denials that do not
advance the control sequence. A fresh explicit campaign/report is required;
neither service nor client substitutes a newer predecessor.

Before approval, the service checks that the independently selected operator
can still receive and use the delegation. It installs the original key on that
same operator session before acknowledging success. The known approval reply
reports `approval: "delegated"`, `policy_changed: false` and
`effect_authority: false`. No key bytes cross either socket.

The operator uses `tower-operator-exchange` with its own profile to send
`next` with `{"action":"promote_policy","campaign":"801","operation":"802"}`,
then explicitly selects the returned Promote action at its exact predecessor.
See [Tower promotion](TOWER_POLICY_PROMOTION.md) for the complete request and
receipt contracts. The actor cannot perform either governor approval or the
operator's promotion. Later publication requires new current evidence, helper
review and a fresh independent human key under the new policy/authority epoch.

## Bounded transport, retries and retirement

The dedicated governor protocol uses a four-byte big-endian unsigned length
followed by one JSON document. It is separate from the newline-framed Tower
operator endpoint. Kernel credentials are verified before any protocol byte.
The client verifies its server before sending and validates the correlated
complete response before writing it to output. It never auto-approves a report,
promotes a policy, reconnects or resends a request.

The original session retains candidate, authenticated connection, exchange and
elapsed allowances across reconnects. Additional fixed ceilings bound total
read/write bytes, socket calls, per-turn work and retained cache bytes. A turn
handles at most one complete request. There are at most eight selected campaign
attempts, at most 32 cache entries and one acknowledged approval. A report is at
most 1 MiB; custodian requests are at most 2 MiB.

Canonical exact retries return the original cached response. A changed review
request for an entered campaign ID refuses. An exact approval retry retains its
original result, including a known refusal; a separate corrected explicit
approval can proceed only if the original native gate and unchanged cut still
permit it. A lost acknowledgment never reconstructs or reissues a nonclone key.
Entered uncertain work returns an unconfirmed error.
An already delegated grant remains with the operator if the governor connection
fails or the governor sidecar retires. Sidecar failure never resubmits an effect,
refunds an unknown outcome or stops the publication owner. Operator retirement
still drops its own grant. Both sidecars run after the independent Stop check;
new governor work ends before the terminal operator reply grace.

## Evidence and remaining scope

The authored regression suite uses an actual separate governor process, native retained
proposal/review cases, real authenticated sockets, an existing operator
connection, two original publications and a fresh human decision after
promotion. Paired refusals cover overlapping identities, mismatched reports,
stale complete cuts, foreign audiences, finite custody and malformed frames.
Client negatives use genuine native reports with specific fields altered or
removed. Exact executed results are recorded in `IMPLEMENTATION_STATUS.md`.
The qualified compiler built the complete example test harness. In the current
execution environment, AF_UNIX socket creation itself is refused, so the live
listener/client/end-to-end cases did not reach their intended protocol work.

These are scoped Unix reference regressions; source coverage alone does not
establish execution or full qualification. Kernel credentials
identify the connecting local process under the trusted host assumptions; they
do not prove evaluator competence or cryptographic independence. Live recovered
service execution, mediated/numerical governor composition, richer publication
profiles, policy efficacy and
full production/runtime qualification remain separate unfinished capabilities.
