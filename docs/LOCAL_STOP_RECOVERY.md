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

## Operator command

```text
supervise_publication recover-stop CONFIG OPERATION
```

Unlike `stop-peer`, this is a privileged local-storage command, not a remote
reviewer command. Use the retained private supervisor CONFIG and the original
nonzero stop OPERATION. Live stop control uses the request ID as that operation.
If no stop exists, this command requests terminal shutdown of the configured
local domain under a new explicit operation. It cannot be used to reopen intake.

The command reads the actual canonical snapshot to select its exact original
stop request, or the current pre-stop sequence and authority epoch. The native
recovery call checks that predecessor again under its exclusive lock. Concurrent
ownership or incompatible state change refuses; no retry loop silently replaces
the preconditions. Missing stores are not created. A different existing stop
operation is a conflict. No saved submit JSON or publication recipe is needed.

The complete Config must still parse, but its source is never read and no helper,
reviewer socket or producer is started. Checked-source and whole-input journal
guards are preserved by native replay, not disabled to allow recovery. This
operation stops and settles only; it cannot authorize or send a publication.

JSON output distinguishes `stopped_drained` from `stopped_pending` and `advanced`
from `already_drained`. It includes the native revocation/fence counters, retained
charges, execution count and every unresolved/irrecoverable attempt. IDs and
64-bit counters are decimal strings to preserve precision. No payload or private
helper evidence is exposed in this recovery report. Only a drained result exits
successfully. An acknowledged stop with unresolved liabilities emits its report
and then exits with an error; stopping never invents a refund for unknown work.

Failure writing or flushing output occurs AFTER native recovery. The error states
that the stop was acknowledged and whether it was drained. Rerun the same command
to reconcile lost output, never resend the original effect. A completed retry does
not consume another journal event or require a new clock observation. Native
I/O failure remains unconfirmed, including when a replacement may be visible.

Six command regressions cover missing sources, completed retries, lost write or
flush output, unfenced stop recovery, retention of actual published charges,
expired in-flight obligations, and invalid/busy/missing/foreign callers. The two
publication compositions use real helper/reviewer sockets with synthetic verdicts.
These tests are authored but unexecuted in the editing environment.
