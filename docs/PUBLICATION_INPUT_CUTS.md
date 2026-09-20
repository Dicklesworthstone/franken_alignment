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

## Durable packets and existing execution paths

`FilePublicationCapture::new_at_cut` emits `FAPCAP02`, adding the feed source and
covered sequence to the immutable image. `new` still emits byte-identical legacy
`FAPCAP01`. The existing `PublicationInputFile` reads both, with the same size and
regular-file checks. Consumers must not stamp an old image with the latest feed
head: the producer is asserting the prefix actually included during construction.

Bind the version-two ORIGINAL packet through `bind_publication_file_source`. The
journal stores it as subtag 8 of existing publication event 30, preserving old
source-binding subtag 3. Current observations keep existing event subtag 4 and
carry the packet's explicit version. Decoding never infers missing cut metadata.
Older readers reject version-two packets/new binding tags rather than stripping
requirements. Generic reopening reconstructs the binding and invalidation floors.
Recovery still withdraws keys and sendable envelopes; it does not make a saved
capture fresh or erase an unresolved effect charge.

The original machine selects the bound native operation for both initial binding
and subsequent capture. Ordinary refreshes, supervised steps and all existing
atomic completion variants therefore consume the SAME rule. Feed catch-up inside
a completion raises the candidate floor before its witness read, including on the
second staged feed acquisition. No new driver, completion API, event importer or
permitting fallback was added. Once initially bound with a cut, a version-one
reread, changed feed identity, old cut or same-generation metadata edit cannot
satisfy the requirement. Initial legacy bindings cannot be upgraded by later data.

A lagging cut fails capture installation under the existing durable policy: after
withdrawal, the owner is quarantined until fenced recovery. It is not silently
retried or acknowledged as successful acquisition. This is deliberately stricter
than buffering a lagging producer while retaining an authorized live attempt;
there is no new recoverable-lag transition in this reference profile. Successful
read identities are still not installation receipts. Atomic completion exposes no
committed feed reports on failure and leaves either the withdrawn original cut
or the whole acknowledged effect/receipt cut, never partial permitting metadata.

## Scope and verification

Existing unbound/legacy profiles retain their explicit trusted-current-input
assumption. Selecting an old profile is not claimed to enforce this stronger
cross-source relation; bind a version-two original packet for each required source.
The producer's cut is still an assertion about actual snapshot construction, not
a signature or proof of real-world capture completeness. Sequential reads are not
a distributed transaction. No additional runtime, dependency or authority is added.

Seven unit regressions cover exact prefix neighbors, generation/cut separation,
missing coverage, identity, legacy behavior, floor preservation and integer limits.
Ten additional file/atomic-completion/helper-socket regressions cover original
binding atomicity, version compatibility, stale rereads and newer-but-lagged images,
selective versus opaque invalidation, gap repair, exact content revalidation,
metadata downgrade/equivocation, recovery and staged post-dispatch changes.
All tests are authored but unexecuted. The required remote verifier remains
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` on the exact
integrated revision. No compilation, formatting, Clippy, execution credit or Bead
closure is claimed by source tests alone.
