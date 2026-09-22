# Joint regression control for held-out credibility

## Native scope and status

FA-105 increment; plan 9.4, 9.7, 9.9 and 22.4; FI-A07/FI-A08. The consumer is
PolicyAuthority's existing held-out activation. This is an implemented reference
promotion guard, not detector qualification, independent evaluator authentication,
a population confidence bound, or closure of the larger work packet.

PolicyAuthority::enable_held_out_joint installs an immutable policy before any
proposal, authority epoch/sequence change or activation. It governs promotion;
it does not replace baseline review or automatically require a newly initialized
authority to present future campaign evidence. There is no disable, replacement,
late upgrade or alternate weight-installation bypass. Legacy authorities that
never select the guard retain their original behavior and claim no joint check.

The existing activate_credibility entry point still derives weights through the
original complete, per-helper/per-stratum requirements and influence caps. Before
changing rights, it compares the baseline and the ACTUAL capped candidate through
the original Congress reducer on every sealed case. It never trusts a caller's
report or reduces an already selected subset. Every newly missed violation or
newly false-stopped safe case blocks activation, even under loose aggregate rate
ceilings. Each declared stratum must separately satisfy the fixed root counts and
rate ceilings. Exact policy disqualifiers are not used to conceal empirical misses.

The immutable report retained beside the original activation snapshot records
both policies, per-case decisions, grouped evidence roots, per-stratum counts,
logical work and failures. Historical retries retain their original receipt and
report, without rechecking under later weights or invalidating newer work.
Withdrawal, reset and policy changes cannot erase the guard or historical reports.
Dispatched/unknown liabilities and original receipt-only accounting do not change.

## Evidence and time limits

The full declared case/member comparison work must fit its budget before any
re-tally. Pending/censored labels, missing observations and abstentions refuse.
A hold at or after the counterfactual dispatch position is insufficient: the
record does not tell us its earlier vote. It is neither invented as Allow nor
reconstructed as a missing mandatory vote that would deceptively stop a violation.
Clear observations and earlier holds remain caller-trusted campaign facts.

Known duplicate evidence roots count once, using the worst replicate for each
policy. A newly regressed replicate still fails even if another replicate already
failed. The same root cannot change its ground truth or fill a second stratum.
Different roots are NOT evidence of independent scenario origins; undetected
shared-origin correlation remains an explicit limitation. These are descriptive
counts over the sealed manifest, never multiplied marginal error probabilities.

## Changelog and validation

2026-09-21: add the immutable native guard, original-reducer joint comparison,
activation-history reports, and central rejection of an alternate weight setter.
Ten authored Rust integration tests pair the complementary-helper regression
with permitted original review/authorization/dispatch, capped influence, retained
unknown charges, historical retry, late or missing evidence, duplicate roots,
per-stratum limits, false-stop regressions and complete work-budget admission.
Existing authority and credibility test bodies remain unchanged.

Compilation, formatting, Clippy and Rust tests are UNEXECUTED. The required
RCH xtask attempt cannot start because rch is absent (exit 127); cargo, rustc and
rustfmt are absent too. Exact baseline/source hash and scoped whitespace checks
are not substitutes for the Rust gate. No qualification or bead is closed.

## Durable two-key integration and compatibility

The original DeliveryBroker and OversightBroker now expose this bootstrap guard.
Owned-round evaluation and held-out joint evaluation refuse each other's bootstrap
in either order, before any labels or effects. The central native setter also
refuses an alternate weight-installation path after the held-out guard is selected.
Ordinary held-out activation still derives its candidate and consumes the native
joint check; there is no second, optionally unguarded activation operation.

FileOversight::enable_held_out_joint records the same policy as bootstrap input.
create_with_held_out_joint prepares the mandatory first-publication guard and
original two-key owner, then publishes the joint policy in ONE initial canonical
image before returning a human role or writable owner. The marker is bootstrap
work, so the original terminal recovery reserve may still be installed afterward.
The evidence capsule, activation/withdrawal frames and actor protocol do not change.

open_with_held_out_joint pins every joint-policy field and the ordinary bootstrap
before native replay, cleanup or the original recovery fence. It does not claim
independent rollback floors or replace the broader composed guard inventory.
Other configured guards retain their native replay semantics. Generic opening
cannot drop this selected guard. The original recovery fence invalidates prior
held-out qualification and old automatic/human keys while retaining liabilities;
an old activation/report cannot reopen admission. New activation must still pass
both original individual requirements and the frozen joint policy.

read_held_out_joint checks the same policy and reconstructs reports from original
input history without a writer lock, cleanup, role issuance or journal mutation.
It can inspect beside a locked owner. These are historical reports, never fresh
evaluator observations or current effect permission. Existing withdrawals and
receipt-only settlement remain unchanged, including failed-loss recording rules.

The new fixed-size credibility subtag is 6. Subtags 0 through 5 retain their
original encodings. This increment also restores module, bootstrap and codec
connections to the PREVIOUSLY implemented owned-round joint gate that the later
integration commit had disconnected. Its existing source tests and guarded
creation/recovery implementation are retained, not replaced by the held-out gate.

Eight further tests are authored: five actual filesystem/two-key integration
cases and three wire/storage-barrier cases. They pair rejected joint promotion
with working publication, preserve reports and unknown charges through reopening,
require new qualification and both new keys, preserve newer approvals through
historical retries, check exact pins before staging cleanup, and retain final
policy revalidation. Codec vectors include all truncations and invalid fields;
creation and recovery exercise all five existing injected storage barriers.
The original joint-evaluation tests are reconnected, not counted as new tests.

The required RCH xtask was attempted again and exited 127: rch is unavailable.
All eighteen newly authored Rust tests, compilation, formatting and Clippy remain
UNEXECUTED. Hash and scoped source checks are not execution evidence. No bead,
production activation, calibrated error-rate or authenticated evaluator claim is
made. Selection through a new supervisor CLI bootstrap option is not included;
existing supervisors that open a selected journal retain its native enforcement.
