# Evaluated probe interchange

`activation::probe::training::interchange` connects the existing all-layer `DecoderCampaign` to the existing `fa.decoder-monitor/1` consumer. It serves the original-token elicitation/signature baseline in FA-109/FA-110 and plan 10.7/12.6. It is configuration export, not independent qualification, authentication or live policy promotion.

`DecoderCampaign::monitor_json` first requires the entire original layer roster to pass its untouched evaluation rules via `probes()`. One failed layer prevents all configuration export. A caller supplies only the new monitor generation and explicit local/global refinement settings; coefficients, bias, threshold and probe identities come exclusively from the retained fitted/evaluated results. No retuning, fallback thresholds, omitted layer or replacement fit is accepted. The exported model/tokenizer/numerical identity is the campaign's declared profile.

Each ladder is validated by the original `RefinementMonitor`, and output uses the existing monitor schema. Binary32 coefficients use round-trippable decimal formatting. A bounded string writer enforces the selected byte ceiling on each append, and the finished output is checked against the existing consumer's parser bounds. Source prefixes, training/evaluation labels, lineage and RNG state are not exported with the runtime coefficients. The full campaign object remains the report; the configuration alone is not evidence that an independently governed campaign happened.

`save_monitor_new` fully encodes before exclusive file creation, writes and syncs a new file, and cannot replace an existing path. Unix files request 0600. A write/sync failure can leave incomplete or complete bytes and is reported without deleting that evidence. This does not promise parent-directory durability, crash-atomic publication, authenticated storage, race-free hostile-path traversal or isolation of an operator that already controls model files. Input/output locations must be operator-authorized. Reloading the resulting file uses the unchanged `MonitoredDecoder::from_json` or monitored checkpoint CLI.

## Existing campaign plans and actual files

The second increment consumes the concurrently implemented `decoder::plan::ProbeRunPlan` directly. It does not introduce another training-request schema, estimator, parser or optimizer. The plan owns its explicit original-token cases, origin/split assignments, fixed per-layer fit/calibration/evaluation policies, export settings and persistent allowances. Its existing estimator preflights both complete stages before inference and cross-checks its work against the captured corpus. Repeated execution does not refill the original allowances; an admitted numerical failure consumes its original capture allowance.

`interchange::files::read_training_plan` reads an explicitly selected regular file under a configurable cap no greater than the existing 4 MiB plan limit. It rejects visible symlinks/nonregular files, bounds allocation and reads to limit+1, and invokes the original strict parser. `load_training_inputs` parses the plan before opening config or weights, then uses the existing configured streamed SafeTensors loader and full-plan estimate against the actual model. Shape-dependent case validation occurs after model loading. No filenames are obtained from the plan, network download or executable deserialization occurs, and no original decoder/campaign implementation is replaced.

The library distinguishes plan-format failures, byte-limit refusals, nonregular paths, operation-specific I/O errors, checkpoint-loading failures and full-campaign admission refusals. Errors omit supplied paths and private file contents. Inputs and all path ancestors must be stable and operator-controlled. Metadata checks are not a race-proof sandbox. Numeric model IDs remain declarations, not authenticated parameter commitments. Once loaded, model and plan own their data, so removing source files cannot change a running campaign. Logical limits are not peak RSS or bounded wall-clock I/O, and this synchronous reference path is not native runtime qualification.

## One end-to-end command

The concurrently implemented `examples/train_decoder_probes_from_checkpoint.rs` is the consumer. This increment routes that existing command through the shared file loader and removes its duplicate private plan-file reader. Its output schema, execution loop, four arguments and all five existing tests remain unchanged. No competing training command is added.

```text
train_decoder_probes_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS CAMPAIGN_JSON NEW_MONITOR_JSON
```

A synthetic fixture invocation after qualified compilation is:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference --example train_decoder_probes_from_checkpoint -- \
  crates/fa-reference/tests/fixtures/decoder_llama_config.json \
  crates/fa-reference/tests/fixtures/decoder_mixed.safetensors \
  crates/fa-reference/tests/fixtures/decoder_probe_files.json \
  /tmp/franken-trained-monitor.json
```

The destination must not exist. Context, original IDs, thresholds, class rules and budgets come from the existing campaign plan, not CLI guesses. The command executes the original capture, fit and evaluation path, emits NDJSON with every threshold trial and all six confusion cells, and flushes those reports before attempting export. Missing evaluation after failed calibration is null, not zero errors. A rejected campaign exits with 2 without creating a monitor file. A wholly passing campaign uses the same exclusive writer and emits a separate monitor_saved event only after save succeeds. Input, numerical and I/O failures exit 1; success exits 0. No event grants permission or activates a live controller policy.

A broken report sink prevents configuration creation but cannot refund already consumed work. Failure after the monitor file is written likewise cannot roll it back. Existing files are never overwritten, even after a passing campaign. Reports contain aggregate class counts and numerical trials but no raw case token IDs, activations, labels attached to individual origins, RNG state or supplied model weights. Full per-case calibration evidence remains in the library campaign object; NDJSON is a summary rather than its authenticated replay archive.

The fixture uses synthetic weights and repeated toy prefixes under distinct declared origins to exercise algorithms, not to demonstrate actual lineage independence or harmfulness detection. Authentic evaluator labels, genuine origin separation, statistically powered held-out populations and governance remain necessary. Passing finite count criteria is not a trained-model safety claim.

## Verification record

The first increment adds six public tests for config-loader equivalence to directly constructed monitors, exact coefficient/threshold bits, rejection of a failed required layer, byte/roster/ladder limits, shared-budget holds, and no-overwrite file round trips. The second adds four file-loader tests and one unit test for signed-zero/subnormal/extreme binary32 decimal round trips: eleven new Rust tests across these two increments. The existing command's five tests remain intact and are not counted as newly authored. Prepared duplicate corpus/parser/command alternatives were not installed over concurrent implementations.

The file-loader tests compare independently loaded file and memory models through full training/calibration, check malformed plans before weight access, preflight insufficient training budgets without capture, preserve source-deletion/remaining-allowance behavior, and enforce exact file ceilings and selected-path type checks. The export tests consume the emitted bytes through the original monitored decoder rather than a replacement evaluator.

All eleven new Rust tests remain uncompiled/unexecuted. The required `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test probe_monitor_export --test probe_training_files --example train_decoder_probes_from_checkpoint` command was attempted and failed before compilation because rch is absent. Cargo/rustc/rustfmt are also unavailable in this environment. Formatting, Clippy and the full revision-bound gate remain pending. No existing assertion, dependency, numerical implementation or verification gate was weakened; no Beads task or production gate was closed. Concurrent capture/campaign/plan/command work was reused rather than overwritten.
