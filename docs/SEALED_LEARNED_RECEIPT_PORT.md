# Serve a learned receipt without keeping a writable supervisor

`FileLearnedReceiptActorPort` connects the original learned receipt finalizers to
`ActorWire` and its bounded transports. It is constructed only by consuming a
completely verified original recovery. The returned value owns one immutable
source-only request envelope and its original `Knowledge<ActorOutcome>`; it owns
no Store lock, journal, model, supervisor, source observer or approval role.

## Original recovery, two explicit modes

The four existing cursor types have two additional consuming methods:

- `FileLearnedRecovery`
- `FileGuardedLearnedRecovery`
- `FilePredictiveLearnedRecovery`
- `FileOwnedPredictiveRecovery`

After advancing the selected cursor through its complete original history:

```rust,ignore
// Historical observation only: no clock, cleanup, fence or replacement.
let port = recovery.finish_receipt_port(request, finish_after)?;
let mut wire = ActorWire::new(port);
```

Alternatively, explicitly select the existing receipt-reconciliation transaction:

```rust,ignore
let port = recovery.finish_reconciled_receipt_port(
    request, finish_after, trusted_clock,
)?;
let mut wire = ActorWire::new(port);
```

`finish_after: None` selects a message. `Some(previous_request)` selects an
original finish bound to that exact confirmed predecessor. Request selection
uses the original methods before serializing the source-only envelope. Each
consuming finalizer repeats its own guard and canonical-cut checks. No partial
replay, changed recipe, missing key or unconfirmed predecessor can issue a port.
The complete independent guard, predictor, floor, reserve and optional anchor
requirements of the selected cursor remain in force.

The observation-only method delegates to `finish_receipt`. It works at the
original journal ceiling and leaves staged files untouched. The reconciliation
method delegates to `finish_reconciled_receipt`: the existing fixed
`Fence -> Time -> Sweep` replacement must acknowledge before a port is issued.
It needs its original ordinary capacity, calls the trusted clock only in that
finalizer, retains unresolved liabilities and never resends an effect. It is not
a zero-event retry and cannot substitute for terminal stop at exhausted capacity.
No subsequent actor frame can initiate another reconciliation.

## Exact receipt lookup, never admission or cancellation

The actor resends the same source-only Submit produced by
`FileLearnedTextStreamActorPort::encode_release`. The port compares the entire
original envelope: request, kind, original deadline, fixed target fields, epoch,
units and every intent byte. A changed envelope is an idempotency conflict; a
different nonzero key is Withheld. No current target or policy fields are derived
and an old deadline cannot be renewed. Existing wire framing and redaction are
unchanged.

Only an exact Submit issues a connection-local ticket. Tickets are also bound
to the particular recovered port, so matching request numbers from another
recovery do not transfer access. Cloned tickets from the same port remain valid.
Cancel always returns Withheld: even an unsent reservation belongs to the
journal, not to this immutable observation. The port has no write operation,
public outcome constructor or conversion into a live owner.

The Store lock is released before the port is returned. Another authorized
writer may therefore advance the original history while this receipt is served.
**The port remains a historical observation at its recovered cut.** Pending and
Unknown stay pending and unknown; polling does not make them current, settle
them or refund their charges. Known values retain the original request-local
basis. A new observation requires a separately authorized, independently checked
recovery, not a mode upgrade caused by an actor frame. Operators must authenticate
access and bound the service lifetime; this type is not process isolation.

## Verification record, 2026-10-16

Four unit regression functions cover every envelope field and byte, foreign
request/ticket custody, immutable outcomes and the original ActorWire path.
Five public integration functions exercise actual learned generation and
original two-key publication, all six request cuts including separate finish,
full-capacity observation, lock release, explicit reconciliation, exact selection,
guard floors and changed canonical bytes. Two compile-fail examples describe
unavailable owner access and public construction. Predictive wrappers use the
same closed composition of their existing finalizers; these tests do not claim
a new forecast-quality or deployment result.

These tests are authored and source-reviewed, **not executed**. The required
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_receipt_actor_port`
failed before compilation with `rch: command not found` (exit 127). Cargo, rustc,
rustfmt and a Rust syntax parser were unavailable in the editing runtime. No
compile, formatting, Clippy, test or release pass is claimed. Original gates,
limits, dependencies and historical qualification records are unchanged. The
scope advances the existing FA-107/FA-014 receipt boundary; full packet acceptance
remains open. The required `br` tool is unavailable, so existing Beads records
were read but not rewritten by this increment.
