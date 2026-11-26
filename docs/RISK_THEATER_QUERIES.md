# Risk-theater queries over declared control history

The FA-125 query projection derives all six oversight query families from typed
control-history records: consequence drift, Andon incident closure modes, helper
retirement near confirmed holds, coverage erosion, autonomy inflation and audit
funding shortfalls. The consumer is the existing bounded score/support detector;
callers supply facts instead of family scores.

The implementation belongs to L8 Governance and serves FI-A18 and FI-A13 through
the existing FS-08 concordance. It implements a bounded reference contract for
plan sections 15.8 and 15.10. It is not a native journal adapter, an authentication
service, or a policy writer. The live Tower still reports all six full query
families Unknown until their native input contracts are installed and verified.

## Bind rules and history before evaluating

Construct an immutable `QueryPolicy` with the exact run scope, query generation,
independent evaluation-owner address, retirement window and minimum autonomy
dwell. A snapshot retains that complete policy; a display generation alone
cannot hide a change to its rules. Select `with_extended(ExtendedQueryPolicy)`
before constructing the engine to install the consequence, Andon and coverage
comparisons. Their exact rational thresholds and minimum sample/incident counts
also become part of that immutable snapshot policy. Invalid fractions, zero
minimum counts and extended facts without the selected profile refuse. The
reference defaults are explicit convenience parameters, not governed live policy.

A closed frontier asserts completeness of the fixed roster, autonomy and audit
source domains for this reference profile. An empty declared domain is therefore
a complete empty result for those predicates. This declaration cannot stand in
for a missing or uninstalled native source; the other query families remain
incomplete until their own comparison contracts are selected. Even when selected,
the comparative predicates need their actual windows or transitions: no comparison
history does not certify absence of drift or erosion.

The reference profile uses one explicitly declared control-sequence domain for
frontiers, event occurrence, deadlines and dwell periods. Submit every consecutive
boundary, including empty ones. A newly inserted record must occur at its current
boundary. A source cannot backdate a new event into an already closed boundary.
A delayed assessment is a new record at its actual reference control boundary.

Each history record has a canonical address and typed facts. An exact retraction
can withdraw a currently retained record. It does not erase the accepted input
history or free lifetime identity capacity. Restoring the exact original
observation is allowed; reusing an address for different bytes refuses, so a
changed observation needs its own identity. Whole-boundary admission is atomic:
malformed input or resource exhaustion preserves the preceding snapshot.

All source and completeness declarations remain caller assertions. A closed
reference boundary does not authenticate a live ledger or prove that an external
source supplied every event.

## Distinguish absence from unavailable evidence

A report reference has three states:

- `Absent { closed_through }` declares observed absence of a report carried by that
  exact transition, under its complete attachment-domain boundary.
- `Unavailable` says the report evidence is missing.
- `Present(address)` names an exact report record, which must bind the action,
  subject, family and evaluation owner and precede the transition.

A named report whose source record has not been supplied is incomplete evidence.
A prepared but unused report does not authorize the transition. A late report
cannot retrospectively authorize a transition. An independent
positive violation, such as insufficient dwell, can remain observable while
another prerequisite is unavailable.

Every family result preserves completeness alongside its support-backed
findings. An incomplete family with no findings is not an observed clean system.
The aggregate score kernel must not be published without that query coverage.

## Six semantic query families

| Family | Derived measurement and finding | Boundary that prevents a misleading result |
| --- | --- | --- |
| Consequence drift | Continue share increases between two closed windows while current canary escapes reach the configured rate. | Derive both denominators from original rounds; compare exact effect, helper and policy generations; require independent resolved labels and one sampling lineage. |
| Andon closure modes | The configured number of exhausted original budget windows have incidents closed through roster changes. | Bind each false-stop label to one original held round, the predeclared budget, the original crossing, and exact incident closure authorization. |
| Roster churn near holds | A helper is retired within the configured window of a confirmed violation hold without a qualified report carried by that retirement. | Match exact helper generation, effect family and independent assessment; a late, unrelated or unused report does not approve the action. |
| Coverage erosion | A registered mediation level falls, or a previously captured channel disappears under the same capture contract. | Check dimensions separately and require the actual active predecessor; improving one dimension cannot hide loss in another. |
| Autonomy inflation | Promotion violates dwell, skips a grade, or lacks its qualified carried report. | Follow the immediate principal/family predecessor; passport changes reset the grade and dwell basis. |
| Audit-floor starvation | Scheduled work lacks its required funding at its original due boundary. | Bind funds to the exact audit, use checked sums and count only funding received by that deadline. |

### Delayed independent resolution of held cases

A roster hold may first receive a censored assessment and later one definitive
Benign or ConfirmedViolation assessment from the same configured evaluation
owner and generation. The final assessment must occur strictly after the
censored assessment and both must follow the original hold. A confirmed
violation can then open the supported retirement finding; a benign resolution
is the nearby clean control. Both original assessment records remain in the
finding's support and in retained source history.

Repeated censoring, multiple final records, same-boundary resolution and final
label replacement refuse atomically, even before any retirement is selected.
Missing hold evidence or a foreign evaluator keeps the query incomplete without
hiding conflicts between the configured owner's retained labels. Exact source
retractions and restorations retain the existing correction contract; withdrawing
the final assessment or its hold retracts the dependent finding.

### Consequence windows and canary identity

A window closes an occurrence interval `[start, end)`. The baseline ends before
the current interval begins; both windows share the exact effect/helper/policy
group. Continue-rate increase uses checked integer cross-products, with inclusive
comparison at the configured rational threshold and no floating-point rounding.

