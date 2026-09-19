# Final-cut publication validation (FA-062 reference profiles)

## Consumer and status

The existing `DeliveryBroker` consumes this gate immediately before authorizing
an attempt and again in its common dispatch path, before the controller consumes
the effect permit. `OversightBroker` exposes the same configuration and input
operations without exposing its inner delivery owner. Ordinary and human-key
dispatch share the check. The policy, committee, human, deadline, revocation,
mediation, fleet and endpoint checks remain independent requirements.

The Unix `FileOversight` profile now journals these requirements and observations
and consumes the SAME witness check again at first publication, after dispatch.
It enables the existing publication guard with the witness gate, so raw publish
cannot bypass that final check. There is one canonical journal and one original
rights ledger, not a second persistence or authorization system.

These are bounded reference capabilities, not activation of a production
feature. Source and regression tests are implemented; Rust compilation,
formatting, Clippy and execution have NOT been performed in this editing
environment, which has neither the Rust toolchain nor RCH. FA-062 is not closed.
The verifier must run `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask
-- check` on the exact integrated revision. No previous execution evidence is
claimed for this code.

## In-memory host integration

Before the first proposal, call `enable_publication_validation(PublicationLimits)`
on the delivery or oversight broker. The configured profile applies to every
subsequent proposal and cannot be disabled or widened. The lifetime binding
capacity is at most `MAX_PUBLICATION_BINDINGS` (16); rejection occurs before a
new proposal is inserted, and cancellation does not reopen that capacity.

After `propose`, use its **controller-produced frozen action**, including the
policy-derived dependencies, to bind a `PublicationJudgment`. The binding holds
one structured `WitnessJudgment`, one opaque `OpaqueJudgment`, or both. Binding
requires at least one lane and must happen while the attempt is Reviewing,
before authorization. It is write-once: changing the judgment, dropping a lane,
or rebinding the action requires a new reviewed attempt, not editing this one.

Call `record_publication_inputs(attempt, expected_revision, Some(inputs))` with
actual current host observations before authorization and publication. Call it
with `None` when those observations are unavailable. This changes observations,
not reviewed requirements. The exact predecessor prevents stale writers from
overwriting a newer observation. Structured revision, control-cut and semantic
epoch high-water marks survive unavailability and actor reset.

## Durable host integration and recovery

`FileOversight::create_with_publication_validation(directory, profile, limits)`
puts the required gate into the first canonical image, before returning an
owner. Existing owners can enable it before any proposal using
`enable_publication_validation(journal_revision, limits)`. For stream profiles,
perform the existing stream bootstrap before enabling this additional gate.
There is no disable or post-review upgrade operation.

The types in `observed::publication::witnesses` retain replayable observations:
`FileWitnessInput` records the complete bounded snapshot and the independently
admitted closing marker for its projection; `FilePublicationInputs` adds an
optional whole opaque helper input. `FilePublicationEvidence::new` freezes the
ORIGINAL inputs plus ordered witness requests. Its versioned canonical packet
round-trip reconstructs the existing validated types. It stores neither a permit
nor an asserted successful validation report. Missing admitted closure remains
missing even when the snapshot itself contains a Closed assertion.

After proposing, call `bind_publication_evidence(journal_revision, attempt,
evidence)`. The machine derives the action binding from its original proposal,
not an action supplied in the serialized packet. Record current observations
with `record_publication_inputs(journal_revision, attempt, input_revision,
Some(inputs))`, or pass `None` for unavailability. The initial input revision is
zero. Binding and current observations are separate journal events: updating an
observation cannot recapture the original judgment at a more convenient cut.

A stale journal/input predecessor is rejected without mutation. After a current
observation update begins, failure during capacity checks, encoding, replay,
allocation or canonical replacement makes the live owner unavailable. Its old
acknowledged snapshot remains historical data, not an eligible quiet fallback.
Recovery must reopen the canonical prefix through the original fenced path.
This rule applies to positive replacements as well as explicit capture loss;
a malformed/regressing current replacement can also require recovery.

