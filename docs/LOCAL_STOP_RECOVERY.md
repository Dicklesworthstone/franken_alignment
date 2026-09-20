# Recover a stopped local publication without the supervisor process

The native `FileOversight::recover_stopped` operation in
`observed::shutdown::recovery` closes a local lifecycle gap: a supervisor or its
stop reply may disappear while the original durable journal remains available.
Recovery does not require the original actor submit document, an evidence file,
a producer bundle, running helpers, a human-approval key, or a live socket.

Supply the independently retained `FileOversightProfile`, original `StopRequest`,
and a fresh trusted clock callback. The original exclusive Store lock stays held
from canonical acquisition through the terminal replacement or confirmation. A
live owner makes this call refuse as Busy; it is not a competing-owner bypass.

## First recovery and unfinished recovery

When the requested stop has not completed, the method invokes the original
fixed Stop -> Fence -> StopProgress transaction. That transaction preflights its
finite event and byte capacity, preserves every journaled optional gate and
performs one canonical replacement. Only original endpoint outcomes release
charges. Already executed work stays charged; expired unknown work stays charged
and unresolved. A returned Advanced result is not necessarily drained.

The clock callback is consulted once only on this path, after canonical identity
and original stop preconditions have been checked. Clock failure, stale time,
insufficient capacity and storage failures return no candidate result. Storage
failure can follow a visible replacement; it is not proof of nonexecution.

## Exact completed retry

When canonical replay already reports the exact original stop as drained, the
method confirms durability and returns AlreadyDrained. It does not append an
event, advance an epoch, renew a source observation, call the clock callback or
rewrite the canonical image. Thus a lost output or final directory-sync error
can be reconciled even at the journal's event ceiling. The operation does not
return a live owner or a recovered reviewer role, so this fast path cannot
reintroduce permitting authority without a recovery fence.

The whole original StopRequest must match, not just its operation number.
Changing its predecessor fields is a binding error; substituting an operation is
a conflict. These checks precede pending-file cleanup. Staged leftovers are never
promoted, even if they contain a formerly valid live journal.

Both variants expose native snapshot and progress data. In particular, use
`progress().drained()` rather than treating any successful read or stop receipt
as complete settlement. The existing `open_stopped` API remains available when
trusted code specifically needs a stopped owner; its semantics are unchanged.
The existing fleet coordinator remains the mechanism for a registered roster
and independent whole-prefix floors. This local call is not a substitute for
those floors, source authentication, distributed simultaneity or process death.

## Verification boundary

Eight native regression tests exercise reserved/sent/executed work, completed
retries at the exact event ceiling, unfenced stops, expired liabilities, exact
request/profile/lock admission, all five replacement barriers, pending-image
refusal and clock/capacity failures. Votes are deterministic test fixtures, not
model inference. Rust compilation and execution require the repository gate:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

The editing environment lacks RCH and the Rust toolchain. The source is authored,
not runtime-qualified here; no Bead or production gate is closed by this change.
