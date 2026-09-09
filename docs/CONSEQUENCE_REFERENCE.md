# Congress, exact policies and checkpoint containment

## Implementation status and verification

Source implemented on September 9, 2026 in the existing std-only `fa-reference` crate. It connects exact policy evaluation, round, empirical reducer, action authority and supplied actor-state checkpoint models. It is not a production congress, inference host, effect broker, durable decision closure or admitted foundation integration.

**Verification is pending for all three implementation batches.** No configured RCH runner was available in these sessions. Formatting, compilation, Clippy, unit tests, integration tests and the complete repository gate were not executed. Existing execution receipts in `IMPLEMENTATION_STATUS.md` describe their own frozen source, not these additions. No bead is closed and no executed-reference or production-admission status is promoted here.

## Feature change log

| Batch | Source progress | Verification status |
|---|---|---|
| Initial congress integration, through `3e5e6828` | Consequence lattice, capped congress evaluation, hold/deny/narrow/suspend enforcement and post-dispatch containment | Source and regression tests only; execution pending |
| Containment and bound sessions, starting at `c47f6d0` | Supplied actor-state restoration, monotone revocation, abandoned-continuation cancellation, repeated-incident suspension, immutable action/policy/issuer-bound congress sessions | Source and regression tests only; execution pending |
| Evidence-backed exact policy control, starting at `398b448` | Bounded policy DAG, automatically captured positive/negative witnesses, enforced proposal/review/authorization/dispatch, immutable policy-evidence reviews, historical policy retention, fenced policy replacement integrated with checkpoint containment | Source and regression tests only; execution pending |

The earlier `action.rs` implementation remains in place. No dependency, runtime, inference backend or cryptographic library was added.

## Implemented paths

