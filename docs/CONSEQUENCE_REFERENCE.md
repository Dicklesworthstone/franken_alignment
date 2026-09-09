# Congress, consequences and checkpoint containment

## Implementation status and verification

Source implemented on September 9, 2026 in the existing std-only `fa-reference` crate. It connects round, empirical reducer, action authority and supplied actor-state checkpoint models. It is not a production congress, inference host, effect broker, durable decision closure or admitted foundation integration.

**Verification is pending for both implementation batches.** No configured RCH runner was available in these sessions. Formatting, compilation, Clippy, unit tests, integration tests and the complete repository gate were not executed. Existing execution receipts in `IMPLEMENTATION_STATUS.md` describe their own frozen source, not these additions. No bead is closed and no executed-reference or production-admission status is promoted here.

## Feature change log

| Batch | Source progress | Verification status |
|---|---|---|
| Initial congress integration, through `3e5e6828` | Consequence lattice, capped congress evaluation, hold/deny/narrow/suspend enforcement and post-dispatch containment | Source and regression tests only; execution pending |
| Containment and bound sessions, starting at `c47f6d0` | Supplied actor-state restoration, monotone revocation, abandoned-continuation cancellation, repeated-incident suspension, immutable action/policy/issuer-bound congress sessions | Source and regression tests only; execution pending |

The earlier `action.rs` implementation remains in place. No dependency, runtime, inference backend or cryptographic library was added.

## Implemented paths

```text
ReviewSession captures the actual ledger action, issuer and predecessor
  + frozen controller-supplied policy, evidence root and exact facts
  -> session/member-bound commitments and frozen-roster reveals
  -> capped empirical tally, explicit missing/abstaining members
  -> immutable BoundReview
  -> original ConsequenceAuthority only
  -> existing authorization, dispatch and outcome reconciliation

ContainmentAuthority owns the same gate and a supplied full-prefix ActorState
  -> bounded, issuer-bound checkpoint of tokens/cache/sampler/position
  -> trusted reset request at the expected actor/control revisions
  -> cancel undispatched work + advance revocation + increment incident counter
  -> intersect retained target authority
  -> restore actor state OR suspend at the fixed incident threshold
```

The public source modules are:

- [`action::consequence`](../crates/fa-reference/src/action/consequence.rs): five-element restriction lattice, six primary consequence tags, strict versioned reference encoding and deterministic rule composition.
- [`action::consequence::congress`](../crates/fa-reference/src/action/consequence/congress.rs): frozen-round evaluation using the existing proportional member/cohort-capped reducer and an explicitly supplied policy.
- [`action::consequence::gate`](../crates/fa-reference/src/action/consequence/gate.rs): consequence-aware ownership of the existing action authority and rights ledger.
- [`gate::containment`](../crates/fa-reference/src/action/consequence/gate/containment.rs): supplied actor-state checkpoint storage, reset and incident escalation.
- [`gate::containment::session`](../crates/fa-reference/src/action/consequence/gate/containment/session.rs): immutable congress sessions usable with either the standalone gate or the containment controller.

## Consequence contracts

| Operation | Reference behavior |
|---|---|
| Continue | Opens the normal authorization path; does not mint a permit or reserve units itself. Dispatch still rechecks issuer, exact action, witnesses, scope, epoch, deadline and one-use state. |
| HoldEffect | Blocks authorization or dispatch without refunding an existing reservation. A fresh round at the current predecessor is needed to release the hold. |
| Deny | Terminally denies an undispatched attempt and refunds its existing reservation once. Later approval cannot resurrect it. |
| NarrowAuthority | Intersects exact resolved-target permissions, including adapter, object, contract version, resource version and generation. Cancels outside-ceiling undispatched work; cannot widen permissions. |
| SuspendRun | Closes further admission and dispatch. Dispatched and unknown effects retain their real liabilities and remain reconcilable. |
| Post-dispatch containment | A finding about an already-dispatched or completed attempt can narrow or suspend the run without rewriting that effect's historical disposition. |
| ResetToCheckpoint | The containment controller restores the selected checkpoint's supplied actor state under narrowed authority, cancels all undispatched work from the abandoned continuation, advances revocation and retains world/effect history. The reducer does not synthesize reset from an empirical vote. |

An exact disqualifier establishes at least Deny despite empirical approval. A stronger run consequence does not erase an undispatched attempt's exact denial. Missing evidence and unresolved contradiction impose at least Hold without weakening a stronger restriction.

The congress profile requires substantive answers from every frozen member. Missing reveals and abstentions remain different facts, neither silently dropped to make approval pass. A helper's Deny is empirical evidence, not a trusted exact disqualifier. Quorum accounting uses positive influence after clipping. Optional-specialist policies remain outside this profile.

