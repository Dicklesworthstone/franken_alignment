# Exact host-buffer KV restoration and portable images

## Implemented consumer

The checked tensor capture path now has a return path. `TensorContract::prepare_restore` stages one token; `KvCapture::prepare_restore` stages a contiguous captured window of both keys and values. The resulting plan owns exclusive mutable borrows of the caller's destination slices. `commit` writes their actual bytes and returns historical write metadata. Dropping a prepared plan writes nothing.

`KvCapture::snapshot` freezes a complete retained prefix into an immutable `KvImage`. The image can be cloned, serialized, decoded against a separately retained descriptor, and restored through that same writer even after the capture store is dropped. This is an implemented data round trip, not a callback claiming that restoration occurred.

This is bounded, safe, std-only reference data-plane functionality. It does not claim that a running inference host resumed, that device transfers completed, or that an actor's external effects were rolled back. No authority, permit, revocation state or resource rights enter the image or writer. Native restart still requires the complete token/RNG/mask/host state and its qualification campaign.

## Failure and representation contract

All destination shapes, strides, byte extents, absolute token positions, source identities and scalar encodings are validated before commit. Both K and V, across the entire requested window, are staged before either destination changes. A bad value buffer discovered after staging keys therefore leaves both original buffers intact. Commit performs only prevalidated slice writes and consumes the plan; it has no Result-producing operation or callback. This establishes call-level atomicity under safe Rust's exclusive borrows, not OS-crash or cross-device transactional atomicity. Allocator aborts are not modeled as recoverable transactions.

The inverse binary32/binary16/bfloat16 encoding is lossless. It refuses values not exactly representable by the registered scalar type rather than rounding them. Signed zero and finite subnormals survive. The final encoded scalar is decoded through the existing capture decoder and compared bit-for-bit to its source. Nonfinite inputs refuse.

Destination layouts can permute axes or include padding under the original sufficient non-overlap certificate. Only selected head/channel coordinates are written; padding, unselected tokens and other batches remain unchanged. Destination batch placement is explicit. Absolute positions must agree with the source range. Restoration to a later page does not imply that preceding positions were restored.

Separate K/V destinations require exclusive borrows and different declared object IDs. Shared storage uses one exclusive slice and disjoint enclosing key/value layout spans. General interleaved shared layouts remain unsupported. Buffer identities and readiness remain host assertions, not authenticated facts. The writer must be provisioned with the correct host-owned buffers; this library is not an actor-facing memory-write capability.

## Immutable snapshots and canonical bytes

Initial snapshot construction copies O(position count) metadata but shares the existing immutable SourceFrame arrays. Cloning the snapshot shares both arrays and token metadata in O(1). Further capture appends and recycling the original borrowed buffers do not change the snapshot. Restoring one copy into independently owned mutable buffers cannot change another copy. There is no mutable branch that silently reuses the original stream identity for new observations.

The image carries the complete registered K/V capture profiles, original scalar encodings and byte orders, stored head/channel dimensions, query-head grouping, source stream/batch, contiguous absolute position/sequence range, and source capture revision. Imported tokens carry source identity and validated values, not a fabricated historical host-buffer capture receipt. The data image is not convertible into a live capture frontier or actor authority.

The binary format has an eight-byte FAKVIMG/version-1 domain and a fixed 156-byte header. Header integers are big-endian. Payload order is token-major, key coordinates then value coordinates, each in its original registered scalar representation and byte order. Physical padding is not serialized. Consequently half-width capture remains half-width in the portable payload; no compression ratio is claimed. Exact length is 156 plus positions times the sum of key and value scalar bytes per position. The maximum is 4,194,460 bytes.

The separately retainable descriptor is exactly the same fixed-size header. Import compares the whole descriptor before allocating tensor arrays. Shapes, count/value budgets, position/sequence overflow, enum tags and exact input length are checked. Every scalar must be finite; a later malformed value refuses the whole image, not a successfully parsed prefix. Truncation and trailing bytes refuse. An empty image represents the actual empty retained prefix, not proof that a model had no unseen cache or permission to restore a zero-filled substitute.

