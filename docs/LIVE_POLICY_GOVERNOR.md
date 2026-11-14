# Independently approve policy changes in a running publication service

The Linux reference executable can provision a separate governor endpoint for a
new or independently recovered ordinary Tower publication run, or a new or
independently recovered nonpredictive learned sequence with explicit numerical
allowances. The governor
requests a replay of the
complete original policy corpus and explicitly approves the exact returned
report. That approval delegates one original opaque permit to the independently
selected operator. The operator then uses the existing `next` / `promote` path.
Approval alone changes neither policy nor effect authority.

This implements live reference compositions for plan §22.7 / FA-130. The
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

## Govern a new resident learned sequence

The learned service has its own explicit selection:

```sh
supervise_publication serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --campaign-profile CAMPAIGN_PROFILE \
  --operator-profile OPERATOR_PROFILE --tower-generation 41 \
  --governor-profile GOVERNOR_PROFILE
```

The sequence must contain at least two Messages, with an optional final Finish.
This selection creates a new nonpredictive, nonmediated learned owner. Creation
refuses continuation and recovery selections; the explicit anchored recovery
selection below accepts them without a creation campaign profile. Both refuse
topology, forecast and joint credibility selections before model/helper assets
or live resources are acquired. The closed argv also
admits the existing optional checkpoint before generation and new output history
anchor. A version-two sequence may select its original per-entry precheck
listeners; those remain observations and cannot create governor authority.

The campaign profile, governor profile and every recipe's recovery expectations
must select exactly the same original campaign limits. Before reading model
assets, the command checks every actor's audience, credentials and scheduled
human/Stop endpoints against the separately selected governor. Later actors
cannot inherit the governor's identity. The original source path, disclosure
listeners and retained output paths cannot alias the governor socket. Ordinary
learned and native-text services keep their existing default promotion refusal.

The original guarded learned constructor installs the stream, source, campaign
guard, learned configuration and terminal reserve in its first canonical image.
Only that acknowledged creation returns the governor retained by the new live
service. There is no late campaign enable, reopened role getter, replacement
numerical owner or approval reconstructed from report bytes.

### Pay for Request/Approve separately from Promote

Add the following optional object to the governor profile. Its values are JSON
numbers, like the other profile counters:

```json
"numerical_replay": {
  "max_replay_events": 4096,
  "events": 8192
}
```

`max_replay_events` is the largest complete history plus proposed event admitted
by one governor Request or Approve. `events` is the cumulative allowance across
every entered numerical operation in that original service. Both are positive;
the per-operation limit cannot exceed the native journal ceiling, and the total
cannot exceed `exchange_limit * max_replay_events`. Choose bounds for the actual
retained history and campaign attempts; these examples promise no universal
capacity. Refused native contracts after entry still consume the reservation.

The independent operator requires its own promotion allowance:

```json
"policy_promotion": {
  "queries": 16,
  "max_replay_events": 4096,
  "numerical_replay_events": 4096,
  "authority": "promote_approved_policy"
}
```

This second numerical allowance is positive and no larger than its one-Promote
history ceiling. The governor cannot spend it to request or approve a campaign;
the operator cannot spend the governor's remaining work. Each native transition
reserves its current journal revision plus one before reconstruction, operation
entry and admission snapshot invalidation. A short allowance refuses without
consuming the snapshot. Get, Next and exact retained retries perform no numerical
reconstruction. Entered failure, warm replay, lost replies and reconnects never
refund or refill either allowance. Units count admitted replay events, not
elapsed time or scalar products; the original recipe's numerical limits also
remain in force.

### Promote at an unchanged original learned cut

Use the same explicit Get, complete-report Review, exact-report Approve and
operator Next/Promote documents described below. After a first publication has
supplied retained cases, wait for the later learned message to finish generation
and withhold its actor Submit until the policy transition completes. The optional
original precheck listener can expose this existing post-generation wait point.
Numerical advancement changes the actor revision and stales an earlier campaign;
the service never silently rebases its report or approval.

Promotion preserves the original completed numerical draft, KV state, sampled
draws, source accounting and immutable recipe. New intake builds its action at
the current native policy epoch. It requires fresh current source evidence,
original native helper review, a new independent human approval and the selected
final publication check. Previously admitted attempts keep their original
specifications and are cancelled by the native promotion transition. Publication
or source checks are not waived by a policy report or by available replay work.

The same governor and operator survive sequence handoffs with their original
deadlines, connection counts, caches and work budgets. They run after the
independent Stop check at generation, actor, helper review, human review,
publication and helper-retirement checkpoints. Native and process helper wrappers
expose their existing supervisor for this bounded turn; no wrapper or driver is
reconstructed. Predictive promotion still needs a separately reviewed successor
predictor binding. Installed joint credibility likewise needs a fresh
qualification after policy change and is outside this initial live selection.

## Recover a governed learned sequence

