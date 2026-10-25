# Required forecasts in learned publication

Recipe `fa.learned-publication/5` connects independently provisioned forecasts
to `supervise_publication create-learned-generated`. It retains version 4's
finite message continuation, checkpoint selection and native/human review, and
requires a fresh forecast before sampling each message. Finish has a separate
forecast and review at the confirmed final message boundary.

## Select the recipe

Starting from a complete version 4 recipe, set `schema` to
`"fa.learned-publication/5"` and add the absolute normalized path
`"forecast": "/operator/primary/forecast.json"` inside `files`. All other fields
remain required, including `sidecar.priority` and `native_review.round_ids`.

The command and continuation options stay the same:

```text
supervise_publication create-learned-generated CONFIG RECIPE REVIEWER_PROFILE
supervise_publication create-learned-generated CONFIG NEXT_RECIPE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST
supervise_publication create-learned-generated CONFIG FINISH_RECIPE REVIEWER_PROFILE --finish-after-message PREVIOUS_REQUEST
```

The next recipe selects a new request while retaining the same generation and
forecast configuration. A new predictor, changed prediction allowance or recipe
downgrade cannot reopen the existing history. Versions 1 through 4 keep their
existing meaning and reject the extra `files.forecast` field; version 5 requires
it and never falls back to unforecast generation.

## Forecast artifact

The artifact is a closed JSON object with schema `fa.learned-forecast/1`. The
loader rejects duplicate, unknown and cross-mode fields. It reads at most 2 MiB
from a regular file, using the same aggregate `asset_bytes` allowance as the
other primary and native assets. Artifact syntax and original predictor
validation precede weight intake. Binding to the actual imported model completes
before any durable store is opened.

| Object or field | Required contract |
| --- | --- |
| `model_identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation`; exact equality with the imported decoder identity |
| `source` | `kind`, `layer`, `profile`; owned K/V additionally requires `side` |
| `source.kind` | `raw_residual` or `owned_learned_kv` |
| `source.profile` | `tenant`, `model`, `model_generation`, `tap`, `layout_generation`; exact original contract for the selected layer and source |
| `source.side` | Owned K/V only: `key` or `value` |
| `probe` | `id`, `generation`, `weights_bits`, `bias_bits`, `threshold_bits` |
| `registration` | `domain`, `generation`, `policy_generation`, `event_prefix_hex`, `negative`, `at_threshold`, `positive` |
| Each probability pair | `null`, `alternative`: independent event-probability numerators on the original scale 65,536; both must be in 1 through 65,535 |
| `alpha` | `numerator`, `denominator`: the original lifetime error-budget fraction with `0 < numerator < denominator` |
| `stream` | The original numerical source stream, matching the recipe |
| `max_predictions` | Original lifetime prediction allowance, in 1 through 512 |
| `max_prediction_age_ticks` | Positive original prediction age; it does not extend request or source deadlines |
| `publication_stream` | Exact recipe `id`, `generation`, `max_messages`, `max_message_bytes`, `max_total_bytes` |
| `learned_budget` | Owned K/V only: `per_job`, `lifetime`, `max_retained_source_bytes` |
| Each learned work budget | `encoded_bytes`, `probe_coordinates`, `reconstruction_products`, `materialized_values`, `refinements`; original caps and accounting apply |

Coefficient fields contain unsigned 32-bit integers holding the exact IEEE-754
binary32 words. For example, 1.0 is `1065353216` and negative zero is
`2147483648`. Decimal coefficient values, out-of-range words, infinities and NaNs
refuse. Signed zero stays part of the immutable predictor identity. The weight
count must match the actual selected tap: a GQA K/V width is not inferred from
the decoder's residual width. The original decoder ceiling remains 2,048.

`event_prefix_hex` uses bounded lowercase hexadecimal and the original complete
message-prefix registration rules. Each probability pair describes the event
under two distributions; the pair is not required to sum to 65,536. Prediction
work and the lifetime likelihood evidence remain shared across messages and
finish. A zero owned work allowance is a legal refusal policy, not a request to
substitute an exact predictor or acquire new budget.

This is an illustrative raw-residual artifact for the deterministic two-value
test model. Its identities, stream limits, coefficients and neutral probabilities
must be independently replaced for another model or workload; it supplies no
calibration evidence.

