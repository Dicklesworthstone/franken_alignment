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

## Runnable operator phases

The existing `supervise_publication` executable exposes two explicit operations:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- stop-admission /operator/config.json 900
cargo run --locked -p fa-reference --example supervise_publication -- drain-stopped /operator/config.json 900
```

`stop-admission` requires no Unix-time observation. It returns success when the
original admission stop is acknowledged, even with charged obligations, and emits
`fa.admission-stop/1` with `drain_attempted:false`, actual native stopped/drained/
fenced state, appended event count, executions and liabilities. It never labels
an admission stop as a newly executed drain. An exact retry appends no records.

`drain-stopped` refuses an active domain and acquires Unix milliseconds only if
the matching stop requires settlement. It uses the original drain JSON renderer
and exits nonzero for remaining obligations. A clock-free stop plus subsequent
drain therefore works when only the original three-event reserve remains.
Missing/expired endpoint outcomes remain charged and are reported as unresolved.

Both commands validate their entire three-argument invocation before CONFIG,
including refusing guard, credibility, witness and reviewer flags rather than
silently discarding them. They use the ordinary source-free recovery profile;
`recover-stop --guarded` and separately reconstructed learned profiles remain
independent routes, not fallbacks selected after failure. No domain is created,
no source or helper is read, and no original submit document is needed. The
Config path/profile and its configured shared clock remain operator assertions.

One monotonic runtime budget starts before configuration ingestion, is checked
before opening and at every original event/Ready boundary, and on both sides of
a required drain-clock call. A slow clock cannot supply a usable timestamp past
that budget. Storage completion remains synchronous; once acknowledged, results
are reported rather than hidden because an earlier operation was expensive.
Clock failure cannot undo the previously acknowledged admission stop. Output
failure reports uncertainty about delivery of the acknowledgment, not absence of
the native stop. No command automatically retries a publication or restores roles.

Seven operator regression functions cover source/clock-free stop, real two-key
liability classes and the original drain renderer, closed actual command routing,
every replay/clock deadline boundary, clock refusal/unwind, write/flush output
failures, and the exact three-event reserve with no-write completed retries.
All prior command tests and the old receipt renderer remain unchanged. Together
with the native increment, fifteen Rust regression functions are authored. The
example and full xtask RCH attempts still exit 127 before Cargo; compilation,
execution, rustfmt and Clippy remain unverified. FA-014 is not closed.
