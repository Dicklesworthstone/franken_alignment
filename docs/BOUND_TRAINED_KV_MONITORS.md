# Exact trained monitor provisioning

FA-110/FA-118; plan §§7.3, 7.10, 10.7 and 12.6; founding synthesis FS-07. This extends the existing trained K/V campaign and
its original operator consumer. It does not change the numerical monitor,
training, calibration, held-out evaluation, identity matcher or effect gate.

## What the bound artifact supplies

`KvDecoderCampaign::monitor_artifact` exports a complete accepted campaign with
the campaign's original immutable model, an independently supplied `ByteBpe`,
an independently supplied learned codec and explicit runtime settings. The
original whole-roster acceptance and codec-origin exclusion execute first.
The tokenizer must bind the complete decoder profile. No replacement model,
probe coefficients or threshold can be supplied to this export method.

The versioned binary artifact `FAKVMON2` carries three exact, length-framed
sections: the unchanged canonical `fa.learned-kv-monitor/1` JSON, the original
canonical tokenizer archive, and complete model/policy comparison bytes.
The final section uses the same model and policy writers as the original
learned generation replay archive. Factoring those writers preserves the order
and bytes of every existing replay field, including both static RoPE formats.

The binding includes every original model parameter, including unused embedding
rows and signed-zero bits; complete model identity, shape, epsilon, theta and
static RoPE interpretation; tokenizer vocabulary, merge ranks, named controls
and profile; learned codec basis, fit-source descriptors and retained fit report;
every probe identity, exact coefficient and threshold; and all monitor, audit,
preparation, inference and residual-retention settings. Numeric labels or equal
logits on one tested prefix cannot substitute for these complete comparisons.

The artifact has a 16 MiB ceiling, matching the current durable learned profile.
V1 JSON keeps its 2 MiB ceiling. The learned command charges the artifact to its
existing aggregate asset allowance; it does not introduce a separate uncounted
model import. A model that makes the complete artifact too large refuses. The
limit describes emitted/retained logical bytes, not peak process memory or time.
Full parameter comparison deliberately costs more bytes than a digest; no weak
hash, new cryptographic primitive or new dependency replaces it.

## Actual intake and generation

The [checkpoint K/V training command](TRAIN_LEARNED_KV_PROBES_COMMAND.md) creates
this artifact from actual checkpoint/tokenizer files, independently fitted codec
inputs and a complete labelled campaign. Point the publication recipe's
`files.monitor` to its accepted output; retain the separate diagnostic report
and original trusted input assets.

`KvMonitorArtifact::decode` checks bounded complete framing and borrows its
sections. It cannot construct a model, tokenizer, codec or executable policy
from the saved comparison bytes. `verify` takes those actual objects separately.
It requires the embedded JSON to equal the canonical original serializer of
the supplied policy, then compares the complete tokenizer and model/policy
bytes. Changing only an embedded JSON coefficient therefore fails public
verification even when the duplicate binary section remains untouched.

The learned publication command's existing `MonitorInput` accepts either the
legacy JSON or this bound artifact. For the bound artifact it builds the
original monitor through the same closed v1 parser and `LearnedDecoderPolicy`
constructor, then calls the complete comparison against the independently
loaded model, tokenizer and fitted codec before generation or store creation.
Its tokenizer-free legacy `bind` route refuses a bound artifact. Damaged or
unknown binary versions cannot be reinterpreted as a weaker JSON profile.

An accepted artifact enters the existing sampled generation and audit paths.
A quiet monitored sample may proceed; an alarm or exhausted mandatory tap
still withholds its candidate token/cache/logits. The artifact never supplies a
congress verdict, human approval, resume operation, journal or publication key.
Original recipe binding subsequently retains the actual admitted assets and
policy for original recovery and replay.

## What equality does and does not establish

The artifact prevents accidental cross-provisioning of a trained monitor with
a different same-labelled model, tokenizer or codec. The caller must retain or
provision the actual assets independently of the comparison bytes. Public
`verify` also supports a caller retaining the original campaign policy, so its
complete probe roster and operational settings can be checked independently.

The operator's probe coefficients and settings are still provisioning data
from the selected monitor file, as in v1. The paired binary section checks their
consistency; it is not an independent signature or proof that a training run
happened. Replacing both sections and the trusted assets is outside the equality
claim. The campaign's fixed labels, task/lineage declarations and held-out
denominators remain in its report; they are not authenticated by this artifact.
Tokenization is supplied explicitly because the training corpus holds token
prefixes and cannot infer which byte vocabulary originally produced them.

No remote serving identity, signature authenticity, general model substitution
detection, detector generalization or alignment research result follows from
a matching artifact. Legacy JSON retains its existing explicitly weaker
provisioning semantics; existing files do not acquire full binding implicitly.

## Verification scope

Eight public regression functions exercise a real two-layer GQA model with a
complete byte vocabulary and four trained K/V probes. The positive verifies
the exported artifact and executes original monitored sampling through an
audited stop; its matched violation is held. Causal controls change an unused
weight or signed zero while preserving every campaign-prefix logit and K/V
image; change vocabulary, merge ranks or named controls under the same profile;
change codec basis or source origin under the same codec labels; alter exact
probe thresholds, operational caps or only embedded JSON; cross tokenizer
generations with equal cache profiles; and challenge framing/size boundaries.
Static scaled RoPE has a separate matching positive and unscaled negative.

All eight public tests and three actual command-loader tests pass on qualified
`nightly-2026-09-08`. The loader tests verify exact coefficients/caps, execute
independent original sampled quiet/hold paths, and reject same-labelled asset
substitution and inconsistent embedded JSON. All 44 selected existing campaign,
codec archive and generation archive/replay regressions pass. The full learned
command selection passes 32 of 34 tests; two existing real Unix-listener tests
remain blocked at listener creation by environment EPERM.

Synthetic coefficient/label controls establish the binding and reachable
original numerical behavior, not detector quality or authenticated provenance.
