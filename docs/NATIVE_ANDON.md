# Native evaluator-driven Andon accounting

Source implementation, 2026-10-13. Plan section 9.9; FI-A08, FI-A13, FI-A18;
scoped continuation of `franken_alignment-credibility-ledger-dc0.4`. Neither the
bead nor FA-105 is closed. See also [evaluated import](EVALUATED_ANDON_IMPORT.md).

## Original consumer, not a replacement congress

`OversightBroker::enable_andon` binds an original `AndonLedger` to the original
native credibility protocol, complete committee input contracts and effect
scope. It must run after `enable_credibility` and before proposals or rounds.
Supply independently recovered accounting when continuing a deployment; a new
empty ledger does not recover old charges. A configured lane cannot be disabled
or rebound. `register_andon_window` uses the original immutable, nonoverlapping
window transition; it cannot enlarge an existing allowance or erase an incident.

The existing `apply_review` prepares the actual decision's accounting basis
before its original authority transition. Only successful application publishes
that basis. The existing `record_evaluation` then performs the original
process-local evaluator-issuer, origin-consistency and final-label checks, and
stages both the label and original Andon transition before publishing either.
Missing occurrence windows, conflicting old rounds, capacity, unavailable clock
or a backwards clock leave the label history, credibility revision and Andon
state unchanged. Adding a genuinely missing window permits the same label retry.
No later receipt-construction step asks the caller to restate a false-stop claim.

Only an applied stop with a purely empirical decision, complete non-abstaining
verdicts, at least one holding/denying helper and a final Benign assessment
produces a `NativeEvaluatedFalseHold`. Ready minority votes, exact disqualifiers,
missing evidence, contradictions, censored labels and genuine violations are not
charged by this lane. Their original credibility accounting is unchanged;
its broader lifetime false-stop count need not equal this narrower count.

## Time and identity

Native windows use the broker's trusted **elapsed-clock domain**, not the
counterfactual control-sequence domain of the offline campaign join. Occurrence
is the clock observed at successful review application; confirmation is the
original broker clock when the final benign label is accepted. A delayed label
charges the old occurrence window. An exact label retry neither reads a new clock
nor changes the first confirmation timestamp. Do not mix offline and native
clock domains under the same accounting identity.

The compact fact's report ID is the frozen native evaluation period. Holder
generations are the original helper contracts' nonzero **model epochs**; this is
an explicit profile, not an invented composite generation. The receipt retains
the full contracts, protocol, scope, actual EvaluationCase (including policy and
congress generations), original Assessment, exact occurrence-window budget and compact fact. Retain that complete
basis independently. The compact ledger record does not replace the source
assessment or establish global uniqueness/authentication of report IDs.

## Authority and durability boundaries

This feature has no policy, weight, roster, permit, refund or endpoint mutation.
Exhaustion opens the original calibration incident. Existing individual and joint
promotion checks are unchanged. The original IndependentEvaluator brand prevents
cross-instance label injection in this process; it is not a cryptographic or
institutional-independence proof.

`andon_ledger` and `andon_receipt` return in-memory observations. Label acceptance
is NOT a durable accounting acknowledgment. Persist the emitted receipt through
the original file adapter before reporting durability. Native evaluation-state
restart reconstruction, signed evaluator intake and automatic command-line
operator activation are not implemented by this increment. Recovered accounting
can seed a new owner, but it does not recreate absent native source receipts.

## Verification

Nine public native-path regression functions and three focused internal boundary
functions accompany the implementation, plus an opaque-construction compile-fail
example. Public tests exercise original proposal, capture, commit/reveal, apply,
native evaluator and label entry points; the focused rule/clock tests are logical
boundary cases, not live deployment evidence. Previous test assertions are not
changed. The two original source preimages match their retained Git blob hashes.
Targeted integration tests, targeted library tests and the unchanged xtask gate
were each attempted with `RCH_REQUIRE_REMOTE=1 rch exec` and exited 127 before
Cargo because `rch` is absent. This is a selected-file worktree, not a complete
checkout. Compilation, Rust tests, formatting and Clippy remain unexecuted; source
inspection establishes no latency or production qualification.
