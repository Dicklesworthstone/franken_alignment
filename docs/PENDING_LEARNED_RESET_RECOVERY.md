# Complete a pending learned reset before the recovery fence

The original durable reset can leave an acknowledged `BeginReset` without its
completion after a process or storage failure. Ordinary recovery intentionally
fences that intent and leaves it as an interrupted barrier. This additional,
explicit recovery path serves the existing paired learned reset consumer without
changing ordinary open, the reset algorithm, checkpoint selection, or journal tags.
It belongs to the plan's L5 containment/recovery work (section 11.10; FA-108).

## Explicit selection, original computation, one publication

The supervisor independently supplies the exact intended checkpoint ID,
`FileResetRequest`, and `KvRestartBudget` with
`FileLearnedResetIntent::for_recovery`. This constructs comparison data, not a
checkpoint, approval, or new reset instruction. The journal must already contain
the matching original intent and a reconstructible original paired checkpoint.

Begin with `FileOversight::begin_open_with_learned_generation` and the separately
retained exact numerical/text recipe. Advance `FileLearnedRecovery` until Ready;
no checkpoint, writable owner, or review key is available during replay. Then
call `finish_pending_reset(&expected_intent)` instead of ordinary `finish`.

Before doing new restart work, the method checks the complete command, original
control predecessor, uninterrupted status, both event slots, and the exact
canonical image held under the original exclusive lock. It invokes the EXISTING
`Machine::prepare_learned_reset`: the original full-prefix audit, exact KV restore,
conserved continuation budgets, containment transition, key withdrawal, and
comparison witness are not reimplemented. An original refusal is retained as a
refusal, including its actual admitted work and any numerical failure.

The resulting event remains private. The existing recovery finish appends its
original fence and uses the same persistence boundary for ONE canonical image
containing BOTH completion and fence. It checks the canonical input again before
cleanup and replacement. No intermediate reset-only file, usable owner, or old
approval is released. Failed preparation, encoding, or storage returns no candidate
result. A caught unwind cannot reuse this consumed recovery object.

After success, obtain the acknowledged original outcome from
`learned_reset_result(operation)`. Successful recovery does not mean successful
restoration: the inner result can be an original refusal or an incident-driven
non-restoration. The returned owner is paused. Fresh time and original resume
remain mandatory, and a restored empty source needs a new accepted token before
it can support a proposal. Congress and both configured effect keys still apply.

## Retry and refusal boundaries

An ambiguous replacement may already have published both events. Repeating the
same independently supplied command against that complete history performs only
the usual recovery fence, not another reset, audit, incident, or budget refill.
A conflicting command or a different pending operation refuses.

This is NOT an escape from a recorded fence or stop. An already interrupted
intent remains interrupted. Changed original control/actor/epoch values refuse;
no instruction is rebound to current authority. An expectation without a prior
BeginReset cannot create one. Corrupt suffixes and incomplete/failed replay cannot
reach completion. Ordinary `finish` and all existing open methods keep their
original quarantine-only behavior.

Dispatched and executed effects remain charged. Recovery does not run external
effects, infer nonexecution, or refund an unknown send. Existing reconciliation
and sealing obtain the endpoint's original outcomes separately. The local journal
is the sole publication sink; this is not a distributed transaction protocol.

Logical reset accounting describes the single journaled operation. Replaying
history and repeating an unacknowledged recovery can do physical work again.
There is no cross-process CPU escrow, anti-rollback store, cryptographic source
authentication, or preemptive latency guarantee. Independently retaining the
recipe and intended command does not authenticate an unanchored journal suffix.
The base recovery path returns only its existing human role; composed guard
profiles still require their separately matched role-aware recovery paths.

## Verification status

Nine regression functions and one compile-fail example are authored. They use
original numerical/reset/review operations and real local journals. Coverage
includes original-path parity and sampled continuation, fresh two-key publication,
complete command matching, preserved ordinary-fence/stop barriers, actual audit
refusal, all five original storage-failure seams, ambiguous-success retries,
unknown/executed liabilities, exact versus one-less event capacity, incomplete
replay, changed canonical bytes, and a corrupt completed-reset witness.
The original checkpoint and reset test bodies are unchanged.

Targeted and full checks were attempted through `RCH_REQUIRE_REMOTE=1 rch exec`.
Both stopped before compilation because `rch` is absent (exit 127). Cargo, rustc,
and rustfmt are absent too. Rust compilation, tests, formatting and Clippy are
UNEXECUTED. Selected-source hash/whitespace checks are not runtime qualification.
No Bead, release gate, restart grade, or empirical detector claim is closed.

