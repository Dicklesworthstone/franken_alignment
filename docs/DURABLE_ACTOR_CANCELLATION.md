# Cancel an acknowledged durable actor request

The existing Linux actor command accepts one additional explicit mode:

```
supervise_publication actor-submit ACTOR_PROFILE --durable CLIENT_PROFILE cancel MIN_REV
```

It takes no replacement Submit document and cannot select a new request key.
The original actor profile chooses the retained request; the original durable
client profile, server/transport binding, peer credentials, lifetime budgets and
independently retained minimum revision still apply. Existing create, submit,
resume and inspect commands keep their behavior.

## Do not create an action merely to cancel it

The current wire protocol requires an exact Submit retry to recover a ticket on
a new connection. Therefore cancellation requires the latest retained original
server response to be an acknowledgment: Pending, OutcomeUnknown or terminal Known.
An older Pending response overwritten by ControllerUnavailable is not sufficient.
A missing request, missing response, ControllerUnavailable, Withheld, Stale,
Absent or an error does not establish a server record. The command checks this
in the complete canonical client image before checking the socket directory,
acquiring a writer or cleaning staging, then rechecks the locked image before
connection. Unacknowledged requests require independent supervisor resolution;
this command does not send them again simply to obtain a cancellation ticket.

The guarantee is conditional on the selected durable server preserving its
original immutable request book. Client files and peer replies are not signed
receipts; a rolled-back or hostile server, tampered client archive or incorrect
server identity invalidates this assumption. The command adds no remote
exactly-once guarantee or authenticated proof of server persistence.

## One original cancellation, not a new outcome evaluator

After kernel peer verification, an exact retry reacquires only the original
request ticket. A terminal or unavailable retry response is returned without
sending Cancel. For Pending or OutcomeUnknown, one explicit Cancel is prepared
and sent through the original durable client, with its write-ahead intent and
acknowledgment-before-response rules. This uses the original gateway cancellation
and effect ledger; it does not grant an approval, invent a snapshot or execute
an adapter. There is no automatic cancellation retry, reconnect or unknown-outcome
poll loop. Submission and cancellation share the same bounded receive helper.

The response remains the original Knowledge value. Cancellation-before-dispatch
can release only the original unsent reservation. A dispatched unresolved effect
remains charged and OutcomeUnknown, even after a successful protocol exchange.
An already executed request remains Executed and is reported as an unsuccessful
cancellation, not as cancelled. Only original server reconciliation establishes
execution/nonexecution. Other terminal nonexecution dispositions are reported
unchanged, not relabelled as cancellation.

Each explicit invocation has one finite monotonic scheduling budget. Expiry after
ticket recovery is checked before preparing Cancel. Expiry, socket/storage error
or process loss cannot undo an entered cancellation. The original client image
retains its interrupted command and spent work. A later explicit cancel repeats
the original request-key recovery; an already cancelled request returns its
retained terminal result without a second Cancel. Ordinary resume remains an
observation/submission-recovery request, not an implicit cancellation policy.

Reporting uses the existing historical JSON envelope, including the unmodified
nested server response and separate work/reservation fields. Unknown cancellation
returns a nonzero result immediately. An acknowledged cancellation remains in the
client/server histories when stdout write or flush fails; no replacement effect
or extra cancellation is sent to repair output.

## Regression source and qualification

Seven new regression functions use actual Unix sockets and the original
FileOversight request book. They cover reviewing and authorized-but-undispatched
cancellation with obsolete key rejection; dispatched, published-unreconciled and
confirmed-executed controls; missing/unanswered requests before cleanup or socket
access; lost Cancel response followed by exact no-repeat recovery; expiry between
ticket recovery and cancellation; rejected evidence/options; and stdout write or
flush failure after acknowledgment. Successful publication/reconciliation remains
covered by the original durable command tests. A previously added fixture used a
20-byte payload with a 16-unit ingress ceiling; it now uses an 11-byte payload,
retaining every expected charge and assertion instead of weakening the guard.

These are authored tests, not passing results. Required remote-only RCH targeted
and unchanged xtask commands are attempted; rch is unavailable, so Rust compilation,
tests, rustfmt and Clippy remain unexecuted. No native library rule, dependency,
wire command, journal encoding, Beads status or qualification gate changes.
