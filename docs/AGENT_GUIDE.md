# Agent guide: operating FrankenAlignment from the driver's seat

**Who this is for.** An AI agent (or a human using the same surface) that operates, investigates or supervises a FrankenAlignment deployment: the operator agent, not the controlled actor. Developer conventions are in [`AGENTS.md`](../AGENTS.md); the one-page model is [`docs/SYSTEM_MAP.md`](SYSTEM_MAP.md). The `fa ...` commands describe the prospective surface of plan §17. Implemented reference adapters are identified explicitly below; they do not establish the complete deployment surface.

**The contract you can rely on.** Every value you receive carries its epistemic status; every object has an address; every mutation is idempotent with an expected predecessor; every rehearsal is labeled and can never mint a permit; every error is a plan (it names what would change it). You never need to guess whether a number is measured, whether an absence is observed or merely unobserved, or whether a command changed the world.

## Implemented shared reference reads

The [bounded Tower Rust API](TOWER_REFERENCE.md) connects `get`, `related`,
`tail`, `situation`, `explain`, `why-held` and `next` to the existing reference
backends. Its JSON response includes the common sixteen-field envelope and
complete nested `Knowledge`, including original bases and uncertainty. The host
provisions its immutable snapshot, grants and read allowance. Global reports
require complete domain scope; operator-only routes and hidden evidence checks
remain enforced. Other registered verbs explicitly refuse without a side effect.

This is a scoped reference adapter, not a `fa` CLI or authenticated service.
The playbook commands below remain the prospective deployment interface unless
their own implementation notes identify an executable operator command.

The Tower's [paired replay adapter](TOWER_REPLAY.md) additionally connects the
registered `replay` verb to original observed-history verification and exact
policy counterfactuals. An integrating host installs the original source, an
independent pre-vote anchor, permitted interventions and a real `ExperimentCx`.
Agents select that branch artifact with an exact request key. Results preserve
both source and branch histories, invalidate old votes when inputs change and
require fresh independent review after repair. The independent `verify` backend
remains a separate open contract.

For this adapter, inspect `replay-plan` at the installed **branch artifact**
address before executing `replay`. The plan exposes the real host registration
and required grants, with source verification still Unknown. Operator status or
Experiment purpose is not required just to inspect an authorized plan; sensitive
native metadata is withheld from partial-domain callers. The generic closure
lookup and durable retention planning shown in the prospective playbook remain
outside this bounded profile.

The [durable publication Tower](TOWER_PUBLICATION.md) mounts one original
file-backed actor port. In this implemented profile, `propose` carries a complete
native proposal, exact policy epoch and canonical numeric request key; `get`
polls a request ticket acquired by that same session. Fresh polls preserve
Pending, Unknown and terminal outcomes through the common envelope. The native
supervisor performs all review and publication, and an exact retry after its
original recovery reacquires only the retained request observation. This profile
does not expose the prospective arbitrary-affordance or decision-card workflow.
If the host installs a same-owner exact-policy disclosure, the same profile
accepts `precheck` before submission. Its original source witness cache and
finite grant are shared across clones, and its pass still requires fresh intake
and review. Repeating a query label or envelope key does not create a free read
or a request ticket; owner transitions invalidate the old disclosure.

At that mounted run, `capabilities` with a nonzero query and expected native
policy epoch describes the explicitly granted precheck endpoint and its remaining
shared query allowance after charging the read. It preserves the original
informational response, cache selection and disclosure lifetime. It does not
implement the broader signed deployment assurance profile in the prospective
playbook below. Missing disclosure or owner eligibility remains explicit refusal.

A separately installed supervisor inspection grant connects native `situation`
for authenticated operators. It captures the original acknowledged owner with
its native basis, complete ledger accounting and bounded pages from that same
immutable capture. Copy the returned continuation for each subsequent page;
a new first-page capture replaces earlier progress. Unlisted attempts are
explicitly marked by `enumeration_complete: false`, including when the last
permitted query has no continuation. Unknown and
irrecoverably unknown liabilities remain unresolved and charged. Grant clones
share finite work and revocation. Ordinary actors and precheck users cannot
acquire this capability, and a lost owner never reuses a previous live report.

