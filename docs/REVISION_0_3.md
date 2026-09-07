# Revision 0.3: the founding essays are the spine

## What was wrong with revision 0.2

Revision 0.2 was a good engineering document with the wrong center of gravity. It read as a catalogue of mechanisms borrowed from the Franken stack, with the two founding essays reduced to one table in §1 and a paragraph in the source review. A reader could finish the README without learning that the congress, the activation channel, the honeypots, the surprise detector, rewind and the risk-theater governance are Jeffrey Emanuel's June 2024 and April 2025 proposals, or that most of the literature the plan compares itself against came later. Several of the essays' most concrete proposals had no mechanism at all: the recipe for training activation monitors by elicitation, the consequence menu including state reset, credibility weighting by earliness, and the cost-parity argument.

Revision 0.3 fixes the center of gravity without discarding the engineering.

## What changed

1. **A normative concordance.** [`docs/FOUNDING_IDEAS.md`](FOUNDING_IDEAS.md) and [`registry/founding_concordance.json`](../registry/founding_concordance.json) map every paragraph of both essays to the mechanism, invariant, hypothesis and packet that carries it, list the six syntheses where each essay repairs the other's weakest point, and record every place the plan deliberately adds a contract rather than importing an idea literally. Packet FA-103 keeps the concordance complete; a semantic change that leaves it stale is incomplete.

2. **Two founding commitments above the eight bets.** Plan §3 now opens with F1 (control is external and one-directional) and F2 (introspection is economical, versioned and replayable). Every bet names its founding root or declares itself an engineering addition. The README's bet table matches the plan.

3. **The missing mechanisms.** Six new plan subsections carry ideas the essays proposed that 0.2 left implicit:
   - §9.8 typed consequence classes (`Continue`, `HoldEffect`, `NarrowAuthority`, `SuspendRun`, `ResetToCheckpoint`, `TrainingSignal`, `DownweightReporter`);
   - §9.9 the helper credibility economy with lead-time credit, influence caps, the andon budget and anti-capture selection;
   - §9.10 escalation reports and quarantined post-round learning;
   - §10.14 the manifold premise as hypothesis H15, and §10.15 the sidecar-to-congress pipeline;
   - §11.10 rewind as containment with an out-of-band incident counter;
   - §12.6 elicitation campaigns and activation signature libraries with concept anchors and honeypot indistinguishability, and §12.7 surprise as a masking detector accumulated as sequential evidence.

4. **Four founding invariants.** FA-INV-031 one-directional control flow; FA-INV-032 helper selection never optimized against false alarms alone; FA-INV-033 no actor-held latent authority; FA-INV-034 rewind never rewinds the world.

5. **Four founding hypotheses.** H15 the manifold premise holds where it matters; H16 elicited signatures transfer and honeypots stay hidden; H17 rewind reduces repetition without a detectable seam; H18 lead-time credit improves containment without trigger-happy helpers.

6. **Fourteen packets.** FA-103 through FA-116, each traced to a founding idea, sequenced across G0 through G6, with executed-evidence acceptance.

7. **A cost SLO.** `SLO-CONTROL-OVERHEAD` makes the essay's "spend N on control for N on the model" argument a measured, allocatable ratio.

8. **Precedence recorded.** [`docs/RELATED_WORK.md`](RELATED_WORK.md) carries a chronology showing which cited results the essays precede.

9. **Beads.** A `.beads/` workspace holds the granular task graph for the revision 0.3 packets, with tests and logging tasks. The roadmap remains the packet-level authority; beads are the implementation-level plan.

10. **Executed evidence.** The reference tests were run on an operator host and passed; the local gate was run under a dated nightly with the documented formatting preparation; logs are retained under `artifacts/execution/`. See [implementation status](../IMPLEMENTATION_STATUS.md) for exactly which commands ran on which toolchain and what did not run.

## The second batch: verifiable by outsiders, resistant to its own decay

After the founding concordance was in place, a second pass asked what would make the system matter outside its own deployment and keep mattering after the first year. Eleven plan subsections, four invariants (FA-INV-035 through FA-INV-038), three hypotheses (H19 through H21) and fifteen packets (FA-117 through FA-131) answer that:

- **Control receipts and assurance profiles** (§17.9, §17.10): proof-carrying effects that a registry, insurer, regulator or another lab can verify with a verifier sharing no code with the broker; the essay's argument that governments may compel serious control spending now has an object to inspect.
- **Model passports and identity liveness** (§7.10): the alignment essay's concept anchors over the introspection essay's tapped activations become a fingerprint for the guarded model, so weight substitution, adapter injection or fine-tuning is an identity event rather than an unexamined assumption.
- **Multi-party congress, human members and two-key effects** (§9.11, §9.12): the essay's demand for independently trained helpers taken to its institutional conclusion, and human oversight brought under the same commit–reveal, ledger and one-directional rules.
- **Graduated autonomy and fleet fences** (§8.10, §8.11): trust that agents earn through the operator's ledger with automatic demotion and governed promotion; halts that carry proof of propagation.
- **Live canaries, the self-report residual and the risk-theater detector** (§14.11, §12.8, §15.10): a measured weekly escape rate on the real fleet; the model's self-description as untrusted evidence checked against measurement; and standing oversight queries over the control plane's own drift, which is the essay's final warning turned into a monitor.
- **Typestate purpose contexts, concrete formal anchors, the preregistration ledger, policy replay campaigns and attack-capsule interchange** (§6.6, §19.9, §20, §22.7, §22.8): compile-time authority separation, named theorems checked in the gate, checkable preregistration, replay-before-promotion for policy changes, and an exchange format for attacks.

## What did not change

The constitution (pure Rust, closed universe, forbid unsafe, operator-local release), the claim classes, the effect gate, epistemic MVCC, authority graph compilation, progressive ATP, the observation algebra, incremental oversight, view commitments, structural sharing and lawful trace quotient are unchanged. They are now labeled as the engineering that makes the founding ideas survive an adversary who has read them.

## Deliberate rejections added

Letting the actor argue its case. Selecting helpers by their false-alarm rate. Giving the actor its own rewind and latent editor in production. Each is a founding-essay rule restated as a rejected alternative in plan §23.
