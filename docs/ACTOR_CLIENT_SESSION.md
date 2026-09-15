# Retained actor clients and explicit reconnection

Consumer: the external dispatcher side of the existing durable actor gateway.
ActorClient uses ActorExchange and the original wire command encoder. It can
submit, poll and request cancellation; it cannot obtain either execution key,
review an effect, publish, repair an authority or initiate source acquisition.

## One session, original identities

ActorClientState owns a bounded original-proposal book and the lifetime I/O budget.
ActorClient owns one active exchange and no unbounded command queue. A new request
consumes a request slot and its logical payload allowance only after encoding and
exchange admission. Every execution-bearing field is retained. Reuse with different
bytes refuses locally before I/O. Failed or cancelled requests remain tombstones;
capacity cannot evict one and make its identifier fresh again. Exact retries do
not consume another request slot, but do consume exchange and I/O allowances.

A successful submission response establishes this wire session's ticket visibility.
An unavailable/refused RPC does not. Poll and Cancel require that visibility;
Pending and Unknown do not grant permission to execute anything. The original
server alone determines admission, actual effects and cancellation semantics.
Cancellation after dispatch remains an outcome observation, not a refund.

After a full terminal observation, contradictory outcomes, lower terminal basis
generations and Pending regressions close the connection without overwriting the
last accepted observation. Unknown/Withheld availability reports remain possible;
they cannot erase the separately retained terminal history. The response parser
validates names, fields and correlation, not the truth of a hostile server's claims.

## Explicit recovery, not automatic resubmission

into_state closes the transport while retaining proposals, observations, work and
an interrupted command. connect attaches an explicitly supplied stream to the same
state and sends nothing. connect_unix additionally sets the supplied UnixStream
nonblocking; setup failure returns both owners. No listener, address discovery,
authentication exchange, retry timer or second runtime is created.

A fresh wire session has no imported server tickets. The caller explicitly chooses
retry_submission(id), which encodes the identical original proposal. It cannot
mint a new identifier, upgrade its policy epoch, change a deadline or acquire an
execution key. After exact submit reacquires visibility, Poll/Cancel are available.
The original durable server decides whether the request was committed or is still
new. Recovery of a committed key needs no new source snapshot or helper review.
All budget counters survive this move, including failed exchanges and retries.

This is client-process-local retention. Losing the client process itself requires
an independently retained original proposal; this API does not create a durable
client outbox. Likewise, the caller must authenticate and route the supplied stream
to the original server domain. The existing wire has no signed session nonce or
independent authority identity, and this change does not pretend otherwise.

Generic I/O can execute caller code. An I/O call is marked active before invoking
that code; if it unwinds and the caller catches the panic, the next step fails the
exchange instead of replaying possibly transmitted bytes. The call remains spent.
Byte counters record only returned/acknowledged byte counts, not unknowable physical
traffic during a panicking implementation. A deadline/watchdog remains a host duty.

## Original path and verification

Nine session scenarios use actual Unix socket pairs and the original ActorChannel,
FileOversight and FileSupervisedDriver. They cover partial seven-byte server reads,
full helper/human/publication/reconciliation, lost submit acknowledgments, exclusive
owner reopening after publication, bounded tombstones, remote unavailability,
lifetime budget exhaustion, contradictory terminal reports, in-flight cancellation,
and the registered-file intake path. The latter starts with no cached snapshot,
performs actual source capture, and resolves/retries after deleting the source file.
Owner reopening drops all live original authority objects; it is not an OS kill or
a power-loss test. Helper verdicts are explicitly synthetic fixtures.

One additional exchange regression catches an I/O unwind and requires no second
write. Together with the first increment: nineteen Rust test functions and two
compile-fail examples. No existing test body, server command, reducer, journal,
source format, budget ceiling, dependency or historical execution record changed.

The targeted RCH invocation failed with command-not-found (127), before compilation.
Rust compilation, formatting, Clippy, tests and doctests remain unexecuted. These
source scenarios are not execution qualification; no Beads or production gates
are closed. See ACTOR_CLIENT.md for the common bounded response/transport contract.
