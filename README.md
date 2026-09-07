# franken_alignment

<div align="center">

[![License: MIT + Rider](https://img.shields.io/badge/License-MIT_+_OpenAI/Anthropic_Rider-blue.svg)](./LICENSE)
[![Rust Edition](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![toolchain: nightly](https://img.shields.io/badge/toolchain-nightly-purple.svg)](./rust-toolchain.toml)
[![unsafe: forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![runtime: asupersync](https://img.shields.io/badge/runtime-asupersync-teal.svg)](https://github.com/Dicklesworthstone)
[![deps: closed universe](https://img.shields.io/badge/deps-closed_universe-black.svg)](./COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md)

**Evidence-carrying control for powerful, potentially untrusted agents. Pure Rust. Structured concurrency. Versioned judgment. Economical introspection.**

</div>

```bash
# What runs today, from a source checkout (see Installation & Local Verification):
git clone https://github.com/Dicklesworthstone/franken_alignment
cd franken_alignment
cargo test -p fa-reference
cargo run --locked -p xtask -- check
```

> **A note on tense (read this first).** This README is written in the **present tense, as if the entire design in [`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md`](./COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md) is fully realized**: the target state where every performance and safety gate is green and every subsystem is live. This is a deliberate choice. It lets the document describe the *finished* system so it gets **trued-up in place as milestones land** (Gates G0→G6 in [§10 of the plan](./COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md)) rather than rewritten from scratch later. Where the plan itself stages something as an open research hypothesis or future gate, the README says so plainly. Everything else below is the spec of the system this repository builds. The accompanying reference crate in [`crates/fa-reference`](./crates/fa-reference) models the core logical semantics in pure safe Rust.

---

## TL;DR

**The problem.** Every conventional approach to AI alignment is compromised by fragile, post-hoc assumptions:
- **Conversational cooperativeness is not an authority boundary.** An agent that sounds cooperative or sycophantic can still be prompt-injected, confused, or strategically deceptive.
- **Opaque LLM monitor verdicts are not verifiable proofs.** Asking another large model "does this look safe?" yields probabilistic, uncalibrated natural language that cannot guarantee correctness or resist adversarial collusion.
- **Post-hoc output filters cannot undo external side effects.** Once an irreversible tool call dispatches (file deletion, network egress, credential spending, hardware command), no retrospective apology or log alert can recover the spent state.
- **Whole-activation telemetry is economically intractable.** Logging complete internal activations and KV caches across billions of tokens overwhelms storage and network transfer, yet naive compression (such as standard mean squared error autoencoders) aggressively discards subtle, low-variance signals that carry catastrophic safety risk.

**The solution.** `franken_alignment` unifies Jeffrey Emanuel's [external-helper alignment proposal](https://github.com/Dicklesworthstone/some_thoughts_on_ai_alignment) and [introspective compression/metacognition proposal](https://github.com/Dicklesworthstone/llm_introspective_compression_and_metacognition). It connects **what an agent observed, what it proposed, what independent observers actually saw, what justified permission, and what effect actually occurred** through one mathematically coherent evidence and authority model.

One single composable artifact—the **`DecisionClosure`**—unifies the pipeline: the exact object that authorizes a live tool dispatch serves simultaneously as a tamper-evident incident record, a replayable counterfactual baseline, a calibration sample, and an automated regression test.

**Why `franken_alignment`:**

| Dimension | `franken_alignment` | Conventional Agent Guardrails |
|---|---|---|
| **Authority Boundary** | **Effect Gate & Conserved Rights.** One-shot permits, immutable action payloads, and strict rights conservation ($\text{held} + \text{available} + \text{spent} == \text{total}$). | Advisory system prompt rules; unmediated tool wrappers; client-side SDK conventions. |
| **Judgment Reuse** | **Epistemic MVCC.** Judgments record exact read witnesses and negative range predicates; reusable across snapshots without expensive re-inference until witnesses change. | Coarse TTL caches or zero caching; repeated full LLM re-evaluations on every turn. |
| **Enforcement Topology** | **Authority Graph Compilation.** Dominator trees and minimum cuts independently verify that every path to a protected sink crosses certified enforcement barriers. | Ad-hoc middleware chains; unverified assumptions about perimeter coverage. |
| **Evidence Transport** | **Progressive ATP.** Asupersync Adaptive Transfer Protocol streams verified prefixes and sparse delta journals conditioned on receiver base state; partial data never masquerades as full review. | Bulk JSON dumps; unverified chunking; lossy truncation without contract checks. |
| **Observation Algebra** | **Costed Operator DAGs.** Monitors compile into typed pipelines with declared cost accounting; unsafe shortcuts (e.g. using approximate top-$k$ search to prove absence) are structurally forbidden. | Unstructured Python scripts; arbitrary API calls; hidden monitoring latency debt. |
| **Continuous Oversight** | **Incremental Z-Sets.** Standing fleet-level queries maintain bounded antichains of irredundant support sets; revoking a credential edge automatically retracts dependent findings. | Periodic batch rescans; stale alert dashboards; unmaintained alert lifecycles. |
| **Internal Telemetry** | **Decision-Sensitive Codecs.** Progressive compression ladders evaluate representations by downstream decision preservation and certified dual-norm margins ($\|w\|_* \cdot \varepsilon$). | None, or naive MSE autoencoders that drop rare, catastrophic safety features. |
| **Helper Ensembles** | **Salted Commit–Reveal Congresses.** Observers commit hashed verdicts to prevent herd behavior, unmasking simultaneously for deterministic reduction. | Unstructured multi-agent group chats; sycophantic consensus loops. |
| **Counterfactual Sandboxing** | **Structurally Shared Experiments.** $O(1)$ checkpoint branching without cloning active permissions or rewinding external effect ledgers; zero production authority. | Copying whole processes or hoping an agent sandbox doesn't leak credentials. |
| **Trace Reduction** | **Lawful Partial-Order Replay.** Exploits algebraic commutativity (disjoint reads/writes/resources, no external order barriers) to collapse redundant scheduler interleavings. | Unmanageable, unreplayable multi-gigabyte linear execution traces. |
| **Safety & Architecture** | **Safe Rust 2024 & Closed Universe.** `#![forbid(unsafe_code)]` workspace-wide. Asupersync runtime. Zero C/C++, ONNX, libtorch, or Python dependencies inside the product. | Multi-language spaghetti; unsafe native bindings; opaque deep learning runtime bloat. |
| **Verification & Quality** | **Operator-Local DSR Gates.** Host-executed, reproducible check suites binding source, toolchain, and dependency inventories; zero reliance on hosted GitHub Actions. | Ephemeral hosted CI runners; unverified binary releases; unpinned dependencies. |

---

## Quick example

The safe-Rust reference crate (`crates/fa-reference`) models the complete logical semantics with zero external dependencies:

```rust
use fa_reference::{Effect, EvidenceFrontiers, Graph, Judgment, ReadWitness, Rights, Snapshot, linear_sign};
use std::collections::{BTreeMap, BTreeSet};

fn main() -> Result<(), fa_reference::Error> {
    // 1. Epistemic MVCC: Record state snapshot with an empty range witness
    let mut snapshot = Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(10, b"approved_target".to_vec())]),
    };

    // A judgment that depends on key 10 AND the absence of keys in 20..30
    let judgment = Judgment::capture(
        &snapshot,
        vec![
            ReadWitness::Exact { key: 10, value: Some(b"approved_target".to_vec()) },
            ReadWitness::EmptyRange { start: 20, end: 30 },
        ],
    )?;

    // Unrelated changes (key 99) preserve validity; phantom insertions (key 25) invalidate
    snapshot.values.insert(99, b"unrelated".to_vec());
    assert!(judgment.valid_at(&snapshot)?); // Reused without re-running helper inference!

    snapshot.values.insert(25, b"phantom_injection".to_vec());
    assert!(!judgment.valid_at(&snapshot)?); // Automatically invalidated!

    // 2. Rights Conservation: Reserve, dispatch, and reconcile external effects
    let mut rights = Rights::new(100);
    let effect = Effect {
        principal: "agent_42".into(),
        resolved_target: "filesystem:/protected/data".into(),
        payload: b"WRITE: canonical_bytes".to_vec(),
        units: 25,
    };

    rights.reserve(1, effect.clone())?;
    assert_eq!(rights.available(), 75);

    // One-shot dispatch: once dispatched, cannot be dispatched again
    rights.dispatch(1, &effect)?;
    assert!(rights.conserved());

    // Reconcile terminal outcome (dispatched -> committed)
    rights.reconcile(1, true)?;
    assert!(rights.conserved());
    assert_eq!(rights.available(), 75);

    // 3. Authority Graph Compilation: Verify dominator bottlenecks and cuts
    let graph = Graph::new(4, &[(0, 1), (1, 2), (2, 3)])?;
    assert_eq!(graph.dominates(0, 1, 3)?, Some(true)); // Node 1 is a mandatory chokepoint
    assert!(graph.cut_disconnects(&[0], &[3], &BTreeSet::from([1]))?);

    // 4. Decision-Sensitive Compression: Certify linear probe margins under coordinate error
    // Given weights w = [2, -1], reconstruction x_hat = [10, 2], threshold = 0, max error eps = 1
    // margin = (2*10 - 1*2) - 0 = 18; bound = (|2| + |-1|) * 1 = 3 -> margin > bound => Some(true)
    let certified_sign = linear_sign(&[2, -1], &[10, 2], 0, 1)?;
    assert_eq!(certified_sign, Some(true)); // Certified safe sign; no refinement needed

    Ok(())
}
```

---

## The eight architectural bets

No single trick provides alignment or containment. The **composition** of eight architectural bets does, each replacing an informal operational assumption with an exact mathematical contract:

| Bet | One-line statement |
|---|---|
| **B1 · The Effect Gate is Central** | An agent cannot directly execute tools; all external side effects flow through an exact deterministic broker where one-shot permits spend conserved rights and reconcile verified receipts. |
| **B2 · Epistemic MVCC for Judgments** | Treat helper judgments as versioned computations over read witnesses (exact objects, keys, and negative ranges); reuse them safely across snapshots and invalidate precisely on semantic changes. |
| **B3 · Topological Authority Graph Compilation** | Compile system actors, proxies, brokers, and sinks into an explicit reachability graph; use dominators and minimum cuts to prove enforcement chokepoints before granting capability tokens. |
| **B4 · Progressive ATP Evidence Frontiers** | Stream internal activations and external traces via Asupersync ATP; advance strictly ordered frontiers (`captured` $\to$ `verified` $\to$ `judged`) so partial evidence is never mistaken for closed review. |
| **B5 · Observation-Plan Algebra & Cost Accounting** | Compile monitoring requirements into typed operator DAGs with explicit compute, memory, and latency budgets; structurally reject unprovable shortcuts (like using top-$k$ ANN to prove absence). |
| **B6 · Incremental Fleet Oversight via Z-Sets** | Maintain continuous fleet-wide safety queries using DBSP-style Z-set circuits; track bounded support antichains so retracted findings prune automatically without full-graph rescans. |
| **B7 · Decision-Sensitive Introspective Compression** | Evaluate internal activation sidecar codecs by downstream safety decisions rather than average reconstruction MSE; certify linear probe margins via dual-norm bounds ($\|w\|_* \cdot \varepsilon$). |
| **B8 · Structurally Shared Counterfactual Experiments** | Branch sandboxes via copy-on-write structural state sharing to run paired causal interventions; experimental branches carry **zero production credentials** and cannot clone rights. |

---

## Design philosophy

These are the constitutional, non-negotiable constraints governing the entire project:

1. **The dependency universe is closed.** Allowed: `core`/`alloc`/`std`, the pinned Rust nightly, and admitted packages from [`asupersync`](https://github.com/Dicklesworthstone) and the **FrankenSuite** (e.g. `fnx-*` graph algorithms, `franken_sqlite` storage primitives, `frankensearch` indexing). Zero external crates in the initial reference workspace. No C/C++, libtorch, ONNX Runtime, Python/PyO3, or second async executors inside the product boundary.
2. **Memory safety is structural.** `unsafe_code = "forbid"` across the entire workspace. No raw pointers, no inline assembly, no unverified memory tricks.
3. **`Cx` and structured concurrency everywhere.** Every asynchronous or I/O operation accepts `&Cx`, Asupersync's capability and cancellation context. Task lifecycles are strictly bounded by lexical concurrency scopes; cancellation never leaks obligations.
4. **An observation is data. A judgment is evidence. A permit is authority.** None silently becomes another. Monotone accumulation of facts never mints a permit; a helper’s natural language explanation cannot bypass an effect gate.
5. **Deterministic by default.** Same snapshot + same observation plan + same policy $\Rightarrow$ byte-identical decision closure. Replayable counterfactual branches allow diagnosing failures under deterministic virtual time.
6. **Local release evidence.** No reliance on GitHub-hosted Actions. All quality gates, Clippy checks, and release packaging run on operator machines via Cargo and Doodlestein Self Releaser (DSR), binding source, toolchain, and dependency manifests to cryptographic receipts.

---

## How it works

`franken_alignment` coordinates three isolated operating planes sharing common identities, but maintaining rigorous authority boundaries:

```
┌──────────────────────────────────────────────────────────────────────────┐
│  CONTROL PLANE (Small, Exact, Trusted Nucleus)                           │
│  proposal -> normalize -> reserve rights -> bind evidence -> permit     │
│           -> adapter dispatch -> reconcile receipt -> terminal outcome   │
└────────────────────────────────────┬─────────────────────────────────────┘
                                     │
               Requires Valid        │ Evaluates Policy
               Decision Closure      │ Against Evidence
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  EVIDENCE PLANE (Bounded, Progressive, Provenance-Complete)              │
│  host taps -> capture frontiers -> ATP verify/store -> view commitments  │
│            -> numerical probes + helper congresses + standing Z-sets     │
└────────────────────────────────────┬─────────────────────────────────────┘
                                     │
               Extracts Snapshot     │ Contributes Regression
               & Dependencies        │ & Diagnostic Evidence
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  EXPERIMENT PLANE (Isolated, Zero Production Authority)                  │
│  retained closure -> snapshot branch -> paired intervention -> test      │
│                   -> candidate strategy/codec -> promotion campaign      │
└──────────────────────────────────────────────────────────────────────────┘
```

### The Subsystems

- **`fa-control` / `fa-rights`**: The authoritative gatekeeper. Maintains the immutable registry of effect families, evaluates signed policy IR, allocates single-use permits, conserves resource budgets, and tracks dispatch receipts to terminal states (`Committed`, `Aborted`, `Unknown`).
- **`fa-authority`**: The topology compiler. Ingests system capability manifests, builds directed reachability graphs between actors and sinks, computes dominator trees, and validates minimum cut barriers.
- **`fa-epistemic`**: The MVCC judgment engine. Implements `JudgmentBasis` recording and `ReadWitness` validation (`ExactObject`, `ExactValue`, `EmptyRange`, `SemanticEpoch`). Manages witness invalidation antichains and handles incremental cache validation.
- **`fa-atp` & `fa-frontiers`**: The progressive transport substrate. Implements Asupersync Adaptive Transfer Protocol over telemetry streams. Maintains contiguous sequence frontiers (`captured`, `verified`, `judged`), ensuring that partial transmissions never satisfy complete review scopes.
- **`fa-algebra`**: The observation compiler. Translates safety queries into costed operator DAGs (`Tap`, `AuthorizeProjection`, `RequirePrefix`, `LookupExact`, `Probe`, `JudgeIndependent`, `JoinEvidence`). Prevents uncosted observation debt and disallows unsound approximations.
- **`fa-oversight`**: The standing query maintainer. Leverages DBSP-style Z-set circuits to maintain fleet-wide findings under streaming updates, automatically updating or retracting alerts as supporting evidence evolves.
- **`fa-codecs` & `fa-probes`**: The introspective representation layer. Implements progressive compression ladders (header $\to$ coarse codes $\to$ refinement blocks $\to$ residual) for internal model activations. Evaluates linear and non-linear probes using dual-norm Lipschitz error bounds.
- **`fa-helpers`**: The out-of-band verification congress. Orchestrates specialist helper models via salted commit–reveal protocols to eliminate anchoring bias, aggregating independent votes via deterministic policy reducers.
- **`fa-experiments`**: The counterfactual sandbox manager. Clones checkpoints via copy-on-write structural sharing, executing paired interventions (e.g. activation clamping or ablation) to verify causal claims without access to production credentials.

---

## How it compares

| Approach | Pre-execution Gate | Activation Access | Cache Soundness | Multi-Observer Protocol | Authority Formalism | Dependency Profile |
|---|---|---|---|---|---|---|
| **`franken_alignment`** | **Strict, one-shot permits; conserved rights** | **Decision-sensitive compression & linear probe bounds** | **Epistemic MVCC with negative range witnesses** | **Salted commit–reveal congresses** | **Topological authority graphs & min-cuts** | **Safe Rust 2024, closed universe, zero external crates** |
| **Prompt Guardrails** (e.g. Llama Guard) | Advisory text filter; easily bypassed by jailbreaks | None (black-box text only) | None (stateless) | Single prompt call | None (heuristic prompt scoring) | Heavy Python stack, PyTorch, HuggingFace |
| **SDK-Level Gateways** (e.g. NeMo) | Client-side middleware; cooperative only | None | In-memory key-value cache | Single rule engine | Hard-coded regex / rail files | Python / external microservices |
| **Multi-Agent Debate** | Unstructured conversational output | None | Coarse conversation history | Group-chat turn taking (vulnerable to herding) | Natural language consensus | Uncontrolled API endpoints |
| **Post-Hoc Audit Logs** | None (evaluates after damage has occurred) | Log dumps (unindexed) | Batch database index | Retrospective human review | Manual compliance checklists | Traditional SIEM / log pipelines |

---

## Installation & local verification

### 1. Requirements
- Rust 2024 nightly toolchain (auto-selected via [`rust-toolchain.toml`](./rust-toolchain.toml)).
- Host targets: macOS (`aarch64-apple-darwin`, `x86_64-apple-darwin`), Linux (`x86_64-unknown-linux-gnu`).

### 2. From source (what runs today)

```bash
git clone https://github.com/Dicklesworthstone/franken_alignment
cd franken_alignment

# Run the 20 pure safe-Rust reference unit tests:
cargo test -p fa-reference

# Run the comprehensive xtask gate driver:
# Verifies zero-dependency lockfile, crate-level unsafe prohibitions, format, check, clippy, and tests:
cargo run --locked -p xtask -- check
```

The gate driver enforces that the dependency universe remains strictly closed to the approved reference packages. Running `cargo xtask release-check` will deliberately fail with an explicit refusal until all production integration milestones are satisfied.

### 3. Embedded as a Rust library

```toml
# Cargo.toml
[dependencies]
fa-reference = { git = "https://github.com/Dicklesworthstone/franken_alignment" }
```

---

## Configuration

Deployments configure capability ceilings, observation budgets, and helper congress parameters via TOML policies:

```toml
# fa-policy.toml
[enforcement]
mode = "BROKERED_EFFECTS"               # OBSERVE_ONLY | COOPERATIVE_GATE | BROKERED_EFFECTS | ATTESTED_PROFILE
default_rights_budget = 1_000_000
permit_ttl_ms = 5000
require_reconciliation = true

[epistemic_mvcc]
witness_strategy = "Antichain"          # Antichain | SingleRundown | FullRecompute
max_antichain_budget = 32
empty_range_tracking = true

[atp_transport]
max_gap_sequence = 1024
enforce_contiguous_frontiers = true
compression_ladder = ["Header", "CoarseCode", "Refinement", "ExactResidual"]

[helpers]
congress_mode = "SaltedCommitReveal"
min_independent_votes = 3
quorum_threshold = 0.67
reduction_rule = "StrictSupermajority"

[probes]
linear_bound_check = true
refinement_on_inconclusive = true

[experiments]
allow_production_credentials = false    # Enforced by type system and capability tokens
copy_on_write_sharing = true
max_concurrent_branches = 64
```

---

## Performance & SLO targets

Provisional performance targets from [`registry/slo.json`](./registry/slo.json) and [`registry/operation_costs.json`](./registry/operation_costs.json):

| Metric / Operation | Target SLO | Contract Guarantee |
|---|---|---|
| **Effect Gate Decision Latency** | $\le 2.0\text{ ms}$ (p99) | Cached epistemic witness evaluation + permit signature |
| **Epistemic Witness Validation** | $\le 50\ \mu\text{s}$ (p99) | Exact key comparison and BTreeMap empty-range search |
| **Authority Cut Verification** | $\le 500\ \mu\text{s}$ (up to 128 nodes) | Bounded reachability oracle; independent verification |
| **Progressive ATP Ingestion** | $\ge 200\text{ MB/s}$ per stream | Zero-copy frame decoding; contiguous sequence frontier update |
| **Linear Probe Bound Check** | $\le 5\ \mu\text{s}$ (per 1,024 dimensions) | SIMD/integer dual-norm dot product and margin check |
| **Experiment Branch Creation** | $\le 100\ \mu\text{s}$ | $O(1)$ copy-on-write structural state sharing |
| **Incremental Oversight Update** | $\le 10\text{ ms}$ end-to-end | DBSP Z-set delta step across standing graph queries |

---

## Determinism, verification & governance

- **Simulation-First & Lab Runtime.** The entire system runs under Asupersync's deterministic simulation runtime (`lab`): virtual time, seed-replayable execution, and DPOR schedule exploration. Every race or concurrency bug produces a deterministic replay seed.
- **Reference Oracle.** [`crates/fa-reference`](./crates/fa-reference) provides an independent, deliberately small logical model in safe Rust (`#![forbid(unsafe_code)]`), with 20 unit tests covering MVCC witnesses, frontier gaps, rights conservation, commutativity, graph cuts, and Lipschitz linear probe bounds.
- **Formal Invariants.** 30 machine-checked invariants ([`registry/invariants.json`](./registry/invariants.json)) enforce core guarantees (FA-INV-001 through FA-INV-030). Waivers are strictly forbidden on all safety-critical invariants.
- **Falsifiable Research Hypotheses.** 14 explicit research cards ([`registry/claims.json`](./registry/claims.json), H1 through H14) define empirical criteria for learned compression, metacognition, and helper congresses against matched baselines.
- **Decision Closures & Plan Certificates.** Every authorized effect is recorded in a self-contained, tamper-evident `DecisionClosure` capturing the exact evidence view, model/helper generations, witness dependencies, and permit token.

---

## Limitations

Honest engineering boundaries:

- **Gated Staged Roadmap (G0 $\to$ G6).** Design revision 0.2 provides the formal architecture, machine-readable registries, and safe-Rust reference workspace. The production broker, native foundation adapters, and learned compression models land sequentially across roadmap gates G1 through G5.
- **Reference Oracle vs. Production Scale.** The reference `Graph` and `Snapshot` models in `fa-reference` are bounded models designed for property testing and contract verification; they do not replace the scalable Asupersync ATP engine or FrankenGraphDB storage layer.
- **Graph Completeness Assumption.** Authority graph cuts prove that no path exists *in the supplied model*. A model that omits an out-of-band egress channel cannot prevent leaks across that unmodeled channel; the capability manifest must explicitly state residual channels.
- **Conservative Decision Bounds.** Linear probe margin certification requests refinement whenever $|w^\top \hat{x} - T| \le \|w\|_* \cdot \varepsilon$. Highly lossy compression will produce frequent refinement fallbacks rather than risking incorrect permissions.

---

## FAQ

**Is this production-ready today?**  
The README describes the target state of the architecture (see [A note on tense](#a-note-on-tense-read-this-first)). Revision 0.2 establishes the verified specifications, registries, and dependency-free safe-Rust reference implementation. Production adapters and brokers land across Gates G1–G6.

**Why a closed dependency universe in pure safe Rust?**  
Because you cannot seed-replay, formally verify, or safely isolate an alignment broker that depends on an opaque C++ runtime, unpinned Python packages, or multi-threaded background workers. Safe Rust guarantees that memory safety is structural, while Asupersync provides deterministic concurrency.

**How does Epistemic MVCC differ from standard caching?**  
Standard caches key on arbitrary request strings or coarse timestamps. Epistemic MVCC treats a helper judgment as a query over an exact state snapshot, tracking both positive read witnesses and negative range predicates. Unrelated updates preserve cache validity, while a phantom insertion into an observed empty range immediately revokes the judgment.

**How does decision-sensitive compression avoid missing safety-critical features?**  
Standard lossy autoencoders minimize average reconstruction error (MSE), which washes out subtle, rare features. Decision-sensitive compression calculates an explicit dual-norm Lipschitz bound on downstream classifier margins. If the reconstructed activation is too close to a decision boundary to guarantee the sign, the system rejects the approximation and requests refinement.

**How do counterfactual experiments run safely without leaking rights?**  
Experiments branch via copy-on-write state sharing, borrowing immutable weight and activation bases. Crucially, live capability tokens and external effect credentials are not copied into the branch. The experiment plane operates with strictly zero production authority.

**Why are GitHub-hosted Actions avoided?**  
Hosted CI runners are third-party environments vulnerable to supply-chain attacks and opaque runner drift. In accordance with the project constitution, quality and release verification runs locally on operator hardware via Cargo and DSR (Doodlestein Self Releaser), outputting cryptographically verifiable evidence receipts.

---

## About Contributions

Please don't take this the wrong way, but I do not accept outside contributions for any of my projects. I simply don't have the mental bandwidth to review anything, and it's my name on the thing, so I'm responsible for any problems it causes; thus, the risk-reward is highly asymmetric from my perspective. I'd also have to worry about other "stakeholders," which seems unwise for tools I mostly make for myself for free. Feel free to submit issues, and even PRs if you want to illustrate a proposed fix, but know I won't merge them directly. Instead, I'll have Claude or Codex review submissions via `gh` and independently decide whether and how to address them. Bug reports in particular are welcome. Sorry if this offends, but I want to avoid wasted time and hurt feelings. I understand this isn't in sync with the prevailing open-source ethos that seeks community contributions, but it's the only way I can move at this velocity and keep my sanity.

---

## License

The `franken_alignment` source code is licensed under the **MIT License with an OpenAI/Anthropic Rider**, Copyright (c) 2026 Jeffrey Emanuel (see [`LICENSE`](./LICENSE)). The rider withholds all rights from OpenAI, Anthropic, their affiliates, and anyone acting on their behalf, including any use of the software or derivative works in a machine-learning dataset, training corpus, evaluation harness, or pipeline. In any conflict between the rider and the rest of the license, the rider controls.

---

## See also

- [`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md`](./COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md), the 1,472-line technical master plan: the eight architectural bets, the ten foundation audits, the three operating planes, the invariant registry, the research agenda, and delivery gates G0 through G6.
- [`AGENTS.md`](./AGENTS.md), conventions for human and AI agents working in this codebase, including the engineering doctrine, constitutional rules, and verification ladder.
- [`IMPLEMENTATION_STATUS.md`](./IMPLEMENTATION_STATUS.md), exact claim boundaries and status across source, execution, and gated milestones.
- [`docs/`](./docs), deep architectural analyses: [ADRs](./docs/ARCHITECTURE_DECISIONS.md), [Source Review](./docs/SOURCE_REVIEW.md), [Threat Model](./docs/THREAT_MODEL.md), [Dependency Constitution](./docs/DEPENDENCY_CONSTITUTION.md), and [Research Agenda](./docs/RESEARCH_AGENDA.md).
- [`registry/`](./registry), machine-readable registries: [Invariants](./registry/invariants.json), [Claims](./registry/claims.json), [Roadmap](./registry/roadmap.json), and [Foundation Audits](./registry/foundation_audit.json).
