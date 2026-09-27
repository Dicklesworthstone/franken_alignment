# Source-bound activation probe helpers

The `oversight::sidecar::probe_helper` module adds a numerical consumer of the
existing `SidecarReceiver`. It serves plan sections 10.3, 10.4 and 10.15,
packet FA-113, and founding ideas FI-A05 and FI-I04. This is reference source,
not a qualified production feature or a change to the plan's contracts.

## Evaluation

`SidecarProbeEvaluator::new` freezes a complete probe roster, the original
member port, its sidecar packet, the checked source and aggregate work limits.
Every captured K/V row needs at least one distinct compatible probe. Coverage
is limited to that source descriptor; a latest-position capture is not a
whole-prefix observation.

`evaluate` accepts the original decoded WorkerInput; `evaluate_port` uses the
existing wire encoder and decoder. The receiver checks the complete request,
including the member, round, root, salt limit, profile and submitted bytes.
It reconstructs only residuals disclosed in that packet. Probe scoring uses
the existing exact linear accumulator. Reports contain DisclosedProbe values,
not the underlying source view or its undisclosed residuals.

A complete quiet roster yields the helper recommendation Allow. An alarm yields
Hold. Uncertainty or exact threshold equality yields Abstain. These results
remain inputs to the existing congress; they are not effect permits, calibrated
harm probabilities or policy disqualifiers. Refinement requires the original
planner and a fresh round. Publication still requires current evidence and all
configured authorization keys.

## Cooperative evaluation

`begin` binds and receives the original request once, reconstructing its disclosed
residuals within the frozen aggregate allowance. It scores no probes. Each
`advance(expected_revision)` then computes at most one registered probe using
the existing receiver and exact accumulator. The roster order is unchanged, and
even an early alarm must wait for every registered probe before a report exists.
The synchronous `evaluate` and `evaluate_port` methods consume this same stepper.

`revision` identifies the exact predecessor; stale advances and cancellations
make no progress and spend no work. A Busy latch is installed before numerical
calls, so a caught unwind cannot retry the same operation. Individual receiver
reconstruction and probe calls are bounded but are not internally preempted.

`cancel(expected_revision)` releases unfinished receiver/source ownership.
Completed disclosed observations and work remain inspectable; cancellation
does not refund reconstruction or probe work. Active evaluation becomes
Cancelled, while a completed report or the first failure remains visible.
Repeated cancellation at the resulting revision is a no-op. Partial observations
never become a report, helper vote, or authorization.

## Resource and failure behavior

Admission bounds the complete input, probe count, coefficient coordinates,
residual bytes, materialized values and reconstruction products. The reservation
covers coarse scoring plus disclosed reconstruction. Refinement may reduce actual
scoring work, but does not refill the original source or planner budgets.
An invalid first input, computation error or interrupted evaluation prevents
another evaluation on the same owner. Work counters describe completed calls;
failed calls may have additional unreported work.

The source binding is independently provisioned in process. Byte equality is
not cryptographic authentication, and numerical fidelity does not establish a
probe's empirical detection quality or statistical independence.

## Original-protocol transport

`probe_helper::peer::ProbeHelperClient` owns one evaluator and one original
HelperClient. Only the computed result can populate its response. Construction
performs no I/O; the Unix constructor sets a provisioned socket nonblocking.
Each step runs an original protocol operation or one fully admitted evaluation.
It yields after input decoding and after judgment, before commitment output.
The reveal waits for the original coordinator signal. Bounded drive calls yield
on backpressure, input readiness, judgment and termination.

Partial writes retain the existing protocol offsets. On error or an unwind,
the connection is dropped rather than restored to an older offset. Cancellation
also closes it. Already transmitted bytes cannot be withdrawn, and a missing
reveal remains missing to the congress. Reports preserve completed numerical
work. Salt-length checks and clearing are not entropy or secure-erasure claims.
Generic Read/Write implementations retain the caller's bounded-I/O obligation.
This does not provide a new runtime, process launcher or remote source importer.

## Verification status

Twenty-seven integration tests and five compile-fail examples are authored. The first
twelve cover actual coarse abstention, refinement, alarm and quiet paths,
two-key publication, stale sources, exact and insufficient budgets, roster
completeness, request binding and one-shot behavior. Eight transport tests cover
fragmentation, Interrupted/WouldBlock, original frames, reveal ordering,
cancellation, invalid requests, partial-write panic and real Unix-socket paths.
Seven cooperative tests add exact synchronous/stepped parity after disclosure,
one-probe progress, the complete-roster barrier after an early alarm, stale
revisions, cancellation before input and between probes, retained completed
reports and exact submitted-byte binding. Missing helpers continue to hold
original authorization. Socket controls run in one process and do not establish
process isolation.
Existing receiver, numerical, wire, congress and authority algorithms are unchanged.

The required targeted and full RCH commands were attempted again on September
27, 2026. Both stopped before compilation with exit 127 because rch is absent.
Cargo, rustc and rustfmt are absent too. Compilation, Rust tests, rustfmt and
Clippy remain unexecuted. No Bead or qualification gate is closed. Source hash
and whitespace checks are not runtime verification.

The cooperative additions have source review only. Workspace execution became
unavailable before their RCH commands could run; the historical checks above do
not qualify these changes. Their compilation, tests, formatting and Clippy are
UNEXECUTED.
