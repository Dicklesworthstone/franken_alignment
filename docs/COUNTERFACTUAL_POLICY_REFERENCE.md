# Counterfactual policy experiments and finite repair search

## Status and consumer

Implemented source in the std-only `fa-reference` crate on September 9, 2026.
The consumer is a detached investigator asking which registered changes would
resolve an archived exact-policy violation, without acquiring the controller's
rights or pretending to know how helpers would judge the changed input.

**Execution pending:** no configured RCH runner was available for this batch.
Compilation, formatting, Clippy, unit/integration/doctest execution and the full
repository gate have not been run. Earlier receipts do not qualify this source.
No production admission or bead closure is claimed.

Feature changes: `PolicyExperiment` adds paired baseline/intervention evaluation;
`search_repairs` adds exhaustive finite-menu comparison and scoped minimality;
`policy_counterfactual` adds an offline file-consuming example. These are L7
reference capabilities, not an inference simulator or a replacement for the
production experiment executor.

## Paired experiments

[The implementation](../crates/fa-reference/src/action/consequence/experiment.rs)
first verifies `DecisionArchive` against the independently retained, pre-vote
`ReviewAnchor`. It then independently checks that its partial-evidence evaluation
reproduces the archived predicate trace. A self-consistent forged archive and
forged anchor remain outside this provenance contract.

`InterventionScope` freezes which action fields and keys are editable. Payload,
resolved target and resource-unit edits still pass `FrozenAction` validation.
Key changes name the exact expected original value or an observed absence;
unknown keys, wrong preconditions, duplicate variables and out-of-scope edits
refuse. A key inside an observed empty domain is a known absence, not unknown.
Policy, generations, principal, authority, helper roster and old vote values are
not editable through this interface.

Every branch starts from the untouched baseline. No successful or failed branch
mutates the archive, another branch, a controller, actor state, budgets or effect
history. The internal action is marked `Purpose::Experiment`; the public result
is a report, not a new action permit. Copying the experiment copies evidence only.

Reports retain the applied edits, baseline and candidate root results, all
candidate predicate results and per-node changes. Even a changed payload that
leaves every exact predicate satisfied invalidates the original empirical
judgment: those helpers saw different bytes. The next requirement is a fresh
independent review, an exact-policy violation, or more evidence. An empty or
semantically unchanged branch describes history, never renewed permission.

## Absence after intervention

The archive retains an observed slice, not the whole provider database. The
experiment separately tracks exact key facts and the union of closed observed
domains. Adjacent intervals and exact singleton observations compose; an
unobserved gap is not silently filled.

An empty-range witness supplies closure for that interval. A positive member
used to disprove emptiness supplies only an exact fact about that member. After
deleting that member, an otherwise unobserved range is **Unknown**, not empty.
After inserting into a previously closed empty range, its closure remains known
and its emptiness becomes false. Exact maximum-integer keys do not require an
overflowing successor; half-open range queries still exclude their upper end.

Unknown survives negation. A satisfied Boolean arm does not hide unknown nodes
elsewhere in the trace: that branch still requires more evidence. A known false
root can establish an exact violation even when another predicate is unknown.
No empirical vote is recomputed from edited evidence or treated as a simulator.

## Finite repair search

[The search](../crates/fa-reference/src/action/consequence/experiment/search.rs)
evaluates every subset of a supplied menu of at most eight edits, including the
empty control: at most 256 branches. Each mask retains either its full report or
its refusal. Alternative edits to the same variable therefore produce explicit
conflicting-subset refusals, not silently excluded failed experiments.

A sufficient repair satisfies all exact-policy requirements with no unknown
predicate. Its minimality is classified as:

- `EstablishedWithinMenu`: every proper subset establishes an exact violation.
- `NotMinimal`: a proper subset also satisfies the exact requirements.
- `Undetermined`: no smaller success is known, but a smaller branch is unknown or refused.

All proper subsets are checked, not only single-edit removals; arbitrary Boolean
policies are not monotone in edit count. This is inclusion-minimality within the
supplied menu, not a globally cheapest repair, a behavioral prediction, a causal
model of the world, or proof that the modified action deserves permission.

One branch permits at most 64 edits and 64 KiB of aggregate intervention bytes,
including expected values. Observations and repeated policy read occurrences
retain the existing 64-witness/64-KiB bounds. Search validates the whole menu's
byte bound before cloning subsets. It retains all reports, so output memory grows
with the enumerated menu; no benchmark or allocation claim is made.

## Offline payload comparison

The [example](../crates/fa-reference/examples/policy_counterfactual.rs) reads an
anchor, archive and replacement payload from caller-selected files. It uses the
existing canonical archive decoder, verifies the separate anchor and reports the
comparison. It constructs no controller, calls no adapter, uses no network and
writes no files. File reads are byte-bounded; no IO-latency, hostile-filesystem or
crash-durability guarantee is implied.

```sh
cargo run --locked -p fa-reference --example policy_counterfactual -- \
  anchor.bin decision.bin replacement-payload.bin
```

Repository compilation still uses the required RCH route. The command above is
the example's interface, not evidence of execution. Exit zero means comparison
completed, including a reported violation or unknown result. Output explicitly
says `permission=not_issued`; malformed inputs or IO/verification failure exit 2.
Neither payload bytes nor archived secrets are dumped into the report.

## Tests and relationship to the plan

There are six evidence-kernel unit tests, twelve public-API integration tests,
two example reader tests and one compile-fail doctest preventing report-to-permit
substitution. They are unexecuted test source. The public tests build genuine
reference review sessions and archives, pair repairs with unchanged controls,
check live-ledger isolation and original-permit binding, retain unseen-member
uncertainty and exercise joint repairs, non-monotone minimality, unknown smaller
branches, explicit conflicting edits and exact search capacity.

This serves plan sections 11.3 and 11.6 (authority-free branches and fresh
judgments after changed inputs) and section 12 (paired interventions without a
persuasion channel), grounded in FI-A16 and FI-I11. It does not establish H5,
execute transformer counterfactuals, authenticate providers, supply durable
experiment storage or satisfy the full production experiment/rehearsal packets.
