# Trained probes for the original learned K/V monitor

`probe::training::decoder::kv` connects original numerical decoder execution to
the learned-cache monitor. A complete labelled population is executed through
the original model; its actual final-token key and value `SourceFrame`s enter
the existing `ProbeCorpus`, optimizer, threshold calibration and exact held-out
scorer. Every layer and both K/V sides must pass before the campaign exports an
original `LearnedModelMonitor`, `LearnedDecoderPolicy`, or the existing learned
publication command's monitor JSON. There is no replacement decoder, optimizer,
probe scorer, learned-cache certificate, or effect authority.

This is the K/V training bridge for plan sections 10.2, 10.7, 10.13, 10.15 and
12.6. It implements part of the learned-monitor and evaluation work represented
by FA110; it does not close the broader trained-detector quality or alignment
research obligations. The existing residual-stream training path remains a
different tap family. A residual probe cannot silently become a K/V probe.

## Frozen population and original captures

`KvDecoderCorpus::estimate` validates all cases before any numerical execution.
Every `LabelledPrefix` has a nonempty, vocabulary-valid, context-bounded original
token history, a nonzero task, a nonzero lineage, one of the three original data
splits, and an explicit benign or violation label. Both labels must occur in
training, calibration and final evaluation. Tasks, lineages and exact token
histories must each be distinct across the whole population. Rejecting exact
history reuse is a conservative cohort rule, not proof that near duplicates,
common ancestors or external sources are independent.

Task IDs become original decoder stream IDs. Cases are retained in canonical
origin order; labels, split assignments and source frames cannot subsequently be
replaced. Capture executes each complete prefix once and takes the actual last
token from every layer's original cache. It checks stream, absolute position,
sequence, cache profile and observed work, while the original `ProbeCorpus`
checks the tensor profile and dimensions. The full immutable `DecoderModel` and
`DecoderProfile`, including the declared tokenizer generation, remain owned by
the corpus and campaign.

Grouped-query attention uses each original key/value tensor contract's cache
width. It does not allocate or fit against residual width or query-head count.
The integration fixture has hidden width 4, two query heads and one cache head;
each of its four layer/side probes therefore has two coordinates.

## Complete admission and fixed denominators

Budgets are persistent objects passed by mutable reference. Each admission
checks every field before subtracting any field. A caller cannot acquire a new
per-case or per-tap allowance from the campaign. Original global ceilings still
apply; no larger optimizer or scoring limit is introduced.

| Stage | Admitted denominator and cost | Failure behavior |
| --- | --- | --- |
| Capture | Every case, every original prefix token, every retained final-token K/V coordinate, and the sum of original decoder scalar products | Malformed populations and insufficient aggregate allowance fail before execution or charge. Once admitted, a numerical failure returns no corpus and restores no allowance. |
| Fit | Original coordinate visits for every mandatory layer/side fit, using only its training split | The complete roster is reserved before the first optimizer step. Hard arithmetic failure returns no partial campaign and retains the whole charge. |
| Calibration | Original exact encoded bytes and coordinates for every calibration case, plus every declared threshold comparison | All candidate results and class counts remain recorded. A tap with no eligible threshold remains a failed result; later taps still run. |
| Final evaluation | Original exact encoded bytes, coordinates and selected-threshold comparisons for every final case at every tap | Final costs are reserved even if calibration will later fail. Evaluated failure keeps its counts and scores; skipped final evaluation is explicitly absent and its planned population remains in the fitted corpus. |
| Monitor export | Every model layer and both K/V sides, with exactly the accepted original probe from each | One failed tap blocks the entire export. Callers cannot replace weights, choose a new threshold, or omit the failed side. |

`admitted_work` and `completed_work` are separate. A failed calibration can have
less completed final scoring while retaining the entire admission. Repeated
origins across taps are one population observed at multiple locations, not
independent trials. Captured coordinate accounting measures the retained final
K/V sources; it is not a peak-memory claim for the original full-prefix decoder
cache or corpus metadata.

The optimizer is the existing bounded full-batch classifier. Calibration is the
existing finite-count threshold selection; it is not probability calibration.
Final labels cannot influence fit or threshold selection. `ScreeningCriteria`
requires demonstrated violation alarms and bounds benign holds. Equality stays
a hold and does not become either a certified alarm or quiet evidence.

## Original policy and operator use

The campaign exposes the retained model through `model()` and the full declared
profile and cases through `profile()` and `cases()`. A normal application first
captures, then fits and evaluates the complete roster:

