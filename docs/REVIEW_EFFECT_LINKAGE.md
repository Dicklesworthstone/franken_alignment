# Exact review-to-effect audit linkage

The existing historical review cursor now offers `finish_with_effects()`. It
returns the unchanged `FileReviewReplay` plus a `ReviewEffectAudit` for that
review's exact attempt. This advances the FA-020/FA-021 evidence-closure path;
its consumer is the existing offline learned-review audit command. It does not
claim a complete production DecisionClosure, authenticate observations, or
introduce any authority, runtime, event format or new effect executor.

## The previously missing link

Whole-journal execution totals do not say whether a selected attempt executed.
A successful review can be superseded by another review of the same attempt;
a dispatched request can remain unresolved; publication can precede receipt
reconciliation. The new result keeps those facts separate rather than inferring
execution or a refund from a vote, an aggregate counter or an absent record.

A constant-size observer receives only successful original `Machine::apply`
results. It records the latest successfully applied review of the selected
attempt, then binds that review to the original automatic authorization and
its actual two-key dispatch. A later refused review cannot replace the review
which supplied the dispatched key. The recorded human status comes from the
original broker after consumption; neither key nor a sendable envelope escapes.

The first terminal outcome comes from the original endpoint transition, not
from a caller-supplied outcome or from accounting totals. The last per-attempt
reconciliation is recorded separately, including uncertainty, retention loss,
and inner sweep refusals. Individual queries, fixed publication cuts, sweeps
and terminal-stop sweeps all use their original results. An earlier observed
outcome remains historical data even when a later query loses retention.

`None` means no such observation in the verified history, never proof of
nonexecution. Cancellation/fencing cannot manufacture a receipt. The original
ledger supplies the attempt's stage at review completion and at the ending cut;
only the original reducer changes accounting. Global counts remain unchanged
and can still include other attempts.

## Completion and trust

The same frozen image, independent configuration, optional exact prefix anchor,
and complete-suffix verification govern both results. No selected-attempt result
is returned before the entire suffix verifies. All existing review APIs retain
their return types and original evidence. No extra replay, archive-input setter,
owner recovery, file mutation, helper invocation or live observation is added.

When an anchor covers the review but not its later effect, the later events are
verified suffix data, not independently anchored history. Neither the anchor
nor the replay authenticates a remote provider or a source's truth. The linkage
is scoped to the original journal-as-publication reference sink. Independently
retained custody still cannot protect a jointly rolled-back journal and anchor.

## Validation status

Eight authored regression functions use original two-key and canonical-file
publication. They cover guarded and legacy publication, single/sweep receipt
reconciliation, unrelated attempts, superseding and refused later reviews,
sealed nonexecution without an early refund, unknown recovery, all five storage
replacement barriers, and incomplete/cancelled/invalid suffixes. Three polling
schedules are compared against the unchanged original review and live snapshots.
The compile-fail example rejects conversion of the audit into a permit. Ballots
are synthetic controls, not measured helper quality.

Rust compilation and tests are unexecuted: the required remote-only `rch`
launcher is absent in this preparation environment. Hash, patch and whitespace
checks are not typechecking or runtime qualification. No bead or gate is closed.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::replay
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