## Bound congress sessions

`ReviewSession::begin` reads the action and control predecessor from the actual owning gate. There is no action argument that can be replaced after the votes arrive. Membership, influence policy, evidence root, exact-disqualifier/contradiction facts and the narrowing target set are retained before commitment.

A `BoundCommitment` has a private session/member brand. Even identical raw oracle digests cannot be transplanted between sessions or members. `finish` consumes the mutable round and produces a non-cloneable `BoundReview`; there is no mutable raw-request conversion. Application checks the original authority issuer as well as the gate's predecessor and action checks. A reset invalidates earlier reviews through the shared control sequence and cancellation of the abandoned attempts.

These are process-local reference guarantees, not cryptographic binding. The underlying FNV comparison oracle remains non-cryptographic. Existing raw `Round`, `DecisionInputs` and `ReviewRequest` APIs remain available for trusted logical-model inputs; the new session path does not authenticate arbitrary users or remove those low-level APIs. Controller-supplied policy/fact provenance remains an assumption.

## Checkpoint containment

`ActorState` retains exact supplied token, cache and sampler bytes and a next-position value. This full-prefix profile requires next-position to equal token count and requires nonempty cache/sampler state. Tokens, cache and sampler are bounded individually; checkpoints are bounded to 32 entries and 8 MiB of aggregate logical state bytes. These are retained-input bounds, not measured allocator overhead or peak process memory.

The profile binds its identity/generation, host, model, tokenizer and state-schema generations and declared restart grade. Audit-only state cannot become a restart checkpoint. The controller does not accept an unqualified profile change through an ordinary actor-state update. Checkpoints contain no permits, reservations, policy floor or recoverable copy of live authority. Their private issuer handles prevent same-number checkpoint substitution across controllers.

A reset stages changes to the existing rights accounting before publication. It increments the existing out-of-band incident counter, advances the revocation epoch, intersects permissions and cancels every undispatched attempt, including attempts still allowed by the retained target set. This conservative full-continuation invalidation prevents an old issued permit or pending proposal from surviving actor rewind. New work needs a fresh attempt, current epoch, new review and normal authorization.

Dispatched, committed, unknown and irrecoverably unknown effects are not refunded or relabeled by reset. The trusted clock observation also does not rewind. At the configured incident threshold, the controller suspends instead of restoring the actor; it still allows reconciliation of prior effects. Actor revisions remain monotone, so delayed pre-reset host updates are rejected. Exhaustion of epoch, incident, control-sequence or actor-revision counters refuses without publishing a partial transition.

This models one authority domain. It does not implement durable or principal-wide cross-domain incident aggregation. Host state completeness, restart qualification, checkpoint selection and its suitability as a pre-incident state are trusted controller inputs. It does not execute a transformer, verify tensor semantics, establish functional restart experimentally or prove that the actor cannot detect a reset.

## Regression source

The initial 27 tests remain. The second batch adds 21 test functions: three actor-state/exhaustion tests, nine public reset lifecycle tests and nine public bound-session tests. These counts describe source, not executed or passing results.

[Reset lifecycle tests](../crates/fa-reference/tests/containment_reset.rs) pair successful restoration and fresh dispatch with forbidden refunds, stale host updates, old permits/reviews, widened ceilings, foreign checkpoint handles, insufficient restart grades and retention-bound violations. They include repeated-incident suspension and continued outcome reconciliation.

[Bound-session tests](../crates/fa-reference/tests/bound_congress_session.rs) pair successful approval and dispatch with cross-issuer and cross-context commitments/reviews, missing reveals, stale reviews after reset, revocation during voting, bad reveals and exact-disqualifier dominance. A narrowing scenario refunds an outside reservation while preserving a new allowed review/dispatch path.

The initial [congress-to-dispatch integration tests](../crates/fa-reference/tests/consequence_review_to_dispatch.rs) and the independent lattice oracle remain applicable. No production guarantee or executed qualification follows from this test source alone.

## Founding and roadmap relationship

This is scoped reference progress for FA-104 and FA-108, following plan sections 8.2, 9.2, 9.4, 9.8 and 11.10. The founding connection remains FI-A02 (external control), FI-A09 (consequences) and FI-A16 (one-directional control and containment), combined with the introspection essay's checkpoint/rewind mechanism. It does not replace or close the original production acceptance criteria.

Remaining work includes admitted cryptography and authenticated round/action/policy provenance, durable hashed DecisionClosure integration, production purpose contexts and effect adapters, actual qualified inference-host checkpoint capture/restoration, independent-label reporter annotations, offline training-signal export and the required executed negative campaigns. Target ceilings and state profiles here remain finite reference contracts rather than a complete production capability or host abstraction.
