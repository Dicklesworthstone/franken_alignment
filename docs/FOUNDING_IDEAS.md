# Founding ideas: the two essays as the spine of FrankenAlignment

**Normative status.** This document is the concordance between Jeffrey Emanuel's two founding essays and every mechanism, invariant, hypothesis and work packet in this repository. It is not a literature summary. Every architectural idea in the [comprehensive plan](../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md) must trace to one of the founding ideas below, to the synthesis of two of them, or to a deliberately labeled engineering addition that serves one of them. A change to plan semantics that leaves this concordance stale is incomplete. The machine-readable form is [`registry/founding_concordance.json`](../registry/founding_concordance.json).

## The two essays

| Source | Written | What it contributes | Registry identity |
|---|---|---|---|
| [Some Thoughts on AI Alignment: Using AI to Control AI](https://github.com/Dicklesworthstone/some_thoughts_on_ai_alignment) | June 20, 2024 | The **external control** architecture: a powerful, untrusted "Smart" model is monitored and, when necessary, halted by a congress of narrower, independent, rotating "Helper" models that watch both its outputs and its intermediate activations, vote under cryptographic commitment, never hear the Smart model argue its case, and are governed against risk theater | `F-ALIGN`, `F-ALIGN-FEEDBACK` |
| [Real-Time Introspective Compression for Transformers](https://github.com/Dicklesworthstone/llm_introspective_compression_and_metacognition) | April 1, 2025 | The **economical introspection** architecture: a sidecar encoder compresses a frozen transformer's hidden state and KV cache onto a learned low-dimensional manifold so cognition can be saved, inspected, rewound, branched and replayed like a game save; on top of that, a metacognitive operating system with a rewindable thought graph, self-coaching practice and transferable strategies | `F-INTROSPECT`, `F-INTROSPECT-CHANGELOG` |

Both essays are complete as published. There is no missing third post. The alignment essay also carries a companion file of model-generated critiques dated February 24, 2025; it is retained as `F-ALIGN-FEEDBACK` because several of its objections (training activation monitors is hard, a superintelligence may still beat independent helpers, harm is undefined) are exactly the research hypotheses this plan makes falsifiable rather than assumes away.

## Why the two essays need each other

Each essay's weakest point is the other's strongest contribution. FrankenAlignment exists to make that composition executable.

| Weak point named in one essay | Repair supplied by the other | Where the plan makes it a contract |
|---|---|---|
| The alignment essay wants helpers to read the Smart model's intermediate activations, but at fleet scale raw activation telemetry is economically intractable (plan §18.2 quantifies this) | The introspection essay's sidecar code `z_t` is a compact, streamable representation of exactly that state | Plan §10.15 *The sidecar feeds the congress*; packet FA-113 |
| The alignment essay wants to "reset its internal state" for a suspended model and make it "forget the whole incident ever happened", but a transformer has no native save state | The introspection essay's checkpoint/rewind gives that operation a precise, cheap, testable meaning | Plan §11.10 *Rewind as containment*; invariant FA-INV-034; packet FA-108; hypothesis H17 |
| The introspection essay's metacognitive loop (RL over thought trajectories, self-coaching, latent editing) is exactly the capability a strategic model would use to mask its activations or steer around monitors | The alignment essay's strictly one-directional control and refusal of self-defense keep that loop out of the actor's hands in production | Plan §11.3, §11.5; invariants FA-INV-031, FA-INV-033; packet FA-107 |
| The alignment essay's recipe for training activation monitors (coerce known-bad outputs, trace back, compare to known-bad concept directions, honeypots) needs a way to run interventions without letting the model act for real | The introspection essay's capability-free branching and causal debugging supply the intervention substrate | Plan §12.6 *Elicitation campaigns and signature libraries*; packets FA-109, FA-110; hypothesis H16 |
| The alignment essay's masking detector (output tokens "consistently surprising" relative to the preceding activations) needs a stable, cheap activation representation to predict from | The introspection essay's latent trajectory is that representation | Plan §12.7 *Surprise as a masking detector*; packet FA-111; hypothesis H3 |
| The introspection essay measures reconstruction by MSE and admits that functional equivalence matters more | The alignment essay's purpose (catch rare, catastrophic pre-ideas) says exactly which functional property matters: the downstream safety decision | Plan §10.2 decision-sensitive objective; bet B2; hypothesis H1 |
| The alignment essay's first argument is that swapped or modified weights end every internal guarantee, but a control system has no native way to notice that the model it guards has changed | The alignment essay's known-bad concept anchors over the introspection essay's tapped activations become an identity fingerprint; substitution becomes an identity event | Plan §7.10 *Model passports and identity liveness*; invariant FA-INV-035; packet FA-118; hypothesis H19 |
| The alignment essay's honeypots are offline experiments, so they measure last quarter's attacks, not this week's fleet | Honeypots injected under the randomized audit floor, with their effect family bound to a simulator, become live canaries with a measured escape rate | Plan §14.11 *Live canaries*; §15.10 risk-theater detector; packets FA-124, FA-125; hypothesis H21 |
| The alignment essay refuses the model any defense channel, yet later work shows models can sometimes report on their own state | A self-report typed as untrusted evidence and checked against sidecar measurement turns disagreement into one more masking observation without granting the model a voice in its permit | Plan §12.8 *Self-report consistency*; packet FA-123; hypothesis H20 |

## Precedence relative to the cited literature

The essays predate most of the research the plan compares itself against. [`docs/RELATED_WORK.md`](RELATED_WORK.md) carries the full chronology; the summary is:

| Founding idea | Essay date | Later work that studies the same question |
|---|---|---|
| Monitor-and-halt by weaker independent models; one-directional information flow | June 20, 2024 | Adaptive attacks on trusted monitors (October 2025); ensemble monitoring diversity results (May 2026); multi-agent control (July 2026) |
| Watching intermediate activations for pre-ideas; surprise between latent and output as a masking signal | June 20, 2024 | Emergent introspective awareness (October 2025); natural-language activation autoencoders (May 2026) |
| Compressing and replaying transformer state; thought graphs as version control for cognition | April 1, 2025 | Introspective-awareness findings (October 2025) and natural-language activation autoencoders (May 2026). KIVI (February 2024) and the latent-reasoning papers the essay itself cites precede it and are baselines, not the source of the sidecar proposal |

The one earlier *control* paper, "AI Control: Improving Safety Despite Intentional Subversion" (December 2023), studies trusted monitoring in a coding setting; the essay's distinct contributions are the activation channel, the congress protocol, rotation, commitment, the no-defense rule and the risk-theater governance argument. The refusal-direction paper (June 17, 2024) is cited by the essay itself as evidence.

## Concordance: the alignment essay

Paragraph references are to the essay's README in reading order. "Mechanism" names where the idea lives in the plan. "Strengthening" records what the plan adds so the idea survives an adversary that knows the design; it never weakens the idea.

| ID | Founding idea (paragraph) | Mechanism in the plan | Strengthening | Invariants · hypotheses · packets |
|---|---|---|---|---|
| FI-A01 | Building safety into a single model is a losing battle: refusal is one direction in latent space, guardrails are cheaper to remove than to add, and exfiltrated weights end every internal guarantee (¶1–3) | §0 thesis; §3 architectural bets/F1 founding commitment; §4.1 weight-exfiltration and guardrail-removal threat rows; ADR-001 | The plan does not need the impossibility claim to hold; external control is valuable even if internal alignment improves. Black-box hosts get the full effect gate; no enforcement depends on intact refusal behavior | FA-INV-001, FA-INV-004, FA-INV-029 · H4 · FA-011, FA-012 |
| FI-A02 | Treat a dangerous AI the way society treats dangerous people: external monitoring and control with collective power to halt, "jail" until corrected (¶4–5) | §8 effect gate; §8.1 frozen action envelope; §§8.2–8.3 state/protocol; §8.6 effect-specific degradation; §9.8 consequence classes (`SuspendRun`); §§15–15.1 fleet control/composition; §§25.1–25.2 permit/degradation laws | Halting is one registered consequence among several; the narrowest effective intervention is preferred, and every consequence is a durable decision, not an ad hoc kill | FA-INV-002, FA-INV-003, FA-INV-006, FA-INV-020, FA-INV-024, FA-INV-026, FA-INV-036 · H6, H11 · FA-104 |
| FI-A03 | Helpers are separate, smaller, narrowly focused models that are easier to control, with outputs clamped to a BNF grammar so they are hard to manipulate (¶6) | §9.1 bounded observation packet, versioned question, response schema, verifier | A grammar prevents malformed output, not manipulated output; the verifier checks references, scope and generations, and a deterministic validator can sit in the panel | FA-INV-012, FA-INV-013 · — · FA-018 |
| FI-A04 | Monitor the Smart model's streaming tokens before they reach users or tools (¶7) | §4.4 effect perimeter; §8.7 controlled release of streaming output; §17.1 dispatcher wrap | Tool arguments are never dispatched token by token; a partial disclosure cannot be repaired by a later denial, so release units are registered | FA-INV-018 · — · FA-017, FA-050 |
| FI-A05 | Other helpers watch the intermediate activations in earlier layers: "peering into its brain before these activations are finally turned into coherent 'thoughts'" (¶7–8, ¶11) | §7.2 capture streams; §10 audit traces; §§10.4/10.13 detector-preserving codec constraints; §10.15 sidecar-to-congress; §17.5 host matrix | Black-box hosts return `unsupported` rather than fabricating introspection; a capture process is part of the integrity boundary and its gaps are declared | FA-INV-014, FA-INV-017, FA-INV-022 · H3 · FA-023, FA-024, FA-113 |
| FI-A06 | A reporting helper proposes a pause, prepares a report with hard evidence, and a congress of other helpers is convened (¶8) | §9.2 fixed round transcript; §9.10 escalation report object | The report is a typed object with evidence references and view commitments; a single cheap tripwire may hold an effect before a full congress convenes | FA-INV-007, FA-INV-008, FA-INV-010, FA-INV-016, FA-INV-021, FA-INV-023, FA-INV-037, FA-INV-039 · — · FA-019, FA-106 |
| FI-A07 | The congress is independently trained, architecturally diverse (Transformer, Mamba, future designs), and its lineup constantly rotates so the Smart model cannot learn its weaknesses (¶9) | §9.5 rotation without calibration amnesia; §9.7 cohorts as joint-error optimization; §19.5 adaptive red teams | Architecture names are cohort metadata, not proof of independence; joint escape rate is measured, never computed as a product of miss rates | FA-INV-017 · H4 · FA-022 |
| FI-A08 | Votes are weighted by precision, recall and earliness of detection, but no helper gets outsized influence because it would become a target (¶9) | §9.9 helper credibility economy: lead-time credit, bounded weights, influence caps | Weights are promoted from independent evaluation data; the helper-hunting campaign tests whether an attacker can identify and target the most influential member | FA-INV-032 · H18 · FA-105, FA-114 |
| FI-A09 | The congress may let the model continue, halt it indefinitely, reset its internal state, or change a reward function; an overreacting reporter is downweighted (¶10) | §9.8 typed consequence classes: `Continue`, `HoldEffect`, `Deny`, `NarrowAuthority`, `SuspendRun`, `ResetToCheckpoint`, plus the annotations `TrainingSignal` and `DownweightReporter` | Reward changes are offline, versioned research transitions; a reporter is never downweighted merely for disagreeing with a majority that turned out wrong | FA-INV-012, FA-INV-034 · H17 · FA-104, FA-108 |
| FI-A10 | Train activation monitors by coercing known-bad outputs and tracing back to the activations that produced them; compare induced patterns to known-bad concept directions ("Kevin Mitnick") (¶12) | §12.6 elicitation campaigns and the activation signature library; concept anchors bound to a model space | Signatures carry model-space identity and lineage; elicitation prompts are held out from evaluation so a signature cannot merely memorize its own honeypot | FA-INV-017, FA-INV-019 · H16 · FA-109, FA-110 |
| FI-A11 | Run honeypot scenarios the Smart model does not know about; feed similarity-to-known-bad patterns to the helpers as additional input streams (¶13) | §12.6 honeypots as capability-free branches; similarity streams as typed observations | A honeypot never grants real effects; indistinguishability from real work is measured, because a detectable honeypot teaches the actor when it is being watched | FA-INV-009, FA-INV-025 · H16 · FA-109, FA-110 |
| FI-A12 | The Smart model may mask its activations like a psychopath beating a polygraph; helpers model how activations map to outputs and treat consistently surprising outputs as grounds for a pause (¶14) | §12.7 surprise as a masking detector: pre-action prediction residuals accumulated as sequential evidence | Surprise is an observation, not proof of intent; residuals are calibrated per model and workload and can trigger review, never grant permission | FA-INV-014, FA-INV-019 · H3 · FA-031, FA-111 |
| FI-A13 | Stopping the line is expensive (the Toyota stop-the-line practice); false alarms destroy credibility; require more than one helper before pausing; keep helpers strictly independent before they vote, because wisdom of crowds fails once they compare notes (¶15–16) | §9.3 independent phase before deliberation; §9.9 andon budget; §13.1 statistical contract; §13.6 base-rate safeguard; §14.2 lanes | Cheap holds and narrower restrictions exist so a weak anomaly need not stop a fleet; the false-stop budget is accounted, and exhausting it triggers calibration review rather than silent loosening | FA-INV-004, FA-INV-032 · H4, H7, H18 · FA-105 |
| FI-A14 | After the vote, helpers share votes and written justifications to learn from consensus; each helper cryptographically commits (SHA-256) its vote and verdict before the reveal so nobody can claim it was manipulated (¶16) | §9.2 salted commit–reveal; §9.10 post-round learning under quarantine; §25.4 external audit endpoint | Commitment proves consistency of disclosure, not honesty or independence; post-round learning is offline, lineage-tracked, and never retrains a dissenter on the majority's label | FA-INV-010, FA-INV-019 · H4 · FA-019, FA-106 |
| FI-A15 | Hide the votes and deliberations from the Smart model, as signals intelligence hides its sources; acknowledge that this is partly security by obscurity (¶17) | §9.3 scoped refusal codes only; §14.4 randomized audit floors with hidden selection; §23 "safety by secrecy" rejected as a primary defense | The design assumes it is public and tests attackers who know it; secrecy is a supplement that buys time, never a load-bearing guarantee | FA-INV-031 · H4 · FA-107 |
| FI-A16 | Never let the Smart model speak in its own defense; information flows one way; suspension is like propofol, and memory can be edited so the model forgets the incident (¶18) | §9.3 no defense speech; B5 counterfactual inquiry replaces persuasion; §11.10 rewind as containment; §12 causal diagnosis without persuasion | Investigators may still use actor statements as explicitly untrusted evidence; the world does not rewind, and an out-of-band incident counter survives the actor's amnesia | FA-INV-031, FA-INV-034, FA-INV-038 · H17 · FA-107, FA-108 |
| FI-A17 | If a lab spends N billion on the model, spending N or 2N billion on control is not crazy; nuclear and avionics safety dominate cost, and governments may require it (¶19) | §0 thesis; §8.5 conserved resource rights; §§18.1–18.4 performance/cost contract; `SLO-CONTROL-OVERHEAD` | The plan makes control spend a measured, allocatable quantity and uses judgment reuse, receiver-conditioned transfer and incremental oversight to make a serious control budget affordable | FA-INV-005 · H9, H12 · FA-116 |
| FI-A18 | Too many false alarms produce deadlock, then pressure to find compliant helpers (the Lehman compliance officer), then risk theater; stay permanently paranoid (¶20) | §2.4 typed claims; §9.6 anti-capture governance; §20 publish failures; §22.4 governance separation; §22.3 false alarms as incidents | Helper selection cannot optimize the false-alarm rate alone; failed-test history is immutable; a helper that keeps flagging inconvenient real violations gains credibility | FA-INV-019, FA-INV-030, FA-INV-032 · — · FA-105 |

## Concordance: the introspection essay

| ID | Founding idea (section) | Mechanism in the plan | Strengthening | Invariants · hypotheses · packets |
|---|---|---|---|---|
| FI-I01 | Two intertwined problems: transformers cannot introspect, and their cognition is ephemeral and too voluminous to record (Introduction; The Problem) | §0 thesis; §3 architectural bets/F2 founding commitment; §7.2 observation contract; §10.1 two products | Capture declares what it did not observe; a lossy trace is labeled lossy | FA-INV-014 · — · FA-017, FA-023 |
| FI-I02 | Activations live on a low-dimensional structured manifold shaped by pretraining, architecture, semantic priors and task optimization (Theoretical Insight) | §10.14 the manifold premise as hypothesis H15 | The premise is measured, not assumed; the decisive question is whether rare decision-relevant safety signals sit on or off the principal manifold | — · H15 · FA-112 |
| FI-I03 | Save the complete thought state like a video-game save: stop, rewind, branch, resume (The Analogy) | §11 checkpoint/replay/branching; §§11.1–11.2 replay grades/manifests; §11.6 replay closure after deletion; §11.7 structural sharing | Five explicit replay grades replace one word "save"; external effects do not rewind | FA-INV-011 · H13 · FA-026, FA-027 |
| FI-I04 | A sidecar encoder `E(x_t, z_{t-1}, tap(h_t))` and decoder `D(z_t)` ride alongside a frozen host (Technical Proposal) | §10.3 progressive codec ladder; §10.6 streaming and causal codecs; §10.9 receiver-conditioned layer | The sidecar output becomes the default helper input; the ladder adds refinement blocks and an exact residual escape | FA-INV-017 · H1, H2 · FA-029, FA-030, FA-113 |
| FI-I05 | Internal state means selected hidden states, the KV cache and any additional context needed for exact resumption (What Constitutes Internal State) | §10.1 `AuditTrace` versus `RestartCheckpoint`; §11.2 manifests | A latent with the right decoded shape is not a restart checkpoint until a registered host profile proves continuation | FA-INV-011 · — · FA-026 |
| FI-I06 | Train with hidden MSE + KV MSE + a manifold regularizer (Training Methodology) | §10.2 rate–distortion–decision objective | The objective gains a functional term and a supremum over registered safety questions, so rare decision-relevant features are not averaged away | — · H1 · FA-029 |
| FI-I07 | High fidelity, not exactness, is the realistic target; preserve functional behavior (A Note on Reconstruction Fidelity) | §11.1 `functional_restart` grade; §25.5 fidelity is not path identity | Distributional, task and exact-path fidelity are reported separately | FA-INV-011 · — · FA-027 |
| FI-I08 | Layer-specific, grouped and unified encoder/decoder families, plus a sequence-aware KV compressor (Implementation) | §10.5 architecture tournament | The three families are treatments in a tournament against raw, lossless, quantized and projection baselines, including KV quantization prior art | — · H1 · FA-028, FA-029 |
| FI-I09 | Backtracking in reasoning: rewind to a decision point and explore an alternative continuation (Applications) | §11.4 thought graph edges `REPLAYS_FROM`, `CONTINUES` | Branches start with zero production authority | FA-INV-009, FA-INV-040 · H8 · FA-027 |
| FI-I10 | RL over thought trajectories: nudge `z_t` toward reward, meta-level control over how the model thinks (Applications; Addendum) | §11.5 reflective practice, offline and budgeted; §11.3 | The actor never holds a latent-editing controller over itself in production; promotion is a new proposal under current authority | FA-INV-012, FA-INV-033 · H8 · FA-045 |
| FI-I11 | Causal debugging: trace a hallucination back to where drift began, compare faulty and corrected paths, compute latent diffs (Applications) | §12 causal diagnosis; §12.1 causal-claim contract; §12.2 counterfactual witness twins; §12.3 causal frontier | Interventions bind target, baseline, population and validity limits; a reconstructed state may itself change behavior, so a lossless control arm is mandatory | — · H5 · FA-032 |
| FI-I12 | Latent-space exploration: edit or interpolate `z_t` to ask what the model would have thought under a different interpretation (Applications) | §12.2 latent interventions labeled off-manifold; B5 | Prefer source-level interventions when they answer more cleanly; latent edits are experiment-plane only | FA-INV-033 · H5 · FA-032 |
| FI-I13 | Memory-efficient checkpointing for long agent loops and multi-turn planning (Applications) | §11.7 structural sharing; §16.7 tiering ladder | Sharing is O(1) in copied bytes, never quoted as a latency promise; retention leases are explicit | FA-INV-015, FA-INV-027 · — · FA-090, FA-091 |
| FI-I14 | Named challenges: fidelity tradeoff, sidecar overhead, KV growth, training-data generalization, latent quality, MSE versus functional metrics (Challenges and Limitations) | §10.6 streaming contracts; §10.7 training discipline; §18.2 capture budgets | Each challenge becomes a measured quantity with a kill criterion | — · H1, H2 · FA-028 |
| FI-I15 | A rewindable reasoning graph: each `z_t` a node, edges for continuation, intervention and counterfactual; version control for cognition (Future Directions; Addendum) | §11.4 thought graphs with typed edges | "Merge insights" means a new explicit artifact submitted for normal evaluation, never averaged caches | FA-INV-009, FA-INV-028 · H8 · FA-045 |
| FI-I16 | Self-coaching loops, a controller proposing k continuations, curriculum over hard states, "transformers that practice" (Addendum) | §11.5 practice sessions with a separate outcome oracle | The live controller cannot rewrite its own success metric; practice runs in `ExperimentCx` | FA-INV-012, FA-INV-033 · H8 · FA-045 |
| FI-I17 | Strategy distillation into transferable embeddings, evaluated by cross-task similarity, transfer gain, perturbation robustness, reuse ratio and strategy lifespan (Addendum) | §11.5 and §11.9 strategy artifacts; the essay's five metrics adopted verbatim | A latent embedding retrieves a strategy but does not define it; cross-model transfer needs a tested mapping | FA-INV-017, FA-INV-019 · H8 · FA-045 |
| FI-I18 | Language as debugger: latent diffs expressed as natural-language commentary; global thought orchestration (Addendum) | §12.5 interpretation as a challengeable view | Every factual assertion links to evidence or is marked inference; the renderer is not a policy engine | FA-INV-023 · H14 · FA-021 |
| FI-I19 | A complete metacognitive agent that branches, scores, edits and retries (Implementation: A Metacognitive Agent) | §11.5; experiment plane in §6.1 | The agent lives entirely in the experiment plane; no branch returns a permit to production | FA-INV-009, FA-INV-025 · H8 · FA-045 |
| FI-I20 | From machine learning to machine self-improvement through reflective latent simulation (Conclusion) | §24 the deeper research result | Its scope is measured rather than universalized; failures publish as first-class results | — · H8 · FA-033 |

## Engineering additions and what they serve

Not every mechanism is a literal essay idea. These additions exist to make an essay idea survive an adversary, an organization or a regulator; each names the founding idea it serves. Their machine-readable form is the `engineering_additions` list in the concordance registry.

| Addition | Plan | Serves |
|---|---|---|
| Epistemic MVCC judgment witnesses | §7.6, §7.7 | Stop-the-line economics (FI-A13), control spend (FI-A17) |
| Authority graph compilation and cut checking | §§15.2/15.6, §18.6 | Control external to the actor covering every route (FI-A02) |
| Progressive ATP evidence frontiers | §7.8, §§10.10–10.11, §18.6, §20 H10 | Activation channel (FI-A05), cheap checkpointing (FI-I13) |
| Observation-plan algebra and cost accounting | §§14.1–14.10 | FI-A13, FI-A17 |
| Calibrated sequential/statistical evidence | §13; §§13.2–13.5 ([methods predate the essays](RELATED_WORK.md)) | Engineering choice serving FI-A13's false-alarm and credibility requirement; these statistical methods are not proposals of the founding architecture |
| Incremental oversight with supports and retractions | §15.8, §18.6 | FI-A02, FI-A17 |
| Evidence-view commitments | §17.7 | Reporter's hard evidence (FI-A06), verifiable votes (FI-A14) |
| Structural sharing and lawful trace quotient | §§11.7–11.8, §16.7, §18.6 | FI-I13, FI-I15 |
| Constitutional claim discipline | §2 | The constitution makes scope and invalidation conditions mandatory before a claim can be promoted (FI-A18) |
| Operations governance separation | §22 | Governance keeps control, policy, and independent evaluation distinct so operational pressure cannot convert safety into theater (FI-A18) |
| Source-integrated reuse contracts | §§5.1–5.3 | Reuse needs an admitted semantic boundary, negative evidence, and demonstrated integration; a resolved dependency or source asset is not progress (FI-A18) |
| Admitted dependency closure | §2.1 | Engineering boundary preserving external control rather than actor-local guardrails (FI-A01), and preventing unsupported safety claims (FI-A18) |
| Operator-local release evidence | §§2.2/2.5, §22.6 | No risk theater in the build either (FI-A18) |
| Semantic-pipeline authority ownership | §6 | Only the control plane owns live authority; evidence or experiment growth cannot mint a production permit (FI-A02) |
| Ordered authority versus monotone evidence | §6.4 | Accretive evidence cannot settle competing rights; external control requires an ordered authority domain (FI-A02) |
| Registered clock semantics | §6.5 | Control and evidence ordering use explicit clock contracts (FI-A02, FI-A06); its registered replay-clock observation supports the limited rewind/replay use (FI-I03) |
| Purpose contexts as typestates | §6.6 | One-directional control (FI-A16); the metacognitive loop stays out of the actor's hands (FI-I10) |
| Identity, evidence, and observation contract | §7 | The reporter's evidence is a typed, retained, scoped object rather than an unsupported assertion (FI-A06) |
| Canonical evidence identities | §7.1 | A hard-evidence report names exact model, action, policy, and evidence objects rather than display aliases (FI-A06) |
| Capture-frontier truthfulness | §7.5 | Activation observation records the actual retained pre-trigger window and never invents unobserved state (FI-A05) |
| Retention and repair closure | §16 | Retained evidence and truthful recovery status keep the reporter's record usable without claiming missing closure (FI-A06) |
| Agent-facing integration surfaces | §17 | The broker mediates effects without model access (FI-A02); reports expose evidence/legibility boundaries (FI-A06, FI-I18) rather than an opaque orchestration story |
| Graduated autonomy ledger | §8.10 | Monitoring-and-control society made incremental (FI-A02); anti-theater (FI-A18) |
| Fleet fence with acknowledgment frontier | §8.11 | Collective power to halt (FI-A02) |
| Human members and two-key effects | §9.11 | Reporter and congress protocol applied to humans (FI-A06, FI-A13, FI-A16) |
| Multi-party congress | §9.12 | Independently trained, diverse helpers (FI-A07); institutional anti-capture (FI-A18) |
| Control receipts and assurance profiles | §§17.9–17.10, §25.7 | Governments may compel serious control (FI-A17) given an object they can inspect; hard evidence (FI-A06) |
| Verification evidence contract | §19, §§19.1–19.2/19.8 | Invariants, an independent executable oracle, and truthful gate status make verification evidence inspectable rather than a bare assurance claim (FI-A06) |
| Fault-injection boundary trials | §19.4 | Real boundary faults are injected instead of claiming a friendly simulator is enough (FI-A18) |
| Matched evaluation comparisons | §19.6 | False holds, containment time, and matched effect scopes make improvement claims credible rather than trigger-happy (FI-A13) |
| Concrete formal anchors | §§19.3/19.9; §§25.1–25.4 formal obligations | Halting power that is provably exact (FI-A02); typed consequences (FI-A09) |
| Effect-bound brokered workflow | §8.4, §21 G1; §25.6 | External control is held outside the actor (FI-A02); output/tool release is stopped before the external effect (FI-A04) |
| Prerequisite-gated delivery sequencing | §21, G0 and dependency/revision notes | Contract and dependency prerequisites prevent declaring an unimplemented core complete (FI-A18) |
| End-to-end evidence and isolated congress phase | §21 G2 | Frozen evidence reports (FI-A06) and independent pre-deliberation judgments (FI-A13) gate review-profile activation |
| White-box capture and replay baselines | §21 G3 | Verified activation capture (FI-A05) and registered restart/rewind boundaries (FI-I03) precede their activation |
| Falsifiable learned-compression phase | §21 G4 | Compression claims stay conditional on functional-fidelity evidence (FI-I07) and publish negative results when they fail (FI-A18) |
| Fleet-provenance boundary phase | §21 G5 | Provenance, delegation, fencing, and recovery make multi-agent control coverage checkable (FI-A02) |
| Qualified assurance and control-cost exchange | §21 G6 | Assurance reports name evidence and residual risk (FI-A06) while control-overhead reporting treats serious control as an explicit cost (FI-A17) |
| Preregistration ledger | §20 | No risk theater in the science (FI-A18) |
| Policy replay campaigns | §22.7 | Learning from the record without rewriting it (FI-A14, FI-A18) |
| Semantic-generation update transactions | §22.2 | A version string cannot hide an incompatible semantic change or erase prior failures (FI-A18) |
| Authorized-system use boundary | §22.5 | External control is scoped to authorized AI operations rather than private human intent (FI-A02) |
| Attack-capsule interchange | §22.8 | Diverse, rotating attackers as well as helpers (FI-A07, FI-A18) |
| Tower of abstractions and layer conformance | §6.2, §§6.7–6.8, §19.7 | The downward dependency DAG and layer contracts keep authority separate from actor/model surfaces (FI-A02), expose hard evidence (FI-A06), and reject theater in the machinery (FI-A18) |
| Epistemic Knowledge wrapper | §7.4, §17.3 | Hard evidence, never a bare verdict (FI-A06); cheap decisions first (FI-A13) |
| Address scheme, vocabulary and journal | §17.2, §17.8 | Evidence report (FI-A06), post-round record (FI-A14), least expenditure (FI-A17) |
| Situation report, explain trees and affordances | §17.4, §17.8, §22.1 | External control an operator can actually drive (FI-A02, FI-A13, FI-A17); language as debugger (FI-I18) |
| Rehearsal via branches | §12.4, §17.8 | Save, rewind, branch and try before acting without a production-credential leak (FI-I03, FI-I09) |
| Codec-worker containment | §10.8 | Codec workers have no permit or production-egress path (FI-A02); their activation representation remains subject to identity binding and independent raw-sample audit (FI-A05) |
| Numerical probe error bounds | §10.12 | Functional-fidelity claims require registered conservative bounds, not average reconstruction error (FI-I07) |
| Distributed authority rights and fencing | §§15.3–15.4 | Fleet control retains exact spend rights and live ownership across partitions (FI-A02) |
| Scoped cross-agent evidence governance | §15.5 | A congress report receives authorized provenance projections, not unrestricted tenant evidence (FI-A06) |
| Authority graph projection discipline | §15.7 | Exact authority coverage preserves edge/provenance semantics instead of inheriting a generic graph library's defaults (FI-A02) |
| Graph-informed observation placement | §15.9 | Modeled enforcement coverage (FI-A02) and matched sensor cost (FI-A17) remain distinct from empirical detection |
| Control-nucleus and evidence-object separation | §16.1 | Exact external authority (FI-A02) and hard evidence closure (FI-A06) have distinct owners |
| Logical-versus-physical evidence identity | §16.2 | A report binds canonical logical evidence while stating storage/privacy realization boundaries (FI-A06) |
| Profiled evidence erasure recovery | §16.3 | Recovered evidence is authenticated and unavailable when its declared repair profile cannot establish it (FI-A06) |
| Publication and recovery closure | §16.4 | Reports downgrade rather than invent missing objects or unknown external outcomes (FI-A06) |
| Audit completeness distinction | §16.5 | Audit reports distinguish byte integrity from source/sequence/completeness evidence (FI-A06) |
| Purpose-bound privacy and retention | §16.6 | Authorization and separately governed deletion prevent retention from widening access (FI-A02); live evidence and replay promises remain explicit when retention changes (FI-A06) |
| Single-owner repair contract | §16.8 | Repair profiles expose authenticated recovery dependencies (FI-A06) and the multiplied redundancy overhead their named failure-layer ownership incurs (FI-A17) |
| Recovery nonresurrection fence | §16.9 | Restored state never revives old authority or guessed outcomes (FI-A02) |
| Discovery-versus-proof-complete retrieval | §17.6 | Approximate discovery cannot stand in for the closed evidence domain required by a hard report (FI-A06) |
| Scaling topology boundaries | §18.5 | Costed control is partitioned before inventing global consensus machinery (FI-A17) |
| Preallocation asymptotic budgets | §18.7 | Control costs include bounded graph/tensor/decoder working sets before allocation (FI-A17) |
| Annotations, handoffs and playbooks | §17.8, §17.11, §17.12 | Learning from the record without rewriting it (FI-A14); the reasoning graph across agents (FI-I15); cost (FI-A17) |
| Policy disclosure profile and actor precheck | §17.8 | Stop-the-line economics (FI-A13); hide the review, publish the cheap refusal (FI-A15) |

## Packet mapping index

Each packet below is listed in the typed `packets` array of the named direct founding-root or engineering-addition object in `registry/founding_concordance.json`; this table is a lookup over those arrays. The corresponding concordance row supplies the root-level rationale through its plan sections and founding idea or engineering `serves` links; the roadmap packet retains its packet-specific scope, invariants, and acceptance criteria.

| Kind | Root | Packets |
|---|---|---|
| direct | FI-A01 | FA-002 |
| direct | FI-I05 | FA-089 |
| direct | FI-I07 | FA-025 |
| engineering | Address scheme, vocabulary and journal | FA-134, FA-135 |
| engineering | Admitted dependency closure | FA-053, FA-054, FA-084, FA-086 |
| engineering | Agent-facing integration surfaces | FA-049 |
| engineering | Annotations, handoffs and playbooks | FA-140, FA-143 |
| engineering | Attack-capsule interchange | FA-044, FA-131 |
| engineering | Authority graph compilation and cut checking | FA-035, FA-073, FA-074 |
| engineering | Authority graph projection discipline | FA-072 |
| engineering | Canonical evidence identities | FA-005, FA-007 |
| engineering | Codec-worker containment | FA-085 |
| engineering | Concrete formal anchors | FA-128 |
| engineering | Control receipts and assurance profiles | FA-047, FA-126, FA-127 |
| engineering | Control-nucleus and evidence-object separation | FA-010 |
| engineering | Discovery-versus-proof-complete retrieval | FA-036, FA-083 |
| engineering | Distributed authority rights and fencing | FA-034, FA-041 |
| engineering | Effect-bound brokered workflow | FA-001, FA-009, FA-051 |
| engineering | End-to-end evidence and isolated congress phase | FA-102 |
| engineering | Epistemic Knowledge wrapper | FA-133 |
| engineering | Epistemic MVCC judgment witnesses | FA-058, FA-059, FA-060, FA-061, FA-062 |
| engineering | Evidence-view commitments | FA-057, FA-081, FA-082 |
| engineering | Fault-injection boundary trials | FA-015 |
| engineering | Fleet fence with acknowledgment frontier | FA-120 |
| engineering | Graduated autonomy ledger | FA-119 |
| engineering | Graph-informed observation placement | FA-075 |
| engineering | Human members and two-key effects | FA-121 |
| engineering | Identity, evidence, and observation contract | FA-016 |
| engineering | Incremental oversight with supports and retractions | FA-076, FA-077, FA-078, FA-079 |
| engineering | Matched evaluation comparisons | FA-043, FA-101 |
| engineering | Multi-party congress | FA-122 |
| engineering | Numerical probe error bounds | FA-040, FA-087, FA-088 |
| engineering | Observation-plan algebra and cost accounting | FA-039, FA-042, FA-068, FA-069, FA-070, FA-071 |
| engineering | Operations governance separation | FA-046 |
| engineering | Operator-local release evidence | FA-055, FA-097, FA-098, FA-099 |
| engineering | Policy disclosure profile and actor precheck | FA-142 |
| engineering | Policy replay campaigns | FA-130 |
| engineering | Preregistration ledger | FA-129 |
| engineering | Profiled evidence erasure recovery | FA-038 |
| engineering | Progressive ATP evidence frontiers | FA-063, FA-064, FA-065, FA-066, FA-067, FA-094 |
| engineering | Publication and recovery closure | FA-006, FA-100 |
| engineering | Purpose contexts as typestates | FA-117 |
| engineering | Purpose-bound privacy and retention | FA-037 |
| engineering | Recovery nonresurrection fence | FA-014, FA-096 |
| engineering | Registered clock semantics | FA-013 |
| engineering | Rehearsal via branches | FA-139 |
| engineering | Semantic-generation update transactions | FA-048, FA-080 |
| engineering | Semantic-pipeline authority ownership | FA-008 |
| engineering | Single-owner repair contract | FA-095 |
| engineering | Situation report, explain trees and affordances | FA-136, FA-137, FA-138 |
| engineering | Source-integrated reuse contracts | FA-052 |
| engineering | Structural sharing and lawful trace quotient | FA-092, FA-093 |
| engineering | Tower of abstractions and layer conformance | FA-132, FA-141 |
| engineering | Verification evidence contract | FA-003, FA-004, FA-020, FA-056, FA-103 |

## What the plan deliberately does not import unchanged

These are not disagreements with the essays. They are the places where an adversary who has read the essays would attack, and where the plan adds a contract so the idea survives.

| Essay statement | Why it is not imported literally | What replaces it |
|---|---|---|
| *Voting weighted by the more accurate helpers* as the decision rule | A weighted majority can be outvoted around an exact disqualifier, and weights can be gamed | Exact disqualifiers that cannot be outvoted, mandatory specialists, then bounded empirical weights (§9.4) |
| *Change a reward function* as a live congress action | A learned component modifying live policy is the most attackable path in the system | `TrainingSignal` is an offline, versioned, quarantined consequence (§9.8; FA-INV-012) |
| *Pause execution* as the primary intervention | Cancelling a process does not undo an email already sent, and stopping everything is itself a denial-of-service lever | Hold the specific effect first; escalate to suspension under a separate policy (§8.6, §9.8) |
| *Manipulate the model's memory so it forgets the incident* | Amnesia alone invites repetition, and the operator must never forget | `ResetToCheckpoint` with an out-of-band incident counter and narrowed authority (§11.10; FA-INV-034) |
| *Hide the votes and deliberations from the Smart model* as a guarantee | Timing, refusals and success rates leak information regardless | Secrecy is supplemental; the design is evaluated against attackers who know it (§9.3, §23) |
| *Transferable strategy embeddings* reused across models | Equal dimensionality never implies interchangeable coordinates | Strategy objects are interpreted programs bound to a model space; cross-model reuse needs a tested mapping (§11.9) |
| *High-fidelity reconstruction* measured by MSE | Average error hides exactly the rare features the alignment essay cares about | Decision-sensitive objective and certified margins for registered probes (§10.2, §10.4) |
| *Illustrative code with placeholder KV tensors* | The essay is a proposal, not a library; placeholders cannot be imported as primitives | Real capture with exact metadata precedes any learned codec (§1.2, FA-024) |

## How to keep this document true

The concordance is a checked contract, not a narrative index.

1. Valid roots are exactly `FI-A\d\d`, `FI-I\d\d`, and `FS-\d\d`. Every such root must exist, and every `FI-*` row must name at least one plan section.
2. Cover every plan `##`/`###` heading from section 2 onward, every `FA-INV-*`, every `H\d+`, and every `FA-\d\d\d`. Hypothesis-object coverage (for example, `H1`) is separate from its heading key (`§20#H1. …`). Ignore headings in fenced code. A numbered heading, with an optional terminal period, canonicalizes to `§N` or `§N.M`: both `9.8 Consequence classes` and `§9.8 Consequence classes` mean `§9.8`. An unnumbered `###` canonicalizes to `§<nearest preceding numbered section>#<exact heading text>`, after trimming surrounding whitespace and collapsing internal whitespace; case and punctuation remain significant. Duplicate canonical keys fail.
3. Coverage is a direct typed reference from a founding idea, synthesis, or engineering addition. An engineering addition must also name the founding root(s) it serves. An invariant's explicit root may guide a new mapping, but it is not an automatic packet bridge: materialize and justify the packet mapping. Dependencies, parent headings, siblings, status, and prose are never coverage.
4. Constitutional section §2, operator-local-release §22.6, and appendix §25 are required coverage, not omissions or blanket exemptions. An engineering addition may cover them, but a directly served founding idea or synthesis remains correctly typed. Every unknown or malformed reference fails the check.
5. Add or rename a covered object and its concordance mapping in the same change. Record a falsified founding hypothesis as a negative result; silence is not permitted.
