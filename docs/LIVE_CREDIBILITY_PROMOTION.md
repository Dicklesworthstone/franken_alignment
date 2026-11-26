# Promote independently evaluated helper weights on the original owner

The supervisor can delegate one finite helper-weight promotion to the same
running native publication owner. The original independent evaluator supplies
labels through its existing separate role. The original marginal qualification
and independently selected joint replay policy decide whether the deterministic
candidate is eligible. The delegate supplies an operation identifier and exact
predecessor; it cannot supply weights, labels, a positive report or a replacement
joint policy.

This closes the native delegation gap between collecting independent outcomes
and explicitly applying their qualified helper weights. It reuses
`FileOversight::promote_credibility` and the original durable transition. It does
not create a second evaluation ledger, alter the journal format, approve an
effect or substitute policy-campaign promotion for helper-weight qualification.

## Provision separate, finite custody

Install the original joint credibility policy at bootstrap, before actor
requests. Existing marginal-only owners cannot be upgraded by this capability.
The trusted embedding retains the original supervisor and independent evaluator
as separate roles.

```rust,ignore
let (port, custodian) = supervisor.credibility_promotion_port(
    8,       // cumulative original qualification queries
    4096,    // complete journal, including the proposed Promote event
)?;

// Retain the noncloneable custodian independently of the weak port.
let plan = port.next(901)?;
```

Provisioning requires the existing immutable joint policy but does not require
that current evidence already qualifies. It reads no source, observes no new
time, reconstructs no numerical history and preserves the actor's unused
admission snapshot. This permits an independently selected grant to remain
resident while later independent assessments complete.

The noncloneable custodian owns one entered promotion and a finite query
allowance. Weak port clones share those same allowances. Dropping or revoking
the custodian withdraws access, including disclosure of a cached receipt.
A replaced, faulted, borrowed or missing owner cannot supply a new Known result.
Numeric scope equality does not make a foreign owner the original issuer.

The maximum query allowance is `MAX_CREDIBILITY_PROMOTION_QUERIES`. Complete
history work is bounded by `MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS` and the
explicit per-grant ceiling. Counts are checked before spending the mutation.

## Read the actual qualification, then explicitly select a transition

`next(operation)` spends one query and returns the original qualification at
one acknowledged historical cut. A Known plan carries:

| Coordinate | Meaning |
|---|---|
| `scope` | Original tenant, principal, run, branch, authority and Effect purpose |
| `journal_revision` | Complete acknowledged native journal |
| `control_sequence` | Original controller sequence |
| `authority_epoch` | Original effect-authority revocation floor |
| `evaluation_revision` | Independent evaluation inventory and labels |
| `congress_generation` | Current original helper-weight generation |
| `policy_generation` | Current native publication policy generation |
| `recorded_at` | Last original recorded elapsed time, including explicit absence |

The plan returns an executable native request only when the original marginal
and joint predicates qualify and all checked resource/control preconditions
hold. Missing, censored or unfinished evidence remains a refusal. A marginally
qualified candidate can still fail joint replay. Unchanged weights, exhausted
joint work, replay limits, stopped/suspended state, an unready native clock,
interrupted source state and unsupported numerical requirements remain explicit
block reasons.

This is a historical qualification query. It does not read fresh source, refresh
the clock, reserve the predecessor or promise eligibility at a later time.
Another assessment or native transition can make the plan stale. A blocked
query preserves the separate mutation allowance and actor admission snapshot.

`promote(request)` compares the complete current native cut with the submitted
predecessor. It uses the original native operation to recompute qualification;
a caller-provided positive plan cannot bypass that gate.

Malformed, foreign, stale or cheap resource/control refusals do not enter the
mutation. Once the native operation is entered, the grant spends its sole
mutation before replay, I/O or qualification can fail. A direct unqualified
request therefore consumes the entered operation and retains its refusal.
An entered sentinel prevents retry after an unwind or an uncertain result.

## Preserve native accounting and effect custody

A successful receipt records the before/after cuts, operation identifier,
cancelled original request identifiers and refunded units. The adapter checks
the native marginal and joint reports, original joint policy and these exact
transition invariants:

- Journal revision, control sequence, authority epoch, evaluation revision and
  congress generation each advance exactly once.
- Publication policy generation and recorded time remain unchanged.
- Already charged effect liabilities, including Unknown effects, remain charged.
- Only unused reservations cancelled by the original transition are refunded.
- Existing execution records, payload, target, ceiling, dispatcher epoch,
  suspension and Stop state retain their original meaning.

