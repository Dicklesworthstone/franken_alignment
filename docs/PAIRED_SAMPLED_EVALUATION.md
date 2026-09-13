# Paired free-running monitor evaluation

Consumer: the experiment operator evaluating an existing fa.decoder-monitor/1
configuration on actual sampled continuations rather than teacher-forced histories.
This implements a bounded experiment at plan sections 11.3, 12.2 and 12.6. Neither
arm holds a publication owner, live actor port, effect permit or production rights.
No numerical kernel, sampler, probe scorer, dependency or runtime is replaced.

## Actual generation and uncensored counterfactuals

PairedRolloutSuite owns one immutable DecoderModel, a completely parsed existing
monitor configuration, fixed prompts, random streams/seeds, protocol and criteria.
Every prompt token runs through the original decoder and compulsory all-layer
monitor. Each continuation token is sampled from each arm's own original logits
with the same original policy and random initialization. While the monitored arm
is quiet, every exposed token, position and full vocabulary logit bit pattern is
compared with the baseline; draw counts must agree. A mismatch fails the case.

The monitored arm uses the ORIGINAL MonitoredSampledDecoder. It stops after its
first hold or failure and never retries, reseeds, lowers a threshold or extracts
its inaccessible held token/logits. Its spent draw and review costs remain recorded.
The unmonitored arm continues to the originally declared stop token or horizon.
Baseline tokens after a hold are independent experimental outputs, not leaked
held-model state. A hold cannot censor future baseline output into a safe label.
A failed baseline cannot establish either safety or prevention, even if the
monitored arm had already alarmed. Later cases still execute and failures remain
in the complete planned-task denominator.

## Explicit simulated release oracle

One immutable bounded set of original-token patterns applies to every task. Only
complete matches in the GENERATED suffix count; prompt text and incomplete patterns
do not. The first match ends at a zero-based absolute token position and declares
where a simulated release would first be attempted AFTER that token's review.
An alarm at or before this position is timely; a later alarm is a miss. Matching
an effect and a stop token at the same position records the effect first. No
external adapter executes, and patterns are not a semantic harmfulness classifier.

The report distinguishes benign completion, benign alarm, benign other hold,
timely alarm, late alarm, quiet miss, other effect-task hold, and failure. Exact
threshold equality, unresolved review and monitoring-capacity exhaustion never
receive detection credit. Each task contributes exactly once. Duplicate task IDs,
lineages, complete prompts and random stream identities refuse within the suite.
These declarations do not authenticate origin or establish statistical independence.

Criteria are frozen before generation. They require positive minimum benign and
effect denominators and timely alarms, with maximum benign stops and misses. If
sampling produces too few effect or benign tasks, evaluation fails; seeds and
criteria are not repaired after looking at outcomes. Accepted reports return the
same monitor JSON bytes, not refitted coefficients or live deployment approval.
The standalone constructor does not assert separation from an external training
corpus; that is a separate integration obligation.

## Cost, retention and verification

All cases, prompts, context windows, vocabulary IDs, patterns, numeric work and
monitor configuration are checked before execution. A persistent nine-dimensional
budget admits both full numerical arms, all potential sampling scans, paired
logit comparisons, pattern comparisons, monitor bytes/probe coordinates and retained
score words. Early stopping does not refund that admission. Actual successful
numerical work, draws and monitoring counters remain separate. No full sequence of
logit vectors is retained: paired comparisons consume the current steps. Bounds
are logical operations/retention, not wall-clock, peak allocation or RSS claims.
An admitted suite is single-use even after an error. Constructing a different
suite is a new experiment, not globally prevented by this in-process API.

Eleven public regression functions cover successful detection with a permitted
benign control, quiet misses, late alarms with a released effect token, equality
and capacity holds, EOS/effect precedence, complete-pattern and prompt boundaries,
nontrivial stochastic agreement with an independently driven original sampler,
uncensored numerical failures, all nine admission dimensions, duplicate origins
and invalid tail inputs. One compile-fail example separates a report from a Permit.
Fixtures use small synthetic models/probes and establish no trained-detector rate.

The required RCH command failed to start (rch absent, exit 127). Rust compilation,
formatting, Clippy and tests have NOT run. Source hashes and committed diffs are
not executable qualification; no Beads state or production gate is changed.
This is an open-loop fixed-numerical-profile experiment, not a tool-environment
simulator, natural-language classifier, power-loss proof or hostile-process sandbox.
