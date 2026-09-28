# Native model judgments in the durable learned congress

This L3/L5 integration serves FA-113 and plan sections 9.1–9.4 and 10.15. It
connects the existing source-bound SidecarNativeEvaluator to the existing durable
Coordinator, DurableSession and learned-sidecar finish transaction. It does not
replace any numerical, congress, disclosure, authority or journal algorithm.

## One independently provisioned round

`FileOversight::begin_native_sidecar_review` consumes a live FileLearnedSidecar,
one original LearnedWorkerRound, the complete member map, and NativeReviewLimits.
Each NativeReviewMember supplies one unadvanced original NativeEvaluator, a
nonempty frozen SidecarProbeQuery inventory and an independently provisioned salt.
Every captured K/V row requires a compatible query. This covers the captured
source descriptor, not unobserved positions or the actor's whole history.

Before durable Begin, admission checks the exact member set, full input profiles,
original model state, query identities and coverage, salt lengths, deadlines,
receiver limits and aggregate native-policy budgets. NativeReviewCost::required
sums each original model's scalar-product and sampling-entry ceiling. An exact
allowance admits; one less refuses. The reservation is retained after early
stops, failures or cancellation. It is not measured runtime or peak memory.

Each advance permits at most one request admission, exact probe, native token,
commitment or reveal per member. Complete quiet numerical probes start the
original native evaluator over the WHOLE original helper input. Quiet probes
alone do not supply Allow. The native model must finish its own monitored output
and strict response parser. Its Allow/Hold/Deny/Abstain remains an ordinary
original congress vote. Numerical alarms Hold; unresolved/equal-threshold probes
Abstain without model inference. Malformed or held native output remains a failed,
missing member, never an invented vote or replacement worker.

The original source is checked before every member quantum. Completed judgment
yields before queuing a commitment. The original coordinator alone opens reveals,
and every accepted phase crosses its original journal boundary. One-round
execution applies the completed original review without buying an unusable next
refinement. An abstention therefore still blocks permission. The round is leased
against manual commit/reveal/finish fallback, even after driver cancellation.

## Outcomes, failures and authority

`records` retains original numerical progress and first member failures. Queued
messages are not necessarily durably accepted votes. An interrupted operation is
explicit; completed counters can understate work. Unwinding drops active ports
and native owners and leaves the driver failed. Cancellation releases unfinished
owners while preserving costs. Stale/foreign calls perform no work. A poll limit
never slides a deadline or produces a successful review.

`outcome` is available only after the existing durable sidecar finish transaction
acknowledges it. The original outcome retrieval APIs work unchanged, including
recovery after a lost reply. Journal replay verifies original actor numerics,
source packets and accepted congress transcripts; it does NOT rerun the native
helper models or authenticate their identity. Native progress records are local
execution diagnostics, not a new persisted compute escrow or evidence of helper
independence. Models and source bindings are provisioned in process. This is not
process isolation, network authentication, calibrated detection or wall-clock
preemption. One model token and canonical filesystem operations are synchronous.

Application still requires the original current source/input, exact policy and
all configured effect keys. A native Allow is not a permit, nor proof of an
executed effect. Recovery never resumes a driver, reinstates old keys or refunds
unknown external effects. No new journal tag, wire format or dependency is added.

## Authored regression coverage

Eight integration functions use the original history-sensitive actor fixture,
fitted learned codec, exact probes, byte-BPE native-model fixture and real Unix
journals. They cover complete native Allow through both publication keys,
question-dependent native Deny despite quiet probes, numerical uncertainty and
alarm before native inference, malformed/held model output, exact/one-less native
reservations, incomplete profiles/rosters, source loss and partial cancellation,
stale/foreign calls, deadlines, poll exhaustion and a real staged-file barrier.
Two compile-fail examples exclude manual verdict and mutable evaluator access.
Synthetic model coefficients test execution plumbing, not learned safety quality.

The preparation environment has no RCH, Cargo, rustc or rustfmt. Targeted and
full RCH commands stop before compilation (exit 127). Rust compilation, tests,
formatting and Clippy are UNEXECUTED. Selected source and hash/whitespace checks
are not a complete checkout or runtime qualification. No Bead or gate is closed.
