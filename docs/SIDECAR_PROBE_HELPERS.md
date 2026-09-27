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

## Verification status

Twelve integration tests and three compile-fail examples are authored. They
cover actual coarse abstention, refinement, alarm and quiet paths, two-key
publication, stale sources, exact and insufficient budgets, roster completeness,
request binding and one-shot behavior. Existing receiver and numerical code
is unchanged.

The required targeted and full RCH commands were attempted again on September
27, 2026. Both stopped before compilation with exit 127 because rch is absent.
Cargo, rustc and rustfmt are absent too. Compilation, Rust tests, rustfmt and
Clippy remain unexecuted. No Bead or qualification gate is closed. Source hash
and whitespace checks are not runtime verification.
