# Stop admission before the clock is available

FA-014 / plan 16.4 and 25.4. Time is required to resolve expired endpoint
obligations, but loss of the clock must not prevent an operator from stopping
admission. The original combined recovery API remains unchanged. Two explicit
terminal operations now share its fully replayed, exclusively locked cursor.

## Native operations

`FileStopRecovery::finish_admission_stop(request)` consumes a complete original
replay and commits only the original `Stop` and `Fence` events in one canonical
replacement. It has no clock callback, evidence provider, helper, or live-owner
return. It uses the original source-admission, consistency, stop, revocation,
fence, event/byte-limit and recovery-reserve checks. There is no synthetic sweep.
The result is `FileAdmissionStoppedHistory`, an immutable native snapshot, stop
progress and acknowledged history anchor. Sent/unknown/executed effects retain
their charges. Cancelling undispatched work follows the existing native rules.

An exact already-stopped retry confirms the same image without another event,
clock, fence or replacement, including at capacity and while still undrained.
The `appended_events` diagnostic distinguishes that observation from a new
Stop/Fence; an unchanged result does not claim a newly installed fence. A changed
operation or predecessor is refused, not repaired into another stop request.

`FileStopRecovery::finish_stopped_drain(request, clock)` requires an existing
matching stop. It cannot stop an active domain. After complete replay and exact
canonical comparison, it requests a fallible current clock and appends only the
original `StopProgress` event. It never reopens intake, reconstructs approval,
resends publication, or adds a redundant recovery fence. Already-drained retries
skip the clock and write, even at capacity. An explicit Stop/Fence followed by
this drain uses the same three logical event slots as the old combined path,
but two acknowledgments. No recovery allowance is replenished.

Both methods are consumed operations. Ordinary or independently anchored cursor
construction, full-history replay, error/unwind latching and cooperative
cancellation remain the existing APIs. Invalid input, missing event space and
failed preflight leave canonical and pending files unchanged. Drain validates
canonical identity before the clock and again after native preflight. Clock
failure, panic or regressing time cannot undo an acknowledged admission stop.

## Acknowledgment and costs

The new stop has one canonical replacement; a later required drain has another.
Each replacement retains all original file and directory synchronization rules.
Every encoded prefix is admitted before mutation. Result allocation precedes
storage, and the exact successful bytes become the returned anchor. A storage
failure may nevertheless leave a visible canonical result; no candidate result
escapes and an explicit retry must inspect the actual file. The old archive
must remain retained until external retention of its successor is acknowledged.

The drain reports original endpoint results. An executed receipt remains charged;
a missing/expired unknown receipt is not refunded. Admission stopped, fenced,
drained, and acknowledged are not interchangeable claims. No new domain wire
format, executor, authority ledger or production activation is introduced.
Source authenticity, storage isolation and independent anti-rollback custody
remain external assumptions. Decoding, individual events and storage calls are
synchronous; cooperative replay is not a hard-real-time shutdown guarantee.

## Verification status

Eight native regression functions compare the new operations with original
Machine events across reserved/sent/executed/expired liabilities and replay
quanta. They cover exact three-event reserve use, full-capacity retries, active
and incomplete refusals, foreign/stale requests, clock errors/unwinds, canonical
replacement on both sides of clock acquisition, all five storage barriers for
both phases, cancellation at each cut, independent anchors and insufficient
capacity. Existing tests and assertions are unchanged.

These tests are authored, not executed. Required targeted and full xtask RCH
launches exited 127 before Cargo because rch is absent; cargo/rustc/rustfmt are
also absent. Selected-file source/hash checks are not compilation, test or
format qualification. FA-014 remains open pending exact-revision verification.
