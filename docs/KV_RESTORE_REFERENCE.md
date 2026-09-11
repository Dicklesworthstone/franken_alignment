# Exact host-buffer KV restoration

## Implemented consumer

The checked tensor capture path now has a return path. `TensorContract::prepare_restore` stages one token; `KvCapture::prepare_restore` stages a contiguous captured window of both keys and values. The resulting plan owns exclusive mutable borrows of the caller's destination slices. `commit` writes their actual bytes and returns historical write metadata. Dropping a prepared plan writes nothing.

This is bounded, safe, std-only reference data-plane functionality. It does not claim that a running inference host resumed, that device transfers completed, or that an actor's external effects were rolled back. No authority, permit, revocation state or resource rights enter the image or writer. Native restart still requires the complete token/RNG/mask/host state and its qualification campaign.

## Failure and representation contract

All destination shapes, strides, byte extents, absolute token positions, source identities and scalar encodings are validated before commit. Both K and V, across the entire requested window, are staged before either destination changes. A bad value buffer discovered after staging keys therefore leaves both original buffers intact. Commit performs only prevalidated slice writes and consumes the plan; it has no Result-producing operation or callback. This establishes call-level atomicity under safe Rust's exclusive borrows, not OS-crash or cross-device transactional atomicity.

The inverse binary32/binary16/bfloat16 encoding is lossless. It refuses values not exactly representable by the registered scalar type rather than rounding them. Signed zero and finite subnormals survive. The final encoded scalar is decoded through the existing capture decoder and compared bit-for-bit to its source. Nonfinite inputs refuse.

Destination layouts can permute axes or include padding under the original sufficient non-overlap certificate. Only selected head/channel coordinates are written; padding, unselected tokens and other batches remain unchanged. Destination batch placement is explicit. Absolute positions must agree with the source range. Restoration to a later page does not imply that preceding positions were restored.

Separate K/V destinations require exclusive borrows and different declared object IDs. Shared storage uses one exclusive slice and disjoint enclosing key/value layout spans. General interleaved shared layouts remain unsupported. Buffer identities and readiness remain host assertions, not authenticated facts.

## Bounds and tests

The source remains unchanged, including its capture frontier and generation floors. Restoration is bounded by the original 4,096-position and 1,048,576-normalized-value limits. Staging retains one offset and up to four bytes per written scalar; `staged_bytes` reports this logical storage, including struct padding but excluding vector capacity and allocator metadata. Destination buffers remain caller-owned. No peak-memory or latency measurement is claimed.

The initial change adds two scalar unit tests and six public byte-write tests. They cover every finite binary16/bfloat16 word in both byte orders, refusal of rounding, mixed-precision GQA storage, signed zeros, late destination failure, dropped plans, shared-buffer overlap, page-range restoration into permuted padded layouts, stale revisions, missing positions and source-identity substitution. These tests have not been compiled or executed: Cargo and the required RCH runner are unavailable in this session. No bead or production gate is closed.

This is scoped progress toward the data restoration obligations of FA-026 and FA-085, serving FI-I03/FI-I05 and plan 10.6, 11.1 and 11.2. It does not waive the native-host, complete mutable-state, durability, compatibility or executed-negative-test obligations of those packets. Existing capture semantics, test assertions, dependencies and verification gates are unchanged.
