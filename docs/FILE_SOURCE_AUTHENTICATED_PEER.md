# Authenticated registered-source actor transport

Linux durable actor ingress can now compose all three existing boundaries in one
ordered path:

1. `PeerSession` verifies the connected Unix peer with the frozen SO_PEERCRED
   `PeerPolicy` before reading actor bytes.
2. `UnixActorConnection` applies the existing bounded newline framing and
   write/flush backpressure.
3. Only a complete, valid, previously unseen `Submit` frame invokes the existing
   registered-file intake hook.
4. That hook persists a fresh source observation and installs its bounded one-use
   snapshot before the original durable `FileActorPort` performs submission.

No source read occurs for socket fragments, malformed frames, Poll, Cancel, exact
submission retries, conflicting retries whose key already exists, or a socket
rejected by kernel credentials. A reconnect must authenticate again. Once the
original request exists, its exact retry reacquires ticket visibility without
renewing the source or requiring the evidence file to remain available.

The transport change is deliberately internal. `ActorChannel` already had a
crate-private complete-frame admission hook; `UnixActorConnection` now carries
that hook through the same bounded socket drive, and `PeerSession` exposes it only
to crate-owned trusted integrations. Actors cannot install an admission callback,
obtain the request port, acquire the supervisor, or bypass the normal wire codec.
The default public `drive` path is unchanged.

`FileActorSupervisor::drive_peer_from_file` first verifies that the authenticated
peer session owns the exact durable port paired with that supervisor. A foreign
session therefore refuses before parsing or source activity. One bounded drive
may complete several frames, so supervisor-only diagnostics return a vector of
source-intake reports in frame order. Those reports never enter actor response
bytes and never constitute permits or endpoint outcomes.

The socket budget continues to bound read/write bytes, complete frames and I/O
attempts only. Registered source/journal work is synchronous and can exceed the
socket latency budget; this increment does not introduce an executor, watchdog or
asynchronous disk layer.

Regression source covers a cold authenticated submission with exactly one source
read, disconnect plus authenticated exact retry after deleting the evidence file,
wrong-PID rejection before any source I/O or durable request admission, and
read-free authenticated Poll/Cancel operations. These use actual Unix socket pairs
and Linux SO_PEERCRED observations.

SO_PEERCRED remains a local kernel identity boundary, not executable attestation.
Transferred file descriptors and same-UID/GID processes remain within the explicit
limits of the configured policy. The required RCH runner is unavailable here, so
Rust compilation, formatting, Clippy, tests and doctests remain unexecuted. No
Beads or production-gate status is changed.
