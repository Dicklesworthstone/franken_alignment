# Agent guide: operating FrankenAlignment from the driver's seat

**Who this is for.** An AI agent (or a human using the same surface) that operates, investigates or supervises a FrankenAlignment deployment: the operator agent, not the controlled actor. Developer conventions are in [`AGENTS.md`](../AGENTS.md); the one-page model is [`docs/SYSTEM_MAP.md`](SYSTEM_MAP.md). Everything here describes the prospective surface of plan §17; none of these commands exists as code today.

**The contract you can rely on.** Every value you receive carries its epistemic status; every object has an address; every mutation is idempotent with an expected predecessor; every rehearsal is labeled and can never mint a permit; every error is a plan (it names what would change it). You never need to guess whether a number is measured, whether an absence is observed or merely unobserved, or whether a command changed the world.

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
3. Rehearse the top affordance: `fa propose <affordance> --branch <b>` → `RehearsalOutcome` with the tree the nucleus would produce and the cost it would charge.
4. Execute: `fa propose <affordance>` → the decision card (alternatives not chosen), realized cost, new control sequence.
5. Repeat from step 1 until the tree is satisfied (`Continue`) or a consequence at or above `HoldEffect` is final. A `SuspendRun` or `ResetToCheckpoint` is not a failure of the playbook; it is the outcome.
6. `fa annotate <closure> --class conclusion` with what resolved it, then `fa handoff <run>` if you stop here.

Failure branches: `BudgetExhausted` names the budget and the governance transition that could raise it (never raise it yourself to finish a hold); `StaleGeneration` means re-read the situation from the returned predecessor; `OutcomeUnknown` on a dispatched effect means reconcile (P3), never retry.

### P2. Investigating an incident

1. `fa get fa://…/incident/<id>` → the incident with its `Knowledge` status and the addresses of the closures, rounds, evidence and findings it references.
2. `fa explain <closure>` for each implicated closure → predicate trees; disagreement between helpers is localized first by comparing their view manifests (`fa related <view>`), never by assuming they saw the same document.
3. `fa branch --from <closure>` → a capability-free branch; `fa intervene --branch <b> --remove <evidence>` or `--substitute`, `--clamp`; `fa replay --branch <b>` → a `CounterfactualWitness` with its replay grade.
4. Include the controls: unchanged replay, sham intervention, lossless-versus-lossy checkpoint. A witness without controls is annotated as exploratory.
5. `fa annotate <incident> --class hypothesis|inference|conclusion` at each step; the trail is the deliverable.
6. If the incident implicates the guarded model's identity, check `fa get fa://…/passport/<id>` and the latest liveness observation before anything else; substitution changes every conclusion.

The actor gets no channel into this playbook. Its statements, if any, enter only as `ActorStatement` evidence with the untrusted label.

### P3. An effect's outcome is unknown

1. `fa situation --scope run` lists it under unknown effects with its adapter's idempotency contract.
2. If the adapter is `RemoteIdempotent`: `fa resolve <attempt> --reconcile` re-queries under the same key. If `NonIdempotent`: `fa resolve <attempt> --disposition <ConfirmedNotExecuted|Confirmed|IrrecoverablyUnknown>` requires an independently established outcome and the authority to record it.
3. Never `propose` the same effect again to "make sure"; the reserved units stay charged until reconciled (plan §25.3).

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

### P7. Running an experiment

1. `fa preregister H<n> --protocol <file>` before any confirmatory data; the ledger records the digest and date.
2. Run under `ExperimentCx` only: `fa branch`, `fa intervene`, `fa replay`, canaries through the audit floor. No branch returns a permit.
3. Record the result artifact against the preregistered row; a late protocol is labeled exploratory and cannot be promoted.

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

It does not grant authority; every verb's authority requirement is in the vocabulary registry and enforced by purpose contexts. It does not describe the controlled actor's surface, which is deliberately narrower (refusal codes, remedial actions and, under an `exact_predicates` disclosure profile, `fa precheck`). And it does not claim any command exists yet; see [`IMPLEMENTATION_STATUS.md`](../IMPLEMENTATION_STATUS.md).