Native promotion withdraws old helper/human keys and retained review sessions
according to the existing original transition. Fresh subsequent publication
requires the original helper review and separate human key again. Neither a plan
nor a receipt is an effect permit.

Exact retry of the same operation and predecessor can return the retained
historical result while original healthy owner and custodian remain alive.
It spends no second mutation and performs no second numerical replay.
An altered predecessor conflicts, a different operation exceeds the grant, and
lost custody cannot recreate a receipt. This is live-grant idempotency, not
durable permission to retry across a restart.

## Numerical owners require explicit reconstruction funding

Ordinary nonnumerical grants refuse native/learned numerical histories. The
embedding may explicitly select:

```rust,ignore
let (port, custodian) =
    supervisor.credibility_promotion_port_with_numerical_budget(
        8,
        4096,
        4096,
    )?;
```

The entered mutation reserves the complete original history plus its proposed
event once, before native replay. The reservation is not refunded after an
uncertain or refused entered result. Next and exact cached retries perform no
numerical reconstruction. The original immutable numerical recipe still owns
its numerical ceilings; event units do not establish CPU-time bounds.

Owners requiring action-consistency forecasts remain unsupported even with this
allowance. A replay grant does not install missing independent predictors or
reconstruct missing role custody.

## Select the live Tower operator capability

The existing Linux operator endpoint can retain this grant on the same live
owner. Its independently selected public profile opts in with
`credibility_promotion`. Omission or `null` keeps the capability disabled.
The service retains the noncloneable custodian; reconnecting the operator does
not renew its queries, replay allowance or sole entered mutation.

