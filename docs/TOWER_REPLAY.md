# Paired policy replay through the Tower

`fa_reference::tower` connects the registered `replay` operation to the original
observed-history verifier and `PolicyExperiment` engine. It reconstructs a real
historical decision, evaluates a host-registered intervention against the same
retained evidence, and returns the actual baseline and treated policy results.
The result is a sealed `TowerRehearsalOutcome`, labeled `rehearsal: true`, with a
complete native `CounterfactualReport`. It cannot become a `Permit`.

This is the bounded `ObservedPolicyExperiment` profile of L7 decision replay.
It does not run new helpers, restart a model, predict an external effect or renew
historical approval. The independent `verify` verb remains unsupported: the
plan's isolated verifier must not share the policy code used by this replay.

## Inspect the plan before running an experiment

`Verb::ReplayPlan` is a cheap read at the **installed branch artifact address**.
It returns that artifact's historical source address and complete source basis,
the actual intervention registration and the requirements for executing replay.
This profile does not implement generic discovery from an arbitrary closure
address, durable retention leases or a verified model-restart grade.

The metadata contains the original native scope, attempt, round and sequence
coordinates, receipt presence, retained-data charge, registration ID, permission
flags for payload/target/units, and every registered key in sorted order. Keys
are not silently filtered to those currently observed. The wire also names the
native edit count/byte bounds, the logical operation cost and the required
operator, full-domain, Experiment-context and exact-key admission.

An authorized reader can inspect a plan without operator status or an Experiment
purpose grant. Both possible source families are admitted before record lookup
so a hidden source cannot disclose its existence. Partial callers receive the
authorized plan with sensitive native metadata `Withheld`; full-domain callers
can inspect that metadata. Global accounting is disclosed only to a full-domain
operator, as on the other Tower reads.

`Known` plan metadata means the registration was inspected. It does not mean the
archive passed replay. The planner calls no verifier, performs no evaluation and
looks up no cached experiment. All nine assurance properties remain `Unknown`,
including after a previous replay. An installed but tampered history can have a
truthful plan and subsequently fail the real replay checks.

Each admitted plan read consumes one read unit, including missing, stale or
future records. It appends no branch event and occupies no replay operation
slot. An optional request key does not enter the experiment cache. Malformed
arguments and denied source scope fail before a read charge. The outer result
keeps the branch artifact's registration basis; the historical source basis and
metadata basis remain in their separate original domain.

## A real baseline and a real intervention

The host supplies an `ObservedReplayRecord` containing an original
`ObservedDecisionArchive`, its separately retained pre-vote
`ObservedReviewAnchor`, an optional original `ObservedReceipt`, the explicit
native `Scope` and an `InterventionScope`. Installation validates identity and
resource bounds. It does not derive the expected anchor from the archive and
does not claim that installation successfully verified the history.

An admitted request first invokes the original full-input and timing-aware
archive verifier, including receipt binding when a receipt is supplied. It then
constructs the original `PolicyExperiment` and runs the requested interventions.
Every run starts from the immutable historical baseline.

The registered intervention can permit payload, target, units and selected exact
keys. A key edit names its expected original value; `None` means observed absence,
not a wildcard. Missing evidence does not become an empty domain. Duplicate,
out-of-scope or stale edits retain the original engine's refusal behavior.

If a real edit repairs a policy violation, the report can change from `Violated`
to `Satisfied` while the original replayed congress decision remains `Deny`.
The old votes become `InvalidatedByIntervention`, and the next requirement is
`FreshIndependentReview`. No new `Continue` consequence is invented. Empty or
no-op interventions remain `HistoricalOnly`.

## Provisioning and execution

1. Create a branch `DomainEventJournal` with an actual `ExperimentCx`, then build
   a `TowerBuilder` using the branch's own basis and finite operation allowance.
2. Call `enable_replay(cx)` to transfer that context into the Tower. Its branch
   must match the journal, its numeric tenant/run must be nonzero, and its epoch
   must match the branch basis. A purpose tag in an actor request cannot replace
   this owned context.
3. Import a branch-owned `ResolvedObject` identifying the experiment artifact.
   Call `insert_replay(address, branch_basis, source_address, source_basis, record)`.
   The source is an explicitly versioned closure or receipt. A receipt address
   requires an actual retained receipt. Native tenant/run identity must match
   the installed context, and the experiment branch must differ from the native
   historical branch.
