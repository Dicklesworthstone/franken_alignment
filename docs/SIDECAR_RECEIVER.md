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
