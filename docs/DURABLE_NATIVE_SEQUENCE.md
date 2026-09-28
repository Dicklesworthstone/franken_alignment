# Native-model congress across durable residual refinement

`FileOversight::begin_native_sidecar_sequence` connects the original native-model
reviewer to the original multi-round learned-sidecar refinement path. It serves
FA-113 and plan section 10.15: unresolved disclosed numerical evidence can now
purchase an original residual and receive a fresh complete native-model review,
without replacing the congress, codec, journal or either publication key.

## Freeze the whole campaign before its first result

The caller supplies the existing source-bound sidecar, ordered original
LearnedWorkerRound values, NativeSequenceRosters and NativeReviewLimits. Each
round has a complete, independently constructed fresh native-model roster. All
input profiles, K/V queries, model owners, salts, round identities, evidence roots
and commit/reveal windows are checked and fixed before the first durable Begin.
The original row-coverage and query checks apply to every future roster too.
There is no method to replace a model, change a query or reassign a missing vote.

NativeReviewCost::required_sequence sums the original model-policy ceilings over
all scheduled rosters. Admission requires the entire cost, including future and
ultimately unused models, within the original 256-evaluation aggregate maximum.
At most the original 32 refinement rounds may be supplied. limits.polls is one
lifetime allowance, not a per-round refill. The helper, receiver and numerical
probe limits retain their original per-evaluation meaning. These are logical
admission reservations, not measured FLOPs, wall time or a durable compute escrow.

All scheduled IDs are leased against the existing manual phase fallback. Public
single-round admission cannot adopt these leases. The sequence alone can begin
its provisioned successor using the same private admission implementation. An
unstarted or cancelled round never becomes a manual replacement-vote opportunity.

## Original evidence, fresh judgment, original authority

Each sequence advance delegates to the existing FileNativeSidecarReview, with
at most one admission, probe, native token, commitment or reveal per member.
The complete original source is checked at its existing per-member boundary.
Numerical uncertainty remains Abstain; a numerical alarm remains Hold. Quiet
probes merely admit the native evaluator over the whole actual helper input.
Only its original monitored completion and parser can produce a native ballot.
Native Deny is not changed to Allow; malformed/held native output remains missing.

Only the original witnessed finish decides whether refinement is allowed. The
sequence offers a next round only while its predeclared window and lifetime poll
allowance remain usable. A final, missing or denying result does not hunt for
another model. When refinement occurs, the SAME sidecar advances through its
original budget accounting, and a new original congress receives the richer
input. The transition call initializes the successor but runs no successor
admission, probe or model token. Old votes cannot approve the new bytes.

The original public single-round API remains non-refining. The sequence shares
its implementation rather than maintaining a second numerical/protocol engine.
No journal or wire tag, stored recipe, dependency or public actor command changes.

## Retain work and acknowledged outcomes through failure

Records cover every scheduled round and member. Unstarted members are None,
not invented quiet observations. Started records retain actual native/probe
progress, original worker states, first failures and queued-message indicators.
Interrupted calls are marked because their last returned counters may omit work.
History contains acknowledged original finish results in fixed schedule order.

Source loss, pending original work, storage failures, cancellation and poll
exhaustion release all active and future native owners. The whole reservation,
returned measurements and acknowledged history remain inspectable. A caught
unwind leaves the sequence failed and releases both active and unused rosters;
no later call can resume that interrupted execution. Stale/foreign calls and a
backward clock refuse before new work. Deadlines are never moved.

A richer receiver may fail its fixed byte allowance AFTER refinement was
acknowledged. That refinement and its disclosure cost remain in history and the
original durable outcome index. Earlier votes are still invalid, and no new
model runs. Dynamic richer-input size admission is not represented as something
known before that packet exists.

Publication still requires fresh original source/input, original congress
application, automatic authorization and the configured human key. Journal
replay reconstructs actor numerics, source packets and accepted original
transcripts; it does not authenticate or rerun the helper models. Progress and
reservations are process-local, not statistical independence or isolation proof.
Individual model tokens and whole-journal replay/fsync remain synchronous.

## Implementation and verification status

Added the fixed native sequence and eight integration regression functions,
plus a compile-fail replacement boundary. Positive source tests use real original
probe uncertainty, residual refinement, fresh native Allow and both keys to one
canonical publication. Other tests cover native Deny after refinement, missing
native output, complete future admission, exact/one-less aggregate budgets,
lifetime polls, partial cancellation, source and pending-token barriers, richer
receiver refusal, unchanged archives and manual-phase leases. Existing native
single-round tests and their model fixtures are unchanged.

Compilation, Rust tests, rustfmt and Clippy remain UNEXECUTED: the required
remote-only RCH launcher is absent in this preparation environment. Source and
hash/whitespace checks do not qualify these authored controls. No Bead, runtime
admission, detector, independence or production release is certified here.
