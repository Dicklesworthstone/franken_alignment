# System map: the tower of abstractions

**Purpose.** This is the one-page model of FrankenAlignment for an agent that has to understand the situation accurately and drive it with the least expenditure. It restates the [comprehensive plan](../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md) as a tower of nine layers, each answering one question, each depending only on the layers below, each exporting its objects through one epistemic type, all sharing one address scheme, one journal and one verb vocabulary. The machine-readable form is [`registry/system_map.json`](../registry/system_map.json); the vocabulary is [`registry/vocabulary.json`](../registry/vocabulary.json). Plan §6.7 and §6.8 are normative; this document is their navigable projection.

## The tower

| Layer | Name | Question it answers | Its objects | Plan | Invariants that hold across its upper boundary |
|---|---|---|---|---|---|
| L8 | Governance | Is the control system itself still trustworthy? | helper/actor/andon ledgers, risk-theater findings, promotions, assurance profiles, preregistration rows, policy-replay campaigns | §8.10, §9.9, §15.10, §17.10, §20, §22 | FA-INV-032, FA-INV-036, FA-INV-019 |
| L7 | Experiment | What would have happened otherwise? | branches, counterfactual witnesses, elicitation campaigns, canaries, practice sessions, rehearsal outcomes | §11, §12, §14.11 | FA-INV-009, FA-INV-025, FA-INV-033, FA-INV-040 |
| L6 | Closure | Why was this allowed or held, verifiably? | decision closures, control receipts, incident and attack capsules, annotations, handoffs | §7.3, §17.9, §22.8, §17.8 | FA-INV-007, FA-INV-016, FA-INV-037 |
| L5 | Effect | What actually happened in the world? | broker, adapters, dispatch, reconciliation, outcomes, fences | §8.3, §8.4, §8.11, §16.4 | FA-INV-006, FA-INV-018, FA-INV-020 |
| L4 | Authority | What may happen now? | policy epochs, rights, permits, autonomy grades, consequences, revocation floors | §8.1–§8.10, §9.8 | FA-INV-001 through FA-INV-005, FA-INV-034, FA-INV-038 |
| L3 | Judgment | What do independent observers conclude, and at what cost? | probes, helpers, similarity streams, surprise residuals, rounds, reducers, credibility weights | §9, §10.4, §12.6–§12.8, §13 | FA-INV-010, FA-INV-012, FA-INV-031 |
| L2 | Evidence | What can be relied on, under which assumptions? | authenticated objects, view manifests, read witnesses, frontiers, supports and retractions | §7.3–§7.9, §17.7, §15.8 | FA-INV-014, FA-INV-021, FA-INV-022, FA-INV-023, FA-INV-026 |
| L1 | Observation | What was seen, and what was not? | taps, capture streams, coverage records, sidecar codes, tool and environment channels | §7.2, §10, §18.2 | FA-INV-013, FA-INV-015 |
| L0 | Identity | What exactly is this thing? | canonical bytes, digests, addresses, model passports, generations, epochs, the three clocks | §6.5, §7.1, §7.10 | FA-INV-017, FA-INV-035, FA-INV-029 |

Read it bottom-up to build the system and top-down to understand a situation. The founding essays map onto it directly: the alignment essay is L3, L4 and L8 (independent judgment, external authority, governance against theater); the introspection essay is L1, L2 and L7 (economical observation, replayable evidence, rewindable experiment); the plan's engineering is L0, L5 and L6 (identity, effect, closure) and the rules that keep the layers apart.

## The five rules

1. **Downward dependence only.** L(n) imports only from L(<n). The crate DAG of plan §6.2 is the compile-time image of this rule.
2. **One epistemic type at every boundary.** Every exported value is `Known`, `Pending`, `Unknown`, `Withheld`, `Stale` or `Absent` with its basis (plan §17.3). No bare safety-relevant scalar exists above L0 (FA-INV-039).
3. **One address scheme.** `fa://<tenant>/<kind>/<id>[@<generation>]` names every object at every layer; `fa get` resolves all of them; `fa related` walks typed edges.
4. **One journal.** Every L3–L8 transition emits one typed event into one ordered journal per authority domain. Situation, explanation, replay and the risk-theater detector are folds over it.
5. **Executable contracts between layers.** Each boundary has a conformance suite; `fa doctor` composes them bottom-up and reports `Unknown{LowerLayerUnverified}` above any layer whose suite did not execute.

## The epistemic type

```
Knowledge<T> = Known   { value: T, basis: { claim_class, origin, complete_for_contract, semantically_valid,
                                            available_for_replay, generation, control_seq } }
             | Pending { frontier, expected_cost }
             | Unknown { reason }                       // not observed, or lower layer unverified; never permission
             | Withheld{ authority_required }           // exists; caller may not see it; content not inferable
             | Stale   { generation, current }          // known under a previous generation
             | Absent  { closed_domain, frontier }      // observed absent over a complete named domain
```

## The vocabulary

| Layer | Read | Rehearsal / experiment | Mutation |
|---|---|---|---|
| L0–L2 | `get`, `related`, `capabilities`, `when` | | `invalidate` |
| L3 | `explain`, `cost` | `convene --branch` | `convene` |
| L4–L5 | `situation`, `why-held`, `next`, `tail` | `propose --branch`, `precheck` | `propose`, `hold`, `narrow`, `suspend`, `reset`, `fence`, `resolve` |
| L6 | `explain`, `verify` | | `annotate`, `handoff` |
| L7 | `replay-plan` | `branch`, `intervene`, `replay`, `rehearse` | none (no production authority) |
| L8 | `doctor`, `ledger`, `findings` | `policy-replay` | `promote`, `rotate`, `preregister` |

Every mutation names its expected predecessor and an idempotency key. Every rehearsal returns a `RehearsalOutcome` and can never mint a permit (FA-INV-040). Every response is the envelope of plan §17.3 with typed affordances.

## How an agent reads a situation

1. `fa situation --scope run --since <seq>`: the folded state, ordered by decision relevance, every item addressed and wrapped.
2. For a held item: `fa why-held <attempt>` gives the unsatisfied predicates and the affordances that could satisfy them; `fa next <attempt>` ranks those by information gain per cost.
3. Before acting: `fa propose <affordance> --branch <b>` rehearses; the `RehearsalOutcome` shows the tree the nucleus would produce.
4. Act: `fa propose <affordance>`; the response carries the decision card, realized cost and the new control sequence.
5. Understand: `fa explain <closure>` is the predicate tree; `fa related <address>` walks evidence, provenance and annotations.
6. Leave a trail: `fa annotate`, then `fa handoff <run>` so the next agent starts at your control sequence.

## What is not in this map

The map lists layers, boundaries, types, verbs and invariants. It does not claim any of them exists as code today; [`IMPLEMENTATION_STATUS.md`](../IMPLEMENTATION_STATUS.md) is the statement of what exists. Packet FA-132 keeps the map complete and checked against the plan, the registries and the beads, the way FA-103 keeps the founding concordance complete.
