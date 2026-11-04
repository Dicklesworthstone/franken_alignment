# Run a learned prompt counterfactual campaign

`compare-learned-prompt` runs a fixed numerical experiment on an original prompt
and one exact source edit. Both arms use the same independently supplied model,
tokenizer, fitted K/V codec, complete monitor, sampling policy and seed schedule.
Each arm performs actual monitored prompt inference and sampled continuation.
The command reports whether a predeclared continuation-token pattern was observed,
absent within a completed horizon, or censored by a hold or failure.

```text
supervise_publication compare-learned-prompt PLAN
```

This command supplies an operator entry point to the existing
[source-level prompt experiment](PROMPT_COUNTERFACTUALS.md). It does not load a
publication configuration, create or open an authority store, acquire credentials,
start helper processes, change a live actor, or issue a publication key. It can
run while an unrelated live owner retains its original store lock.

## Closed plan

The complete command accepts exactly one plan pathname. Live-service options,
history outputs, credibility activation, extra arguments and unknown plan fields
are rejected. Paths must be absolute normalized regular-file selections under
the existing operator input assumptions; the command does not discover assets.
The current entry point inherits the Linux supervisor build profile.

The following is a complete example for independently provisioned assets whose
original tokenizer encodes the prompt as `[120, 121]` and declares token `256`
as a control. Its edit replaces the second prompt token with token `65`; its
question is whether continuation token `65` occurs. These token IDs and small
model limits are illustrative, not defaults or promises about another model.

```json
{
  "schema": "fa.learned-prompt-counterfactual/1",
  "campaign": 71,
  "intervention": 72,
  "runtime_ms": 60000,
  "asset_bytes": 4194304,
  "model": {
    "identity": {
      "tenant": 1,
      "model": 9,
      "model_generation": 1,
      "tokenizer_generation": 1,
      "profile_generation": 1
    },
    "context": 16,
    "stream": 5,
    "evaluation_origin": 201
  },
  "files": {
    "model_config": "/operator/model.json",
    "weights": "/operator/weights.safetensors",
    "tokenizer": {
      "format": "huggingface_raw_bytelevel",
      "path": "/operator/tokenizer.json"
    },
    "sampling": "/operator/sampling.json",
    "prompt": "/operator/prompt.txt",
    "fit_binding": "/operator/fit-binding.json",
    "fit_archive": "/operator/fit.fakv",
    "monitor": "/operator/learned-monitor.json"
  },
  "generation": {
    "max_new_tokens": 2,
    "stop_tokens": [256],
    "tokenization": {
      "input_bytes": 65536,
      "pair_lookups": 196608,
      "heap_pops": 196608
    },
    "decoder_products": 1000000,
    "vocabulary_scores": 1024,
    "telemetry": {
      "compression_source_values": 1000000,
      "compression_encoded_bytes": 1000000,
      "compression_work_units": 1000000,
      "source_check_values": 1000000,
      "source_check_encoded_bytes": 1000000,
      "source_check_reconstruction_products": 1000000,
      "monitor_encoded_bytes": 1000000,
      "monitor_probe_coordinates": 1000000,
      "monitor_reconstruction_products": 1000000,
      "monitor_materialized_values": 1000000,
      "monitor_refinements": 1000
    }
  },
  "edit": {
    "start": 1,
    "expected": [121],
    "replacement": [65]
  },
  "schedule": {
    "seeds": [0, 7, 18446744073709551615],
    "outcome_tokens": [65]
  },
  "budget": {
    "trials": 3,
    "positions": 24,
    "decoder_products": 6000000,
    "vocabulary_scores": 3084,
    "outcome_comparisons": 12
  }
}
```

All integer inputs are exact unsigned JSON integers, including the full `u64`
seed range. The parser preserves integer spelling and rejects duplicate keys;
seeds are unique and ordered. Output integers are decimal strings, so downstream
JSON consumers do not lose precision.

### Model and monitor inputs

The command reuses the original Llama config, bounded weight and tokenizer
readers. `weights` can instead be an explicit
[checkpoint source manifest](CHECKPOINT_SOURCE_MANIFEST.md):

```json
{
  "format": "checkpoint_source",
  "path": "/operator/checkpoint-source.json"
}
```

That original reader supports admitted single or sharded bases with an optional
static LoRA adapter and charges the shared asset allowance. The complete final
model must match the independently selected identities and profile. Native
tokenizer archives and the existing explicit raw-byte, Unicode-17 GPT-2,
Unicode-17 Llama-3, BOS and whole-piece tokenizer formats retain their original
parsers and semantics; there is no format fallback or implicit prompt template.

The prompt file is UTF-8 and is encoded once by the selected tokenizer. Edit and
outcome arrays contain original token IDs. The exact expected span is checked
against that result before weight intake. Empty expected or replacement spans
express insertion or deletion; the resulting prompt must remain nonempty and
fit the original context and generation limits. An unchanged replacement is a
sham control. Different prompt lengths change later absolute positions as part
of the intervention.

`fit_binding` is the independent original source inventory. The fit archive is
verified and its original fit replayed; coefficients are never accepted as an
unverified replacement codec. Its cache profile must match the model. Declared
fit origins and source streams cannot overlap the experiment's evaluation origin
or source stream. The complete original monitor is then bound to the actual
model, tokenizer and codec. A trained binary monitor retains its existing full
artifact verification; a JSON monitor is explicitly supplied coefficients, not
a claim of training or calibration.

Identity numbers and lineage declarations are operator assumptions, not signatures
or proof that assets were collected independently. Preserve the selected plan,
model, tokenizer, sampling file, fit binding/archive and monitor with the result.
The report does not duplicate or authenticate all of those asset bytes.

