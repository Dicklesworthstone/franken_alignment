# Bounded learned-policy comparison campaigns

`comparison::verified::campaign` turns the original single-case learned-monitor
comparison into a fixed-cohort executable experiment. It serves the plan's L7
paired-evaluation and decision-preservation work (§10.2, §12 and §19); it is not a
new policy reducer, safety qualification, statistical estimator or promotion API.

## Execution

Construct each case with `CampaignCase::new(id, preparation)`, where preparation
comes from an original `GenerationCheckpoint::begin_policy_comparison` or a
recipe-matched `GenerationArchive::begin_policy_comparison`. Only unadvanced
preparations are accepted. Candidate policies, original source recipes and all
native per-run budgets are already frozen before the campaign starts.

`ComparisonCampaign::required_cost(&cases)` computes aggregate numerical
admission using the original decoder estimator and the numerical reservation
already checked by the original replay constructor. `new(cases, CampaignBudget(cost))` can admit exactly that
cost. One less in any component refuses before campaign inference. Up to 64
cases are supported; case IDs must be nonzero and unique. The reservation covers
original baseline reconstruction plus both experimental arms at their declared
position ceiling, even when an early stop is likely. Early holds/failures do not
refill it or make space for post-hoc replacement cases.

`run_next(expected_case_id)` runs the next fixed case. An accidental retry with
the preceding ID returns `Stale` rather than running a sibling. Each ordinary
verification or comparison failure becomes a retained row, after which the next
case can still execute. `run_to_completion()` evaluates the whole fixed cohort.
Completion means every case has a result, not that a candidate passed.

## Bounded stepping and cancellation

An unstarted campaign can be consumed by `into_stepped()`. The resulting
`SteppedCampaign::advance(expected_revision, max_operations)` executes at most
4096 original engine calls per invocation. Each operation is one baseline replay
step or one paired step; a paired step can compute one token in each arm. The
original finite state comparison/encoding still runs at its native byte limits.
This is cooperative numerical work bounding, not preemptive wall-clock control.
Zero operations do nothing, including for empty checkpoints. Stale revisions and
oversized calls refuse without advancing either engine. Successful nonempty
calls advance the campaign revision once. Partial execution can be continued in
the same process without recomputing earlier positions or resetting budgets.
This is not a durable campaign checkpoint or an external-effect recovery API.

`progress()` returns original partial work reports and phase, but no paired
report is exposed before complete original baseline verification. Resuming with
different step sizes uses exactly the same original engines and produces the
same terminal case reports as whole-case execution. `reports()` retains completed
rows. There is no conversion back that could bypass an active partial verifier.

`cancel(expected_revision)` stops both active and queued work. Every remaining
case receives an explicit `Cancelled` row. A partly executed case retains its
original verification/comparison work; an unstarted case remains visibly
unverified. Completed rows never change. The summary retains its original cohort
size and reservation, adds a cancellation count, and distinguishes these rows
from matched stops and failures. Cancellation does not compute a final token,
finish verification, refund a reservation or expose either experimental arm.
Repeated cancellation at the current revision is a read.

## Results and boundaries

Rows preserve the original preparation report, original paired report when
verification succeeded, and a typed terminal outcome. Matched stops, candidate
versus baseline decision differences, both-held runs, position exhaustion,
verification errors and comparison errors stay separate. Missing or failed
observations never become matched/quiet results. The summary retains the full
cohort denominator, completed and pending counts, and the unchanged reservation.
Repeated evaluation origins count as one declared lineage even across different
streams, not as independent trials. Even distinct source labels do not establish
independence, source authenticity, detector accuracy or a confidence interval.

Baseline recomputation work stays separate from the two experiment work rows.
Each retains its original telemetry-completeness qualifications. Reservations
are logical product/score/position counts, not actual latency, peak RAM or a
complete measure of failed telemetry work. Per-case native limits still apply;
up to 64 preparations retain their own bounded models, policies and owners.

The campaign never exposes an executable arm, mutable verifier, token continuation,
permit or policy-promotion operation. A later live source hold remains untouched.
The original verifier is required even for an empty saved prefix. A caught unwind
latches the campaign; an interrupted case cannot be silently retried or replaced.

## Authored verification

The first eight integration tests use the original small deterministic decoder,
fitted KV codec and probes. They cover both directions of disagreement, both-held cases,
exact versus one-less aggregate limits, original single-case parity and differing
telemetry costs, terminal archive verification, stale retries, exhausted and
failed cases followed by successful siblings, invalid/adopted rosters, and
preservation of later live holds. Ten additional stepped-runner tests cover
segmentation parity, exact per-call operation bounds, partial-verification and
partial-comparison cancellation, preservation of completed rows, empty-state
verification, corrupt imported cache expectations, exhausted siblings, adoption
boundaries and same-origin grouping across different streams. Three compile-fail
examples protect authority and mutable-owner boundaries. There are eighteen
integration tests in total. These are numerical reference controls, not
trained-detector or deployment evidence.

The preparation environment has no RCH, Cargo, rustc or rustfmt. Compilation,
Rust tests, formatting, Clippy and the required remote gate are unexecuted.
No qualification, release, roadmap packet or Bead is closed by this addition.
