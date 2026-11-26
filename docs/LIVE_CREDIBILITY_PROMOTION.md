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

## Source and qualification boundary

The native regression definitions cover positive joint qualification, fresh
two-key publication after promotion, retained Unknown charges and refunded
unused reservations; exact and stale predecessors; live retry across clones;
marginal/joint counterexamples; unfinished, pending and censored cases; work and
history limits; original owner, borrow and revocation boundaries; and original
clock, Fence, Stop and suspension behavior. Compile-fail examples keep grant
custody and effect authority separate.

These are authored regression definitions, not an execution receipt. The
required command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`
could not start because `rch` was absent (exit 127), and the execution host
subsequently disconnected. Compilation, runtime tests, Clippy and canonical
qualification remain open. FA-105/134 and existing evaluation acceptance
conditions retain their original status.
