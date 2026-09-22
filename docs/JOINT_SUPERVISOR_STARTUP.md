# Joint qualification with the supervisor's final-publication guards

FA-105 / FA-062 integration; plan 9.4, 9.7, 9.9 and 8.8; FI-A07/FI-A08.
The consumer is the existing runnable publication supervisor. This is configuration
of original gates, not a scoring model, new effect authority, or qualification of
helpers. Native evaluator and storage authenticity assumptions are unchanged.

## Atomic native composition

`JointPublicationProfile` composes the immutable `HeldOutJointPolicy` with the
existing optional witness limits and optional change-feed/freshness policy. A
feed requires witness validation. Exact-snapshot fallback is selected explicitly;
prefix routing is pinned and a subtree-routing journal is not reinterpreted.

`FileOversight::create_with_joint_publication` validates the original transition
sequence before creating storage and publishes one first canonical image. Both
the owner and separate human role are returned only after acknowledgment. There
is no intermediate owner with only one of the selected gates. Existing bootstrap
events are reused, so no event tags or evidence-capsule formats change.

`open_with_joint_publication` pins every selected joint, witness, change, freshness
and fallback field before native replay, cleanup or the original recovery fence.
None means absence, not "accept any saved policy". Existing source, identity,
credential and other gates still run in native replay; this narrow profile is
not the broader FileGuardSet contract or an independent anti-rollback floor.
`read_joint_publication` checks one canonical cut without a writer lock, cleanup,
role issuance, source acquisition or journal mutation. Reports remain historical.

The joint policy constrains credibility promotion; it does not prevent acquisition
of baseline observations before any activation. Once activated, original freshness,
recovery invalidation, capped weights and joint comparison all remain mandatory.
Human approval and the automatic permit still control dispatch; final witness
validation still controls first publication. Lost evidence is not a nonexecution
receipt, and unresolved charges remain until the original endpoint settles them.

## Implementation status and changelog

2026-09-21: add native one-image composition, exact pinned recovery and read-only
inspection. Five authored Rust regressions cover all four supported compositions,
configuration/absence mismatches before cleanup, invalid configuration before
storage, all five original I/O barriers for creation and recovery, and paired
original two-key publication versus post-dispatch whole-input loss. The latter
keeps the charge until actual endpoint sealing and receipt reconciliation.

The required RCH xtask command was attempted and could not start (exit 127: rch
unavailable). Rust compilation, formatting, Clippy and tests are UNEXECUTED. Source
hash and lexical/whitespace checks do not substitute for the required verifier.
No bead, production activation, statistical bound or authenticated evaluator claim
is made. Supervisor command integration follows in the next implementation step.
