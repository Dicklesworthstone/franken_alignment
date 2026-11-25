# Risk-theater queries over declared control history

The FA-125 query projection derives oversight findings from typed control-history
records. Its first three query families cover helper retirement after confirmed
holds, autonomy promotion outside its dwell/report contract, and scheduled
audits whose declared funding is insufficient. The consumer is the existing
bounded score/support detector; callers supply facts instead of family scores.

The implementation belongs to L8 Governance and serves FI-A18 and FI-A13 through
the existing FS-08 concordance. It implements a bounded reference contract for
plan sections 15.8 and 15.10. It is not a native journal adapter, an authentication
service, or a policy writer. The live Tower still reports all six full query
families Unknown until their native input contracts are installed and verified.

## Bind rules and history before evaluating

Construct an immutable `QueryPolicy` with the exact run scope, query generation,
independent evaluation-owner address, retirement window and minimum autonomy
dwell. A snapshot retains that complete policy; a display generation alone
cannot hide a change to its rules.

A closed frontier asserts completeness of the fixed roster, autonomy and audit
source domains for this reference profile. An empty declared domain is therefore
a complete empty result for those predicates. This declaration cannot stand in
for a missing or uninstalled native source; the other query families remain
incomplete until their own comparison contracts are selected.

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

Public regression sources accompany the implementation and pair findings with
near-identical permitted histories. The new source has not been formatted,
compiled or executed in this session: Rust and RCH tooling were unavailable,
and the execution workspace subsequently disconnected. GitHub source review
and committed-content verification do not substitute for the required RCH gate.

FA-125 remains in progress. Native authenticated complete input feeds, governed
query promotion, durable incident lifecycle, evaluator/governance access control,
independent whole-query differential qualification and the full executed gate
remain outstanding.
