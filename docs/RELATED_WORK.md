# Related work and comparison obligations

**Verified-source cutoff: September 6, 2026.** This is a selective primary-source comparison, not an exhaustive survey. Unless otherwise noted, the retrieved arXiv abstracts and metadata were reviewed; full-paper experiments were not reproduced. Dates distinguish original submission from revision.

## The founding essays come first

The architecture originates in two essays by Jeffrey Emanuel, and most of the literature below was published after them. The chronology matters for honest comparison in both directions: later work that studies the same questions is evidence about the essays' proposals, not their source.

| Date | Work | Relationship |
|---|---|---|
| October 18, 2018 and July 15, 2021 | Confidence sequences (Howard et al.); conformal prediction (Angelopoulos and Bates) | Statistical methods the plan uses; not proposals of this architecture |
| December 12, 2023 | AI Control (Greenblatt et al.) | Precedes the essays; studies trusted editing and untrusted monitoring of an intentionally subversive model in a coding setting |
| February 5, 2024 | KIVI (Liu et al.) | Precedes the essays; a KV-quantization baseline for the sidecar tournament, not a monitoring or replay proposal |
| June 17, 2024 | Refusal is mediated by a single direction (Arditi et al.) | Cited by the alignment essay three days later as evidence that internal guardrails are brittle |
| **June 20, 2024** | **Some Thoughts on AI Alignment: Using AI to Control AI (Emanuel)** | **Founding essay.** External control by a congress of narrower, independent, rotating helpers watching outputs and intermediate activations; commit–reveal voting; no self-defense channel; honeypot elicitation of activation signatures; surprise as a masking detector; stop-the-line economics and risk-theater governance |
| February 24, 2025 | Model-generated critiques of the alignment essay (retained as `F-ALIGN-FEEDBACK`) | Objections that this plan turns into hypotheses H3, H4, H16 |
| **April 1, 2025** | **Real-Time Introspective Compression for Transformers (Emanuel)** | **Founding essay.** Sidecar compression of hidden state and KV cache onto a learned manifold; save, rewind, branch and replay; rewindable thought graph, self-coaching practice, strategy distillation |
| October 10, 2025 | Adaptive attacks on trusted monitors (Terekhov et al.) | Studies attacks the alignment essay anticipated when it required rotation, influence caps and one-directional flow |
| October 29, 2025 | Signs of introspection in LLMs (Anthropic) | Later evidence on the introspective ability the introspection essay proposed to instrument directly |
| May 7, 2026 | Natural-language activation autoencoders (Transformer Circuits) | Later interpretable-latent work; a baseline for the sidecar tournament |
| May 14, 2026 | Ensemble monitoring: diverse signals outweigh compute (Koran et al.) | Later evidence for the essay's architecture-diverse congress; the plan still measures joint failure rather than assuming it |
| submitted July 8, 2026 | Multi-agent control: distributed attacks (Makins et al.) | Later evidence for the fleet-level composition problem the plan addresses in §15 |

The plan does not claim that the essays anticipated every component technique. It claims them as the origin of the architecture and records the chronology so that neither the essays nor the later papers are miscredited.

