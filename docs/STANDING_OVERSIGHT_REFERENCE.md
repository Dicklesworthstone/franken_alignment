# Atomic standing oversight at closed boundaries

The bounded standing-oversight reference in
`crates/fa-reference/src/standing_oversight.rs` implements FA-076–080 under plan
§§7.9, 14.9, 15.8 and 22.2. Its consumers are the four registered reference
queries: ungated two-hop sink reachability, unreviewed artifact dependencies,
invalidated held proposals and shared helper failure cohorts. A finding is an
observation with support; it cannot supply an execution permit or change policy.

## One complete boundary per update

`apply_delta_and_close_frontier` accepts exactly the next sequence, starting at
one. An empty completed batch still advances one boundary. Replaying the current
sequence, skipping a sequence or supplying an unclosed boundary refuses before
any retained state changes. A large requested sequence cannot create an implicit
loop that manufactures all intervening frontiers.

The `frontier_closed` and `negative_domain_closed` inputs are declarations by the
reference caller. They are not authenticated journal-closure proofs. Queries
that infer an absent gate or absent review still require the negative domain to
be explicitly closed.

All candidate facts, exact tuple indexes, cached findings, product frontiers and
the closed sequence commit together only after the independent batch comparison
succeeds. A late oracle error, including support-width exhaustion, preserves the
complete previous engine. A refused candidate can therefore be corrected and
submitted at the same next sequence without retaining rejected facts or indexes.

## Exact facts and supports

A stable fact ID cannot silently acquire a different `BaseFactKind`. Retraction
must name the complete existing fact, including its original semantic fields.
To replace a fact, retract the exact old tuple to zero and then insert the new
tuple. Independent support remains active until its own exact retraction.

Publicly constructed snapshots are checked at the batch boundary: no more than
512 facts, map keys equal to their contained fact IDs, and strictly positive
retained multiplicities. The oracle also checks its antichain-width option when
the snapshot is empty. An invalid snapshot cannot become an apparently complete
empty result by filtering malformed records out of the relation.

`SupportPath` fields are public reference inputs, so antichain updates recheck
the nonempty positive support and the 16-atom limits. Width failure preserves the
original weight map, visible paths and precision. A rejected support cannot
reappear after another support is subsequently retracted.

Semantic-generation advancement stages the new generation and support rebuild
together. A failed rebuild retains the original generation and rebuild count;
an unchanged generation is an idempotent observation rather than a new rebuild.

## Indexed maintenance at the final cut

Exact tuple indexes select finding keys affected by each delta before and after
its admission. Each selected key is rebuilt once from the final closed batch;
unaffected findings remain cached. Weighted joins include changes to both inputs,
and fact-ID replacement routes both the withdrawn tuple and its replacement.
Gate and review indexes retain every active identity and multiplicity, so only
the last exact blocker withdrawal exposes an absent-gate or absent-review
finding. Wrapping negative-guard display IDs are not used as index keys.

Support width is checked against the final batch. Coarsening retains the complete
underlying facts, allowing omitted alternatives to reappear after retraction.
Spill uses the independent full batch evaluator. Existing coarsened or spilled
findings retain their precision state across unrelated and empty batches, and
return to exact bounded support when their remaining paths fit.

`affected_findings` counts selected keys, including withdrawn findings.
`incremental_work_units` counts deltas, indexed neighbor/contribution visits and
selected-key refreshes; `batch_work_units` separately counts complete oracle and
spill tuple visits. Atomic candidate cloning and the independent full oracle
still process whole retained state. These counters do not establish sublinear
total update cost. `retained_state_bytes` estimates current fact, index and finding
payloads; it excludes allocator overhead, frontier storage and temporary copies,
and is not an RSS measurement. Empty index buckets are removed after withdrawal.

## Verification boundary

The 11 public tests in `standing_oversight_boundaries.rs` cover exact identity,
canonical replacement, replay/gap refusal, malformed public snapshots, atomic
support-width failure and late-oracle rollback. The 46-fact fanout case reaches
529 support paths and refuses after the incremental candidate has been computed.

Nine tests in `standing_oversight_incremental.rs` add weighted joins, independent
blocker multiplicities, same-ID replacement, final-batch width, idle coarsening
and spill, hidden-support recovery, exact tuple hash collisions, both graph join
directions and bounded index retention. Mixed histories compare all four queries
with the independent batch evaluator after 96 mutation cuts each and a semantic
generation rebuild.

All 20 tests passed with the complete `fa-reference` crate and the standard LLVM
backend on `nightly-2026-09-08`, using frozen base `d3703cfb` plus the roster
resolution and standing-maintenance patches. The local canonical gate stopped at
`source_snapshot`: current files and digests differ from the checked-in inventory,
including pre-existing unrelated files. The full gate and remote RCH
qualification remain open. These are bounded reference semantics; they do not
establish a native authenticated event feed or production scalability.
