# Source-only learned output on the actor transport

Status: source integration; execution validation is unavailable. This connects
the completed learned generator to the original actor wire, bounded channel and
generic Unix transports (plan 8.7, 9.3 and 17.1). It adds no JSON operation,
journal event, request ledger, endpoint, model loader or approval capability.

`FileLearnedTextActorPort::encode_request` encodes a 24-byte intent: `FALTXT`,
zero, version 1, an eight-byte big-endian request key, and eight-byte effect
units. The outer Submit target, epoch and deadline keep their original meanings;
outer units must be 24. The original source-only port supplies the actual output
and actual charge. A two-byte generated message can therefore be proposed with
two effect units even though its transport descriptor is longer. The inner key
must match the outer key. Trailing bytes, old raw payloads and alternate domains
are refused, not interpreted as replacement text.

`FileLearnedTextStreamActorPort::encode_release` encodes a 17-byte intent:
`FALREL`, zero, version 1, the eight-byte key, and tag 0 (Message) or 1 (Finish).
The outer deadline is meaningful. The other outer fields are canonical envelope
markers: all five target fields are 1, epoch is 0, and units is 17. The receiver
rejects variations. These markers never reach the effect owner as a target or
charge: the original stream builder derives the current real destination,
epoch, complete cumulative frame and full cost. The trusted concrete port
selects the route; an actor cannot switch routes by choosing a marker.

Use `encode_command(Command::Submit { ... })`, `ActorWire::new(port)` and
`ActorChannel::new(...)` unchanged. New requests still require the original
supervisor-owned single-use snapshot. Neither the wire nor the actor acquires
clock, snapshot, source, human reviewer, inference or publication privileges.
The same sealed `ActorRequestPort` implementations are accepted by the existing
generic Unix transports; peer admission remains a separate host responsibility.

Only exact submission retries acquire historical tickets on a new connection.
Poll by numeric ID alone stays withheld. Reopening the journal fences old keys
and pauses generation; historical cancellation is observable without resuming
that generation. Cancellation or transport EOF does not prove an unknown effect
unexecuted, and a prior message key cannot become a stream-finish key.

Authored regressions cover independent golden bytes, full-width identifiers,
every descriptor truncation, suffixes, profile/key conflicts, actual generated
content and charges, early finish, one-use observations, channel fragmentation,
consumed-prefix accounting, write/flush backpressure, fenced recovery and all
five journal replacement fault barriers. They use the existing synthetic native
model fixture; this is not a trained-detector effectiveness claim.

Validation: targeted and full mandatory RCH commands cannot launch because
`rch` is absent (exit 127). Compilation, tests, rustfmt and Clippy are unexecuted.
Preimage hashes and source checks do not qualify runtime behavior. No existing
gate is weakened and no bead is marked complete.
