# Independent stop during supervised publication

The Linux `supervise_publication` commands using `--reviewer-profile` now bind a
separate `stop-REQUEST_ID.sock` in the independently provisioned reviewer socket
directory. The same configured kernel UID/GID/PID rule applies, but the protocol
is stop-only. It needs neither a pending human review nor an evidence read.

The service begins after native actor admission and before original witness
capture/helper launch. It is checked again before launch, during helper polling,
while waiting for a reviewer connection, while waiting for its decision, while
delivering its receipt, and before each publication-driver step. `create`,
`create-checked`, and new `submit`/`submit-checked` requests share this exact path.
Explicit resume and exact retained retries do not create any control endpoint.
Legacy commands without a peer profile do not gain an unchecked stop listener.

## Authority and scheduling

The listener uses the existing protected-directory/inode-owned socket lifecycle,
independent peer admission quota, and native `StopControlConnection`. Admission
precedes protocol I/O. The native owner supplies its original reviewer role and a
new random nonce; no approval request, permit, replacement budget, or second
journal is created. The operation identifier is the original actor request ID.

Idle polling does not read the journal or advance its clock. Once a permitted
peer is admitted, forward workflow progress pauses for its bounded stop exchange.
The original logical/wall deadline and the peer profile's `runtime_ms` both apply.
Silence, malformed bytes, and admission exhaustion fail through the original
workflow's stop/drain error path, never a weaker peer rule or reset quota. One
unauthorized candidate below that quota is closed without disclosing offer bytes.

The original reserved-capacity stop runs before its fallible settlement clock.
After application, only the historical network receipt is drained under the
configured cleanup limit. Losing that reply does not undo the stop or invoke the
native application again. A refused/unconfirmed stop or incomplete drain remains
an error with recovery still required; unknown effects are not locally refunded.
The actor receives only the original gateway's terminal result, not a synthetic
success constructed from a network message.

This is cooperative synchronous polling, NOT hard real-time preemption. An
individual filesystem operation, helper launch, or original driver call can block;
this service cannot interrupt the kernel or publish a bound on stop latency. A
request that has not reached a checked boundary may race publication. Authenticated
process identity is not proof of a human decision or protection from a compromised
host. Independent emergency mechanisms remain necessary for such profiles.

## API consumer and tests

An independent client authenticates the same connected socket through
`VerifiedReviewerSocket`, calls `into_stop_client(expected_audience, request_id)`,
and explicitly requests stop only after the nonce-bound offer passes validation.
Reading an offer or merely connecting does not send a stop request. The native
receipt separates unconfirmed stop, stopped with refused drain, pending drain,
and drained stop.

The example tests exercise stalled real helper children, waiting for any human
reviewer, permitted publication with an unused control listener, source-free exact
retry beside an occupied control path, wrong kernel identity without disclosure,
wrong session binding, and lost receipt without duplicate application. Helper
verdicts are explicitly synthetic and do not validate a detector.

Verification remains pending: the editing environment lacks RCH and a Rust
compiler. Run the example tests and full `xtask check` through the repository's
required `RCH_REQUIRE_REMOTE=1 rch exec -- ...` path. Source checks are not runtime
qualification, and this change closes no production Bead.
