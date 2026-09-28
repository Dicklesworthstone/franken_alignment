# Native token-corpus codec training

`DecoderModel::begin_codec_training` connects original token inference to the
existing learned-codec fitter and its replay checkpoint. It serves plan 10.5,
10.6 and 10.7: the configured native model produces every training K/V value,
and one frozen, bounded corpus supplies the fit. It is offline L7 work, not
an effect permit, live monitor promotion or detector qualification.

Each input fixes its original task ID, stream and complete token sequence.
All source IDs, stream uniqueness, vocabulary/context constraints, total token
and retained-value counts, and the SUM of decoder-product allowances are checked
before a numerical session is created. Documents are executed in canonical
origin order and each begins with a fresh original cache. No caller-selected
intermediate tensors or fitted coefficients enter this path. Source identity
and independence from other datasets remain operator declarations, not proofs.

Each revision-checked `advance` captures at most one original token. It snapshots
the complete all-layer cache once at document completion, not the growing prefix
on every token. After all documents complete, a distinct advance invokes the
original `fit_with_checkpoint`. The final fit is synchronous and bounded by
its original parameter, scratch and loop caps; token-level yielding is not a
claim of preemption inside that fitter or a wall-clock bound.

Only complete fitting can release the original usable codec and fit checkpoint.
The checkpoint's existing archive encoder, parser and original-fit replay remain
the portability route. No new codec format, coefficient deserializer, numerical
algorithm or authority transition is introduced. Original held-out checks still
reject training task/stream overlap when the codec feeds monitored generation.

Cancellation releases unfinished source ownership while retaining per-document
attempt counts, accepted original work and completed-capture counts. Ordinary
errors and caught unwinds latch the run; stale calls do not advance anything.
A failed numerical step reserves its declared product allowance but may have
unreported partial computation. A failed fit may likewise have bounded work
without a fit report. Neither failure nor cancellation refunds research work.
The entire corpus must fit the original retention bound; this is not streaming
PCA or a global research-budget escrow. Parameter/scratch fit admission happens
after capture and cannot undo that capture cost.

Seven integration test functions use original nonzero-attention inference and
fitting, comparing exact archive bytes with independent per-document recompute
and fitting, then exercising original archive replay and monitored generation.
They cover all partial cancellation cuts, canonical ordering, exact summed
budgets, malformed source/vocabulary/context inputs, original fit refusal and
an actual arithmetic-overflow control. One compile-fail example denies escape
of a partially constructed executable decoder.

Compilation, tests, formatting and Clippy remain unexecuted here: required
remote-only RCH is absent. Small deterministic fixtures establish no detector
quality or production readiness. No Bead or qualification gate is closed.