The same live service can recover an already campaign-guarded, nonpredictive,
nonmediated learned stream. Select the continuation explicitly and retain the
original full numerical recipe, source contract and stream limits:

```sh
supervise_publication serve-learned-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST \
  --history-anchor INPUT_ANCHOR --retain-history-anchor NEW_OUTPUT_ANCHOR \
  --operator-profile OPERATOR_PROFILE --tower-generation 42 \
  --governor-profile GOVERNOR_PROFILE
```

Use `--continue-generation` instead of `--after-message PREVIOUS_REQUEST` for
the original unsubmitted generation. These selections are mutually exclusive.
An already admitted request must use its original receipt/reconciliation path;
recovery never turns a missing response into permission to resubmit an effect.

Every selected recipe must carry the existing
[`fa.learned-recovery-policy/2` envelope](LEARNED_RECOVERY_POLICY.md) with the
**complete current effective policy** and exact original campaign inventory.
All recipes must agree on the full policy, including its predicates, and the
governor profile must select those same campaign limits. Missing expectations,
same-generation policy disagreement, unsupported modes and conflicting role
credentials refuse before numerical assets are loaded. The admitted recipe
documents are retained for the actual loaders; they are not reread after this
check. `--campaign-profile` is creation-only and refuses during recovery.

An explicitly governed recovery accepts 1–16 remaining entries. It may contain
a single Message, a single Finish, or Messages followed by an optional final
Finish. A single Finish requires `--after-message` naming its nonzero original
receipt-confirmed Message; it refuses `--continue-generation` and
`--checkpoint-before-generation` before recipe or model assets are read.
The original Finish checks still refuse an unresolved or Unknown predecessor.
Finish does not start a continuation or consume another sampled draw, and still
requires fresh source, native or process helper review, and human approval.

Creation and sequences without an explicit recovery governor retain their
2–16 entry bound; governed creation still requires at least two Messages.
No selection increases the original message, byte, sampling, numerical or
campaign budget. Only entry one may select recovery floors and history custody;
every entry still selects its own actor, request, sidecar and review rounds.

The original exclusive recovery cursor verifies the independent anchor, exact
policy and complete guard inventory before its sole recovery fence. Its deadline
and authenticated Stop checkpoints run before each original event and at Ready.
Pending Stop uses the same terminal exchange and retains the acknowledged
history even after a lost reply; no live governor is issued on that branch.
Otherwise the same native finalizer returns the fresh human and governor roles.
Old actor ports, governor handles, effect keys and campaign approvals remain
revoked. Original learned state, confirmed output, unresolved charges and spent
campaign capacity remain part of the recovered owner.

The new invocation independently provisions its governor/operator credentials
and finite transport and numerical replay allowances. These new service grants
do not reset persistent native generation or campaign expenditure. Reconnects
and message handoffs within the invocation retain the same grants, deadlines
and remaining work. A later promotion needs a fresh complete campaign, explicit
independent approval and the separate paid Promote operation. Fresh source,
helper and human checks remain mandatory for every new publication.

When successor-anchor retention is selected, a failure during post-fence
request preflight or listener startup retains that exact acknowledged recovery
cut. Normal completion and later failures retain the original live owner's final
acknowledged cut. Neither path reopens the store or retries an effect. Keep the
old independent anchor until its successor is acknowledged.

The recovery adapter and public-owner regression sources have independent
source review and syntax checks. Rust compilation, authenticated service
execution and full qualification remain pending; the required RCH runner is
unavailable in the implementation workspace.

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

An explicitly numerical governor adds `numerical_replay` to the Known value,
containing canonical decimal strings `max_replay_events`, `events` and
`remaining_events`. The first two must exactly match the independent connection
profile, and the remaining value cannot exceed the original total. Ordinary
profiles omit the entire object and reject an unselected object; numerical
profiles reject its omission. A cached Get retains its historical remaining
count. Neither a nonzero count nor a discovery response creates a grant.

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
Learned cases additionally retain a separate governor process through two real
Messages and Finish, promote after generation while actor submission waits, and
require fresh native and human review. Budget cases exercise real decoder
history, exact complete-history charging, one-short snapshot preservation and
nonrefundable native/storage/report-output failures. The actual standalone native
promotion targets execute separately from the example harness.

Current revision-specific compilation and execution results are recorded in
`IMPLEMENTATION_STATUS.md`. The complete production executable and test-harness
metadata are separate checks from linking and executing that large harness. In
this execution environment, AF_UNIX socket creation itself is refused; real
listener/client/end-to-end execution remains unverified. No assertion is removed
or replaced to turn that environment refusal into protocol evidence.

These are scoped Unix reference regressions; source coverage alone does not
establish execution or full qualification. Kernel credentials
identify the connecting local process under the trusted host assumptions; they
do not prove evaluator competence or cryptographic independence. Live recovered
service execution, mediated or predictive learned governor composition, live
learned recovery, joint credibility requalification, policy efficacy and
full production/runtime qualification remain separate unfinished capabilities.
