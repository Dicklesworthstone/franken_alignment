# Complete native accounting synchronization

Source implementation, 2026-10-13. Extends [native accounting](NATIVE_ANDON.md)
and [native receipt import](NATIVE_ANDON_IMPORT.md), plan section 9.9,
FI-A08/FI-A13/FI-A18 and the still-open credibility-ledger-dc0.4 work.
No qualification gate is closed.

## The actual source, not a selected receipt list

`OversightBroker::andon_source` borrows the complete original native accounting
cut and every receipt retained by that owner. There is no subset constructor.
The source includes all windows, all compact confirmations, every incident and
historical seed accounting even when native receipts for seed records are absent.
Holding this view prevents mutable access to the broker; it copies no authority.

Each native receipt retains its original label-acceptance credibility revision.
The source orders receipts by that revision, not by round ID, occurrence time or
confirmation time. Multiple confirmations can share a clock tick and arrive in
reverse round-ID order. Replaying the same facts in the wrong order can produce
equal false-stop counts and costs but a different `opened_by_round` incident.
Exact label retries preserve their first revision and confirmation timestamp.

## One complete synchronization entry point

`fa_journal::evaluated_andon::native_sync::NativeAndonSync::begin` accepts the
original `FileAndonLedger`, independently retained `NativeAndonBinding`, and
actual `OversightBroker`. It obtains the source itself. It preflights every window
registration and original receipt transition on a private accounting candidate.
The candidate must equal the whole native ledger, not just its counts. This
checks exact confirmations, incident opener and contributors, budgets and seed.
A missing seed, extra target record, wrong occurrence budget, wrong source or
wrong incident therefore refuses before the first append. A seed without native
receipts must already exist in the independently recovered file; it is not
silently reconstructed from unauthenticated compact facts by this operation.

The cursor holds the original writer exclusively and source immutably. It
registers every source window, including windows without charges, then persists
all original native receipts in acceptance order. The existing FileAndonLedger
per-record synchronization remains the only publication path. No new file
format, encoder, storage primitive, native label importer or repair path exists.

`advance(max_records)` bounds original window/confirmation calls. Progress counts
include both classes and exact no-write retries. Zero performs no calls. Pending
work cannot be finished as complete. The failure latch is set before entry,
including unwinding. Full accounting equality is rechecked after the last
acknowledged transition, before Complete is returned. `source_revision` identifies
the frozen native cut, not a filesystem revision or a promise about future labels.

Dropping or failing a cursor does not undo a synchronized prefix. Reopen with the
independently retained original head and use the same native source. Complete
uncertain suffixes replay through the original file adapter; duplicate facts do
not append or charge again. Partial tails remain diagnostic and are not repaired.
Each file call is synchronous: a record-count budget is not a wall-time deadline.

## Boundaries and verification

Complete means the file's accounting equals this frozen native accounting cut.
It does NOT mean that pending/censored native labels have resolved, that native
source assessments are themselves durable, or that a restarted native evaluation
engine has been reconstructed. Compact accounting still does not authenticate an
evaluator or replace full source evidence. Native and file acknowledgments remain
separate, and disk publication remains a prefix, not all-or-nothing atomicity.

Seven new file-backed regressions exercise the actual native broker and original
writer: equal-tick reverse-ID label order, equal-count/different-incident refusal,
bounded interruption and anchored resume, uncharged windows and pending labels,
missing historical seed with a matching positive case, whole-source mismatch
preflight, and complete/partial uncertain suffixes. A compile-fail example checks
that the broker cannot be mutated while synchronization borrows its source.
Original native and campaign importer tests are unchanged.

The required native-sync integration, journal doctest and unchanged full xtask
commands were attempted through `RCH_REQUIRE_REMOTE=1 rch exec`. All exited 127
before Cargo because rch is absent. Rust compilation, runtime tests, rustfmt and
Clippy remain unexecuted. Source/patch checks do not qualify these semantics.
