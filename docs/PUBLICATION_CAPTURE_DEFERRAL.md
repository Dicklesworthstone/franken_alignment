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
unchanged. The durable reader consumes this same transition; automatic supervised retry
integration is separate work. Existing strict driver/completion routes are unchanged.

## Durable acquisition and replay

`FileOversight::refresh_publication_from_file_or_defer` uses the same concrete
`PublicationInputFile` and the original locked canonical journal. It preflights
stage, revision and the existing cut binding, durably withdraws eligibility, then
reads the actual producer file. A successful result contains an acknowledged
`FileCaptureObservation { identity, outcome }`, not a key or validation receipt.

Only a validated lag behind a complete feed is a deferral. Read/decoding errors
remain inner errors with unavailable inputs. Invalid installations, event/byte
capacity failures and storage errors after a successful read remain outer errors;
the owner is poisoned before encoding/replay/storage, just as in the strict path.
A failed acknowledgment cannot return a candidate generation, refund or outcome.
Recovery reads the actual canonical image and fences old keys. A committed
producer generation and snapshot floor survive recovery but never restore a live
capture or an undispatched approval. Unknown outcomes cannot cause effect replay.

The explicit observation uses subtag 9 of the original publication-witness event
30. The immutable packet is retained, and original semantic replay recomputes
Installed versus Deferred. No serialized success bit is trusted. Existing source
bindings and strict event subtag 4 remain byte-identical; old readers reject the
new tag. Reclassifying a lagging event as strict does not make its replay legal.
The original journal limits, work classification and recovery reserve remain in
force. No second storage sink, event importer or authority ledger is introduced.

`publication_input_cut.last` now describes the latest admitted image, including
a deferred one. It is historical data: neither it nor the acknowledged outcome
is fresh permission. Once the source catches up, its inputs must still satisfy
the original exact value, absence, range and opaque checks. The reviewed binding
is never recaptured from that newer image.

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

2026-09-20 UTC, durable increment: added seven real-file integration regressions
and four deterministic storage/replay regressions. Coverage includes retained
original human/automatic keys, successful caught-up publication, exact negative
witness checks, strict-route rollback refusal, restart floors, feed-gap repair,
read failure, rejected late/legacy calls, all five withdrawal and observation
barriers, exact event-capacity neighbors and semantic downgrade refusal. Fixture
helper votes are not native inference evidence; injected barriers are not hardware
power-cut evidence. The required RCH integration-test command again exited 127
because `rch` is absent. Compilation, Clippy, formatting and runtime tests remain
unexecuted; lexical/whitespace checks are only source screens.
