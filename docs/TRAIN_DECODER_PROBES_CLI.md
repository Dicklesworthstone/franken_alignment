# Train monitored-decoder probes from actual checkpoint files

Consumer: the operator preparing a complete monitor JSON for the existing
`decoder_monitored_from_checkpoint` example. This closes the file-to-training
workflow using the original checkpoint loader, DecoderCorpus, three-way trainer,
exact calibration/evaluation and evaluated-monitor interchange. It does not
replace their formats or implementations and adds no runtime or dependency.

## Invocation

After the repository's required build/verification flow, the example executable
accepts four explicit paths:

```text
train_decoder_probes_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS CAMPAIGN_JSON NEW_MONITOR_JSON
```

The source lives at
`crates/fa-reference/examples/train_decoder_probes_from_checkpoint.rs`.
`CONFIG_JSON` and `WEIGHTS_SAFETENSORS` use the existing supported Llama-style
profile and weight-file limits, not arbitrary model architectures. The campaign
uses `fa.decoder-probe-campaign/1`; its context and model/tokenizer identities are
explicit and independent of names in the checkpoint. The exact complete schema
is illustrated by `tests/fixtures/decoder_probe_campaign.json`. That file contains
six synthetic test cases and tiny-model budgets; it is not a real detector dataset.

The campaign must include both classes in training, calibration and evaluation,
with disjoint declared tasks and lineages. Original token IDs must already come
from the matching tokenizer. No string tokenization, label inference, dataset
splitting, learned parameters, threshold grid, criteria or budget is guessed.
The final residual from every layer is learned/evaluated for each prefix; this
selected-frame evaluation does not establish long-run every-token error rates.

## Results and failure behavior

Standard output is NDJSON: every calibration threshold trial with its six
confusion cells, each layer's selected threshold and untouched evaluation, a
complete campaign summary, and (only after file-write/sync success) monitor_saved.
Counts are per layer and retain both class denominators, including boundary
holds. A layer with no qualified threshold is explicitly unevaluated with null
counts, not silently removed. Full source-bound scores remain available through
the library's original CalibrationRun and EvaluationReport APIs.

Exit 0 means the entire roster passed its declared finite-sample rules and the
new monitor file was saved. Exit 2 means statistical rejection: reports were
printed but no output file was created. Exit 1 reports configuration, numerical,
I/O or publication errors to standard error. It never deletes, replaces or
truncates an existing output; the original exclusive writer uses owner-only mode
on Unix. Validation and bounded serialization precede creation.

A broken report sink before file creation prevents creating the monitor. A later
write/sync or stdout error may leave the new artifact; the command reports error
and never rolls back fitted work, retries under replenished budgets, deletes the
partial file or claims an atomic namespace commit. Inputs and parent directories
are operator-controlled; regular-file/symlink checks are not hostile-filesystem
containment. Saved configuration is data, not detector authentication, safety
certification, automatic policy promotion or effect authorization.

## Regression source and verification

Five example regression scenarios include actual temporary checkpoint/config
files, three-way training and exported-monitor import with positive quiet and
negative held inference. Others cover evaluation failure without output, refusal
to overwrite an existing artifact, broken output accounting and malformed-plan
rejection before opening missing weights. All parameters/labels are synthetic.
These Rust tests, formatting and compilation have NOT run in this environment.
The required RCH command could not start because rch is absent. No task closes,
production gates or historical verification records are modified.