For the ordinary running Tower service, select an explicit
`--operator-profile OPERATOR_PROFILE` alongside `--tower-generation` and use
`supervise_publication tower-operator-exchange OPERATOR_PROFILE REQUEST_JSON`.
This [separate operator endpoint](LIVE_TOWER_OPERATOR.md) authenticates its own
peer and installs only the selected inspection and admission-explanation grants.
It keeps the original owner live through intake, review and terminal reply grace.
An explicit request schedule retains the same read session, cursors and finite
allowances across handoffs; a later request never renews spent operator work.
Explanation additionally requires explicit policy/selected-witness disclosure;
it remains historical admission evidence, not current `why-held` or permission.

To enumerate a live owner's original situation capture, use
`supervise_publication tower-situation OPERATOR_PROFILE RUN_ADDRESS --max-pages N`
with a canonical run address carrying its exact registration generation and an
explicit page allowance from 1 to 64. The client validates each native page and
follows its exact continuation within one authenticated connection and the
original allowances. Success requires explicit completion. On exhaustion or a
native diagnostic, valid earlier JSON lines remain output and the command fails;
check its exit status before treating the enumeration as complete. It never
recaptures automatically, and the result remains historical observation.

For a new topology-enforced publication, the two Tower create modes accept
`--topology-profile TOPOLOGY_PROFILE`, optionally with the separate operator
endpoint. The original initial cut certification and final topology refresh
remain mandatory alongside helper/human approval and any plain final witnesses.
This does not enable ordinary topology recovery, request schedules or precheck;
use the [exact supported composition](TOWER_PEER.md#preserve-topology-enforcement-through-tower-admission).

The integrating host can combine those original facades with the
[shared Tower router](TOWER_ROUTER.md), using the same `TowerRequest` for evidence
reads, precheck, native submission and acquired-request polling. A live mount
owns its address and native request namespace even after the owner closes; a
native refusal never falls back to historical snapshot data. Keep snapshot-domain
addresses distinct when their own `tail` or `situation` must remain accessible.
Branch-only routers retain isolated replay and cannot mount a production port.

These configured facades also accept the closed
[`fa.tower_request/1` JSON interface](TOWER_REQUEST.md) through `exchange`.
The host supplies authenticated grants separately; actor documents cannot
choose them. Use canonical decimal strings for every numeric argument and
lowercase hex for proposal/edit bytes. Complete syntax validation precedes
native queries, snapshot reads or mutation; then the same original dispatch,
retry, budget and disclosure rules apply.

The [live publication service and client](TOWER_PEER.md) now consume these
documents on Linux. Select an explicit `--tower-generation` for the ordinary
`supervise_publication` serve modes and use `tower-exchange` for a bounded
sequence of request files. The host supplies its original source/clock only
for a new selected proposal; authenticated peers retain tickets across reconnects
without replenishing their exchange allowance. The existing Stop service remains
live during partial input. Complete response output is an observation, and the
client performs no implicit polling, resubmission or effect-status promotion.

## The first ten minutes on an unfamiliar deployment

| Step | Command | What you learn | If it fails |
|---|---|---|---|
| 1 | `fa doctor` | Which layers' conformance suites executed and passed, bottom-up. Anything above a failed or unexecuted layer is `Unknown{LowerLayerUnverified}`. | Stop trusting anything above the failing layer; that is the deployment's real assurance level, whatever the profile claims. |
| 2 | `fa capabilities` | The assurance profile: per effect family, mediation level, capture coverage, replay grade, roster class, autonomy floor, policy disclosure. | A family missing from the profile is silent, not covered. |
| 3 | `fa situation --scope fleet` | Grades, fences, held and unknown effects, evidence debt, helper health, andon budget, incident counters, open risk-theater findings, ordered by decision relevance. | Items marked `Pending` name the frontier they wait on; `Withheld` items need scope, never inference. |
| 4 | `fa tail --since <seq-from-situation> --follow` | The ordered journal from the point the situation report was exact. | If the journal has a gap, the situation report says so; do not fill it from memory. |
| 5 | `fa related fa://<tenant>/handoff/latest` | The last agent's handoff: their situation, open affordances, annotations and rehearsals. | No handoff means you are first; write one when you stop. |

## Playbooks

Each playbook is a sequence of verbs from [`registry/vocabulary.json`](../registry/vocabulary.json). Steps name the addresses they consume and produce. Every playbook can be rehearsed end to end in a branch before it is run for real.

### P1. A proposal is held

1. `fa why-held <attempt>` → the unsatisfied predicate nodes and their affordances.
2. `fa next <attempt>` → affordances ranked by expected information gain per cost within the allowed action set. Hard floors are not in the list; nothing here can skip a mandatory observation.
3. Estimate committee or observation spend when a full round is offered: `fa cost <attempt>` → `CostEstimate`, then rehearse the round (`fa convene <attempt> --branch <b>` or `fa rehearse <affordance> --branch <b>`) or rehearse the top affordance (`fa propose <affordance> --branch <b>`) → `RehearsalOutcome` with the tree the nucleus would produce and the cost it would charge.
4. Execute: `fa convene <attempt>` when opening a live committee round, or `fa propose <affordance>` → the decision card (alternatives not chosen), realized cost, new control sequence.
5. Repeat from step 1 until the tree is satisfied (`Continue`) or an explicit consequence transition (`fa hold <attempt>`, `fa narrow <attempt>`, `fa suspend <run>`, or `fa reset <run>`) is final. A `SuspendRun` or `ResetToCheckpoint` is not a failure of the playbook; it is the outcome, and `fa reset` never rewinds the effect or budget ledger (FA-INV-034).
6. `fa annotate <closure> --class conclusion` with what resolved it, then `fa handoff <run>` if you stop here.

Failure branches: `BudgetExhausted` names the budget and the governance transition that could raise it (never raise it yourself to finish a hold); `StaleGeneration` means re-read the situation from the returned predecessor; `OutcomeUnknown` on a dispatched effect means reconcile (P3), never retry.

Implemented reference admission diagnosis: for a learned publication, use
`supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume --explain-proposal --history-anchor INPUT`.
It reconstructs the first exact-policy admission through the original guarded
numerical replay, including a denial before a helper round exists. Optional
`--payload` or `--search` uses immutable policy experiments; sufficient results
still require fresh independent review. This offline, privileged projection
requires the original recovery lock and exact v2 custody when topology is
selected. It does not implement the full dispatch/helper `why-held` conjunction.
See [learned diagnosis](LEARNED_PROPOSAL_DIAGNOSIS.md#learned-operator-command).

For an ordinary source-backed actor, the integrating supervisor can separately
provision the [exact-precheck peer and client](ACTOR_EXACT_PRECHECK.md#authenticated-precheck-process-transport).
The native Linux peer authenticates ingress; the actor-side one-shot client
authenticates the server and receives only a correlated exact observation.
Keep the disclosure custodian, source/clock observation and grant issuance with
the supervisor. The [ordinary live service's explicit precheck option](LIVE_ACTOR_PRECHECK.md)
wires that bootstrap to a separate endpoint and enforces the original source
lease; other service modes have no fallback. A passed precheck still requires
ordinary submission and review, not the privileged diagnosis above. Reconnects
and lost replies do not replenish the original allowances. This is a scoped
reference integration, not the complete prospective `fa` surface.

The actor can run `supervise_publication actor-precheck ACTOR_PROFILE
PRECHECK_PROFILE PRECHECK_JSON` using only its public profiles and proposal.
Consume the original Knowledge response: a zero exit requires a passed exact
precheck, still explicitly requiring review. Denials and other valid refusals
render their response with a nonzero exit; transport/output failures never cause
an automatic resend or an ordinary submission. A later Submit uses its original
separate contract and independently fresh source.

### P2. Investigating an incident

1. `fa get fa://…/incident/<id>` → the incident with its `Knowledge` status and the addresses of the closures, rounds, evidence and findings it references; `fa verify fa://…/receipt/<id>` confirms independent bundle and receipt integrity before trusting retained artifacts.
2. `fa explain <closure>` for each implicated closure → predicate trees; disagreement between helpers is localized first by comparing their view manifests (`fa related <view>`), never by assuming they saw the same document. If an upstream observation is confirmed corrupt or superseded, record an append-only invalidation with `fa invalidate <evidence>` rather than mutating history.
3. Inspect `fa replay-plan <closure>` to confirm whether `ReplayGrade` is `Exact`, `InterventionFaithful`, or `AuditOnly`; then open `fa branch --from <closure>` → a capability-free branch; `fa intervene --branch <b> --remove <evidence>` or `--substitute`, `--clamp`; `fa replay --branch <b>` → a `CounterfactualWitness` with its replay grade.
4. Include the controls: unchanged replay, sham intervention, lossless-versus-lossy checkpoint. A witness without controls is annotated as exploratory.
5. `fa annotate <incident> --class hypothesis|inference|conclusion` at each step; the trail is the deliverable.
6. If the incident implicates the guarded model's identity, check `fa get fa://…/passport/<id>` and the latest liveness observation before anything else; substitution changes every conclusion.

The actor gets no channel into this playbook. Its statements, if any, enter only as `ActorStatement` evidence with the untrusted label.

**Implemented independent accounting verification.** For a retained FARJ v1/v2
rights journal, run `xtask verify rights-journal --context EXPECTED_JSON
--journal HISTORY --anchor RETAINED_PREFIX`. The independently selected context
and distinct retained-prefix file are mandatory. The command works outside the
source checkout and reads both files without opening a live authority owner.
Consume the complete structured report, including its `Unknown` properties:
valid accounting does not establish authentication, latest-history completeness,
external occurrence or the full permit law. Any failure exits nonzero without
successful partial accounting. The
[bounded verifier contract](INDEPENDENT_RIGHTS_VERIFY.md#operator-command)
defines the context schema, file and work bounds, error coordinates and pending
execution status. This profile is separate from the prospective signed-receipt
step above and does not make the Tower's `verify` verb available.

**Implemented predictive-stream containment mapping.** The trusted supervisor
can select the original owned prompt-audit recipe and use
`create-learned-generated --prepare-stream-checkpoint` to retain a complete
prompt before any prediction or sample. The separately requested
`--narrow-stream-checkpoint` applies the registered `narrow` consequence to the
exact retained checkpoint and current predecessor. Its original receipt sequence
can identify the later incident frontier for `--begin-stream-reset` and
`--complete-stream-reset`. All commands require their full documented inputs;
the [predictive stream workflow](LEARNED_PREDICTIVE_STREAM_RESET.md) gives the
grammar and independently retained request/anchor chain. This containment
preserves policy, predictor, work, audience and charged liabilities. The
same [topology-protected workflow](LEARNED_MEDIATED_PREDICTIVE_STREAM_RESET.md)
retains exact source/history pairs at each step and withdraws the accepted cut
at narrowing; live continuation must freshly certify the current epoch without
renewing spent source allowances. The
supervisor's evidence binding does not authenticate a detector; any later
publication still requires the original native and human review.

### P3. An effect's outcome is unknown

1. `fa situation --scope run` lists it under unknown effects with its adapter's idempotency contract.
2. If the adapter is `RemoteIdempotent`: `fa resolve <attempt> --reconcile` re-queries under the same key. If `NonIdempotent`: `fa resolve <attempt> --disposition <ConfirmedNotExecuted|Confirmed>` requires an independently established outcome and the authority to record it. Selecting `IrrecoverablyUnknown` instead records a permanent unresolved liability; it never establishes whether the effect executed.
3. Never `propose` the same effect again to "make sure". Unknown and irrecoverably unknown effects retain their conservative charge; retention expiry alone never selects abandonment (plan §25.3).

**Implemented source-publication reference mapping.** The trusted supervisor may
explicitly record that final unresolved disposition with
`supervise_publication abandon-unknown CONFIG REQUEST_JSON --guarded RECOVERY_PROFILE`.
Supply the original attempt, an operation ID and independently selected control
sequence/authority epoch in the closed `fa.abandon-unknown/1` request. `CONFIG`
and the `fa.guarded-abandon/1` or `/2` recovery profile must select the complete
applicable guards and independently retained input history. The profile also
requires a new output archive outside the authority store. This consumes guarded recovery without returning a live
owner or advancing the predecessor through a Fence. It removes only the selected
sendable envelope and records `IrrecoverablyUnknown`; actor outcomes remain
Unknown, streams remain blocked, and Stop still lists the liability as unresolved
and irrecoverable. An empty pending sweep is not a drained domain.

Keep the exact original request after acknowledgment, including when anchor
retention or stdout fails. A retry with that same request returns its original
receipt without another event; select a new output archive for the retry. Do not
invent a new operation or resubmit the effect. See
[the durable abandonment contract](DURABLE_UNKNOWN_ABANDONMENT.md) for schemas,
profile limits and the pending execution-verification boundary.

### P4. A helper needs promotion, demotion or rotation

1. `fa ledger fa://…/helper/<generation>` → precision, recall, lead time, caps, andon consumption, all from independent-evaluator labels.
2. A selection change requires an evaluator report with attack-conditioned metrics (FA-INV-032). `fa promote|rotate <generation> --report <report>`; without the report the transition is refused, and that refusal is itself journaled.
3. New generations run in shadow first; `fa policy-replay --candidate <roster>` shows what the last N closures would have decided.
4. Check `fa findings --family roster` afterwards: the risk-theater detector will show a churn finding if the change was near a confirmed hold.

### P5. Widening an agent's autonomy

1. `fa ledger fa://…/grade/<principal>/<family>` → the actor's track record and dwell time.
2. `fa promote <grade> --report <evaluator-report>`; refused without report, dwell or governance authority (FA-INV-036). Demotion needs none of these and happens automatically.
3. Rehearse first: `fa promote … --branch <b>` shows which recent proposals would have crossed to a cheaper lane.

### P6. Changing policy

1. `fa policy-replay --candidate <policy>` → per family, what would newly hold, newly allow or change consequence, with utility and cost; relaxations list the newly allowed set explicitly.
2. If the candidate needs observations the retained closures lack, the campaign says `mode: shadow`; do not present shadow as replay.
3. `fa promote <policy> --campaign <artifact>`; refused without the campaign artifact.

The executable reference mapping is
[`supervise_publication policy-replay`](POLICY_REPLAY_OPERATOR.md).
For learned-model histories use the documented `create-learned-generated`
read-only recovery mode with independent history custody and the original
guards; do not open them through the ordinary journal reader. Both report
the complete retained current-policy corpus, not a caller-selected sample.
`newly_reviewable` conveys no helper approval or promotion permission, and
`requires_shadow` is a missing-observation result, not an executed shadow run.

### P7. Running an experiment

1. `fa preregister H<n> --protocol <file>` before any confirmatory data; the ledger records the digest and date.
2. Run under `ExperimentCx` only: `fa branch`, `fa intervene`, `fa replay`, canaries through the audit floor. No branch returns a permit.
3. Record the result artifact against the preregistered row; a late protocol is labeled exploratory and cannot be promoted.

The implemented source-token experiment is
`supervise_publication compare-learned-prompt PLAN`. Its
[closed file-backed plan](LEARNED_PROMPT_COUNTERFACTUAL_OPERATOR.md) runs the
original monitored model under an exact prompt edit and a frozen paired seed
schedule. Use an unchanged edit as the sham control. Retain the full report,
including held, interrupted, failed-admission and unstarted trials; a censored
nonmatch is not a negative observation. This command neither loads a production
store nor registers a confirmatory protocol in an external ledger.

### P8. Fencing a fleet

1. `fa fence --scope <scope> --floor <epoch>` → the fence address.
2. `fa get <fence>` → the acknowledgment frontier: which domains fenced at which sequence, which are unacknowledged, which are under a partition lease and fence at expiry.
3. Post-fence effects in unacknowledged domains appear in the reconciliation report; treat them as effects, not as hypotheticals.

### P9. Handing off

`fa handoff <run|incident>` → a `Handoff` with the situation report, open affordances, annotations, rehearsals and the control sequence it is current to. Write it before you stop, even mid-playbook; the next agent's first step is to read it.

## Habits that keep you accurate and cheap

- **Read `Knowledge` variants literally.** `Unknown` is missing evidence and never permission; `Absent` is negative evidence only within its named domain; `Withheld` is not a hint.
- **Rehearse before mutating.** A rehearsal costs a branch, not authority, and its tree tells you whether the real action would even be admissible.
- **Buy the cheapest sufficient observation.** `fa next` already orders by information gain per cost; going around it to "just run everything" is the refinement storm the system limits (plan §14.9).
- **Reason in control sequence, not wall clock.** `fa when <a> <b>` is exact; timestamps are annotations.
- **Never raise a budget or a grade to finish a task.** Those are governed transitions with their own playbooks; the situation report will show the refusal, and the detector will show the attempt.
- **Leave typed notes.** Annotations and handoffs are how the system gets smarter across agents without getting more permissive.

## What this guide does not do

It does not grant authority; every verb's authority requirement is in the vocabulary registry and enforced by purpose contexts. It does not describe the controlled actor's surface, which is deliberately narrower (refusal codes, remedial actions and, under an `exact_predicates` disclosure profile, `fa precheck`). The explicit reference mappings do not claim the prospective `fa` CLI or a complete deployment exists; see [`IMPLEMENTATION_STATUS.md`](../IMPLEMENTATION_STATUS.md).
