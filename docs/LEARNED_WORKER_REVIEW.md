# Bounded worker-driven learned-sidecar review

## Actual consumer, original protocol

`learned_host::sidecar::workers` connects the source-bound learned sidecar to
`HelperRound`, rather than asking a supervisor to manufacture commits/reveals
or manually reconnect every refinement round. It serves plan 9.2 and 10.15.
The source remains the original broker-owned learned generator. The codec,
checked source, residual priority and disclosure budgets cannot change.

`begin_learned_worker_review` consumes the initial sidecar and freezes a finite
schedule of round IDs, reference roots, commit/reveal deadlines, per-round
helper limits and total poll allowance. It returns exactly one original
`HelperPort` per registered member. Each port sees only that member's original
view. There is no coordinator verdict-submission or missing-worker replacement.
This is a bounded in-memory execution selection, not a durable preregistration
service or a claim that no separate earlier experiment/review occurred.

`advance_learned_worker_review` observes the supplied trusted clock and checks
the exact broker/source/input cut before the original worker round consumes
at most one queued reply per member. Early finish stays pending; a clock is not
advanced to make it complete. A completed abstention uses the existing sidecar
refiner and original input recorder, then starts one fresh scheduled round.
New workers receive the richer current view with a new round identity. Neither
inference nor compression nor fitting runs again. Old ports and votes cannot
be repurposed for that new round. The invocation never polls the new round.

## Results, failure and authority

Every completed original review archive is retained, including missing workers
and abstentions. `Stopped` describes review termination, not permission. Stops
separately identify a substantive decision, missing workers, unavailable
residuals, exhausted disclosure allowance and exhaustion of the round schedule.
No residual is bought once there is no scheduled reviewer left. `take_review`
returns the original final review exactly once. Applying it and obtaining keys
still require the original broker operations and fresh evidence. A late source
change cannot make an earlier Allow current. Human keys and endpoint checks
are unchanged. Missing replies do not trigger another attempt or count as votes.

Stale revisions, old clock values and foreign broker calls make no progress.
Once an admitted operation starts, an error or unwind poisons the sequence and
drops its active original round, closing its ports. Cancellation and dropping
also close ports; neither rewinds input retention, cancels the action, refunds
rights nor reconciles an external effect. An underlying successful input
refinement may remain recorded if subsequent round admission fails. Such a
failure exposes no final review; original input approval was already cleared.
Earlier complete archives and the last observed worker statuses remain visible.

Future round IDs are checked initially but are not reserved outside the
original broker. Concurrent use by other supervisor work causes a real
Duplicate refusal when reached, not an ID substitution. Deadlines never slide.
A missed future window is a failure, not a fresh extension.

## Costs and execution boundary

At most 32 rounds and 65,536 polls can be selected. Empty polls count. The
original per-round member/input/salt caps, disclosure caps and cumulative broker
input limits remain active. Up to 32 original archives retain their complete
bounded helper inputs; this is not a peak-RSS or network-byte measurement.
Caller-retained ports/views and monitor/model objects are not free storage.
The code supplies no executor, process sandbox, cryptographic commitment,
helper/model authentication or proof of independent model reasoning. Ports can
be provisioned through the existing helper socket/process bridge by the host.

Seven integration tests exercise actual learned inference, original worker
commit/reveal, refinement and two-key one-use publication. They cover incomplete
independent commits, wrong reveals, disconnection, deadline-missing outcomes,
round/disclosure/poll limits, stale and foreign calls, source changes, cancellation,
drop, rejected schedule admission and final Allow invalidation. Two compile-fail
examples prohibit coordinator votes and planner extraction. Ballots and tiny
model weights are explicit synthetic controls, not measured detector quality.

Compilation, rustfmt, Clippy and all new Rust tests remain unexecuted in this
selected-source environment. The required remote-only RCH gate cannot start
because rch is absent (exit 127). No historical receipt or source/hash check
qualifies this code. No Bead, production gate or release is closed.

## Nonblocking socket/process integration

`begin_learned_socket_review` admits a complete supervisor-provisioned Unix
socket roster for EVERY scheduled round before starting the first review. All
streams are made nonblocking before any evidence is sent. Callers can use the
existing `launch_helpers` to obtain those streams, but must retain its separate
`HelperChildren` owners and explicitly stop/reap them. This adapter never
spawns, replaces or authenticates a process and creates no executor/thread.
The bounded future socket inventory is real retained state, not free resources.

`pump_learned_socket_review` uses the same private core transition as port-based
review. The exact source check and fail-closed guard precede every I/O pass.
Each original `HelperConnection` performs at most one bounded nonblocking
read/write step per invocation; the original coordinator then consumes queued
replies. Interrupted/WouldBlock offsets remain the original I/O engine's state.
Uncommitted workers get no I/O after commit expiry; no worker gets I/O after
reveal expiry. Only the currently selected round gets I/O, and moving to richer evidence
closes its old transports before installing its fresh round's connections.
A failed or disconnected member stays missing; no replacement socket or vote
can be provided. The first wire failure per round/member remains inspectable,
with cumulative attempted connection-step counts rather than invented byte or
syscall measurements.

Stale/foreign calls have no I/O and preserve usable connections. A genuine
source failure, exhausted poll allowance or cancellation closes active AND
unstarted sockets. A caught unwind drops the locally held active connections
and original round; unused future descriptors remain owned until driver drop.
Completed original reviews still require original authority application and
keys. Peer provisioning/authentication, descendant containment, scheduling,
real elapsed-clock observation and process reaping are supervisor obligations.

Six additional Unix regression functions exercise the original HelperClient
wire parser over real Unix streams, and a separate-process path uses the
existing launcher with a synthetic helper entrypoint. Tests check actual packet-
driven abstention/refinement, distinct fresh connections, two-key publication,
no bytes sent after source loss, disconnected-worker missingness, inventory
refusal, expired uncontacted workers receiving no evidence, and cancellation/poll
exhaustion closing unstarted peers. The test
entrypoint chooses a deterministic vote from the received residual count; it is
not a helper-model or detector-quality claim. One compile-fail example denies
raw port extraction. The seven original worker tests remain unchanged.
All thirteen regression functions, the separate helper entrypoint and three
compile-fail examples remain unexecuted pending the required fresh RCH gate.
