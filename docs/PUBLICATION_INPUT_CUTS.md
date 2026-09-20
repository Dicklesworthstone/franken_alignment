# Publication input cuts (FA-061 / FA-062)

A new read of a producer file does not establish that its image includes a newly
received invalidation. Producer generation and read time are different from change
coverage. The optional source binding now records a `PublicationInputCut` naming
the configured change feed and the complete prefix actually included in the image.
It belongs to the original observation basis (plan sections 7.6–7.8), not authority.

## Native contract

The types are exported from `delivery::publication_gate::changes`.
`bind_publication_source_at_cut` requires the original image's cut to equal the
current complete feed prefix when the source is initially bound. Missing records,
an old image, a foreign feed, or a previously bound source refuse. The operation
never recaptures the judgment, manufactures freshness, or grants a permit.

Every selected invalidation raises that source's `required_through` to the observed
feed head. Negative-key/range dependencies, opaque inputs, conservative routing and
gap repair use the existing selection rules. Unrelated changes do not raise the
floor for an unaffected structured-only judgment. No floor is reset by withdrawal.

`record_captured_publication_inputs_at_cut` uses the original one-use read cycle,
checking that the image covers this floor and does not claim changes beyond the
complete local feed. Its cut cannot regress. A changed cut requires a new producer
generation even if the payload is byte-identical. Merely incrementing generation
while repeating the old cut is insufficient. Omitting the cut on an already bound
source refuses; a legacy source cannot be upgraded by supplying later metadata.

All exact structured/opaque, clock, source, committee, human and effect checks still
apply. A suitable cut is necessary, not evidence that changed contents still match
the reviewed judgment. `publication_input_cut` is historical diagnostic data, not a
permit. Failed native admission does not advance the input revision or change
rights. The caller must first withdraw and genuinely reacquire the image.

## Scope and verification

This commit implements the native cut/floor rule; durable versioned packets and
replay consumption follow separately. Existing unbound/legacy profiles retain their
explicit trusted-current-input assumption. No existing packet encoding is changed.
The producer's cut is still an assertion about actual snapshot construction, not
a signature or proof of real-world capture completeness. Sequential reads are not
a distributed transaction. No additional runtime, dependency or authority is added.

Seven unit regressions cover exact prefix neighbors, generation/cut separation,
missing coverage, identity, legacy behavior, floor preservation and integer limits.
These tests are authored but unexecuted. The required remote verifier remains
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` on the exact
integrated revision. No compilation, formatting, Clippy, execution credit or Bead
closure is claimed by source tests alone.