4. Dispatch `Verb::Replay` at the installed branch artifact with a nonempty
   idempotency key. `ReadArguments::Replay { interventions }` supplies the edits;
   `ReadArguments::None` requests the unedited baseline control.

Execution additionally requires operator access, complete domain visibility and
the host's allowed `Experiment` purpose. The historical source address must be
visible. Production journals, missing contexts and unauthorized callers fail
before native replay or cache lookup.

The [public integration tests](../crates/fa-reference/tests/tower_replay.rs)
construct genuine oversight sessions, commitments, reveals, archives and receipts
through the original APIs, then use these provisioning steps. They retain the
original broker and endpoint to check that replay does not change either owner.

## Three separate ordering domains

| Domain | Retained identity |
|---|---|
| Historical Tower source | Original closure/receipt address and complete source `Basis`, including its original control cut and proof states |
| Native historical decision | Original native scope, attempt, round, expected control sequence and optional actual receipt sequence |
| Current experiment branch | Fresh `ExperimentCx`, branch artifact, branch policy epoch and branch journal sequence |

These sequence numbers are not compared as one clock. A historical source cut
can be greater than the fresh branch's initial zero sequence. A current branch
epoch can differ from the source's historical policy epoch. The source basis is
never rewritten to fit the branch's object-import rule; it is carried separately.
The original source policy epoch still must match its retained native action.

After a successful experiment, one actual `BranchOutcome` event is appended to
the isolated branch. Its domain-separated reference checksum covers the complete
structured result. This checksum is not a cryptographic authentication claim.
Only then does the branch basis advance to that event's sequence. Subsequent
`tail` and `situation` reads observe the branch's new cut; imported object and
historical source bases remain unchanged.

## Exact retry and bounded work

Each new admitted replay consumes one logical operation unit. The original
authority ledger and effect budget are never copied, spent or refunded by this
facade. The unit is a bounded query allowance, not a measurement of inference
time, memory traffic or the original effect's cost.

The owned Tower caches the complete admitted request and its response. Reusing
the same key with the exact request returns the original response without another
experiment, charge or event, even after the allowance is exhausted. Changing the
operation ID, target, generation, arguments or edits under that key fails.
Every retry rechecks current caller grants before accessing the cache. This is
process-local ownership; no durable or cross-instance retry guarantee is claimed.

Malformed requests, missing keys, forbidden contexts, stale target generations,
oversized edit collections, exhausted capacity and missing replay sources fail
before native work. A failed native verification or semantic edit consumes its
admitted unit, caches explicit uncertainty and appends no success event. A
completed experiment whose response exceeds the caller's output bound retains
its branch event and charge, returns explicit uncertainty, and caches that exact
refusal. It never truncates a result while calling it `Known`.

Imports are limited to 32 records and 8 MiB total logical retained-data charges.
There are at most 64 cached replay operations per Tower, in addition to the
existing operation and journal limits. Original native bounds constrain all
public archive collections, input metadata and intervention bytes. The importer
also bounds the receipt's public stopped-sibling collection. Retained copies of
source data and admitted requests discard caller-owned spare buffer capacity.
The complete summary has an absolute 1 MiB encoding cap, and the ordinary Tower
response cap can be smaller.

## What the report establishes

The wire response includes the original decision and rules, evaluation trace and
witnesses, full vote tally, missing and abstaining members, native identity and
control cuts, complete intervention bytes, baseline and treated truth, per-node
changes, next requirement and empirical status. Bytes are losslessly encoded as
hex; missing bytes and present empty bytes remain distinct.

Replay consistency and retained-anchor binding become `Known` only after the
original replay succeeds. Receipt binding becomes `Known` only when an actual
receipt is supplied and checked. Those observations retain the complete original
source basis. Authentication, independent-verifier isolation, provider truth,
external execution, current eligibility and full ledger accounting remain
`Unknown`. The branch result's outer basis does not upgrade those source claims.

This profile leaves independent verification, authenticated context provisioning,
durable retention and retry recovery, model replay, native mutation dispatch and
full deployment qualification open. The common interface is documented in
[TOWER_REFERENCE.md](TOWER_REFERENCE.md).
