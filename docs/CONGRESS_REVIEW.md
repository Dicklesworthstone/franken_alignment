# Frozen congress review (reference integration)

This source connects the existing commit–reveal transcript to the existing
bounded, cohort-capped empirical reducer (plan §§9.2–9.3; FI-A06, FI-A08,
FI-A14). It is an independently usable reference composition, not completion
of the native-runtime or production-broker roadmap packets.

`round::review::ReviewPolicy` freezes member identities, cohorts, weights,
influence caps and decision thresholds before a `CongressReview` accepts any
commitment. Registration is bounded by the reducer's 128-member/128-byte
identifier profile. Impossible thresholds and overflowing complete tallies
refuse at registration. Evidence-view bytes and round identity also freeze.

Only validated reveals enter the reducer. Every registered member must reveal;
missing members and explicit abstentions hold separately. Positive authorization
requires sufficient admitted permit weight, a bounded hold weight, and enough
permit-bearing cohorts after clipping. An empirical Deny is a hold vote, not an
exact proof. Caller-supplied `ExactStatus::Disqualified` dominates the empirical
lane; unknown exact evidence cannot yield a Ready decision. There is no actor
statement input and no API for substituting a caller-computed tally.

The positive observable is a complete committed affirmative congress producing
the actual capped reduction. Paired negatives cover missing/abstaining members,
changed reveals, independent thresholds, same-cohort clipping, registration
bounds and arithmetic overflow. Tests are authored in
`crates/fa-reference/tests/congress_review.rs` but have NOT been executed in the
editing environment: Rust, Cargo and RCH are unavailable. Existing historical
qualification receipts do not validate this addition. No bead is closed.

The enclosing round still uses its documented non-cryptographic FNV comparison
oracle. This code establishes neither real commitment security nor helper
independence, calibration, provider authentication, durable authority, external
effects or production readiness. The exact-status input remains a trusted
reference assumption, not evidence that an external verifier ran.

## Reviewed action authority

`action::congress::CongressAuthority` is the L4 reference consumer of this L3
review. Its trusted constructor freezes a policy and privately owns the
existing `ReferenceAuthority`; no mutable inner authority or raw `Permit`
escapes. It exposes the existing proposal, preparation, review, authorization,
dispatch and reconciliation lifecycle. It accepts neither a caller-supplied
`Reduction` nor a replacement transcript.

Before accepting commitments, `propose` binds the attempt, every frozen action
field (including scope, resolved target, payload, witnesses, policy epoch,
deadline and units), snapshot semantic epoch, complete registered voting
policy and exact declared helper input. Fields are length-framed and retained
as bytes, not summarized by FNV. The whole challenge must fit the enclosing
round's **4096-byte** bound, including framing. This is narrower than the raw
action payload bound. Overflow refuses before inserting an attempt; it never
truncates dependencies. The challenge is a reference preimage, not a new wire
format or production evidence commitment.

Authorization consults the owned complete review and original captured logical
judgment, then reserves through the existing authority. Dispatch rechecks the
binding, snapshot completeness, semantic epoch, exact witnesses and fresh
caller-supplied exact-validator status before the original one-use transition.
A stale epoch or elapsed deadline still refuses. Failed checks do not refund a
reservation. Dispatched or unknown effects retain their original liability;
only an explicit trusted nonexecution outcome permits post-dispatch refund.
The wrapper does not turn a caller's cancellation into nonexecution evidence.

Twelve public integration tests in
`crates/fa-reference/tests/congress_authority.rs` cover a complete positive
lifecycle, missing and nonaffirmative votes, exact disqualification, evidence
changes before and after authorization, expiry/revocation, permit isolation,
budget contention, unknown reconciliation, a policy substitution, and exact
challenge capacity with a one-over refusal. Three compile-fail cases document
non-clonable authority/permits and refusal to wrap a raw permit externally.
Together with the seven review tests, these are **19 authored runtime tests
and 3 authored compile-fail cases, all UNEXECUTED**. The required command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` was
attempted and stopped before compilation because `rch` was absent (exit 127).
No compiler, rustfmt, Clippy or runtime pass is claimed; no historical receipt
qualifies these changes and no roadmap or bead is closed.

Trusted bootstrap, clock readings, snapshot authenticity, actual helper input
capture, exact-validator results and remote-outcome facts remain assumptions.
Review commitments still use the enclosing reference-only comparison oracle.
No helper process authentication, durable journal, credential broker, external
publication, production runtime admission or production safety claim follows.
No agent-facing verb, wire format or dependency is introduced.