For an ordinary service, select the original joint policy at creation through
`--andon` with `schema_version: "fa.native-andon-operator/2"`. That profile
requires the complete `joint` object, evaluation protocol and accounting
inventory described in [Joint native Andon](JOINT_NATIVE_ANDON.md#shared-operator-profile-and-guarded-promotion).
The example
[joint accounting profile](../crates/fa-reference/fixtures/joint_native_andon_operator.json)
shows the closed format; its numerical policy inputs are examples, not
recommended calibration thresholds. Creation requires identical initial/current
windows and minimum evaluation revision zero. A marginal-only `/1` owner cannot
gain joint qualification by installing the operator grant.

With the original actor/reviewer profiles, independent evaluator profile and new
retained-history destination, the ordinary creation command is:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --tower-generation 41 --requests 7,8 \
  --andon JOINT_ANDON_PROFILE \
  --retain-history-anchor /absolute/independent/new-history-anchor \
  --operator-profile OPERATOR_PROFILE \
  --evaluator-profile EVALUATOR_PROFILE
```

The existing [live evaluation workflow](LIVE_ANDON_EVALUATION.md#create-an-accounted-service)
owns bootstrap, credential separation, case discovery and assessments. Its
`evaluator-case` and `evaluator-assess` commands continue to use the separately
authenticated evaluator endpoint. The operator receives no evaluator credential
or permission to label a case. The grant can be selected before evidence
qualifies and remain resident while that evaluator supplies later assessments.

For example, a public operator profile selecting only this capability is:

```json
{
  "schema": "fa.tower-operator-service/1",
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "socket": "/run/fa/operator.sock",
  "supervisor": {"uid": 1000, "gid": 1000, "pid": null},
  "operator": {"uid": 1001, "gid": 1000, "pid": null},
  "candidate_limit": 8,
  "connection_limit": 4,
  "exchange_limit": 32,
  "response_bytes": 65536,
  "runtime_ms": 60000,
  "poll_ms": 5,
  "inspection_queries": 0,
  "explanation_queries": 0,
  "explanation_replay_events": 0,
  "explanation_disclosure": "none",
  "credibility_promotion": {
    "queries": 8,
    "max_replay_events": 4096,
    "authority": "promote_qualified_credibility"
  }
}
```

Replace the scope, socket and kernel identities with the independently selected
values for the deployment. The existing
[operator profile and endpoint checks](LIVE_TOWER_OPERATOR.md#public-operator-profile)
still apply. The `credibility_promotion` object is closed:

| Field | Contract |
|---|---|
| `queries` | Required positive unsigned JSON integer, bounded by `MAX_CREDIBILITY_PROMOTION_QUERIES` |
| `max_replay_events` | Required positive unsigned JSON integer, bounded by `MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS`; covers the complete history plus proposed Promote event |
| `authority` | Required exact string `"promote_qualified_credibility"` |
| `numerical_replay_events` | Optional positive unsigned JSON integer no greater than `max_replay_events`; omission withholds numerical reconstruction work |

Duplicate, unknown or missing required fields refuse. Caller-selected weights,
labels, candidates and an `operations` override are not profile fields. There
is exactly one entered mutation per native custodian. Numerical funding applies
only to a supported original numerical owner that already has its joint policy;
it does not enable an unsupported bootstrap or command combination. A separate
`governance_ledger` selection is required to read accounting through Ledger.

**Select `response_bytes: 65536` for this operation.** The server reserves space
for a complete worst-case receipt before calling native promotion, including
up to `MAX_ATTEMPTS` (1,024) cancelled identifiers and the full predecessor and
successor. The 64 KiB selection covers this bound. The unchanged 16 KiB default
cannot cover it, so Promote returns `BudgetExhausted` before spending the
operation or changing journal, weights or reservations. Next can still describe
native qualification under that smaller allowance. Increasing output capacity
adds no native authority, query budget or replay allowance.

## Select a jointly evaluated learned service

Single-request and resident learned services accept the original joint Andon
`fa.native-andon-operator/2` profile, together with the independently
authenticated evaluator and retained-history output. Atomic creation installs
that exact policy with the original numerical recipe, guards, accounting and
terminal reserve before returning any live role:

```sh
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE \
  --actor-profile ACTOR_PROFILE \
  --andon JOINT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --retain-history-anchor /absolute/independent/new-learned-anchor \
  --operator-profile OPERATOR_PROFILE --tower-generation 41
```

For a finite sequence, use `serve-learned-generated CONFIG SEQUENCE_PROFILE
REVIEWER_PROFILE` with the same paired evaluation and operator options; each
entry selects its own actor profile. The original evaluator and promotion
custodian remain attached across Message/Finish handoffs. Neither reconnecting
nor advancing to another entry replenishes their finite work.

Add explicit numerical funding to the operator profile above:

```json
"credibility_promotion": {
  "queries": 8,
  "max_replay_events": 4096,
  "numerical_replay_events": 4096,
  "authority": "promote_qualified_credibility"
}
```

Retain `response_bytes: 65536` for the complete mutation receipt. The event
allowance must cover the complete acknowledged history plus the proposed
promotion event at the actual selected cut; 4096 is an example ceiling, not an
estimate of the work for every deployment. The learned operator profile
rejects missing numerical funding before startup. Insufficient funding at the
selected cut blocks promotion while preserving its independent mutation allowance.
Selecting the grant with marginal-only Andon `/1` refuses during preflight.

Explicit learned continuation also supplies the independent input history and
new output. The complete recovery requirements pin every original joint-policy
field before the sole Fence. Evaluation-only learned recovery accepts that same
exact joint contract but does not start an operator or publication service.
The Andon joint policy is separate from the learned recipe's held-out
publication policy; existing forecast, topology, campaign and held-out
composition exclusions remain in force for this evaluated service.

The original independent evaluator must still discover and label actual
completed cases. A single learned request may supply insufficient independent
evidence; installing a grant does not manufacture the missing strata.
Use the existing explicit Next/Promote documents below only when the native
plan is eligible. Another native transition can stale that predecessor.

Promotion preserves the original numerical owner and charged liabilities while
withdrawing old identity and helper/human approvals. A later release needs fresh
identity, source, helper review and independent human approval at the new
authority epoch. A request accepted under an older epoch is not automatically
rewritten or retried. See the
[learned evaluation workflow](LIVE_ANDON_EVALUATION.md#create-an-evaluated-learned-service)
and [resident contract](RESIDENT_LEARNED_PUBLICATION.md#independent-evaluation-and-andon-accounting)
for the original scheduling and custody boundaries.

## Select a jointly evaluated resident native service

The resident native command also accepts the original joint Andon `/2` policy:

```sh
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --andon JOINT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --retain-history-anchor /absolute/independent/new-native-anchor \
  --operator-profile OPERATOR_PROFILE --tower-generation 41
```

Use the numerically funded operator object above and `response_bytes: 65536`.
Atomic bootstrap installs the selected original joint policy with the decoder,
tokenizer, source, generated stream, evaluation, accounting and terminal reserve.
The evaluator and operator stay with that same owner through every Message and
Finish. A marginal-only selection with a promotion grant refuses during
preflight; it cannot install a policy after startup.

Anchored remaining sequences may use `--after-message PREVIOUS_REQUEST` or
`--continue-generation`, together with the independent input history and
`--evaluation-recovery REQUIREMENTS`. The complete recovery contract requires
the exact original joint policy before its Fence, while preserving original
numerical work, cases, labels, costs and liabilities. A sole remaining Finish
requires the confirmed Message predecessor and performs no new inference.

Qualification and promotion retain the same explicit Next/Promote law below.
The original grant is shared across entries, and a label supplies no publication
key. Missing violation evidence remains an explicit marginal refusal. After a
permitted weight transition, any subsequent entry must independently satisfy
the new authority epoch and fresh source, helper and human requirements.
The service never rewrites a stale selected request to make it acceptable.

The [resident native contract](RESIDENT_NATIVE_PUBLICATION.md#independent-live-evaluation-and-andon-accounting)
defines request sequencing, complete endpoint/input custody, independent Stop,
exact recovery and the continuing exclusions for topology, campaign governance
and separate held-out publication qualification.

## Inspect and submit explicit Tower documents

Next and Promote reuse the existing registered verbs. The closed
`arguments.kind: "credibility_promotion"` discriminator selects helper-weight
promotion. The native `arguments.operation` and predecessor integers use
canonical decimal strings, separately from the numeric operator-profile fields.
The outer `operation_id` is the request's correlation label.

For the example scope and registration above, save this complete request as
`credibility-next.json`:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-credibility-901",
  "address": "fa://1/run/3@41",
  "verb": "next",
  "arguments": {
    "kind": "credibility_promotion",
    "operation": "901"
  },
  "idempotency_key": "901"
}
```

Send that document through the authenticated operator client:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE credibility-next.json
```

The `credibility-promotion-next` result uses schema
`fa.publication-credibility-promotion-next/1`. Its native Known value gives
`can_promote`, an explicit `block_reason` when blocked, and an `action` only
when the original qualification and checked preconditions permit one. Planning
cost, information gain, ranking and current eligibility remain explicitly
Unknown. The query reads acknowledged original state; it neither acquires fresh
source evidence nor refreshes the clock.

When `can_promote` is true, inspect the complete request object at
`result.value.knowledge.value.action`. If you select that transition, save that
object as `credibility-promote.json` and submit it explicitly:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE credibility-promote.json
```

The returned action uses `verb: "promote"`, the same
`kind: "credibility_promotion"`, the exact operation and all eight basis fields
listed above under `expected`. Its explicit registration generation and
idempotency key must be retained. Another assessment or journal transition can
invalidate that predecessor. A previous Known plan provides no permission to
replace its cut, supply a positive report or skip native requalification.

The `credibility-promotion` result uses schema
`fa.publication-credibility-promotion/1`. A successful Known receipt retains
the full before/after cuts, cancelled identifiers and refunded units described
above. It carries no weights, independent labels, payload, evaluator role or
effect permit. Operator capability discovery separately lists
`credibility_promotion` with its actual installed-grant observation and
remaining finite work; discovery itself spends none of that work.

The client validates the selected audience, registration, operation,
predecessor, result shape and receipt transitions before emitting a reply or
sending a later explicit document. It never follows a returned action or
automatically retries a promotion. An explicit same-grant retry may obtain the
cached historical result under the native idempotency rules above. A lost
transport or malformed reply does not establish that the native mutation failed
or was rolled back.

## Source and qualification boundary

The native regression definitions cover positive joint qualification, fresh
two-key publication after promotion, retained Unknown charges and refunded
unused reservations; exact and stale predecessors; live retry across clones;
marginal/joint counterexamples; unfinished, pending and censored cases; work and
history limits; original owner, borrow and revocation boundaries; and original
clock, Fence, Stop and suspension behavior. Compile-fail examples keep grant
custody and effect authority separate.

Five Tower and eight authenticated live service/client regression definitions
also cover original labels arriving while the operator is resident, closed
request/receipt validation, role and owner boundaries, insufficient output
capacity followed by the same action's permitted transition, exact retries and
client termination on causally corrupted replies. All 28 changed Rust files
passed scoped Tree-sitter parsing; the eleven Tower-owned files also received
scoped rustfmt parsing/formatting before the formatter host restarted.

Three additional learned numerical regression definitions exercise actual
original generation, paid promotion, retained Unknown charges, refunds of unused
reservations, exact retries during later review work and publication through
fresh keys. Paired missing/one-short numerical allowances and stale predecessor
cases check that refused work preserves the original operation and state. Live
learned service regressions cover the new joint selection and exact recovery.
These newly added sources have not been compiled or executed.

These are authored regression definitions, not an execution receipt. The
required command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`
could not start because `rch` was absent (exit 127), and the execution host
subsequently disconnected. Compilation, runtime tests, Clippy and canonical
qualification remain open. FA-105/134 and existing evaluation acceptance
conditions retain their original status.
