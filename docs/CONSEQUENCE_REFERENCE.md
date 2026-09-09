# Congress-to-consequence reference integration

## Scope and verification status

Source implemented on September 9, 2026 in the existing std-only `fa-reference` crate. This connects the previously separate round, empirical reducer and action-authority models. It is not a production congress, effect broker, durable decision closure or admitted foundation integration.

**Verification is pending.** This implementation session had no configured RCH runner. Formatting, compilation, Clippy, unit tests, integration tests and the complete repository gate were not executed. Existing execution receipts in `IMPLEMENTATION_STATUS.md` describe their own frozen source, not these additions. No bead is closed and no executed-reference or production-admission status is promoted by this document.

## Implemented path

```text
Round: frozen membership, commitments and reveals
  -> CongressPolicy: exact roster, capped member/cohort influence
  -> RoundEvaluation: tally plus missing/abstaining members
  -> Decision: restriction, exact-disqualifier and missing-evidence rules
  -> ReviewRequest: exact action, round, evidence root, reducer generation,
                    expected review-control predecessor
  -> ConsequenceAuthority: hold / deny / narrow / suspend
  -> existing ReferenceAuthority: authorize, dispatch, reconcile
```

The public modules are:

- [`action::consequence`](../crates/fa-reference/src/action/consequence.rs): independent five-element restriction lattice, six primary consequence tags, strict two-byte versioned reference encoding, reset requirements and deterministic rule composition.
- [`action::consequence::congress`](../crates/fa-reference/src/action/consequence/congress.rs): frozen-round evaluation using the existing proportional member/cohort-capped reducer and an explicitly supplied threshold policy.
- [`action::consequence::gate`](../crates/fa-reference/src/action/consequence/gate.rs): consequence-aware composition owning the existing reference authority and rights ledger. It exposes neither a mutable inner authority nor a cloned permit.

The existing `action.rs` implementation is preserved; its only change is the module declaration. No dependency or runtime was added.

## Behavioral contracts

| Operation | Reference behavior |
|---|---|
| Continue | Allows the normal authorization path; does not mint a permit or reserve units by itself. Dispatch still rechecks the issuer, exact frozen action, witnesses, scope, policy epoch, deadline and one-use state. |
| HoldEffect | Blocks authorization or dispatch. A previously authorized attempt retains its reservation and underlying stage, so a hold cannot silently refund units. A fresh round under the current predecessor is required to release the hold. |
| Deny | Terminally denies an undispatched attempt and refunds an existing reservation once. A later Continue cannot resurrect it. |
| NarrowAuthority | Intersects an exact resolved-target ceiling, including adapter, object, contract version, expected resource version and target generation. Cancels only outside-ceiling undispatched work. No widening/restoration operation is exposed. |
| SuspendRun | Closes further admission and dispatch, cancels undispatched work and preserves dispatched/unknown outcomes and liabilities. Trusted outcome reconciliation remains available. |
| Post-dispatch containment | Narrowing and suspension can be triggered by findings about an already-dispatched or completed attempt. Its historical disposition is not rewritten as Denied or Cancelled. |
| ResetToCheckpoint | Encoded and represented as a distinct primary operation requiring a narrowing floor and functional restart. No actor-state restoration is implemented or synthesized by voting. |

An exact disqualifier establishes at least Deny regardless of empirical approval; a stronger run-level consequence does not erase an undispatched attempt's exact denial. Missing mandatory evidence and unresolved contradiction impose at least Hold, without weakening an existing Deny or stronger restriction.

This bounded congress profile requires substantive answers from every frozen member. Missing reveals and explicit abstentions are retained separately; neither is silently removed to make a quorum pass. A helper's Deny is an empirical hold recommendation, not a trusted exact disqualifier. Quorum accounting uses positive admitted influence after clipping, not merely the number of names in a roster. Optional-specialist policies are outside this profile.

Review application checks exact action equality, scope, expected review-control sequence and a previously unused round identity. Reapplying an already-used round after a late reveal cannot replace its earlier hold. All fallible accounting checks finish before the transition publishes its state and reference receipt. The receipt collection is bounded; the review-control sequence is not claimed to be a universal durable event journal.

## Regression source added

Twenty-seven tests are included: five consequence-law tests, six congress-policy tests, twelve gate tests and four cross-module integration tests. They are source, not passed execution evidence.

The independent oracle includes `consequence_chain_laws`, `disqualifier_yields_deny` and `reset_requires_narrowing`. Lattice laws enumerate all five-element pairs and triples; missing-evidence/disqualifier composition enumerates the bounded input combinations.

The [integration tests](../crates/fa-reference/tests/consequence_review_to_dispatch.rs) connect actual reference commitments/reveals through the reducer to authorization and dispatch, including positive dispatch, exact-disqualifier dominance, late-reveal replay refusal and action-substitution refusal. Gate regressions additionally cover reservation conservation, already-issued permit holds, double refunds, narrowing without widening, unknown liabilities, post-dispatch containment, stale predecessors, expiry, revocation and cross-issuer permits.

## Founding and roadmap relationship

This is reference implementation progress for FA-104 and `franken_alignment-consequence-classes-4f1`, especially the independent oracle requested by `.4`. It follows plan sections 8.2, 9.4 and 9.8 and founding ideas FI-A02, FI-A09 and FI-A16: independent judgments must result in concrete external restrictions without granting the actor a defense channel or rewriting the world.

The full acceptance of the production type/reducer/closure/gate beads is not claimed. Remaining work includes authenticated round/action/policy binding, admitted cryptography, durable hashed DecisionClosure integration, production purpose contexts and effect adapters, registered checkpoint restoration, independent-label reporter annotations, offline training-signal export, and the required executed negative campaigns. Target ceilings here are a finite reference profile, not a complete production capability algebra. The underlying `Round` remains explicitly non-cryptographic, and snapshots, policy, contradiction and disqualifier facts remain trusted model inputs.
