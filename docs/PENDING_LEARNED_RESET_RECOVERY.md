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
