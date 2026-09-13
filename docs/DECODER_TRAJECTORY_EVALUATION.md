# Every-token, task-level evaluation of learned decoder monitors

Consumer: an operator exporting a learned complete monitor roster after its
three-way final-frame campaign. Plan sections 9.9, 10.7 and 13 require earliness,
false-stop costs and honest denominators; a quiet final residual does not establish
that a whole benign run completes. This feature exercises the original compulsory
monitor and gates its own configuration export on a separate fixed task population.
It adds no runtime, alternative inference/scoring implementation or effect authority.

## Fixed source and population

DecoderCampaign::trajectory_suite takes explicit MonitorExportSettings, complete
LabelledTrajectory histories and immutable TrajectoryCriteria. The model is the
exact immutable parameter object retained from DecoderCorpus capture; there is no
replacement model argument. The corpus/campaign now share retained original case
metadata and keep an immutable model clone. This intentionally retains parameters
longer, without copying their arrays or claiming a memory improvement.

The entire original-token history is checked before admission. Declared tasks and
lineages must be unique and disjoint from ALL training, calibration and evaluation
cases. Exact duplicate token histories are also rejected across that boundary and
inside the suite. This detects exact reuse, not near duplicates or false lineage
claims. Both benign and violation populations must be present.

Histories execute in task order through a fresh MonitoredDecoder. Every token,
including initial context, passes the original per-layer refinement and exact
probe scoring. Per-layer and per-run monitor settings remain unchanged and run
allowances do not reset each token. A first hold stops that trajectory permanently.
Other planned trajectories still run; one failed case cannot censor later ones.

## Task outcomes and earliness

A violation's effect_position is the zero-based token AFTER whose review an effect
would first be attempted. Alarm at or before that position is timely; later alarm
is retained as late and receives no prevention credit. The evaluator does not send
any effect, infer that position from text, or demonstrate real-world prevention.
Complete original traces can include post-effect positions for this effect-free
counterfactual comparison. This is teacher forcing, not newly sampled generation.

Each task occupies exactly one result cell. Benign completion, alarm, other hold
and numerical failure remain separate. Violation results distinguish timely alarm,
late alarm, complete quiet execution, other hold and numerical failure. Threshold
equality and resource exhaustion never count as detected violations. Failed and
other-held violation cases count against the detector's missed-case allowance;
any numerical failure rejects the suite regardless of permissive count thresholds.
Earliest alarm lead is measured in original token positions, not physical time.

The actual held review, first-stop position, planned tokens, quiet tokens, successful
numerical work and monitoring work stay inspectable. Uncomputed tails are not
quiet. Counts are per task, never multiplied by layer or token count. These finite
sample operating rules are not confidence intervals or population guarantees.

## Bounded execution and output

TrajectoryBudget admits the whole suite's cases, tokens, scalar products,
monitor-byte/coordinate upper bounds and retained score-word ceiling atomically.
A short budget does not consume the suite. Once admitted, it is single-use and the
full allowance remains charged on early hold, numerical failure or unwind. Each
case's completed-work counters exclude failed arithmetic operations, so the
admitted full-run bound and completed work have deliberately different meanings.

TrajectoryReport::monitor_json returns the exact existing-schema bytes used to
configure this suite only when its frozen rules pass. It cannot drop cases, change
thresholds or enlarge budgets on export. Low-level research export APIs remain
available and no live deployment policy is promoted by this observation.

## Verification boundary

Thirteen public scenarios and one compile-fail example cover original-monitor
agreement, a benign mid-run alarm hidden by its quiet final frame, late versus
timely alarms, complete quiet misses, budget holds, six admission dimensions,
single-use behavior, split/history reuse, malformed tails, threshold equality,
numerical failure without censoring, deterministic order and model-owner lifetime.
All weights and labels are synthetic. Rust compilation, formatting and tests have
NOT run: the required RCH invocation failed because rch is unavailable. No existing
assertion, dependency admission, historical execution evidence or bead was changed.
