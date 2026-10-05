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

## Live policy-only intake at the same transport boundary

`FileActorSupervisor<FileOversight>` now supplies
`prepare_learned_policy_intake` and coupled `exchange_learned_text_actor_from_policy_file`
and `feed_learned_text_actor_from_policy_file` methods. Stream counterparts use
`exchange_learned_text_stream_actor_from_policy_file` and
`feed_learned_text_stream_actor_from_policy_file`. The same supervisor returned
by the original gateway remains the only authority owner.

Before acquiring a new observation, the coupled path checks the fixed intent
and the ORIGINAL source/request binding. Incomplete, interrupted or paused
output and premature stream finish refuse before clock or file work. Exact or
conflicting historical retries, poll, cancel, malformed input and incomplete
frames do no source work. Foreign gateways refuse before parsing or mutation.
Original channel write AND flush backpressure prevents a second observation
from being acquired for buffered next-request bytes.

A valid NEW request withdraws any old one-use snapshot, retains exclusive host
custody across clock and source callbacks, and uses the SAME `policy_snapshot`
parser as native/probe review. It requires exact scope and roster, complete
policy data, and EMPTY helper contexts. A full-context registered source is
refused, not silently downgraded. File failure, a failed post-read clock, or an
unwind leaves no old or partially installed observation eligible. The original
port then derives output again and consumes the prepared slot in its original
request transaction; a successful intake report does not assert that this
separate durable request transaction committed.

On Linux, the supervisor's `drive_learned_text_peer_from_policy_file` and
`poll_learned_text_listener_from_policy_file` methods integrate the existing
kernel-credential-bound `PeerSession` and named `UnixPeerListener`. The stream
counterparts insert `_stream` after `learned_text`. Gateway identity and original
socket budgets are checked before accepting or reading a peer; original kernel
credentials are checked before evidence acquisition. Per-frame diagnostic
capacity is reserved before any source work, and reports retain accept and drive
results separately. Use an ordinary or native review driver's `supervisor_mut()`
to service this same listener while retaining the original review job and keys.

Only the original redacted wire response reaches the actor. Source identities,
read failures and preparation results remain in the original supervisor-only
`FileActorExchange`, `FileActorFeed` and `FileActorPeerDrive` reports. This adds no
provider or permission serializer. A file observation is not a durable producer
floor, an authenticated source, a lease guarantee or an OS containment proof.
Socket work budgets do not bound synchronous source/journal/replay latency.

Fourteen additional regression functions cover actual files, failed context and
scope/completeness validation, lost one-use observations, post-read clock errors
and unwinds, reentrant actor calls, foreign ownership, all five request storage
barriers, paused recovery, read-free retries, channel backpressure, real named
sockets, credential rejection and invalid socket budgets. End-to-end scenarios
carry socket-originated generated text through the ORIGINAL native helper
models and independent human key, with actual native allow/deny controls. The
stream scenario rejects finish both before publication and while published but
unreconciled, then requires new native review and a different human key for its
receipt-confirmed finish. These are authored source tests, not executed results.

The final targeted and full RCH gate attempts still cannot launch: `rch` is
absent (exit 127). No local compiler fallback was used. Source/hash checks do not
replace compilation, tests, rustfmt or Clippy; all remain unexecuted.
