# Exact snapshot validation after change-feed retention loss

Plan 7.7 permits exact fallback or refusal when an invalidation index lacks a
complete tail. The native `enable_publication_snapshot_fallback` profile selects
the former for the existing snapshot-based witnesses. It is disabled by default,
requires a configured producer heartbeat lease, and can be enabled only before
any proposal. Existing strict profiles and packet encodings retain their meaning.

The fallback does not reset `through`, copy `observed_through` into it, synthesize
notifications, or label an incomplete history complete. Feed status and ordinary
feed freshness continue to report Incomplete. A separate internal acquisition
marker records that a valid producer identity/time observation occurred in the
current dispatcher epoch, despite missing history. Explicit withdrawal clears
that marker; recovery's original epoch fence makes it stale. Malformed, clockless,
future, expired, rolled-back or conflicting observations cannot activate it.

Only a source-bound, cut-bound input at the EXACT latest observed feed head can
use fallback. A legacy input, absent input cut, lagging snapshot, snapshot ahead
of the observed head, or unavailable index cannot. Original source generations,
exact same-generation contents and metadata, snapshot/control/semantic floors,
and one-capture-per-boundary consumption remain mandatory. Initial binding under
this profile still produces no fresh input or permission.

At authorization, dispatch and first publication the original PublicationJudgment
validates from scratch. No notification-derived skip mask is supplied. All captured
exact-value, absence, empty-range and membership dependencies and the entire
opaque helper input are checked under the original shared comparison budget.
Missing closing evidence, content/semantics drift and budget exhaustion refuse.
The existing policy, review, human, credential, target and deadline requirements
are independent. Fallback never edits a judgment or replenishes effect rights.

This is current-state validation, NOT reconstruction of a lost event history.
It applies only to the presently supported snapshot and whole-input witnesses.
It must not be reused for a future witness whose correctness requires every
intervening event. Producer completeness, identity authenticity, clock truth and
filesystem isolation remain host assumptions. A producer bundle supplies a
coherent local image but does not prove that all real-world changes were observed.

The first increment implements the native gate and eight lease/history tests,
including strict-mode controls, exact source/head matching, time boundaries,
generation conflict, withdrawal/recovery, real gap repair and overflow. Durable
configuration and actual file/driver integration follow as a separate increment.
Rust tests, compilation, formatting and Clippy are UNEXECUTED in this environment.
The required RCH xtask command exits 127 because rch is absent; cargo and rustc
are absent as well. FA-061/062 remain open pending exact-revision verification.

## Durable profile and concrete consumers

`FileOversight::create_with_publication_snapshot_fallback` initializes validation,
change routing, producer freshness and the explicit fallback in the first
canonical image. `enable_publication_snapshot_fallback(revision)` also supports
bootstrap of an existing owner, before any proposal. The original native broker
performs admission. There is no live disable, per-attempt bypass or budget change.
The new selection is a bootstrap-class event, so original recovery-reserve
configuration remains available and reserves are not spent on normal work.

`open_with_publication_snapshot_fallback` pins all three original profiles AND
exactly one fallback selection before cleanup or a recovery write. The original
`open_with_publication_change_freshness` remains the strict pinned profile and
rejects a stored fallback, also before cleanup/fencing. Mismatched pins in either
direction return no owner. Generic open replays the stored selection and the
original recovery fence; it cannot restore old sendable keys or a current lease.
A querying owner can inspect `publication_snapshot_fallback_enabled`; unavailable
owners return Unavailable, not an apparently trustworthy historical selection.

The selected mode is recorded as subtag 3 in the existing publication freshness
event. Other tags and encodings are unchanged. Readers predating that subtag
refuse it instead of interpreting it as ordinary complete-history eligibility.
This is a new versioned policy choice, not an automatic change to existing hosts.
Existing command-line profile schemas remain strict; enabling this increment is
an explicit embedding/host API choice, not a new runnable profile schema.

The existing coupled producer reader, coherent original binder, ordinary
supervised driver and feed-aware atomic completion all consume the native gate.
No alternate snapshot importer, driver, file format, or effect ledger is added.
After an evicted producer window is read, its heartbeat can identify a fresh
current snapshot while its retained feed still reports Incomplete. Each source
capture is tied to that observed head. Successful fallback does not fabricate an
acknowledged change suffix; the status still reports the actual retained gap.
An initial original-evidence binding likewise installs no fresh witness input.

Each effect boundary rechecks the complete supported original witness set. A
new phantom, changed range membership, deleted exact value or changed opaque
profile still fails after dispatch even when its notification has been lost.
Unrelated changes have a positive publication control. Missing closure, expired
producer time, unbound/legacy attempts and exhausted comparison budgets remain
refusals. Explicit withdrawal before acquisition and quarantine after ambiguous
installation remain independent of the fallback. Sealing does not itself refund
rights; receipt reconciliation alone settles them. Previously executed outcomes
remain authoritative on source-free recovery.

Eleven file/native-effect tests cover these positive and negative paths, actual
producer-derived window eviction, initial binding after eviction, bidirectional
configuration pins before cleanup, unchanged permits, failed replacement and
clock unwind. Three helper-socket driver tests use the original atomic completion
and ordinary step paths, including changed inputs between its independent
acquisitions, late bundle loss and budget exhaustion without consuming either
key. One wire regression checks the explicit bootstrap tag and strict framing.
Together with the eight native lease tests, 23 Rust tests are authored.

All 23 tests remain UNEXECUTED. Source checks and remote blob comparisons do not
substitute for the exact-revision RCH verifier, Rust compilation, formatting or
Clippy. This extends a reference capability and does not qualify production
activation, source authenticity, real-world capture completeness or arbitrary
history-dependent witnesses. FA-061/062 remain open.
