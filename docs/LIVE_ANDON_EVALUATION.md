# Independent evaluation on the original live Tower owner

The Linux `supervise_publication` service can provision native helper credibility
and Andon accounting in an ordinary publication owner and accept independent
assessment while that same owner retains the journal lock. An operator can read
the acknowledged results through [Tower Ledger](TOWER_GOVERNANCE_LEDGER.md)
and [Findings](TOWER_GOVERNANCE_FINDINGS.md).
The actor still needs original source admission, helper review, independent human
approval and publication checks for every new effect. Native and learned
generated services also support the explicit compositions below.

This connects FA-105/049/134, plan §§9.9, 13.4, 17.1, 22.3–22.4 and
FI-A08/A13/A18. It is a bounded reference composition; execution qualification
remains pending. It does not make helper or actor judgments into ground truth.

## Create an accounted service

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --tower-generation 41 --requests 7,8 \
  --andon ANDON_PROFILE \
  --retain-history-anchor /absolute/independent/new-anchor \
  --operator-profile OPERATOR_PROFILE \
  --evaluator-profile EVALUATOR_PROFILE
```

The original ordinary profiles still define the scope, actor/reviewer identities,
source, helpers, work limits and independent Stop. Select the operator's finite
`governance_ledger` grant to read accounting. Operator and evaluator endpoints are
optional and independently selected; a read grant cannot issue a label.

`ANDON_PROFILE` reuses `fa.native-andon-operator/1` or its explicit joint `/2`
profile and the [native operator contract](NATIVE_ANDON_OPERATOR.md). The ordinary
fixture is `crates/fa-reference/fixtures/native_andon_operator.json`. Creation
requires identical initial/current windows and evaluation revision zero. The
original evaluated, source-guarded owner and terminal recovery reserve appear in
its first durable image. No provisional unaccounted owner escapes.

Creation requires a new independent history-anchor output. It may use the actor
profile's single request or an explicit finite `--requests` schedule. The checked
variant is `serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE
WITNESS_PROFILE` with the same flags; only the original simple witness composition
is supported. An explicit `--precheck-schedule` can accompany `--requests`.

## Create an evaluated native generated service

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text RECIPE \
  --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --retain-history-anchor /absolute/independent/new-native-anchor
```

The checked form adds `WITNESS_PROFILE` after `REVIEWER_PROFILE` and uses
`serve-create-checked`; the live composition accepts the simple witness profile.
Both accounting and evaluator profiles are required together, with a new retained
history output. The existing single-request native actor wire still accepts only
the original generated-source reference; the service never emits unapproved text
to stdout. Actor submission and independent human review remain separate steps.

The [atomic native constructor](NATIVE_GENERATED_EVALUATION.md) installs source,
decoder/tokenizer, stream/publication, terminal reserve, evaluation and optional
joint/Andon configuration in one first image. The returned evaluator stays with
the same owner across numerical generation, actor intake, process-helper review
and publication. The existing `evaluator-case` and `evaluator-assess` commands
operate on that original owner; the journal is not reopened to label a live case.

Optional `--precheck-profile` and paired `--operator-profile` /
`--tower-generation` retain their existing native contracts. Evaluator credentials
must be disjoint from the actor, reviewer/Stop and operator. Audiences, credential
separation, supported modes and aliases among source/witness inputs, sockets,
profiles and retained history are checked before clock, numerical or owner work.
The original reducers validate the complete bootstrap before store creation.
Select accounting windows covering the
actual native occurrence times under the configured Unix-millisecond clock.

After the request workflow completes, a healthy unstopped owner remains available
for evaluation under the **remaining original endpoint and invocation budgets**.
This gives a final Hold a real opportunity for case discovery and a benign,
violation or censored label. A case read and one assessment consume two existing
candidate slots. Exhausting a budget or the original deadline ends this phase;
it does not restart generation, helpers, a held review or publication.

Independent Stop is serviced first, including after any separately selected
operator command. Original operator grants may still service their selected
cancellation, narrowing, reconciliation, Fence and bounded queries; no grant is
renewed. Actor transport failure retires actor intake while preserving the
evaluator's remaining opportunity and the recorded error. Storage failure or
Stop ends new evaluation. Terminal reply grace then drops evaluator authority
and permits only an already acknowledged reply to drain. Final retained history
includes any acknowledged assessments and accounting.