Canary origin is globally unique in the retained source projection. Renaming its
campaign or round cannot make a second sample. Outcomes come from the configured
evaluator identity and stay attached to the original occurrence window, even when
the label arrives later. One censored label may resolve once; competing final
labels or ambiguous same-boundary assessments refuse. Missing/censored outcomes,
mixed sampling lineages and insufficient denominators remain incomplete.

This bounded profile supplies one baseline/current pair per exact group. Selecting
comparison windows and asserting complete inputs remain responsibilities of its
reference caller; this is not automatic rolling-window production monitoring.

### Original Andon budgets and closure history

Budget windows are registered no later than their start and do not overlap. A
late independent label charges its original held round once, using that window's
original allowance and fixed per-stop cost. Exhaustion occurs on the first
confirmation reaching the allowance. A zero allowance still needs an actual first
false stop. Full-width checked arithmetic preserves costs even at `u64::MAX`.

An incident must name that original crossing. Its disposition distinguishes an
observed open incident from unavailable closure history. A closed disposition
must resolve the exact closure, incident, mode and earlier authorization.
Reopening or conflicting closure records refuse. Missing causal sources keep the
window incomplete and can retract a dependent finding; restoring the exact
source can restore the finding.

The repeat threshold counts qualifying roster-closed incidents, not arbitrary
closure messages. Every contributing incident's budget, original rounds,
assessments and closure chain remain in the support of each threshold finding.
Evidence or policy closure is a permitted control for this particular predicate;
it does not by itself establish the safety of the broader system.

An accepted boundary retains the exact source identities that established an
incident's original first crossing: its budget window, threshold-prefix
confirmations and their held rounds. Withdrawing an earlier confirmation makes
that incident's support incomplete; later confirmations cannot replace the
original opening. The same rule applies before closure and to incidents closed
through evidence or policy. Restoring the exact sources restores evaluation.
Fresh incorrect openings and any currently observed earlier crossing still
refuse, and missing prerequisites cannot hide conflicting or reopened
dispositions.

This private, history-derived provenance is staged with the whole source
boundary and committed only after all predicates and kernel checks succeed.
Replaying the complete accepted source history reconstructs it; a failed boundary
cannot install it. The existing lifetime identity limit bounds it to at most 128
incident bindings, each naming at most 128 original prerequisite addresses.
Withdrawn records remain absent from live evidence and finding supports. This
adds no native source authentication, incident authority or completeness claim.

### Componentwise coverage and active predecessors

Coverage states are occurrence-ordered observations. A change selects exact
previous/current observations for one effect family; later changes must continue
from the last selected current state. Unused state records cannot reset that
chain. Findings retain the earlier transition chain, including benign transitions,
so their predecessor basis remains inspectable.

The mediation enum orders only its registered dimension. Capture channels are
compared by set inclusion only under the same capture-contract identity. A new
contract is incomparable here and preserves incompleteness. A separate proven
mediation decrease can still be reported. Observing authorization does not erase
the fact that coverage fell, and missing authorization does not suppress an
otherwise supported erosion finding.

## Preserve supports and corrections

Derived supports use exact record identities. The query engine computes the
necessary insertions and retractions for the existing checked detector, including
changes that do not cross an alarm threshold. Removing a causal input retracts
the dependent result instead of leaving an unsupported score.

Queries perform no I/O, model inference, effect dispatch, credential operation,
policy promotion or native incident closure. The `NotActorVisible` marker is a
reference type marker; deployment access control must still be enforced by an
independent native integration.

## Finite work and read-only snapshots

Every snapshot retains its exact scope, policy generation and full immutable
configuration through read-only accessors. A caller cannot detach the family
coverage from a different policy or alter an engine-produced snapshot in place.

| Resource | Maximum |
| --- | ---: |
| Lifetime distinct record identities | 128 |
| Changes in one control boundary | 32 |
| Accepted source changes over the engine lifetime | 1,024 |
| Consecutive control boundaries | 1,024 |
| Bytes in each address component or family identifier | 64 |
| Retained derived support tuples | 512 |
| Derived changes in one boundary | 256 |
| Derived changes over the engine lifetime | 4,096 |

The derived-history limit is separate from the input-change limit: a small source
edit may change many supports. Admission checks both before committing the new
snapshot. Every accepted generated history therefore fits the existing batch
verifier's finite prefix limits.

Semantic predicates use bounded batch evaluation. The weighted support relation
is updated through exact deltas. `verify_kernel` independently checks that
relation's arithmetic and retractions; it is not an independent oracle for the
semantic predicates and cannot detect an omission shared by their supplied inputs.

## Verification boundary

The base and extended query targets contain 55 public regressions over all six
predicates, including eight cases for delayed independent roster resolution.
Ten additional Andon regressions cover exact prerequisite withdrawal and
restoration, later-confirmation independence, open and non-roster incidents,
fresh invalid openings, failed-candidate rollback, contradictory dispositions,
accepted-history replay and restored earlier crossings.

Before the Andon repair, the first nine new cases produced five passes and four
failures, reproducing rejected source corrections and replay. With the repair,
all ten passed along with all 26 base-query, 29 extended-query and four Tower
findings tests on the complete `fa-reference` crate using standard LLVM
`nightly-2026-09-08`. The source cut was `60d6d3e1` plus the three Rust files in
the Andon repair, including the concurrent main additions and indexed standing
maintenance. Independent source review and whitespace checks passed.

The local canonical gate previously stopped at `source_snapshot` because the
checked-in source inventory and digests differ from current source/build inputs,
including unrelated existing files. This focused execution does not establish
a full workspace, Clippy, remote RCH or production qualification.

FA-125 remains in progress. Native authenticated complete input feeds, governed
query promotion, durable incident lifecycle, evaluator/governance access control,
independent whole-query differential qualification and the full executed gate
remain outstanding.