`open_with_publication_validation(directory, profile, expected_limits)` pins
exactly one matching gate configuration before replay, cleanup or a recovery
write. Ordinary `open` also replays the stored gate; omitting the explicit pin
does not disable it. Recovery retains original evidence, observation revisions,
high-water marks and capacity consumption, but cancels undispatched attempts,
withdraws old keys and discards sendable envelopes. New attempts start without a
publication binding or current observation. Historical observations therefore
cannot authorize fresh work by being recovered.

`retained_publication_evidence` exposes original observation data, not a key.
`publication_validation_profile`, `publication_input_revision` and
`publication_validation` refuse on an unavailable owner. The last report is the
last ACKNOWLEDGED comparison, not a claim that an unsuccessful authorization was
journaled or that the currently recorded inputs still match it.

## Final-cut semantics and failure behavior

The broker recomputes validation using the immutable retained action and latest
recorded inputs. It never accepts an external `PublicationReport` as permission.
Structured validation uses the bounded FA-060 exact refinement engine:
value/version dependencies, absent keys, empty ranges, complete range membership,
source/domain semantics and closing frontiers are rechecked. Unrelated updates
at a newer cut can still pass.

Opaque equality includes the entire submitted byte stream, input profile,
tokenizer/model/policy epochs, ordered parts and omissions. An explanation such
as “only these bytes mattered” does not narrow it. Both lanes share one fixed
step/byte validation budget. Exhaustion is Incomplete, not validity; there is no
unbudgeted validation fallback or carry-over of partial results across cuts.

A refused authorization/dispatch returns no sendable envelope and does not spend
the effect or human key or refund a reservation. Evidence can be refreshed and
the original unspent keys retried while all their other conditions still hold.
After dispatch, `publish_checked` (including the credentialed path) rechecks the
witnesses before first execution. Missing or invalid evidence seals that endpoint
request as unexecuted; later better evidence cannot reroll it into execution.
Sealing itself does not refund: original receipt-based reconciliation settles it.

Previously resolved outcomes and execution deadlines take priority over new
witness checks. Evidence loss, human expiry and restart cannot turn an executed
publication into nonexecution or refund it. Existing unresolved liabilities stay
charged until the original endpoint protocol resolves them.

## Bounds and explicit nonclaims

Packets are at most 3 MiB, with the native limits of 256 snapshot entries, 8 KiB
per value, 64 witness requests and bounded whole-helper inputs. The durable
profile reconstructs at most one admitted authenticated prefix of 4,096
positions per packet. Larger prefixes refuse with Limit; this is a replay-work
bound, not a claim that larger live frontiers are invalid. Nonterminal prefixes,
other stages and unrelated projections are NOT exported as a reusable full
frontier history. Their omission cannot manufacture negative evidence for the
witness consumer. Packet decoding/capture and bounded prefix reconstruction are
additional work, not included in the validation comparison budget. The enclosing
journal's event/byte limits bound total retained history.

Journal event tag 30 carries gate configuration, original binding or current
observation. Existing event tags/bytes are unchanged. Readers predating this tag
refuse it rather than loading a weaker profile. Exact configuration pins are not
cryptographic anti-rollback protection for the journal or storage medium.

Current inputs remain trusted host assertions. Neither profile captures a live
provider request, authenticates an adapter, proves every remote change was
recorded, or establishes wall-clock freshness. Existing live policy-state and
whole-committee gates are not replaced. The packet prefix profile is not a
production stream adapter, and authored tests are not crash or alignment proofs.

## Regression coverage

`oversight/publication/tests.rs` and `tests/publication_delivery.rs` cover the
original validator and in-memory review/permit/endpoint path.
`observed/publication/witnesses/tests.rs` adds packet round-trips, all witness
kinds, opaque metadata/omissions, missing closures, truncations and limit neighbors.
`tests/file_publication_witnesses.rs` adds durable successful publication/replay,
post-dispatch negative evidence, recoverable pre-dispatch unavailability, closure
loss, opaque drift, pinned and generic reopen, old-key withdrawal, failed-update
quarantine, immutable original bindings and executed-outcome reconciliation.
All new tests await execution by the verifier.
