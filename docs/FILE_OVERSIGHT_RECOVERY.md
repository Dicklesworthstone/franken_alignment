# Durable full-input oversight and mandatory two-key publication

## Implemented reference consumer

`delivery::persistent::observed::FileOversight` connects the ORIGINAL
`OversightBroker` to the existing Unix canonical-publication storage owner.
Previously the file-backed host replayed `DeliveryBroker` and supplied reference
ballots without retaining the full observed-input and human-key state machines.
The new profile makes full-input congress review AND a separately held human key
mandatory on the same durable publication path. It does not replace any original
policy, congress, human-key, endpoint or resource-accounting reducer.

This is a reference integration in the existing L3/L4/L5 boundaries, supporting
plan sections 8.3, 8.4, 8.11, 9.11 and 16.4. It is not a new actor-facing verb,
independent journal service, production assurance profile, or a claim that every
optional OversightBroker subprofile can now recover.

## One effect sink and one ordered history

The new `FileOversightProfile` binds the complete existing `FileDeliveryProfile`,
effective committee contracts and mandatory human-review policy. It uses the
same locked `Store`, staged write, file sync, canonical rename and directory sync
as `FileDelivery`. The journal is domain-separated from the one-key profile; a
file from either profile cannot be opened as the other. No import can downgrade
an observed history to reference-ballot or one-key dispatch events.

The original bounded encoding primitives, action/snapshot event encoding,
bootstrap binding and storage barriers are shared. Helper contracts preserve
model/tokenizer/profile identity, profile bytes, projection ID and exact question.
Only the helper contract's unused bootstrap policy epoch is normalized, because
the original `profile_at(action_epoch)` substitutes it at every use. Actual
observation policy epochs are retained exactly.

Every mutation first bounds and encodes its inputs, replays the original broker
and memory-only endpoint into a private candidate, applies the original operation,
and replaces the canonical file before returning any result. The candidate has
no filesystem endpoint, subprocess launch, network transport or external effect.
The journal replacement is the only publication. No caller-supplied decision,
rights counter, refund, endpoint outcome or saved approval receipt is accepted
as authoritative replay input.

The original ceilings remain 4,096 events and 16 MiB, with smaller configured
limits permitted. Original helper-input, round, human-request, attempt and receipt
retention bounds remain enforced. History replay/rewrite is bounded but quadratic
across a sequence of appends. A full journal can block stop/drain/recovery; no
unaccounted emergency capacity or implicit new authority domain is invented.

## Real full-input reviews, not reconstructed summaries

`record_inputs` retains each original `EvidenceViewManifest`: exact submitted
bytes, ordered parts, model/tokenizer/profile epochs, omission metadata, selected
original identities, transforms, redactions and source windows. Decoding calls
the ORIGINAL `ActualHelperInput`, `EvidenceViewManifest` and `CommitteeInput`
constructors. Their completeness, roster, action-frame and question checks still
apply. Provenance declarations do not become authenticated provider observations.

`begin_review`, `commit_review`, `open_reveals`, `reveal_review` and `finish_review`
consume the original `ObservedSession` protocol with explicitly admitted logical
time. Member identity and the supplied digest are trusted worker-channel inputs,
not authenticated network messages. The existing reference commitment algorithm
is unchanged. Missing helpers remain in the original denominator; a deadline is
not extended and an empty response is not an Allow vote.

`finish_review` returns `Result<Result<ObservedReceipt, Error>, JournalError>`.
An outer error means there is no acknowledged journal result. An inner error means
the original completed review was consumed and its application refusal was
COMMITTED. For example, an Allow review against stale current input burns its
round rather than letting the caller repair the evidence and reuse the completed
judgment. A different round with genuinely new input can still be reviewed.
Restrictive original decisions retain the native `current=None` behavior.

Authorization, human approval requests and dispatch require the caller to supply
the exact current `CommitteeInput`. The host compares it with the original
broker's retained input and records its revision instead of maintaining a shadow
input ledger or copying identical views into every event. This comparison does
not prove a provider was freshly sampled: capture freshness remains the host's
responsibility. `inputs_unavailable` durably invalidates prior approval without
requiring a new observation.

## Separate reviewer custody and no one-key fallback

Trusted `create` and `open` return `(FileOversight, FileHumanReviewer)`. The role is
not cloneable or obtainable from the owner, a request, a status or a journal.
The embedding host must provision it to its separately authenticated reviewer.
This is process-local custody separation, not human authentication or a digital
signature. Code with bootstrap access and arbitrary access to protected storage
is inside this reference profile's trusted boundary.

