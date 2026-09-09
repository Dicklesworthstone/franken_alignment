# Whole-input oversight at the effect boundary

## Scope, consumer and verification

`action::consequence::oversight::OversightBroker` composes the existing exact
policy/congress controller and receipt-gated delivery broker with the existing
`ActualHelperInput` and `EvidenceViewManifest` models. Its consumer is the
reference dispatcher: an empirical approval must not outlive the complete input
that its helpers evaluated merely because the exact policy's narrower read set
still matches. Timed congress sessions also give silent helpers a bounded,
explicit missing outcome instead of an indefinitely open review.

Source and regression tests were added on September 9, 2026. Compilation,
rustfmt, Clippy and tests have not been executed: this session had no installed
Rust/RCH runner. Existing operator receipts refer to other frozen source. No
bead, production packet or dependency-admission status is promoted here.

This is std-only reference code with in-memory state and trusted supplied clock,
provider-capture, profile and transform declarations. It runs no inference,
network transport, process sandbox, authenticated capture or durable journal.
It does not prove that a provider disclosed every hidden input or that a helper
actually read its packet or voted honestly. The admitted production runtime,
cryptographic/capture contracts and independent execution campaign remain
prerequisites, not replaced by these types.

## Inputs that can support an empirical review

`CommitteeContract` freezes the exact helper roster, per-helper model/tokenizer
profile, projection identity and question. The roster must exactly match the
controller's congress. The profile's policy epoch is instantiated from the
frozen action; its model, tokenizer, profile and input-contract identities are
not silently rotated. A helper/model-contract change requires a separately
specified transition; this facade does not implement helper rotation.

`CommitteeInput::capture` retains a full immutable manifest for every member.
The existing full-input validator verifies the byte partition and question, and
the view manifest verifies declared source/projection bindings. The new admission
checks tenant, projection, helper profile and exact configured question. This
initial complete-view profile rejects declared truncated windows, redaction and
Gapped/Unsupported/Redacted omissions. An explicit ClosedAbsent omission is a
trusted declaration, not an inferred absence or provider-authentication proof.

Each actual input must contain `action_frame(action)` as the Other/ordinal-zero
part. That versioned frame binds the execution-bearing fields and exact payload
with fixed integer fields and explicit lengths. It deliberately does not copy
private policy read witnesses into helper inputs. Additional authorized evidence
can be supplied as other input parts. No rendering or summarization is silently
substituted for the exact frame; payloads that cannot fit the existing 64-KiB
whole-input limit refuse instead of being truncated. This is an opaque reference
input contract, not a real model's tokenizer/serving ABI or a cryptographic hash.

Whole-input equality includes every submitted byte and its part/omission layout,
source generation, transformation, redaction/window declarations, projection,
model/tokenizer epochs and input contract. Transformation/capture truth and
qualification remain trusted assumptions. The facade records these identities
and invalidates reuse when they change; it does not execute or independently
qualify a transform.

## Live path and invalidation

```text
propose -> exact policy derives action and read witnesses
record_inputs -> all helper manifests checked against that frozen action
begin_review -> freeze complete views, revision, policy, round and time window
commit -> reveal -> finish -> immutable ObservedReview
apply_review -> verify issuer, view revision, current input and exact policy
  -> authorize with freshly supplied current views
  -> dispatch with freshly supplied current views
  -> original keyed publication and receipt reconciliation
```

`record_inputs` uses an expected revision. Re-recording exactly equal complete
inputs is idempotent. Any admitted change increments the revision and clears
approval. `inputs_unavailable` clears the current basis and approval without
refunding reservations. Restoring identical bytes after that recorded outage
still requires a fresh independent review. A stale capture update cannot replace
a newer revision.

At apply, authorize and dispatch, positive operation requires explicitly supplied
current views matching the retained complete basis. The caller must obtain these
from the actual trusted capture boundary; there is no background watcher that
can discover undisclosed out-of-band changes. A failed/oversized capture is not
implicitly an accepted freshness update. The provider must report the outage or
supply no current view; a changed current view cannot pass the positive checks.

The facade owns its delivery broker. It exposes no mutable inner controller,
raw policy-review application, raw trusted-outcome input, or inner dispatch
conversion. A review from another facade instance cannot apply even when its
numeric identities, action and view bytes match. Existing low-level reference
APIs still exist for their declared trusted model uses; they cannot mutate the
private broker owned by this facade.

