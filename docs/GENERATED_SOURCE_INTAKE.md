# Registered-file intake for generated-text actors

The source-only actor wire now connects to the existing registered-file intake
path. This serves the native-text stream's actual actor gateway and supervised
driver (FA-107; plan 8.7, 9.11, 17.1). The actor continues to supply only a retained
generation reference or an explicit finish intent, not message bytes, evidence,
a clock, a numerical state or an effect key.

`FileActorSupervisor::exchange_generated_actor_from_file` handles one original
JSON exchange. `feed_generated_actor_from_file` uses the original newline-framed
channel. Both also exist on `FileSupervisedDriver`, which performs its existing
helper cleanup after the operation. They accept the same sealed `EvidenceFile`
readers as the original file-backed driver; no alternative parser, snapshot
store, source lease, request book or execution path is introduced.

## Ordering and failure contract

The exact gateway identity is checked before parsing or observation. The original
JSON parser and fixed source-reference decoder then run before source acquisition.
Only a well-formed new Submit prepares evidence. Poll, cancel, recorded exact
retries and recorded conflicts do not read a file or sample time. The original
port still validates retry identities and projects the current request outcome.

Preparation uses `prepare_wire_submission` and `prepare_file_intake` unchanged.
The old one-use admission slot is withdrawn before a new observation begins.
The registered source is reread and durably observed, then checked at a fresh
post-read tick against the lease that began at read START. A slow successful read
can therefore fail freshness. Missing/incomplete evidence, a failed observation,
or an expired lease cannot fall back to the old slot. A valid but unavailable,
stale or incomplete generation may still cause an observation before the original
source-linked proposal checks refuse it; fixed-shape validation is not a claim
that the generation is eligible.

Source preparation and proposal admission are separate durable cuts. An intake
report can be successful even when the ensuing submission refuses or faults.
Reports retain the original observation and storage diagnostics for the supervisor;
actor responses contain only the original redacted outcome/error representation.
Source failure does not fabricate a NotAdmitted request record. A failed journal
replacement keeps the original fault latch and exposes no candidate ticket.

Newline framing and write/flush backpressure remain unchanged. Partial frames and
blocked replies cannot acquire evidence for the next request. Native generation
and source revalidation are separate: intake does not generate a new token, alter
sampling allowances, supply replacement output or authorize publication. The
cumulative message charge, original congress, separately held human key and fresh
publication checks remain mandatory.

## Validation scope

Eight `generated_source_` regression functions are authored using public APIs,
actual temporary source/journal files, synthetic weights and the original native
numerical implementation. They cover the full reference two-key publication
lifecycle, malformed fixed/JSON input, recorded retries/conflicts/poll/cancel,
source loss and incomplete-observation repair, exact lease expiry, foreign
owners, reentrant actor access, a real create-new staging conflict, and channel
newline/write/flush boundaries. The staging test is one real filesystem failure;
it is not an all-barrier or hardware power-loss campaign. The congress test uses
explicit reference ballots, not independently authenticated helper inference.

The targeted attempt failed before compilation (`rch: command not found`, exit
127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generated_source_
```

Rust compilation, all tests, rustfmt, Clippy and the complete repository gate are
UNEXECUTED. Source/whitespace/hash review is not runtime qualification. This adds
no dependency, journal tag, actor verb, listener or executor, and closes no bead.
Storage fidelity, file producer authenticity, trusted clocks and deployment
isolation remain assumptions of the existing reference profile.

## Authenticated Unix peers and restart

On Linux, `drive_generated_peer_from_file` is available on the original supervisor
and supervised driver for `PeerSession<FileGeneratedTextActorPort>`. It uses the
original kernel-credential check, frozen peer policy, connection limit, revocation,
framing, socket budgets and ticket-preserving reconnect. The exact gateway identity
is checked before socket reads. Source preparation uses the same private adapter
as JSON/channel intake; no credentials are accepted from a request. The existing
`FileActorPeerDrive` report retains source diagnostics in completed-frame order.
Its bounded intake vector is reserved before driving any source transaction.

A disconnected or revoked peer, invalid socket budget, foreign gateway or malformed
reference cannot consume source evidence. An admitted request with an unsent reply
survives disconnect. Reconnecting the same session retains its tickets; a fresh
session must submit the exact original reference to reacquire one. Neither route
re-observes a recorded request or renews its authority. Source preparation is not
repeated for a previously recorded policy refusal, even if the file later changes.
A new request must obtain its own observation. Cancellation/revocation of a socket
still does not cancel a dispatched effect or refund an unknown outcome.

Five additional `generated_source_peer_` tests use actual Unix socket pairs and
`PeerCredentials::observe`, including a mismatched UID and a matching positive
control. They exercise supervised-driver intake, fragmented input and blocked
replies, actual producer-file changes affecting policy admission, lost replies,
ticket custody, foreign owners, zero/invalid drive budgets, ingress revocation,
and exact-history required-source recovery. The recovery case starts a fresh
FileEvidenceSource with no in-memory version floor: the ORIGINAL durable source
gate must reject an older producer file. A repaired source can prepare evidence
but cannot silently resume the paused decoder; explicit resume and new native
generation retain the original spent sampling work before another proposal.

These tests use same-process socket pairs, not an isolated hostile executable or
a remote peer. SO_PEERCRED identifies connection-time UID/GID/PID; it does not
attest code or authenticate a transferred descriptor's current holder. Source
producer authenticity and clock fidelity remain operator assumptions. Socket
budgets bound socket work, not synchronous file IO, numerical replay or disk latency.

All THIRTEEN authored regression functions, compilation, rustfmt, Clippy and the
full gate remain UNEXECUTED. The second targeted RCH attempt failed before
compilation (`rch: command not found`, exit 127). The command above selects both
the eight original integration tests and these five peer tests. No new dependency,
journal encoding, actor verb, listener, runtime or production qualification is
introduced; existing source and transport implementations are unchanged.
