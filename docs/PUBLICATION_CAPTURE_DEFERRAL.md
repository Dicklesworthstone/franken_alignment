# Nonpermitting publication-capture deferral (FA-061 / FA-062)

A complete invalidation feed can legitimately arrive before its producer has
built a snapshot covering the new prefix. The original source-cut contract
correctly rejects that snapshot as publication evidence. A new, explicit native
operation now records that situation without treating an unchanged review or
an existing undispatched reservation as corrupt.

`DeliveryBroker::record_captured_publication_inputs_or_defer` consumes the
original source-bound acquisition cycle. It returns `PublicationCaptureOutcome`
from `delivery::publication_gate::changes`: `Installed` means only that current
inputs may undergo their original one-use comparison; `Deferred` means that no
current inputs were installed and no effect eligibility was created.

Deferral is limited to Reviewing/Authorized attempts, an already cut-bound
source, and a complete locally observed feed. The image must preserve producer
identity, generation/content consistency, monotonic included-prefix coverage and
all structured snapshot floors. Only lag behind selected invalidations may be
deferred. Unknown coverage, future or foreign cuts, generation rollback,
same-generation edits and legacy bindings still refuse. Dispatched effects
cannot acquire a longer execution window through this operation.

A deferred observation retains its exact image and newest generation, updates
its included-prefix and structured high-water marks, consumes its pending read,
and leaves current inputs absent and freshness false. Withdrawal does not erase
those marks. Catching up needs another real read; editing a cut requires a new
producer generation even when all payload bytes are unchanged. The required
invalidation floor never decreases. The original reviewed judgment, control
sequence, reservation, human approvals and effect rights are not modified by
this observation. They remain subject to their original expiry and revocation.

Work is bounded by the existing immutable input types and source slots. Deferred
images occupy the one retained generation image, not another current-input copy.
The operation does not create a polling loop, queue, executor or second ledger.
The producer still asserts actual capture completeness; no authentication or
cross-source atomicity is established. Existing strict capture methods remain
unchanged. This initial increment supplies the native transition; the durable
reader and supervised consumer integration are separate work.

## Verification and change log

2026-09-20 UTC: added the native transition, explicit outcome type, and seven unit
regressions. Tests cover exhaustive prefix neighbors, generation/content
retention, new-read consumption, feed gaps, metadata substitution, withdrawal
floors, legacy refusal and integer boundaries. They are authored, not executed.

Attempted `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference
publication_gate::source::deferred`; this environment returned exit 127 because
`rch` is unavailable. Rust/Cargo/rustfmt are also absent. Whitespace and local
source-hash checks do not establish compilation, formatting or test success.
No production claim, gate update, dependency admission or Bead closure is made.
The 1,485,584-byte Beads export returned empty content through the available
file/blob reads in this session; it was not modified or claimed as reviewed.
