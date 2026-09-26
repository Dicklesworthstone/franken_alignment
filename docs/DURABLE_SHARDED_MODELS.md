# Sharded models in the durable monitored decoder

## Consumer and contract

`FileDecoderConfig::new_sharded` connects the existing SafeTensors shard loader
to the original durable monitored decoder. Supply `FileDecoderShardInputs` with
the raw index and a complete map of literal shard labels to actual bytes, plus
the independently negotiated profile and output-head mode. The constructor
validates the original tensor assignments, coverage, formats and tied-head rules;
it neither opens files named by the index nor computes a token.

The exact index bytes, labels, physical source bytes, head mode, monitor, sampler
and limits participate in immutable configuration equality. Recovery therefore
rejects repackaging as a monolithic archive or changing even index whitespace,
regardless of numerical equivalence. It uses the existing pinned openers and full
numerical replay, not a new cursor, executable-state import or checkpoint store.
Fresh clock/source admission and explicit resume remain necessary. A held run
cannot reroll; acknowledged work and random draws remain owned by the original
reducers. Generation does not authorize message publication.

This is an integration of the existing L1 numerical input and L4 recovery paths
serving the plan's registered restart and host-input contracts (§§11.2, 17.1).
It does not change plan semantics, grant actor file access, create a new runtime,
or increase the model's existing dimensional or parameter limits.

The authenticated native service consumes this configuration through an explicit
version-2 recipe; see [native sharded publication](NATIVE_SHARDED_PUBLICATION.md).

## Bounds and compatibility

The index is bounded by `MAX_WEIGHT_INDEX_BYTES`; the set is bounded by
`MAX_WEIGHT_SHARDS` and the **aggregate**, not per-file,
`MAX_WEIGHT_SET_BYTES`. Journal decoding spends the remaining set allowance
before retaining each source and rejects duplicate/out-of-order labels. Source
names remain the original parser's literal restricted labels. The public
`check_labels` helper runs that same index/assignment preflight before a consumer
opens separately selected files; it does not validate unread tensor bytes.

Existing single-file event tags 0 (independent head) and 12 (tied head) and their
payloads are unchanged. New decoder-event tags 13 and 14 distinguish independent
and tied sharded layouts. After the existing profile fields, their payload is a
bounded index blob, count, sorted `(label blob, source blob)` pairs, then the
existing monitor and sampler blobs. Old readers reject these new tags. No other
journal, numerical-witness or actor-wire representation changes.

`input_bytes()` includes the physical index and all shard bytes, not a fictional
concatenated file. It is not a peak-memory metric. Raw source retention, decoded
parameters, history replay and complete journal replacement remain bounded but
potentially expensive. File authenticity, filesystem isolation, arithmetic
compatibility and external anti-rollback protection remain operator assumptions.

## Source and execution status

Six regression functions use existing synthetic weights and real durable files.
They cover both head modes, complete assignment enforcement, misplaced/missing
sources, signed-zero head conflicts across files, encoding/truncation/order and
length claims, exact-index and monolithic substitutions without canonical writes,
partial continuation versus single-file inference with equal work, and held-run
recovery. The existing tied-head/legacy tests are unchanged. The shared splitter
only partitions test F32 archives; production never rewrites an input archive.

**UNEXECUTED:** the fresh required RCH gate invocation failed before compilation
because `rch` is unavailable (exit 127). Rust compilation, these tests, rustfmt
and Clippy have not run. Source/hash/whitespace checks do not qualify execution,
trained-model behavior, detector quality or production deployment. No Beads item
is closed and no historical receipt certifies this addition.
