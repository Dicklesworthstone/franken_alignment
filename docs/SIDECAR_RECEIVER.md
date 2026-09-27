# Numerical consumption of disclosed helper sidecars

## Source-backed receiver, not an unchecked byte importer

`oversight::sidecar::receiver` is the L3 receiver-side counterpart to the
existing sidecar producer (plan 10.4, 10.15 and 14's bounded refinement).
`SidecarReceiver::new` binds one original `HelperPort`, its exact
`SidecarCommitteeRound`, and the already source-checked `CheckedLearnedKv`.
The original sidecar encoder verifies the complete base and selected residual
bytes. The original worker encoder and decoder retain the expected request.
`receive` compares the complete parsed request, including root, round, member,
salt limit, input profile, every submitted byte, parts and omissions. It does
not compare an FNV digest instead of these fields.

The binding must be provisioned by the trusted supervisor in the same process.
This is not standalone deserialization of a source certificate, authentication
of a remote producer, a new network format, or evidence that the source is still
live. Existing broker source/currentness checks remain indispensable. A sender's
claimed reconstruction error cannot manufacture a `CheckedLearnedKv`.

## Only the actual disclosure may become exact

Before materialization, admission sums every selected residual's encoded bytes,
original channels and original reconstruction-product estimate. `receive` uses
the original `verify_residual` and `LearnedKvView::refine` for precisely those
groups. It never promotes an undisclosed group merely because the supervisor's
source still retains its exact residual. Whole-request mismatch exposes no
partially reconstructed view.

`ReceivedSidecar::evaluate_probe` uses the original exact linear accumulator and
source-checked intervals. It returns `DisclosedProbe`, containing only the score
interval, original probe/frame identity, outcome and numerical work. It does not
return the original view-bearing observation, which would let its recipient
reach undisclosed source residuals. Neither the receiver, received object nor
its Debug output exposes the source or a mutable view. A received coarse object
remains coarse when another receiver materializes a later disclosure.

Certified quiet is only the registered probe's numerical sign, not action
approval or calibrated detector quality. Exact threshold equality remains
undecided. A complete action judgment and an effect permit are different types.

## Costs and verification

Packet and wire-request lengths are bounded, with the original encoder's own
fixed frame limit in force. One request serialization can be allocated before
the tighter caller request-byte limit is checked. Source objects are shared,
including supervisor-retained residual storage; this is not an RSS or sandbox
claim. Receive reports distinguish payload/request bytes, selected groups,
residual bytes, materialized values and reconstruction products. Per-probe
allowances charge original full-vector coordinates and necessary reconstruction
products, not merely nonzero coefficients. Explicit repeated probe calls are
separate work, not a cache hit or a global research-budget escrow.

Seven integration tests use actual original fitting, source checking, exact
refinement, wire parsing and reference congress. They compare coarse and exact
intervals to separately called original APIs; show an off-axis source resolving
to quiet for one probe and alarm for another; reject request/root/source
substitution; exercise exact and one-less limits; and preserve earlier coarse
views. Two compile-fail examples prohibit hidden source/view extraction.
Tiny weights and synthetic ballots are controls, not deployment evidence.

These tests are authored but unexecuted. The required remote-only RCH command
cannot start in this environment because `rch` is absent. No compilation,
rustfmt, Clippy, runtime correctness, detector qualification or Bead closure is
claimed by source inspection or Git publication. The local material is a
selected-source checkout, not the complete project.

## Native helpers use numerical evidence, not residual-count heuristics

`receiver::native::SidecarNativeEvaluator` freezes a nonempty finite probe
inventory, an aggregate numerical allowance, the receiver, and a fresh original
`NativeEvaluator`. All probe rows and total reconstruction costs are admitted
before scoring. Each `advance` evaluates one original exact probe or one original
native generation token. Completing the last quiet probe also performs original
whole-input tokenization/admission, but no native token computation. Stale
revisions do no work; errors and caught unwinds do not restore a runnable phase.

All registered probe queries run before classification. A certified alarm
produces `Hold`; unresolved or exact-threshold evidence produces `Abstain`.
Only a completely quiet inventory starts the original native helper over the
ENTIRE unchanged submitted input, including its binary sidecar, action frame and
question. The native model may still deny, hold or abstain. Quiet probes never
synthesize `Allow`. Native monitoring failures, incomplete output and malformed
categorical responses return errors, never substitute ballots. Progress reports
separate the numerical versus model basis and say whether native work started.
The selected probes are a declared inventory, not an assertion of complete
all-layer coverage or empirical detector qualification.

`SidecarNativePeer` connects this evaluator to the original `HelperClient` over
host-supplied bounded/nonblocking streams. It prepares one original commitment
and reveal from the completed result. There is no alternate response setter,
mutable evaluator or inference retry. A wire/evaluation error drops the client,
retains work, and lets the ORIGINAL coordinator treat the absent ballot as
missing. A caught unwind drops the locally owned connection and leaves a failure
latched before any retry. Cancellation destroys numerical and connection
ownership; completed observations and cost reports remain, not refunds.

Eleven additional regression functions cover actual uncertainty-driven
refinement, independent native computation and publication, a genuine native
Deny despite quiet probes, alarm/equality precedence, cooperative versus whole
evaluation, aggregate limits, whole-input mismatches, failed native monitoring,
invalid generated answers, cancellation, original socket commit/reveal,
missing-vote handling, and an injected transport unwind. Two additional
compile-fail examples prohibit a mutable native escape and caller-supplied peer
votes. The source model and helper weights are explicitly synthetic; the helper
fixture computes its answers through the original tokenizer, decoder, monitors
and strict verdict parser. No preselected vote replaces that computation.

Both receiver and evaluator require independently provisioned typed source
bindings in this process. This does not add portable authenticated source
import, a new process launcher, a sandbox, automatic deployment into the existing
socket driver, durable recovery or CLI wiring. Existing source-currentness,
congress, human-key and one-use effect checks remain unchanged. All eighteen
regression functions and four compile-fail examples are still unexecuted pending
the required RCH gate; this source addition is not production qualification.
