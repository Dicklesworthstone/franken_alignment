# Anchored source-free terminal recovery

`FileOversight::recover_stopped_anchored` adds exact independently retained
history continuity to the original source-free stop operation. It returns the
existing `FileStoppedHistory`: one native stop observation and the exact private
anchor for that same acknowledged cut. It returns no live owner or reviewer.

```rust,ignore
let history = FileOversight::recover_stopped_anchored(
    directory,
    independently_selected_profile,
    original_stop_request,
    &independently_retained_anchor,
    trusted_clock,
)?;
let (stopped, successor_anchor) = history.into_parts();
```

This is an explicit library API, not a new CLI verb or a changed default.
`recover_stopped` retains its original signature and behavior. Learned numerical
histories continue to use their independently configured guarded recovery
cursors; see [learned terminal recovery](LEARNED_STOP_RECOVERY.md). This API uses
the ordinary original journal decoder and machine replay. It cannot inject a
missing numerical recipe or relax an optional-contract failure to make replay
succeed. Nor does an anchor replace a deployment's independent guard inventory,
effective-policy floor, authentication or storage assumptions.

## Admission and continuity

The original store stays exclusively locked from reading through completion.
The independently supplied profile and limits are checked, and the canonical
journal is decoded using its original format. The supplied anchor must match
an exact prefix of that original history before machine replay begins. This
rejects a valid same-counter fork with different historical bytes, not merely
corrupted data. A canonical history older than the anchor is refused even when
the caller supplies the original stop counters from that older history.

The full unanchored suffix must still pass the original decoder and reducer.
The request names the exact original control and authority predecessor. A
matching retained stop reuses that request. A conflicting request for the same
operation is a binding error; a different operation cannot replace a stop.

Canonical bytes are compared again after replay and before calling the terminal
clock. The original terminal preflight compares them again after the clock and
before cleanup or replacement. Invalid input, a failed prefix check, a stale
clock or insufficient terminal capacity cannot promote a candidate anchor or
clean abandoned staging as though the stop had succeeded. A clock callback that
changes the canonical image cannot cause recovery to overwrite that new image
with an earlier reconstructed state. These checks supplement the original
trusted-directory and cooperating-writer lock contract; they are not containment
of an arbitrary malicious filesystem or privileged host.

## Completion, retry and liabilities

Advancing recovery reuses the original fixed `Stop -> Fence -> StopProgress`
transaction and its finite recovery reserve. Its fully replayed machine enters
the existing consuming terminal finalizer: no second replay or ordinary recovery
fence consumes terminal capacity first. Every new encoded prefix must still fit
the original event and byte limits. The successful replacement bytes move into
the returned anchor without a second encoding or post-acknowledgment disk read.

An already-drained matching stop confirms the identical canonical image and
returns an identical anchor without calling the clock or appending/replacing
anything. This remains available at full journal capacity. Canonical durability
confirmation can remove abandoned staging; staging is never promoted to history.

Reserved-but-undispatched work is cancelled under the original rules. Sent work
is reconciled only by the original endpoint protocol. Confirmed executions stay
executed and charged. Expired unknown outcomes stay explicit and charged, so
`FileStoppedHistory` is not evidence of a drained domain. A pending retry still
needs a fresh clock and finite capacity and can produce a later successor anchor.

Native storage failure returns neither a stop observation nor a candidate
anchor. A replacement can nevertheless be visible after an ambiguous final
synchronization. Reopening the same operation with the independently retained
anchor checks the actual visible canonical history, rather than assuming that
failure meant nonexecution. No effect is resent.

## Private custody and costs

The anchor contains original private history, including payloads and review
inputs. It is operator evidence, not actor output, a signature or a permit.
Capture does not acknowledge export. Retain and synchronize a separately held
archive, keeping the previous archive until that succeeds. Failure to retain
or render the result cannot undo an acknowledged stop, refund a liability or
justify a new publication attempt. Joint rollback of journal and archive still
defeats this continuity assumption.

This synchronous API reads and decodes one complete bounded canonical history,
re-encodes the retained prefix for comparison, and replays the original machine
once. Additional canonical comparisons read the bounded image; terminal
preflight retains the original fixed three-event transaction and byte costs.
The returned anchor owns an already bounded canonical byte buffer. There is no
hard latency bound, asynchronous runtime or new dependency, and no remote-provider
exactly-once or OS-process stopping claim.

## Regression coverage and qualification

Eight new native regression functions cover original reserved/sent/executed
outcomes; valid same-path, same-counter forks; rollback behind a retained stopped
successor; full-capacity zero-clock/zero-replacement retries; all five Store
barriers for sent and executed histories; stale/unavailable clocks and a valid
canonical change inside the callback; expired unknown liabilities; and exact
request/profile/exclusive-owner admission. Every refusal group includes a
near-identical successful case. Tests reuse the original two-key fixtures,
canonical encoder, replay reducer and Store fault injection. The original
source-free recovery tests are unchanged.

These new tests have not executed. Required remote-only RCH dispatches stopped
before compilation because `rch` is absent in this environment. Typechecking,
formatting qualification, Clippy and the full xtask gate remain unexecuted.
The scoped work and full FA-014 acceptance remain open. The implementation serves
plan sections 16.4, 16.5, 16.9 and 25.4, FI-A02 and nonresurrection invariants; it
is not a production safety or release qualification.
