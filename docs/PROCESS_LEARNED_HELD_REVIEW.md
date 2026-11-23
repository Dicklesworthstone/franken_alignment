# Explicit continuation of a held learned process review

A completed process-helper Hold can retain the next **original fixed** review
slot for an independently requested continuation. The same
`FileProcessSupervisedDriver` keeps the actor request, frozen action, learned
generation, sidecar, executable schedule and spent work. A Hold does not launch
another cohort by itself and a continuation ticket grants no publication key.

This extends the original process publication path in FA-018, FA-019, FA-107
and FA-113 and the held-review path in plan sections 9.2, 9.8, 10.15 and 17.8.
It preserves the founding separation of observation, congress judgment and
effect authority (FI-A03, FI-A06, FI-A09, FI-A13, FI-A14 and FI-I04). It does not
close those broader packets.

## Original custody and exact eligibility

The original process launch must already declare every round ID, window,
member executable, argument list, environment, working directory and bounded
start reservation. Only the immediately following untouched slot can be
selected. A different ID, a new executable inventory or a newly calculated
window cannot replace that slot after a result is visible.

Eligibility requires the original successful Hold application to remain the
latest completed review of the same admitted request. Its action, authority
epoch, control sequence, input revision, current learned sidecar and acknowledged
owner must still agree. A later Begin, Continue, narrowing, suspension,
cancellation, Stop, source evolution or pending numerical operation cannot
borrow an earlier Hold. An expired fixed commit window or exhausted cumulative
protocol allowance makes the slot unavailable.

The original learned forecast rules also remain in force. Ordinary absence of
a predictor is supported. An explicitly required raw or owned predictor must
retain the exact settled forecast used by the original review. A pending,
crossed, missing or differently owned forecast does not supply eligibility.
No predictor is rerun or replaced to obtain another vote.

Process and native offers have distinct private provenance. Process custody
does not borrow a native worker lease, instantiate a native evaluator or enter
the generic review launcher. Copied receipts and public basis values cannot
construct the sealed one-use ticket.

## Typed driver operation

Create and retain the original finite `FileSupervisorConveneGrant` before the
first process review finishes. Its weak port shares the existing independent
query and operation bounds. The public sequence is:

1. Complete the original review and inspect `held_successor_round()`. The
   observed round is fixed in the original schedule; inspection reads neither
   a policy file nor a live clock.
2. Independently query `port.next(request, round)` and submit the returned exact
   `convene_request()` through `port.convene(...)`. This consumes one operation,
   records Pending and queues no helper work.
3. Continue nonblocking child cleanup. `take_pending_convene()` leaves the
   ticket queued until **every predecessor direct child is confirmed reaped**.
   Missing or unresolved startup custody never counts as reaping.
4. Pass the one-use ticket to `convene_from_policy_file(source, ticket, clock)`.
   The driver checks the exact predecessor and reaping before topology, source
   or clock acquisition. It then refreshes the required current observations,
   verifies the original sidecar, and invokes the original process launcher.
5. Advance the same process driver. A later Hold requires another independent
   operation for the next original slot. Only an actual Continue makes the
   original job eligible for fresh human approval and normal publication.

A completed held driver's ordinary `step` returns Idle after child cleanup. It
does not reread the policy source, sample the clock, repeat a review, discard the
held job or publish. The service or typed caller must still enforce its original
wall-time limit and service independent Stop while it waits.

An accepted final Pending operation remains usable when its grant's remaining
operation count reaches zero. Once that ticket is taken and abandoned, the
same fixed slot cannot be purchased again. Exact duplicate requests return the
historical receipt without another source read or another operation charge.

## Cumulative work, failure and handoff

The continuation launch consumes **one remaining original protocol poll**.
Subsequent process I/O shares the same original poll maximum. Starts were
reserved for the complete schedule before the first Begin; neither the start
reservation nor numerical, sidecar, disclosure or cleanup allowances are
renewed by a Hold. The original input bytes remain unchanged across an explicit
held continuation. Witnessed residual refinement remains its separate original
path and retains its existing input-revision rules.

Source acquisition, current topology and trusted time remain prerequisites.
A missing source refuses before a launch poll. A complete source carrying a
changed semantic snapshot can consume the launch poll before the original
Begin rejects it. Neither outcome starts a replacement cohort or creates a
retry entitlement.

The original launcher marks a round consumed before spawning and retains
recoverable partial children and uncertain startup on error. Its post-launch
clock check still precedes request bytes. Exit status is cleanup evidence,
never a helper ballot. Reaping, an accepted commit/reveal and the resulting
congress consequence remain separate observations.

A consumed ticket records Opened only after durable Begin and the selected
launch succeed; a refused consumed ticket records Failed. Dropping a taken
ticket records Abandoned. A queued receipt can remain historically Pending
after its weak custody is withdrawn; that cached word does not keep a ticket
usable. Cancellation, helper-stop, grant revocation, owner drop and terminal
handoff retire continuation custody without refunding work or unknown effects.

The successor's Continue still requires the existing automatic permit, a fresh
independent human permit, current policy, exact learned input, any required
credential and the original publication witness. Convening supplies none of
those effect authorities.

## Verification boundary

Four private regression functions use real child commit/reveal protocols for
Hold-to-Continue, unchanged packets and original spending, stale source and
Continue refusal, exact window/poll/unknown-startup controls, and a real
post-launch clock panic with retained children. Seven public driver regressions
cover one publication through fresh two-key approval, idempotence, abandoned
tickets, original-only slot selection, source loss/change, stale predecessors,
Stop/cancellation/numerical changes, expiration, repeated independently requested
Holds and retirement before generic handoff. Child fixtures provide explicitly
synthetic ballots; they establish no detector quality.

These tests are authored and independently source-reviewed, **not executed**.
The required remote-only command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-08 check --locked --workspace --all-targets`
could not start because `rch` is absent (exit 127). The workspace subsequently
became unavailable. No Rust parsing, formatting, compilation, Clippy or runtime
pass is claimed for this addition. Existing historical execution receipts do
not qualify these changed bytes; broader verification and deployment remain
open.

See [the process publication contract](PROCESS_LEARNED_PUBLICATION.md) and the
distinct [retained native review contract](LEARNED_HELD_REVIEW.md).
