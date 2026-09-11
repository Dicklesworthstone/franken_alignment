# Nonblocking Unix actor transport

## Connected capability

`oversight::actor_transport::UnixActorConnection` connects a real, operator-supplied UnixStream to the existing ActorChannel and ActorWire. There is one codec, one framing state machine, one ticket session and the original actor mailbox; this adapter does not introduce a competing JSON protocol or resource ledger. The supervisor remains entirely outside the connection.

The actor submits or polls through the opposite socket. Its JSON uses the existing decimal-string identifiers, hexadecimal payloads and redacted Knowledge responses. It cannot invoke supervisor review, publish directly, advance the clock, supply a snapshot or helper outcome, fetch secret helper inputs, or obtain a permit through this interface. Cancellation queues the original stop request and reports the original actor observation, not a fabricated cancellation receipt. See ACTOR_WIRE_REFERENCE.md for the exact shared schema.

## Scheduling, fragmentation and backpressure

Construction sets the socket nonblocking. A native scheduler may borrow its descriptor through AsFd, then invoke drive with explicit byte, record and I/O-attempt budgets. There is no thread, accept loop, listener, authentication service, executor, subprocess or automatic review. The caller provisions the connected peer and its permitted ActorPort. Non-Unix builds retain the generic actor codec/channel; this module is Unix-only.

The adapter reads into one 8,192-byte buffer and preserves the unconsumed suffix across calls. Every record goes through the original channel, whose single response must be fully written and flushed before another record is admitted. Partial writes acknowledge only the actually accepted prefix. WouldBlock yields immediately; Interrupted consumes the finite I/O-attempt budget. Defaults are 65,536 bytes in each direction, 16 records and 64 read/write/flush attempts per drive. Fixed ceilings are 1 MiB per direction, 64 records and 256 attempts. A standard socket flush may perform no syscall, so the counter is explicitly API attempts, not measured syscalls or CPU cost. Kernel socket buffers and original mailbox/codec storage are additional, not counted by the 8 KiB ingress size.

ConnectionStatus::interest distinguishes LocalWork, Readable, Writable and Closed. A coalesced request already in the adapter buffer must be driven locally rather than waiting for another readability edge. A fully written response awaiting the channel's flush barrier likewise has local work. No scheduling hint implies an effect is approved. The caller must provide usable budgets and fair scheduling across its bounded connection set.

Invalid budgets refuse before any I/O or intake. A later read/write/flush failure is returned alongside already completed progress and latched. Subsequent drives do no work. This preserves the distinction between a refused call and a partially completed call whose acknowledgment failed. Host-only status reports framing state, buffered byte counts and the failure direction/kind; it is not returned to the actor as an effect outcome.

## Reconnects, permission and outcome recovery

Disconnect, half-close, malformed framing, channel quota exhaustion, exhausted drive budgets and failed reply writes never cancel requests, refund units or create nonexecution receipts. The original supervisor and endpoint remain responsible for outcomes. into_session closes the old socket and retains the original ActorWire ticket/idempotency session for an explicitly provisioned reconnect. Incomplete input and unsent response bytes are discarded; accepted effects are not. Clients retry the same request key and exact proposal, not a newly generated effect. New channel quotas are local transport budgets, not renewed mailbox or production-rights budgets.

The end-to-end path is socket command -> original actor intake -> policy/congress review -> original automatic and optional human keys -> original delivery endpoint -> actor-only outcome projection. The transport does not read, infer or mutate the original control state. Exact denials remain terminal for their request keys even if a later snapshot would pass. A separately submitted new request still needs normal review. A consumed permit cannot be reconstructed from an Executed or Pending JSON observation.

A disconnected actor can reconnect while an effect is Unknown. Its cancellation request cannot refund a disclosed effect. A message that was never delivered also remains charged until the original endpoint establishes and records definitive nonexecution; sealing then prevents its delayed delivery from executing. When execution already happened, the execution receipt wins. Duplicate submission and receipt handling preserve the original one-use state.

The same connection works with the existing filesystem-backed endpoint. The integration case publishes the actual payload file, loses its acknowledgment, closes the actor socket and endpoint worker, reconnects the SAME actor session, reopens with the surviving endpoint recovery handle, supplies current clock observations, and reconciles before exposing Executed. It does not recreate a lost controller, fabricate a receipt from arbitrary file bytes, or restore an old budget. Helpers and snapshots in tests are explicit fixture inputs, not live inference or authenticated observations.

## Verification and boundaries

The first implementation increment adds six public real-socket-pair tests plus a compile-fail doctest: fragmented intake, one-byte response writes, reply backpressure before a second admission, coalesced input across record budgets, half-close with a truncated tail, idle WouldBlock, zero/invalid budgets and failed response delivery with an unchanged original reservation.

The second adds seven end-to-end socket tests and one scheduling-hint unit test. Positive publication and exact duplicate retry are paired with exact denial, unknown disclosed effects, unseen-message sealing, missing human keys, transport quota closure, and socket plus filesystem recovery. Existing actor fixtures construct actual reference reviews and permits; tests do not replace the broker with an always-approved callback. The filesystem test creates and removes only its own exclusively created temporary directory. Byte comparisons and conserved-budget assertions remain on the original APIs.

All fourteen Rust test functions and the doctest are UNEXECUTED source: Cargo/rustc and a configured RCH runner are unavailable here. Formatting, compilation, Clippy and the revision-bound full gate remain pending. No bead is closed and no historical receipt qualifies this code. The concurrent codec/channel implementation is reused unchanged and its tests are not counted as new tests in this batch.

This is std-only reference integration progress for FA-107 and plan 9.3/17.1. It establishes no peer identity, real process credential perimeter, durable controller recovery, native-runtime admission, authenticated snapshots or helper inference. An operating host must bound connection counts, authenticate and isolate peers, schedule drives, retain the original supervisor, and close or reconnect terminal channels. All existing authority and delivery guards remain in their original implementations. No Cargo dependency, unsafe block, secondary runtime or permissive fallback was added.
