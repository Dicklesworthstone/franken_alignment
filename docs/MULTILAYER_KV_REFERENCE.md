# Complete registered KV-layer sets

`activation::tensor::kv::model` adds an all-or-none consumer of the existing checked host tensor capture and immutable KV image APIs. A frozen ModelKvProfile enumerates every required layer, its distinct K/V taps, formats, head/channel semantics and shared tenant/model generation. Completeness is relative to that explicit inventory, not proof that a serving host declared all of its actual state.

## Capture and ownership

ModelKvCapture requires exactly that layer roster in every append. All layers must name the same next absolute positions, source sequence and token count. Physical page offsets, layouts, precisions and head groupings can differ. No mutable per-layer accessor allows independent frontier advancement.

KvCapture::prepare_append stages the original single-layer capture and retains an exclusive borrow without publishing it. The original append API now immediately commits this same plan. Model-wide appends prepare ALL layers before any commit. A late NaN, destination-capacity error, format mismatch or missing layer cannot publish a partial set of layers or half a K/V pair. Dropping a prepared single-layer append preserves logical state. Vector capacity can grow during preparation; successful byte counters exclude failed work. General allocator aborts are not recoverable transitions.

Buffer-generation floors are additionally tracked across layers, so moving a buffer identity to another layer does not reset its floor. One declared shared storage object requires an identical base slice and generation plus disjoint enclosing spans across the entire submitted layer set. More general interleaved alias layouts refuse. Distinct buffer IDs and temporal coherence remain host assertions; no GPU synchronization, authentication or simultaneous device snapshot is established.

The aggregate ceilings are 128 layers, 4,096 token positions, 16,777,216 normalized values and 4,096 remembered buffer identities. Each original layer retains its own 1,048,576-value limit. Smaller configured position/value budgets are supported. Aggregate limits are checked before reading numerical data; per-layer format/value validation still runs in the original capture implementation. Captures append only new rows. Preparation copies bounded generation metadata, not old activation arrays. Alias checking is quadratic in at most 256 submitted tensor spans, not in their scalar count.

ModelKvImage freezes the entire common cut. An independently assembled set of per-layer images must match every registered layer and its stream, batch, prefix, sequence, revision and count. Missing, extra, substituted and mixed-revision layers refuse. Empty retained prefixes are represented without inventing unseen rows. Snapshot freezing copies token metadata while sharing the existing immutable arrays; cloning the model image shares its metadata as well. No ActorState, Permit, resource budget, revocation floor or qualified restart grade is present.

## Qualification and change record

The first increment includes ten public tests and a compile-fail permission-conversion doctest. Tests pair complete mixed-precision/GQA capture with omitted layers, mismatched cuts, late nonfinite failures, generation migration, global budgets, shared-buffer overlap, mixed snapshots and dropped prepared appends. They exercise actual supplied host slices, not success callbacks or live model execution.

Rust compilation, formatting, Clippy and the required RCH gate have NOT run. The active environment has no Cargo/rustc/RCH or br executable; no bead is closed and no historical execution receipt qualifies this source. Existing assertions and gates are unchanged. This is data-plane reference progress for FA-024, FA-026 and FA-085, serving FI-A05/FI-I05 and plan 10.6, 11.2 and 11.7. Complete native mutable-state capture, original-token recomputation, qualified continuation, authenticated storage, masks/sampler/RNG state and durable authority recovery remain separate obligations.
