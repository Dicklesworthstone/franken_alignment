# Catching cooperative preparation up with live control

Unqualified source implementation. Plan roots: 6.3 (owned work and cancellation),
6.4–6.5 (ordered authority and explicit clocks), 10.6 and 18.3 (scheduling and
replay costs). The actual consumer is the learned step intent/completion path.

## Keep verified history when the same live journal grows

Both `FileLearnedIntentPreparation` and `FileLearnedStepPreparation` now expose
`catch_up(&host, expected_revision, expected_events, max_events)`. The revision
names the exact LIVE journal revision observed by the caller; the event cursor
names the task's current progress. The call adopts that acknowledged prefix and
runs at most `max_events` original reducer transitions from the existing cursor.
It does not restart verified history, skip newly appended records, perform new
inference, mutate the live owner or write the canonical file.

This makes cooperative preparation usable when a host records time or serves
actor cancellation between replay quanta. All appended changes are replayed,
including cancellation and receipt settlement. The same owner has an append-only
acknowledged event vector: its identity pins that immutable history, not merely
a filename or a numeric revision. No saved candidate or foreign owner's prefix
is trusted. A recovered owner has a different identity and cannot adopt the task.

The strict `advance` and `finish` methods are unchanged: journal mutation still
makes them stale. Catch-up is an explicit alternative to discarding the task;
it never silently changes their contract. Ready becomes Replaying when unread
entries are adopted. Finishing before they have all executed still refuses.
After another live write, even a previously Ready task must catch up again.

## No replacement operation or authority restoration

Catch-up first rechecks the original operation's full admission law against the
live owner, including exact numerical actor revision/position, pending identity,
reset and recovery pause, source-interruption latch, storage availability and
ordinary-event capacity. A completed/changed step cannot become the next step
under the old task. Adopting a larger revision cannot resume a fenced generator.
Two preparations still cannot commit the same intent twice.

A preflight refusal leaves the old target and replay cursor unchanged. Once an
original reducer runs, its first failure is sticky and an unwind leaves the
candidate Interrupted. No retry can relabel failed history as Ready. Finalization
still calls the original intent or witnessed numerical-completion persistence
boundary and repeats its admission checks. Catch-up itself returns progress only,
never an actor outcome, model sample, refund or either approval key.

The original two-acknowledgment contract remains: intent finalization writes only
Begin, and completion finalization returns a numerical result only after its own
witnessed write succeeds. Loss of either write poisons the live owner; independently
configured recovery decides canonical visibility. Dropping a task changes no
pending obligation. New clock/source writes may consume capacity that was merely
checked earlier; catch-up rechecks it rather than treating it as a reservation.

## Cost and execution limits

The retained cursor avoids repeating already verified events when an otherwise
compatible append arrives. It does not eliminate replay by the OTHER journal
transactions, bound an event's inference time, or guarantee progress when writers
append faster than reconstruction. No runtime, thread, journal tag, numerical
algorithm, second ledger or new publication path is added. Logical numerical and
effect accounting is unchanged; discarded physical replay work is not a new
durable compute charge.

Eight regression functions are authored with the existing original numerical
fixture. They compare every sampled result/cache/sampler under interleaved clock
writes, preserve read-only canonical bytes during catch-up, deny early finish,
check stale/foreign input and changed numerical state, preserve cancellation and
receipt accounting, exercise both stages at all five storage barriers, enforce
capacity after intervening writes, and retain original reducer failure on a
controlled malformed tail. Synthetic parameters do not establish detector quality.

Required targeted RCH execution and the full xtask gate cannot launch because
`rch` is absent (exit 127). Compilation, tests, rustfmt and Clippy remain
unexecuted. Source/hash checks are not runtime qualification. No gate or bead
is closed by this addition.
