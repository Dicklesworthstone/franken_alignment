# Complete learned-KV checkpoint audits

## Connected path

`DecoderCheckpoint::audit_learned_cache` now connects the original decoder's complete captured KV state to an independently fitted learned codec, the existing source checker, and automatic residual refinement. The result is a `LearnedCheckpointAudit` retaining compression costs and a `LearnedModelReport`. It does not fit coefficients, choose a different rank, infer omitted taps, or introduce another model or probe evaluator.

`LearnedModelMonitor::new` freezes the exact ModelKvProfile and a nonempty original probe family for EVERY layer and both key/value sides. Missing, extra or profile-incompatible taps refuse. The audit visits the whole captured position interval in position-major, then layer/side order. It has no public row-subset argument that could quietly remove a difficult position. Empty source evidence is incomplete rather than vacuously quiet.

The checkpoint entry point additionally requires the original model/cache profile and token count to agree. `LearnedKvCodec::evaluate_held_out` rejects declared training origins or overlapping source streams before returning the compact image. `CheckedLearnedKv::new` derives every bound from the actual original cache and retains only the explicitly chosen exact residuals. The adaptive monitor then uses the original exact integer score intervals and original source-bound XOR blocks. No source-reported MSE becomes a decision certificate.

This is a finite checkpoint audit, not a live token-release, effect-permit or trained-detector qualification interface. Probe IDs and original task IDs do not authenticate their provenance. The returned evidence establishes the numeric relation to its actual captured source and frozen probes, not whether those probes detect all harmful behavior. A source-checked learned result never becomes a live original SourceFrame or a restored exact actor checkpoint.

## Coverage and shared budgets

NoAlarm is returned only after every planned row returns NoAlarm for all its frozen probes. The result preserves total planned rows, certified-quiet rows, examined/unexamined rows, and the exact blocking row when analysis stops. A row on which no probe pass could be afforded remains unexamined. A row whose coarse pass remains ambiguous is examined but not quiet. An alarm on the last row cannot be hidden by preceding quiet rows; an exhausted budget cannot shorten the denominator and produce complete_quiet.

The full checked base representation is charged once for the entire scan. Each standalone row report continues to obey its own original base-inclusive fixed budget; the private aggregator credits only that same already-paid immutable base and charges each row's remaining costs. Consequently, simply summing per-row encoded_bytes is not the aggregate byte account. Every selected residual, reconstruction product, probe coordinate, materialized scalar and promotion consumes one shared remaining allowance across positions and layers. Earlier expenditure is never reset for the next row.

The model audit admits a report capacity of at most 4,096 rows before numerical probe evaluation. Its byte/product/value/refinement caps use the existing bounded row-monitor budget and each row keeps its own 128-promotion ceiling. Logical representation access is distinct from all retained evidence storage and initial source-checking work. The source-check report includes ALL retained residuals, even blocks that no probe asks to decode. Preparation independently budgets actual compression and source checking; model-monitor resource exhaustion remains an explicit result rather than a claimed clean scan.

Only fitted parameters, checked compact evidence, explicitly retained XOR residuals, and the monitor's actual probe roster survive in the returned audit. It does not secretly keep a DecoderCheckpoint or its original KV arrays. Exact residual retention can itself approach or exceed original storage, and the report exposes that cost. Per-row immutable observations do retain metadata and materialized selected groups; encoded size is not peak RSS or a latency measurement.

Calling this whole-prefix audit after every new token would cause cumulative quadratic source traffic. This implementation does not claim to be an incremental capture pipeline or to erase that cost. Block/stream deployment, host observation authentication, nonlinear-codec qualification and wiring to live release/authority gates remain separate work.

## Source scenarios and execution status

Seven additional Rust test functions exercise complete quiet with one shared base charge; a last-layer/last-position alarm in KV produced by the actual original decoder; absent residuals versus an exhausted refinement allowance; missing/wrong roster and truncated row-report capacity; one global promotion allowance across multiple positions; original preparation limits and held-out identities; and owner destruction after a completed audit. The numerical alarm control uses the original SourceFrame's lossless block and the original exact probe evaluator, not a new float score. Another scenario gives repeated rare-but-benign rows a limited versus sufficient refinement budget and requires different coverage outcomes without changing the probes.

Together with ADAPTIVE_LEARNED_MONITOR.md, this adds fifteen Rust test functions and two compile-fail examples. Existing decoder, learned fitting, source-checking, numerical probe, and exact residual implementations are unchanged. No dependency or unsafe code was added.

The combined command was attempted:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_monitor --test learned_model_monitor
```

It failed before compilation with `rch: command not found`, exit 127. New Rust compilation, test execution, doctests, formatting, Clippy and revision-bound qualification remain unverified. These are test sources, not passing execution evidence. No Beads task or production gate was closed, and no trained serving model or independent harmfulness labels were evaluated in this session.
