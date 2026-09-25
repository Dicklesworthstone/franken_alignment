# Native checked publication with joint-helper qualification

## Capability and boundary

`FileOversight::create_generated_text_stream_with_joint_publication` composes the
existing native-only complete-message stream, logical recovery reserve, exact
publication witness profile, optional producer change/freshness/snapshot profile,
held-out joint-helper policy, model, monitor, sampler and tokenizer in one first
canonical image. It reuses `GeneratedPublicationProfile` and the original
`EnableHeldOutJoint` event; it does not create a second evaluator or authority.

The baseline native generator remains usable. Promotion of helper credibility
retains the original independent joint-evaluation conditions. A completed model
message still needs original helper review, an independent human key and fresh
publication validation. Construction alone grants none of them.

`open_generated_text_stream_with_joint_publication` checks the complete original
native/witness selection and exactly one independently supplied joint policy
before numerical replay, cleanup, role issuance or fencing. Missing joint policy
is a mismatch, not an opportunity to retrofit one. The original recovery fence
preserves acknowledged numerical progress and liabilities while pausing inference
and withdrawing old keys and active qualification. This is not an independent
anti-rollback anchor, process-isolation proof or remote-publication transaction.

`check_generated_text_joint_publication` checks configuration on an existing
owner before source preparation. Success does not mean a source is fresh or the
helpers are currently qualified.

## Source and acceptance

Source is `crates/fa-reference/src/action/consequence/delivery/persistent/observed/stream/generated/checked/joint.rs`.
The adjacent `joint/tests.rs` contains five authored test functions: continuation
versus uninterrupted native computation across all three feed modes; changed
joint/witness/reserve rejection without a recovery write; rejection of a valid
checked-native image lacking the joint policy; invalid pre-store configuration
with a valid control at the same destination; and exact-one-policy preflight.
Numerical weights are explicitly synthetic fixtures. File operations are real
when executed; neither fixture quality nor source inspection is live evidence.

This composes the existing contracts of plan §§7.6, 8.3, 8.7, 9.7 and 11 rather
than changing their semantics. It adds no dependency, journal/wire encoding,
actor command, safety-policy relaxation or qualification claim.

## Execution status and change record

2026-09-25: source addition, **UNVERIFIED**. The required RCH command was attempted
and stopped before compilation because `rch` is unavailable (exit 127).
Compilation, the five tests, rustfmt, Clippy and the full gate remain UNEXECUTED.
Existing historical receipts do not validate this source; no bead is closed.
The prior checked-native bootstrap on main is preserved, not replaced.
