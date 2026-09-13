# Durable actor requests through the original nonblocking Unix transport

Consumer: the supervising host already using UnixActorConnection and native
readiness scheduling. This completes the FILE_ACTOR_GATEWAY.md transport path
without a second socket loop, command grammar, authority or endpoint adapter.

UnixActorConnection now accepts the same sealed port parameter as ActorWire and
ActorChannel, with the original in-memory ActorPort as its default. A host can
construct ActorWire::new(file_actor_port), wrap it in ActorChannel::new, and pass
that channel with its connected UnixStream to UnixActorConnection::new. Descriptor
borrowing, readiness hints, buffered-input handling, partial output and the
write/flush barrier stay in the existing implementation. The entire drive body
and its original unit-test body are unchanged; only the port types are generalized.

DriveBudget still bounds socket read/write bytes, frames and I/O attempts. The
new durable submission performs the original synchronous journal replay and
replacement inside a completed frame. Those disk operations are independently
bounded by FileDelivery's journal limits, not by the socket counters or a latency
SLO. No background thread, foreign executor, hidden asynchronous guarantee or
production performance result is introduced. A native host must account for that
synchronous storage work when scheduling. A new request still needs one fresh
supervisor snapshot; exact retries need neither a snapshot nor another admission.

Two additional public socket scenarios drive the real adapter with seven-byte
reads, three-byte writes and bounded operation counts, then perform original
congress-backed publication and observe its redacted outcome. The reconnect case
loses the socket before its prepared response is sent, retains the same wire
session, and requires an exact retry to leave the durable request count/revision
unchanged. These are executable test sources, not executed network measurements.

The additional storage regression reuses the existing test-only barriers around
stage creation, write, file synchronization, rename and directory synchronization.
Both admitting and refused submissions are exercised. Before publication, recovery
must find no new request; after successful rename followed by failed directory
synchronization, recovery must find the exact original request even though no
acknowledgment escaped. The old in-process owner remains faulted and cannot return
a candidate status. A recovered accepted-but-undispatched request is cancelled by
the original fence; a recovered refusal remains refused. Exact retry adds no event.
This is not a power-loss, malicious rollback or filesystem authenticity proof.

Together with the first two increments there are twenty regression scenarios in
twenty-one new Rust test functions, including the subprocess entry point, plus one
compile-fail boundary. All original test bodies are retained. The required command
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check was attempted
but could not start because rch is absent; compilation, formatting, Clippy, Rust
tests and revision-bound qualification remain unexecuted. Source hash/diff checks
are not Rust execution. No dependency, original reducer, bead closure or historical
verification evidence changed. The operator-file scope and remaining authentication,
anti-rollback, full-OversightBroker migration and hostile-process limitations in
FILE_ACTOR_GATEWAY.md continue to apply.
