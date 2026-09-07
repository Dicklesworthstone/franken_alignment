# Revision 0.2: the architectural leap, not another plug-in inventory

## The original omission

Revision 0.1 combined the two founding ideas with sensible controls, but treated too many foundations as storage/search/numerical plug-ins. The deeper question is what those systems let us stop doing, or do under a fundamentally better abstraction. Revision 0.2 answers that question in five linked steps.

## 1. Stop paying for the same judgment repeatedly

From SQLite's refinable witnesses and final-effect rules comes **epistemic MVCC**. A judgment is a computation over an exact evidence snapshot. Record what its trusted operators read, including absence predicates and semantics. Opaque helper inference depends on the whole packet. Validate those dependencies at reuse and final publication. An unrelated update no longer requires rerunning every expensive observer; a relevant update can no longer slip through a coarse cache.

This is both an economic optimization and a correctness contract. It remains useful even if learned compression fails to outperform simple codecs.

## 2. Stop moving or copying state the receiver already has

From ATP's object/prefix/journal machinery and FS/SQLite structural sharing comes **receiver-conditioned progressive evidence**. A receiver with a verified model/context/base gets only a valid residual or refinement. A stale base triggers a literal fallback. Independent experiments share immutable bytes but never live authority. Capture and transfer remain bounded, and a useful partial observation does not masquerade as a complete review.

The syndrome module is an experiment source, not an unearned live compression guarantee. Nominal latent dimensions do not establish real byte or latency savings.

## 3. Stop analyzing every agent in isolation

From NetworkX comes **authority graph compilation**: dominators and cuts can identify the boundaries that every modeled effect path must cross. From GraphDB comes **incremental oversight**: changes update findings and supports rather than force complete rescans. Typed projection, deletion, negative evidence and complete frontiers prevent a beautiful but stale graph from granting authority.

The optimized solver and the independent verifier are separate. A cut proof always carries its graph-completeness assumption. A generic dense min-cut implementation is a baseline, not the scale architecture.

## 4. Stop guessing what evidence an observer actually saw

From Markdown's context-bundle commitments comes **exact evidence-view identity**. Bind the source, transformations, redactions, omissions, order and final submitted bytes. From Search's producer identities comes generational compatibility. From NumPy/Torch comes shape-safe numerical processing and decision-sensitive refinement.

The observer can still be wrong, but disagreement becomes reproducible. A codec can still miss an unforeseen feature, but its measured scope cannot silently expand. A search can still be approximate, but it cannot prove absence.

## 5. Stop weakening the contract at deployment

The pure-Rust closed universe is enforced at the dependency graph, not the marketing name. DSR local gates execute the actual source and bind nightly/dependencies/targets/artifacts to receipts. Dry runs and partial matrices do not pass. Release credentials remain outside supervised agents.

## Deliberate rejections

No CRDT authority ledger just because evidence is monotone. No opaque verdict “rebase.” No averaging hidden states and calling it a semantic branch merge. No best-effort repair lease as a security fence. No multiplication of isolated speedups. No wholesale dependency import because a crate belongs to the FrankenSuite. No former Python test result recycled as a Rust test result.

## What is actually in this revision

The main plan is integrated with new normative subsections rather than relying on a separate wish list. The source audit records fixed refs and read scope. The roadmap has 102 packets and the invariant registry 30 obligations. The reference workspace is Rust-only, with a local gate driver and no external packages. Its 20 test functions are source-present and unexecuted here. All production functionality remains gated. See the exact preparation checks in [validation](../artifacts/VALIDATION_REPORT.md).
