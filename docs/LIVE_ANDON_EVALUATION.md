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
Ledger. Recovery selects `--after-message`, an independent input anchor and the
same explicit evaluation recovery requirements used below; one fresh evaluator
serves the recovered suffix without renewing its allowance at handoffs. Joint
profiles, governance, topology and selected prechecks remain separate. Required
RCH compilation and runtime qualification remain pending.

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

The shared assessment adapter performs the same check before case lookup or a
benign assessment's Time event. A foreign role therefore cannot partially
timestamp a replacement owner before the final native label guard refuses it.

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
live native assessments and Ledger observations, original later two-key
publication, wrong case/audience/revision, exact retry, finite reconnects,
partial frames, Stop, lost replies, uncertain persistence, receipt tampering,
source independence and custody collisions. These are test definitions, not
executed evidence. Scoped syntax/format checks and independent source review
were performed. Compilation, runtime tests, Clippy and the canonical gate remain
pending because required remote-only RCH is unavailable. No packet or production
qualification is closed by this integration.
