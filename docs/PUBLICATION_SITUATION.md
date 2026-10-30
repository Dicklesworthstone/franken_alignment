# Replay-verified publication situations

This scoped FA-136 increment serves plan §17.8 and FI-A02/FI-A18: an operator can
identify pending work and unresolved publication liabilities from one original
journal cut, without opening a recovered owner or performing any effect. It is
not the full fleet situation/planner/address-resolver implementation.

## Native capture and read-only replay

`FileOversight::situation()` extracts a healthy live owner's acknowledged state.
`FileOversight::begin_read_situation(directory, profile, optional_anchor)` reads
one bounded canonical image and returns `FileSituationReplay`. Advance it by
1–64 original events per call, then consume it with `finish`; `read_situation`
is the synchronous convenience operation. Zero event budget yields without work.
The cursor exposes progress but never a partial situation. Every event, including
an unrelated suffix after the interesting effect, must pass the original machine.
Failure or unwind permanently disqualifies that reconstruction.

Reads can run beside a live writer or after a crash. They neither acquire its
writer lock nor fence, clean staging, observe time, read evidence, call helpers,
reconcile outcomes, refund rights or supply recovered roles. Replaying an event
uses the original in-memory publication machine, not an external send. The
profile is independently supplied; a file cannot choose its own bootstrap.
Unsupported histories needing separately bound numerical recipes fail closed,
with no plain-history fallback.

Optional `FileHistoryAnchor` custody checks the exact original prefix against
the same image before replay. A valid append-only successor is accepted; an older
cut or equal-counter fork is not. The report names the checked prefix separately
from its end revision. An unanchored suffix is not authenticated by a prefix,
and replacing both the journal and independently retained anchor defeats this
protection. No signature, source-authenticity or power-loss qualification is
claimed.

## One historical basis, bounded immutable pages

A finished `FileSituation` yields `Knowledge<FileSituationPage, FileSituationBasis>`.
The basis binds the complete authority scope, original journal revision, original
control sequence, recorded clock observation and capture origin. Journal revision
and control sequence are not interchangeable. A recorded tick is historical,
not current time or a fresh source lease.

The origin distinguishes `AcknowledgedOwner` from `CanonicalImage`. An ambiguous
replacement can be visible on disk without being acknowledged to the writer:
a poisoned owner refuses live capture, while an independent canonical read may
report the visible image with its weaker origin explicitly retained.

Each page has 1–64 entries (or one complete empty page for an empty history).
Continuation tokens are process-local, non-authorizing data branded to the exact
immutable report. They cannot cross captures, even equal-revision captures or
forks. Repeating a token on the same report is read-only and deterministic. A
writer may advance after capture without changing any of its remaining pages.
The immutable report retains bounded attempt metadata, not the full journal,
source snapshots or helper sessions after replay finishes.

## What is reported

Whole-cut accounting is repeated on every page: available, reserved and charged
units, policy/dispatcher epochs, suspension and recorded stop status, execution
count, total attempts, retained actor-request count, and pending/unresolved/
terminal attempt counts. Page boundaries do not shrink these totals.

Each attempt includes its original stage, exact target identity, requested units,
payload byte count, proposal epoch and deadline. Payload bytes, witnesses,
policy literals, helper inputs, votes, credentials and permits are not exported.
This remains a privileged supervisor surface, not a redacted actor protocol.
Retained requests refused before any attempt are counted but are not invented as
attempt records.

Unresolved outcomes come first, followed by pending attempts, then confirmed,
denied or cancelled attempts. IDs break ties. This is deterministic lifecycle
triage, not planner-estimated information gain or an instruction to perform a
remedy. Dispatching, Unknown and IrrecoverablyUnknown all count as unresolved.
Abandoned uncertainty remains charged; it never becomes a confirmed failure or
success. Only the original ledger and endpoint receipts establish their outcomes.

## Verification status

Ten native regression functions and two compile-fail examples accompany this
increment. They exercise real two-key publication, unknown abandonment without a
refund, whole-cut pagination, no prefix escape, immutable captures, equal-counter
fork/rollback refusal, bad suffixes, a visible unacknowledged replacement,
empty history and malformed input. Positive cases use the original owner and
canonical journal, not fabricated success states.

Compilation, runtime execution, formatting and Clippy are pending. The required
remote RCH runner and Cargo are absent in this editing environment. Source and
whitespace review do not establish passing tests. No bead, production gate or
full FA-136 acceptance is closed. Source authenticity, deployment authorization,
numerical recipe selection, fleet folds, general address resolution and planner
ranking remain separate obligations.
