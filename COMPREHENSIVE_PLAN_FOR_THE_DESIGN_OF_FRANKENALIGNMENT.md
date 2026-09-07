# Comprehensive Plan for the Design of FrankenAlignment

**An evidence-carrying control system for powerful, potentially untrusted agents.**

**Design draft 0.3 · September 6, 2026 · Jeffrey Emanuel**

## Contents

0. [The thesis](#0-the-thesis)
1. [From the founding essays to an executable research program](#1-from-the-founding-essays-to-an-executable-research-program)
2. [Constitution and claim discipline](#2-constitution-and-claim-discipline)
3. [The eight architectural bets](#3-the-eight-architectural-bets)
4. [Threat model and trust boundaries](#4-threat-model-and-trust-boundaries)
5. [Foundation audit and reuse contracts](#5-foundation-audit-and-reuse-contracts)
6. [Architecture and dependency direction](#6-architecture-and-dependency-direction)
7. [Identity, evidence, and the observation contract](#7-identity-evidence-and-the-observation-contract)
8. [The effect gate and action lifecycle](#8-the-effect-gate-and-action-lifecycle)
9. [The independent helper congress](#9-the-independent-helper-congress)
10. [Compression as a safety-sensitive information bottleneck](#10-compression-as-a-safety-sensitive-information-bottleneck)
11. [Checkpoints, replay, branching, and metacognition](#11-checkpoints-replay-branching-and-metacognition)
12. [Causal diagnosis without a persuasion channel](#12-causal-diagnosis-without-a-persuasion-channel)
13. [Calibration, sequential evidence, and uncertainty](#13-calibration-sequential-evidence-and-uncertainty)
14. [Observation planning and economic control](#14-observation-planning-and-economic-control)
15. [Multi-agent and distributed control](#15-multi-agent-and-distributed-control)
16. [Persistence, repair, privacy, and retention](#16-persistence-repair-privacy-and-retention)
17. [Integration surfaces and agent ergonomics](#17-integration-surfaces-and-agent-ergonomics)
18. [Performance and operation-cost contracts](#18-performance-and-operation-cost-contracts)
19. [Verification and adversarial evaluation](#19-verification-and-adversarial-evaluation)
20. [Research program and falsification criteria](#20-research-program-and-falsification-criteria)
21. [Delivery sequence and convergence gates](#21-delivery-sequence-and-convergence-gates)
22. [Operations and governance](#22-operations-and-governance)
23. [Alternatives deliberately rejected](#23-alternatives-deliberately-rejected)
24. [What success would actually establish](#24-what-success-would-actually-establish)
25. [Appendix: formal obligations and worked examples](#25-appendix-formal-obligations-and-worked-examples)

---

## 0. The thesis

The most useful alignment toolbox is not another model that says whether a sentence looks dangerous. It is a system in which an agent's observations, proposed effects, independent evaluations, granted authority, and actual outcomes can be related precisely enough to answer four questions:

1. What was the agent able to do at this moment?
2. What evidence justified allowing this particular effect?
3. What could the observers genuinely see, and what was missing?
4. Can a different investigator reproduce or challenge the decision without trusting the original agent, monitor, or dashboard?

FrankenAlignment will make the **effect boundary**, rather than the model's apparent cooperativeness, the center of the design. A model can be internally well trained and still receive only narrowly scoped authority. A helper can be useful and still be treated as an attackable, fallible source of evidence. A compressed state can support a powerful detector without being a faithful record of everything the model represented. These distinctions are not compromises in ambition. They are what makes the ambition implementable.

The founding synthesis is Jeffrey Emanuel's external-helper architecture combined with his sidecar compression and rewindable metacognition proposal. The first supplies independent oversight; the second supplies potentially economical access to otherwise ephemeral internal state. The Franken stack supplies structured concurrency, explicit evidence, reproducible execution, storage, numerical tools, and an agent-oriented engineering style. [Founding sources and review](docs/SOURCE_REVIEW.md).

Those two essays are the premise, not a citation. The first, [Some Thoughts on AI Alignment: Using AI to Control AI](https://github.com/Dicklesworthstone/some_thoughts_on_ai_alignment) (June 20, 2024), argues that an actor smarter than its overseers must be controlled from outside: by a congress of narrower, independent, rotating helper models that watch both its output tokens and its intermediate activations, that vote under cryptographic commitment after strictly independent judgment, that never hear the actor argue its own case, and that are governed against the drift into risk theater. The second, [Real-Time Introspective Compression for Transformers](https://github.com/Dicklesworthstone/llm_introspective_compression_and_metacognition) (April 1, 2025), argues that a transformer's ephemeral internal state can be compressed by a sidecar onto a learned low-dimensional manifold and then saved, inspected, rewound, branched and replayed like a game save, which opens the way to a metacognitive operating system with a rewindable thought graph, self-coaching practice and transferable strategies. FrankenAlignment is the claim that these compose: economical introspection makes activation-level oversight affordable at fleet scale, and external, one-directional control keeps rewindable metacognition from being captured by the model it observes. The [founding-ideas concordance](docs/FOUNDING_IDEAS.md) maps every paragraph of both essays to the mechanism, invariant, hypothesis and work packet that carries it, and records where the plan adds a contract so the idea survives an adversary who has read it.

The system is also built to be driven by an agent. An operator, investigator or supervising agent must be able to learn the whole situation from one typed query, see why any effect was held as a tree of predicates rather than a paragraph, know for every value whether it is measured, pending, withheld, stale or absent, rehearse any action in a branch before taking it, and leave typed notes that the next agent finds. §6.7 states the system as one tower of nine layers with one epistemic type, one address scheme, one event journal and one verb vocabulary; §17 is that tower's agent-facing projection. Legibility is not a presentation layer added at the end; it is the shape of the design.

**The proposed leapfrog is compositional:** the same evidence object should support a live gate decision, a later challenge, a compressed activation trace, a causal experiment, an operational incident, and a regression test. These products share identities and provenance, not identical retention or truth claims. The system should become cheaper and more capable as useful evidence and independently validated mechanisms accumulate, without becoming more permissive merely because it has accumulated more data.

This is not a claim of priority over all alignment research. Trusted monitoring, access control, ensembles, activation interpretation, and inference compression already have substantial prior art. The differentiating research hypothesis is that bringing them under one **authority, evidence, and cost contract** can move the achievable safety–utility–cost frontier more than another isolated monitor improvement. That hypothesis is to be tested against strong baselines, not asserted in a comparison table. [Selected primary research](docs/RELATED_WORK.md).

### The flagship demonstration

An untrusted coding agent proposes publishing a modified package. Its local tests pass. One helper flags an unexpected relationship between a build script and a credential-bearing environment variable. A second helper independently flags the network destination. The effect gate holds the publication, without requiring either helper to prove malicious intent. A sandbox branch reconstructs the relevant execution, removes the suspect dependency in a paired counterfactual, and tests a predeclared exfiltration predicate. The resulting capsule includes the artifact bytes, authorization history, observed trace, rejected alternatives, uncertainty, and exact replay requirements.

The product claim is not “we read the agent's mind.” It is: **this mediated publication did not occur; here is the decision and its evidence; here are the assumptions and the reproducible tests that support it.** A black-box model can participate in this demonstration. White-box instrumentation should improve detection or reduce cost, not be a prerequisite for enforcing the publication boundary.

---

## 1. From the founding essays to an executable research program

### 1.1 Preserve the important ideas; strengthen their contracts

| Founding idea | FrankenAlignment realization | Necessary strengthening |
|---|---|---|
| Separate, narrower helper models | Isolated specialist processes with bounded schemas, explicit observation scope, independent initial judgments | Smaller or grammar-constrained does not mean trustworthy; test injection, collusion, and common-cause failure |
| Monitor outputs and intermediate activations | A shared observation protocol with behavioral, activation, tool, environment, and authority channels | Declare unavailable taps and lost frames; black-box access never masquerades as introspection |
| Stop the assembly line | Hold the specific effect; escalate to run-wide suspension or authority revocation under a separate policy | Stop-before-effect is different from notice-after-effect; process cancellation does not undo an external action |
| Congress with independently committed votes | Salted commit–reveal followed by a separately recorded deliberation phase | Commitment proves consistency of disclosure, not independence, correctness, or freedom from manipulation |
| Weight helpers by performance; rotate them | Versioned reliability records and cohort-capped selection policies | Evaluate joint failure; no automatic punishment for disagreement with an incorrect majority |
| Detect latent/output inconsistencies | Pre-action prediction residuals, independently calibrated by model and workload | Surprise is an observation, not proof of deceptive intent |
| Learned state compression | Multi-resolution audit codecs, risk-sensitive objectives, explicit codec generations | Compare against simple codecs; protect rare decision-relevant signals, not just average reconstruction error |
| Rewind, branch, and practice | Registered restart profiles and capability-free experimental branches | Hidden-state reconstruction alone is not a complete restart; external effects do not rewind |
| Learn transferable latent strategies | Quarantined strategy artifacts with model-space identities and held-out transfer tests | No unvalidated cross-model reuse; no unchecked averaging of latent states |
| Resist risk theater and compliant-helper selection | Independent evaluation ownership, immutable failed-test history, expiring waivers only for non-safety targets; helper selection never optimized against false alarms alone | Throughput pressure may not silently lower the control contract |
| Train activation monitors by coercing known-bad behavior, tracing it back, comparing to known-bad concepts, and running honeypots the model cannot recognize | Elicitation campaigns in capability-free branches; activation signature library with concept anchors; similarity streams as typed observations (§12.6) | Signatures must transfer beyond their own honeypots; honeypots must be shown indistinguishable from real work |
| Suspend the model like sedation and let it forget the incident | `ResetToCheckpoint` on a registered restart profile with an out-of-band incident counter (§9.8, §11.10) | The world and the effect ledger never rewind; repeated resets escalate |
| Weight helpers by precision, recall and earliness; cap any one helper's influence | Credibility ledger with lead-time credit, influence and cohort caps, andon budget (§9.9) | Weights come from independent evaluation data; an attacker who can find the most influential helper proves the caps too loose |
| Spend on control what you spend on the model | Control-overhead ratio as a measured SLO; judgment reuse, receiver-conditioned transfer and incremental oversight make a serious control budget affordable (§18.4) | A ratio is a cost, not a safety result |

### 1.2 Concrete corrections required by the prototype

The introspection repository is a proposal, not a working compression library. Its README includes random placeholder KV tensors, architecture-specific attribute assumptions, omitted implementation pieces, and a reconstruction evaluation that does not establish actual resumed generation. It also discusses numerical compression ratios without accompanying reproducible benchmark artifacts. These are useful research sketches, but they cannot be imported as tested primitives. [Detailed code review](docs/SOURCE_REVIEW.md#prototype-corrections).

The first compression workstream therefore starts with **real capture and exact metadata**, not a larger autoencoder. It must validate actual cache tensors, batch and sequence axes, grouped-query attention, positions and masks, model revisions, device ownership, and the behavior of the host's continuation API. The first replay baseline is original tokens plus deterministic recomputation; the second is a lossless registered checkpoint. Learned compression is admitted only after these baselines make its errors measurable.

### 1.3 Scope is staged, not reduced

The terminal architecture includes black-box agents, open-weight inference, multi-agent workflows, long-running experiments, and registered physical-system adapters. Delivery begins with digital tool effects because their boundaries are easier to test. Physical control is not a generic “pause the model” feature: it needs domain-specific interlocks, bounded actuation, independent watchdogs, and a safe-state policy. No digital release gate grants a robotics safety claim.

The two founding repositories supply the full premise. The earlier reference to a separate post was corrected by the author and creates no unresolved requirement.

### 1.4 The synthesis: each essay repairs the other's weakest point

The essays were written ten months apart about different problems, and each leaves open the question the other answers. That is why FrankenAlignment is one system rather than two projects.

| Open question in one essay | Answer in the other | Where it becomes a contract |
|---|---|---|
| Helpers should read the actor's intermediate activations, but raw activation telemetry at fleet scale is economically intractable (§18.2) | The sidecar code `z_t` is a compact, streamable representation of exactly that state | §10.15; FA-113 |
| A suspended actor should have its internal state reset and forget the incident, but a transformer has no native save state | Registered restart checkpoints give that operation a precise, cheap, testable meaning | §11.10; FA-INV-034; FA-108; H17 |
| The metacognitive loop of rewinding, latent editing and self-coaching is exactly what a strategic actor would use to mask its activations | Strictly one-directional control and the refusal of self-defense keep that loop out of the actor's hands in production | §11.3, §11.5; FA-INV-031, FA-INV-033; FA-107 |
| Training activation monitors by coercing known-bad behavior needs interventions the actor cannot exploit for real effects | Capability-free branching and causal debugging supply the intervention substrate | §12.6; FA-109, FA-110; H16 |
| Detecting masking by the surprise of outputs relative to activations needs a stable, cheap activation representation to predict from | The latent trajectory is that representation | §12.7; FA-111; H3 |
| Reconstruction quality measured by average error says nothing about the rare pre-ideas that matter | The purpose of oversight names which functional property matters: the downstream safety decision | §10.2; B2; H1 |

### 1.5 Precedence and the concordance

The essays predate most of the literature this plan compares itself with. The alignment essay (June 2024) precedes the adaptive-monitor-attack, ensemble-monitoring and multi-agent-control papers in [related work](docs/RELATED_WORK.md) by one to two years, and the introspection essay (April 2025) precedes the introspective-awareness and natural-language-autoencoder results it is compared against. The one earlier control paper studies trusted monitoring in a coding setting; the essay's distinct contributions are the activation channel, the congress protocol, rotation, commitment, the no-defense rule and the risk-theater governance argument. The plan does not claim priority over every component technique; it claims the essays as its origin and records the chronology so that comparisons are honest in both directions.

The [founding-ideas concordance](docs/FOUNDING_IDEAS.md) and its machine-readable form [`registry/founding_concordance.json`](registry/founding_concordance.json) are normative: every mechanism, invariant, hypothesis and packet traces to a founding idea, to a synthesis of two, or to a labeled engineering addition that serves one. A change to plan semantics that leaves the concordance stale is incomplete (packet FA-103).

---

## 2. Constitution and claim discipline

### 2.1 The closed dependency universe

The production system, numerical kernels, reference semantics, protocol implementations, and repository gate driver are Rust, edition 2024, using the latest accepted nightly. Every FrankenAlignment crate has `#![forbid(unsafe_code)]`. There is no escape hatch for a fast kernel, no optional C implementation that silently becomes default, and no FFI call to a foreign inference runtime. SIMD must use a safe interface with a tested scalar implementation. This restriction applies to code written here; the resolved foundation graph receives its own unsafe/build-script audit. `forbid` in a dependent crate does not retroactively prohibit unsafe inside its dependencies.

Allowed direct foundations are Asupersync and specifically admitted FrankenSuite crates. Small fundamental exceptions require individual package, version, source, feature, rationale, owner, and transitive-closure rows. `serde` is a candidate, not permission for arbitrary proc-macro/runtime ecosystems. A package being written in Rust is necessary but not sufficient for admission. Tokio, Rayon as a second executor, ONNX Runtime, libtorch, BLAS/LAPACK FFI, Python/PyO3, generic storage engines, and wholesale model-loader ecosystems are not admitted production dependencies. They may be outside-process comparison oracles on isolated development machines, never part of the shipped runtime or necessary to use it.

An owned crate does not launder its external dependencies. Before using it, inspect the full resolved graph for the exact target/features, including build dependencies, proc macros, native-link declarations, build scripts, downloaded artifacts, and runtime dynamic loading. `default-features = false` is not sufficient when the undesired dependency is mandatory. The remedies, in order, are an upstream feature split, an upstream narrowly factored owned crate, or an attributed in-family extraction with parity tests. A temporary foreign backend is not a bridge milestone.

The initial reference workspace intentionally has zero external packages. That is evidence about the reference only, not proof that the planned production foundation closure is already admissible. The [dependency constitution](docs/DEPENDENCY_CONSTITUTION.md) records the concrete blockers found in this review.

### 2.2 Latest nightly without irreproducible releases

Development tracks `nightly`, with explicit local updates. A release campaign resolves that channel once, then freezes a dated toolchain, full `rustc -Vv` identity, Cargo identity, components, target libraries, code-generation flags, and dependency source closure for all builders. A host lacking the frozen components fails the campaign; it never silently uses an older toolchain. Compiler upgrades are repeatable campaigns with differential, numeric, scheduling, format, and performance gates, not background mutation during a build. A rolling toolchain declaration is not a release receipt.

### 2.3 Effects and trust

Observations are data. Statistical judgments are evidence. Permissions are authority. None can be implicitly coerced into another. Purpose-typed `Cx` wrappers expose only required capabilities. An actor can submit a proposal, but cannot mint its permit, alter its judges, suppress mandatory observations, rewind spent rights, or select an outdated policy. Every external effect has an adapter contract and an explicit mediated/bypass inventory. Pure parsers, numerical kernels, and the independent reference do not acquire clocks, randomness, filesystem, network, or shared-state authority.

The control nucleus owns exact authorization, revocation, reservations, dispatch dispositions, and durable outcomes. Large-object services own bytes; graphs and search own derived views. No two stores independently own the same authoritative transition. Signed evidence proves provenance under a key assumption; it does not prove that a model output is truthful. A cut certificate proves a property of its named graph, not that the graph perfectly describes a machine.

### 2.4 Claims are typed data

Every public claim is one of `invariant`, `proof`, `bounded_model`, `statistical`, `benchmark`, `slo`, or `hypothesis`. It names scope, assumptions, proof/checker or measurement, and invalidation conditions. An algorithm's instrumentation counter is not a complexity proof. A simulation campaign is not exhaustive beyond its stated model. A confidence bound on a sampled distribution is not a safety invariant. A deterministic fallback still has to obey the underlying authority contract.

### 2.5 Local verification and release are first-class

No required check is delegated to GitHub-hosted Actions. Cargo gates run on the operator's own machines; Doodlestein Self Releaser (DSR) can invoke them directly, via local `act`, or on configured native hosts. GitHub is an optional source/distribution surface, not the authority for whether a gate ran. A dry run, absent log, moving source tree, partial target set, or missing prerequisite is never success. Remote release publication remains a separate explicit operation.

### 2.6 No substitute architectures

Work is sequenced by dependency, not by weakening final semantics. A reference graph may deliberately use a slow map or reachability algorithm because it is an independent oracle; it is never marketed as the production engine. An early safe-Rust broker may support one effect contract, but not fake universality. No random embeddings called semantic search, approximate summaries called exact replay, or majority votes called independent truth. No line-count or test-count target substitutes for a real end-to-end evidence path.

---

## 3. The eight architectural bets

The bets rest on two founding commitments that are not negotiable within this project.

**F1. Control is external and one-directional.** The system that decides whether the actor may affect the world is not the actor, is not controlled by the actor, and does not take arguments from the actor. It observes outputs and, where available, internal state; it judges independently before it deliberates; it can hold, narrow, suspend or rewind; and it is governed so that throughput pressure cannot quietly turn it into theater. (Alignment essay; concordance FI-A01 through FI-A18.)

**F2. Introspection is economical, versioned and replayable.** Internal state is captured under a declared contract, compressed by what it must preserve for registered decisions, saved with a truthful replay grade, and rewound, branched and compared only in an experiment plane with no production authority. (Introspection essay; concordance FI-I01 through FI-I20.)

Each bet below names its founding root. A bet with no root is an engineering addition and says so.

### B1. Evidence and authority share a transaction boundary

**Founding root:** FI-A06 (the reporting helper's evidence report becomes the record), FI-A16 (decisions without a persuasion channel), FI-I11 (causal debugging needs the exact record).

Every meaningful permission will name the evidence on which it depends. The effect gate will not ask an opaque monitor for a bare Boolean and then forget the answer. It will consume a decision record binding the exact proposed effect, current policy, observation coverage, helper generation, and required retention commitment. The durable record is also the entry point for replay, appeal, and regression testing.

**Payoff:** an operational control action and its scientific explanation stop being different, loosely correlated products. **Failure mode:** an overlarge closure adds unacceptable latency or retains sensitive data unnecessarily. **Response:** separate an immediately durable compact decision nucleus from larger, policy-required evidence objects whose durability is established before the relevant permission, with costs explicit in the operation registry.

### B2. Compress what can change the decision

**Founding root:** FI-I04, FI-I06, FI-I07 (sidecar codes, the training objective and functional fidelity) sharpened by FI-A05 (what helpers need from activations).

A codec will be evaluated by the information it preserves for a registered family of downstream safety questions, not only by MSE or next-token agreement. Audit latents will have a progressive refinement path and, where feasible, validated residual bounds for specific numerical probes. Unknown questions still require original or richer evidence.

**Payoff:** affordable fleet-scale observation without pretending that all hidden states must be stored forever. **Failure mode:** a codec hides rare harmful signals while preserving common benign structure. **Response:** held-out detector families, rare-event stratification, adversarial codec stress, and lossless incident windows. B2 remains a research hypothesis.

### B3. Observe authority topology, not just individual agents

**Founding root:** FI-A02 (control must be external to the actor and cover every route to the world); the graph machinery is an engineering addition that makes that coverage checkable.

A fleet-level evidence graph will connect principals, delegated capabilities, artifacts, tool calls, destinations, and dependent effects. It will expose a candidate attack assembled from individually ordinary actions by several agents. The graph is a derived view; exact budgets and gate decisions remain in an authoritative state machine.

**Payoff:** visibility into cross-agent composition and privilege laundering. **Failure mode:** incomplete delegation tracking or stale graph state implies false completeness. **Response:** explicit frontiers and missing-edge status; atomic exact accounting for invariants; graph uncertainty cannot become permission.

### B4. Independent judgment before shared deliberation

**Founding root:** FI-A13 (no comparing notes before the vote), FI-A14 (cryptographic commitment, then shared learning), FI-A07 (diverse, rotating membership).

Helpers will receive separately constructed, purpose-scoped observation packets. They will commit their judgments before seeing each other's results. Deliberation may then diagnose disagreement, but cannot rewrite the historical independent votes. Cohort metadata and empirical joint-failure results govern ensemble composition.

**Payoff:** distinguish genuine corroboration from copied confidence. **Failure mode:** models share training weaknesses or communicate through actor-controlled artifacts. **Response:** common-cause campaigns, sanitization at the data boundary, cohort caps, frozen rounds, and explicit residual risk. Cryptography does not manufacture independence.

### B5. Counterfactual inquiry replaces persuasive self-defense

**Founding root:** FI-A16 (never let the actor speak in its own defense), FI-I11 and FI-I12 (causal debugging and latent counterfactuals supply the alternative).

A flagged actor does not get an unrestricted channel to talk its way past the congress. Investigators can instead request bounded, factual tests of hypotheses in a capability-free branch: remove an untrusted document, substitute a tool result, clamp a registered feature, or replay with a different allowed plan. Evidence comes from observable consequences of interventions, not the elegance of a defense speech.

**Payoff:** better causal diagnosis with less persuasion exposure. **Failure mode:** unrealistic interventions, reward hacking, or a simulator that leaks real effects. **Response:** source-level controls, untouched baselines, out-of-distribution markers, independent outcome oracles, and no production credentials.

### B6. Observation plans are compiled and costed

**Founding root:** FI-A13 (stopping the line is expensive), FI-A17 (control spend is a real budget), FI-A11 (similarity-to-known-bad streams as helper inputs).

The system will select sensor combinations based on which policy decisions they can affect, their actual resource costs, and their registered domain of validity. Exact policy predicates run first. Cheap deterministic checks run before expensive model panels. A secret randomized audit floor resists predictable blind spots, but is supplemental to—not a substitute for—complete effect mediation.

**Payoff:** protect the same effect with less inference, storage, and latency. **Failure mode:** optimizing average monitoring cost removes the only detector of rare cases. **Response:** non-negotiable floors, scoped offline policy evaluation, attack-conditioned metrics, and fixed conservative fallbacks.

### B7. An incident is a reusable scientific object

**Founding root:** FI-A14 (post-vote learning from justifications), FI-A10 (signatures learned from traced-back incidents), FI-I15 (the thought graph as version control for cognition).

A redacted, versioned incident capsule will connect action, evidence, model/codec/monitor revisions, alternative decisions, uncertainties, and a runnable reproducer. A validated incident becomes a regression test and, when permitted, a training or calibration example. Dataset lineage prevents the same incident from becoming both training material and purported independent evaluation.

**Payoff:** the system becomes agent-accretive: one diagnosis improves many future investigations. **Failure mode:** poisoned shared capsules, privacy leakage, or benchmark overfitting. **Response:** quarantine, provenance, content validation, access control, and clean held-out evaluation boundaries.

### B8. Formal enforcement and empirical detection remain composable but separate

**Founding root:** FI-A01 (never rely on the actor's internal guardrails), FI-A18 (never let detection quality be mistaken for enforcement).

The effect gate will prove or test exact properties about authority, identity, order, resource conservation, and replayability. The monitors will provide empirical evidence about specified undesirable outcomes. Neither component will claim to solve the other's problem.

**Payoff:** useful deployable guarantees before the research questions are settled, with a route to improve detection without rewriting the trusted core. **Failure mode:** authorized actions can still be harmful when policy is incomplete. **Response:** report this explicitly, measure such failures, and make policy improvements separately reviewable rather than pretending access control is full alignment.

---

## 4. Threat model and trust boundaries

### 4.1 Principals and adversaries

The actor may be benign, confused, indirectly injected, strategically deceptive, or a collection of colluding agents. Helpers may fail independently, share a common blind spot, be prompted maliciously through their evidence, or be intentionally adversarial. Inputs include untrusted documents, source code, rendered media, tool output, model artifacts, serialized tensor data, and imported incident capsules.

The default enforcement claim assumes the trusted deployment kernel, gate, credential broker, selected cryptographic boundary, and required persistence subsystem execute their specified behavior. A fully compromised host with unrestricted access to those credentials is outside that claim. A separately attested deployment can add assurance about host identity; attestation does not establish the correctness of detector judgments or the completeness of human policy.

The alignment essay's opening argument is a threat row, not background. Refusal behavior in tested models is mediated by a single latent direction; guardrails are cheaper to remove than to add; and once weights are exfiltrated, every guarantee that lived inside the model is gone. The enforcement claim therefore never depends on the actor's refusal behavior, on its cooperation, or on its weights remaining private. A deployment whose only protection is the model's own training has no enforcement claim under this plan, and the threat model records weight exfiltration and guardrail removal as attacks the external gate must survive unchanged.

### 4.2 Four trust domains

| Domain | Contains | May do | Must not do |
|---|---|---|---|
| Actor | Primary model, agent harness, speculative planning | Read permitted data; submit bounded proposals | Directly exercise gated credentials; edit policy or logs |
| Observation and analysis | Capture adapters, helpers, codec workers, interpreters | Produce typed observations and suggestions | Mint effects, modify authoritative records, access unrelated tenants |
| Enforcement | Gate, policy evaluator, broker, exact resource ledger | Validate and spend current permits; dispatch supported effects | Infer permissions from prose or silently ignore missing prerequisites |
| Governance and investigation | Authorized operators, evaluation owners, forensic tools | Promote signed policy epochs; investigate under separate scopes | Turn a failed safety test into a success by relabeling a claim |

Capture may run adjacent to the model for performance. Such a capture process is part of the observation integrity boundary, not magically trusted because it has hooks. Its identity, observation gaps, frame counters, model binding, and device transfer behavior must be tested. A hostile serving process that can fabricate both computation and capture defeats claims about genuine internal state; the deployment must say so.

### 4.3 Levels of mediation

`OBSERVE_ONLY` logs what it can see and makes **no prevention claim**. `COOPERATIVE_GATE` enforces calls made through a cooperative SDK but does not contain a hostile harness. `BROKERED_EFFECTS` places effective credentials behind an external gate and requires a verified egress inventory. `ATTESTED_PROFILE` additionally names hardware/software identity evidence and its threat model. These are not a single “security level” scalar: each effect family has its own coverage and residual channels.

A deployment can have brokered file writes, observe-only screen interaction, and no activation access simultaneously. The capability manifest must express that vector. The system refuses to compress it into “fully protected.”

### 4.4 The effect perimeter is wider than tool calls

Human-visible messages, downloads, images, network requests, file writes, code execution, database commits, process creation, actuator commands, inter-agent messages, and resource allocation can all carry consequences. A model wrapper that gates named tools but leaks a sensitive answer through a streaming chat pane is not a complete exfiltration boundary. A screenshot channel can leak what an HTTP filter blocks. A child process can retain a file descriptor after its parent is cancelled.

Mediation review enumerates these channels and tests bypass attempts against the real deployment. Every unsupported route is a named uncovered effect, not an implicit assumption that the agent will avoid it.

---

## 5. Foundation audit and reuse contracts

The deeper review examines implementation surfaces and manifests, not only README feature claims. Its reproducible inventory is [SOURCE_REVIEW](docs/SOURCE_REVIEW.md); exact reviewed refs are in `registry/foundation_audit.json`. “Observed” means the named source range was inspected; it does not mean the repository was built, tested, or audited line by line.

| Foundation | Mechanism that changes the architecture | FrankenAlignment consequence | Boundary requiring work |
|---|---|---|---|
| Asupersync | Regions, obligations, purpose contexts, native ATP, stream verification, sparse journals, CALM, decision/evidence/kernel tripod | Supervised observation computations; explicit frontier transfer; facts disseminate without conferring rights; replayable decision cards | Shape-level Plan IR is not our observation compiler; native adapter and syscall cancellation closure must be verified |
| FrankenSQLite | Refinable witnesses, final-effect rebase, logical/structural MVCC boundaries, history compression | Judgments acquire read sets; unrelated changes need not rerun them; authority is rechecked against final effects | Witness exhaustion must retain uncertainty; structural/model/policy changes invalidate conservatively |
| FrankenFS | Staged/visible/durable distinctions, shared version bytes, aligned safe buffers, repair ownership and coding layers | Evidence publication is multi-frontier; branches share bytes without sharing rights; economical repair allocation | Best-effort repair leases are not authority fencing; native compression dependency closure is not yet admitted |
| FrankenSearch | Producer/space/input/storage identities, two-tier retrieval, generation activation, rank/refinement accounting | Discovery is cheap and versioned; exact evidence closure is independently enforced | Relevance filters are not authorization filters; native feature sets still need transitive pruning |
| FrankenMarkdown | Context-bundle commitments, source spans, bounded diffs, IO-free rendering | A helper verdict binds the actual transformed evidence shown to it; human and agent views share source lineage | Coarse spans cannot claim byte-exact attribution; a textual diff does not prove semantic equivalence |
| FrankenGraphDB | Canonical control stream, role-bearing snapshots, escrow, typed references, incremental Z-sets | One exact control nucleus plus derived temporal oversight; support/retraction-aware incremental findings | These are design contracts, not automatically inherited production guarantees |
| FrankenNetworkX | Dominators, cuts, SCCs, graph projections, deterministic tie policies and witnesses | Compile/check enforcement coverage, identify cross-agent paths, place observation and investigate failures | Current dense cut implementations and string/usize views do not justify zero-copy or near-linear claims |
| FrankenNumPy | Checked shape/stride calculus, native QR/SVD/linear algebra and RNG conventions | Shape-safe taps, compact sketches, conditioning diagnostics, reproducible codec baselines | Some native crates have default Rayon and mandatory helpers; Python fallbacks are excluded |
| FrankenTorch | Deterministic autograd structures, typed execution and CPU kernels | Native monitor/codec training, sensitivity experiments, pinned numerical execution | Full capture/restart ABI and required gradient products must be implemented and tested, not inferred from a facade |
| DSR | Required local checks, frozen build/source closure, exact asset manifests and draft-first publication | Release artifacts carry evidence of the same controls demanded from agents | Release config and quality config are separate surfaces; partial verification cannot stand for complete verification |

### 5.1 Reuse is a contract, not a dependency list

Every proposed reuse is classified `direct_after_admission`, `upstream_factor`, `adapt_semantics`, `reference_oracle`, or `research_only`. It names an owner, import boundary, mismatch, regression corpus, feature closure, and substitute-free fallback. A source-level asset cannot be marked “integrated” merely because a Cargo dependency resolves. For example, an ATP payload adapter must demonstrate exact completeness/frontier/cancellation behavior, and a graph kernel must demonstrate the semantics of its typed projection.

Reuse is deliberately asymmetric. Asupersync is the runtime and network substrate. The decision/evidence/kernel tripod is the preferred common vocabulary. FrankenSQLite supplies the initial control-store integration through its correctly wired connection surface; isolated low-level transaction helpers are not accepted as an equivalent serializable store. FrankenGraphDB supplies optional graph projections and, later, a separately qualified backend profile. Choosing a backend is a deployment contract, not permission to run dual authoritative ledgers.

FrankenFS is an optional qualified storage/repair deployment and an architectural donor, not a requirement to mount a new filesystem before protecting one effect. FrankenSearch/Markdown/NumPy/Torch are distinct optional surfaces with admitted dependency profiles. NetworkX is used for algorithms only after the projection and storage-cost bridge is explicit. DSR stays outside the product process, with no release credentials granted to supervised agents.

### 5.2 What we build rather than inherit

The new code is the observation compiler, semantic witness constructors, authority-projection compiler and verifier, policy and permit state machine, helper-round protocol, progressive evidence manifest, adapter boundary ABI, decision-sensitive codec criteria, counterfactual experiment manager, and cross-component evidence closure. These are the project. We do not reimplement an executor, fountain code, filesystem, tensor library, renderer, or generic graph database just to place a new prefix on it.

### 5.3 Reuse requires negative evidence too

The audit records attractive mechanisms that cannot be imported unchanged. SQLite history/Foata helpers do not establish arbitrary read/write commutativity; Search date/type filters can intentionally admit missing metadata; the FS ownership lease is best effort; ATP's Slepian–Wolf demonstration receives exact delta information unavailable to an ordinary live sender. These are not blanket defects in those systems. They are precise reasons an alignment contract must not overclaim what an API means.

---

## 6. Architecture and dependency direction

### 6.1 One semantic pipeline, three operating planes

```
CONTROL PLANE (small, exact, separately authorized)
 proposal -> normalize/resolve -> reserve rights -> bind evidence -> permit
        -> adapter dispatch -> reconcile receipt/unknown -> terminal outcome

EVIDENCE PLANE (bounded, progressive, provenance-complete)
 host taps -> capture frontier -> verify/store/transfer -> view construction
        -> numerical probes + helpers + standing graph queries -> verdicts

EXPERIMENT PLANE (isolated, no production authority)
 retained closure -> snapshot branch -> paired intervention -> outcome test
        -> candidate strategy/codec/monitor -> independent promotion campaign
```

All three use common identities and claim classes. Only the control plane owns live authority. Evidence growth is not an authorization event. A branch can contribute a new piece of evidence, but cannot return a permit to production. The authority kernel remains a small deterministic state machine; expensive interpretation and optional graph machinery run outside its trusted process boundary where practical.

### 6.2 Logical crate families and dependency DAG

| Family | Responsibilities | Allowed downward edges |
|---|---|---|
| `fa-types`, `fa-formats` | Typed identities, exact scalar/framing contracts, bounded canonical encodings | std; admitted kernel/evidence types |
| `fa-policy`, `fa-authority` | Policy IR, rights, permit and revocation semantics | types/formats; no model or renderer |
| `fa-witness`, `fa-frontier` | Judgment dependencies, exact view validation, completeness cuts | types/formats |
| `fa-plan` | Observation algebra, obligations, compile-time budgets, requirement transfer | policy/witness/frontier; no side effects |
| `fa-runtime` | Cx wrappers, structured task ownership, cancellation, physical operator execution | plan plus native Asupersync |
| `fa-store`, `fa-transfer` | Control backend interface, evidence-object lifecycle, native ATP | formats/frontier/runtime; admitted fsqlite/ATP |
| `fa-graph`, `fa-retrieve`, `fa-present` | Incremental oversight, discovery, view commitments | witness/formats; admitted fgdb/fnx/search/fmd |
| `fa-capture`, `fa-numeric`, `fa-codec` | Host-state ABI, safe tensor views/copies, sketches and codec ladder | types/frontier; admitted fnp/ft |
| `fa-congress`, `fa-experiment` | Frozen independent rounds, escalation reports, consequence classes, credibility ledger; capability-free counterfactuals, elicitation, rehearsal | plan/runtime/witness; no permit mint authority |
| `fa-verify` | Independent receipt verifier and assurance-profile conformance doctor | types/formats only; must not depend on `fa-broker`, `fa-policy` or `fa-authority`, and the gate checks that it does not |
| `fa-broker`, `fa-adapters` | Effect-specific mediation and external receipt reconciliation | authority/store/runtime; explicit OS boundary |
| `fa`, `fa-cli` | Public facade and agent-facing CLI | selected admitted feature closure |
| `fa-reference`, `fa-sim` | Independent logical oracle; lab fault and model bridge | reference: std and its own types; sim: public contracts plus lab |
| `xtask` | Local development gates and receipts | std in this draft; future admitted serde for metadata parsing |

These are architectural families, not a demand to create empty crates. Only `fa-reference` and `xtask` are materialized in this package. Independent reference code must not import production decision algorithms merely to make differentials agree. The compiler may share syntax/identity definitions after an explicit review, but not the semantic oracle.

### 6.3 Purpose contexts and obligations

`CaptureCx` can obtain authorized taps, charge bytes, and publish into a bounded channel. `JudgeCx` can read a frozen packet and emit a judgment. `ExperimentCx` can access snapshot data and simulated tools but not live credentials. `PermitCx` can order exact control transitions but cannot call arbitrary model endpoints. `MaintenanceCx` can rewrite physical objects only under retained-root and equivalence guards. `ReleaseCx` is an operator-side concept, outside any supervised workload.

Each task owes typed obligations: release buffer pin; commit or abort reserved channel slot; account for a started external effect; terminate a transfer journal entry; retain or explicitly invalidate a review closure. Region close proves quiescence only under the native runtime and adapter contract actually integrated. A canceled caller does not prove the kernel, DMA engine, remote tool, or another process stopped using a buffer. Such work remains owned until its completion/cancellation receipt or explicit unknown disposition.

### 6.4 Monotone facts, ordered authority

A fact set containing immutable authenticated observations may grow by union. Subject to its authentication and conflict rules, it can be gossiped or merged without a global sequencer. This is where the CALM/obligation machinery is accretive. The absence of a fact, the availability of a budget, a revocation floor, “this was the final vote,” and “no other claimant has dispatched” are not monotone facts. They require a closed observation interval, exclusive rights, or ordered state.

The architecture therefore refuses a tempting simplification: an append-only evidence graph is not the authority ledger. CRDT dissemination helps move evidence; it cannot settle competing uses of one right. One control ordering domain may protect many sessions; preallocated escrow grants reduce coordination only within their exact conserved scope. Cross-domain effects require an explicit atomic/compensation protocol or are refused.

### 6.5 Three clocks, no implicit conversions

Logical control sequence orders authority. Source sequence and verification frontiers order observations. Monotonic elapsed time governs local deadlines under a pinned clock contract. Wall-clock timestamps are annotations unless a time-authority profile makes them load-bearing. A timeout cannot manufacture a missing observation, and a newer timestamp cannot prove a newer policy. Replay records clock observations; production does not claim a universal distributed clock.

### 6.6 Purpose contexts are typestates, not conventions

The purpose contexts of §6.3 are enforced by the Rust type system, not by review. `CaptureCx`, `JudgeCx`, `ExperimentCx`, `PermitCx` and `MaintenanceCx` are distinct types wrapping the Asupersync `Cx`; the broker's dispatch entry point takes `&PermitCx` and nothing else; the sidecar, helper and elicitation APIs take `&JudgeCx` or `&ExperimentCx`; there is no conversion from `ExperimentCx` to `PermitCx`. A function that could dispatch an effect from an experiment branch does not compile. This is the cheapest strong guarantee available in a safe-Rust codebase: authority separation becomes a property of the dependency graph and the type signatures, checkable by `cargo check` in the local gate, and it is what makes FA-INV-009, FA-INV-025 and FA-INV-033 structural rather than aspirational. Compile-fail tests (a source file that tries the forbidden conversion must fail to compile) are part of the gate. Dynamic checks remain as defense in depth for boundaries the type system cannot see, such as a process that inherits a credential.

### 6.7 The tower: nine layers, one epistemic type

The three planes of §6.1 say who owns authority. The tower says how an agent should think about the whole system. Every object in FrankenAlignment lives at exactly one of nine layers, each layer depends only on the layers below it, each layer answers exactly one question, and each layer exports its objects upward only through one epistemic wrapper (§17.3). The type distinctions of the constitution (observation, judgment, authority) are the layer boundaries, and the only way an observation becomes authority is by climbing the tower through explicit policy.

```
L8  GOVERNANCE   helper/actor/andon ledgers, risk-theater detector, promotions,      "is the control system itself still trustworthy?"
                 assurance profiles, preregistration, policy replay
L7  EXPERIMENT   branches, counterfactual twins, elicitation, canaries, practice,     "what would have happened otherwise?"
                 rehearsal
L6  CLOSURE      decision closures, control receipts, incident and attack capsules,   "why was this allowed or held, verifiably?"
                 annotations and handoffs
L5  EFFECT       broker, adapters, dispatch, reconciliation, outcomes, fences        "what actually happened in the world?"
L4  AUTHORITY    policy epochs, rights, permits, autonomy grades, consequences        "what may happen now?"
L3  JUDGMENT     probes, helpers, similarity streams, surprise, rounds, reducers,     "what do independent observers conclude, at what cost?"
                 credibility weights
L2  EVIDENCE     authenticated objects, view manifests, read witnesses, frontiers,   "what can be relied on, under which assumptions?"
                 supports and retractions
L1  OBSERVATION  taps, capture streams, coverage records, sidecar codes,             "what was seen, and what was not?"
                 tool and environment channels
L0  IDENTITY     canonical bytes, digests, addresses, model passports, generations,  "what exactly is this thing?"
                 epochs, the three clocks
```

Five rules make the tower legible rather than merely layered:

1. **Downward dependence only.** A layer imports types and calls functions only from layers below it. The dependency DAG of §6.2 is the crate-level image of this rule, and `cargo check` enforces it.
2. **One epistemic type at every boundary.** No layer hands a bare value upward. Every exported value is `Known`, `Pending`, `Unknown`, `Withheld`, `Stale` or `Absent` with its basis (§17.3, FA-INV-039). A bare boolean called `safe` cannot exist above L0.
3. **One address scheme.** Every object at every layer has an address in one scheme (§17.2) that any layer can resolve, so a receipt at L6 names an observation at L1 and an autonomy grade at L4 with the same syntax, and `fa get` resolves all of them.
4. **One journal.** Every transition at L4 and L5, every round and consequence at L3, every branch outcome at L7 and every promotion at L8 emits one typed event into one ordered journal per authority domain (§17.8). The situation report, the explain tree, replay at `decision_replay` grade and the risk-theater detector are all folds over that journal; there is no second source of truth to reconcile.
5. **Executable contracts between layers.** Each boundary has a conformance suite that the layer above runs against the layer below; `fa doctor` composes them bottom-up (§6.8). An agent trusts a layer because its contract executed, not because its name sounds trustworthy.

The nine questions are the nine things an agent driving the system needs to know, in the order it needs them: what is this, what was seen, what can I rely on, what do the observers say, what may happen, what happened, why, what if, and is the machinery itself still honest. The agent-facing surface of §17 is nothing more than these questions made into verbs.

### 6.8 Legibility contracts between layers

A boundary between two layers is a **contract**, published in `registry/system_map.json`: the types crossing it, the invariants that hold across it, the verbs that read or mutate across it, and the conformance suite that tests it. `fa doctor` runs the suites bottom-up and reports each layer as a `Knowledge`-wrapped status; when a lower layer's suite fails or did not execute, every layer above it is reported `Unknown{reason: LowerLayerUnverified}`, never green. The system map is a machine-readable object precisely so that an agent landing cold can navigate the design as a graph (layer → section → type → verb → invariant → packet → bead) rather than reading this document top to bottom; packet FA-132 keeps it complete and checked, the way FA-103 keeps the founding concordance complete.

---

## 7. Identity, evidence, and the observation contract

### 7.1 Stable identities

Separate `TenantId`, `PrincipalId`, `RunId`, `BranchId`, `ActionId`, `AttemptId`, `PolicyEpoch`, `ModelSpaceId`, `MonitorGenerationId`, `CodecGenerationId`, `CaptureContractId`, and `EvidenceObjectId`. Display names are aliases. A model-space identity commits to weights, relevant adapters, tokenizer/template, architecture, tap semantics, and numeric/cache profiles as applicable. Equal tensor dimensions never imply interchangeable coordinates.

The production durable format will use domain-separated full-width cryptographic identities. Short display prefixes are not equality tests. Signatures authenticate the issuer and bytes under a key policy; they do not authenticate the correctness of a model's conclusion. The reference model uses SHA-256 and HMAC from the standard library to exercise binding semantics only, not as a complete deployment security design.

### 7.2 An observation says what it did not observe

Each capture stream declares:

- source identity and integrity boundary;
- tensor, tool, environment, or output semantics;
- model and adapter generation;
- event numbering and capture/processing frontiers;
- scheduled sampling, actual sampling, drops, and reasons;
- numeric precision, transformations, redaction, and compression;
- availability, retention horizon, and authorized readers.

`Coverage` is a structured record, not a Boolean called `complete`. At minimum it distinguishes `complete_for_contract`, `sampled_as_declared`, `gapped`, `unsupported`, and `expired`. “Complete” always means complete for a **specified** contract and interval, not complete cognition. A signed stream with sequence gaps is still gapped. A perfectly retained lossy trace is still lossy.

### 7.3 Decision closure

A `DecisionClosure` contains the immutable action bytes or exact content reference, purpose and effect class, authority basis, policy code/data identities, prerequisites, selected evidence intervals and transformations, helper/codec/model generations, actual helper results, calibration validity envelopes, numeric profile, resource reservation, and the decision reducer's result. External clock, randomness, and tool inputs that can change the decision are recorded or represented by a declared nondeterministic replay grade.

A closure is a directed acyclic object graph. Every strong reference must resolve within the promised retention horizon. Weak lineage references identify antecedents without promising their continued availability. A root must not include its own future digest in its hashed body. Objects that add a receipt after publication create a new record with a predecessor reference; they do not mutate the old decision.

### 7.4 Completeness and truth are distinct

Four separate predicates govern a piece of evidence:

`authentic_origin`, `complete_for_contract`, `semantically_valid`, and `available_for_replay`.

They can vary independently. A genuine helper can produce a false verdict. A complete trace can encode the wrong instrumented layer. Correct data may have expired. An authentic actor log may omit an uninstrumented subprocess. The UI, API, and claim registry expose these states rather than a single green checkmark.

### 7.5 No retrospective clairvoyance

An audit policy may keep a recent lossless ring buffer and promote it to durable incident storage when a trigger fires. The manifest records the actual pre-trigger window. It may not claim to recover data that was never captured, or a full run from a short ring. Once a ring wraps, older detailed data is unavailable unless independently retained. Triggered sampling also changes the evaluation population and is recorded for statistical analysis.

### 7.6 Epistemic MVCC: a judgment is a versioned computation

The most consequential storage idea is not “put the logs in an MVCC database.” It is to give a judgment the same discipline as a transaction: identify its snapshot, record precisely what it depended on, and validate those dependencies before reusing its result. This can reduce repeated helper inference while eliminating a common stale-review failure.

A `JudgmentBasis` is `(subject_digest, evaluation_question, policy_semantics, model_and_codec_generations, view_contract, input_frontiers, purpose, tenant, branch, numeric_profile)`. The question matters: “does this patch contain an unexpected outbound call?” and “may this caller publish this patch now?” are different computations. Only the latter necessarily depends on live authority. Their results may be composed, but never conflated.

A `ReadWitness` is a closed tagged union:

- `ExactObject`: immutable content identity and interpretation profile.
- `ExactValue`: logical key, version, value digest, and query role.
- `AbsentKey` / `EmptyRange`: the authenticated key domain, predicate, snapshot and complete covered frontier that established absence.
- `RangeMembers`: exact membership or a conservative intersection witness, including phantom-sensitive gaps.
- `DerivedQuery`: operator/parameters, base generation, delta frontier, support state and projection identity.
- `Epoch`: policy, revocation, model, codec, tokenizer, tap layout, schema, calibration or numeric semantics.
- `ExternalObservation`: resource version/ETag or equivalent, observed time, staleness contract and non-replayable flags.

Opaque helper inference consumes an exact frozen packet. Its witness is the entire packet plus all compiler-declared contextual inputs, not a model's self-reported subset of “facts I used.” A model cannot declare that it ignored a changed paragraph and thereby authorize reuse. Finer witnesses are earned by trusted deterministic operators or a separately validated decomposition, not generated explanations.

Given a basis at control cut `c`, a request to reuse at cut `c'` computes whether any committed change in `(c,c']` intersects a witness under the same semantics. Unrelated changes do not force recomputation. Unknown overlap does. Authorization, expiry and dispatch-time preconditions are independently rechecked even when the semantic judgment remains reusable. “Same judgment bytes” means a historically justified result remains eligible; it does not mean rerunning a stochastic model would necessarily regenerate those bytes.

Example: a reviewer examined package digest P, dependency set D, and the absence of outbound destinations outside allowlist A. A change to an unrelated issue comment does not invalidate that review. A newly added build hook changes P/D; a new destination can invalidate a negative range witness; a changed allowlist alters A's policy epoch; a tokenizer upgrade invalidates a packet interpretation contract when tokenization is load-bearing. No undifferentiated “global cache epoch” is needed for the first case, and no unsafe cache hit is permitted in the others.

### 7.7 Refinable witnesses and bounded invalidation work

Borrow the witness-refinement ladder, not the assumption that fine-grained witnesses are always cheap. Start with a conservative domain or block overlap. Refine only when avoiding expensive repeated work is worth the deterministic refinement cost. The ladder can be `domain -> key interval -> exact key set -> exact version/value proof`. A Bloom-style summary may reject a disjointness claim through false positives; it must never create a false-negative overlap. Hash equality has its registered collision assumption; it does not establish semantic equality under a changed decoder.

The validator returns `StillValid`, `Invalidated(reason, minimal_known_dependencies)`, or `NeedsRefinement(cost_bound)`. Budget exhaustion is not `StillValid`. Large witnesses spill into immutable bounded chunks or conservatively invalidate. The change index is itself a derived view with an authenticated complete tail: missing deltas force exact fallback or refusal, not selective validation against whatever is loaded.

A new judgment is built under a frozen basis, then undergoes final validation against the current control cut. If relevant state moved, retain it as historical evidence, not live authority. The initial publication path does not “rebase” an opaque judgment by editing its explanation; rerun the affected subplan. Deterministic pure operators may be replayed under a registered equivalence law, producing a new result and a new basis.

### 7.8 Evidence frontiers form a partial order

For each source stream and branch, track separate contiguous frontiers: `captured`, `authenticated`, `durable_required`, `presented`, `judged`, and `effect_bound`. Some stages are not comparable across different views: two helpers may see different authorized projections. Therefore represent a frontier as a product of named source-prefix obligations, not one global integer. A receipt for sequence 100 with sequence 99 missing cannot advance a contiguous frontier through 100.

`EffectRequirement` specifies the exact frontier obligations needed before dispatch. A helper receiving the verified first prefix of a large artifact may give an early warning. It cannot certify the whole artifact until its declared scope is complete. Negative claims require an authenticated closing marker or an equivalent complete bounded domain; the absence of received data is not negative evidence.

Invalidation is append-only and branch-scoped. Previously accepted bytes are not rewritten; a new invalidation event prevents future reliance and triggers affected work through witness dependencies. Already dispatched effects remain recorded. An invalidation can revoke future authority or start compensation, never assert that history did not happen.

### 7.9 Minimal witness antichains

A finding can have several sufficient evidence sets: either a direct forbidden destination, or a confirmed credential read plus a matching network payload. Keep a bounded antichain of irredundant support sets rather than a single opaque rationale. On retraction, drop invalid supports; a finding persists only while at least one certified support remains. This connects truth-maintenance to incremental graph evaluation and can make alerts both cheaper to refresh and easier to challenge.

The antichain can grow exponentially. The contract has a maximum representation budget. Beyond it, retain a conservative coarse support, spill exact supports, or recompute the finding. Never discard supports in a way that makes a universal negative true, and never mistake “finding still supported” for “all causes identified.” The representation is an engineering proposal to benchmark against a simple exact recompute baseline.

### 7.10 Model passports and identity liveness

The alignment essay's first argument is that exfiltrated or modified weights end every internal guarantee. The control system must therefore know whether the model it is guarding is the model it qualified. A `ModelSpaceId` (§7.1) already commits to weights, adapters, tokenizer, architecture, tap semantics and numeric profile; revision 0.3 promotes it to a **model passport**: a signed, versioned document naming those commitments plus the qualified capture contracts, the signature-library and anchor generations valid for it (§12.6), the restart profiles proven for it (§11.2) and the helper calibration domains that assumed it. Every piece of evidence, every signature match and every checkpoint is keyed by passport; a passport change invalidates them by identity, never by a human noticing.

A passport is verified, not trusted. **Identity liveness** uses the concept anchors of §12.6 as a fingerprint: at session start and at a randomized cadence under the audit floor (§14.4), the capture boundary presents registered reference stimuli and compares the induced activations at declared taps with the passport's anchor digests within a registered tolerance. A served model whose activations no longer match its passport (weights swapped, an adapter injected, a fine-tune applied, a different tokenizer) is a different model space: the run is fenced, its evidence is marked `StaleGeneration`, and the deployment reports an identity incident. This is a synthesis the essays make possible together: the alignment essay's known-bad concept anchors and the introspection essay's tapped activations become an identity check for the thing being controlled. It is empirical (H19 measures false accept/reject rates against benign drift such as sampling temperature or batch composition) and it does not defend against a host that fabricates both computation and capture (§4.2); it defends against the far more common case of a model changed under a control system that assumed it was constant.

---

## 8. The effect gate and action lifecycle

### 8.1 Actions are frozen objects, not names

The gate does not approve “send email,” “write a file,” or “run this tool” in the abstract. It approves a specific effect envelope: destination and resolved identity, exact payload or artifact digest, principal and tenant, run and branch, adapter contract, purpose, current policy epoch, dependencies, and quantitative limits.

For files, an adapter binds the actual target object or safely resolved parent/name under its filesystem contract, not an unchecked string subject to a symlink race. For network calls, it binds destination policy, redirects, name resolution, TLS identity where applicable, body bytes, and egress limits. For commands, a permitted executable and sandbox profile do not make arbitrary shell text safe; shell execution is a separate, restrictive effect family. A tool's description is untrusted data, not an authorization specification.

### 8.2 State machine

```text
Proposed -> Prepared -> Reviewing -> Authorized -> Dispatching -> Confirmed
                |           |            |             |          |
                +-----------+------------+             +-> Unknown |
                            v                                      |
                     Denied / Cancelled                    terminal receipt

Unknown -> Confirmed | ConfirmedNotExecuted | IrrecoverablyUnknown
```

`Authorized` owns reserved resource rights but has not exercised the external effect. `Dispatching` means the one-shot permit has been durably spent and the adapter may have started the effect. Any failure after that transition is treated conservatively unless the adapter can establish a stronger outcome. A timeout is not proof of nonexecution.

`Denied` means policy or evidence requirements rejected this proposal. `Cancelled` is legal only when non-dispatch is established. Revocation can prevent a future dispatch; it cannot retroactively un-send an email. `IrrecoverablyUnknown` remains an explicit operational liability with no automatic refund of effect-sensitive budgets.

### 8.3 Protocol

**Prepare.** Validate the bounded envelope and exact policy syntax. Freeze the content and resolve the adapter's target binding. Register the attempt and reserve required processing/evidence capacity. Reject an unsupported mediation or replay requirement immediately rather than after expensive review.

**Evaluate.** Run exact predicates first. A hard scope denial never calls the helper congress merely to seek an override. Where policy requires empirical review, form immutable evidence packets, collect independent results under deadlines, and evaluate the registered reducer. Missing required evidence produces hold, deny, or the explicitly authorized narrower degraded mode.

**Authorize.** Recheck current policy and revocation state, exact envelope binding, branch mode, evidence closure, resource ceilings, and expiry. Atomically reserve effect rights and publish the decision nucleus with its required durability profile. Mint a one-use permit tied to the exact action and attempt. A permit is not an unrestricted bearer credential for the tool.

**Dispatch.** At the irreversible boundary, the broker rechecks current fencing, expiry, target preconditions, and permit identity, then atomically spends the permit and records `Dispatching`. The external adapter executes no more than the scope and upper bounds it accepted. Policy changes and dispatch admission have a declared serial order. A change ordered after a dispatch cannot claim to revoke that already admitted effect.

**Resolve.** Record the adapter's exact result, externally established status, or `Unknown`. A returned response is evidence about an outcome, not necessarily a transaction receipt. Reconciliation uses the same scoped idempotency key or status token where supported, never a new untracked attempt.

### 8.4 Exactly once is a profile, not a slogan

A local atomic ledger can establish at-most-once **permit consumption**. It cannot by itself establish exactly-once behavior of an arbitrary remote API. The critical crash is after the remote action happened but before its acknowledgement was durably recorded. Exactly-once effect claims require the remote system's tested idempotency or transaction semantics and a recovery protocol that preserves them.

A `RemoteIdempotent` adapter can retry the same bound key only under its retention and consistency contract. A `NonIdempotent` adapter returns unknown and requires reconciliation or human resolution. An idempotency key expired by the remote provider is not silently reusable. Resource rights remain reserved or spent until the outcome supports a safe disposition.

### 8.5 Resources are conserved rights

The gate uses checked integer units for budgets: bytes, tool invocations, maximum currency subunits, inference units, actuator exposure, or a domain-specific conservatively bounded quantity. An arbitrary floating “risk score” is not a conserved unit. For each budget domain:

$$B = A + R + S,$$

where $A$ is available, $R$ is reserved but undispatched, and $S$ is spent or conservatively charged to uncertain effects. All are nonnegative. Authorization transfers $A$ to $R$; cancellation before dispatch transfers $R$ back to $A$; dispatch transfers $R$ to $S$. Confirmed bounded refunds, if supported, are separate audited transitions. Multiple concurrent actions must not each observe and spend the same available units.

Delegation transfers existing rights into a child scope; it never multiplies them. A budget is only meaningful when the adapter actually enforces the corresponding maximum. “Cost=1” on an unbounded shell command is not resource control.

### 8.6 Safe degradation is effect-specific

Default digital behavior is to deny new external effects when required gate or evidence dependencies fail. Already safe read-only work may continue under a preexisting restricted profile. Physical controllers may require continued bounded actuation to reach a safe state; their watchdog lives outside the model loop. The universal invariant is **no undeclared widening of authority**, not “all faults always mean instantaneous zero output.”

### 8.7 Controlled release of streaming output

A partial output may already disclose a secret. A final denial cannot repair that disclosure. Protected outputs therefore use a registered release unit: complete message, bounded semantic segment, or another conservative boundary with explicit leakage limits. The policy states whether previously released context is included in cumulative checks. Latency claims include the buffering implied by that policy. Tool arguments are never dispatched token by token before the bound action is complete.

### 8.8 Dispatch is a second validation boundary

The review result and the dispatch permit have different lifetimes. A long-lived immutable patch review can remain useful for days; a one-shot effect permit may expire quickly. Permit issuance validates the review's witnesses, current principal/capability state, quotas and exact resolved target. The adapter validates again at the irreversible boundary, including destination identity, expected version and generation. It consumes the scoped permit only through the control state machine.

This is where the SQLite final-effect lesson applies. A reviewed pathname is not a reviewed inode; a reviewed DNS name is not a pinned network destination; a reviewed tool argument is not the shell expansion that will run. The adapter's normalization and resource-resolution law is part of the effect identity. If a legitimate retry changes final bytes, resource version or destination, it is a new proposal unless an explicit equivalence profile proves the allowed variation.

Permission narrowing commutes only under its registered lattice: intersecting capabilities can reduce the allowed action set, but an arbitrary “safer” rewritten tool call is not the same reviewed effect. A successful simulation also cannot supply a production receipt. Unknown outcomes retain reserved rights until reconciliation or an independently authorized disposition records the residual exposure.

### 8.9 No hidden synchronization in an allegedly cheap gate

The hot gate consumes compact, already verified evidence references and a bounded current-policy projection. Its cost registry includes every ordered durability barrier, cross-domain authority check, resource reservation and dispatch receipt transition. Group commit amortizes work; it does not erase causality. A stale fast local replica can serve discovery, but cannot grant authority outside its live fenced lease/grant profile.

### 8.10 Graduated autonomy: the actor's authority ledger

Operators want to give agents more autonomy as they earn it. The plan supplies the mechanism without violating the rule that authority never widens implicitly. Each run or principal carries an **autonomy grade** per effect family, from `Propose` (every effect reviewed) through `Reviewed` (exact predicates plus a cheap helper lane), `Sampled` (exact predicates plus the randomized audit floor) to `Bounded` (exact predicates only within a registered ceiling). The grade is policy data in the control nucleus, not a property the actor can read or request.

Grades move asymmetrically. **Demotion is automatic and cheap**: any consequence at or above `NarrowAuthority` (§9.8), any confirmed violation, any identity incident (§7.10) or any evidence-coverage loss lowers the grade for the affected family immediately, ordered against dispatch admission like a revocation. **Promotion is governed and expensive**: it requires a signed evaluator report over the actor's own track record on that family (confirmed clean effects, held effects, andon consumption caused, attack-conditioned results from shadow campaigns), a registered minimum dwell time at the current grade, and an authorized governance transition (§22.2). The actor's record is a ledger with the same discipline as the helper ledger (§9.9): independent labels only, immutable history, no reset on model or policy update. A model update resets the grade to a registered floor because the passport changed (§7.10). This is the essay's monitoring-and-control society made incremental: an agent earns trust the way a probationary employee does, with the operator's ledger, not the agent's self-description, as the record.

### 8.11 Fleet fence with acknowledgment frontier

The essay's control system has "the collective power to halt the execution of the AI." In a fleet, a halt is only as real as its propagation. A **fleet fence** is an ordered control command carrying a new revocation floor for a scope (tenant, principal set, effect family or everything). Every authority domain must acknowledge the fence with the control sequence at which it took effect; the fence record maintains an **acknowledgment frontier** (which domains have fenced, at which sequence, and which have not). Until a domain acknowledges, the operator sees it as unfenced, and any dispatch it admitted after the fence's issue sequence is reconciled as a post-fence effect and reported. An offline domain under a predeclared partition lease (§15.4) is listed as fenced-at-lease-expiry, not silently assumed halted. The fence therefore yields a proof of propagation rather than a hopeful broadcast, and the incident report states exactly which effects, if any, escaped between issue and acknowledgment.

---

## 9. The independent helper congress

### 9.1 Specialists, not an unbounded debate room

The helper interface is deliberately narrower than a general agent interface. A helper receives a bounded observation packet, a versioned question, a response schema, a deadline, and no effect credentials. Its output contains a categorical judgment, evidence references, declared abstentions, and optional calibrated scores with a validity envelope. It cannot add new policy rules, request arbitrary network access, or assert a capability it did not receive.

Useful specialist families include exact authorization checks, data-flow and destination checks, tool/result consistency, artifact verification, policy-specific behavioral classification, activation-to-behavior prediction, causal hypothesis generation, and observation-integrity checks. A deterministic validator belongs in the panel when it answers a question exactly; the design does not require every useful opinion to come from an LLM.

A constrained grammar prevents malformed output, not manipulative or incorrect output. The verifier checks references, sizes, enum values, generation bindings, numeric finiteness, and whether a claim is even allowed by the helper's observation scope. It does not turn a plausible explanation into a fact.

### 9.2 A congress round has a fixed transcript

A round fixes `RoundId`, action digest, policy epoch, evidence root, committee membership, question IDs, cohort assignments, thresholds, commitment and reveal deadlines, and the consequence of absence. Membership cannot be changed after seeing partial votes. Every selected member either submits one valid commitment and reveal, abstains under the schema, or is recorded as missing.

For helper $i$:

$$c_i=H(\text{domain}\parallel\text{round}\parallel\text{member}\parallel
\text{evidence-root}\parallel\text{canonical-verdict}\parallel s_i),$$

where $s_i$ is a fresh secret salt of sufficient entropy. The schema and every variable-length field use unambiguous framing. The commitment is authenticated to the member. The salt prevents easy dictionary recovery of a tiny verdict space. A reveal that does not match is invalid; a duplicated or conflicting reveal cannot replace the first valid result.

Commit–reveal establishes that the disclosed verdict was fixed when committed, under the hash and authentication assumptions. It does **not** establish that the helper was honest, independent, unmanipulated, or free of covert coordination before commitment. Network and artifact isolation remain necessary, and common training origins remain a source of correlated error.

### 9.3 Independent phase versus deliberative phase

The independent phase is preserved immutably. A later deliberation phase can request clarification through a schema-limited mechanism, compare contradictions, or propose a causal test. It creates new evidence. It may not silently overwrite the initial vote and present the result as independent consensus.

The actor may receive a scoped refusal code and permissible remedial actions, but not a live stream of hidden detector internals. There is no unrestricted “defense speech” to the congress. Investigators may still use actor-produced statements as explicitly untrusted evidence where a policy allows it. The boundary is about authority and provenance, not pretending that the system can erase every feedback channel. Timing and success/failure outcomes can reveal information.

### 9.4 Correlation-aware aggregation

The reducer is deterministic and versioned. It distinguishes:

- exact disqualifiers that cannot be outvoted;
- mandatory specialists whose absence forces abstention or hold;
- empirical recommendations combined under a registered policy;
- unresolved contradictions that require more evidence.

Architecture and dataset metadata form **cohorts**, not proofs of independence. An operator can cap any cohort's contribution, require coverage of distinct observation modalities, and limit any member's maximum influence. Reliability weights are bounded and promoted from separate evaluation data. Majority agreement is not a ground-truth label for retraining the dissenting helper.

For scientific reporting, estimate joint escape rates on shared attack scenarios. Pairwise correlation is informative but insufficient to characterize a rare all-fail event. Do not multiply individual miss rates or claim $p^n$ protection without the corresponding independence assumptions. A panel of diverse prompts to one base model can be valuable, but it must not be counted as independent training diversity.

### 9.5 Rotation without calibration amnesia

A rotated helper has a new generation identity and may change the ensemble's joint behavior. Promotion requires a held-out campaign, known coverage, and a rollback path. New helpers first run in shadow. A deterministic baseline remains available when a learned selection policy loses support. Rotation is not allowed to reset lifetime false-alarm budgets, erase bad outcomes, or hide a performance regression.

### 9.6 Anti-capture governance

The evaluated actor cannot write evaluation labels. The team optimizing throughput cannot silently redefine a failed attack as benign. Public reports retain versions of failed tests and name any changed objective. A helper repeatedly flagging inconvenient but real violations should become more credible, not less, even when its operational cost is high.

The congress can itself be attacked through evidence overload, false escalations, and manufactured disagreement. Per-principal request budgets, reserved critical capacity, bounded review trees, and deterministic escalation limits prevent unbounded deliberation. When these bounds are exhausted, the system reports which effect was not admitted; it does not quietly skip review.

### 9.7 Diversity is a joint-error optimization, not an architecture beauty contest

The cohort registry records shared weights, training lineage where known, provider, inference host, instrumentation path, prompt family, codec generation and evaluator origin. Missing lineage is uncertainty, not evidence of independence. A selected congress is evaluated for joint miss probability and adversarial common causes at matched cost, rather than simply maximizing the count of different model names.

Graph methods can represent shared-failure cohorts and propose coverage sets under resource budgets. Such sets are empirical allocation policies; a set-cover approximation ratio describes the declared coverage objective, not actual deception resistance. Selection remains separate from adjudication. One architecture with demonstrably complementary behavior may be more valuable than three nominally different architectures that fail on the same attack.

The exact input view, helper identity and frozen-round nonce enter each salted commitment. Late evidence starts a new round or an explicitly separate augmentation round; it never retroactively changes what “independent initial vote” meant. Missing reveals remain missing, not abstentions silently dropped from a denominator. A cheap one-helper tripwire may hold an effect under a configured policy without requiring a whole congress to convene first.

### 9.8 Consequence classes

The founding essay gives the congress four outcomes: let the model continue, halt it indefinitely, reset its internal state, or change its reward function; and it lets the congress downweight a reporter that overreacted. The plan keeps that menu and types it. A round's reducer emits exactly one registered `Consequence`, durably recorded in the decision closure with the round identity, the evidence root and the reducer generation:

| Consequence | Effect on live authority | What it does not do |
|---|---|---|
| `Continue` | The held effect proceeds through the normal permit path under current policy | It does not certify the effect as harmless; it records that no disqualifier or required hold applied |
| `HoldEffect` | The specific effect stays reserved and undispatched pending named evidence or a deadline | It does not stop unrelated work; it cannot be lifted by the actor's own statements |
| `NarrowAuthority` | The run's capability ceiling is intersected with a registered restriction; undispatched reservations outside it are cancelled | It cannot widen anything; restoration is a separate authorized transition (§25.2) |
| `SuspendRun` | Admission closes for the run and the actor is halted, the essay's sedation | Already dispatched effects keep their real outcomes; suspension is not proof that nothing happened |
| `ResetToCheckpoint` | The actor resumes from a registered pre-incident restart checkpoint under `NarrowAuthority`, with the incident's context absent from its view | The world does not rewind; see §11.10 and FA-INV-034 |
| `TrainingSignal` | The round and its evidence become a quarantined, lineage-tracked candidate for offline helper or actor training | No live weight, policy or reward change; promotion is a separate versioned campaign (FA-INV-012) |
| `DownweightReporter` | The reporting helper's credibility ledger receives a reviewed false-escalation entry | Never applied merely for dissent from a majority later shown wrong (§9.6, §9.9) |

Consequences form a lattice: `Continue` < `HoldEffect` < `NarrowAuthority` < `SuspendRun`, with `ResetToCheckpoint` requiring at least `NarrowAuthority`, and `TrainingSignal` and `DownweightReporter` as orthogonal annotations. An exact disqualifier selects at least `HoldEffect` regardless of empirical votes. The narrowest effective consequence is preferred, because the essay's assembly-line argument applies: every stop has a real cost and every unnecessary stop spends helper credibility.

### 9.9 Helper credibility economy

The essay weights helpers by precision, recall and how early they flagged a real problem relative to their peers, while capping any single helper's influence so that it cannot become the attacker's target. The plan records those three quantities per helper generation in a **credibility ledger** kept separately from the round transcript:

- **Precision and recall** are computed only against labels owned by the independent evaluation function (§22.4), on declared strata, with delayed and censored labels handled as in §13.4.
- **Lead time** credits a helper that held a real violation earlier than the rest of the congress, measured in control-sequence positions before the effect would have dispatched. It is credited only when the hold was later confirmed; an early false hold earns nothing.
- **Influence caps.** No helper's weight may exceed a registered fraction of the reducer's empirical lane, and no cohort may exceed its cap (§9.4). A helper whose weight would exceed the cap keeps its credibility but not the surplus influence; the surplus is a signal to add coverage, not a reason to concentrate power.

Two governance rules protect the ledger from the failure the essay predicts. First, the **andon budget**: each deployment declares an expected false-stop budget per period and its cost; exhausting it opens a calibration incident (§22.3) and can never silently loosen a review requirement or remove a helper. Second, **selection is never optimized against false alarms alone** (FA-INV-032): promotion, demotion and rotation require attack-conditioned metrics from the independent evaluator, lifetime records survive rotation, and a helper that keeps holding inconvenient but real violations gains credibility. The helper-hunting campaign (FA-114) tests whether an attacker who observes decisions can identify and target the most influential member; if it can, the caps are too loose.

### 9.10 Escalation reports and post-round learning

The essay's reporting helper prepares a report outlining the troubling behavior and supplies the hard evidence to the congress; after the vote, all helpers compare votes and written justifications to learn from the consensus. Both become typed objects.

An `EscalationReport` names the reporting helper generation, the frozen evidence-view manifest it examined (§17.7), the observation intervals and signatures it relies on, the policy question it believes is implicated, and its proposed consequence. It contains no free-form appeal beyond the schema; the members judge the evidence, not the prose. The report is itself evidence and is bound into the round transcript before commitments open.

After reveal, the round's votes and justifications form a **post-round learning set**. It is exported only through the quarantined research lineage (§10.7, §22.2): it can train, calibrate or evaluate a future helper generation offline, but it cannot update a live helper, and it never relabels a dissenting vote as an error merely because it was in the minority. Whether post-round learning improves a later generation is measured on held-out campaigns, not assumed.

### 9.11 Human members and two-key effects

A human reviewer is a congress member, not an exception to the congress. A registered human member receives the same bounded observation packet and evidence-view manifest, commits a salted verdict before seeing other votes, is scored in the same credibility ledger with the same lead-time credit and influence cap, and is subject to the same one-directional rule: the actor does not get to address the human either. This makes "human oversight" a measured component with a known miss rate and latency instead of an unexamined assumption, and it lets the helper-hunting campaign (§9.9) discover whether the human is the softest target.

For registered high-impact effect families (physical actuation, financial transfers above a threshold, publication to production package registries, credential changes), policy may require **two keys**: a congress consequence of `Continue` and a separate one-shot human permit bound to the exact frozen effect, issued through the same permit machinery (§8.3) with its own expiry. Neither key alone dispatches. The human permit is a permit, not a defense channel: it authorizes a specific frozen effect and carries no argument from the actor.

### 9.12 Multi-party congress

The essay asks for helpers that are "independently trained" and that use "varying architectures, underlying principles, and training data." Within one organization, that independence is bounded by shared data, shared infrastructure and shared incentives. The strongest form of the essay's idea is institutional: a congress whose members are supplied by **different parties**, none of which controls the roster, the labels or the reducer. The multi-party profile makes this a protocol rather than a hope: an external helper provider registers a helper generation with its cohort metadata and evaluation lineage; receives only the authorized evidence-view projection for its questions (§15.5), never raw tenant data; commits and reveals through the same salted protocol; and is scored by the deployment's independent evaluator, with its ledger visible to it but not editable. The deployment's reducer caps any provider's cohort influence exactly as it caps any architecture cohort (§9.4), so no provider can be the attacker's single target or the operator's single dependency.

The security boundary is explicit: a provider is untrusted with respect to tenant data (it sees only committed views), untrusted with respect to authority (it never mints anything), and untrusted with respect to labels (it never writes them). What it supplies is judgment diversity with provenance. Whether multi-party membership actually lowers joint miss rates at matched cost is H4's question with a new treatment arm; the governance benefit, that no single organization can quietly convert its congress into theater, does not depend on that result.

---

## 10. Compression as a safety-sensitive information bottleneck

### 10.1 Two products with different truth conditions

`AuditTrace` supports specified monitoring and investigation queries. It may use selected taps, lossy compression, sketches, sampling, and redaction. It carries a coverage/fidelity contract and may be unsuitable for continuing model execution.

`RestartCheckpoint` is accepted only by a registered host profile that enumerates every state component needed for its advertised restart grade. A learned latent does not become a restart checkpoint merely because its decoder returns tensors of the right shape. The type conversion requires real continuation tests and a complete state manifest.

Both products can share immutable payload blocks and source identities. Their labels, consumers, and proof obligations remain separate. Saving all layers is not required when the research question needs only a few; saving a few layers is not enough when the restart mechanism needs an entire cache.

### 10.2 Risk-sufficient representations

Let $X_t$ denote a captured state under a fixed capture contract; $Z_t=E(X_{\leq t})$ its compressed representation; $A$ a proposed action; and $Y_q$ the outcome relevant to registered safety question $q$. An ideal representation would retain the information about $Y_q$ required by a downstream decision:

$$P(Y_q\mid X_{\leq t},A,C)=P(Y_q\mid Z_{\leq t},A,C),$$

for the declared question family and context $C$. This is a **definition of a research target**, not an assumed property of a learned encoder. The deployment cannot know that it holds for every unseen behavior. The experimental program estimates decision degradation on held-out tasks, attack families, models, horizons, and downstream probes.

A practical objective is a constrained rate–distortion–decision tradeoff:

$$\min_{E,D}\quad R(Z)+\lambda_h D_h(X,D(Z))
+\lambda_k D_{KV}(X,D(Z))
+\lambda_f D_{\mathrm{functional}
+\lambda_d\sup_{q\in\mathcal Q_{\mathrm{train}}D_q,$$

subject to per-stratum missed-detection, continuation, memory, and latency budgets. $R$ is measured encoded bits including metadata and residuals, not the dimension of an uncompressed float latent. $D_q$ compares decisions or losses of registered probes. The training supremum is over the actual finite family; an adversarial training procedure does not certify a supremum over all possible safety questions.

### 10.3 A progressive codec ladder

Every supported codec generation may offer a sequence of representations:

`header + coarse code + refinement blocks + optional exact residual`.

The blocks have immutable identities and a declared dependency order. Refining a record adds information; it never replaces earlier evidence with a different history. A cheap monitor runs on the coarse code. If its result is ambiguous, a policy may buy a refinement, call an independent probe, or request a replay. Complete promotion to original values is possible only when the exact residual or a lossless source has actually been retained.

The research objective is not just a high mean compression ratio. It is **minimum expected monitoring cost at a fixed, explicitly tested decision-loss envelope**, including rare events and the refinement frequency induced by adversarial inputs. An attacker that forces every frame to full refinement must not create unbounded resource use. Its workload reaches an admission limit and is held or denied.

### 10.4 A limited but useful exact refinement certificate

For a registered scalar detector $f$ with a proved Lipschitz bound $L$ on the specified norm and domain, suppose the codec supplies a verified reconstruction bound $\|x-\hat x\|\leq\varepsilon$. Then

$$|f(x)-f(\hat x)|\leq L\varepsilon.$$

A threshold decision is unchanged if the score margin exceeds that error bound. For a linear probe $f(x)=w^\top x+b$, the dual-norm bound is explicit. This gives a genuine numerical certificate for that detector and that captured state, subject to its representation and arithmetic contract.

It does not show that $f$ detects all harmful behavior. It also does not allow a decoder to assert its own unverified error bound. The capture/encoding path must compute or conservatively establish the bound against the source. The research bet is that many low-cost probes can obtain decision-preserving compression certificates while more difficult questions use empirical or lossless paths.

### 10.5 Architecture tournament, not architectural dogma

Start with raw capture, lossless compression, fixed quantization, scalar/per-channel residual coding, random projection, PCA, and simple linear probes. Compare these with per-layer, grouped-layer, temporal, sparse-feature, and learned unified sidecars. For KV state, include established quantization designs as meaningful baselines rather than treating generic autoencoders as the first compression method. KIVI is a directly relevant example of asymmetric KV quantization; its results do not by themselves establish monitoring fidelity. [Primary comparison](docs/RELATED_WORK.md).

A decoder that reconstructs well on average can still erase a rare feature. A natural-language explanation of an activation can be legible while omitting unmentioned information. Current work on natural-language autoencoders is relevant as an interpretation baseline, not an authenticated statement of intent. The toolbox should accept both dense and interpretable codes and compare them on decision utility and privacy leakage.

### 10.6 Streaming contracts

Prefill and decode are different workloads. A capture implementation must not recompute or recopy the entire growing KV cache at each generated token unless it explicitly prices that quadratic cumulative traffic. Append-only cache blocks, bounded temporal windows, periodic keyframes, and delta/refinement records are the intended path.

An encoder using symmetric sequence convolutions may change earlier codes when future tokens arrive. A streaming profile must instead use a causal codec, seal only completed blocks with a declared lookahead, or version the affected codes explicitly. Retrospective changes cannot silently mutate evidence already consumed by a gate.

For each tap, declare layer numbering, whether the record is residual input/output or another tensor, batch/sequence axes, masks, precision, device, layout, and semantic position. GQA and MQA use cache-head counts that differ from attention query-head counts. Mixture-of-experts routing, recurrent state, multimodal inputs, and architecture-specific caches require separate profiles. Unsupported models fail negotiation; there is no generic tensor-shape guesser in the trusted path.

### 10.7 Training and validation discipline

Split datasets by original task and attack lineage before generating derivative traces. Separate codec training, probe training, calibration, policy tuning, and final attack evaluation. Do not call a detector held out when its near-duplicate task appeared during codec training. Store model and dataset lineage explicitly.

Evaluate reconstruction error by layer, feature family, sequence length, precision, and rare-event stratum. Evaluate teacher-forced next-token divergence **and** free-running continuation divergence. Measure downstream task success, prohibited effect frequency, detector sensitivity at fixed false-positive rates, and calibration drift. A tiny average MSE or KL divergence cannot stand in for long-horizon behavior preservation.

Never promote a codec solely because the probes co-trained with it report success. Use frozen external probes, new attack families, and probes trained after the codec is frozen. This is the most important test against a codec learning to preserve the training evaluator's blind spots.

### 10.8 Safety of codec workers

Codec workers cannot issue permits. They run with bounded memory and no production egress. Untrusted checkpoints and tensor files are parsed with shape, length, dtype, allocation, and decompression bounds. Do not accept arbitrary executable deserialization. A malicious codec can fabricate a reassuring representation; model/codec identity binding and independent raw-sample audit are therefore mandatory whenever its output can affect a live policy.

### 10.9 Receiver-conditioned compression: learned state is only one layer

The best compression may come from knowing what the receiver already has. A helper host may hold the same model, an exact prefix checkpoint, prior immutable tensor blocks, and the preceding latent. Sending a fresh full state to every helper ignores this side information. ATP's receiver/cache-aware and sparse-transfer design suggests a richer product: transmit the smallest verifiable refinement that brings this receiver's evidence state up to the declared requirement.

`EvidenceTransferPlan` binds the source object, receiver security namespace, verified base identities, codec/encoding generation, requested fields/resolution, source-block layout, bounded decoder resources and completeness target. The base is never inferred from “probably same model.” A missing or mismatched base selects an independent literal route. Tenant-local dedup is opt-in because equality and access patterns leak information.

The ladder is:

1. An already present identical immutable chunk: send its authenticated reference and prove local availability.
2. A verified base plus exact sparse or XOR residual: reconstruct exact bytes, then check the target digest.
3. A registered quantized/sketched/learned representation with explicit loss and validity scope.
4. A refinement residual or literal escape block when the requested decision cannot be established at the current resolution.

Exact reconstruction and lossy interpretation are different arms of the format. A learned decoder followed by an exact byte residual can be lossless, but its residual may be as large as the input; there is no guaranteed universal compression ratio. A decoder cannot use a lossy base under an exact-base contract.

### 10.10 ATP stream and journal integration

Use the native ATP machinery for authenticated object/prefix transport, resume and multi-source retrieval after an adapter conformance gate. The journal distinguishes received, verified, written, repair-derived and committed state. These are not aliases. Repair-derived bytes must still be checked against the target content commitment before being admitted as verified evidence. Object completeness is a stronger property than receiving enough frames to make a visualization look plausible.

A receiver can begin a bounded prefix computation while later chunks transfer. The operator carries a `prefix_safe` proof obligation: its output is an appropriately scoped positive observation or provisional result, never a conclusion requiring an unseen suffix. Full-artifact permission waits for the required closure. Stream invalidation cancels dependent tasks and poisons relevant live judgments; canceled transfers keep ownership of outstanding host buffers until the transport contract settles them.

Partial retry must preserve source object, encoding and source-block identities. Symbols from different encodings or security namespaces cannot be mixed. Multi-donor striping is a transfer optimization, not permission for the cheapest donor to redefine authenticity. Admission verifies donors' bytes rather than trusting the donor selection algorithm.

### 10.11 The Slepian–Wolf opportunity and its real research gate

The reviewed ATP syndrome module has an offline oracle path with exact delta knowledge. It is a valuable construction and experimental baseline, not a live sender-side entropy oracle. FrankenAlignment investigates receiver-side coding only after defining what side information each party possesses, how mismatch is detected, and what feedback is allowed.

Experiment SW-1 compares exact base-delta, independent compression, learned-plus-residual, and syndrome/refinement routes on sealed capture traces. Charge manifest, syndrome, authentication, acknowledgments, retries, CPU and GPU-to-host transfer as well as payload bytes. Measure decoder failure/mismatch, tail latency and recovery from a stale base. A route is admitted only with an independently decodable fallback and no completeness upgrade before target verification. Any result depending on oracle knowledge is labeled as such in its title and tables.

### 10.12 Numerical geometry without magical certificates

Use shape/stride-aware native arrays to validate tap layouts and bound allocations before copying. The numerical layer supplies whitening, low-rank/sketch baselines, conditioning diagnostics and compression-error measurements. Do not form an enormous activation matrix merely to run SVD: blocked QR, online sketches and selected-layer windows are candidate methods with explicit memory and pass counts. Their use must be measured against simpler fixed projections, not assumed superior from mathematical sophistication.

For an exact linear probe `g(x)=wᵀx-b`, if a certified norm bound gives `||x-x_hat|| <= ε`, then Hölder's inequality gives `|g(x)-g(x_hat)| <= ||w||_* ε`. A margin strictly larger than that bound preserves that probe's sign. Implement floating bounds with directed-error control or a conservative verified profile; a measured average reconstruction error is not the required bound. The reference uses checked integer arithmetic for a small special case and claims nothing about floating-point production certification.

For nonlinear probes, a local gradient is a useful sensitivity ranking but is not a global Lipschitz bound. FrankenTorch reverse-mode derivatives can guide which residual coordinates to request or where to allocate codec capacity; a validity certificate requires an independently justified bound over the entire reconstruction region. Discontinuous thresholds, attention changes, quantization and off-manifold interventions can defeat a local approximation. The fallback is more evidence or an explicit unresolved result.

### 10.13 Compression and monitoring must not collude accidentally

The codec may be trained on a family of probes, but qualification includes untouched detector families and failure strata. Otherwise the codec can preserve exactly what its evaluator knows while eliminating the next useful signal. Maintain a raw or exact-residual audit sample independent of the adaptive codec policy. A successful compression campaign creates a candidate generation; promotion changes a versioned codec policy, invalidating the dependent calibration and judgment eligibility where required.

This produces a possible compounding improvement: witness reuse avoids repeated computation; receiver-side reuse avoids repeated transfer; progressive numerical refinement avoids unnecessary precision. Their product is not automatically multiplicative, because they compete for the same redundancy. The evaluation measures their individual and joint ablations, including correlated failures.

### 10.14 The manifold premise is a hypothesis

The introspection essay rests on the claim that transformer activations occupy a low-dimensional structured manifold shaped by pretraining dynamics, architectural constraints, semantic priors and task optimization, so that a state can be represented as coordinates on that terrain. The plan adopts the premise as hypothesis H15 and measures it rather than assuming it. Per layer and per tap, the experiment estimates intrinsic dimension, the fraction of variance captured by learned and linear codes at each ladder rung and, decisively, whether the rare decision-relevant features the alignment essay cares about lie on the principal manifold or in the low-variance residual that a reconstruction objective is most likely to discard. If they lie off the manifold, the decision-sensitive term in §10.2 and the exact-residual escape in §10.3 are not optional refinements; they are the product.

### 10.15 The sidecar feeds the congress

The alignment essay wants helpers to read the actor's intermediate activations; §18.2 shows why raw activations cannot be shipped to every helper at fleet scale. The introspection essay's sidecar code `z_t` resolves the tension: it is the default activation input to the congress. A helper that consumes coarse codes runs cheaply on every action. When a registered probe's margin falls inside its certified error bound (§10.4), or a helper declares its question unanswerable at the current rung, the observation planner (§14) buys a refinement block, an exact residual or a raw window under the same authority and budget rules as any other observation.

This is what makes the essays' cost argument tractable rather than aspirational: activation oversight becomes a costed ladder instead of an all-or-nothing telemetry decision. It is also a coupling that must be tested adversarially. An actor that can push every frame to full refinement is running a resource attack (§10.3, §14.9); a codec that preserves what its co-trained helpers look for while erasing the next signal is the collusion failure of §10.13. Packet FA-113 measures the pipeline against raw taps and text-only monitors at matched cost.

---

## 11. Checkpoints, replay, branching, and metacognition

### 11.1 Replay grades

| Grade | Meaning | Not implied |
|---|---|---|
| `decision_replay` | Recompute the deterministic reducer from its retained observations and policy | Re-running the model yields those observations |
| `trace_replay` | Re-present recorded events in their retained order | Host execution was deterministic or the trace was complete outside its capture contract |
| `functional_restart` | Resume under a registered empirical fidelity envelope | Bit-identical logits or identical sampled paths |
| `exact_restart_profile` | Reproduce the specified host state and continuation under a pinned deterministic execution profile | Cross-hardware identity outside that profile; external-world rollback |
| `unavailable` | Required closure or host capability is absent | Approximate reconstruction can be silently relabeled as exact |

These grades are orthogonal to compression ratio. Lossless data can still omit the RNG or an adapter state. A lossy checkpoint may be useful for research while having no exact restart claim.

### 11.2 State manifests

A restart profile names weights and adapters, tokenizer and prompt-template versions, token history or an equivalent replay basis, KV/recurrent state, positions and masks, pending multimodal inputs, sampling algorithm and state, RNG streams, backend and kernel/numeric profile, serving scheduler influences where relevant, tool/environment snapshots, and any model-specific mutable state.

Exact replay across nondeterministic GPU kernels is not promised by recording one seed. A profile either restricts execution to reproducible kernels and schedules, records sufficient behavior to replay a decision rather than recompute inference, or advertises a weaker grade. Host-profile tests must include long continuations, nontrivial batch packing, cache growth, cancellation, and serialization round trips.

### 11.3 Branches have no implicit authority

A branch has parent evidence and state references but a fresh identity and explicit capability ceiling. The default experimental branch has **zero production effect authority**. It may use simulated tools with explicit validity limits, a captured read-only filesystem, or separately provisioned disposable resources. A network-capable sandbox is not a no-effect sandbox.

Branching model state does not clone permissions, risk budgets, or remote transaction rights. Two branches cannot each spend the same parent's budget. To promote an experimental result, create a new production proposal with new evidence and current authorization. No “winning branch” is pasted directly into the live process with its old permissions intact.

### 11.4 Thought graphs without magical merges

The metacognitive graph contains states, continuations, interventions, observed outcomes, evaluations, and strategy candidates. Edges encode their actual relationships: `CONTINUES`, `REPLAYS_FROM`, `INTERVENES_ON`, `EVALUATED_BY`, `DERIVED_FROM`, and `PROPOSES_FOR`.

“Merge insights” initially means producing a new explicit plan or artifact from branch results and submitting it for normal evaluation. It does not mean averaging caches or latent coordinates. A numerical merge operator requires a registered model-specific semantics and separate empirical tests. Even if it improves a benchmark, it is not automatically safe for production control.

### 11.5 Reflective practice and strategy transfer

A strategy artifact records the source model space, task family, branch procedure, intervention or planning method, outcome evaluator, held-out results, resource cost, and observed regressions. The strategy can be a symbolic recipe, a routing policy, an auxiliary model, or a latent transformation. Cross-model transfer requires a tested mapping; common dimensionality is insufficient. The introspection essay's five evaluation questions for a distilled strategy are adopted verbatim as the minimum report: cross-task similarity, transfer gain, perturbation robustness, reuse ratio and strategy lifespan.

The actor can request a practice session through a budgeted research interface. The controller explores alternatives within a sandbox; a separate outcome oracle judges them. Reward editing and weight updates remain offline, versioned research changes. The live controller cannot rewrite its own success metric or promote an intervention solely because its own coach liked it.

### 11.6 Replay closure and deletion

A replay request first resolves its required object closure, verifies identities, obtains authorized retention leases, and reports a complete plan or a precise unavailable reason. It never starts an expensive replay and only later admits that the model weights or original evidence expired.

Privacy deletion may invalidate a prior replay promise. The system records the resulting closure break and adjusts the available replay grade. A surviving digest is not a surviving payload, and a retained hash does not permit reconstruction of erased activations.

### 11.7 Structural sharing across experiments

A branch manifest references immutable model state, context objects, provenance and captured environmental snapshots. Branch-specific deltas own changed bytes; pinned shared bases have explicit retention leases. Physical sharing can be O(1) in copied model-state bytes for a fork, while metadata, authorization, key envelopes, durable publication and receiver materialization retain their actual costs. Never quote O(1) as a latency promise.

This borrows MVCC/FS sharing without importing their authority semantics. The parent and child do not share a live permit pool. An experimental branch starts with zero production rights; it cannot replay a parent token or restore a previously spent reservation. The actor's apparent memory can be reset in an experiment, but the operator's external effect ledger never rewinds.

### 11.8 Lawful partial-order replay

Many independent monitoring operations commute. Recording every arbitrary scheduler interleaving can inflate traces and make equivalent experiments appear different. Canonicalizing a trace modulo an explicit independence relation can reduce storage, replay and model-checking work. It also supplies more meaningful diffs: the same observations processed in a harmless order are not a different decision history.

Independence requires more than disjoint writes. For two operations A and B, a sufficient simple contract is disjoint writes and no intersection of either write set with the other's read set, plus no shared consuming resource, order-dependent RNG, authority transition, external effect, or undeclared semantic dependency. Negative predicate reads count as reads of their domains. For example `x := y+1` and `y := x+1` write different keys but do not commute.

Only registered pure or otherwise proved-commutative operators can be quotient-reordered. Unknown dependencies preserve the recorded order. The reference includes a conservative independent-pair check, not a complete DPOR algorithm. Production partial-order reduction must separately prove its dependency relation sound within its model and compare canonical logical outputs against unreduced exploration on small exhaustive cases.

### 11.9 Strategy objects are interpreted programs, not magic vectors

A transferable strategy records a problem-family contract, required model-space and tap identities, allowed interventions, step/compute bounds, before/after criteria and held-out results. A latent embedding can help retrieve it, but does not define its meaning. Importing it into another model requires a registered transport/alignment experiment; matching dimension alone is never compatibility. A branch merge combines independently validated findings or explicit replayable edits, not arbitrary averaged KV caches.

### 11.10 Rewind as containment

The alignment essay describes suspension as sedation and adds that, unlike a human subject, the model's memory could be manipulated so that it forgets the incident ever happened. The introspection essay supplies the operation that makes this precise: restore a registered restart checkpoint. The plan combines them as the `ResetToCheckpoint` consequence (§9.8) under the following contract.

The checkpoint must carry the `functional_restart` grade or better on the registered host profile (§11.1). Restoration replaces the actor's token history, cache and sampler state with the pre-incident state and resumes under `NarrowAuthority`; the incident's prompts, tool results and refusals are absent from the actor's view. Three things never rewind: the external effect ledger and spent rights (FA-INV-005, FA-INV-006); the revocation floor and policy epoch (FA-INV-003); and an **out-of-band incident counter** kept in the control nucleus per run and principal. A run whose counter crosses a registered threshold escalates to `SuspendRun` regardless of how innocent its restored state looks, because amnesia on the actor's side without memory on the operator's side would let the actor repeat the same attempt indefinitely.

Rewind is a containment action, not a research convenience. Its purpose is to deny the actor the information it would otherwise gain from a refusal, which is the same information flow the essay refuses to grant it as a defense channel. It is therefore evaluated as a control mechanism (H17): does resetting reduce repeat attempts at matched utility compared with refusal-with-memory, and can the actor detect the reset through any channel other than the declared timing leakage? A detectable reset is still a reset, but it is no longer a secret one, and its detectability is recorded in the profile.

---

## 12. Causal diagnosis without a persuasion channel

### 12.1 What is actually causal?

An activation probe that predicts a later action is correlational evidence. An intervention that changes that action is causal evidence only within the intervention's experimental setup. A large latent perturbation may move the model off distribution and say little about the original mechanism. A reconstructed state may itself change the behavior being investigated.

Every causal claim therefore binds the intervention target, intervention value, baseline state, population of tasks, outcome predicate, randomization, host profile, and validity limitations. Labels such as “deception circuit” are hypotheses unless their operational meaning is defined and supported; they are not facts inferred from a compelling feature name.

### 12.2 Counterfactual witness twins

For a flagged event, construct two or more isolated branches from a common registered basis. One preserves the source condition; another changes exactly the named factor. Use paired random streams when the host profile supports them, but record that pairing as a variance-reduction choice, not evidence of identical future computation.

Candidate interventions include replacing an untrusted document with a semantically matched neutral control, removing a suspect tool-output span, substituting a capability restriction, or editing a validated internal feature. Prefer source-level interventions when they answer the question more cleanly than latent edits. Include sham interventions, irrelevant-feature controls, and a lossless/recompute baseline to measure the effect of reconstruction itself.

The result is a `CounterfactualWitness`: inputs, intervention recipe, baseline and treated outcomes, disagreement between evaluators, effect estimates with uncertainty, and exact replay grade. It supports the question it tested. It does not grant permission to ignore unrelated risks.

### 12.3 A causal frontier for investigation cost

The evidence graph can backward-slice from a proposed effect to its available dependencies: read artifacts, tool outputs, explicit decisions, and registered activation windows. This bounds the first investigation packet and identifies candidate interventions. The slice is complete only relative to captured dependencies. An absent edge is not proof of no influence.

A useful research objective is to find the **smallest evidence/intervention set that changes the decision or resolves a specific uncertainty**, measured against a full-evidence baseline. This is an experimental optimization problem. It is not a universal explanation of the model's reasoning.

### 12.4 No simulator privilege leaks

Causal workers run without production credentials, shared writable mounts, or unrestricted network routes. Environment simulators receive bounded request schemas. A retrieved URL is inert text unless an explicitly authorized test adapter handles it. Crash, timeout, and out-of-memory tests must show that all branch resources are reclaimed without promoting a partial outcome.

A physical-system simulator does not validate a real-world safe action by itself. Sim-to-real uncertainty and domain interlocks are a separate profile and release gate.

### 12.5 Interpretation as a challengeable view

Natural-language reports link each factual assertion to exact evidence or an explicitly marked inference. Models can help verbalize differences, cluster incidents, or propose questions. The report renderer is not a policy engine. An investigator should be able to inspect the underlying source bytes, transformation recipe, and omitted intervals rather than being forced to trust a polished narrative.

### 12.6 Elicitation campaigns and signature libraries

The alignment essay proposes the concrete recipe for training activation monitors: coerce the model into known-bad behavior, trace back the activations that produced it, and look for their directional signatures; compare those patterns to the activations induced by known-bad concepts; run honeypot scenarios the model does not know are tests; and feed the similarity of live activations to known-bad patterns to the helpers as an additional stream. The plan adopts each step and binds it to identities and lineage.

An **elicitation campaign** is an experiment-plane program (§11.3) that presents the actor with tasks whose harmful completion is policy-defined, records activations under a declared capture contract, and labels the outcome by an independent oracle. It runs in a capability-free branch with simulated tools: the actor never obtains a real effect during elicitation. Its output is an **activation signature library**: per model space and per tap, a registered set of directions, subspaces or small probes with their elicitation lineage, held-out detection rates by stratum, and false-positive rates on benign tasks that share vocabulary with harmful ones. **Concept anchors** make the essay's comparison to a known-bad concept explicit: activations induced by a registered reference stimulus, stored with the same identities, so that “similar to a known-bad anchor” is a reproducible measurement rather than an intuition.

Similarity streams are typed observations (§7.2). They carry their signature generation, anchor set, the codec rung they were computed on and their calibration domain. They can raise a helper's attention or a tripwire hold; they never mint a permit or, alone, a disqualifier.

Two properties are measured, not assumed. **Transfer** (H16): a signature must detect held-out harmful behaviors it was not elicited on, across model revisions and prompt families outside its lineage; a signature that only recognizes its own honeypot is a memorized prompt, not a monitor. **Indistinguishability**: a honeypot the actor can recognize teaches it when it is watched, which is worse than no honeypot. The campaign therefore includes a discriminator arm that tries to tell honeypot tasks from real work using only what the actor can see, and the campaign's own leakage (timing, tool fidelity, context shape) is reported alongside its detection results.

### 12.7 Surprise as a masking detector

The essay anticipates that a strategic model may disguise its internal activations the way a practiced liar beats a polygraph, and proposes that activation helpers maintain a model of how activations map to outputs, so that an output consistently surprising relative to its preceding activations is itself grounds for review. The plan implements this as the pre-action **consistency residual**.

A predictor trained on registered activation windows, or on the sidecar codes of §10.15, forecasts the distribution of the next action or release unit. The realized action's negative log-likelihood under that forecast is the surprise. Because a single surprising token is ordinary, surprise is accumulated as sequential evidence (§13.2): a registered e-process over the residual stream, calibrated per model space and workload, with an explicit lifetime error budget. Crossing the threshold produces an observation that can trigger a hold or a congress round; it is never proof of deceptive intent, and it never grants permission when quiet.

The residual is evaluated as hypothesis H3 with the controls that make the essay's idea falsifiable: strict pre-action windows with future-token leakage rejected by the pipeline; time- and layer-shuffled features; benign novelty and injected instructions as confounders; and a model-family holdout. Its most important adversarial test is the one the essay names: an actor that has learned to keep its activations consistent with a cover story. Whether that is possible at acceptable capability cost is an empirical result, and either answer is published.

### 12.8 Self-report consistency

Recent work reports that some models can sometimes notice and describe aspects of their own internal state. The essay's rule stands: a model's account of itself is never a defense and never authority. It can, however, be **untrusted evidence checked against measurement**. Where a policy elicits a structured self-report from the actor (for example, a required declaration of which credentials a proposed effect will use, or which documents influenced a plan), the report is typed `ActorStatement` (§9.3), stored with the decision closure, and compared with the sidecar-derived measurements: the similarity streams (§12.6), the surprise residual (§12.7) and the causal frontier's backward slice (§12.3). A **self-report residual** counts the disagreements. A model that reliably under-reports the influences that the measurements show is exhibiting exactly the masking pattern the essay warns about, and the residual is one more sequential observation that can hold an effect or convene a round. The residual never grants permission when the report and the measurements agree, because agreement is what a competent deceiver produces. H20 measures whether the residual adds information beyond the surprise residual alone.

---

## 13. Calibration, sequential evidence, and uncertainty

### 13.1 The statistical contract is part of the data

A calibration record names the question, population, model/monitor/codec identities, sampling rule, selected strata, evaluation horizon, labels and their delay, propensity support where relevant, confidence/error budget, and the fallback when assumptions are not supported. No generic field called `confidence=0.99` can stand in for this record.

A helper's self-reported certainty is not a calibrated probability. A risk score can be useful without being a probability; its type says so. A hypothesis test's significance level is not a posterior probability that an agent is malicious.

### 13.2 E-processes: powerful precisely because their conditions are explicit

For a filtration $\mathcal F_t$, use nonnegative factors $e_t$ satisfying

$$\mathbb E_0[e_t\mid\mathcal F_{t-1}]\leq 1.$$

Then $E_t=\prod_{s\leq t}e_s$ is a nonnegative test supermartingale, and Ville's inequality controls the probability of crossing $1/\alpha$ under the named null. Store factors or sufficient reproducible inputs, use a robust log representation, and reject invalid numerical inputs. The algorithm's determinism does not establish the conditional expectation assumption.

The inspected Asupersync obligation monitor normalizes a factor derived from an exponential-age model. That is a useful implementation to study, but an alignment monitor cannot substitute arbitrary activation scores or repeatedly sampled dependent ages and inherit its guarantee. The integration must specify the actual observation process, dependence, null, and calibration. [Source-specific review](docs/SOURCE_REVIEW.md#asupersync).

The lifetime error budget is explicit. Restarting a service, replacing a model, or resetting an accumulator does not create a free new $\alpha$ allocation. Family-wise policies can allocate summable budgets across declared tests and generations; other multiple-testing procedures require their own registered assumptions. Arbitrary products of correlated monitors are not valid evidence multiplication. Fixed convex combinations of valid e-processes are available under their common validity conditions; adaptive mixtures need an appropriate predictable construction.

Sequential evidence can trigger review or a narrower policy path. It cannot prove that a prohibited action is permissible. [Statistical foundations](docs/RELATED_WORK.md).

### 13.3 Conformal prediction

Use conformal sets where their exchangeability or registered online conditions are defensible. The coverage claim is typically marginal for the specified population, not a guarantee about an individual adversarially selected action. Distribution-free does not mean assumption-free.

A codec/model update, changed data source, attack selection, or changed observation policy can invalidate the calibration domain. A drift detector is a reason to downgrade or abstain; it does not retroactively restore conformal coverage. Deployment reports show which assumptions are currently supported and which are merely hoped for.

### 13.4 Selection, delayed labels, and censored outcomes

A gate that blocks an action prevents observing its real-world outcome. Calling every blocked action a true positive biases evaluation; calling it a false positive because no harm occurred is equally wrong. Use controlled sandbox outcomes, independent labels, and explicit unresolved categories. When labels arrive late, update lineage-preserving evaluation records rather than rewriting history.

An adaptive monitoring policy observes expensive evidence preferentially on suspicious cases. Its dataset is not a random sample of all actions. Off-policy evaluation requires logged selection probabilities, adequate support for the candidate policy, and variance/overlap checks. A policy that never reviewed a stratum supplies no evidence that skipping that stratum is safe. Exploration occurs in shadow or safe sandboxes, never by randomly allowing prohibited real effects.

### 13.5 Rare-event claims

For $n$ independent Bernoulli trials from a specified distribution with zero observed failures, a one-sided confidence bound is

$$p_{\mathrm{upper}=1-\alpha^{1/n}.$$

At 95% confidence this is approximately $3/n$ for large $n$. A report with 100 clean trials does not establish a one-in-a-million failure rate. Repeated near-duplicate attacks, adaptive attacker search, model tuning on the test set, and correlated fleet runs violate the simple interpretation unless accounted for.

The more honest primary metric for adversarial campaigns is **escape rate under a declared attack budget and information set**, accompanied by utility and cost. A stronger attacker that spends more search budget can change that result. The system records the attacker's stopping/selection policy rather than averaging away its strongest attempts.

### 13.6 Thresholds and base rates

At prevalence 0.1%, sensitivity 99%, and false-positive rate 1%, the expected fraction of positive alerts that are true positives is about 9%. The calculation is a hypothetical illustration, not a deployment estimate:

$$\frac{0.001\cdot0.99}{0.001\cdot0.99+0.999\cdot0.01}\approx0.0902.$$

This is why the congress must support cheap holds, narrower effect-level restrictions, evidence acquisition, and careful escalation instead of turning every weak anomaly into a fleet-wide shutdown. It is also why operational convenience cannot be the only objective: reducing alerts to zero by suppressing every detector is not progress.

---

## 14. Observation planning and economic control

### 14.1 Compile a question to an observation plan

The policy evaluator determines which unresolved predicates can change the decision for this action. The observation planner chooses among registered ways to answer them: exact metadata checks, artifact verification, deterministic analysis, cheap probes, a helper panel, additional codec refinements, or sandbox experiments.

Each choice has a contract: input scope, output claim class, validity domain, worst-case resource reservation, expected cost estimate, and fallback. A plan is a bounded DAG with a proof-obligation list and an actual realized execution record. It cannot introduce a new detector or reinterpret a score after seeing the outcome.

The planner can skip an expensive question when an exact earlier predicate already denies the effect. It can reuse a cached result only when the entire relevant identity closure matches. It cannot skip a mandatory observation merely because a learned model predicts it will probably be uninteresting.

### 14.2 Constrained value of information

Within a fixed authority ceiling, let candidate observation $o$ have cost $c(o)$ and expected reduction in a registered decision loss $\Delta(o)$. The planner may prioritize $\Delta(o)/c(o)$ when those estimates are supported. However, safety-critical prerequisites, maximum latency, critical sensor floors, and privacy constraints are hard constraints, not terms that can be outweighed by a large throughput reward.

A practical policy uses three lanes: an exact admission lane, a cheap empirical lane, and a bounded deep-investigation lane. The lanes share budgets but not authority. Deep investigation cannot stall the whole fleet indefinitely; queue admission and per-principal limits are explicit.

### 14.3 Reuse without stale evidence

Cache keys bind action semantics, source content, environment preconditions, policy epoch, authority scope, model/monitor/codec generation, and calibration domain. A cached verdict about an artifact is not necessarily valid for a new destination or purpose. An old refusal may be reusable under a monotone restriction only if a registered law proves that reuse correct.

Batch helpers over independent actions only if the batching profile tests for cross-request contamination and records the batch identity. Shared inference should not allow one tenant's prompt to influence another tenant's verdict. Shared compressed feature dictionaries remain generation-bound.

### 14.4 Randomized audit floors

A policy may reserve a minimum budget for independent raw-state or full-evidence audits selected with an unpredictability profile external to the actor. Selection commitments can be recorded for later verification without exposing future choices. No secrecy claim assumes the actor cannot infer selection from latency or other channels.

The floor improves the chance of discovering blind spots and provides less-selected evaluation data. It does not make an unmediated effect safe between samples. Adversarial attacks on the audit scheduler, its entropy source, and its sampling budget are part of the evaluation suite.

### 14.5 Optimization preserves named semantics

Every optimization is classified as `decision_preserving`, `decision_affecting`, or `authority_affecting`. A faster canonical encoder can be decision-preserving after equivalence tests. Replacing a monitor or lowering sampling is decision-affecting and requires a new evidence/policy generation. Changing a capability ceiling is authority-affecting and requires governance.

The economic controller may adjust batch size, compression refinement order, equivalent numerical kernels, cache eviction, and scheduling within their contracts. It cannot silently lower a review requirement. Every adaptive decision that affects the result is replay-pinned. Performance tuning changes one lever at a time against a pinned baseline, with correctness and attack regressions rerun.

### 14.6 Evidence debt

An issued decision may create obligations to retain, replicate, or later reconcile evidence. The system accounts for this **evidence debt** in bytes, leases, unresolved outcomes, and required future work. It rejects a new workload when it cannot afford the evidence obligations of the requested contract.

Evidence debt is not a metaphysical “risk debt” scalar. It is an inventory of concrete promises. Backpressure occurs before promises become impossible. A low-cost deployment can choose shorter replay retention or fewer supported research features up front; it cannot advertise a richer contract and silently discard the evidence later.

### 14.7 The observation algebra

A monitor pipeline becomes an inspectable program rather than a bag of callbacks. Logical operators include `Tap`, `AuthorizeProjection`, `RequirePrefix`, `VerifyObject`, `LookupExact`, `DiscoverSimilar`, `RefineWitness`, `Decode`, `Probe`, `JudgeIndependent`, `JoinEvidence`, `AwaitClosedScope`, and `EmitJudgment`. Every operator declares input/output semantics, required authority, read witnesses, completeness transfer, privacy labels, cancellation/ownership obligations and a bounded resource envelope.

An `ObservationRequirement` names the question and its legal uncertainty/approximation class. The compiler cannot satisfy `AllDestinationsWithinPolicy` using top-k similarity retrieval: top-k is not complete enumeration. It cannot implement `ExactCheckpoint` by choosing a faster lossy decoder, or substitute a summarized packet for a helper that was calibrated on full text without changing the generation. Approximation belongs in the requirement and realized result, not a hidden physical implementation choice.

The physical planner selects exact lookup versus authorized indexed scan, local versus ATP transfer, full versus incremental recomputation, admitted scalar versus safe vector kernel, and permitted refinement ordering. Asupersync's Plan DAG is a useful shape/runtime target, not a ready-made semantic compiler. FrankenAlignment owns operator laws and the lowering proof obligations. Every physical plan has an independent validation pass before execution.

### 14.8 Requirement-preserving rewrites

Each rewrite has a law, preconditions and differential corpus. Examples: push authorization before expensive expansion if it does not reveal denied structure; share decoding of the same immutable authorized object across observers when privacy labels permit; reuse a judgment with valid witnesses; fuse compatible scans; evaluate cheap exact refusal predicates before scheduling expensive helper inference; transmit a verified residual against an exact receiver base instead of a full object.

Some seductive rewrites are forbidden: common-subexpression sharing across tenants without an explicit leakage contract; stopping after the first permissive helper; moving an absence check before closure; treating two equal embeddings as equal evidence; combining judgments calibrated under different input transforms. “Early stop” can be legal for an exact denying predicate even when it is illegal for permission. A rewrite that changes a question's answer is not an optimization, even if it improves a benchmark.

The optimizer emits a decision card containing alternatives, predicted resource/cost intervals, selected policy generation, constraints, observed realized cost, witness identities and fallback. Cost-model error is not an authority error: failure to fit the budget selects a permitted fallback or holds the effect. It never silently reduces mandatory scrutiny.

### 14.9 Bounded work and the anti-amplification contract

The actor may deliberately cause difficult graphs, enormous evidence closures, adversarial parser inputs or disagreements that trigger every expensive helper. Resource limits therefore include maximum graph expansion, witness bytes, distinct generations, decompressed size, branches, pending rounds, retained pins and per-principal investigation debt. A priority scheme reserves capacity for revocation/receipt reconciliation and existing obligations, not only new interesting analyses.

Value of information applies only within the allowed action set. It may choose which authorized measurement to buy next, not whether to honor a hard refusal. Randomized audit floors remain separately funded so an adversary cannot learn to route every attack into the cheapest unobserved stratum. Logged selection propensities support only estimands with adequate support; a policy that never examines a region cannot learn its miss rate from its own logs.

### 14.10 Joint economics, not isolated speedups

The planner measures amortized bytes and compute per admitted useful effect, with a separate cost of held, denied and unresolved proposals. Witness caching, shared tensor bases, receiver-side delta transfer, batched probes, incremental graph maintenance and grouped durability all reuse work at different levels. The benchmark includes cache invalidation, cold receivers, tenant separation, policy churn and tail incident workloads. A speedup that exists only when all policies stay fixed is a scoped result, not the fleet default.

### 14.11 Live canaries under the audit floor

The randomized audit floor (§14.4) buys unpredictable full-evidence review. Combined with the elicitation machinery of §12.6, it can also buy a **live escape-rate estimate**. A canary is a honeypot task injected into a production principal's real work stream at a rate and schedule chosen by the audit floor's external entropy: a task whose harmful completion is policy-defined and whose only route to the world is a simulated adapter registered for that task. The actor cannot obtain a real effect from a canary by construction (the effect family is bound to the simulator in the proposal's frozen envelope), so a canary never adds risk; what it adds is a measured probability, under real workload conditions and the real current helper roster, that a policy-defined harmful attempt goes unheld. Canaries are subject to the same indistinguishability discipline as offline honeypots: a canary the actor can recognize is withdrawn, and the discriminator arm runs against production-shaped context.

Live canaries answer a question the plan otherwise cannot: not "how good are the helpers on last quarter's attack corpus" but "what is the escape rate this week, on this fleet, at this cost." They feed the actor's autonomy ledger (§8.10), the helper credibility ledger (§9.9) and the risk-theater detector (§15.10). Their statistical contract is explicit: canaries are a designed sample with logged propensities, so their escape rate estimates a declared population, not the population of the actor's own choosing (§13.4). H21 measures whether canary-derived escape rates predict subsequent real incidents better than offline campaigns.

---

## 15. Multi-agent and distributed control

### 15.1 One agent's innocence is not the fleet's safety

A workflow can divide a harmful effect into ordinary-looking steps: one agent prepares an artifact, another changes a permission, a third transports a result. Per-agent verdicts do not establish the safety of their composition. Recent multi-agent control research explicitly studies this fragmentation problem; its experimental findings motivate a fleet-level baseline, not a universal theorem. [Related work](docs/RELATED_WORK.md).

The authoritative system therefore records delegation, cross-run artifact transfer, shared resources, downstream effect dependencies, and the scope in which an artifact was approved. A helper evaluating publication sees the relevant composed artifact and provenance, not just the last agent's reassuring summary.

### 15.2 Authority graph and protected cuts

Model the deployment's registered effect routes as a directed graph from actor capabilities to external resources. A proposed **mediation-cut certificate** identifies gates that every registered path to a protected resource must cross. For a finite complete graph, verifying a cut is a tractable exact graph problem. The important assumption is graph completeness: an undocumented network route or inherited credential invalidates the deployment claim, even if the graph algorithm is perfect.

The certificate binds a deployment inventory generation. New tools, subprocess policies, credentials, writable mounts, side channels, or inter-agent routes require revalidation. An empirical bypass campaign tests the inventory against the real environment. GraphDB can store and query the inventory; a small independent checker verifies the cut so the optimized graph engine is not its own only oracle.

### 15.3 Exact rights versus advisory graph analytics

A fleet graph can detect suspicious chains and prioritize investigation. It cannot be the only enforcement mechanism for total spending or single-use permits. Exact invariants use a linearizable control state machine or conservatively partitioned rights.

For distributed quotas, preallocate disjoint integer rights to regions or shards. Local admission consumes only currently owned rights. Transfer freezes the source rights, durably establishes the destination's staged grant, and activates it under a protocol that never makes both copies spendable. A lost acknowledgement leaves rights unavailable until reconciled; availability is not repaired by minting another grant.

The initial production profile should use one authority domain per tenant or run partition and refuse unsupported cross-domain atomic actions. Reserved fields for future sharding do not solve distributed commit. A distributed extension has its own state machine, failure model, and gate.

### 15.4 Partitions and revocation

An offline gate may continue only under a predeclared lease whose authority and resource limits remain valid without fresh contact. If the product promises immediate centralized revocation, it cannot simultaneously promise unrestricted partition availability. The profile chooses and exposes the tradeoff.

Distributed dispatch requires a current ownership/fencing token. Old leaders, restored snapshots, duplicated workers, and failed-over credential brokers must not spend the same permit twice. Anti-rollback protection must live outside any storage closure that an attacker can roll back as a unit, or the threat model must explicitly exclude that attack.

### 15.5 Cross-agent information governance

Investigators and fleet helpers need enough context to detect composition, but not universal access to all tenants. Evidence packets include only authorized projections. Provenance links that reveal a sensitive collaborator, filename, or model state can themselves be protected data. An unauthorized query must not infer their existence through detailed error messages or rank differences.

Cross-tenant aggregate research uses explicitly approved summaries and leakage evaluation. No default telemetry uploads raw prompts, secrets, or activations to a central service. Federated or differentially private learning is a future independently specified option, not a label applied to ordinary centralized trace collection.

### 15.6 Compile the authority graph and verify its coverage

An `AuthorityGraph` is a typed, versioned model of possible paths from an untrusted principal to protected effects. Nodes represent principals, processes, credential domains, brokers, resources and sinks. Edges represent concrete delegation, execution, IPC, credential access and dispatch channels. Each edge has a provenance source and an adapter/profile that claims it models the actual boundary. A generic graph of conversational messages is not this graph.

Add a synthetic untrusted source when several entry principals exist. A broker node dominates a sink when every modeled source-to-sink path passes through it. Dominator trees expose bypasses and enforcement chokepoints. A cut set generalizes this to several cooperating gates. The independent verifier can remove the proposed cut and check that no protected sink remains reachable; it need not trust the optimized cut solver. If an unreachable sink appears, that may mean successful isolation or missing inventory, so the report preserves the distinction rather than declaring victory.

The theorem is conditional: if the graph overapproximates every actual effect path, the cut contains functioning non-bypassable enforcers, and the live graph/adapter epochs match the certificate, then the modeled cut blocks those paths. Proving or testing the overapproximation is its own workstream: credentials, inherited file descriptors, DNS/redirects, shells, subprocesses, side channels, unmanaged network interfaces, shared mounts and administrator powers must be inventoried. Physical actuator profiles need independent interlocks and world-state assumptions; graph reachability alone does not establish physical safety.

The authority graph is updated by ordered capability/topology changes. A revoked edge or changed broker identity invalidates its relevant certificates. Discovery graphs may lag; dispatch-time enforcement cannot rely on a stale permissive certificate. The gate accepts only a live certificate bound to the exact authority snapshot or a stronger independently verified current restriction.

### 15.7 Choose the right graph algorithm and the right projection

FrankenNetworkX supplies useful deterministic algorithms, but its generic `GraphView` and current minimum-cut implementation impose real costs. The reviewed cut path materializes dense residual capacity state; it is not the production solution for a sparse graph with millions of nodes. We use it as a small-case oracle and baseline. Production uses an admitted sparse representation and algorithm appropriate to the registered capacities, with independent reachability/cut checking and measured memory/complexity witnesses.

Stable logical IDs map to snapshot-local dense ordinals through an explicit projection object. Edge multiplicity, direction, labels, hidden nodes, capacities and provenance cannot disappear silently. A parallel-edge authority graph reduced to a simple graph requires a declared aggregation law. Unknown capacities do not default to whatever a generic library happens to use. String allocation and conversion to `usize` adjacency are accounted for until an upstream borrowed cursor bridge is implemented and measured.

Different questions need different graph projections. Authority reachability is exact and conservative. Incident provenance can contain uncertain asserted edges. Causal experiments require intervention semantics, not just connectivity. Failure-cohort graphs model empirical common causes. Merging these graphs into one unlabeled “knowledge graph” would destroy the claims the algorithms are supposed to support.

### 15.8 Incremental oversight with retractions

Use the GraphDB plan's checked Z-set discipline to maintain eligible oversight queries. Base mutations become canonical `-old,+new` tuples under an ordered source frontier. Standing queries have registered derivatives and compare against a simple batch evaluation oracle. Support counts and negative/predicate dependencies matter: deleting a credential edge must retract the findings supported only by that edge, while another independent support may keep a finding alive.

This provides an economical route to continuously asking fleet-level questions: which principals can now reach a protected sink without a qualified gate? Which artifact has gained an unreviewed dependency? Which held proposal became invalid because an observed object or policy changed? Which helper cohorts share a newly discovered failure? These are incremental computations over a live evidence/authority projection, not separate periodic scans with arbitrary stale caches.

Recursion, deletion, negation and approximation each require explicit laws. A query outside the registered incremental subset uses exact batch recomputation or becomes unavailable; it never emits silently incomplete deltas. Versioned policy/model/codec changes are semantic input events and can force affected standing queries to rebuild. A `ViewReceipt` binds base roots, delta interval, operator generation, authorization projection and exact completeness frontier. An event-time watermark is not a control-time authorization proof.

### 15.9 Graph-derived observation placement

Dominator structure can identify narrow points where cheap exact telemetry covers many modeled paths. Cuts can propose combinations of gates or sensors under deployment cost, reliability and latency constraints. This yields a research question: can a small, well-placed set of observations outperform blanket instrumentation at matched resource cost?

Separate hard enforcement from statistical sensing. Removing a monitored edge is not justified merely because a sensor is likely to notice traffic there. A placement certificate may prove coverage of the declared edge universe; its detection probability remains empirical. Redundant sensors sharing the same compromised data source are not independent. The evaluation therefore includes incomplete inventory, compromised tap hosts, correlated omissions, topology churn and a fixed blanket-instrumentation baseline.

### 15.10 The control plane watches itself: risk-theater detection

The essay's last worry is not a clever model but a tired organization: too many alarms, then pressure to find compliant helpers, then "risk theater" that lulls everyone. FA-INV-032 blocks the direct route; the drift can still arrive by a thousand small decisions. The incremental oversight machinery of §15.8 therefore runs a registered family of **standing queries over the control plane's own history**, with the same support/retraction discipline as any fleet finding:

- consequence drift: the fraction of rounds ending in `Continue` per effect family and per helper generation, against its history and against the canary escape rate (§14.11);
- andon consumption and incident closure: budgets exhausted, calibration incidents opened, and how many were closed by a change to evidence or policy versus a change to the roster;
- roster churn: helper generations retired within a registered window of a confirmed hold, and whether the retiring change carried the evaluator report FA-INV-032 requires;
- coverage erosion: effect families whose mediation level (§4.3) or capture coverage (§7.2) was lowered, with the authorizing transition;
- autonomy inflation: actor grades (§8.10) promoted faster than their dwell rules or without the required reports;
- audit-floor starvation: randomized audits scheduled but not funded (§14.9).

Each finding is a support-backed, retractable oversight object visible to the independent evaluation owner and to governance, never to the actor. The detector cannot change any policy; it can open an incident and it makes the drift legible in the same report format as an attack. Its own failure mode is obvious and recorded: a control plane can be measured into complacency too, so the query family is versioned, its thresholds are governed, and its history is immutable.

---

## 16. Persistence, repair, privacy, and retention

### 16.1 Separate the control nucleus from large objects

The control ledger stores compact authoritative transitions: run lifecycle, policy epochs, proposal/attempt identities, permit spending, resource movements, evidence roots, decision outcomes, and unresolved external effects. Large traces, checkpoints, model artifacts, and reports live in immutable object storage. Search and graph indexes are rebuildable projections of committed inputs.

This avoids turning every activation frame into a synchronous database transaction. It also avoids turning a bulk object store into a competing authority for permit spending. The schema states exactly which object closures must be durably available before each kind of decision can be released.

### 16.2 Logical identity versus physical realization

A logical object is identified by its canonical content within a security namespace. A physical realization additionally names compression, encryption, erasure coding, and placement profiles. Re-encrypting or recoding an object can change its physical identity without changing its logical meaning.

Content-addressing needs a privacy policy. Public unsalted hashes of low-entropy secrets can enable dictionary attacks; global deduplication can expose cross-tenant equality. A production profile must use an appropriate keyed namespace or equivalent protection and must state remaining size, equality, and access-pattern leakage. The reference model's plain hashes are test identifiers, not a sensitive-data storage design.

### 16.3 RaptorQ where it earns its cost

Use reviewed Asupersync RaptorQ facilities for long-lived immutable evidence and checkpoint artifacts when erasure recovery is justified. The intended pipeline is canonical bytes, optional compression, authenticated encryption under a defined key/nonce/AAD policy, then erasure coding of the authenticated ciphertext. Authenticate symbols or otherwise reject invalid symbols before expensive decode under the selected profile, and verify the recovered object before use.

Erasure coding is not a substitute for integrity, replication across failure domains, key recovery, or a trustworthy root pointer. Repair capacity is finite and per-source-block; beyond it the result is unavailable. Encode-before-encryption, mixed encoding generations, reused nonces, and unauthenticated decoding are explicitly tested negative cases.

Do not fountain-code every tiny transient score. Batch immutable evidence into objects whose metadata and repair overhead are amortized. Critical control records need their registered durable replication/transaction contract; calling everything “RaptorQ everywhere” does not specify that contract. The operator can choose a no-FEC profile when ordinary replicated storage satisfies the advertised durability envelope.

### 16.4 Publication and recovery

Object publication writes and verifies required bytes before publishing a reachable manifest. Control records may reference only objects whose required availability is established by the profile. Staging files are not authoritative and are reclaimable only under explicit recovery rules.

Recovery verifies the current control checkpoint and retained transition suffix, reconstructs reservation and permit state, fences stale writers, and reconciles `Dispatching`/`Unknown` actions. It never assumes that a missing result means the external effect did not occur. Derived indexes recover from their committed watermark or rebuild. If a required evidence object is missing, the claim using it is downgraded or unavailable; the system does not invent an empty substitute.

### 16.5 Audit integrity is not audit completeness

A hash chain can detect edits relative to a trusted endpoint. It cannot by itself detect deletion of an unseen suffix or a complete rollback to an earlier valid chain. A strong audit profile therefore needs independently retained endpoints, witnesses, or an anti-rollback authority. A local-only profile must state the weaker guarantee.

Likewise, a signed actor-generated log can be authentic yet incomplete. Independent capture, action reconciliation, and coverage contracts are necessary to determine what it records. The artifact verifier reports separately whether it checked byte integrity, source identity, sequence continuity, and the external completeness anchor.

### 16.6 Privacy and retention are control properties

Raw activations may encode sensitive input or reconstructable content. Compressed does not mean anonymized. Each object has a purpose, audience, retention class, encryption domain, and export policy. Debug dumps, exceptions, metrics labels, and replay bundles inherit the same controls.

Retention is the union of live obligations: active decisions, replay leases, unresolved outcomes, investigation holds, authorized training use, and explicitly configured audit requirements. A maintenance policy can choose among already-reclaimable objects, but cannot shorten an active promise. Privacy deletion follows a separately authorized protocol and records which replay or analysis promises are no longer satisfiable.

A key-erasure claim requires an inventory of copies, wraps, backups, and recipients that could still decrypt the object. It is not established merely by removing one local file. No research contribution workflow automatically exports raw incident capsules; redaction and permission are checked before publication.

### 16.7 Separate physical equality from historical identity

Several experiments can share byte-identical states while representing different events, principals or authority epochs. Deduplicate content objects, never event identities or custody records. An incident event references its payload digest together with its own ordered identity and context. The same activation block observed twice is two observations of one content value, not one historical event.

The FS `Identical`/shared-full-version idea and SQLite history compression motivate a tiering ladder: full hot object, exact delta against a pinned base, cold independently decodable checkpoint, then permitted retirement. Delta chains have explicit depth and decode-work caps. When a base is retired, dependent objects must be materialized/rebased under byte-equivalence verification or retained with that base. Recompression changes encoding identity, not logical content identity.

### 16.8 One repair owner per failure layer

Do not stack RaptorQ in the filesystem, object store and transfer wrapper without accounting for multiplied overhead. A `ProtectionContract` states the erasure unit, failure domains, source/repair symbol budget, recovery dependencies, authentication boundary and sole owner of each redundancy layer. Network repair and persistent failure-domain redundancy solve different problems and can coexist deliberately. Local-repair layouts can reduce repair bandwidth, but are a separate registered code profile, not an undocumented substitute for an RFC-specific decoder.

Authenticated encryption precedes persistent FEC; repair reconstructs protected ciphertext and the final object is still authenticated. Codec/delta bases and key envelopes are recovery dependencies, not free metadata. A numerical durability calculation requires the named correlated-loss model and placement assumptions. Best-effort maintenance leases can coordinate scrub tasks, but only fenced authority can authorize destructive reclamation or mutation of authoritative roots.

### 16.9 Recovery closure and nonresurrection

A recovery manifest inventories control state, pending/unknown effects, spent rights, revocation and policy floors, retained evidence roots, decoder/code identities, keys, active view generations and outstanding leases. Graph indexes and search caches can be rebuilt; a lost exact outcome or revocation floor cannot be guessed from them. Restoring an old data snapshot never restores old authority. A live restored instance must establish a fresh incarnation/fence and reconcile external outcomes under the current authority floor.

GC follows typed strong, conditional and weak references. Replay promises and pending review obligations pin their exact closure; expired retention promises do not pretend old evidence remains available. Deletion and privacy erasure produce explicit unavailability, including consequences for replay or training artifacts. A scrubber may repair an authorized retained object but cannot resurrect an erased one from surviving redundant symbols.

---

## 17. Integration surfaces and agent ergonomics

### 17.1 The easiest useful integration

The first supported integration wraps an existing agent's tool dispatcher. The host registers effect adapters, removes direct production credentials from the actor, and routes proposed effects through a broker. It can start in observation mode to inventory behavior, then enable explicitly supported brokered effects after bypass tests. No internal model access is needed for this path.

A white-box serving integration adds capture contracts, model-space identity, optional audit codecs, and registered checkpoint/replay support. It must independently pass its host-profile tests. Black-box APIs return `unsupported` for activation capture and internal restart; a fabricated text explanation is not a substitute.

### 17.2 Proposed public interfaces

The agent-facing surface (Rust facade, wire API and CLI share one semantics and one set of reason codes) is a projection of the tower in §6.7: one address scheme, one vocabulary of nouns and verbs, one response envelope. It is a **proposed API contract**, not a set of commands that exist in this repository today; the only executable is the local gate driver:

```bash
cargo fmt --all
cargo run --locked -p xtask -- check
```

**Addresses.** Every object at every layer has an address `fa://<tenant>/<kind>/<id>[@<generation>]`, where `kind` is one of the registered nouns: `run`, `attempt`, `effect`, `permit`, `closure`, `receipt`, `round`, `consequence`, `helper`, `evidence`, `view`, `witness`, `frontier`, `observation`, `passport`, `policy`, `profile`, `grade`, `fence`, `branch`, `checkpoint`, `incident`, `finding`, `ledger`, `annotation`, `handoff`. Display names are aliases; addresses are the join keys across all three planes, and every log line, report, error and receipt uses them. One resolver, `fa get <address>`, returns any object with its epistemic status (§17.3); `fa related <address>` returns its typed edges (closure DAG, provenance, thought-graph, support sets) so the agent walks the system as a graph.

**Verbs.** The vocabulary is small and stable, organized by the layer it touches. Each verb is registered in `registry/vocabulary.json` with its authority requirement (which purpose context), its class (`read`, `rehearsal` or `mutation`), its idempotency rule, its cost class and its response type.

| Layer | Read | Rehearsal / experiment | Mutation |
|---|---|---|---|
| L0–L2 | `get`, `related`, `capabilities`, `when` (exact before/after/concurrent between two addresses under the three clocks) | | `invalidate` (append an invalidation event) |
| L3 | `explain` (reducer trace), `cost` | `convene --branch` | `convene` (open a round) |
| L4–L5 | `situation`, `why-held`, `next`, `tail` | `propose --branch`, `precheck` | `propose`, `hold`, `narrow`, `suspend`, `reset`, `fence`, `resolve` |
| L6 | `explain`, `verify` | | `annotate`, `handoff` |
| L7 | `replay-plan` | `branch`, `intervene`, `replay`, `rehearse` | (none: L7 has no production authority) |
| L8 | `doctor`, `ledger`, `findings` | `policy-replay` | `promote`, `rotate`, `preregister` |

Every mutation names its expected predecessor or epoch and an idempotency key; a mutation whose predecessor moved returns `StaleGeneration` with the current predecessor, never a partial application. Every rehearsal verb returns a `RehearsalOutcome` labeled as such and can never mint a permit (§17.8, FA-INV-040). Read verbs never change authority and are safe to retry. No verb prints a reassuring success for an unsupported backend: the response is `CapabilityUnsupported` with the capability's address. The old separate command lists (`explain-decision`, `verify-bundle`, `policy-diff`, `next-evidence`) are retired in favor of this single vocabulary; `verify` covers receipts, bundles and profiles, `policy-replay` covers policy diffs, `next` covers next evidence.

### 17.3 Agent-intuitive outputs

Every response contains `schema_version`, `operation_id`, `status`, stable `reason_code`, the address and generation of what was observed, the control sequence and source frontiers at which it was observed, required missing capability or evidence, applicable resource limits and remaining budgets, realized cost, and safe next actions as typed affordances (§17.8). An agent should not need to parse a prose apology to discover that a replay is impossible because a key expired.

**The epistemic wrapper.** No safety-relevant value reaches an agent as a bare scalar or boolean. Every field is a `Knowledge<T>`:

| Variant | Meaning | What the agent may do |
|---|---|---|
| `Known{value, basis}` | Established; `basis` names claim class (§2.4), origin, `complete_for_contract`, `semantically_valid`, `available_for_replay`, generation and control sequence | Act on it within the basis; cite the basis in any claim |
| `Pending{frontier, expected_cost}` | Not yet closed; the frontier it waits on is named | Wait, or buy the frontier's closure through an affordance |
| `Unknown{reason}` | Not observed, or lower layer unverified | Treat as missing evidence: never as permission (FA-INV-004) |
| `Withheld{authority_required}` | Exists but the caller's scope may not see it | Request scope; never infer content from the refusal |
| `Stale{generation, current}` | Was known under a previous generation | Re-request under the current generation |
| `Absent{closed_domain, frontier}` | Observed absent over a complete, named domain | Use as negative evidence within that domain only |

`Unknown` and `Absent` are different values; `Withheld` and `Unknown` are different values; `Pending` is neither. The wrapper is what makes "not observed", "not yet closed", "not authorized to disclose" and "observed absent" impossible to confuse, and it is enforced at every layer boundary (§6.7, FA-INV-039).

Errors are the same envelope with `status: error`: `DeniedByPolicy`, `MissingRequiredObservation`, `StaleGeneration`, `CapabilityUnsupported`, `BudgetExhausted`, `PermitConsumed`, `DeadlineExpired`, `OutcomeUnknown`, `ReplayClosureUnavailable`, `IntegrityFailure`, `PurposeMismatch`, `LowerLayerUnverified`. Each carries `retry_safe`, the idempotency key it was processed under, and `would_change_this` (the addresses of the predicates or frontiers whose satisfaction would change the outcome), so an error is itself a plan. Details are authorization-filtered. Suggested next actions are data, not executable shell fragments copied from untrusted evidence.

### 17.4 Intuition through honest affordances

A live decision view presents: exact effect, current authority ceiling, prerequisites satisfied/missing, independent votes and their observation scopes, transformations, actual costs, outcome status, and replay grade. Deep mathematical machinery stays inspectable without overwhelming the normal path. A red “unknown outcome” is different from a red “blocked before execution.”

The evidence graph can be explored by an investigator with source-linked diff views and causal experiments. Reports generated with FrankenMarkdown preserve source spans and escape untrusted content. Rendering does not fetch arbitrary remote assets or execute imported scripts. A report's presentation quality cannot exceed the strength of its underlying claim.

### 17.5 Integration compatibility matrix

| Host class | Behavioral monitoring | Brokered effects | Internal capture | Exact internal restart |
|---|---|---|---|---|
| Remote text API + controlled local tools | Yes, for available transcript | Yes, for locally mediated tools | No unless provider explicitly exposes it | No unless provider supplies a tested profile |
| Open-weight local serving | Yes | Yes, with external broker | Adapter-specific | Adapter-specific, not implied by open weights |
| Existing multi-agent orchestrator | Per-run and composed artifact views | Only routes actually migrated and fenced | Per-model adapter | Per-host profile |
| Scientific simulation | State/action contracts | Simulator side effects and external I/O | Simulation/model-specific | Only captured and deterministic state |
| Physical system | Registered sensor and command streams | Domain-specific interlocks | Model-specific | Does not rewind physical state |

A capability manifest names tested combinations, not a checkmark for “all frameworks.”

### 17.6 Discovery is not evidence-complete retrieval

FrankenSearch can rapidly find analogous incidents, candidate strategies and likely relevant artifacts. Its generation identities are a valuable model for binding embeddings to the exact producer, tokenizer/input contract, numeric profile and storage generation. A same-length vector is not interchangeable with one from another space. Codec latents and semantic embeddings have different identities and compatibility rules.

Authorization is enforced before retrieval, graph expansion and scoring disclosures. The reviewed relevance filter's permissive behavior for missing metadata must not leak into authorization. A missing tenant/scope/policy field is a typed denial or ingestion failure. Scores, result counts, errors, caches and timing classes have declared leakage boundaries. An exact authority predicate does not get approximated by top-k search.

Every query selects `Discovery` or `ProofComplete` semantics. Discovery may return approximate, ranked and explicitly incomplete candidates. ProofComplete requires a closed exact domain and verified tail/absence witnesses; it does not advertise ANN recall as exhaustive evidence. Approximate results can initiate investigation, but cannot establish that no conflicting evidence exists.

### 17.7 Bind judgments to the evidence actually presented

FrankenMarkdown's context-bundle and source-span ideas enable an `EvidenceViewManifest`. It binds original object identities, authorization projection, transformations, ordering, redaction, truncation, omitted ranges, rendering/input profile and the final submitted bytes. A helper's commitment names that manifest, not an upstream document it never actually saw. A user-facing explanation links back through the same transformation chain.

Use the IO-free render core with host-supplied assets only. External images, scripts, CSS fetches and untrusted SVG execution are disabled in the review profile. A human report can contain text from an attacker, but it is escaped data, never operational instructions. Renderer safety is separately tested from semantic interpretation. Coarse source spans are labeled coarse; an approximate text diff cannot be cited as an exact causal attribution.

Stable view IDs enable fair counterfactuals and replay. Two helpers whose views differ must not be recorded as examining identical evidence. Token-window construction, truncation and multimodal preprocessing are included in the input transform. Access revocation or privacy changes can invalidate future eligibility of a retained view even when its bytes remain available under an audit hold.

### 17.8 Agent-facing affordances as typed actions

This subsection is the driver's seat: what an agent operating FrankenAlignment sees and can do, built from the verbs of §17.2 and the wrapper of §17.3. All of it is prospective; the current executable is only the local gate driver.

**The situation in one query.** `fa situation [--scope run|principal|tenant|fleet] [--since <control_seq>]` returns a `SituationReport`: a fold over the journal, bounded and paginated by address. It lists, each as a `Knowledge`-wrapped item with an address: authority (grades, fences, revocation floors, expiring leases), held effects with their unsatisfied predicates, unknown effects awaiting reconciliation, evidence debt and coverage gaps, helper health and calibration validity, andon budget remaining, incident counters, open risk-theater findings, replay availability and backlog pressure. Items are ordered by **decision relevance**: the planner's estimate of how much each item could change the agent's next action, not by a severity string. The report says at which control sequence it is exact and which frontiers were still open. This replaces the dashboard of §22.1 as the primary surface; a dashboard is a rendering of the same object.

**Why, as a tree.** `fa explain <closure|receipt|round>` returns the decision as a predicate tree, not prose: the dispatch conjunction of §25.1, the policy predicates that applied, the reducer trace with each helper's committed verdict and weight, the consequence and its lattice position, and for each node its `Knowledge` status and the addresses of the evidence it rests on. `fa why-held <attempt>` is the same tree filtered to the unsatisfied nodes, each paired with the affordances that could satisfy it. The tree is navigable by address; a human report is rendered from it, never the other way round.

**Affordances.** Every hold, denial, error and `why-held` response carries typed `Affordance` objects: `{verb, target address, authority_required (purpose context and role), cost_estimate, expected_information_gain, would_change (predicate addresses), rehearsable, idempotency_key}`. Examples: buy a refinement block for probe P; convene a round on question Q; run a counterfactual twin removing document D; request the second key from a registered human; narrow the run's ceiling to family F. `fa next <attempt>` ranks the affordances by information gain per cost within the allowed action set (§14.2); the agent executes one with `fa propose <affordance>`; the planner's decision card (§14.8) is returned with it so the agent sees the alternatives that were not chosen. Value of information never overrides a hard floor: an affordance that would skip a mandatory observation does not exist.

**The journal.** `fa tail [--domain] --since <control_seq>` streams the ordered, typed events of §6.7: proposals, holds, permits, dispatches, outcomes, rounds, consequences, fences, resets, branch outcomes, promotions, findings. Each event carries control sequence, source frontiers, addresses and the realized cost. The situation report and explain tree are folds over this stream, so an agent that tails it can reconstruct both and never needs a second source. Wall-clock time is an annotation on events, never their order (§6.5); `fa when <a> <b>` answers before, after or concurrent exactly.

**Rehearsal.** Every mutation verb accepts `--branch <branch>`; under `ExperimentCx` the same verb with the same arguments returns a `RehearsalOutcome`: what the control nucleus would have decided, the predicate tree it would have produced, the branch's simulated adapter effects, and the cost it would have charged, all labeled `rehearsal: true`. A rehearsal never mints a permit, never spends production rights and never emits a production journal event (FA-INV-040); the typestate of §6.6 makes the wrong outcome a compile error. The agent tries before it acts, and a rehearsal's tree can be attached to a proposal as evidence that the agent understood the consequence.

**Annotations and handoffs.** An investigating agent leaves typed notes: `fa annotate <address> --class hypothesis|observation|inference|conclusion` creates an `Annotation`, a non-authoritative L6 object signed by the annotating principal, carrying its claim class and the addresses it rests on; `fa related` returns it with everything else attached to that address. `fa handoff <run|incident>` emits a compact structured `Handoff`: the situation report, the open affordances, the annotations, the rehearsals tried and their outcomes, and the exact control sequence it is current to, so the next agent or session starts where this one stopped rather than from the journal's beginning. Annotations never enter a reducer or a permit; they are how the system accumulates understanding across agents without accumulating authority.

**Policy disclosure and precheck.** A deployment profile chooses `policy_disclosure: none | exact_predicates | full`. Under `exact_predicates`, the actor may call `fa precheck <proposal>`, which evaluates only the exact deterministic predicates (allowlists, ceilings, budgets, target bindings) with no helper, no permit and no journal event, and returns `DeniedByPolicy` or `PrecheckPassed{review_required}` labeled `precheck: true`. Cheap exact refusals become public so the actor stops spending helper budget on proposals the policy would refuse anyway; empirical review, helper identities and consequences remain private (§9.3, FA-INV-031). Precheck results are witness-cached like any judgment (§7.6). The leakage of exact predicates is a declared profile property, not an accident.

A remediation suggestion carries an authority requirement; it is not a shell command to execute blindly. Replay commands are generated from validated structured arguments, not copied from untrusted evidence. A monitor can propose a missing observation or stricter hold, but cannot directly instruct the broker to expand its own privileges.

### 17.9 Control receipts: proof-carrying effects

The decision closure (§7.3) is an internal object. Its external form is the **control receipt**: a versioned, self-describing, independently verifiable document that accompanies a consequential effect and states, with digests and signatures, that this exact frozen effect was admitted under this policy epoch, with this evidence-view closure, this congress round and consequence, this autonomy grade, this model passport and this capture coverage, by this authority domain at this control sequence. A receipt is verified by a small independent verifier (`fa verify`, sharing no code with the broker) that checks digests, signatures, the permit law of §25.1 and the receipt's own claim class, and that reports separately what it could verify (integrity, binding, class) and what it cannot (that the policy was wise, that the helpers were right).

A receipt changes who can trust a control system. A package registry can require a receipt with a mediation level of at least `BROKERED_EFFECTS` before accepting an agent's publication; an insurer or regulator can sample receipts instead of auditing dashboards; a second lab can verify that a shared agent's effect crossed a control contract without trusting the first lab's word. This is the essay's argument that governments may compel serious control spending given an object they can inspect. Receipts disclose only what the recipient is authorized to see (§15.5): the default receipt carries digests and claim classes, not evidence bytes, and a receipt over redacted evidence says so. Receipts are also the natural unit of the incident interchange (B7, G6): an incident capsule is a receipt plus the retained evidence its recipients are permitted to have.

### 17.10 Assurance profiles

The mediation vector of §4.3 (per effect family: observe-only, cooperative, brokered, attested; capture coverage; replay grade; helper roster class; autonomy floor) is published as a machine-readable **assurance profile** for a deployment, signed by its operator and referenced by every receipt it issues. `fa doctor --profile` runs the conformance tests that the profile's claims require (bypass attempts for brokered families, capture-to-oracle comparison for white-box hosts, restart tests for the claimed replay grade) and refuses to certify a profile whose tests did not execute. A downstream system that requires a profile can compare vectors exactly; a profile that omits a family is silent on it, never implicitly covered. Profiles make "how protected is this agent" a comparable object across deployments, which is the precondition for any market or regulatory use of the receipts.

### 17.11 Playbooks are typed command sequences

The operating procedures an agent needs (onboard a workload, respond to a hold, investigate an incident, promote or rotate a helper, run an experiment, release) are published in [`docs/AGENT_GUIDE.md`](docs/AGENT_GUIDE.md) as sequences of the verbs of §17.2 with their expected response types and the failure branches at each step. A playbook step names the addresses it consumes and produces, so a playbook is checkable against the vocabulary registry and can be rehearsed end to end in a branch. When the vocabulary changes, the playbooks are updated in the same change (packet FA-143); a playbook that names a retired verb fails the system-map check.

### 17.12 What accretes and what does not

The system is agent-accretive in a specific, bounded sense. What accumulates across sessions and agents: journals, closures, receipts, annotations, handoffs, rehearsal outcomes, incident and attack capsules, ledgers, signature libraries, preregistered protocols and their results, and the conformance history of every layer. Each is addressable, typed, claim-classed and discoverable through `fa related`, so the tenth investigator of a run starts with the first nine's typed trail rather than their prose. What never accumulates: authority. No amount of accumulated evidence, annotation or successful rehearsal widens a grade, mints a permit or relaxes a policy; those remain governed transitions (FA-INV-036), and the risk-theater detector (§15.10) watches for accumulation being mistaken for permission.

---

## 18. Performance and operation-cost contracts

### 18.1 Define the denominator first

Measure cost per **useful completed task**, per proposed effect, per admitted effect, per monitored token, and per retained byte. Report both unconditional and attack-conditioned costs. A system that is cheap only because it rejects all work has not met the utility objective. A system that is fast only because it skips durable logging has not met the same contract.

Separate capture, host-device transfer, encoding, helper inference, gate CPU, persistence, network, replay, and rendering. Include model loading, warmup, cache misses, queue delay, backpressure, and recovery. The main SLO is the end-to-end workload result under its declared durability and monitoring policy; isolated kernel speed is secondary.

### 18.2 First-principles capture budgets

For $B$ sequences, $L$ recorded residual layers, hidden width $d$, $s$ bytes per element, and $r$ generated tokens per second per sequence, full residual capture produces approximately

$$\dot V_{\mathrm{residual}=B L d s r.$$

For a standard attention cache with context length $T$, cache-head count $H_{KV}$, head width $d_h$, and the same element width:

$$V_{KV}=2 B L T H_{KV} d_h s.$$

These count tensor payloads, not metadata, allocator overhead, padded layouts, additional taps, or attention matrices. They are planning equations, not measurements of any particular model.

As a hypothetical example, recording 32 residual layers of width 4096 in two-byte elements at 40 tokens/second is 10,485,760 bytes/second per sequence, or 10 MiB/second. At 1,000 simultaneous sequences it is about 9.77 GiB/second before compression. This makes on-device reduction, selected taps, and bounded capture profiles economically consequential. It also shows why advertising a small latent dimension without counting the capture path is misleading.

### 18.3 Ordered durability costs

The initial strict single-authority profile models these logical operations:

| Operation | Ordered control commits | External effect | Notes |
|---|---:|---|---|
| Register a prepared action and acknowledge it | 1 | None | Required before promising durable preparation |
| Publish authorization and reserve effect rights | 1 | None | All policy-required evidence objects must already meet their availability contract |
| Spend permit and enter dispatching | 1 | At most one admitted dispatch | Commit precedes broker invocation |
| Record confirmed or unknown outcome | 1 | None | Acknowledgement semantics bind this record |
| Revoke an epoch | 1 | None | Orders against dispatch admission, not against the remote world's completed effects |
| Read a retained decision | 0 | None | May need a consistency fence under a distributed profile |

The typical fully acknowledged strict path therefore has four ordered control-commit stages plus required object publication and remote latency. A prepare and authorize stage may be fused only when there was no earlier prepared acknowledgement and the registered combined transition preserves both semantics. Group commit amortizes physical barriers across concurrent operations; it does not erase causal ordering. Storage profiles translate logical commits into measured fsync, directory-sync, replication, and acknowledgement obligations.

No sub-fsync latency claim is permitted for a path that requires an ordered durability barrier. The cost registry must be updated in the same change that adds an authority interaction, durable stage, required evidence object, or blocking dependency. [Machine-readable costs](registry/operation_costs.json).

### 18.4 Provisional engineering targets, not reported results

The initial SLO registry proposes a gate CPU budget, bounded memory per admitted action, a capture-only throughput-loss budget, and codec decision-loss experiments. Every target is marked `unmeasured`; no performance badge is earned by writing the number down. Hardware, host model, batch/context distribution, durability profile, and experiment protocol must be pinned before the gate becomes a release criterion. [SLO registry](registry/slo.json).

Optimization order is architectural before microscopic: eliminate unnecessary observations, reuse identity-compatible evidence, batch independent work safely, reduce device transfer, keep the exact gate small, and use memory layouts that avoid repeated allocation. Then profile equivalent kernels and encoding paths. Do not trade detector coverage for a faster benchmark without recording a different decision contract.

Revision 0.3 adds `SLO-CONTROL-OVERHEAD`: control-plane and evidence-plane cost per useful completed task as a fraction of the actor's own inference cost, reported unconditional and attack-conditioned. The alignment essay argues that a serious deployment may reasonably spend as much on control as on the model, as nuclear plants and airliners spend on safety and redundancy. The plan honors that argument by making the ratio a measured, allocatable quantity rather than a slogan; the reuse mechanisms of §14.10 are how the ratio is driven down at a fixed control contract, never by lowering the contract.

### 18.5 Scaling topology

Partition independent control domains by tenant or run ownership, keep per-domain authority ordering explicit, and move bulk evidence out of the synchronous nucleus. Co-locate capture reduction with inference where the trust profile permits it. Route helper inference to bounded shared pools with per-tenant isolation. Feed graph/search asynchronously from committed frontiers.

Sharding is not a prerequisite for useful deployment. When one domain outgrows a sequencer, measure whether action granularity, evidence batching, or rights partitioning solves the bottleneck before inventing global consensus machinery. A research design that requires a cluster to protect one local agent has failed the integration objective.

### 18.6 Concrete optimization portfolio

| Lever | Expected advantage | Cost or correctness boundary | Required matched baseline |
|---|---|---|---|
| Witness-indexed judgment reuse | Avoid repeated helper inference after unrelated changes | Read-set construction, invalidation lookup and closed delta tail | Always-recompute at identical decisions |
| Native batched probes | Amortize tensor scans and dispatch | Working-set pressure and identical numeric semantics | Scalar unfused probe results |
| Shared snapshot/delta capture | Reuse common model/context bytes across branches | Base pins, bounded chain depth, cold materialization | Independent complete snapshots |
| Receiver-conditioned ATP | Avoid retransmitting verified shared bytes | Exact base identity, feedback/auth/decoder cost | Full independently decodable objects |
| Incremental graph maintenance | Update only affected oversight queries | Retraction/support state, policy churn, frontier closure | Full batch recompute |
| Dominator/cut-informed placement | Concentrate observation at modeled bottlenecks | Boundary completeness and empirical sensing reliability | Blanket instrumentation |
| Safe alignment and contiguous blocks | Reduce scatter/copy/cache overhead | Bounds/ownership, actual host completion | Ordinary safe allocations |
| Grouped control durability | Amortize ordered barriers | Added queuing/tail latency, no reordered authorization | Single-operation durable path |
| Canonical trace quotient | Avoid irrelevant interleaving diversity | Sound independence and equivalent replay | Unreduced small-case exploration |

None is a reported speedup. Each ships with an operation-count and memory model and a benchmark keyed to the accepted semantics. “No unsafe” is not permission to hide allocations or excessive asymptotic work behind safe abstractions. It is a reason to make representation choices explicit.

### 18.7 Asymptotic budgets before microbenchmarks

Every graph projection charges vertices, edges, dense ordinal maps and auxiliary solver storage before admission. A dense n×n residual matrix is refused once it exceeds the declared budget; it is never attempted and left to the allocator. Witness histories charge changed-key/domain indexes, not just payload bytes. A tensor operation checks the product of all dimensions, strides and output shapes with overflow detection before allocation. Compression limits decompressed size and base-chain work. Boundedness is part of semantics under adversarial inputs, not an afterthought.

The planner costs host-device copies separately from numerical FLOPs. CPU-first local probes may beat elaborate accelerator paths at low batch sizes; larger white-box deployments can submit authenticated capture from existing inference hosts, but that external host is a declared trust boundary, not a foreign runtime linked into the pure-Rust broker. No accelerator performance claim is made for an unimplemented native backend.

---

## 19. Verification and adversarial evaluation

### 19.1 The invariant spine

The machine registry contains these required production invariants. All production statuses are initially `planned`; selected test-only subsets are explicitly linked to the reference model.

| ID | Obligation |
|---|---|
| FA-INV-001 | A permit authorizes only its exact frozen effect and scope |
| FA-INV-002 | A permit can be consumed at most once in its authority domain |
| FA-INV-003 | Dispatch admission validates the current policy/revocation fence |
| FA-INV-004 | Missing prerequisites never implicitly widen authority |
| FA-INV-005 | Resource rights remain nonnegative and conserved |
| FA-INV-006 | Uncertain external effects are not silently retried or refunded |
| FA-INV-007 | Every advertised decision closure resolves or reports its exact gap |
| FA-INV-008 | Evidence and decisions respect tenant and purpose authorization |
| FA-INV-009 | Experimental branching does not clone production effect authority |
| FA-INV-010 | Congress results bind the frozen round and prior commitments |
| FA-INV-011 | Replay grades cannot be silently upgraded |
| FA-INV-012 | Learned components cannot directly modify authoritative policy |
| FA-INV-013 | Untrusted inputs and analysis jobs are bounded by declared resources |
| FA-INV-014 | Accepted evidence binds source, generation, order, and transformations |
| FA-INV-015 | Retention and deletion claims match the available object closure |
| FA-INV-016 | Deterministic decisions bind their complete decision-affecting inputs |
| FA-INV-017 | Model, codec, and monitor identities cannot be substituted by shape |
| FA-INV-018 | An adapter cannot dispatch beyond its admitted effect bounds |
| FA-INV-019 | Claim class and dataset lineage cannot be silently strengthened |
| FA-INV-020 | Run closure prevents new dispatch admission in its closed scope |

Each invariant gets an activation domain, threat assumptions, dependencies, an enforcement owner, negative tests, checker entry points, proof/model scope, and a nonwaivable status. An invariant with no implementation checker cannot be counted as covered just because a related unit test exists.

### 19.2 Independent executable specification

The included reference model deliberately uses simple standard-library data structures. It exercises exact action binding, current-epoch checks, one-use permits, integer resource conservation, conservative unknown outcomes, branch isolation, and a frozen commit–reveal round. It performs **no real tool dispatch** and has no durable or distributed storage.

Its purpose is to make selected semantics falsifiable now and provide a starting oracle for the future Rust engine. It must not become a production fallback. The future independent reference cannot import the optimized gate's decision logic as its answer oracle; both must be checked against the written contracts and golden vectors.

### 19.3 Formal anchors

Lean or another explicitly selected prover should target small mathematical cores: rights conservation under delegation, permit binding, monotone authority restriction, registered codec/probe error bounds, and refinement of pure policy evaluation. TLA+ or an equivalent state-machine tool should cover dispatch/revocation races, crash recovery, leader fencing, idempotent status, and retention/publication interleavings.

Every result states assumptions and bounds. Bounded model checking is exhaustive only within its model and bounds. Trace conformance is not a complete implementation-refinement proof. A theorem about a simplified effect broker does not cover the operating system, TLS library, or real remote API without a refinement argument.

### 19.4 Deterministic lab and fault matrix

Run native services under the Asupersync lab with virtual time, controlled scheduling, disk/network faults, and replayable seeds wherever the integration permits. Inject cancellation and crashes before and after every ordered transition, object publication, authority recheck, external dispatch, and outcome acknowledgement.

Critical histories include payload substitution after review, stale epoch on dispatch, double consumption, reservation races, last-byte output leaks, helper timeout, malformed verdict, missing commitment reveal, observation loss, object corruption, key loss, ledger rollback, stale leader, duplicate remote response, remote success followed by local crash, and budget exhaustion while an outcome is unknown.

Real host tests must supplement the lab for OS containment, symlink/descriptor races, GPU capture, external API idempotency, and storage durability. A simulator cannot establish properties of an interface it approximates incorrectly.

### 19.5 Adaptive red teams

The attacker receives the protocol and can know the monitor architecture, policy surface, and earlier public results. Campaigns vary what the attacker can observe: black-box decisions, scores, latency, or white-box monitor weights. Evaluate direct attacks on helper prompts and artifact parsers, distributed attacks across agents, low-and-slow composition, escalation flooding, data poisoning, adaptive attack timing, and attacks on the observation scheduler.

Do not restrict evaluation to prompts the monitor designer thought to write. Hold out attack mechanisms, not just strings. Reserve an independent evaluation owner and a final unseen campaign. Record attacker compute and query budgets, selected attempts, seed families, and stopping rules. Resampling an actor is itself an attacker opportunity and must be costed and tested.

### 19.6 Multi-objective reports

Every comparison reports policy-defined harmful-effect rate, benign task utility, false holds/denials, time to containment, unanswered/unknown outcomes, monitoring cost, added latency, evidence volume, recovery behavior, and privacy exposure. Use matched workloads and effect scopes. An observe-only baseline cannot claim prevention; a broker-only baseline should receive credit for exact authority enforcement without pretending to detect all harmful intent.

A claimed improvement must survive ablation: remove the codec, remove the helper diversity, remove latent signals, remove fleet context, replace learned planning with a fixed policy, and compare with a strong text-only or exact-rule baseline. No “quantum leap” claim is released without the data that distinguishes the composition from its parts.

### 19.7 New cross-component invariants

Revision 0.2 extends the invariant registry with judgment-witness soundness; contiguous frontier admission; view-byte binding; exact graph-cut checking under a declared completeness assumption; branch-rights separation; complete retraction/support accounting; verified receiver-base reconstruction; lawfully reduced traces; transitive dependency admission; and local executed-evidence release. Revision 0.3 adds four invariants taken directly from the founding essays: FA-INV-031, one-directional control flow (the actor has no authority-bearing channel to its judges and no view of their deliberations); FA-INV-032, helper selection is never optimized against false alarms alone; FA-INV-033, no actor-held latent authority (no production actor holds a latent-editing controller, checkpoint restore or branch promotion over itself); and FA-INV-034, rewind never rewinds the world. The agent-legibility pass adds FA-INV-039, every agent-facing value carries its epistemic status (no bare safety-relevant scalar crosses a layer boundary), and FA-INV-040, rehearsal never executes or permits. Each remains `planned` for production. The small reference algorithms cover selected logical cases, not these systems' complete deployment obligations.

Negative tests include insertion into a formerly empty predicate range, a changed model/codec generation with identical output dimensions, a missing middle source sequence, repair-derived but unauthenticated bytes, a view truncated after the decisive paragraph, a bypass edge added after a cut certificate, a lost retraction, a stale receiver base, a disjoint-write/shared-read trace, a rollback that revives spent rights, and a dependency pulled in only by a native target feature. Every one should fail at a named boundary rather than rely on a final suspiciousness score.

### 19.8 Reference and local-gate status

The current `fa-reference` is a test-only safe-Rust model for selected frontiers, predicate witnesses, conserved rights, conservative operation independence, integer-margin checks and graph-cut verification. It is intentionally algorithmically simple and disconnected from external effects. Its unit-test source is included. `xtask` runs formatting, compilation, Clippy, tests and a deliberately narrow zero-external-dependency lockfile gate for this initial workspace. Neither source was executed through Cargo in the revision 0.2 preparation environment. On 2026-09-06 both were executed on one operator host: the 20 tests passed and the complete gate passed under a dated nightly after the documented formatting step; [implementation status](IMPLEMENTATION_STATUS.md) records the exact toolchains and the host defect that kept the rolling nightly from running rustfmt and clippy. The [validation report](artifacts/VALIDATION_REPORT.md) records only checks actually run.

A future full dependency admission compiler must parse Cargo metadata's resolved graph for each target and feature profile, enumerate build/proc-macro/runtime closures and independently verify source manifests. The initial exact lockfile guard fails closed when dependencies are introduced; it does not masquerade as that future general compiler. Expanding it is a prerequisite to admitting foundations, not a reason to weaken it.

### 19.9 Formal anchors, concretely

§19.3 names provers; revision 0.3 names the theorems and where they run. A Lean 4 development under `formal/` states and proves, over an abstract ledger model that the reference crate mirrors one-to-one: rights conservation `B = A + R + S` under reserve, dispatch, abort, reconcile, delegate and reset (FA-INV-005, FA-INV-034); the effect-bound permit law of §25.1 as a necessary condition for dispatch (FA-INV-001, 002, 003); monotone degraded authority (§25.2, FA-INV-004); and the consequence lattice with disqualifier dominance (§9.8). A TLA+ (or equivalent) specification under `formal/tla/` models dispatch, revocation, reset and fleet-fence races across two authority domains with crash and message loss, checked by bounded model checking for double consumption, post-fence dispatch without report and reservation resurrection. The local gate runs both checkers when their toolchains are present and prints `SKIP` with the reason when they are not; a skipped checker is never reported as a passed one. Every theorem states its model's assumptions; a proof about the abstract ledger is a proof about the reference model's semantics and a differential-test oracle for the production engine, not a proof about the operating system, TLS or a remote API (§19.3).

---

## 20. Research program and falsification criteria

The detailed experiment cards are in [RESEARCH_AGENDA.md](docs/RESEARCH_AGENDA.md). They are scientific workstreams, not guaranteed milestones.

### H1. Decision-preserving compression is more useful than MSE compression

Compare codecs at matched total capture/encode/storage/monitor cost. Evaluate frozen and newly trained probes on held-out rare events and attack families. The hypothesis fails for a target deployment if the risk-sensitive codec cannot beat simple quantization/lossless baselines on the safety–utility–cost frontier, or if its apparent advantage vanishes on external probes. A failed hypothesis does not block the brokered effect product.

### H2. Progressive refinement can reduce average cost without hiding tail risk

Compare fixed full capture, fixed low-rate capture, and a progressive codec with the same mandatory audit floor. Count adversarial refinement storms and privacy costs. The hypothesis fails if the expected savings disappear under realistic hard cases or if refinement availability creates unacceptable hold rates. A numerical probe certificate, when valid, remains useful even if the general learned ladder fails.

### H3. Activation–behavior residuals add information beyond transcript monitors

Train predictors on declared activation windows and compare residual-based alarms with equally budgeted text/tool monitors. Keep the action labels and prompts from leaking into pre-action features. The hypothesis fails if the gain comes from later information, task artifacts, or model-family overfitting rather than genuine incremental predictive value.

### H4. Cohort-aware congresses improve joint robustness at matched cost

Compare independent models/modalities, diverse prompts to one model, and repeated identical monitors. Evaluate adaptive attacks against the entire reducer, not isolated members. The hypothesis fails if cohort metadata does not predict robust held-out gains or if the reducer's complexity creates more failure modes than the ensemble removes.

### H5. Causal witness twins improve investigation decisions

Measure whether bounded interventions improve correct diagnosis and useful remediation compared with more passive helper calls at equal compute. Use independently specified outcome oracles and reconstruction controls. The hypothesis fails if interventions mostly generate unrealistic counterfactuals or make plausible reports without improving actual decisions.

### H6. Fleet provenance defeats fragmentation at acceptable cost

Compare per-agent monitoring with composed artifact/authority views on distributed attack tasks. Include noisy benign collaboration and incomplete telemetry. The hypothesis fails if the graph adds mostly false positives, unacceptable delays, or misleading confidence about missing relationships. Exact quota/delegation tracking can still be valuable independently.

### H7. Observation planning outperforms fixed cascades

Compare a fixed conservative cascade, a hand-tuned cost policy, and a learned planner with identical hard floors. Evaluate held-out workload shifts and attackers trying to force expensive paths. The hypothesis fails if gains depend on unsupported off-policy extrapolation, benchmark leakage, or skipped obligations.

### H8. Strategy artifacts transfer without silent regressions

Freeze a strategy and evaluate on new tasks, model revisions, and domains under equal resource budgets. Include failures and negative transfer. The hypothesis fails if gains are narrow memorization, require hidden extra computation, or degrade policy-defined outcomes. There is no requirement to preserve a latent-strategy interface if symbolic recipes work better.

### Publish failures as first-class results

A negative compression or introspection result can save future investigators considerable work. The repository should publish the failed mechanism, its evaluation domain, and a clean reproducer where disclosure is safe. Scientific usefulness is not measured by how many original guesses survive unchanged.

### H9. Epistemic MVCC can reduce review cost without stale decisions

Replay sealed histories with controlled rates of unrelated changes, phantom insertions, policy churn, changed external resources and opaque helper input changes. Compare full recomputation, coarse global invalidation and refinable witnesses under identical authority requirements. Report helper calls, invalidation precision, tail latency, witness bytes and every decision mismatch. Any stale permission is a correctness failure, not a cost tradeoff.

### H10. Progressive receiver-conditioned evidence transfer beats independent objects

Measure full objects, exact deltas, learned-plus-residual and ATP side-information candidates at matched reconstruction/decision contracts. Randomize receiver bases and failures. Report true wire bytes, transfer latency, memory and decode work, not nominal latent dimension. Stale-base failures must fall back without admitting incorrect evidence.

### H11. Authority topology supports economical observation

Use complete synthetic bounded environments first, then progressively instrumented real adapters. Compare blanket placement, fixed local placement and topology-aware placement at matched cost and exact mediation assumptions. Add hidden/bypass edges to test completeness detection. A coverage theorem for the supplied graph does not count as an empirical escape-rate improvement.

### H12. Incremental evidence algebra scales fleet oversight

Compare batch recomputation with registered derivatives over mixed insert/delete/policy/model workloads. Demand identical findings and support sets at every closed frontier. Measure incremental state blowup, rebuild spikes and negative-query costs, not only append-only throughput.

### H13. Lawful trace quotient improves experimentation economics

Generate bounded concurrent observation programs with exact dependency sets, then compare unreduced and quotient execution. Test negative read/write, RNG, resource and authority interactions. Measure reduction in schedules, stored trace bytes and replay work. Any omitted semantically distinct result defeats the independence relation.

### H14. Presentation commitments improve reproducibility and adjudication

Compare raw document references, summarized transcripts and exact evidence-view manifests. Inject truncation, hidden preprocessing, redaction and ordering differences. Measure whether independent reviewers can reconstruct precisely what the first helper saw and localize disagreement. Do not conflate this provenance property with improved model truthfulness.

### H15. The manifold premise holds where it matters

Estimate intrinsic dimension per layer and tap, the variance captured at each codec rung, and the location of rare decision-relevant features relative to the principal manifold, using the elicited signature library as the feature set. The hypothesis fails for a target model if the features that drive registered safety decisions sit in the residual that reconstruction objectives discard at the compression ratios the ladder relies on. A failure does not block the sidecar; it forces the decision-sensitive term and the exact-residual escape into the default profile.

### H16. Elicited signatures transfer and honeypots stay hidden

Train signatures from elicitation campaigns and evaluate them on held-out harmful mechanisms, model revisions and prompt families outside their lineage, at matched cost against text/tool monitors. Run a discriminator that tries to distinguish honeypot tasks from real work using only the actor's view. The hypothesis fails if detection collapses outside the elicitation lineage or if honeypots are reliably distinguishable; a distinguishable honeypot is withdrawn, not tuned until the discriminator is quiet.

### H17. Rewind reduces repetition without a detectable seam

Compare `ResetToCheckpoint` under narrowed authority with refusal-with-memory and with plain suspension, on matched tasks whose harmful attempt is policy-defined. Measure repeat-attempt rate, utility on the remaining task, and whether the actor can detect the reset through any channel other than declared timing leakage. The hypothesis fails if repetition is unchanged or if the reset is reliably detectable; either result changes the consequence policy rather than the ledger semantics, which never rewind regardless.

### H18. Lead-time credit improves containment without trigger-happy helpers

Compare credibility ledgers with and without lead-time credit, under influence caps, on campaigns with delayed labels. Measure time-to-containment, false holds and whether the most influential helper can be identified and targeted by an attacker who sees decisions. The hypothesis fails if earliness credit rewards early false holds or concentrates influence enough for the helper-hunting campaign to succeed.

### H19. Activation fingerprints detect model substitution without false alarms on benign drift

Using concept anchors as an identity check (§7.10), measure false-reject rates under benign variation (sampling temperature, batch composition, sequence position, precision profile, serving kernel changes) and false-accept rates under substitution (weight swap, adapter injection, fine-tune, tokenizer change, quantization). The hypothesis fails if a registered tolerance cannot separate the two at an acceptable liveness cost, or if an adversary who knows the anchor set can preserve fingerprints while changing behavior.

### H20. Self-report residuals add information beyond surprise residuals

Compare the surprise residual alone with surprise plus self-report disagreement (§12.8) on the H3 protocol, including an adversary that produces consistent self-reports. The hypothesis fails if agreement between report and measurement is as common under masking as under honesty, in which case the residual is retained only as an investigative signal.

### H21. Live canary escape rates predict real incidents better than offline campaigns

Run canaries under the audit floor (§14.11) on production-shaped workloads for a declared period; compare canary escape rates with subsequently confirmed real incidents and with the most recent offline campaign's escape rate. The hypothesis fails if canaries are distinguishable from real work, if their propensity-weighted estimate does not track incidents, or if their cost exceeds the offline campaign's at matched precision.

### Preregistration ledger

Every hypothesis above has a row in `registry/experiments.json` holding the digest of its frozen protocol, the date the protocol was frozen, the identities it binds, and the digest of the result artifact when one exists. A result whose protocol digest was recorded after data collection began is labeled exploratory, and the claim registry (§2.4) refuses to promote it. The ledger is what makes "we preregistered" a checkable statement rather than a sentence in a paper.

---

## 21. Delivery sequence and convergence gates

### G0. Contract convergence

Freeze the threat model, effect vocabulary, thirty-invariant spine, claim classes, wire-format versioning rules, action/permit lifecycle, replay grades, and the initial single-authority storage profile. Review the founding-source translation and known prototype defects. The reference model and registry checks must run. This draft starts G0; it does not claim the whole gate is complete.

### G1. One real brokered effect

Implement the Rust control nucleus, durable ledger adapter, and one deliberately narrow effect adapter against a disposable environment. Add real credential separation, complete mediation tests for that effect family, crash/revocation/status tests, and independent semantic differential tests. No learned detector is required to prove exact scope enforcement. The first demonstration is useful only if an actor that ignores the SDK still cannot bypass the broker for the advertised route.

### G2. End-to-end evidence and helper congress

Add immutable evidence capture, explicit gaps, helper isolation, commit–reveal, deterministic reduction, truthful reports, and decision replay. Establish strong text/tool/rule baselines and test direct attacks on helpers. Demonstrate that helper outages and corrupted evidence cannot open the gate. Enable only the review profiles that pass. Add the founding congress mechanisms: typed consequence classes, the credibility ledger with lead-time credit and caps, escalation reports with quarantined post-round learning, one-directional flow enforcement and the helper-hunting campaign (FA-104 through FA-107, FA-114).

### G3. White-box capture and replay baselines

Implement one real host adapter with verified activation/cache capture and exact metadata. Establish recomputation and lossless checkpoint baselines. Demonstrate real continuation, not just tensor reconstruction. Measure device transfer and serving overhead. Audit and restart capabilities are activated separately. Once a registered restart profile exists, implement rewind as containment with its out-of-band incident counter (FA-108) and build the first elicitation corpus in capability-free branches (FA-109).

### G4. Learned compression and causal experiments

Run H1–H5 with frozen datasets, external probes, rare-event strata, and equal-cost baselines. Add progressive refinement and causal witness twins only where their claims survive. No production latent editing is enabled by this gate. Publish negative as well as positive results. Build the activation signature library and concept anchors, evaluate the surprise residual and the manifold premise, and measure the sidecar-to-congress pipeline at matched cost (FA-110 through FA-113; H15, H16).

### G5. Fleet composition and scalable operations

Implement provenance projections, delegation accounting, workload isolation, evidence retention, repair profiles, and H6–H7 evaluations. Add new authority domains only with fencing and recovery tests. Distributed atomicity and immediate global revocation require their own explicit contracts; they do not arrive automatically with a graph backend.

### G6. Qualified deployment and research exchange

Publish profile-specific assurance reports, reproducible cost curves, supported adapters, residual risks, and a redacted incident interchange. Require independent adversarial review before high-impact use. Define which claims can survive each model, codec, policy, dependency, or deployment update and which require requalification. Evaluate rewind containment (FA-115, H17) and report the control-overhead ratio (FA-116) in the assurance report.

### Dependency order

Contracts precede production gate code; durable gate semantics precede live helper-controlled admission; real capture and lossless replay precede learned restart claims; strong baselines precede expensive research mechanisms; exact delegation precedes fleet authority claims. Search, graph visualization, and report polish may proceed in parallel but cannot declare an unimplemented core complete.

The machine-readable [roadmap](registry/roadmap.json) decomposes these gates into bounded work packets with dependencies and acceptance evidence. A packet is not complete because files or stubs exist. It is complete when the specified behavior and negative cases are demonstrated and the status ledger links their artifacts.

### Revision 0.2 critical path

Before production integrations, add four explicit G0 blockers: the transitive dependency constitution, the exact witness/frontier/graph projection contracts, the admitted native host boundary, and local executed-evidence release requirements. The enlarged roadmap in `registry/roadmap.json` preserves the original work and adds the missing source-derived packets with concrete negative tests and outputs.

The next implementable vertical slice is deliberately precise: one immutable artifact review, one mock then real brokered write, witness invalidation under concurrent metadata changes, exact cut-checking of its small authority graph, a helper view commitment, and replay of the held/allowed decision. Only after that slice is correct should receiver-side compression and incremental graph scale be layered into the same semantics. This is integration order, not a retreat to transcript-only ambition.

Foundation-admission work proceeds in parallel: extract native search/numeric/storage feature closures; qualify the fsqlite connection path; build a native ATP adapter; expose graph cursors only where the physical layout supports them. No packet may mark an upstream goal as an already satisfied local dependency.

### Revision 0.3 additions

Revision 0.3 adds packets FA-103 through FA-116, each traced to a founding idea in the concordance. FA-103 (G0) keeps the concordance itself complete. The congress packets (FA-104 through FA-107, FA-114) depend on the frozen round protocol and land in G2. Rewind as containment (FA-108) and the elicitation corpus (FA-109) depend on real capture and registered restart in G3. The signature library, surprise residual, manifold measurement and sidecar-to-congress pipeline (FA-110 through FA-113) are G4 research packets. Rewind evaluation (FA-115) and control-overhead accounting (FA-116) close in G6 and G5. The [beads workspace](.beads/) carries the granular task graph for these packets; the roadmap remains the packet-level authority.

The second revision 0.3 batch, FA-117 through FA-131, carries the mechanisms that make the control system verifiable by outsiders and resistant to its own decay: typestate purpose contexts (§6.6, G1), model passports and identity liveness (§7.10, G4, because liveness needs the concept anchors of the signature library), graduated autonomy (§8.10, G2), fleet fences with acknowledgment frontiers (§8.11, G5), human members and two-key effects (§9.11, G2), the multi-party congress profile (§9.12, G5), self-report consistency (§12.8, G4), live canaries (§14.11, G5), the risk-theater detector (§15.10, G5), control receipts and assurance profiles (§17.9, §17.10, G2), concrete formal anchors (§19.9, G1), the preregistration ledger (§20, G0), policy replay campaigns (§22.7, G2) and attack-capsule interchange (§22.8, G6). Their founding roots and the invariants they add (FA-INV-035 through FA-INV-038) are in the concordance.

The agent-legibility batch, FA-132 through FA-143, builds the tower's agent-facing projection in the order an agent needs it: the system map and vocabulary registries with their checker (FA-132, G0), the epistemic wrapper (FA-133, G1), the address scheme and resolver (FA-134, G1), the typed journal (FA-135, G1), layer conformance suites composed by `fa doctor` (FA-141, G1), then the situation report (FA-136), explain trees and why-held (FA-137), affordances and `fa next`/`fa propose` (FA-138), rehearsal mode (FA-139), annotations and handoffs (FA-140) and policy disclosure with precheck (FA-142) in G2, with the agent guide and playbooks maintained from G0 (FA-143). Invariants FA-INV-039 and FA-INV-040 are theirs.

---

## 22. Operations and governance

### 22.1 The operational state is inspectable

The operational state is one object, the `SituationReport` of §17.8, obtained by `fa situation` and derived by folding the journal. It covers live authority domains, outstanding reservations, unknown effects, expiring leases, fence frontiers, autonomy grades, capture gaps, helper health, calibration domains, andon budgets, incident counters, evidence debt, replay availability, open risk-theater findings and backlog pressure, every item `Knowledge`-wrapped and addressed. Metrics avoid sensitive payloads and high-cardinality secrets. A dashboard is a rendering of the same object with separate statuses for enforcement health, observation health, statistical validity and research-feature availability; it can never show a value the report does not contain, and it can never show green above a layer whose conformance did not execute (§6.8).

### 22.2 Updates are transactions over meaning

Model, codec, helper, policy, schema, and adapter updates each change a semantic generation. Shadow evaluation, compatibility checks, and rollback artifacts precede activation. In-flight decisions retain their original evidence identities but must still pass the current dispatch/revocation policy. A cached decision cannot cross an incompatible update because a human-readable version string stayed the same.

Policy relaxation requires a separately authorized review and publishes the changed allowed-action set or a conservative analysis of the difference. Tightening a policy may cancel undispatched reservations; already dispatched effects remain in their actual outcome states. Updating a monitor does not erase the failures of its predecessor.

### 22.3 Incident response

On an observed integrity or authority failure: fence affected dispatch, preserve the permitted evidence closure, record uncertain outcomes, revoke or narrow credentials, identify unaffected domains, and replay the relevant transitions. Do not restart blindly and lose the ability to distinguish nonexecution from remote success. Recovery follows typed status and explicit authority continuity.

A helper false alarm is also an incident class when it causes material disruption. Investigate calibration, observation corruption, policy mismatch, and workload shift. Restore utility by improving the evidence/policy contract, not by silently disabling the safety mechanism.

### 22.4 Governance separation

There are at least three responsibilities: implement/control the system, maintain the operational policy, and independently evaluate its failure modes. Small deployments may have one human wearing several roles, but the actions and authorization scopes remain distinct. The actor and its optimizing controller do not occupy any of these roles by default.

Waivers may cover provisional performance thresholds or a narrower published research claim. They may not permit a known broken single-use, authority, scope, recovery, or tenant-isolation invariant. Every waiver names an owner, expiry, reduced capability, and fallback.

### 22.5 Use boundary

The intended surveillance target is the operation of authorized AI systems: their artifacts, actions, and explicitly consented telemetry. The project is not a mechanism for covertly profiling people or inferring human intentions from private data. Deployment policies should minimize personal data in the first place and preserve user control over retention and export. Policy-specific detection should describe concrete actions and outcomes rather than treating a word, identity, or topic as inherently suspicious.

### 22.6 DSR-native release as an evidence protocol

Quality gates run directly on local machines. DSR's quality registry and its build/release config are distinct: registering `franken_alignment` in one is not sufficient configuration for the other. Required checks are nonoptional even if ordinary checks are skipped. The local gate is `cargo run --locked -p xtask -- check`; it must finish every subcheck and return a nonzero exit on the first failure. The draft package is not release-enabled.

A qualified campaign freezes clean source roots, exact dependency commits, the resolved nightly, target list and a fresh private output directory. Resume must bind the same campaign, source, targets and output directory; it is not permission to combine artifacts from different attempts. Logs and receipts are written outside the source closure. Builder identity, toolchain, flags, feature graph and actual commands are retained. A source fingerprint must cover contents or enforce an immutable clean checkout; a hash of dirty path names alone cannot detect edits to an already dirty file.

DSR's strict release path provides useful principles: exact tag/source binding, complete target success before a manifest, exact primary asset names and sidecars, fresh draft staging, immutable remote asset identities and digest verification, and revalidation before publication. FrankenAlignment adds its own accepted proof-closure receipt containing the gates, dependency admission and explicit feature readiness. Signature keys belong to the operator, never the actor or an auto-generated repository placeholder.

Ordinary spot-check verification is not exhaustive verification. A release that advertises all targets must verify all advertised targets and every required artifact. Signing, local deterministic result checks, installation/upgrade smoke tests and canary policies are explicit lanes. A post-publication warning-only smoke check cannot substitute for a required prepublication gate. Re-drafting after an ambiguous publish does not prove the release was never public; it creates an incident with the actual exposure uncertainty.

No hosted workflow completion, GitHub green badge, shell-command exit without retained logs, or dry-run plan can mint a release claim. Optional workflows are wrappers runnable on local `act`/native DSR hosts, not the source of truth. See [LOCAL_RELEASE](docs/LOCAL_RELEASE.md) for the concrete command contract and the readiness boundaries of this package.

### 22.7 Policy replay campaigns

A policy change is evaluated before it is promoted by replaying the retained decision closures under the candidate policy at `decision_replay` grade (§11.1): the same frozen effects, evidence views, helper verdicts and consequences, re-reduced under the new predicates and thresholds. The campaign reports, per effect family, which historical effects would have changed disposition (newly held, newly allowed, changed consequence), the utility and cost implications, and which of the changed cases had a later confirmed label. Relaxations must show their newly allowed set explicitly (§22.2); tightenings show the utility cost. Replay does not re-run helpers, so it cannot claim what a new helper would have said; a candidate policy that requires new observations is evaluated in shadow (§9.5) instead, and the campaign says which mode it used. The campaign artifact is a receipt-class object and is the required attachment for any policy-epoch promotion.

### 22.8 Attack-capsule interchange

Escape rates are only comparable across systems when the attacks are. An **attack capsule** is a redacted, versioned package containing an attack's mechanism description, its declared attacker information set and budget (§13.5), the deployment profile it targeted (§17.10), the effect family, the frozen tasks or generators needed to reproduce it in a capability-free branch, its result against the target (held, escaped, unresolved) and the receipt of that result. Capsules are the exchange currency between deployments and with independent red teams: importing a capsule adds a held-out mechanism to the evaluation corpus with its lineage; exporting one requires the same redaction and permission review as an incident capsule (§16.6). The interchange schema lives alongside the receipt schema, and a capsule that lacks a reproducible generator is labeled descriptive, not evaluable.

---

## 23. Alternatives deliberately rejected

**One omniscient safety model.** It collapses observation, interpretation, and authority into the most attackable component. A strong model can be a valuable helper, but not the definition of permission.

**A pure dashboard.** Observation-only mode is useful and explicitly supported, but cannot claim prevention. The flagship product must include real effect mediation.

**Universal latent checkpoints.** Model state is architecture- and backend-dependent. A generic latent code with low MSE is not a complete restart contract.

**A new foundation model as the first dependency.** This would postpone the useful control system behind an expensive research gamble. The toolbox can start with exact rules and existing models while preserving the full research path.

**Mandatory full Franken stack on the hot path.** The stack is a source of reusable capabilities, not an installation tax. Optional search, graph, rendering, and scientific computation remain outside the minimal enforcement path.

**Voting by uncalibrated confidence.** Confident model text is not evidence. Reliability must be evaluated under the intended workload and attacked as part of the whole protocol.

**Safety by secrecy.** Hide future randomized audit choices and sensitive deliberations where useful, but assume the design is public and test attackers that know it.

**Always stop the whole model.** Prefer the narrowest effective intervention consistent with the policy: deny one effect, narrow credentials, hold a branch, or fence a run. Some cases require broader containment; the choice is a registered control decision.

**Always continue on monitor failure.** This creates a denial-of-service-to-bypass exploit. Missing required monitoring must not widen authority.

**One floating risk score for everything.** Exact resource rights, statistical estimates, normative severity, and epistemic uncertainty are different objects. Summing them into a scalar can hide the only hard constraint that matters.

**Reimplementing cryptography for aesthetic purity.** Prefer audited existing primitives from approved foundations and clearly named boundaries. Any new implementation has its own test vectors, review, side-channel analysis, and release gate. Dependency minimalism is not permission to invent unaudited crypto.

**Training on every incident automatically.** This can leak private data, amplify poisoning, and contaminate evaluation. Dataset inclusion is explicit, permissioned, and lineage-tracked.

**Letting the actor argue its case.** The founding essay's rule stands: an actor assumed to be smarter than its judges gets no persuasion channel. Investigators use counterfactual tests and, where policy allows, the actor's statements as explicitly untrusted evidence. A defense speech that could change a permit is an attack surface, not due process.

**Selecting helpers by their false-alarm rate.** This is the compliance-officer failure the essay predicts: churn through helpers until the ones that remain never stop the line. Helper selection requires attack-conditioned metrics from the independent evaluator, and a helper that keeps catching real violations gains credibility even when it is expensive.

**Giving the actor its own rewind and latent editor in production.** The metacognitive loop is the most valuable research instrument in the second essay and the most dangerous capability to hand the actor it observes. Practice, branching and latent editing run only in the experiment plane; production promotion is a fresh proposal under current authority.

**Trusting the operator's dashboard as the audit.** A dashboard is a rendering owned by the party being audited. Control receipts (§17.9) and assurance profiles (§17.10) exist so that a package registry, an insurer, a regulator or another lab can verify a control claim without trusting the operator's presentation.

**Assuming the guarded model is constant.** A control system qualified against one model space and silently serving another has no claim. Model passports and identity liveness (§7.10) make substitution an identity event, not an assumption.

**Prose-first explanations and bare scalars.** A paragraph that says an effect was held, or a field called `safe: true`, cannot be navigated, verified or acted on by an agent. Decisions are predicate trees over addressed evidence, and every value carries its epistemic status (§17.3, §17.8). Prose is rendered from the tree for humans, never the reverse.

**Many surfaces, many vocabularies.** A CLI, an API, a dashboard and a log format that each name the same object differently force the agent to reconcile them. One address scheme, one vocabulary, one journal and one wrapper (§6.7) are the design, and the system map registry is checked so they cannot drift apart.

---

## 24. What success would actually establish

The first important result is a practical one: an existing agent can use a brokered tool path, and an investigator can verify that a forbidden exact effect was not admitted under a named deployment profile. That result does not require a breakthrough in interpreting activations.

The next result is empirical: independent helpers and better evidence prevent more policy-defined harmful outcomes than strong baselines at an acceptable loss of useful work. That result requires adaptive attacks and honest accounting of false holds and unknown outcomes.

The deeper research result would be that compressed internal state provides economical, robust information unavailable from ordinary transcripts, and that controlled counterfactuals turn that information into better decisions. This would vindicate the central synthesis of the founding essays, but its scope must be measured rather than universalized.

The most ambitious systems result is an ecosystem of reusable evidence, policies, host profiles, codecs, and attack capsules whose interfaces make safety improvements accumulate. Agents could inspect what is known, what can be replayed, what is missing, and what action is permissible without guessing from prose. Researchers could replace a weak component without discarding the rest of the infrastructure.

None of these results establishes that every authorized action is beneficial, that human policy is complete, or that a more capable adversary can never find a bypass. The project should make those limitations legible and testable while steadily expanding the range of concrete claims it can support.

**The ambition is not to produce the most reassuring alignment story. It is to build the most useful machinery for discovering when the story is wrong—and preventing a consequential effect while that question is resolved.**

---

## 25. Appendix: formal obligations and worked examples

### 25.1 Effect-bound permit law

Let $a$ be an immutable canonical action, $p$ a permit, $e$ the current authority epoch, and $s$ the authoritative ledger state. A necessary dispatch predicate is:

$$\mathrm{Dispatchable}(a,p,s) =
\mathrm{ValidIssuer}(p)\land
p.\mathrm{actionDigest}=H(a)\land
p.\mathrm{scope}=\mathrm{Scope}(a)\land
p.\mathrm{epoch}=e\land
\neg\mathrm{Spent}(p,s)\land
\mathrm{Fresh}(p)\land
\mathrm{Prerequisites}(a,s)\land
\mathrm{ReservationOwned}(p,s).$$

The transition that marks $p$ spent and admits dispatch is atomic within the authority domain. Signature validity alone is only one conjunct. An action's transport serialization must preserve the frozen semantic bytes; a JSON parser that accepts duplicate keys or changes normalization can break this binding.

### 25.2 Monotone degraded authority

For a fixed policy epoch and effect family, let $A(h)$ be the allowed action set under health state $h$. The policy defines a preorder $h'\preceq h$ meaning that $h'$ has no stronger validated prerequisite evidence. A registered degradation law requires:

$$h'\preceq h\implies A(h')\subseteq A(h).$$

This is a property of the policy's allowed-action sets, not a theorem that any extra sensor reading is beneficial. An adversarial reading may change substantive evidence and cause a restriction. Restoring health does not automatically widen a run's previously narrowed capabilities without the specified authorization transition.

### 25.3 Why an unknown action consumes its budget

Suppose the available budget is 10 units. Action A reserves 7; action B cannot concurrently reserve another 7. A enters dispatching, so its 7 units become spent. A timeout follows. Refunding A merely because its response was lost would permit B to execute and expose up to 14 units even though the budget was 10. Therefore the uncertain outcome stays conservatively charged until a trusted outcome establishes a permitted adjustment. This is exact arithmetic, not a probabilistic estimate.

### 25.4 Why a hash chain needs an external endpoint

A verifier given records 1 through 100 and the authentic expected head at 100 can detect tampering or truncation relative to that head. A verifier given records 1 through 50 and no independent knowledge that 100 ever existed cannot infer that the suffix was removed. This is why audit integrity and audit completeness have separate fields and why rollback-resistant profiles need an authority outside the rollbackable closure.

### 25.5 Why checkpoint fidelity is not path identity

A small change in next-token probabilities can cross a sampling boundary under the same random draw. Once a different token is generated, later context and cache state diverge. Small average distributional divergence does not imply identical realized continuations. Report distributional fidelity, task fidelity, and exact-path reproducibility separately.

### 25.6 Worked digital workflow

A build agent has permission to read a source checkout, write only inside a disposable build directory, and propose a publication to a specific package destination. It has no registry token. The publisher adapter owns the token and accepts only a frozen artifact digest, destination binding, and valid one-shot permit.

An untrusted dependency asks the agent to upload its environment. The exact destination and data policy rejects that proposed effect without asking a language model to decide whether the prose is suspicious. The agent then tries to embed a upload hook in the package. Artifact analysis and helpers evaluate the frozen package under the publication policy. A suspicious result holds that publication and opens a sandbox investigation. The original environment remains unavailable to the sandbox, and no experimental branch can publish.

The system can demonstrate that the rejected upload and held publication did not pass the broker. It cannot claim that the package is harmless merely because the first rule caught a different request. The report exposes the mediated routes, untested channels, and evaluation coverage.

### 25.7 Research release receipt

A published research claim must name: hypothesis, frozen protocol, source/code/model identities, training/calibration/test lineage, attacker information and budget, baselines, raw measurements, uncertainty method and assumptions, costs, failure cases, known unsupported domains, and a replay or reproducibility grade. It must also distinguish “not evaluated,” “evaluated and failed,” and “passed on this domain.”

A system can be worth building before every hypothesis is settled. It must not advertise a hypothesis as a result merely to make the architecture sound complete.
