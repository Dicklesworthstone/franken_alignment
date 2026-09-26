# Source-level prompt counterfactuals

`decoder::experiment::prompt` adds an L7 numerical experiment for plan §12.2:
change one named source span and compare actual generated continuations. It
complements, rather than replaces, sparse KV interventions and learned-policy
comparison. It uses the original learned generator, inference, sampler, codec,
source checks and complete per-position K/V monitor roster.

`DecoderModel::prompt_intervention` freezes the model, original GenerationSpec,
learned policy, source/evaluation identities, per-arm numerical and telemetry
ceilings, and an exact expected-token-span replacement. Insertions and deletions
are explicit; the result must satisfy the original nonempty-prompt/context
rules. An unchanged replacement is a sham control. These are original token IDs,
not text decoded and re-tokenized. This API does not assert that a replacement
is semantically matched, harmless or the sole latent cause of a behavior.
Different lengths change subsequent absolute positions as part of the treatment.

`begin` admits the complete longest declared work of BOTH arms, without assuming
an early stop. Both original constructors must succeed before either executes.
One original sampling policy, seed and random stream are used in both arms.
Both prompt phases finish before continuation starts, so paired draws refer to
continuation ordinal, not absolute position. Outcomes may differ even with the
same random word. The pair is one experiment on one source/evaluation lineage,
not two independent observations and not a new live capture.

`advance(expected_revision)` computes at most one next position in each eligible
arm. Each arm stops independently on its original EOS, horizon, hold or error.
The other arm can finish normally; a censored arm is neither silently retried
nor removed. Generation errors are retained as explicit per-arm outcomes. A
stale call does no work; an unwind poisons the paired owner before computation.
`Stopped` means both arms stopped, not that both completed their continuations.

Results are explicitly experimental tokens, samples, logit arrays and diagnostic
statuses. Held steps reveal no selected token, sample or pending logits. No
original DecoderStep, SourceFrame, mutable arm, cache export, checkpoint or
permit is exposed. Experiments cannot be adopted by the production effect host
or clear a held owner. Ordinary source/authority requirements remain unchanged.

`report` separates the full paired numerical reservation from per-arm actual
reserved/accepted numerical work and reported telemetry. A failed preparation
may have bounded unreported telemetry, explicitly marked incomplete. Per-arm
original caps remain active. Each arm retains its original bounded cache/history;
only the latest paired step is retained by the pair itself. Caller-retained
step snapshots, immutable model/codec parameters and allocator overhead are not
free or claimed as peak-RSS bounds. This is not a global experiment escrow.

Eight new integration tests cover sham equality against original stochastic
execution, a source edit that changes real sampled output under the same random
word, unequal prompt lengths, exact edit preconditions, exact and one-less
paired/per-arm budgets, censored holds, missing audits, independent EOS and
stale calls. Two compile-fail examples reject mutable-arm escape and permits.
The tests use the existing nonzero-attention model/codec fixture.

Source addition only: compilation, formatting, Clippy and all new tests remain
unexecuted. The required RCH gate must run on these exact source changes; prior
receipts do not qualify them. No causal-safety, statistical, detector, production
or roadmap/Bead completion claim follows from these authored controls.

## Fixed-seed campaigns and explicit outcome censoring

`PromptIntervention::begin_campaign` freezes an ordered, duplicate-free list of
at most 64 seeds and one exact continuation-token pattern before starting any
trial. This is in-memory experiment registration, not a durable governance
preregistration service. The original intervention, model, policy, sampling
algorithm/stream and stop rules remain fixed; each named seed replaces only the
initial seed in BOTH arms. The template seed remains unchanged.

The complete schedule's position, decoder-product, vocabulary-score and outcome-
query comparison costs must fit `PromptCampaignBudget` before the first trial.
The original per-arm generation and lifetime telemetry ceilings continue to
apply to each trial. This profile does not share numerical prefix work: every
trial executes its two original generators. It retains at most one pair plus
bounded compact result rows, rather than every trial's caches and logits.

Each `advance` executes at most one paired step or records one refused trial
admission. Slots cannot be replaced, retried or reordered after results appear.
All attempted seeds have result rows, including constructor refusal, a partial
cancelled trial, holds and failed audits. A caught unwind leaves the owner failed
and the started-but-unrecorded slot explicit. Cancellation preserves already
recorded rows, records the active trial as interrupted and leaves later seeds
unstarted. Stale advance/cancel calls change nothing.

The frozen query searches only accepted CONTINUATION IDs, never prompt tokens
or withheld candidates. A matching prefix is `Present` even when a later step
holds, because that occurrence was observed. A nonmatch is
`AbsentWithinCompletedHorizon` only after the arm's declared EOS/token horizon;
otherwise it is `Censored`, carrying the original stop reason. Admission failures
and interrupted trials have no fabricated outcomes. The paired 3-by-3 outcome
table is accompanied by declared, started, recorded, unstarted, admission-failed
and interrupted counts. No success-only denominator or implicit negative label
is returned. Original numerical and telemetry work remain in each trial report;
actual query comparisons are recorded separately from their reservation.

These seeds examine one source/evaluation lineage. Distinct PRNG seeds do not
establish independent tasks, attack coverage, calibrated confidence intervals,
causal necessity or a population escape rate. The API cannot prevent a caller
from running and hiding a separate campaign; it preserves the declared schedule
within this owner and makes no external publication/authentication claim.

Six more integration tests compare every seed with separately run original
generators, exercise a real source-induced output change, censored holds and
failed audits, retained admission failures, all five budget boundaries, duplicate
seeds, frozen questions and partial cancellation. One unit test checks overlapping
patterns and exact query comparison counts; two additional compile-fail examples
reject question mutation and live comparison extraction. All fourteen integration
tests, the new unit test and four compile-fail examples remain unexecuted pending
a fresh successful RCH gate; this addition closes no production or Beads gate.
