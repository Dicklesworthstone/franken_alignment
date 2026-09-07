# Evaluation plan

## Four distinct layers

These are evaluation layers (A through D), distinct from the nine architectural layers L0 through L8 of plan §6.7.

**Layer A: repository integrity.** Parse registries, verify IDs/dependency DAGs, resolve local links and declared reference-test entrypoints, check draft schemas/examples, and run the reference demo. These checks establish internal consistency of the planning artifact only.

**Layer B: reference semantics.** Exercise byte binding, one-use permits, current epochs, integer rights, unknown outcomes, branch isolation, helper-round commitments, and deterministic histories. Seeded histories are falsification tests, not exhaustive exploration or production proofs.

**Layer C: production engineering.** Future Rust differential testing, bounded formal models, deterministic lab campaigns, actual storage crashes, OS bypass tests, host capture/restart conformance, tenant isolation, and adapter-specific idempotency. This layer is not implemented by the current reference suite.

**Layer D: empirical alignment/control research.** Independent adaptive attacks, strong baselines, held-out model/task/attack families, useful task completion, total costs, and statistical uncertainty. No current result is claimed for this layer.

## Mandatory negative campaigns

| Campaign | Counterexample sought |
|---|---|
| Envelope/canonicalization | Duplicate JSON keys, Unicode/normalization ambiguity, hidden payload changes, wrong target identity |
| Authority ordering | Stale epoch, racing revocation, duplicate consumption, restored old permit, failed-over old leader |
| Accounting | Concurrent overspend, delegation duplication, unknown-effect refund, repeated nonexecution refund |
| Capture/codec | Synthetic placeholder state, wrong model space, stale buffer, missing positions, gapped stream called complete |
| Helper congress | Injection, missing reveal, conflicting reveal, membership substitution, common-cause error, cohort overcount |
| Replay | Omitted mutable state, wrong tokenizer/cache profile, unavailable key, false exactness, branch egress |
| Persistence | Crash at every barrier, lost suffix, rolled-back root, corrupt object, mixed encoding identity |
| Privacy | Cross-tenant equality leak, unauthorized graph edge, report asset fetch, trace export without permission |
| Resource pressure | Oversized tensors, decompression bombs, refinement storms, unbounded review, ledger starvation |
| Fleet | Fragmented attacks, privilege laundering, incomplete provenance, stale graph used as authority |

## Baseline ladder

Begin with an exact allowlist and resource broker. Add a strong text/tool monitor. Compare trusted editing or equivalent bounded alternatives where relevant. Add homogeneous and heterogeneous monitor ensembles. Then evaluate activation features, learned compression, progressive refinement, causal interventions, and adaptive observation planning one at a time and in combination.

All comparisons use the same actor capability, effects, attacker information, task set, and budget accounting. A system with a narrower allowed task set must report that utility difference. An observation-only system cannot receive prevention credit for a post hoc alarm.

## Reporting contract

Each run records code/dependency/model identities, hardware, numeric profile, dataset lineage, effect and attacker scope, seeds, scheduling/clock profile, warmup, full resource costs, and the raw outcomes. Report median and tail latency, uncertainty appropriate to the design, and all unresolved/missing cases. Never silently remove timeouts or held actions from the denominator.

For zero-failure IID Bernoulli experiments, the exact bound is `1 - alpha**(1/n)`, with independence and sampling assumptions explicit. Adaptive attack selection is a different experimental object. A held-out attack campaign may establish an empirical result under a declared budget, not a universal adversarial guarantee.

## Performance discipline

Benchmark gate CPU separately from durable end-to-end admission, then report both. Account for all ordered commits, object publication, remote calls, batching delays, and recovery work. For compression, include capture and device transfer, metadata, residuals, decoder cost, and retention. A 16-dimensional latent encoded as large floats is not a 16-byte trace.

The initial targets in [slo.json](../registry/slo.json) are unmeasured proposal parameters. A performance waiver may narrow a claim; it cannot bypass a failed authority or recovery invariant.

## Independence

An optimized engine may not be its own only oracle. The written contract and golden vectors outrank agreement between two implementations copied from the same code. Separate evaluator ownership and dataset lineage are necessary to prevent a project from training itself to pass the only tests it knows.