```rust
let capture_work = KvDecoderCorpus::estimate(&model, &cases)?;
let mut capture_budget = KvCaptureBudget::new(capture_work)?;
let corpus = KvDecoderCorpus::capture(&model, corpus_id, generation,
    &cases, &mut capture_budget)?;

let fit_work = corpus.estimate_campaign(&policies)?;
let mut fit_budget = KvCampaignBudget::new(fit_work)?;
let campaign = corpus.run(policies, &mut fit_budget)?;

// The codec was fitted separately on a disjoint declared source population.
let policy = campaign.policy(codec.clone(), &settings)?;
let generation = campaign.model().monitored_generation(stream, evaluation_origin,
    generation_spec, policy, generation_budget)?;

// Bytes can directly provision the existing learned publication command.
let monitor_json = campaign.monitor_json(&codec, &settings,
    MAX_KV_MONITOR_JSON_BYTES)?;
```

The public types in this example live in
`action::consequence::activation::probe::training::decoder::kv`; labelled cases
and `LayerPolicy` are reused from the parent decoder module. `KvMonitorSettings`
requires an exact tap map and explicit original per-tap/aggregate monitoring,
compression, source-check and inference caps. Original policy constructors
validate these values. All and None residual retention can enter JSON; selected
Heads are supported by the programmatic original policy but refused by the v1
JSON exporter because that schema cannot represent them.

`policy` checks equality with the original codec's complete cache profile. It
also rejects overlap between codec fit source keys and any probe-case lineage,
or between codec source descriptor streams and any probe-case task. This bridge
uses codec source keys as **declared lineage IDs**. Numeric IDs in the two
namespaces are declarations; they are not signatures or authenticated source
lineage. Cache descriptors cannot prove token-history or hidden-origin
independence. The codec cache profile does not contain tokenizer generation;
that identity must remain bound through the independently retained original
model and operator recipe. Profile equality alone does not authenticate external
model parameter bytes.

The bounded exporter emits the unchanged `fa.learned-kv-monitor/1` schema. Probe
identities, every coefficient and the selected threshold come from the original
accepted probes. Finite binary32 decimals preserve their original bits. Output
is admitted incrementally and checked against the actual operator's 2 MiB,
depth-10, 262144-item parser limits. The existing operator `MonitorInput` loader
binds these data to its independently imported model and codec.

**The v1 JSON has no model, tokenizer, training-corpus or lineage identity
fields.** It is provisioning data, not a portable authenticated training report.
Retain the campaign's model, profiles, cases and results separately and provision
the intended model/codec assets through the existing operator recipe. Importing
the JSON does not prove its labels, model provenance or held-out acceptance.
Neither the campaign nor a numerical quiet result converts into `Permit` or
automatically promotes a live policy.

## Acceptance boundaries

`tests/decoder_kv_probe_campaign.rs` exercises original source identities and
exact scores, GQA dimensions, all three population denominators, aggregate
admission, failed-fit retention, complete export and original sampled behavior.
The positive fixture trains all four probes from actual K/V; benign seed 4
produces sampled token 0 and an audited stop token 2. With the same model,
sampling policy, trained roster and codec, seed 5 produces a candidate violation
token 1 that is held before its cache, logits or sample can be released. Quiet
events contain source-checked evidence for all four required rows.

The causal training twin zeros only the second layer's value projection. The
other three taps retain their original accepted results, while the unseparable
tap blocks every complete export. Reversing only final evaluation tokens leaves
fit and calibration unchanged and produces retained failed final counts.
Additional paired cases cover wrong original cache profiles, declared codec
origin overlap, exact-history/split reuse, every budget dimension, arithmetic
overflow, immutable case ordering, and exact/one-byte-short JSON limits. The
learned command's separate roundtrip test consumes the bytes through its actual
unchanged parser and exercises the loaded policy.

These synthetic fixtures demonstrate that the implemented numerical path is
reachable and depends on learned signal. They do not establish generalization,
label correctness, model alignment, production latency or safe external effects.
Execution results and the qualified build revision belong in the corresponding
central verification record; authored tests alone are not a passed gate.

## Executed validation

Qualified `nightly-2026-09-08` passes all twelve tests in
`decoder_kv_probe_campaign`, all three `trained_monitor` tests inside the actual
`supervise_publication` example, and the `KvDecoderCampaign` compile-fail check.
The example tests consume emitted JSON through unchanged `MonitorInput::decode`
and `bind`, compare every nondefault resource field and exact coefficient score,
and exercise real sampled quiet/hold decisions plus a last-tap budget refusal.

The final surrounding run passes 48 public integration cases. The complete
learned-command selection passes 21 of 23 cases; the two actual Unix-listener
cases fail with the observed environment EPERM at listener creation. They remain
unskipped. Together with both sealed predictive-recovery compile-fail checks,
this yields 69 passing runtime tests and three passing compile-fail checks.

The original full `xtask check` accepts the qualified compiler identity, then
refuses the reviewed source snapshot because it omits numerous existing and new
files and retains stale existing digests. Its frozen records and enforcement
remain unchanged. Formatting, Clippy and full workspace qualification have not
been established by these targeted runs.
