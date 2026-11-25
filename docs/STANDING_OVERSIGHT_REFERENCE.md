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

All candidate facts, product frontiers, support antichains and the closed
sequence commit together only after the independent batch comparison succeeds.
A late oracle error, including support-width exhaustion, preserves the complete
previous engine. A refused candidate can therefore be corrected and submitted at
the same next sequence without retaining any of its rejected facts.

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

## Verification boundary

`crates/fa-reference/tests/standing_oversight_boundaries.rs` contains public
regressions for valid continuation and canonical replacement paired with replay,
gap, identity, invalid snapshot, support-width and late-oracle failures. The
46-fact fanout case reaches 529 support paths and exercises refusal after the
incremental candidate has already been computed.

These are bounded reference semantics, not a native authenticated event feed or
a scalable donor-backed query engine. Source review and syntax/format checks do
not qualify runtime behavior. Required remote-only RCH compilation, execution
and the canonical `xtask check` gate remain pending for this change.