```json
{
  "schema": "fa.learned-forecast/1",
  "model_identity": {
    "tenant": 1, "model": 9, "model_generation": 1,
    "tokenizer_generation": 1, "profile_generation": 1
  },
  "source": {
    "kind": "raw_residual",
    "layer": 1,
    "profile": {
      "tenant": 1, "model": 9, "model_generation": 1,
      "tap": 4, "layout_generation": 1
    }
  },
  "probe": {
    "id": 41, "generation": 1,
    "weights_bits": [1065353216, 2147483648],
    "bias_bits": 0, "threshold_bits": 1120403456
  },
  "registration": {
    "domain": 77, "generation": 1, "policy_generation": 1,
    "event_prefix_hex": "41",
    "negative": {"null": 32768, "alternative": 32768},
    "at_threshold": {"null": 32768, "alternative": 32768},
    "positive": {"null": 32768, "alternative": 32768}
  },
  "alpha": {"numerator": 1, "denominator": 100},
  "stream": 5,
  "max_predictions": 8,
  "max_prediction_age_ticks": 60000,
  "publication_stream": {
    "id": 9, "generation": 1, "max_messages": 1,
    "max_message_bytes": 64, "max_total_bytes": 64
  }
}
```

Owned K/V selects `"kind": "owned_learned_kv"`, supplies the actual key or value
tap profile and adds `side`. It also supplies the `learned_budget` object above.
The existing owned source-checker and predictor consume that budget, retaining
their original reports, source bytes and failed work. Artifact metadata are
comparison contracts, not model authentication, training or calibration.

## Runtime and recovery

The supervisor keeps the consistency observer outside the source-only actor
port. Startup pins the complete required predictor and the original terminal
recovery reserve in one first canonical image.

The original prompt completes under its existing monitored numerical rules.
At the first output boundary, the supervisor obtains the original request-bound
raw or owned forecast handle without sampling. It then yields to the normal
deadline, authenticated-stop and fresh policy-source checks before the first
draw. Every later sampled step uses that same checked handle and immutable
forecast deadline. The actor submits only the original request and release kind.

After a confirmed message receipt, continuation records the next request and
the retained numerical cut before acquiring a new forecast. Accepted tokens,
K/V state, sampler history and spent work remain in the original owner. Finish
forecasts the separately confirmed cut without additional inference. A message
forecast cannot satisfy finish. Quiet evidence supplies neither native nor human
approval; each release still needs both original review paths and its own receipt.

Every reopened invocation independently reloads the complete generation and
predictor. The selected raw or owned guarded recovery compares that configuration,
the complete guard/policy/floor requirements and the terminal reserve before
returning fresh observer custody. Pending forecasts retain their request and
work and lose coverage under the existing recovery policy. They cannot be
reforecast or resumed as fresh sampled work.

`--resume` uses the same selected guarded recovery for the original message
receipt; the finish form also requires `--finish-after-message PREVIOUS_REQUEST`.
Terminal receipt recovery performs no new forecast, generation or review and
does not need live policy evidence or native helper assets. It still needs the
independent primary generation and predictor configuration to validate history.

## Qualification

On 2026-10-24, frozen `da8077c7` passed all seven original loaded predictive
functions plus the original plain continuation/publication control: eight passed,
zero failed/ignored, 735 filtered on qualified RCH. The same originals passed
again in the complete 63-function combined selection at `a985f9cd`; those
overlapping executions are not added together. This scoped native reference
evidence does not qualify model calibration or the complete workspace.

The nonpredictive downgrade control calls the original
`open_guarded_with_learned_generation` with the same independently supplied
requirements and generation. The operator's test helper selects predictive
recovery, so calling that helper cannot test a nonpredictive refusal. The native
refusal must preserve canonical and staging bytes; selected raw/owned recovery
must acknowledge exactly one fence and retain numerical work, pending forecast,
coverage loss and the prohibition on resumed sampling. This changes the test
route, not a production recovery rule or predictor configuration.

Eight artifact test functions cover exact original configurations, closed input,
model/tap/stream binding, signed zero, GQA widths and exact resource boundaries.
Seven loaded lifecycle tests use actual original numerical and native reviewer
owners, fresh human keys and confirmed publication receipts for both modes.
They call the command's owner helpers directly without a control listener;
existing socket tests retain their separate execution requirements.
They also cover missing custody, expiry or missing policy evidence before
sampling, owned forecast budget exhaustion, interrupted forecasts, changed
immutable predictor configuration and recipe downgrade.

Preparation-time status: these were authored and independently source-reviewed
regressions, not executed evidence. The prescribed RCH launcher was absent in the
restored workspace. After
the complete source was frozen, the executor disconnected before this batch's
syntax or remote-dispatch attempts could start. At that time, no operator parse,
Rust compile, test or workspace qualification was claimed. The deterministic forecast tables
do not establish detection quality, calibration, model-family generalization or
production readiness. FA-111 and its scoped beads remain in progress.

See [publication configuration](LEARNED_OPERATOR_PUBLICATION.md),
[forecast continuation](FORECAST_TEXT_STREAM_CONTINUATION.md),
[owned continuation custody](OWNED_PRE_OUTPUT_CONTINUATION.md) and
[predictive recovery reserves](PREDICTIVE_RECOVERY_RESERVE.md).