### Frozen costs and scheduling

The plan is at most 64 KiB. The nonzero shared asset allowance is at most 64 MiB
and includes the plan itself, auxiliary inputs, checkpoint and fit archive. The
runtime is between 1 and 3,600,000 milliseconds. Context and total native
generation bounds remain at most 4096 positions. There are at most 64 distinct
seed slots and 64 outcome-pattern tokens; neither array may be empty.

The campaign constructor admits the entire declared schedule's longest paired
positions, decoder products, vocabulary scores and outcome comparisons before
the first trial. It grants no discount for likely early stopping. Each trial
also keeps the original per-arm numerical and lifetime telemetry ceilings. The
same initial seed is used in both arms, with paired draws ordered by continuation
ordinal after both prompt phases finish. No seed is rerolled or replaced after
its result appears. At most one pair of generators is retained at a time.

A too-small complete campaign budget refuses before inference. A per-arm
constructor refusal is instead recorded in that seed's `admission_failed` row.
An actual held or failed generation retains its original attempted work and
stop reason. The numerical report does not include fitting or tokenizer work in
the decoder totals; tokenization and asset bytes are reported separately, and
the original independently declared fit budget still bounds fit replay.

## Reading the result

The output schema is `fa.learned-prompt-counterfactual-result/1`. It declares
`purpose: "experiment"`, `effect_authority: "none"`, and a `knowledge` envelope
whose known value is the observed numerical result. Its basis explicitly says
that the model is not authenticated and the population is one declared source
lineage. The envelope does not assert that an unobserved outcome is known.

The `knowledge.value` object contains:

| Fields | Meaning |
| --- | --- |
| `campaign`, `intervention`, `revision`, `status` | Original native owner identity and state |
| `deadline_reached`, `elapsed_ms`, `runtime_ms`, `cancelled_at_ms`, `operation_error` | Observed wall-time/cancellation boundary and any native command failure |
| `model`, `sampling` | Declared model/source identity and original sampling policy, stream and template seed; floating policy values are retained as binary64 bits |
| `original_prompt_tokens`, `treated_prompt_tokens`, `edit` | Actual original and edited token sequences and the exact selected edit |
| `max_new_tokens`, `stop_tokens`, `seeds`, `outcome_tokens` | Frozen horizon, completion controls, complete ordered schedule and exact question |
| `limits` | Actual per-arm generation/telemetry ceilings and the declared complete campaign budget |
| `reservation` | Native worst-case reservation for the complete paired schedule |
| `input` | Shared asset consumption and original tokenization work |
| `denominator` | Declared, started, recorded, unrecorded-started, unstarted, admission-failed, interrupted and observed-pair counts |
| `outcome_table` | All nine baseline/treated combinations of present, absent and censored |
| `trials` | Every recorded seed with typed disposition, optional outcomes and the original paired report/reservation |
| `active_report` | Any retained started comparison not yet represented by a recorded row |

Every arm report preserves its native status, attempted calls, accepted prompt
and continuation counts, reserved and accepted numerical work, detailed reported
telemetry, and whether all attempted telemetry was reported. Decoder products,
sampling attempts, source checks and compression work are distinct quantities.

`present` means the exact pattern occurred in accepted continuation IDs, even
if a later step held. `absent_within_completed_horizon` requires an actual native
stop token or completed token horizon. A held or failed nonmatch is `censored`.
Prompt tokens and withheld candidates cannot satisfy the continuation query.
An admission-failed or interrupted trial has null outcomes; a missing row is
represented explicitly in the denominator and, where available, `active_report`.

`finished` means the declared seed schedule is exhausted. It does not mean all
arms completed, all monitors were quiet, or the changed prompt was safe. A fully
reported campaign with censored or admission-failed trials can return a successful
command exit: those are numerical observations, not silently omitted errors.

## Deadlines, interruption and output failure

One wall-time origin covers plan intake, asset loading, fit replay, the complete
campaign, rendering and output. The command checks it between original native
quanta. Expiry during an active campaign cancels that same owner, records any
active trial as interrupted and leaves later seeds unstarted. The resulting
partial report is still emitted, and the command returns an error. An internal
native advance failure similarly retains the original status, active report and
started-but-unrecorded count rather than inventing a completed row.

Rendering or output can finish after the deadline; no further inference is
scheduled after expiry is observed. `deadline_reached` describes the observation
made before report emission. A deadline first reached during delivery is reflected
in the failing command result. This is cooperative cancellation, not a wall-time
upper bound on a single filesystem read, fit operation or model step.

Output failure returns an error and performs no inference retry or automatic
artifact creation. This command is an in-memory experiment, not a resumable
campaign journal. Preserve any complete delivered report and the independent
inputs; invoking a new campaign explicitly incurs new experimental work.

## Scope and verification

This is bounded CPU reference inference over independently selected checkpoints.
It does not qualify a production serving host, authenticate model provenance,
calibrate a detector, demonstrate no-egress operating-system isolation, or allow
experimental state to replace a live actor. Distinct seeds remain one source
lineage; they do not provide independent-task counts, confidence intervals,
population escape rates or a claim that the edit isolated the sole latent cause.

Eight operator regression sources exercise public command dispatch with real
file-backed nonzero attention, paired original-generator comparisons and sham
controls, explicit checkpoint-source intake, closed argument/schedule admission,
span and lineage boundaries, all complete campaign budget boundaries, censored
and failed denominators, deadline interruption and lost output. They preserve an
original live store byte-for-byte. Source syntax is checked separately from the
required RCH compilation, formatting, Clippy and runtime gate, which remains
pending for this addition. No roadmap packet or bead is closed by source review.
