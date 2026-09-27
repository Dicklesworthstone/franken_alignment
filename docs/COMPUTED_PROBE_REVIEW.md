# Computed probe review with automatic refinement

This L3 adapter connects the source-bound probe evaluator to the original
`LearnedWorkerReview`. It serves FA-113 and plan sections 9.2, 10.3, 10.4,
10.15 and 14.1: actual disclosed activation measurements can request refinement,
then participate in a fresh original congress round. The source, numerical
algorithms, refinement planner, congress and effect rules are unchanged.

## Execution

`OversightBroker::begin_learned_probe_review` consumes one original sidecar,
a fixed worker schedule, a complete member map and `ProbeReviewLimits`.
Each `ProbeReviewMember` provides immutable probes and one separately provisioned
salt for each scheduled round. The member keys must equal the original committee.
Every captured K/V row is covered by the existing evaluator's admission rules.
The maximum is 256 scheduled member evaluations. The reservation covers every
scheduled round at the supplied per-evaluation component ceilings, even when
an early quiet result is expected. Unused capacity cannot buy replacement cases.

`advance_learned_probe_review` executes one original coordinator poll. Its
private callback runs only after the original source and elapsed-time checks.
Each member performs at most one operation per poll: admit its complete request,
score one original probe, queue a completed judgment's commitment, or queue its
reveal when the original coordinator requests it. Admission uses the original
wire encoder/decoder and scores no probes. A completed roster yields before its
commitment is queued, so the supervisor can still cancel that unsent vote.

Members still evaluating advance in the same poll; an earlier member does not
finish its whole roster before later members can begin. Numerical work stops
at the original commit deadline, including for a partly scored roster. Only a
complete judgment can commit. No externally supplied ballot enters this path,
and a peer's measurements or vote are not supplied to another evaluator.

The original driver alone decides whether an abstention buys a predeclared
residual. After its input transaction succeeds, this adapter provisions fresh
original ports and evaluators using the same frozen coefficients and budgets.
It does not score a second round in the same invocation. A refined packet that
exceeds the frozen evaluation ceiling stops the driver; it is not rerun with a
larger allowance or replaced by a coarse Allow.

`history` retains the original completed review archives. `records` describes
provisioned numerical attempts and queued messages, not accepted congress votes.
Their numerical counters describe completed operations; a retained Evaluating
or Busy state after an interruption makes incomplete work explicit. Evaluation
counts increase once at request admission, not once per probe or poll. Cancelling
or losing the original source preserves completed reconstruction and probe work.
Unstarted future rounds have no fabricated measurement rows. The reservation
remains unchanged.

## Authority, lifetime and limits

The adapter owns every original member port. It exposes no member replacement,
manual vote, mutable evaluator, source replacement or reset operation. Stale and
foreign calls preserve its state. Errors and unwinds close the stack-owned active
ports; cancellation closes the current round and preserves completed records.
Partly evaluated slots are cancelled when the original driver stops or fails;
their completed measurements are never turned into a fallback vote. Reports
already judged remain judged even if no commitment was sent.
The original poll, round, source, disclosure and deadline limits still apply.

`take_review` returns only the original completed review, once. Application,
current-input validation, authorization and every installed effect key remain
separate. A source advance invalidates a permitting result even after this driver
has completed. Cancellation does not cancel an external effect or refund rights.

This is local deterministic numerical evaluation, not isolated helper processes,
authenticated transport or independent statistical evidence. Same-source probes
remain same-source. The supplied elapsed tick is a trusted observation; one poll
is bounded numerical work, not preemptive wall-clock scheduling. The per-evaluation
ceilings and finite roster bound logical work, not total allocator overhead or
peak resident memory. There is no persistence or new runtime in this adapter.

## Verification status

Fifteen integration tests and two compile-fail examples are authored. Positive paths
use the existing history-sensitive decoder, learned codec and exact probes.
They exercise two-member computed abstention/refinement into two-key publication,
an actual activation alarm, quiet siblings, exact schedule reservation, preflight
refusals, source changes, stale/foreign calls, cancellation, expired workers,
poll exhaustion, insufficient refined-input capacity and finite round horizons.
Five added tests cover unequal member rosters, one-probe progress, deadline expiry
after partial work, cancellation during a refined round, source loss between
probes, and cancellation after judgment before commitment. Existing synchronous
setups are adapted to multiple polls; the original refinement-to-publication
control and numerical/authority algorithms are preserved.

Targeted and full RCH commands stopped before compilation in this environment:
`rch` is absent (exit 127), as are Cargo, rustc and rustfmt. All new Rust tests,
compilation, rustfmt and Clippy are UNEXECUTED. Source checks and GitHub commits
do not establish runtime correctness, detector quality or production readiness.
No Bead, roadmap qualification or release gate is closed.
