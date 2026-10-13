# Durable import of original native evaluator receipts

Source implementation, 2026-10-13. This continues [native accounting](NATIVE_ANDON.md)
and [the original importer](EVALUATED_ANDON_IMPORT.md), plan section 9.9 and
`franken_alignment-credibility-ledger-dc0.4`. No work packet is closed.

`EvaluatedAndonImport::begin_native` accepts the opaque receipts automatically
emitted by the native broker and an independently retained `NativeAndonBinding`.
Before any append it checks the original file identity, every receipt's complete
scope/protocol/committee binding, and every exact occurrence-window budget.
Changing cost, allowance, window bounds or evaluation period is not silently
accepted just because the deployment/evaluator IDs still match. Empty native
imports still check file identity and the original writer's poison state.

A closed internal source enum shares the existing semantic preflight, progress,
record-count budget, failure latch and original FileAndonLedger publication path
with offline evaluated receipts. There is no extensible trait that lets callers
substitute unvalidated facts, and no second file format, encoder, repair rule,
sync primitive or rights ledger. The old campaign constructor remains available
with its original behavior and unchanged regression assertions.

Native label acceptance and durable import remain separate acknowledgments.
Only the import's synchronized head is durable accounting evidence. An operator
must independently retain the bootstrap binding, native receipts and acknowledged
head; a compact accounting record cannot reconstruct the native assessment's
full provenance. Interrupted imports reopen under that head and retry the same
receipts, including complete uncertain suffixes without duplicate charges.
Partial tails remain diagnostic. Publication is an acknowledged prefix, not an
atomic transaction spanning the native evaluator and disk.

Six new file-backed regression functions drive genuine public native broker
proposal, input capture, commit/reveal, application and evaluator labels into the
original writer. They cover bounded progress/reopen/retry, wrong final source
binding with the same accounting identity, budget drift, empty/oversized groups,
complete uncertainty versus partial tails, and conflicting historical rounds
before the first new append. Fault injection copies bytes emitted by the
original writer, not a substitute codec. These Rust tests are authored, not run.

The source preimage of the original importer was checked by its Git blob hash.
Targeted importer tests, original importer tests and the unchanged full xtask
were attempted through `RCH_REQUIRE_REMOTE=1 rch exec`; execution is unavailable
because `rch` is absent. No compilation, Rust runtime-test, rustfmt, Clippy or
production-qualification claim follows from source review or patch checks.