## Cooperative composed-guard recovery

`begin_open_guarded_with_learned_generation` and its explicitly anchored variant
`begin_open_guarded_anchored_with_learned_generation` return the sealed
`guarded::learned_recovery::FileGuardedLearnedRecovery`. They use the same original
exclusive lock and canonical cut as basic learned recovery, not a second reader
or writer. The independently supplied numerical recipe and decoder inventory are
matched before any event executes; the anchored variant also invokes the original
exact prefix comparison then. No retained anchor is advanced implicitly.

The wrapper freezes the full `FileRecoveryRequirements`, including the guard set,
effective policy, credential epoch and all history floors. `advance` delegates
bounded original event replay and verifies those requirements at completion. A
contract mismatch is a retained Failed state, never Ready and never repairable
by changing the caller's copy. Incomplete, failed and dropped runs expose no roles
and leave the canonical/staging files unchanged. Event quanta do not preempt one
numerical audit, encoding operation or filesystem synchronization.

`finish` publishes the original fence, then provisions the original human,
identity-observer and policy-governor roles together. `finish_pending_reset` first
checks the full requirements, stages the SAME exact pending reset used by the
basic recovery path, checks the requirements again on its resulting candidate,
and only then publishes completion and fence atomically. There is no public
base-owner extraction, mutable-machine view, partial role bundle or new authority
algorithm. All original guard combinations that the base guarded profile excludes
remain excluded; this is not an evaluated/predictive/mediated-profile downgrade.

Six additional regression functions cover all identity/governor presence
combinations, cooperative admission and lock custody, anchored reset through
fresh identity and two-key publication, actual fresh governor use versus an old
role, frozen requirements, terminal guard/policy/floor mismatches before cleanup,
valid equal-counter forks, all five original storage barriers and ambiguous
completion retries, early drop and preserved ordinary quarantine. These tests
and two added compile-fail boundaries are authored but UNEXECUTED. Together this
series has fifteen regression functions and three compile-fail examples. Existing
first-increment regression bodies and original guard/prefix/reset/replay/fence
algorithms are unchanged. The internal composition views are immutable and stop
at the containing observed-owner implementation; none is a public escape hatch.

## Evaluated and predictive learned profiles

The evaluated profile now has its own sealed
`FileEvaluatedLearnedRecovery`, obtained from
`begin_open_evaluated_guarded_with_learned_generation` or its anchored variant.
It pins the independent evaluation protocol, complete recovery requirements and
learned recipe before replay, then checks the full contract before returning
Ready and around any pending reset. Synchronous
`open_evaluated_guarded_with_learned_generation` consumes the same cursor.
The returned roles include the independent evaluator as well as the original
human, identity and policy roles. Old evaluator tickets and roles cannot be reused.

`FileEvaluatedLearnedRecovery`, `FilePredictiveLearnedRecovery` and
`FileOwnedPredictiveRecovery` each expose `finish_pending_reset`. All three call
the original private reset transition only after checking their own requirements,
check those requirements again on the resulting candidate, and publish completion
and the recovery fence through the same atomic replacement. Predictive recovery
retains the exact raw or owned predictor, optional evaluator, source mode, timing
and lifetime ceilings. An already completed matching reset receives only another
fence; a changed, premature or already interrupted intent refuses.

This completes reset recovery and returns fresh role custody. It does not supply
fresh source evidence: the returned generator is paused and its restored source
is Empty. Pending forecasts retain lost coverage and spent work. Legacy predictor
recipes still pin the original source stream and do not automatically accept a
reset successor. Mediated and joint-evaluation profiles require their own complete
contracts and cannot use the evaluated-only path as a fallback.

New runs can independently select the
[versioned forecast reset-successor recipe](LEARNED_FORECAST_RESET_SUCCESSORS.md)
to continue from a clean checkpoint before prompt completion. This does not
change the behavior of existing recipes or recover lost forecast coverage.

The added public integration cases execute cooperative replay, exact protocol and
guard failures, anchored reset, fresh evaluator custody, fresh identity and both
publication keys. Eight additional unit regressions cover both predictor source
modes, retained unanswered forecasts, idempotent completion and all five original
storage failure barriers. See the current implementation-status entry for the
scope of local development execution; historical qualification receipts are
unchanged.
