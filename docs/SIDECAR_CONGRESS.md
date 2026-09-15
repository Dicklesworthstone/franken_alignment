# Source-checked learned sidecar evidence for the helper congress

## Implemented bridge

`oversight::sidecar::SidecarCongressPlan` connects the existing source-checked learned-KV representation to the existing complete helper-input and congress contracts. It is a partial reference implementation of the FA-113 sidecar-to-congress path. It does not replace the congress, introduce a second verdict reducer, authenticate helpers, qualify a codec, or grant effect authority.

The initial helper input contains the exact frozen action frame, a `FASIDE/1` evidence payload and each member's original configured question under its existing input profile. The evidence payload contains `CheckedLearnedKv::encode_base()`: the fitted learned representation plus source-derived per-group error radii and retained-residual lengths, but no exact residual contents. The payload is represented as a normal `PartKind::Evidence`, with a complete `EvidencePartView` and `AuthorizationProjection`, so every existing `CommitteeInput` full-roster, action, question, policy-epoch and provenance check still runs.

All members receive the same sidecar evidence bytes in this initial profile. Their questions and profile identities remain member-specific. The caller-supplied `SidecarIdentity` names the original evidence object/generation and the frozen transform. These identities and the original source capture remain trusted reference inputs; this bridge does not create cryptographic provenance.

## Explicit unanswerable/refinement semantics

This adapter interprets a completed original-round `Verdict::Abstain` as an explicit request for more sidecar evidence. It does **not** change the general congress semantics: an abstention still makes that completed round restrictive if it is applied. The planner therefore examines an **unapplied** `ObservedReview`. A substantive all-member round returns `Final`; the caller may then apply that exact review through the ordinary broker.

If any member is missing, the result is `Missing`, not a refinement request. Transport/protocol absence cannot buy evidence or disappear from the denominator. If every present helper is substantive, no refinement occurs. If one or more helpers abstain, the planner may purchase exactly one residual from a priority list frozen when the plan is constructed.

Every priority group must already have an exact residual retained by the original `CheckedLearnedKv` source. Missing residuals refuse plan construction. There is no hidden original-state fetch, dynamic raw-window fallback, helper-selected group, or MSE-based reconstruction of missing evidence. An empty/exhausted priority list returns `Unresolved`.

A refined payload contains the same complete checked base plus all residual blocks purchased so far, in canonical group order. The new input is a new `CommitteeInput`, and recording it advances the original input revision. The caller must begin a **distinct congress round** over that new input. The earlier abstaining review is not transformed into a permit or edited in place. Applying an old restrictive review remains intentionally allowed by the existing broker, but can only hold/deny; it cannot authorize the newly refined evidence.

## Resource contract

`SidecarCongressBudget` freezes three independent cumulative caps: generated input versions (`rounds`), exact residual bytes purchased once before member replication, and the existing `CommitteeInput::logical_bytes()` charged across every generated version. The reference maxima are 32 input versions and 8 MiB for each byte account. The normal `ActualHelperInput` 64 KiB per-helper input cap remains independently enforced.

Before committing a refinement to planner state, the adapter constructs the complete candidate payload and committee input and verifies the new cumulative totals. If the frozen budget cannot afford another version, it returns `BudgetExhausted`; it does not omit the requested scrutiny and continue. Failed candidates do not advance selected groups or cumulative work. Exact residual bytes are charged once as purchased evidence, while retransmission to every helper and in every later version is visible in cumulative committee-byte accounting.

This is logical retained/input accounting, not network bytes, helper inference cost, allocator traffic, RSS or latency. A larger deployment planner still needs matched-cost raw/text baselines and a policy for selecting the highest-value residual. This reference uses a frozen explicit priority order so no data-dependent heuristic can silently rewrite the scrutiny contract.

## Authority boundary

`SidecarCongressPlan` owns no `OversightBroker`, round, permit or endpoint. It can only construct validated evidence and classify a finished review as final, missing, refinable, exhausted or unresolved. The positive integration path is deliberately the unchanged authority sequence:

1. original action proposal;
2. coarse sidecar `CommitteeInput` recorded;
3. original congress round abstains;
4. one checked residual is purchased;
5. refined input is recorded at a new revision;
6. a fresh original congress round returns substantive Allow votes;
7. the original broker applies that review, authorizes, dispatches and receives an original endpoint receipt.

Thus sidecar evidence can affect what helpers see but cannot turn numerical quiet, a residual, or an abstention into authority. Old coarse restrictive judgments remain fail-closed. Missing evidence never increases authority.

## Source scenarios and verification status

Seven new integration-test functions cover coarse-abstain-refine-Allow through actual original authorization and endpoint execution; old restrictive review application after refinement; exact helper payload/question/profile binding; missing-worker separation; cumulative round/residual/committee resource exhaustion; unavailable residual and exhausted priority behavior; and cross-sidecar review-binding refusal. The positive test confirms that coarse bytes do not contain the exact residual and that the refined input does.

The tests use the real in-memory `OversightBroker`, `ObservedSession`, congress reducer, input-revision semantics, permit consumption and `PublicationEndpoint`. The helper verdicts themselves are synthetic test inputs; no trained helper model or process was executed by these tests. This increment therefore establishes reference data/control integration, not FA-113's empirical matched-cost comparison, helper independence, attack-family performance or production qualification.

Required verification command:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test sidecar_congress
```

The current editing environment does not provide the required RCH toolchain, and no local compilation fallback is permitted. Rust compilation, tests, doctests, formatting and Clippy remain unverified. No Beads task or G4 packet is closed by this source construction.
