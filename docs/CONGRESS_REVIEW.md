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