This single-request selection supports native creation and explicit anchored
recovery, plain or simple checked. Native topology, governor and held-out
qualification selections still need their own complete composition and refuse
this option. The live recovery path below reuses the original
[anchored accounting cursor](NATIVE_GENERATED_EVALUATION.md#recovery-with-independent-contracts).
Learned generation uses its separate commands below.

[Resident native generated sequences](RESIDENT_NATIVE_PUBLICATION.md#independent-live-evaluation-and-andon-accounting)
use their separate `serve-native-generated` command with paired `--andon
ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE` and a required
`--retain-history-anchor NEW_OUTPUT`. The original evaluator and finite
allowances persist across Message/Finish handoffs, including a bounded labeling
opportunity after a final helper Hold. Creation and
[explicitly anchored suffix recovery](RESIDENT_NATIVE_PUBLICATION.md#recover-an-accounted-resident-suffix)
accept the marginal `/1` accounting profile and optional simple witnesses/operator
Ledger. Recovery selects `--after-message` for a confirmed boundary or
[`--continue-generation`](RESIDENT_NATIVE_PUBLICATION.md#resume-interrupted-accounted-generation)
for the exact original pending or completed-but-unsubmitted first Message.
Both require an independent input anchor and the same explicit evaluation
recovery requirements used below; one fresh evaluator serves the recovered
sequence without renewing its allowance at handoffs. Explicit version-two
per-release exact prechecks compose with both creation and recovered sequences,
retaining their original actor grants, complete endpoint/input custody and
actual owner epoch checks before source or numerical work. Joint profiles,
governance and topology remain separate. Required RCH compilation and runtime
qualification remain pending.

## Recover an evaluated native generated service

An interrupted accounted native generation can return to the live service with
the original numerical spend, pending evaluation cases, liabilities and complete
accounting windows. Select all current contracts independently before opening
the journal:

```text
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text RECIPE \
  --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --history-anchor /absolute/independent/original-native-anchor \
  --retain-history-anchor /absolute/independent/new-native-anchor \
  --evaluation-recovery RECOVERY_REQUIREMENTS
```

For simple checked publication, use `serve-open-checked CONFIG ACTOR_PROFILE
REVIEWER_PROFILE WITNESS_PROFILE` followed by the same options. Both accounting
and evaluator profiles, the retained input anchor, a new output anchor and the
recovery requirements file are required together.

The requirements file has the following closed schema. Choose every floor from
independently retained expectations for this original history; the values shown
illustrate the format:

```json
{
  "schema": "fa.native-evaluation-recovery/1",
  "minimum": {
    "journal_revision": 17,
    "control_sequence": 3,
    "authority_epoch": 1
  }
}
```

The file is bounded to 4 KiB. All three floor fields are required; unknown,
duplicate, missing or malformed fields refuse. The service does not infer a
floor from the journal being checked. The input history anchor separately pins
the independently acknowledged history cut.

`ANDON_PROFILE` supplies the original evaluation identity and protocol, initial
windows, the complete current window inventory and the independent minimum
evaluation revision. If the original owner has the supported joint accounting
policy, that policy must also be supplied. Omitting a registered window or
changing a selected contract cannot create a fresh unaccounted owner.
`CONFIG` and `RECIPE` independently select the original source, decoder,
tokenizer, stream, publication limits, current unchanged policy and complete
supported guard inventory, including required guard absences. The live path
consumes `FileAndonGeneratedSourceRecovery`; it does not add a replay reducer.

Without an explicit predecessor option, the service resumes the selected
pending generation or a completed generation that has not yet been submitted.
Use `--after-message PREVIOUS_REQUEST` for the next Message or
`--finish-after-message PREVIOUS_REQUEST` for Finish. These retain the original
confirmed-predecessor, distinct-new-request, stream and recipe requirements.
Finish performs no new token generation. Every new publication still needs
fresh source evidence, current helper review and independent human approval.

The original Stop control is available before replay for every evaluated open.
The original deadline and Stop are checked before recovery, at every one-event
replay boundary and separately at Ready. Stop terminalizes the original owner
before normal recovery can release live human or evaluator roles, and the new
output retains the acknowledged terminal history. Normal recovery acknowledges
its original Fence before provisioning fresh live roles.

A recorded actor request is receipt-only under the existing native recovery
contract. Paired live evaluation explicitly refuses that mode, retains the
acknowledged recovery successor and starts no evaluator listener or new actor
reference. Fresh-source failure after a successful Fence uses the original
failure Stop cleanup and likewise retains the acknowledged successor. Neither
case permits replaying an effect as a new generation.

The input anchor, requirements, accounting/evaluator profiles, recipes and
source/witness inputs must remain disjoint from authority storage, retained
output and every selected service endpoint. The new output must be independently
held and distinct from the input. Original evaluator separation, finite endpoint
and invocation budgets, pending-case discovery, late labeling and terminal reply
grace retain their creation-path contracts. A new invocation cannot replenish
the original native numerical budget or erase prior accounting.

This composition covers single-request plain or simple checked publication.
Resident native sequence recovery uses its
[explicit suffix composition](RESIDENT_NATIVE_PUBLICATION.md#recover-an-accounted-resident-suffix).
Topology, governors and stronger feed/snapshot/held-out qualification remain separate selections; unsupported
combinations refuse without dropping a selected guard. Joint Andon accounting
does not imply held-out joint publication qualification.

Seven integration regressions and strict requirements-parser tests are authored.
They cover positive continuation and fresh human approval/rejection, independent
late labels, accounting-window and protocol retention, each independent floor,
foreign anchors, Ready Stop, source failure, receipt-only refusal and exact
simple-checked guards. Compilation and runtime qualification remain pending the
required remote RCH gate.

## Evaluate recorded native history

Use an explicit historical mode when independent truth arrives after the original
native service has ended:

```text
supervise_publication serve-open CONFIG ORIGINAL_ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text ORIGINAL_RECIPE --evaluate-only \
  --andon CURRENT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --history-anchor RETAINED_INPUT --retain-history-anchor NEW_OUTPUT \
  --evaluation-recovery RECOVERY_REQUIREMENTS
```

For an original simple checked owner, use `serve-open-checked` with its original
`WITNESS_PROFILE` positional argument. To select an already recorded Finish,
place `--finish-after-message PREVIOUS_REQUEST` last. The actor profile then
identifies the original Finish request, and the recipe identifies its explicitly
named original Message predecessor.

This mode requires the same complete
[accounted recovery inputs](#recover-an-evaluated-native-generated-service):
the independent input archive, explicit journal/control/authority floors,
original source/stream/publication/model/tokenizer contracts, original evaluation
protocol and accounting identity, all current windows and the independently
retained evaluation floor. Original numerical assets remain necessary for
verified replay. Current publication evidence and helper invocations are not
needed to evaluate historical cases. Plain and simple checked publication retain
their existing supported accounting choices.

The original sealed cursor reaches Ready before immutable historical selection.
A Message must match its retained source event, generation, generation revision
and exact raw request. A Finish must additionally match the named Message's
accepted Executed receipt, resulting target version and complete cumulative
prefix. These checks reuse the live historical validator. An earlier recorded
request remains valid after later native work; an unsubmitted result, changed
recipe or missing request cannot become a new publication through this mode.
Historical selection itself changes no canonical or staging bytes.

Authenticated Stop is bound before replay and checked at every event, at Ready
and again immediately before the sole Fence. It takes the original terminal path
before any ordinary role issue when already pending. After recovery, the same
Stop service precedes every evaluation quantum. One finite evaluator accepts
the existing `evaluator-case` and `evaluator-assess` operations on retained
cases; its independent credentials, candidate/I/O/byte limits and original
invocation deadline remain in force.

No actor socket or human-review listener is bound. The command creates no
publication reference, does not resume generation, read current source evidence,
invoke helpers, reconcile an uncertain effect or retry dispatch. The actor
profile selects the original request and audience; the reviewer profile supplies
the existing Stop credentials. Operator/precheck services and new-message
continuation are excluded. Independent labels cannot provide a human approval
key, reset numerical work or refund an unknown effect.

Evaluator authority retires before final archive capture. The new output retains
acknowledged labels and any acknowledged Stop from that same owner. A startup
failure after the recovery Fence attempts to retain its exact healthy successor;
the input archive remains unchanged. Existing typed evaluator responses carry
the observations and assessment receipts; this command adds no output schema.

Four service and two core regression tests are authored, including real delayed
Message/Hold/Finish labels, old cases after later work, wrong-recipe/request
refusals, authenticated Ready Stop and expiry, and unchanged staged bytes before
fencing. Source review is complete; formatter, compilation and runtime execution
were unavailable in this session.

## Create an evaluated learned service

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE \
  --actor-profile ACTOR_PROFILE \
  --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --retain-history-anchor /absolute/independent/new-learned-anchor
```

This is fresh single-request creation with the original learned numerical recipe,
native or explicitly selected process helpers, actor ingress and independent
human reviewer. Plain and simple checked publication are supported. Both
accounting and evaluator profiles, the actor profile and a new independent
history output are required together. Select the marginal
`fa.native-andon-operator/1` accounting profile with its unchanged initial
windows and evaluation revision zero.

The [atomic learned constructor](LEARNED_ANDON_BOOTSTRAP.md) installs the exact
recipe, complete guards, evaluation, Andon accounting and terminal recovery
reserve before its sole first-image publication. The separately held evaluator
stays with that same owner. Role separation, audience, profile/source/witness
paths, endpoint aliases and retained-history custody are checked before clocks,
listeners or owner work. No provisional unaccounted store is created.

**Evaluation is scheduled after the original request workflow.** The evaluator
socket is bound before generation, and its original finite lifetime includes
generation, helper review and human review. Case discovery and assessment are
served only after those operations end, using the remaining allowance. If the
allowance expires during the original work, the case remains pending. A final
native Hold can be discovered and independently labeled without rerunning
generation or helpers. A benign Hold charges the original Andon window; a label
cannot approve publication, resample output or release a held effect.

The existing `evaluator-case` and `evaluator-assess` commands below use the same
independently authenticated endpoint. A discovery and assessment consume two
original candidate slots. Stop precedes every evaluation quantum, including
after an optional operator command. A shorter deadline accepted from the actor
also narrows the evaluator's own assessment-time clock check. Logical/wall time,
candidate, I/O and byte allowances never renew. A failed actor reply during this
final phase retires that transport and retains its error while independent
evaluation can use its remaining allowance. Earlier workflow failure, original
Stop or storage failure ends new evaluation. Evaluator authority retires before
terminal actor-reply grace and retained-anchor capture, so the anchor includes
acknowledged labels and accounting from the same owner.

Optional `--precheck-profile` and paired `--operator-profile` /
`--tower-generation` preserve their existing contracts. A separately installed
operator Ledger grant can read accounting and calibration Findings. Required
raw/owned predictors, held-out joint recipes, joint accounting, stronger witness
compositions, topology, campaigns, checkpoints and credibility activation refuse
this single-request opt-in. Explicit single-request continuation uses the complete
anchored recovery composition below. Fresh resident learned sequences use their
separate composition. An accounted learned journal cannot use the narrower
evaluated-only recovery wrapper.

Six substantive learned-service regression functions use the original numerical
and helper paths with an independent evaluator subprocess: human approval and
rejection, final Hold accounting, Stop over an incomplete frame, actual outer
service orchestration, grammar refusal and pre-clock custody/role refusal. They
check original draw counts, unchanged source ownership, complete endpoint
retirement and a final anchor including the assessment. A separate regression
checks deadline narrowing without a Time event, label or budget renewal. The
test-only accounting harness retains the original live owner for inspection;
it does not recover or replace it. Qualified scoped parsing/format and whitespace
checks passed; RCH compilation, runtime tests, Clippy and canonical qualification
remain unexecuted because the runner is unavailable.

## Continue an evaluated learned service

The same paired profiles support an explicit restart of an existing accounted
learned owner. Supply the independently retained history anchor and a new output
in addition to the actor, reviewer, accounting and evaluator profiles:

```text
supervise_publication create-learned-generated CONFIG NEXT_RECIPE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST --actor-profile ACTOR_PROFILE \
  --andon CURRENT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --history-anchor RETAINED_INPUT --retain-history-anchor NEW_OUTPUT
```

Select exactly one original continuation mode:

| Mode | Original work admitted after recovery |
| --- | --- |
| `--continue-generation` | Complete the same unfinished numerical generation before its original request; retained samples are not drawn again. |
| `--after-message PREVIOUS_REQUEST` | Admit a new request only after the named original Message has a confirmed native receipt. |
| `--finish-after-message PREVIOUS_REQUEST` | Admit Finish against the confirmed original Message history, with a fresh helper review and human decision. |

The marginal accounting profile supplies the original deployment/evaluator and
initial windows, **all** current windows, and an independently retained minimum
evaluation revision. The original numerical recipe, complete learned guards,
current policy/floors, simple publication requirements and exact history prefix
must also match. These expectations come from the supplied profiles and archive;
recovery never learns a more permissive contract from the journal being checked.
Input/output archives remain distinct, and evaluator custody covers the retained
input alongside recursive identity, credential, witness and role inputs.

The [complete native cursor](LEARNED_ANDON_RECOVERY.md) replays one original event
at a time. The same invocation deadline and authenticated Stop intake are checked
before open, at every event and separately at Ready. A pending authenticated Stop
uses the original admission Stop/Fence before ordinary fencing or any live role
release. It retains the acknowledged terminal anchor and original liabilities;
its receipt does not claim that effects drained. A refused checkpoint or contract
leaves canonical and staged bytes unchanged.

Ordinary completion acknowledges one native recovery fence, then provisions fresh
separate roles. The command captures that successor before request/predecessor
admission, so a later startup refusal can still retain the acknowledged cut.
Numerical state starts paused, source eligibility and old approvals remain
withdrawn, and the original fresh time/source, helper and human checks govern new
publication. Unknown effects remain charged and are not automatically retried.

Each explicitly selected process invocation has its own finite transport and
evaluation lifetime. It does not revive the previous invocation's roles or unused
service allowance, reset numerical work, or replace cumulative native accounting.
Within the new invocation, replay, generation and review consume its original
deadline; accepted request deadlines can narrow it. Post-review evaluation uses
only the remaining allowance and can label still-pending historical cases on the
same recovered owner. It cannot rerun generation/review or supply an effect key.
The new output anchor includes acknowledged late labels and Andon changes.

Receipt-only/effect-retry modes, offline terminal modes,
predictors, joint accounting, topology, campaigns, checkpoints and credibility
activation remain outside this paired live recovery grammar. The resident
service also accepts an explicitly anchored remaining sequence under the
complete accounting and evaluation requirements described below.

The new `learned/tests/evaluation/recovery.rs` sources exercise actual partial
generation continuation, recovered Message and Finish, deferred historical labels,
fresh human rejection, retained cuts on invalid predecessors, contract/floor
refusals, every replay deadline and authenticated Stop before live role release.
They are authored regressions with independent source review. Required RCH
compilation, execution, Clippy and canonical qualification remain pending.

## Evaluate recorded learned history

Independent assessments may arrive after the original generation and publication
service has ended. Select an evaluation-only invocation explicitly:

```text
supervise_publication create-learned-generated CONFIG ORIGINAL_RECIPE REVIEWER_PROFILE \
  --evaluate-only --actor-profile ORIGINAL_ACTOR_PROFILE \
  --andon CURRENT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --history-anchor RETAINED_INPUT --retain-history-anchor NEW_OUTPUT
```

The recipe and actor profile identify an **already recorded** original Message.
To bind an already recorded Finish instead, also supply
`--finish-after-message PREVIOUS_REQUEST`; the selected recipe and actor request
must match that Finish's original historical binding. A missing request refuses
before the recovery Fence. This mode cannot create the Message or Finish being
selected, resume an interrupted generation, or reconcile or resend an effect.

The same [accounted learned recovery](LEARNED_ANDON_RECOVERY.md) verifies the
independent input anchor, original numerical recipe and model, complete current
guards, policy and counter floors, evaluation protocol, accounting identity and
all current windows. The marginal accounting profile and plain or simple checked
publication remain supported. Original model/codec inputs must still be available
for verified replay; current source evidence, helper-model assets and human
approval inputs are not opened for assessment. An evaluator's label does not
relax those requirements for any later publication.

Only the independent evaluator and authenticated Stop endpoints are served.
The actor profile supplies original audience, request identity and finite service
limits; its socket is never bound. The reviewer profile supplies the existing
Stop credentials; no approval session is opened. `--continue-generation`,
`--after-message`, operator/precheck service options, topology, campaigns,
predictors, stronger qualification and checkpoint compositions are excluded.

Stop and the invocation deadline are checked at every replay boundary, at Ready
and again after historical selection before the sole Fence. One fresh finite
assessment transport then accepts the existing `evaluator-case` and
`evaluator-assess` operations against retained cases. A discovery consumes one
candidate and its later assessment another. The new invocation does not reset
original numerical work, liabilities, pending cases or Andon costs. Unknown
effects remain charged. Stop precedes every assessment quantum, and no queued
assessment can follow evaluator retirement or the retained final anchor.

The command emits no new publication reference. Use the existing typed evaluator
case and assessment replies to observe results. The new independent output anchor
includes all acknowledged assessments and any acknowledged Stop. Input custody
remains unchanged. A post-Fence startup failure retains the acknowledged recovery
cut whenever the original healthy owner still supplies it.

The implementation and positive/causal regression sources are source reviewed.
They have not been compiled or executed in this session; required remote RCH
qualification remains pending.

## Create an evaluated resident learned service

Fresh `serve-learned-generated` can select the same paired accounting/evaluator
profiles and required new history output for a complete finite Message/Finish
sequence. Its original evaluator survives handoffs under one cumulative allowance;
bounded inter-request reply windows and the final case phase service that same
role. The accepted final request deadline cannot renew or outlive the original
invocation lease. Final Hold and human rejection remain labelable without
restarting numerical or helper work. A late intermediate actor reply failure
ends succession but preserves the healthy evaluator's independent opportunity.
The resident command also supports `--continue-generation` and `--after-message`
with a retained input anchor, a new output, the current accounting profile and
a remaining sequence. One recovery fence precedes the entire suffix.
See the full [resident learned contract](RESIDENT_LEARNED_PUBLICATION.md#independent-evaluation-and-andon-accounting)
for the closed supported grammar, complete future input/endpoint custody,
Stop precedence and source-only verification boundary.

## Discover the original case before labeling

### Discover pending cases without a round ID

Use the same independent evaluator endpoint to find original reviews that still
need a final assessment, including retained history reopened with
`--evaluate-only`:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  evaluator-case EVALUATOR_PROFILE --pending
```

The response contains at most 16 original applied cases in ascending round
order. Each entry carries the same minimal case identity as exact-round lookup
and a status of `unassessed` or `censored`. Final benign/violation labels are
excluded. Older policy generations remain discoverable. `total_pending` counts
the complete retained applied pending/censored inventory; `unfinished_reviews`
separately counts reviews that have no successfully applied case and cannot yet
be labeled. An empty applied inventory does not say unfinished reviews passed.

The `fa.native-evaluator-pending-request/1` request binds the original audience
and optional continuation. The `fa.live-evaluator-pending/1` response wraps the
page in `Knowledge` with one immutable original-owner basis. Identity fields,
counts and revision coordinates are canonical decimal strings; a fresh empty
inventory may have evaluation revision zero. Pending requests have a 2 KiB
limit so the complete echoed request and sixteen descriptors fit the existing
24 KiB response bound. No request enlarges the native 512-case inventory.

If `knowledge.value.next_after_round` is non-null, explicitly request the next
page using it and `knowledge.basis.evaluation_revision`:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  evaluator-case EVALUATOR_PROFILE --pending \
  --evaluation-revision REVISION --after-round ROUND
```

Both cursor options are required together in that order and take nonzero
canonical decimal integers. `complete:true` closes only the applied pending-case
suffix strictly after the requested boundary at the reported evaluation cut.
It does not authenticate an external history, close unfinished reviews or make
claims about future cases. The original owner supplies the enumeration; wire
checks validate the declared page and its request binding.

A new applied review or changed assessment invalidates an older continuation,
including changes to a round before its boundary. The response then reports
explicit `Unknown` with no invented current basis or cases; start a new discovery
explicitly. A clock-only event, exact assessment retry or newly started but
unapplied review does not change the applied-case revision. Each page and each
later assessment spends a separate candidate from the original finite service
allowance. The default client returns one page. Explicit bounded traversal is
available below; it never retries, restarts or labels automatically.

Pending discovery pins only the applied evaluation inventory, unlike `--list`,
which pins both the journal and evaluation revisions across all retained cases.
Every pending page returns its current journal, control and authority basis;
clock-only changes between pages therefore do not imply a single journal cut.

Use a returned case identity and evaluation revision to prepare the existing
independent assessment. Origin, evidence and truth still come from the
independent evaluator. Discovery does not reserve that revision, supply a native
ticket or approve an effect. Original owner/fault checks, kernel credentials,
EOF sealing, Stop priority and finite I/O/byte/deadline budgets apply unchanged.
No source refresh, native clock event, generation, helper work or journal write
is performed by discovery.

### Read one exact original case

The same independently configured evaluator can read one exact applied case
without reopening the live journal:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  evaluator-case EVALUATOR_PROFILE 101
```

The round argument is a nonzero canonical decimal integer. This uses the same
original endpoint, evaluator role, kernel credential checks, lifetime and finite
candidate/I/O/byte allowances as `evaluator-assess`. A read and a later label
therefore require separate candidates from the existing allowance. Reconnecting
or repeating a read does not replenish it. No additional profile or grant is
created.

A `fa.native-evaluator-case-request/1` binds the complete selected audience and
round. The validated `fa.live-evaluator-case/1` response binds that exact request
and returns `Knowledge`. A Known value contains only the original round, attempt,
policy generation, congress generation and evidence-root identity. Its one native
basis supplies journal revision, global evaluation revision, control sequence and
authority epoch from the same immutable owner borrow. These coordinates and case
identities are canonical decimal strings; authority epoch zero is valid.

Use `knowledge.value` to bind the later assessment and
`knowledge.basis.evaluation_revision` as its expected evaluation predecessor.
The existing assessment document still takes unsigned integer literals for
numeric fields. Preserve the full integer values when preparing it. Independently
supply the origin, evidence identity and truth; discovery provides none of them.
A recorded review itself advances the global evaluation revision, as does a
new assessment. Another update between discovery and labeling can therefore
make this predecessor stale. Explicitly rediscover before preparing a new label;
the client does not retry, relabel or automatically assess a case.

Unknown `case_not_recorded` has no basis or case value. The client prints that
explicit observation and returns an error. It is not a claim of absence over an
entire history. A foreign, recovered-old or storage-faulted custodian cannot
disclose a case or basis. Stopped or retired services cannot admit a new read.
Partial frames remain sealed until EOF and still yield to the independent Stop
checkpoint.

A read obtains no source, clock, replay, helper, human approval, native ticket,
label or effect authority. It leaves the complete durable image unchanged.
Member verdicts, input/payload bytes, individual labels and the noncloneable role
are not serialized. The exact request, closed result shape and
`effect_authority:false` are validated before client output. This validates the
declared authenticated observation, not the empirical truth of a later label.

The command is registered as an L8 read adapter. Generic actor/operator Tower
requests remain unsupported; knowing a case identifier does not supply the
separate evaluator role.

### List retained cases without known round IDs

An evaluator that does not already know the historical round IDs can discover
them through the same endpoint:

```text
supervise_publication evaluator-case EVALUATOR_PROFILE --list 8
```

`LIMIT` is an explicit integer from 1 through 16. A successful page contains the
original minimal case identities and their `pending`, `censored` or `resolved`
assessment progress. `resolved` does not expose the actual truth label. The page
also contains `total_cases`, `remaining_cases`, `unapplied_rounds` and
`next_after`. `total_cases` counts all retained applied cases across policy
generations; `remaining_cases` counts those strictly after the selected cursor,
including the current page. Begun or unsuccessfully applied reviews remain in
the separate unapplied count. Even an empty applied page may therefore have
unfinished work.

When `next_after` is nonnull, continue with that round and the exact journal and
evaluation revisions from `knowledge.basis`, in this order:

```text
supervise_publication evaluator-case EVALUATOR_PROFILE --list 8 \
  --after ROUND --at-journal JOURNAL_REVISION --at-evaluation EVALUATION_REVISION
```

All three continuation options are required together. IDs and counts use
canonical decimal spelling. The journal revision and round are positive;
evaluation revision zero is valid for an empty initial ledger. Listing copies at
most 16 summaries and counts at most the original 512-case inventory. It does
not calculate a full credibility report, read source evidence, sample Time,
replay history or append any journal event.

Each page pins one exact acknowledged history cut. A new label, new review or
Time-only event invalidates a continuation. `Unknown{stale_basis}` and
`Unknown{cursor_not_recorded}` contain no partial page or substituted current
basis. The client prints the explicit Unknown and returns an error; it never
silently restarts the enumeration. Explicitly start a new initial listing when
the history changes. For labeling, use a currently observed evaluation revision;
earlier pages do not reserve an assessment predecessor.

The additive `fa.native-evaluator-cases-request/1` and
`fa.live-evaluator-cases/1` formats keep the existing `evaluator-case` verb and
socket. The listing request has a 2 KiB limit so its complete echoed frame and
16 descriptors fit the unchanged 24 KiB response bound. A dedicated bounded
parser leaves the exact-case and assessment grammars unchanged. Before output,
the client verifies the complete request, closed schema, exact revision pair,
case identities, ascending unique rounds, domain counts, status and continuation
cardinality. In particular, a nonterminal page cannot silently omit its cursor.

A listing consumes one candidate from the same original evaluator allowance.
Continuation, exact-case reads and assessments consume their own subsequent
candidates; reconnecting or paging does not refill any candidate, byte, I/O or
lifetime budget. Both peers still authenticate before disclosing bytes. A
partial request waits for EOF, yields to Stop and cannot disclose a page.

Fifteen transport regression functions cover strict schemas, maximal pages,
authenticated list/continue/read/assess flow, finite shared allowances, stale
history, private-data exclusions, sealed frames and client validation before
output. They are authored tests; required remote compilation and execution
remain pending.

### Traverse an explicitly bounded inventory

Opt into following the original continuation with `--max-pages`:

```sh
supervise_publication evaluator-case EVALUATOR_PROFILE --pending --max-pages 32
supervise_publication evaluator-case EVALUATOR_PROFILE --list 16 --max-pages 32
```

Pending traversal accepts 1 through 32 pages, enough for the original 512-case
inventory at sixteen cases per page. All-case traversal accepts 1 through 512
pages because its independently selected `LIMIT` may be one. These are ceilings,
not requests to issue every possible page: the first validated terminal page
ends discovery. A zero, noncanonical or excessive bound refuses before reading
the evaluator profile. Bounded traversal begins a fresh inventory and cannot be
combined with supplied manual cursors, exact-round reads or assessment commands.
Omitting `--max-pages` preserves the one-page interface.

Each accepted response is emitted unchanged as one JSON line and flushed before
another connection is attempted. A consumer must require successful command exit
as well as a terminal page. If a late output/deadline check fails after a terminal
page was flushed, the command still reports an incomplete operation. There is no
synthetic completion summary, silent truncation, automatic restart or assessment.

The client retains one profile, one elapsed-time origin, one cumulative work
counter and a finite aggregate byte/candidate allowance for the whole traversal.
Every page also spends the original live service's candidate allowance. Earlier
use of that service may therefore stop discovery before the requested page cap.
The first observed supervisor kernel UID/GID/PID is pinned; every reconnect must
authenticate as that same peer before receiving continuation bytes. This binds
the observed connection identity, not the server's executable or assessment truth.

Pending traversal preserves the initial evaluation revision and total pending
count, checks increasing round boundaries and exact cumulative cardinality, and
refuses backward journal/control/authority coordinates. It permits unrelated
Time or unfinished-review changes under the pending contract, retaining each
page's fresh acknowledged basis. All-case traversal fixes both original revisions,
the total cases and the unapplied-review count; even a Time-only journal change
makes that traversal stale. Cross-page count checks reject apparently valid
individual pages that skip cases, alter totals or falsely end the inventory.

The client returns an explicit incomplete error on Unknown, changed history,
peer/authentication or transport failure, failed output, original deadline/work
exhaustion, candidate exhaustion or an outstanding continuation at the selected
page cap. Previously flushed pages remain available. The next attempt requires
an explicit operator decision and a fresh or manual command; the client never
substitutes a newer cut. Original Stop remains authoritative in the service.

Deadline checks surround connection, transport, validation and output work and
use the same original deadline. Existing blocking connection and output APIs
are checked for lateness; this does not add hard real-time syscall cancellation.
Discovery still performs no native Time, source acquisition, generation,
assessment, policy change or effect.

Sixteen additional regression definitions pair actual native multi-page service
flows with stale/changed histories, output failure, Stop, finite client and
service allowances, per-connection authentication and cross-page wire mutations
that remain valid in isolation. Required Rust/RCH execution remains pending.

## Assess a completed review while service continues

Run this command as the separately configured evaluator process:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON
```

`EVALUATOR_PROFILE` uses the existing `fa.native-evaluator-peer/1` schema. Its
exact publication scope and deployment/evaluator identity must match the native
accounting configuration. Its supervisor credentials must match the original
supervisor. Its evaluator credential policy must be disjoint from the actor,
reviewer/Stop and operator policies. Different exact PIDs can distinguish
processes sharing UID/GID; a wildcard PID cannot establish that distinction.

Both ends verify Linux kernel peer credentials before disclosing assessment
bytes. This identifies local processes; it does not establish institutional
independence, executable provenance or remote cryptographic attestation. The
independently owned socket directory, endpoint identity and existing native
evaluator role remain required.

`ASSESSMENT_JSON` is the same bounded, closed `fa.native-assessment/1` document
used by the offline evaluator. It names the original round, attempt, policy and
congress generations, exact case evidence root, expected global evaluation
revision, independent origin, evidence ID and benign/violation/censored truth.
Use the original case discovery above to obtain these exact identities and
predecessor; the public Ledger deliberately does not expose individual cases,
raw labels or evidence.

The server obtains the ticket from its current native owner and compares the
whole case identity. A new assessment requires the exact evaluation predecessor.
A new benign label first records fresh native Time, then applies the original
accounting law. A later failure does not undo acknowledged Time. Exact native
retries use the original assessment history and add no new clock event, label,
false-stop charge or credibility credit. Same-origin deduplication, delayed and
censored outcomes, wide costs and calibration incidents remain native behavior.
Budget exhaustion cannot loosen a review requirement or remove a helper.

The client validates the full `fa.live-evaluator-receipt/1` before output. The
receipt binds the complete request and original before/after journal, evaluation,
control and authority revisions. Assessment may not change control or authority.
`applied`, `already_applied`, `refused` and `unconfirmed` are distinct. A healthy
Time-only predecessor can accompany a refused label; a storage fault cannot be
reported from stale RAM. A live receipt claims neither a recovery Fence nor a
retained independent anchor. A lost response does not cause automatic retry.

The command is registered as an L8 mutation adapter. Generic actor/operator
Tower requests for `evaluator-assess` remain unsupported: the label path requires
the separate authenticated endpoint and its original noncloneable evaluator.

## Original native evaluator custody

Native callers can use `FileIndependentEvaluator::evaluation_case(&host, round)`
to obtain the original applied case through the separately retained evaluator
role. The accessor and `check_owner(&host)` check the private original issuer
and healthy storage before disclosure. Equal numeric scope or case IDs cannot
transfer custody; recovery does not restore an old role. The read does not
observe Time, label a case or change the journal.

`pending_evaluation_cases(&host, None)` returns the initial bounded inventory
page. Continue with `Some((page.evaluation_revision(), next_after_round))`.
The sealed page contains read-only minimal case observations, total pending and
unfinished counts, and an optional next boundary. It contains no evaluator role,
member verdicts, assessment origins/evidence, human key or native ticket. The
same original-issuer and healthy-owner checks precede every page, including a
stale or malformed continuation.

The shared assessment adapter performs the same check before case lookup or a
benign assessment's Time event. A foreign role therefore cannot partially
timestamp a replacement owner before the final native label guard refuses it.

### Bounded native case discovery

`FileIndependentEvaluator::evaluation_cases(&host, after_round, expected, limit)`
discovers original applied cases without requiring their round IDs in advance.
An initial page uses `None, None`; `limit` is explicitly selected from 1 through
16. The sealed `FileEvaluationCasePage` returns minimal case identities in
ascending round order, the count of all retained applied cases, the separate
count of begun but unapplied rounds, and `remaining_cases` after the selected
cursor. `next_after` appears only when that remaining count exceeds the returned
page. The applied-case domain spans retained policy generations.

Each identity has `Pending`, `Censored` or `Resolved` assessment progress.
`Resolved` does not reveal whether the independent label is benign or a
violation. Labels, origins, independent evidence, helper verdicts and native
tickets are excluded. A begun or unsuccessfully applied review contributes to
the unapplied count rather than becoming a labelable case. An empty applied
page therefore does not imply that every begun review has an independent label.

Continue with the prior page's `next_after` and its exact
`FileEvaluationCasesBasis { journal_revision, evaluation_revision }`. Both
coordinates must still match, and the cursor must name an actual applied case.
New reviews, labels and even Time-only journal events make a continuation stale;
an inserted lower round ID cannot silently disappear between pages. A caller
may explicitly start a new enumeration but receives no automatic retry or
reserved labeling predecessor.

The original evaluator brand and storage health are checked before disclosure.
Each page uses a bounded ordered-map range rather than constructing a full
credibility report or copying raw verdicts. Counting the remaining domain scans
at most the original 512-case bound; copying is limited to 16 summaries. There is
no journal format change, source access, clock observation, numerical work or
authority transition.
Recovery retains the original cases but requires a fresh independently issued
evaluator role; the former owner's handle cannot read the recovered owner.

The native regression tests in `crates/fa-reference/tests/file_evaluation_cases.rs`
exercise pagination, assessment progress, unfinished reviews, stale cursors and
owner/storage boundaries. Compilation and execution remain pending the required
remote RCH gate; authored tests are not execution evidence.

## Lifetime, Stop and custody

Ordinary scheduled creation and guarded recovery retain the evaluator after
the final request workflow, including a final helper Hold. An evaluator may
wait for the actor's actual terminal response, then discover and assess that
original case using its remaining allowance. This final phase does not select
another request, reacquire source evidence, launch helpers or retry publication.
Original Stop runs first, and an operator Fence is checked before any label.
An actor reply failure retires only the actor transport and preserves its error
while the separate evaluator can finish its remaining opportunity. Earlier
workflow failure, unhealthy storage, acknowledged Stop or deadline exhaustion
ends new assessment work. Evaluator intake closes before reply-only grace and
final history retention; the retained anchor includes acknowledged late labels.

One evaluator service spans the whole finite request schedule. Reconnecting or
moving to the next request does not issue another role or replenish candidate,
I/O, byte or time allowances. A complete request requires EOF; a prefix or quiet
socket cannot attach a label. Malformed and credential-refused candidates spend
their original allowance. Each nonblocking turn has bounded calls and bytes and
runs after the original independent Stop checkpoint. The owner deadline remains
an outer limit. Terminal reply grace can drain an already entered response but
cannot accept another assessment.

All current and future reviewer/Stop endpoints, actor/operator/evaluator sockets,
source and witness inputs, profile files, retained archives and authority-store
paths are checked for collisions before owner work. Precheck schedules retain
their original input/disclosure reservations. No listener is silently replaced.

## Recover the same accounted owner

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --tower-generation 41 --requests 7,8 \
  --andon ANDON_PROFILE --guarded RECOVERY_PROFILE \
  --operator-profile OPERATOR_PROFILE \
  --evaluator-profile EVALUATOR_PROFILE
```

Recovery requires an explicit request schedule and the original guarded-submit
profile with independently retained history, minimum floors and a new
`retain_history_anchor` destination. Do not use the create-only retention flag.
The complete native protocol, window inventory, evaluation floor, source guards,
optional joint policy and checked-publication requirements are replay-checked
before returning an owner. The original Fence/Time/Sweep reconciliation does not
resend uncertain effects or refund charges. A pending independent Stop transfers
the same verified recovery cursor through the existing terminal Stop path before
an ordinary Fence or fresh evaluator role escapes.

Historical-only schedules can serve exact receipts without source/helper work.
New requests still require fresh source, helpers and human approval. The checked
open variant accepts the corresponding simple witness profile. This ordinary
recovery command does not select native/learned generated modes, topology, policy
governor, standalone `--precheck-profile` or credibility-activation capsules.
Native generated creation uses the separate composition documented above.

## Evaluate recorded ordinary Tower history

Use this explicit mode when independent labels arrive after an ordinary accounted
publication service has exited:

```text
supervise_publication serve-open CONFIG ORIGINAL_ACTOR_PROFILE REVIEWER_PROFILE \
  --tower-generation 41 --requests 7,8 --evaluate-only \
  --andon CURRENT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --guarded RECOVERY_PROFILE
```

For the corresponding simple checked owner, use `serve-open-checked` and its
original `WITNESS_PROFILE` positional argument. The guarded v1 profile must select
an independently retained input history, explicit journal/control/authority floors
and a new `retain_history_anchor` output. The Andon profile must supply the original
evaluator protocol and accounting identity, complete current window inventory and
independent evaluation revision floor. Original marginal or joint accounting and
simple checked publication keep their exact existing contracts. No contract is
inferred from the journal being checked.

The original actor profile selects the request audience and first request; the
finite unique `--requests` list must consist entirely of recorded original IDs.
The sealed original cursor verifies the complete history before each ID is read
through `recorded_request`. Held, denied and uncertain requests remain their
original historical statuses. A missing ID refuses before the Fence and cannot
turn into a new proposal. The evaluator retains its original independently
selected run-scoped protocol and credential audience; this list is a historical
startup check, not a newly granted publication schedule.

Only the independent evaluator and original authenticated Stop listeners are
opened. The actor and human-review paths are reserved against custody collisions
but never bound. Current source evidence and helper executable contents are not
needed. Recovery performs one original Fence without the live service's Time/Sweep
reconciliation. The command does not select another actor request, refresh source
evidence, invoke helpers, publish, or retry an uncertain effect. Original charges,
pending cases and accounting survive; a label cannot provide a publication key or
refund an Unknown outcome.

Stop is checked before each replay event, at Ready and again after request
selection immediately before fencing. A pending Stop uses the same original
terminal recovery path before fresh roles escape. After recovery, every evaluator
quantum follows the original Stop checkpoint under one finite invocation deadline
and the evaluator's original candidate, byte and I/O allowances. Reconnection does
not replenish them. An authenticated new Stop still invokes its explicit original
terminal handling. An owner whose history already records Stop is refused with
its acknowledged successor retained; evaluation-only startup does not resume an
old cleanup sweep.

The existing `evaluator-case` and `evaluator-assess` commands return the original
typed observations and assessment receipts. The evaluator retires before the new
history archive is captured, including acknowledged labels or terminal Stop.
A startup failure after the Fence attempts to retain the exact healthy successor,
so correcting a listener conflict does not require rollback to an older anchor.
The input archive is preserved. Output, source, configuration and every selected
current or future endpoint must have distinct custody before recovery begins.

The flag is unary and belongs only to the ordinary open grammar above. Creation,
missing accounting/evaluator/recovery selections, duplicate flags, operator or
precheck endpoints, topology, governance and credibility activation are rejected.
Native and learned generated owners retain their separately documented historical
modes. The new service and command regression sources cover delayed original
labels, finite allowances, history and counter mismatches, Stop and deadline
boundaries, and custody races. Required RCH compilation and runtime qualification
remain pending.

## Verification boundary

The generated composition adds five regression functions and two child-process
harness entrypoints. A distinct evaluator PID uses the actual case/assessment
clients after the actor receives its terminal result, with only two original
candidate slots. The tests cover actual approved/rejected publication, benign
labeling and Andon exhaustion after a real helper Hold, original Stop against an
incomplete authenticated frame, and admission refusals before clock/store work.
The original evidence file is removed before late discovery and labeling. These
new functions have received independent source review; their compilation,
formatting and runtime execution remain unverified because RCH is unavailable.

Authored regressions cover actual scheduled publication and anchored recovery,
l