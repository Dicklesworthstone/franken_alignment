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

## Executable integration

As of the 2026-09-26 source addition, native `serve-create-checked` and
`serve-open-checked` accept the existing `fa.supervised-joint-publication/1`
envelope as their witness profile. The private complete selection carries the
joint policy to the corresponding original atomic constructor/opener; it cannot
silently become a witness-only deployment. Existing capture/producer preparation,
helper processes, independent human approval and final publication checks remain
unchanged. Baseline creation is not a held-out qualification.

An already-qualified interrupted generation can use the existing explicit
`--credibility-activation FILE` option with `serve-open[-checked]`. Fresh
activation passes the original individual AND configured joint gates before
numerical continuation. Exact historical requests bypass that file entirely and
retain source-free receipt recovery. The complete invocation, tests, governance
predecessor requirements and limitations are in
[NATIVE_REQUALIFICATION.md](NATIVE_REQUALIFICATION.md).

## Source and acceptance

Source is `crates/fa-reference/src/action/consequence/delivery/persistent/observed/stream/generated/checked/joint.rs`.
The adjacent `joint/tests.rs` contains five authored test functions: continuation
versus uninterrupted native computation across all three feed modes; changed
joint/witness/reserve rejection without a recovery write; rejection of a valid
checked-native image lacking the joint policy; invalid pre-store configuration
with a valid control at the same destination; and exact-one-policy preflight.
Numerical weights are explicitly synthetic fixtures. File operations are real
when executed; neither fixture quality nor source inspection is live evidence.

This composes the existing contracts of plan sections 7.6, 8.3, 8.7, 9.7 and 11
rather than changing their semantics. It adds no dependency, journal/wire
encoding, actor command, safety-policy relaxation or qualification claim.

## Execution status and change record

2026-09-25: library source addition, **UNVERIFIED**. The required RCH command was
attempted and stopped before compilation because `rch` is unavailable (exit 127).
Compilation, the five library tests, rustfmt, Clippy and the full gate remain
UNEXECUTED. Historical receipts do not validate this source; no bead is closed.
The prior checked-native bootstrap on main is preserved, not replaced.

2026-09-26: executable composition and native requalification source added.
Five executable regression functions plus a synthetic helper entrypoint and four
new decoder/governance regressions are authored. Fresh targeted tests and full
gate attempts again failed before compilation (`rch` missing, exit 127). No
runtime result or production qualification follows from these source changes.
The original joint evaluator, constructors and earlier test bodies are unchanged.
