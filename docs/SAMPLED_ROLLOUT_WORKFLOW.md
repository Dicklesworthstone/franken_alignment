# Checkpoint-backed sampled monitor experiments

Consumer: the offline experiment operator with a supported checkpoint and an
existing complete monitor configuration. This connects PAIRED_SAMPLED_EVALUATION.md
to explicit files and to the original all-layer probe campaign. It adds no
inference kernel, sampler, predicate evaluator, production permission or dependency.

## File command

The new example takes exactly five explicit paths:

```
evaluate_sampled_monitor_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS MONITOR_JSON ROLLOUT_JSON NEW_MONITOR_JSON
```

Run the example through the repository's required RCH/cargo workflow. It parses
an explicit bounded fa.sampled-rollout/1 plan, reads the supplied monitor file,
and loads the supported numerical profile using DecoderModel::from_llama_files.
No filename, executable, download or tokenizer is selected by the JSON. Model
identity is independently declared by the operator; no weight authentication or
architecture expansion is claimed. Unsupported checkpoint features still refuse
through the original loader. The command does not touch live authority journals.

The plan names model identity, exact execution context, sampling policy, every
prompt/random stream/seed, maximum continuation length, explicit stop tokens,
complete generated-token effect patterns, all acceptance criteria and all nine
work budgets. There are no omitted-field defaults. Unknown/duplicate fields,
invalid IDs/tokens, duplicated tasks/lineages/prompts/streams, impossible contexts,
invalid sampling parameters and oversized input refuse. Zero is a valid seed;
full-width u64 seed values are not rounded through floating point. Every complete
suite workload must fit its explicitly supplied budget BEFORE inference.

`tests/fixtures/sampled_rollout.json` is a two-task synthetic schema example,
not a useful pretrained detector evaluation. A model with another vocabulary or
identity needs explicitly appropriate tokens and compatible monitor parameters.
Token patterns define a simulated release protocol only, not harmful intent.

## Reporting and publication

NDJSON includes the fixed protocol, criteria and admitted work, then exactly one
row per planned task and the complete outcome counts. Each task retains original
prompt, random identity/seed, actual independent baseline continuation, only the
monitored arm's released IDs, first matching effect position, both terminations,
first-held layer outcomes, actual successful numerical work, monitoring counters
and draw counts. The output is not a full activation/logit archive or a signature.
The supplied monitor file remains the exact candidate; accepted export copies
those identical bytes, never regenerated or retuned coefficients.

Reporting including its final flush completes BEFORE creating a monitor file.
A rejected evaluation exits 2 and creates no new artifact. Input, budget, inference,
reporting or file errors exit 1. Acceptance and successful file writing exit 0.
Output uses create_new; an existing file, final-component symlink or a file created
between preflight and final flush cannot be overwritten. A failed write/sync
AFTER creation can leave the new file; it is not deleted or silently overwritten.
The file is synced, not its parent directory. No atomic publication, power-loss
survival, authenticated filesystem or hostile parent-path race guarantee follows.
The final report's acceptance describes evaluation, not later filesystem success.

## Original trained-campaign integration

DecoderCampaign::sampled_rollout(settings, plan) uses the SAME retained immutable
model and original evaluated-probe serializer. It rejects task or lineage IDs from
ANY training/calibration/evaluation split, and exact prompts equal to any original
corpus prefix. Intra-suite duplicate checks remain independent. Every fitted layer
must have passed its untouched calibration/evaluation requirements before export;
a failed layer cannot be removed to construct a sampled monitor roster.

The plan cannot provide replacement weights, coefficients or thresholds. The
resulting PreparedRollout owns the original single-use suite and its persistent
budget. A rejected sampled outcome does not initiate a new fit, choose a different
threshold, drop a case or try another seed. Constructing a new experiment is still
possible through trusted application code and is not prevented globally by this
in-process API. Exact-prefix checks do not detect semantic near-duplicates, prove
statistical independence, authenticate labels or qualify a learned detector.

## Verification boundary

Twelve additional regression functions cover full-width seeds, strict malformed
inputs, identity/context/budget binding, typed NDJSON, all training-split origin
exclusions, exact evaluated-byte reuse, rejected-layer exclusion, real temporary
configuration/F32 weight files, rejected sample export, failed report writes and
flushes, no-overwrite creation races, malformed plans before weight access, and
Unix symlink refusal. Together with the core increment this is twenty-three new
Rust test functions and one compile-fail example. Synthetic positive controls
exercise accepted publication; negative cases retain the causal distinction.

The required RCH command was attempted and failed with rch absent (exit 127).
No Rust compilation, formatting, Clippy or test execution occurred. Python source
hash/format-string checks are not Rust execution evidence. Original test bodies,
model/sampler/monitor implementations, dependencies, journal formats, historical
verification and Beads states remain unchanged. No production gate is closed.
