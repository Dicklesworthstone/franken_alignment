# Recover saved message intents without new disclosure

`FileStreamActorPort::recovery_only()` returns `FileStreamActorRecoveryPort` for
use with the original ActorWire, ActorChannel, Unix transport and Linux peer
session. `for_request` can permanently narrow it to one key. Neither conversion
changes the owner, installs evidence, or revokes an independently retained
ordinary port. The wrapper cannot unwrap into an admitting port or expose the
current prefix, host, source, observer, reviewer or effect keys.

The raw recovery gate now shares its original retained-key lookup with a private
stream-intent path. A missing key refuses before the stream builder, so neither
a message nor a finish can allocate an attempt or consume an admission snapshot.
The sealed original request books do not remove or reassign keys, and the same
non-Send owner is kept alive across lookup and retry without an external callback
or scheduling point. Original source acquisition and budget rules are unchanged.

The stream wire decoder checks the independently selected stream profile and
complete intent. The original native builder then retrieves that key's retained
cumulative release frame, checking all original intent fields. It never rebuilds
an old message against today's later confirmed prefix. Exact message and finish
retries remain recoverable after subsequent messages, final finish, ordinary
recovery fencing or expiry. A retained admission refusal remains recoverable even
when it has no frozen action. Wrong profile, malformed bytes, altered target,
policy epoch, deadline, message or transport units cannot replace a request.

Poll and Cancel delegate to the same restricted raw gateway. Cross-key and
foreign-owner tickets remain withheld. Cancellation is not rollback: only unsent
reservations can be released, published messages stay disclosed, dispatched work
stays charged until native reconciliation, and a dropped socket is not finish.
The capability is non-admitting, not read-only. It is weakly held and cannot keep
the authority lock alive after the supervisor is dropped.

This closes the message-intent gap in receipt recovery without changing a wire
command or journal format. A raw receipt gateway expects raw saved proposals and
must not be substituted for this stream receiver. The runnable stream receipt
service is the next consumer. The supervisor must still recover the owner under
its exact independent guard/profile/history contract and authenticate connections.
No arbitrary server becomes non-admitting merely because the client requests it.

Six new regression functions exercise original owner/stream/client paths: missing
message/finish with snapshot preservation and an ordinary admitting control;
recovery after two messages and finish with changed-field controls; monotone
selection, foreign tickets and weak custody; original cancellation/charged-unknown
versus executed outcomes; stale-epoch admission refusal across reopening; and durable
client recovery of an unanswered old intent over actual Unix sockets after the
confirmed prefix advanced. One compile-fail boundary rejects admitting unwrap.
These are authored tests, not passing results. The RCH-only target and full gate
are attempted; the editing runtime has no rch. Compilation, tests, rustfmt and
Clippy remain unverified. No dependencies, pre-existing assertions, Beads states,
qualification gates or production claims change.
