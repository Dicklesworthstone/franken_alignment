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
