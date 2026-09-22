# Targeted full-input/two-key settlement

## Status and capability changelog

2026-09-22: source implementation of
`FileOversight::cancel_and_resolve_request`, its supervised-driver integration,
and atomic driver pending reconciliation. Seventeen regression tests are authored:
ten owner tests and seven real-file/Unix-socket driver tests. Tests, compilation,
rustfmt and Clippy are UNEXECUTED. Both targeted attempts failed before execution
(`rch: command not found`, exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference settlement
```

The historical execution receipts do not qualify this change. No broader bead,
production perimeter, cryptographic receipt or remote-provider gate is closed.

## Existing consumer and effect boundary

The original durable full-input/two-key owner previously exposed only
`cancel_request`, which deliberately cannot settle dispatched work. A supervisor
can now select a durable request ID and cancel its original undispatched
reservation or atomically seal its dispatched key and reconcile the ORIGINAL
endpoint receipt. No separate receipt ledger, arbitrary event import, new
canonical event, new dependency or automatic/human key reconstruction is added.
This implements part of the plan's FA-014 recovery/unknown-reconciliation and
FA-INV-005/006 resource-conservation contracts within the existing journal sink.

`Time` and `Cancel` or `Seal` are composed by the same private transaction used
by checked publication. Every prefix retains original canonical admission,
recovery-capacity checks, source admission and consistency preflight. All effects
of speculative replay are in RAM; only one canonical journal replacement becomes
visible. Both original event slots must fit. An error before replacement leaves
the old cut unchanged. Storage failure latches the owner unavailable before
entering I/O, returns no candidate status/refund, and requires exclusive recovery.

Execution wins a cancellation race: a retained real execution becomes Confirmed
and remains visible and charged. A native nonexecution receipt becomes
ConfirmedNotExecuted and seals all delayed deliveries under the same key. A
status miss, expired retention, stale observation or unknown outcome is never
nonexecution evidence. IrrecoverablyUnknown remains charged and refuses.

The original request book supplies status and request-local generation.
`request_resolution` supplies accepted terminal endpoint evidence where one
exists. Refused and already terminal requests have read-only historical retries,
including with stale revision/tick arguments; those arguments do not advance the
clock. A storage-faulted owner refuses even that path. No unrelated request is
cancelled, re-reviewed, fenced or resent. After reopen, old keys remain withdrawn
and a fresh trusted tick is still required for active settlement.

This operation does not repair an interrupted evidence source or fetch helpers,
credentials, or new approvals. Existing source-admission rules explicitly allow
clock/cancellation/sealing while keeping positive publication closed. The actor
wire retains its weaker request-cancellation contract; this is supervisor work,
not a new actor-facing command or publication capability.

## Authored regression coverage

Tests use actual durable requests, original whole-input congress rounds,
independent human-role approval and original endpoint publication. They cover
reserved cancellation, sealing before delayed publication, a real execution with
lost acknowledgment, Unknown and post-reopen settlement, preservation of another
request, source-interruption latching, stale predecessor/time, exact receipt
retention, exact/one-under event capacity, and terminal retries after storage
faults. Every original replacement barrier is exercised for reserved, dispatched
and already-published requests: disk must contain the old or complete cut, never
a partial refund; retry through the failed owner must refuse.

The tests are authored, not passing evidence. They do not establish provider
identity, wall-clock correctness, general process isolation, disk-space reservation,
power-loss durability, or atomicity over independent external effect sinks.

## Supervised-driver integration

`FileSupervisedDriver::cancel_and_resolve_request` uses the original locked owner
without extracting it or acquiring new evidence/approvals. It can settle the
current job or an older durable request. `cancel_and_resolve_active` supplies the
current job identity and acknowledged revision; it requires an explicit trusted
tick. The original weaker `cancel_active` and actor wire methods stay unchanged.

After settlement, original live ledger state and owner health determine cleanup,
not the returned status of a possibly different historical request. An exact retry
for an older terminal request cannot close a newer healthy review. Stale, missing
or retention/capacity-refused work stays in its original phase. A storage-faulted
owner closes its local drive path without acknowledging cancellation or refund.
Conflicting owner borrows refuse before touching a healthy worker pool.

The existing cleanup retires sockets and retains direct child ownership for
nonblocking reaping; it does not detach a cohort or claim Idle means all children
exited. The original next-cohort admission and explicit release/handoff still
apply. The new tests use real Unix sockets, not real helper subprocesses, so they
do not newly qualify subprocess/descendant containment.

`FileSupervisedDriver::reconcile_pending` now invokes the existing atomic clocked
sweep instead of committing Time before trying Sweep. Both original event slots
must fit, even when the tick equals the saved one. Per-attempt failures remain in
the original returned map; endpoint expiry evidence alone can refund a charge.
The exact/one-under capacity test pairs failure with a successful original
expiry reconciliation and checks there is no standalone clock prefix on failure.

The seven driver tests additionally cover active socket closure, healthy review
preservation after refusals, an old terminal retry during a new review, query-only
reopen settlement, all five replacement barriers, expired-retention liabilities,
and terminal/faulted paths that must not invoke clock or evidence callbacks.
