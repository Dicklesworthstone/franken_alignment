# External helpers in durable learned-sidecar review

This adapter connects original Unix helper connections to the existing durable
learned-sidecar finish/refinement transaction (FA-113; plan 9.2, 10.3, 10.15,
16.4). It fills the gap between the in-process computed-probe driver and the
single-round socket pool. Concurrent `FileLearnedSocketRound` remains unchanged;
this scheduler adds complete future-peer custody, fixed-round admission and
automatic progression through acknowledged refinements. Both paths use the
original I/O/coordinator and witnessed finish primitives. It changes neither the
original wire protocol nor
numerical, vote, disclosure, authority or journal algorithms.

## Execution and peer ownership

`FileOversight::begin_learned_transport_review` consumes an acknowledged
`FileLearnedSidecar`, `LearnedWorkerSchedule`, and `LearnedRoundPeers`: one
complete member-to-UnixStream map for EVERY scheduled round. All IDs, roots,
windows, rosters and socket modes are checked before the first durable Begin.
At most 256 round/member sockets are retained. The original helper input and
framing limits apply on each round. Construction transmits no evidence.

The driver leases all scheduled rounds against the existing manual protocol
entry points, including after cancellation or driver loss. Each round has fresh
preassigned connections. A peer cannot reconnect, be substituted, select another
member or receive a preceding round's input. Callers retain responsibility for
identity authentication, process isolation, salt entropy and child reaping; Unix
sockets and the reference FNV comparison commitment provide no such guarantee.
No claim is made that a remote helper actually computed a particular model.

`advance(host, expected_revision, now, snapshot)` takes at most one original
connection step per current member. Source checks precede I/O. The original
Coordinator forwards accepted phases to DurableSession, so commitment/reveal
acceptance is journaled before subsequent phases proceed. Expired uncontacted
workers receive no request bytes. Transport failures stay missing in the original
roster instead of becoming abstentions, synthetic votes or smaller denominators.
The supplied tick is a trusted logical observation, not preemptive wall-clock
control. Journal replay, encoding, allocation and filesystem sync remain
synchronous and can dominate the bounded socket operations.

When the original round completes, its existing witnessed finish transaction
alone decides whether an abstention buys one predeclared residual. The same
transaction retains the exact archive, input revision, disclosure cost and
original application result. A Refined result starts the next fixed round with
the newly acknowledged input; it sends no next-round bytes in that invocation.
The last scheduled round applies without buying an unusable extra refinement.
An enriched packet exceeding the frozen input ceiling fails, preserving the
already acknowledged refinement rather than increasing its allowance.

## Receipt-time-aware pumping

`advance_with_clock(host, expected_revision, observed_at, snapshot, clock)` uses
fresh operator clock samples at poll entry, immediately before and after each
socket operation, and once before round completion. `observed_at` is the explicit
preflight observation; subsequent samples cannot move behind it or behind the
original coordinator. The fixed-tick `advance` calls the SAME path with a constant
clock. A stale/foreign preflight does not invoke the callback or consume a poll.

A reply received after its cutoff is not backdated to the beginning of a slow
poll. Its post-I/O observation goes through the original coordinator before the
reply can be accepted. Time spent acknowledging a prior commitment is sampled
before a subsequent member gets request bytes or a reveal signal. This remains
cooperative: no timer preempts a file write, and trusted clock quality and
scheduling remain host obligations. Every source check and original deadline
rule still applies. The callback supplies time only, not a verdict or source.

Actual connection progress is recorded before another clock call or journal
barrier can fail. A backwards post-I/O observation is a terminal error; a clock
panic during an admitted call keeps the interruption latch and closes stack-owned
active and future peers. Neither case resends bytes or turns partial protocol
work into a completed review. Already sent bytes cannot be withdrawn.

## Failure and authority boundaries

Stale revisions and foreign owners perform no work. Every other admitted call
latches failure before taking stack ownership of active AND future sockets;
errors and unwinds close those sockets without restoring old offsets. Poll
exhaustion and cancellation close all peers. Completed per-peer calls and first
wire failures remain inspectable. Attempted versus returned step counts make an
interrupted call distinct; these are not inference, byte-volume or latency counts.
Future records contain no fabricated activity. The poll ceiling never refills.

`history()` retains acknowledged original FileLearnedSidecarFinish results.
A committed application error is a completed observation, not an invitation to
repeat the round. Existing `learned_sidecar_outcome` and read-only verification
recover these results after response loss or restart. Process-local socket state
is not recovered; the original recovery fence withdraws old sessions and keys.
No new journal tags or formats are introduced.

Finishing a review does not grant a permit or run an effect. Original current
input/source checks, authorization, human review, dispatch and final checked
publication remain mandatory. Cancellation is not proof of external nonexecution
and never refunds an unknown effect. Historical input/results remain private
operator evidence, not current eligibility or an actor-facing source channel.

## Authored verification and limits

Sixteen regression functions use actual local Unix sockets, the existing client,
original tiny-model generation, learned compression, and real journal replacement.
The external helper ballots are explicitly scripted protocol controls. They cover
three rounds of disclosed packets through both publication keys, recovery of
original outcomes, complete future-roster preflight, expired/missing workers,
source/input loss before disclosure, stale/foreign/cancelled calls, richer-input
capacity, alarm/application refusal, all five original storage-failure barriers,
poll exhaustion and drop. Six additional clock controls cover fixed-path parity,
pre-disclosure expiry, commit and reveal receipt-time crossings with near-identical
permitted controls, post-write clock panic, and backwards post-I/O time. First
increment test bodies are preserved. Two compile-fail examples protect peer/vote
ownership.
These are not trained-detector, authenticated-helper or process-isolation evidence.

The targeted and full RCH gates were attempted in the preparation environment;
RCH is unavailable (exit 127), as are Cargo, rustc and rustfmt. All new Rust tests,
compilation, formatting and Clippy are UNEXECUTED. Only selected source files are
present, not a full checkout. Hash/whitespace review is not runtime verification.
No Bead, qualification gate or production capability is declared complete.