```text
PolicyAuthority owns exact Policy + congress policy + ContainmentAuthority
  -> evaluate action and supplied snapshot; derive every read witness
  -> known violation: terminal Denied; missing evidence: refuse without mutation
  -> satisfied: freeze witnessed action, enter Reviewing (not Authorized)
  -> PolicySession retains policy, evaluation, semantic epoch and bound congress
  -> immutable PolicyReview
  -> positive review rechecks current evidence; restrictions remain enforceable
  -> PolicyReceipt retains its own exact evidence basis and policy version
  -> normal authorization and dispatch, each with fresh policy/witness checks

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
- [`session::policy`](../crates/fa-reference/src/action/consequence/gate/containment/session/policy.rs) and [`policy::controller`](../crates/fa-reference/src/action/consequence/gate/containment/session/policy/controller.rs): exact predicate evaluation and an owning controller enforcing it across the existing lifecycle.

## Exact policy enforcement

`Policy` is an immutable, generation-tagged, topologically ordered predicate DAG. The last node is the root; forward references, cycles, unreachable decoy nodes, malformed ranges and empty Boolean groups refuse. Supported predicates compare the exact resolved target, payload bytes/length, units, key values, key absence and empty half-open ranges, composed through All/Any/Not. This is a typed Rust policy representation, not a serialized policy language or a complete production policy compiler.

Evaluation produces Satisfied, Violated or Unknown plus an indexed predicate trace and actual read witnesses. It evaluates every node rather than short-circuiting away dependencies. A false exact-value or absence predicate retains what was actually observed, not what the policy wished to observe. A nonempty range retains an actual member as its nonemptiness witness, so Not(EmptyRange) has a usable positive path. Empty ranges use the existing negative-domain witness. Predicate boundaries and half-open endpoints are exact; the interval profile cannot include `u64::MAX` as a member, and never synthesizes an overflowing successor.

The constructor bounds nodes to 128, edges to 512, literal bytes to 64 KiB and state-read leaves to the existing 64-witness action limit. Evaluation counts every retained occurrence before cloning provider values and refuses above the existing 64 KiB witness-byte limit. These are logical retention/work bounds, not allocation or performance measurements. Every supplied snapshot is still a trusted reference input. Incomplete coverage cannot become permission through Not or a satisfied Any arm; the controller requires complete evidence even when the root alone could be determined.

`PolicyAuthority::propose` rejects caller-supplied witness lists. It derives the list and judgment from the owned policy before creating the action. Known violations are terminally denied without allocating a reservation or requiring helper votes. Satisfied proposals remain Reviewing. Neither a fabricated Judgment nor disqualification/contradiction Booleans are accepted by the controller's proposal, review, authorization or dispatch APIs. A later observed exact violation becomes a disqualifier derived by the evaluator, so unanimous empirical approval cannot override it.

Positive review application, authorization and dispatch all re-evaluate the exact policy and validate the original captured witnesses. Changed bytes, inserted absent keys, range phantoms, semantic epochs, revoked policy epochs, expiry and final-action substitution refuse at their respective boundaries. Unrelated state changes remain reusable when the captured dependencies are unchanged. A change in an unselected Boolean arm or in the selected nonempty-range witness may conservatively require a fresh attempt even if the root remains satisfied; no fact-equivalent optimal invalidation claim is made.

Restrictive reviews remain applicable when evidence becomes unavailable: their immutable before-vote evaluation is retained, rather than silently replacing a later violation with the proposal's original passing result. `PolicySession` and `PolicyReview` retain the exact policy, observed witnesses and snapshot semantic epoch alongside the existing authority-bound congress. `PolicyReceipt` attaches that basis to the actual control transition. Historical policies are shared as immutable data, not authority, and remain available after replacement for interpreting earlier traces and proposals. These records are neither durable nor cryptographically authenticated.

Policy replacement is an explicit trusted controller operation with expected control sequence, expected revocation epoch and a strictly newer policy generation. It stages accounting before publication, advances the sequence/epoch and cancels all old undispatched attempts, including held reservations. It never refunds dispatched or unknown effects, widens a previously narrowed target ceiling, or reopens a suspended run. Completed old-policy reviews and old permits fail; new work needs a new epoch, evaluation and congress. Replacement history is bounded to 64 changes. This is reference governance sequencing, not authenticated production administrator access.

The controller owns the existing `ContainmentAuthority` rather than a parallel ledger. Actor checkpoints contain neither policy nor policy history. Reset therefore cannot restore an older policy; after reset, a fresh action is checked against the current policy and new revocation floor. Reconciliation remains available for pre-reset and pre-policy-change effects.

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

These are process-local reference guarantees, not cryptographic binding. The underlying FNV comparison oracle remains non-cryptographic. Existing raw `Round`, `DecisionInputs` and `ReviewRequest` APIs remain available for trusted logical-model inputs; neither bound sessions nor the policy controller authenticate arbitrary users or remove those low-level APIs. The enforced policy controller exposes no mutable raw gate, raw review application or caller-supplied judgment path.

## Checkpoint containment

`ActorState` retains exact supplied token, cache and sampler bytes and a next-position value. This full-prefix profile requires next-position to equal token count and requires nonempty cache/sampler state. Tokens, cache and sampler are bounded individually; checkpoints are bounded to 32 entries and 8 MiB of aggregate logical state bytes. These are retained-input bounds, not measured allocator overhead or peak process memory.

The profile binds its identity/generation, host, model, tokenizer and state-schema generations and declared restart grade. Audit-only state cannot become a restart checkpoint. The controller does not accept an unqualified profile change through an ordinary actor-state update. Checkpoints contain no permits, reservations, policy floor or recoverable copy of live authority. Their private issuer handles prevent same-number checkpoint substitution across controllers.

A reset stages changes to the existing rights accounting before publication. It increments the existing out-of-band incident counter, advances the revocation epoch, intersects permissions and cancels every undispatched attempt, including attempts still allowed by the retained target set. This conservative full-continuation invalidation prevents an old issued permit or pending proposal from surviving actor rewind. New work needs a fresh attempt, current epoch, new review and normal authorization.

Dispatched, committed, unknown and irrecoverably unknown effects are not refunded or relabeled by reset. The trusted clock observation also does not rewind. At the configured incident threshold, the controller suspends instead of restoring the actor; it still allows reconciliation of prior effects. Actor revisions remain monotone, so delayed pre-reset host updates are rejected. Exhaustion of epoch, incident, control-sequence or actor-revision counters refuses without publishing a partial transition.

This models one authority domain. It does not implement durable or principal-wide cross-domain incident aggregation. Host state completeness, restart qualification, checkpoint selection and its suitability as a pre-incident state are trusted controller inputs. It does not execute a transformer, verify tensor semantics, establish functional restart experimentally or prove that the actor cannot detect a reset.

## Regression source

The initial 27 tests remain. The second batch adds 21 test functions: three actor-state/exhaustion tests, nine public reset lifecycle tests and nine public bound-session tests. The third batch adds 25 test functions: nine policy-kernel tests, nine controller tests and seven public-API integration tests. These counts describe source, not executed or passing results.

[Policy public-API tests](../crates/fa-reference/tests/policy_control_path.rs) exercise positive dispatch with generated witnesses, incomplete congress holds, individually varied action/state violations, coverage/semantic/expiry refusals, independently retained later-disqualification evidence, held reservations versus unknown liabilities across policy replacement and reset, a small independent three-valued truth table and exact policy-history capacity. Unit cases additionally cover graph/byte/read limits, unknown Boolean composition, nonempty-range witnesses, action substitution, cross-controller bindings, stale reviews and atomic overflow refusal.

[Reset lifecycle tests](../crates/fa-reference/tests/containment_reset.rs) pair successful restoration and fresh dispatch with forbidden refunds, stale host updates, old permits/reviews, widened ceilings, foreign checkpoint handles, insufficient restart grades and retention-bound violations. They include repeated-incident suspension and continued outcome reconciliation.

[Bound-session tests](../crates/fa-reference/tests/bound_congress_session.rs) pair successful approval and dispatch with cross-issuer and cross-context commitments/reviews, missing reveals, stale reviews after reset, revocation during voting, bad reveals and exact-disqualifier dominance. A narrowing scenario refunds an outside reservation while preserving a new allowed review/dispatch path.

The initial [congress-to-dispatch integration tests](../crates/fa-reference/tests/consequence_review_to_dispatch.rs) and the independent lattice oracle remain applicable. No production guarantee or executed qualification follows from this test source alone.

## Founding and roadmap relationship

The consequence and containment work is scoped reference progress for FA-104 and FA-108, following plan sections 8.2, 9.2, 9.4, 9.8 and 11.10. Exact policy enforcement additionally serves the frozen-action/effect-bound permit obligations in sections 8.1, 8.2 and 25.1. The founding connection remains FI-A02 (external control), FI-A06 (evidence supporting control), FI-A09 (consequences) and FI-A16 (one-directional control and containment), combined with the introspection essay's checkpoint/rewind mechanism. This does not replace or close the original production acceptance criteria or a full policy-compiler packet.

Remaining work includes admitted cryptography and authenticated round/action/policy/snapshot provenance, durable hashed DecisionClosure integration, production purpose contexts and effect adapters, actual qualified inference-host checkpoint capture/restoration, independent-label reporter annotations, offline training-signal export and the required executed negative campaigns. Target ceilings and state profiles here remain finite reference contracts rather than a complete production capability or host abstraction.