**This format is not an integrity or authenticity mechanism.** A matching descriptor detects metadata substitution, not a finite numeric payload edit. An explicit negative-space regression preserves this distinction: a different finite representable value still parses and restores as different data. A caller needing tamper detection must authenticate the bytes under the admitted storage profile. No digest, signature, secure namespace, durable anti-rollback anchor or qualified restart grade is fabricated here.

## Offline file consumer

`crates/fa-reference/examples/kv_image_restore.rs` reads an independently retained descriptor file and its image, validates them, restores both dense CPU buffers, and writes `keys.bin`, `values.bin`, `descriptor.bin` and `layout.txt` into a NEW directory. It builds no controller, permit, helper or inference host. After the mandatory RCH build/qualification, the example's invocation is:

```text
kv_image_restore <independent-descriptor.bin> <image.bin> <new-output-directory>
```

The descriptor and image bytes come from `capture.snapshot(revision)`, `image.descriptor().encode()` and `image.encode()`. Their retention and authentication are the operator's responsibility. The resulting buffers have contiguous `[1, token_count, cache_heads, channels]` layouts, retain each tensor's original scalar encoding and byte order, and preserve the original absolute starting position in `layout.txt`. It explicitly reports `scope=cpu_cache_values_only`, `authentication=not_established` and `restart_grade=not_established`.

Input reads are bounded, and descriptor/image parsing and RAM restoration finish before creating any output directory. Malformed input creates no output. Existing output directories and files are never overwritten. On Unix the new directory requests mode 0700 and files 0600. Activations and restored buffers remain potentially sensitive plaintext; these modes are not encryption or an export authorization. A trusted local output-parent directory is assumed, not a hostile shared filesystem.

Filesystem writes are separate from the paired RAM commit. An I/O failure can leave partial files; the tool reports failure and leaves the new directory for inspection. There is no fsync, transactional rename, complete-marker or OS-crash durability claim. A successful exit reports only the completed local file operation. The example has no network, subprocess, model-execution or live authority path.

## Bounds and verification

The source remains unchanged, including its capture frontier and generation floors. Restoration and images are bounded by the original 4,096-position and 1,048,576-normalized-value limits. Staging retains one offset and up to four bytes per written scalar; `staged_bytes` reports this logical storage, including struct padding but excluding vector capacity and allocator metadata. Destination buffers remain caller-owned. Import temporarily allocates one normalized scalar vector at a time in addition to retained arrays; metadata and caller-held image copies are not included in the value count. No peak-memory or latency measurement is claimed.

The first increment adds two scalar unit tests and six public byte-write tests. They cover every finite binary16/bfloat16 word in both byte orders, refusal of rounding, mixed-precision GQA storage, signed zeros, late destination failure, dropped plans, shared-buffer overlap, page-range restoration into permuted padded layouts, stale revisions, missing positions and source-identity substitution.

The second increment adds one sharing test, eight public image tests and one compile-fail permission-conversion doctest. They cover scalar-byte round trips after dropping the capture, old snapshots surviving new pages and source reuse, independent writable copies, every truncation and header-byte substitution of a selected image, nonfinite late values, explicit finite-tampering limitations, metadata and budget boundaries, empty prefixes, and exact source identities without fabricated capture receipts.

The file consumer adds four source tests: a mixed-width dense round trip, empty-layout refusal, actual file read/write and no-overwrite behavior, and malformed-input/no-output plus bounded-read checks. Its filesystem tests create and remove only their own exclusively created temporary directories. They are test source, not a report of executed disk restoration.

These twenty-one Rust test functions and the doctest have not been compiled or executed: Cargo and the required RCH runner are unavailable in this session. No bead or production gate is closed. The changes add no dependencies and do not weaken prior assertions or execution gates. Source review checked each new method against the existing tensor/KV APIs; it is not a compiler or gate receipt.

This is scoped progress toward the data restoration/serialization obligations of FA-026, FA-085 and structural sharing in FA-090, serving FI-I03/FI-I05 and plan 10.6, 11.1, 11.2 and 11.7. It does not waive the native-host, complete mutable-state, durability, compatibility or executed-negative-test obligations of those packets.
