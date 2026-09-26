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
