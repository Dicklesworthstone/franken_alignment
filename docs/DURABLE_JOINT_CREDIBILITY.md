# Durable joint-error validation before credibility promotion

## Scope and implementation status

FA-105 increment; plan sections 9.4, 9.7, 9.9 and 22.4; FI-A07/FI-A08.
The consumer is the existing full-input, two-key FileOversight publication owner.
This implements the missing durable route to the native owned-round joint replay
policy. It is not a new scoring model, rights ledger, trained-helper evaluation,
calibrated population bound, authenticated label source or production qualification.
The packet remains open pending independent verification and its other conditions.

## Native policy, durable enforcement

FileOversight::enable_joint_credibility installs the existing EvaluationProtocol
and JointPromotionPolicy in ONE acknowledged bootstrap event, returning the
independent evaluator role only afterward. It must precede every proposal/request.
There is no disable, policy replacement or late marginal-to-joint upgrade.
The separate held-out-capsule lane remains mutually exclusive with owned-round
credibility; this feature does not weaken or claim to validate that other lane.

The unchanged promote_credibility operation invokes the ORIGINAL native broker.
All owned current-policy cases participate. Marginal requirements must first pass;
then native joint replay compares baseline and candidate congress reductions,
groups shared-origin repeats, rejects newly introduced misses or false stops,
applies declared aggregate ceilings and enforces the complete replay budget.
No caller can pass a preapproved report, choose another candidate or omit cases.
A refused promotion leaves both the acknowledged journal and authority unchanged.

joint_credibility_report is a read-only native preview, not a permit.
joint_credibility_promotion(operation) retrieves the report actually consumed by
that historical operation, bound to its original evidence revision and before/after
policies. It does not recalculate history using today's weights. Exact promotion
retries retain the original behavior: no second journal event, refund or fencing.

Native successful promotion still cancels undispatched work, revokes old human
keys and preserves dispatched liabilities. Only the original endpoint's terminal
receipt resolves a charge. Fresh publication still uses both original approval
keys and the existing full-input/policy/first-publication checks.

## Format and compatibility

Credibility subtag 5 contains the original evaluation protocol plus the joint
policy's fixed bounded fields. Existing subtags 0 through 4, outer journal domain
and original bootstrap/profile bytes are unchanged. Decoding constructs the native
JointPromotionPolicy and rejects invalid fractions, identifiers and budgets.
Reports, reduced weights, permits, balances and inferred independence never enter
the encoding. Replay recomputes every actual promotion from original observations.
Legacy marginal-only journals remain explicitly marginal-only; they do not acquire
a joint-validation claim. The new event is bootstrap work, not recovery-tail work.

## Validation and changelog

2026-09-21: add durable native joint bootstrap, strict codec, preview/history APIs,
and nine authored Rust regressions. The scenarios include the native complementary
helper counterexample: marginally qualified 10-to-5 weights can newly pass two
violations at hold allowance 5, while the paired allowance-0 policy still publishes
with both fresh keys. Other cases cover replay, unknown charges, pending cases,
finite budgets, legacy bytes, historical retry and all five injected storage
barriers. Synthetic cases establish intended protocol behavior, not detector skill.

Rust compilation, formatting, Clippy and tests are UNEXECUTED in this environment.
The required RCH xtask invocation cannot start: rch is unavailable (exit 127), and
cargo/rustc/rustfmt are absent. Source hash checks and lexical/diff screens are not
substitutes for that gate. No bead or qualification gate is closed.

## Pinned startup, recovery and evaluator custody

The composed guarded owner now supports create_jointly_evaluated_guarded,
open_jointly_evaluated_guarded and read_joint_credibility. Creation prepares the
existing guard inventory and combined evaluation bootstrap BEFORE making storage,
then publishes ONE first canonical image. Only acknowledgment returns the owner
and its separately held human, evaluator and any configured auxiliary roles.
No live getter recovers an evaluator from an actor-owned handle or report.

Recovery independently pins the complete evaluation protocol and joint policy,
including both rate ceilings and logical work budgets. It checks them before
recorded numerical work, then checks the original guard inventory, effective
policy, credential epoch and external minimum journal/control/authority floors
before cleanup or the single original recovery fence. Missing configuration,
legacy marginal-only configuration, different joint requirements and stale floors
are refusals, not defaults. The old evaluated opener refuses a joint-only image
rather than pretending the caller selected its joint policy. Generic native replay
still preserves whatever guard was explicitly installed; it does not drop it.

The native fence invalidates old approval keys and role brands and requires a new
clock observation. It does not erase labels, pending/censored cases, consumed
joint reports or dispatched charges. A newly issued evaluator receives newly
branded tickets and can resolve a retained censored case through the ORIGINAL
label transition. It cannot rewrite final ground truth or make unfinished cases
vanish. Independent floors are lower bounds, not cryptographic protection against
equal-counter forks, dishonest storage or stale operator-held floors.

Read-only inspection checks the same exact configuration and reconstructs both
marginal evidence and joint promotion history without issuing roles, changing the
journal, or requiring the writer lock. It is historical evidence only.

The startup/recovery increment adds six authored Rust tests (fifteen in total).
They cover one-image creation, read-only inspection beside a locked owner, exact
configuration/guard/floor mismatches with untouched staging files, refusal to
substitute marginal and joint openers, independent role and ticket freshness,
censor resolution followed by real two-key publication, and all five injected
creation/recovery storage barriers. Source review corrected a new test's private
import without widening any production visibility. Compilation, formatting,
Clippy and Rust tests remain UNEXECUTED; the repeated required RCH gate cannot
start because rch is absent. No evaluator authenticity, detector accuracy,
production activation or FA-105 completion is claimed.
