# Check complete token trajectories before exporting a learned monitor

Consumer: the existing `train_decoder_probes_from_checkpoint` operator command.
This integrates DECODER_TRAJECTORY_EVALUATION.md into file-to-training-to-export,
using the original loaders, trainer, monitor, scorer and exclusive artifact writer.
No alternate command, runtime, production gate or dependency is added.

## Existing invocation, optional stronger evaluation

```text
train_decoder_probes_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS CAMPAIGN_JSON NEW_MONITOR_JSON [TRAJECTORY_JSON]
```

Four arguments preserve the existing final-frame campaign and its five regression
scenarios unchanged. A fifth file requests every-token evaluation on a fourth,
disjoint population before export. Omitting it does not claim trajectory coverage.
The optional file uses `fa.decoder-trajectory/1`; its full small synthetic example
is `crates/fa-reference/tests/fixtures/decoder_trajectory.json`.

The strict schema explicitly includes model/tokenizer identities, context, original
histories, benign/violation labels, task/lineage IDs, first effect positions,
finite-sample acceptance criteria and persistent suite admission budgets. It
accepts no model paths, executable names, replacement coefficients, thresholds or
monitor-budget overrides. The monitoring settings come unchanged from the original
training plan. Syntax, unknown/duplicate fields, numeric types and bounded file
reads validate before checkpoint loading; model identity, context and vocabulary
validate before training. Cross-campaign lineage/history exclusion and exact suite
costs validate after the original campaign is available, before trajectory work.

`effect_position` is a zero-based token after whose review an effect would first
be attempted. Benign cases require null. Violation cases require a valid position;
an alarm at that position is timely, an alarm later is not. This is effect-free
teacher-forced evaluation of complete supplied histories, not newly generated
sampled continuations or proof that a real external effect was prevented.

## Results and file behavior

The original calibration trials and final-frame evaluation reports remain first.
When that campaign passes, the optional suite emits one `trajectory_case` per task
followed by `trajectory_complete`. Reports include the first stop, alarm lead,
planned/quiet/computed token counts, monitor work, all nine task-level outcome
cells, class denominators and exact criteria identity. Tokens, logits, raw KV and
sampler state are not included. Quiet tails are never inferred after an early stop.

A benign intermediate alarm still fails even if its final residual is quiet.
Late alarms, threshold equality, monitoring exhaustion and numerical failures
cannot earn timely-alarm credit. A failed case stays counted while later cases
continue. Numerical failures always reject the suite; other failures are judged
by its explicit frozen count criteria. It cannot refit, retune, omit a task or
retry a completed suite with replenished allowances.

Only a passing trajectory report can export its tested configuration. The command
also compares those bytes with the existing campaign serializer under the same
unchanged settings before calling the original no-overwrite writer. A statistical
failure prints its complete reports, exits 2 and creates no monitor file. Malformed
inputs, admission failures or I/O errors exit 1; a completed campaign is not rolled
back. Successful evaluation plus file-write/sync completion exits 0.

A broken report sink before publication prevents artifact creation. Later write,
sync or stdout failure may leave the new artifact and remains an error; no deletion,
retraining or replacement output occurs. Existing destination files remain intact.
Operator-controlled paths and regular-file/symlink checks do not establish hostile
filesystem isolation. Output configuration is data, not automatic deployment.

## Verification boundary

Six plan/report/file tests and six additional tests in the existing command cover
strict parsing, profile/token admission, untouched budgets, single-use execution,
complete NDJSON, bounded regular-file input, unchanged legacy output bytes,
intermediate false stops, late alarms, malformed input before checkpoint access,
broken trajectory reports and reused training origins. The original five command
tests and their checkpoint fixture remain byte-for-byte unchanged.

The prior core suite's exact-threshold fixture now uses one balanced fitting epoch:
its zero bias is exact, whereas repeated rounded sigmoid updates need not preserve
perfect symmetry. The threshold assertion is unchanged and this corrects only the
new fixture, not the original optimizer or an existing reference expectation.

Together these increments add 25 Rust test functions and one compile-fail example.
All labels, parameters and histories are synthetic. The required RCH invocation
could not start because rch is absent; Rust compilation, formatting, tests and
qualification have NOT run. Source identity/diff checks are not executable Rust
validation. No original semantic reducer, dependency manifest, historical execution
result, production qualification or br-managed task is changed.

Exact history and declared lineage exclusion do not detect dishonest labels or
near-duplicate tasks. The trusted operator can run new campaigns and overfit this
population; no independent evaluation governance or statistical population-risk
bound is established. The unchanged four-argument research path remains available.
