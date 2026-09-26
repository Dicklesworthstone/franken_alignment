# Paired learned-monitor policy comparison

## Source addition; not qualified

`sampling::replay::comparison` supplies an L7 experiment over the existing
learned-audited generator. It serves plan sections 10.13, 11.3, 12.2 and 14.5:
measure the consequences of a proposed monitoring-policy change without
installing it in the source generation or mistaking an experiment for authority.
No roadmap packet, Bead, release or production capability is qualified here.

`ReplayableGeneration::compare_policy_from_start` borrows the source's immutable
recipe and constructs two original `LearnedGeneration` owners. The model,
prompt, sampling policy/seed/stream, stop IDs, original numerical ceilings and
aggregate telemetry ceilings are identical. The complete candidate learned
policy is the named intervention; it may change codec, probes, retention or
per-token ceilings. It is not automatically a single-probe causal attribution.
Read-only policy accessors preserve the actual parameter objects, not just IDs.

The method explicitly restarts the EXPERIMENT at position zero. It neither
resumes nor rewinds the source, including a source already held or stopped.
The pair retains one original stream/evaluation origin; it does not manufacture
two independent observations or a new live observation.

## Positive behavior and stopping boundary

Both arms execute real original inference, source-checked learned compression,
complete K/V monitoring and original sampling at every attempted position.
After two quiet observations, the existing checkpoint capture compares exact
cache and logit words, all token/sample history, random draws, probability bits,
sampler state, terminal status and numerical work. Different telemetry costs
are retained separately, not incorrectly required to match.

A matched pair exposes one accepted step and extends only its common prefix.
A hold in either arm stops the pair. The counterpart's extra accepted token is
not exposed through the paired step or common-prefix accessor. Two holds are
reported as `BothHeld`, not a completed equivalent continuation. A failure to
obtain an audit is `Failed`, not a policy disagreement. Both ordinary Result
outcomes and available audit evidence remain inspectable after a failed pair.
There is no mutable-arm accessor, executable-owner extraction or promotion API.

An EOS stop is a matched stop at the observed horizon, not a claim about the
unexecuted remainder. Numerical divergence fails with `Binding` rather than
being passed off as a monitoring-policy effect. Exact comparison of two
computations does not establish model safety, detector accuracy or validity on
another backend, workload or model generation.

## Resource accounting

`ComparisonLimits` bounds attempted positions in each arm and logical bytes in
each temporary comparison state. Two states may coexist; model/codebook
retention, allocator overhead and peak RSS are not this logical byte measure.
The fresh decoder-product and vocabulary-score allowances cover BOTH arms
before either starts, with no assumed early-stop discount. Original per-arm
ceilings continue to apply. Telemetry remains bounded separately by the
original aggregate and per-token ceilings in each arm; total reported telemetry
is the two rows, and may be at most twice the original aggregate ceilings.
These are numerical admission limits, not a global research-resource escrow.

Reservations remain visible on unsuccessful attempts. Generation failures may
have unreported bounded telemetry work; `all_attempted_telemetry_reported` is
then false. A stale expected position changes neither arm. An error or caught
unwind latches the pair, so it cannot retry a half-completed position or refill
allowances. No wall-clock, performance, allocation or statistical claim follows.

## Verified checkpoint and archive baselines

`GenerationCheckpoint::begin_policy_comparison` and
`GenerationArchive::begin_policy_comparison` create a `ComparisonPreparation`.
An existing, unadvanced `GenerationReplay` can also be consumed through
`prepare_policy_comparison`. The candidate policy and BOTH-arm numerical
allowances are admitted and frozen before baseline verification executes. A
partially advanced or already verified replay cannot be adopted after seeing
its numerical results. Neither experimental arm is accessible during preparation.

Preparation delegates every step to the ORIGINAL `GenerationReplay`. It must
rerun the original model, sampler and learned monitoring policy and compare the
complete saved expectation, including its original telemetry counters. A
candidate policy is never substituted into this verification. Imported archives
still require independent exact recipe matching in the existing decoder; file
parsing is not sufficient evidence of a valid baseline.

Only successful `finish` returns a `PolicyComparison`. An incomplete or failed
verifier returns no pair; corrupt cache words, logits, random draws or saved
telemetry cannot be installed as an accepted baseline. Even an empty checkpoint
requires the original final state comparison. Original errors and unwind latching
remain in force. Preparation exposes verification status and reported work,
not unfinished tokens, cache state or executable generation. A failure can leave
unreported bounded work, so partial work rows are not complete-cost receipts.

The reconstructed original owner is discarded; only its immutable replay receipt
is attached to the experiment. The pair then recomputes FROM ZERO. Accordingly,
this profile costs one baseline verification plus two comparison arms. Up to
three generation owners coexist during preparation; no constant-time fork,
direct-KV restart, reduced peak memory or production recovery is claimed.
`baseline_replay` keeps the extra reconstruction cost distinct from each arm's
work and preserves the one original evaluation origin. It does not replenish
source-check/refinement ceilings or mutate a source subsequently held after the
saved checkpoint. Persisted experiments and automatic policy promotion are not
implemented by this API.

## Authored verification

Nineteen integration test functions and one private unit test exercise the
original numerical engine and
fitted codec: stochastic identity with an independent original run; different
probe costs with identical numerical states; candidate-only and baseline-only
alarms; two held arms; exact/zero/one-less position, state-byte and fresh paired
numerical limits; missing candidate audit; stale calls; foreign model policies;
and actual EOS termination. Saved-baseline cases additionally cover segmented,
empty, terminal and archive-import verification; blocked partial finish and late
policy choice; exact reconstruction budgets; held-source preservation; conserved
original telemetry ceilings; and separate reconstruction accounting. The private
unit test corrupts four expected-state fields after a valid control and requires
the original verifier to reject each without releasing a pair. Four compile-fail
examples prohibit executable/mutable arm access and incomplete-preparation escape.

Compilation, rustfmt, Clippy, all new Rust tests and the required RCH gate are
UNEXECUTED in this preparation environment. `rch`, `cargo` and `rustc` are
unavailable. Static review is not a substitute for the required gate, and
historical execution receipts do not qualify this source.
