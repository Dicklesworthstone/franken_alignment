# Evaluated probe rosters as existing monitor configurations

Consumer: MonitoredDecoder::from_json and the existing checkpoint-based monitored
inference example. This extends DECODER_PROBE_CAMPAIGN.md, not the original
optimizer, exact scorer, decoder, sampler or effect gate.

## Complete evaluated roster

DecoderCampaign::monitors constructs the existing RefinementMonitor objects.
DecoderCampaign::to_monitor_json writes the existing fa.decoder-monitor/1 schema.
Both require every actual layer to pass its frozen evaluation criteria. Neither
accepts a layer subset, replacement coefficient or threshold. No failed layer is
silently omitted. The JSON contains the same learned binary32 weights, bias,
selected threshold and identities evaluated by the original exact scorer.
Serialization delegates to the existing training::interchange implementation,
including its output-byte and consumer-parser bounds. No competing serializer or
file writer is introduced. Model identity is declared metadata,
not weight authentication or authorization to promote a detector.

MonitorExport fixes generation, a uniform fidelity ladder/local per-layer budget
and total run budget explicitly. The underlying interchange still supports
individually configured layers. Export validates through the existing monitor constructor.
Small or zero monitor budgets are not enlarged; imported execution still holds on
budget exhaustion. Statistical evaluation of a final residual does not establish
any task-level false-alarm bound for monitoring every token in a long execution.

## Complete operator training plan

ProbeRunPlan parses fa.decoder-probe-campaign/1 with the existing strict JSON
parser. Every case supplies original token IDs, task/lineage, split and label;
every layer supplies FitPolicy and CalibrationPolicy, including BOTH calibration
and final-evaluation criteria. Identity, execution context, capture/training
allowances and monitor-export settings are explicit. There are no defaults,
file paths, executables, replacement activation arrays or supplied scores.
Unknown and duplicate fields, malformed numeric types, nonfinite parameters,
excessive input, unsupported split/label names and invalid policy contracts refuse.

Before inference, run checks the actual model identity, context, full layer
roster, every token ID, origins, class denominators and both capture and campaign
budgets. The metadata-only training/scoring estimate is compared with the
original sealed-corpus estimate after capture. Budgets are owned persistently by
the plan; retry does not replenish them. Preflight failures leave both untouched.
An admitted capture failure consumes capture allowance but does not fabricate
training work. An admitted campaign failure keeps its full campaign charge.

A passing plan returns all original calibration/evaluation objects. A statistical
failure also returns its reports, but monitor export refuses. Low-level research
APIs can still expose candidate probes; neither this result nor serialized JSON
is a policy-promotion or effect capability. The operator can make repeated new
plans or misdeclare lineage/labels, so this is not governance or provenance proof.

## Source validation status

Seven new integration scenarios cover exact coefficient export, direct/imported
monitor and raw-decoder equivalence, learned sticky alarms, failed evaluation,
whole-plan preflight, persistent budgets, strict configuration and numerical
failure. All source/tests are uncompiled and unexecuted in this environment. RCH is absent;
no historical run or production qualification is promoted. The small JSON under
tests/fixtures/decoder_probe_campaign.json is explicitly synthetic fixture data,
not a detector recommended for any real model or production workload.
