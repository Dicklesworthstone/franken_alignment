# Sparse KV experiments

## Implemented reference capability

`activation::tensor::kv::experiment` creates capability-free branches over the existing immutable `KvImage`. The experiment owns one pinned image and a bounded history of sparse, exact scalar edits. A fork copies edited records, not the underlying activation arrays. An unchanged control fork is explicit and consumes a branch slot.

Each edit names key or value, absolute token position, stored cache head, channel, expected parent bits and replacement bits. Query-head counts do not substitute for stored-head indices. Expected values are checked against the actual parent branch, including signed zero. The replacement must be finite and exactly representable in the original registered storage encoding; the existing lossless encoder checks this without rounding. Late invalid edits, duplicate cells and foreign branch handles publish no node or retention charge.

The frozen edit scope restricts both the token interval and the K/V sides. Shape, model/capture contract, encoding, source prefix and unselected coordinates cannot be changed by this API. There is no callback, model execution, controller, Permit, resource grant, or conversion into a SourceFrame or KvImage. Scalar inspection and deltas are explicitly experimental data, not claims that a model produced the edited state. External callers that read raw data must preserve this provenance; a malicious host can always lie when supplying a separate trusted capture API.

## Exact rebasing and retention

Lookup follows a bounded immutable parent chain. `rebase` composes its effective edits against the ORIGINAL pinned image, removes only net no-ops, cross-checks point lookups, and publishes a new one-hop data node. Its derivation retains the original source branch and all intermediate interventions. A rebase is not a numerical merge, does not alter siblings, and never refunds or clears historical edit counts.

Every branch handle pins its original image and derivation through owned shared references. Dropping the capture store, original snapshot handle, or experiment builder cannot cause an extant branch to read freed or newer data. This is process-local ownership, not an authenticated storage-retention lease, privacy-deletion protocol, durable object store or garbage collector. No base retirement is implemented or claimed. Rebase shortens value resolution, not retained provenance.

The upper limits are 128 nonbaseline nodes, resolution depth 32, 1,024 edits per ordinary fork, and 65,536 lifetime retained edit records; callers can choose smaller limits. Rebase entries and submitted no-op edits count. The underlying image keeps its original 4,096-position and 1,048,576-value limits. These are logical bounds, not measured peak memory or latency. Forking with no edits copies no model-state arrays; metadata and allocations still cost work. General allocator aborts are not recoverable transitions.

## Qualification and change record

This first increment includes eight unit tests and one compile-fail permission-conversion doctest. Source tests cover sibling isolation, late invalid edits, parent-relative expected values, signed zero, full small-image rebase equivalence, retained provenance, dropped owners, foreign same-number experiments, scoped edits, nonfinite values and lifetime limits.

Rust compilation, formatting, Clippy and tests have NOT run. The session has no configured Cargo/RCH runner. No bead is closed and no earlier execution receipt qualifies this source. The next integration is experimental restore/probe consumption of the sparse branches, using existing implementations rather than adding another writer or numerical oracle.

This is scoped reference progress for FA-090/FA-091 and plan 11.3, 11.4 and 11.7, serving FI-I03/FI-I05. Full native-host branching, resumed continuations, authenticated base leases, durable exact-delta interchange and production dependency admission remain outstanding. The original packet obligations are not reduced to this reference data structure.
