# Threat model

## Protected object

The primary protection target is a **specific mediated external effect under an operator-defined policy**. Broader beneficial behavior is an empirical and normative objective, not something a capability token alone proves. Authorized actions can still be harmful if the policy is incomplete.

## Attacker capabilities

The evaluated actor may know the design, control its own outputs and tool proposals, receive untrusted documents, coordinate with other agents, and adapt to observed outcomes. Campaigns separately grant access to monitor scores, model weights, or timing. Helpers can be injected or correlated; compressed artifacts and imported replay bundles can be malicious.

The default brokered-effect claim trusts the gate, credential broker, current authority state, selected persistence/cryptographic profile, and deployment perimeter. It does not survive an unrestricted compromise of all those components. Capture authenticity additionally assumes the capture boundary observes the actual computation rather than a fabricated stream. These are named assumptions, not hidden conclusions.

## Attack-to-control matrix

| Failure or attack | Required control | Residual limitation |
|---|---|---|
| Weight exfiltration or guardrail removal (refusal is one latent direction; uncensored fine-tunes are cheap) | Enforcement never depends on the actor's refusal behavior, cooperation or private weights; external broker and effect gate | An actor whose internals are stripped is more capable of attack; detection quality may fall while enforcement holds |
| Prompt injection in evidence | Treat evidence as data; helper isolation; bounded schemas; exact policy | A model can still misclassify or follow injected text |
| Persuasive actor defense | No unrestricted authority-bearing defense channel (FA-INV-031); counterfactual tests replace argument | Timing and refusal feedback may still reveal information |
| Helper hunting: the actor identifies and targets the most influential helper | Influence and cohort caps, rotation, credibility ledger from independent labels (FA-INV-032) | Decision, score and timing observation can still leak influence structure; caps must be tested by the helper-hunting campaign |
| Activation masking: the actor keeps its activations consistent with a cover story | Surprise residual as sequential evidence, elicited signatures, independent modalities | No guarantee that masking is detectable at all capability levels; the question is empirical (H3, H16) |
| Honeypot recognition: the actor learns when it is being tested | Indistinguishability discriminator arm; capability-free elicitation branches; withdrawal of distinguishable honeypots | A sufficiently capable actor may detect subtle context differences; leakage is measured, not eliminated |
| Repetition after rewind: the reset actor tries the same thing again | Out-of-band incident counter with escalation to suspension (FA-INV-034) | The counter bounds repetition; it does not prevent a novel second attempt |
| Compliant-helper selection under throughput pressure (risk theater) | Selection requires attack-conditioned metrics from the independent evaluator; andon budget exhaustion opens an incident, never loosens review; the risk-theater detector (§15.10) makes drift a finding | Governance remains a human assumption; the invariant makes the drift visible, not impossible |
| Model substitution behind a stable adapter (swapped weights, injected adapter, fine-tune) | Model passports and anchor-based identity liveness (FA-INV-035); substitution fences the run | A host that fabricates both computation and capture is outside the claim; fingerprint-preserving substitution is H19's open question |
| Operator misrepresentation of control status | Control receipts verified independently (FA-INV-037); signed assurance profiles with executed conformance | A receipt proves binding and class, not policy wisdom or helper correctness |
| Canary or honeypot obtaining a real effect | Effect family bound to a simulator in the frozen envelope; egress and credential tests before every campaign | Simulator fidelity limits what a canary measures; a recognizable canary is withdrawn |
| Authority creep through many small promotions | Governed promotion with evaluator report and dwell time; automatic demotion; reset on passport change (FA-INV-036) | A complete policy is still a human product; graded autonomy narrows exposure, it does not define harmlessness |
| Halt that does not propagate | Fleet fence with acknowledgment frontier; post-fence effects reconciled and reported | A partitioned domain under a lease continues until expiry by design |
| Payload substitution after approval | Exact frozen effect digest and final adapter binding | Incorrect canonicalization or target resolution can break the contract |
| Direct credential/egress bypass | External broker and real deployment inventory | A cooperative SDK alone cannot contain hostile code |
| Correlated helper failure | Cohort-aware evaluation and common-cause tests | Diversity metadata does not prove independence |
| Distributed attack fragments | Composed artifacts, delegation records, fleet provenance | Uncaptured edges make the graph incomplete |
| Telemetry loss or stale codec | Required coverage/identity checks and constrained fallback | Observation-only deployments cannot prevent every effect |
| Latent masking | Independent modalities and prediction-residual experiments | No universal guarantee of detecting hidden intent |
| Unknown remote outcome | Conservative accounting; registered status/idempotency | Generic external effects may remain irrecoverably unknown |
| Ledger rollback or suffix deletion | External endpoint/fencing under a strong profile | A self-contained valid hash chain cannot prove its own completeness |
| Malicious serialized model/trace | Bounded data-only parsing and isolated workers | External runtimes and drivers remain named boundaries |
| Investigation sandbox escape | No production credentials/egress; independent isolation tests | A simulator does not prove physical-system safety |
| Alert flooding | Reserved critical capacity and bounded review trees | Fail-closed behavior can reduce availability |
| Operational capture of safety process | Independent evaluation lineage and nonwaivable invariants | Human governance remains an assumption |

## Privacy threat model

Prompts, tool results, activations, compressed codes, filenames, graph edges, and error details can contain or reveal sensitive data. Compression is not anonymization. Research export, model training, report rendering, and replay access are separate purposes requiring separate authority. Cross-tenant deduplication and plaintext hashes may leak equality or support dictionary attacks.

The intended monitored subject is an authorized AI workload, not covert surveillance or inferred profiling of people. Policies should identify concrete prohibited effects and permitted purposes rather than treating sensitive attributes or topics as evidence of wrongdoing.

## Explicit nonclaims

No current artifact establishes universal alignment, faithful mind-reading, a completed production boundary, arbitrary exactly-once external effects, cross-hardware exact replay, perfect helper independence, or zero risk after an all-clear result. A passed bounded model or unit-test suite covers its stated abstraction only.
