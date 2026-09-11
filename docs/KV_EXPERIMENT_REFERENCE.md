# Sparse KV interventions, paired restoration and probe comparison

## Implemented reference capability

`activation::tensor::kv::experiment` creates capability-free branches over the existing immutable `KvImage`. The experiment owns one pinned image and a bounded history of sparse, exact scalar edits. A fork copies edited records, not the underlying activation arrays. An unchanged control fork is explicit and consumes a branch slot.

Each edit names key or value, absolute token position, stored cache head, channel, expected parent bits and replacement bits. Query-head counts do not substitute for stored-head indices. Expected values are checked against the actual parent branch, including signed zero. The replacement must be finite and exactly representable in the original registered storage encoding; the existing lossless encoder checks this without rounding. Late invalid edits, duplicate cells and foreign branch handles publish no node or retention charge.

The frozen edit scope restricts both the token interval and the K/V sides. Shape, model/capture contract, encoding, source prefix and unselected coordinates cannot be changed by this API. There is no callback, model execution, controller, Permit, resource grant, or conversion into a SourceFrame or KvImage. Scalar inspection and deltas are explicitly experimental data, not claims that a model produced the edited state. External callers that read raw data must preserve this provenance; a malicious host can always lie when supplying a separate trusted capture API.

## Exact rebasing and retention

Lookup follows a bounded immutable parent chain. `rebase` composes its effective edits against the ORIGINAL pinned image, removes only net no-ops, cross-checks point lookups, and publishes a new one-hop data node. Its derivation retains the original source branch and all intermediate interventions. A rebase is not a numerical merge, does not alter siblings, and never refunds or clears historical edit counts.

Every branch handle pins its original image and derivation through owned shared references. Dropping the capture store, original snapshot handle, or experiment builder cannot cause an extant branch to read freed or newer data. This is process-local ownership, not an authenticated storage-retention lease, privacy-deletion protocol, durable object store or garbage collector. No base retirement is implemented or claimed. Rebase shortens value resolution, not retained provenance. Diagnostic formatting prints ancestor IDs, not a recursively duplicated graph or raw activation arrays.

The upper limits are 128 nonbaseline nodes, resolution depth 32, 1,024 edits per ordinary fork, and 65,536 lifetime retained edit records; callers can choose smaller limits. Rebase entries and submitted no-op edits count. The underlying image keeps its original 4,096-position and 1,048,576-value limits. These are logical bounds, not measured peak memory or latency. Forking with no edits copies no model-state arrays; metadata and allocations still cost work. General allocator aborts are not recoverable transitions.

## Real paired buffer writes

`KvBranch::prepare_restore` materializes only requested rows, shares unchanged scalar arrays, and passes the results to the existing exact `prepare_pair` writer. Changed rows receive one new normalized bit array; sparse overrides cannot mutate the base or a sibling. The temporary synthetic SourceFrames are private implementation details, not exported captured observations. Returned write receipts wrap the original writer metadata with explicit experimental branch provenance.

`prepare_twin_restore` takes a reference branch, a candidate branch, the same absolute window and two separately borrowed destination sets. Both branches must share the identical pinned base instance, not just an equal descriptor or numeric experiment ID. The destination sets must have disjoint declared object IDs in addition to safe Rust's exclusive mutable borrows.

Both legs, including all key/value rows, are staged before a paired plan is returned. A failure in the candidate leg drops the already-staged reference plan without writing either. Dropping the paired plan also writes neither. Commit consumes both plans and invokes only the existing prevalidated slice writes, with no fallible callback between them. This is call-level atomicity in CPU RAM, not crash, device, distributed or native-model restart atomicity. These functions confer no access to buffers the caller did not explicitly lend.

The destination layouts retain the registered original scalar encodings and byte orders. Padded/permuted placement uses the existing layout checks; only selected token/head/channel coordinates change. The image prefix, capture frontier, effect ledger, review history, permits and resource reservations do not participate in these writes. A successful experiment does not promote its buffers into a running actor.

## Paired exact probe calculations

`KvBranch::compare_probe` evaluates the same registered `LinearProbe` at the same stored position in the reference and candidate branches. It uses the original source-checked full-precision codec and exact integer score evaluator. The result carries both experimental bases, probe identity, position, score intervals and categorical outcomes. Unchanged control branches and edits exactly undone recover the original scores without copying those scalar arrays.

A changed sign establishes only the specified linear calculation on the supplied and edited values. It does NOT establish that the intervention is on-manifold, changes a real model's behavior, identifies a unique cause, preserves continuation quality, or improves safety. No helper is rerun or assigned an invented verdict. The comparison is not a ProbeObservation accepted as live evidence, and it cannot issue a Permit. Models, native executors, causal outcome evaluators and sandbox scheduling remain outside this bounded numerical/data-plane implementation.

`copied_values` counts newly materialized changed-row arrays, not codec copies, offsets, allocator capacity or source bytes. `encoded_block_bytes` counts the actual lengths of the two exact codec blocks, not all memory traffic: source checking also re-encodes blocks. Restore `staged_bytes` uses the original writer's offset/scalar staging metric. Neither metric is a latency, compression-ratio or peak-memory benchmark.

Execution is bounded per call, not by a lifetime work ledger. A probe comparison processes two rows of at most 65,536 values each and scans their bounded edit-chain ranges. A restore processes at most the image's 1,048,576 values per leg; a twin can stage twice that. The caller supplies any overall research scheduling and work budget. Repeated failed or successful execution calls do not acquire free production rights, but their cumulative CPU use is not limited by the edit-retention budget.

## Qualification and change record

The first increment adds eight unit tests and the sparse fork/rebase implementation. Source tests cover sibling isolation, late invalid edits, parent-relative expected values, signed zero, full small-image rebase equivalence, retained provenance, dropped owners, foreign same-number experiments, scoped edits, nonfinite values and lifetime limits.

The second increment connects actual paired buffer writes and exact probe consumers and adds twelve public integration tests. These pair successful altered/control writes and exact sign comparison with a late candidate-buffer failure, dropped plans, unchanged controls, mixed-width no-rounding rules, imported-image rebase equivalence, foreign bases/probes, overlapping destination identities, padded partial-window writes, bounds and unchanged live effect reservations. The integration corpus constructs captures through the actual existing tensor APIs. It does not substitute a success callback for capture, restoration or scoring.

Three compile-fail doctests reject conversion of KvBranch into Permit, SourceFrame and KvImage. All twenty Rust test functions and three doctests are source only: Rust compilation, formatting, Clippy and the repository gate have NOT run. The session has no configured Cargo/RCH runner. No bead is closed and no earlier execution receipt qualifies this source. Existing tests and verification gates were not weakened.

This is scoped reference progress for FA-090/FA-091 and plan 11.3, 11.4 and 11.7, serving FI-I03/FI-I05. Full native-host branching, resumed continuations, independently evaluated causal outcomes, authenticated base leases, durable exact-delta interchange and production dependency admission remain outstanding. The original packet obligations are not reduced to this reference data structure.