An issued permit's reservation remains held across a view change. A new review
of the new view can release the same unchanged action into normal one-use
permit dispatch; no extra rights are minted. Restrictive reviews instead retain
their historical pre-vote inputs and remain applicable during a current-view
outage, subject to the underlying policy/sequence checks. Post-dispatch receipts
and nonexecution sealing/reconciliation never require current helper inputs.
Unknown effects cannot be cancelled or refunded merely because monitoring failed.

Policy replacement and actor reset clear empirical approvals and preserve the
underlying epoch, cancellation and external-liability laws. New work captures
views for its new frozen action/epoch and obtains a new review. Dispatcher
restart preserves the original receipt/fence protocol; a fence acknowledgment
is not a substitute for current helper inputs.

## Commit/reveal deadlines

`ReviewWindow` freezes commit and reveal cutoffs before any vote. Creation
requires now < commit_by < reveal_by <= the action's deadline. Every accepted
commit, reveal and finish supplies an explicit monotone tick in the controller's
declared logical clock domain. A rejected operation never rewinds that clock.
The implementation does not establish physical clock synchronization.

Reveal can open before the commit cutoff only once every member has committed.
Otherwise it can open at the cutoff with the missing members retained. Commits
at or after commit_by refuse; reveals at or after reveal_by refuse. A malformed
or conflicting reveal is not counted as completion. Early finish succeeds only
when every member has a valid reveal, including explicit abstentions. Otherwise
finish waits until reveal_by and returns the ordinary reducer's missing-member
outcome. A completely silent round can close without inventing any vote.

Premature finish borrows and preserves the session. Successful finish consumes
its internal policy session exactly once. Applying a completed review cannot
move the controller clock forward: the controller must have independently
observed at least its completion tick. A timeout normally yields Hold, but an
existing exact disqualifier or stronger empirical restriction still dominates;
timeout does not weaken a Deny or turn missing members into harmless abstentions.

## Resource and evidence boundaries

Each committee is limited to 128 members and 1 MiB of counted variable data,
including the manifest's stored submitted-byte copies and local action witness
values. Existing per-input/part/label limits still apply. The facade admits at
most 8 MiB of cumulative captured variable bytes and starts at most 512 rounds.
Prior bases may remain shared by sessions/receipts, so replacing one does not
refund this conservative lifetime admission count. These are logical bounds,
not measurements of allocations or total process memory. External callers can
copy returned evidence independently; such copies confer no authority.

Round identities are reserved when started, including sessions later discarded;
they cannot be reused to solicit a replacement vote under changed context.
Exhaustion refuses new investigation work but does not block cancellation,
fencing or reconciliation of existing obligations. A production long-running
service needs governed retention/rotation, not silent reuse of these finite
reference identities or a reset of unknown liabilities.

`ObservedReceipt` retains the actual per-member views/revision and fixed time
window alongside the original PolicyReceipt. It is an in-memory evidence object,
not a permit or independently authenticated receipt. The existing DecisionArchive
wire profile covers exact policy/congress replay, NOT this added whole-input and
deadline contract. This batch does not claim that exporting that older archive
also exports or verifies the new oversight basis.

## Source tests and plan relationship

Three packet unit tests and twenty-one public integration tests cover positive
publication, changed context with unchanged exact-policy reads, metadata-only
changes, outage/recapture, delayed receipts, foreign reviews/models, declared
missing input, question/projection/roster binding, clock rollback, partial and
silent rounds, late commits/reveals, malformed reveals, premature finish,
policy rotation, actor reset and bounded-round cancellation. They are unexecuted
source. The suites are `observed_oversight.rs` and `oversight_boundaries.rs`.

This serves the plan's whole-input witness and committed-view requirements,
section 8.8 and sections 9.1-9.4: opaque judgments depend on whole actual input,
independent reviews freeze their context, and currentness is rechecked at the
effect boundary. It composes the existing FA-057/FA-081 reference input/view work
with the FA-019/FA-104 congress/control path. It does not close those full
production packets. Founding roots are FI-A03, FI-A06, FI-A13, FI-A14 and FI-A16:
bounded helper interfaces, evidence-backed external control, independent initial
judgment and one-directional authority.