A native human request exposes the exact reviewed action and complete input basis.
`FileHumanReviewer::approve` commits the ORIGINAL approval transition before
returning a noncloneable `FileHumanPermit`. Dispatch requires that key AND the
original automatic permit, exact action and current full input. The native broker
rechecks policy/control/input generations, expiry, revocation and one-use state.
The endpoint independently enforces the shorter of action and human-key deadlines
at first publication. No method supplies a one-key dispatch alternative.

Reviewer rejection/revocation withdraws only the human key. It does not refund an
automatic reservation or claim to stop an already-dispatched effect. Original
cancel/terminal-stop/endpoint-reconciliation operations own those transitions.
A historical clock value may be used only for withdrawal, never approval or
extension of execution authority.

## Recovery, stopping and uncertain effects

Opening replays the history, then durably revokes all pending/approved human keys,
revokes the authority epoch, cancels only undispatched work, installs and confirms
an endpoint fence, and discards original automatic keys, human keys, live helper
sessions and sendable envelopes BEFORE returning an owner or a fresh reviewer
role. The new owner has a new process-local brand. Old roles and keys cannot be
used with it, even when numeric reviewer/request/attempt IDs match.

Saved time is historical; a fresh explicit clock observation is required before
new review, approval, dispatch or ordinary reconciliation. Old completed and
interrupted round IDs remain used. Recovery may inspect the original immutable
human-request evidence, but it cannot turn an Approved status into a new key.

Already-dispatched effects remain Unknown and charged until the original endpoint
resolves them. Execution receipts win without reexecution. Missing live requests
remain unresolved; only a terminal original nonexecution receipt releases their
charges. Retention expiry stays unresolved rather than becoming a refund.

Terminal stop and endpoint drain reuse the original stop operations. Local stop
also withdraws outstanding human keys and private sendable messages. It survives
reopening and cannot be undone by fresh time or a recovery fence. Drain settles
missing requests through original endpoint seals, retains executed charges and
reports every unresolved obligation. A drained domain need not have zero charges.

A failed canonical replacement returns no candidate automatic/human key, review
result, dispatch result, receipt or refund. The owner becomes unavailable, and its
inspection/status methods describe only the last acknowledged cut, which may lag
disk after rename. Exclusive reopening uses the actual canonical file, never an
inert staging file, to recover. Stop-progress reads explicitly refuse a failed
owner rather than presenting stale progress as current.

## Source scenarios and execution status

Eight public behavior tests cover successful full-input/two-key publication,
cross-owner role/key refusal, changed input, missing-helper phase deadlines,
committed stale-review refusal with a permitted new round, endpoint enforcement
of human-key expiry, revocation without automatic refund, and conflicting versus
matching native reveals.

Six public recovery tests cover approved/dispatched/published cuts, interrupted
helper sessions with permitted new-epoch work, mixed terminal stop/drain, profile
binding and downgrade refusal, a real staging-file collision, and retention-expired
liabilities. Their fixtures use the original two-member congress and full evidence
manifests, not mocked approval or accounting.

Three internal fault campaigns exercise approval, dispatch and publication at all
five original storage barriers. They distinguish pre-rename from post-rename cuts,
including visible unacknowledged approvals, consumed human keys, uncertain charges
and execution receipts. A fourth internal test covers truncated framing, forbidden
one-key events and framed-but-illegal history. Four compile-fail examples cover
owner escape hatches and copying the reviewer role or human key.

The required command was attempted during this change:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_oversight
```

It failed with `rch: command not found`. The new Rust implementation, public and
internal tests, compile-fail examples, formatting and Clippy have NOT been compiled
or executed in this editing session. No local build fallback was used. Test source
is not passing evidence, and no Beads task or production qualification is closed.
A verifier must run the revision-bound gate; focused targets also include:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_oversight --test file_oversight_recovery
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib action::consequence::delivery::persistent::observed::tests
```

Existing source reducers, assertions, dependencies and storage behavior are
unchanged. The shared broker addition is a read-only retained-input accessor.
All base-profile limitations still apply: trusted plaintext operator-owned
storage, cooperative kernel locking, no hostile-namespace guarantee, no independent
same-path anti-rollback anchor or authenticated journal, and operator-supplied
clock continuity. This profile does not yet persist decoder/model execution,
learned-monitor state, identity admission, policy campaigns, fleet/stream topology,
or the actor mailbox and live worker transports. Those are not silently restored
as optional gates from an incomplete history.
