# Complete registered KV-layer sets

`activation::tensor::kv::model` adds an all-or-none consumer of the existing checked host tensor capture, immutable KV images and exact CPU-buffer writer. A frozen ModelKvProfile enumerates every required layer, its distinct K/V taps, formats, head/channel semantics and shared tenant/model generation. Completeness is relative to that explicit inventory, not proof that a serving host declared all of its actual state.

## Capture and ownership

ModelKvCapture requires exactly that layer roster in every append. All layers must name the same next absolute positions, source sequence and token count. Physical page offsets, layouts, precisions and head groupings can differ. No mutable per-layer accessor allows independent frontier advancement.

KvCapture::prepare_append stages the original single-layer capture and retains an exclusive borrow without publishing it. The original append API now immediately commits this same plan. Model-wide appends prepare ALL layers before any commit. A late NaN, destination-capacity error, format mismatch or missing layer cannot publish a partial set of layers or half a K/V pair. Dropping a prepared single-layer append preserves logical state. Vector capacity can grow during preparation; successful byte counters exclude failed work. General allocator aborts are not recoverable transitions.

Buffer-generation floors are additionally tracked across layers, so moving a buffer identity to another layer does not reset its floor. One declared shared storage object requires an identical base slice and generation plus disjoint enclosing spans across the entire submitted layer set. More general interleaved alias layouts refuse. Distinct buffer IDs and temporal coherence remain host assertions; no GPU synchronization, authentication or simultaneous device snapshot is established.

The aggregate ceilings are 128 layers, 4,096 token positions, 16,777,216 normalized values and 4,096 remembered buffer identities. Each original layer retains its own 1,048,576-value limit. Smaller configured position/value budgets are supported. Aggregate limits are checked before reading numerical data; per-layer format/value validation still runs in the original capture implementation. Captures append only new rows. Preparation copies bounded generation metadata, not old activation arrays. Alias checking is quadratic in at most 256 submitted tensor spans, not in their scalar count.

ModelKvImage freezes the entire common cut. An independently assembled set of per-layer images must match every registered layer and its stream, batch, prefix, sequence, revision and count. Missing, extra, substituted and mixed-revision layers refuse. Empty retained prefixes are represented without inventing unseen rows. Snapshot freezing copies token metadata while sharing the existing immutable arrays; cloning the model image shares its metadata as well. No ActorState, Permit, resource budget, revocation floor or qualified restart grade is present.

## Portable complete images

ModelKvDescriptor retains the exact layer roster and all original KvImageDescriptors. Its strict format is the eight-byte FAMKVIM/version-1 domain, big-endian profile ID and generation, a big-endian u32 layer count, then ascending u64 layer IDs paired with the original 156-byte layer descriptors. Descriptor length is exactly 28 + layer_count * 164. Duplicate, zero or reordered layer IDs refuse. Every descriptor must name the same source cut and model space.

ModelKvImage::encode writes that descriptor followed by each layer's original scalar payload in ascending layer-ID order. It reuses the original per-layer serializer; no new floating conversion or alternate interpretation is introduced. Binary16/bfloat16 payloads remain half width. Physical padding is omitted. The maximum descriptor is 21,020 bytes, and the maximum whole image is 67,129,884 bytes. These are format bounds, not compression or peak-memory measurements.

Import takes an independently retained expected descriptor, verifies the complete header and global normalized-value bound before allocating scalar arrays, and invokes the original KvImage decoder for each layer. A nonfinite scalar in the last layer rejects the whole image. Every truncation and trailing byte refuses. Only one additional framed per-layer payload is assembled at a time; this adds up to the existing 4 MiB-plus-header layer limit in temporary byte storage. Output arrays, normalized temporary vectors, metadata and allocator overhead remain additional costs.

Matching the descriptor does NOT authenticate payloads or certify replay freshness. A finite, representable payload edit can decode as different data, and this limitation has an explicit regression. The host must authenticate bytes and retention independently. Import does not manufacture historical host capture receipts or a live capture frontier.

## All-layer restoration

ModelKvImage::prepare_restore requires a destination for every registered layer and the same source interval in each. Destination batch/page placement may differ explicitly while preserving absolute positions. Every layer is staged through the original KvImage::prepare_restore / prepare_pair writer before any buffer changes. A failure discovered in the last value buffer drops all earlier prepared writes. Dropping a successful model plan also writes nothing.

Commit consumes the complete plan and executes only the original prevalidated slice writes. Receipt storage is reserved in preparation. The receipt retains the source descriptor, common interval, aggregate values/bytes written and every per-layer destination receipt. Source images may be dropped after preparation because the staged values and descriptor are owned. This is safe-Rust call-level atomicity in CPU RAM, NOT crash/device/distributed atomicity or a serving-host install transaction.

Cross-layer destinations must have disjoint declared storage objects in addition to Rust's exclusive mutable borrows. Shared K/V backing within a single layer remains supported by the original disjoint-span rule. Cross-layer shared allocations need separately described nonoverlapping objects; this profile does not infer or certify hostile aliases. Only requested coordinates change; padding and unrelated positions are preserved. No token, RNG, sampler, mask, backend scheduler, policy or effect-ledger state is restored.

Staged bytes sum the original writer's offset/scalar metric across layers. They exclude vector capacity, source arrays, caller-owned backing buffers and allocator metadata. The worst-case staging is bounded by the aggregate value limit times the original writer record size, not by the serialized half-width payload. No latency, peak-memory, whole-model continuation or transformer-execution claim follows.

## Qualification and change record

The first implementation increment includes ten public capture tests and a compile-fail permission-conversion doctest. Tests pair complete mixed-precision/GQA capture with omitted layers, mismatched cuts, late nonfinite failures, generation migration, global budgets, shared-buffer overlap, mixed snapshots and dropped prepared appends. They exercise actual supplied host slices, not success callbacks or live model execution.

The second increment includes twelve portable-image/restore tests. Independent literal header and scalar ordering checks accompany full byte round trips, all-layer writes after dropping original owners, late destination failure, dropped plans, missing layers, duplicate destination identities, layer-specific padded page placement, every truncation and header-byte substitution of a selected image, mixed revisions, aggregate header-only limits, empty prefixes and the explicit finite-tampering limitation.

All twenty-two Rust test functions and the doctest remain UNEXECUTED. Rust compilation, formatting, Clippy and the required RCH gate have NOT run. The active environment has no Cargo/rustc/RCH or br executable; no bead is closed and no historical execution receipt qualifies this source. Existing assertions and gates are unchanged. This is data-plane reference progress for FA-024, FA-026 and FA-085, serving FI-A05/FI-I05 and plan 10.6, 11.2 and 11.7. Complete native mutable-state capture, original-token recomputation, qualified continuation, authenticated storage, masks/sampler/RNG state and durable authority recovery remain separate obligations.