| Primary source | Relevant contribution and limit | Required FrankenAlignment comparison |
|---|---|---|
| [AI Control: Improving Safety Despite Intentional Subversion](https://arxiv.org/abs/2312.06942v5), Greenblatt et al.; submitted December 12, 2023, revised July 23, 2024 | Studies control protocols for intentionally subversive models in a coding setting, including trusted editing and untrusted monitoring | Compare matched control protocols; do not claim that using weaker helpers or separating trust is new |
| [Refusal in Language Models Is Mediated by a Single Direction](https://arxiv.org/abs/2406.11717v3), Arditi et al.; submitted June 17, 2024, revised October 30, 2024 | Finds a refusal-mediating direction in the tested model family; illustrates the brittleness of that refusal mechanism | Separate refusal classification from authorization and other harms; do not generalize to impossibility of all internal alignment |
| [KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache](https://arxiv.org/abs/2402.02750v2), Liu et al.; submitted February 5, 2024, revised July 25, 2024 | Uses different quantization structures for keys and values to reduce serving memory | Include simple and established KV quantization baselines; benchmark monitoring fidelity separately from ordinary task quality |
| [Adaptive Attacks on Trusted Monitors Subvert AI Control Protocols](https://arxiv.org/abs/2510.09462v2), Terekhov et al.; submitted October 10, 2025, revised March 2, 2026 | Studies attacks that target the monitor through actor-generated content under protocol-aware threat models | Give attackers the protocol and test the entire monitoring/reducer path, including resampling strategies |
| [Ensemble Monitoring for AI Control: Diverse Signals Outweigh More Compute](https://arxiv.org/abs/2605.15377v2), Koran et al.; submitted May 14, 2026, revised May 18, 2026 | Reports gains from diverse monitoring signals over homogeneous ensembles in tested coding tasks | Compare cohort-aware panels with strong diversity baselines at equal cost; do not treat architecture metadata as an independence proof |
| [Multi-Agent AI Control: Distributed Attacks Hamper Per-Instance Monitors](https://arxiv.org/abs/2607.07368v1), Makins et al.; July 8, 2026 | Studies coordinated attacks split across agents and reports a fragmentation effect in a synthetic lab setting | Evaluate composed artifacts, delegation, and fleet context against per-agent baselines; disclose missing cross-agent observations |
| [Time-uniform, nonparametric, nonasymptotic confidence sequences](https://arxiv.org/abs/1810.08240v9), Howard et al.; submitted October 18, 2018, revised August 6, 2022 | Develops time-uniform uncertainty methods under stated conditions | Record conditional assumptions, optional-stopping scope, multiplicity, and error-budget lifetime rather than borrowing the terminology alone |
| [A Gentle Introduction to Conformal Prediction and Distribution-Free Uncertainty Quantification](https://arxiv.org/abs/2107.07511v6), Angelopoulos and Bates; submitted July 15, 2021, revised December 7, 2022 | Introduces conformal uncertainty methods and extensions | Distinguish marginal coverage under a sampling contract from an individual adversarial safety guarantee |

## Introspection and interpretable compression

[Signs of introspection in large language models](https://www.anthropic.com/research/introspection), October 29, 2025, describes context-dependent and unreliable abilities to notice or report aspects of internal state. This motivates experiments, not unrestricted trust in self-report. The associated primary research is [Emergent Introspective Awareness in Large Language Models](https://www.transformer-circuits.pub/2025/introspection/index.html).

The primary-source search excerpt for [Natural Language Autoencoders Produce Unsupervised Explanations of LLM Activations](https://www.transformer-circuits.pub/2026/nla/index.html), published May 7, 2026, describes a learned activation-to-text and text-to-activation reconstruction approach. It is directly relevant prior art for interpretable latent interfaces. **Read-scope limitation:** the full page fetch exceeded the tool's content limit; only the retrieved primary-source excerpt was used. No quantitative claim or detailed assessment of that paper is made here.

The initial experiment program should include interpretable and dense-code baselines. Legibility, numerical fidelity, downstream safety information, and privacy leakage are separate metrics. A natural-language reconstruction should not automatically become an authenticated explanation of intent.

## What is—and is not—claimed as new

This plan does not claim invention of runtime authorization, monitor ensembles, commit–reveal, causal intervention, activation probes, KV compression, or replay. Its proposed contribution is the shared architecture: exact effect-bound authority, explicit observation validity, progressive decision-sensitive compression, challengeable causal/replay artifacts, and total-cost optimization within fixed safety obligations.

A claim of empirical superiority requires matched deployment scope, attacker budget, utility, and resource accounting. A claim of scientific novelty requires a deeper literature review for the particular submitted result. Neither is earned by calling the toolbox “general” or by combining established components in a diagram.
