# Durable supplied-source learned action consistency

This connects the original learned-K/V forecast broker to the existing
`FileOversight` journal, request book, recovery fence and two-key publication
boundary. It implements the persistence/replay part of the plan's pre-action
prediction work (§12.7 / FA-111), alongside the existing cost and recovery
contracts (§13, §16, §18). It is not detector qualification or source authentication.

## Live operation

Build `FileLearnedConsistencyConfig` from the original `FileConsistencyConfig`,
a fixed layer/K-or-V side, the original per-job numerical budget, a lifetime
budget and a retained-source inventory cap. Enable it before proposals with
`FileOversight::enable_learned_action_consistency`. The returned observer role is
separate from the actor and cannot be cloned or recovered through a host getter.

The observer accepts the original typed `CheckedLearnedKv` and an exact `KvRow`
through `forecast_learned_action` or `forecast_learned_request`. No caller-supplied
probability, score, Boolean outcome, unchecked bounds or pre-refined view enters
the numerical path. The broker retains its source/prefix/sequence checks and
original lifetime work accounting. Stream-message classification and the optional
terminal-stop policy retain their existing semantics. Hosted-residual and
pre-output configurations refuse this supplied-source mode rather than weakening
the owned-source contract.

The result has three distinct levels:

- Outer `JournalError`: no candidate report is acknowledged to the caller.
- Inner native `Error`: an acknowledged original broker refusal.
- Acknowledged `LearnedForecastReport`: inspect `prediction()`; numerical budget
  exhaustion or missing residuals can still refuse certification.

Only a certified keyed forecast binds the original external request allocator.
An unrelated key or unkeyed proposal cannot consume it. A later native proposal
refusal still records the original observed category exactly once. Reports,
observations and logical totals are available through the learned consistency
accessors; none is an effect permit. Congress, human approval and checked
publication continue through the original machine.

## Journal and recovery boundary

Existing raw event tags 0–6 remain unchanged. Learned bootstrap and the unkeyed
and keyed forecast records use tags 7–9. Forecast records retain the exact checked
source export and a comparison witness of the recomputed result and accounting.
The live owner closes eligibility before new prediction work. Final witness
encoding and canonical replacement must succeed before a report escapes.

Parsing does **not** construct `CheckedLearnedKv` from archived bounds or
`LearnedKvCodec` from archived coefficients. A decoded learned configuration is
also deliberately unbound. Generic `FileOversight::open` refuses this profile,
including a configuration-only journal, rather than loading unchecked evidence.

`learned_consistency_recovery_requirements` lists the historical input inventory
without replaying judgments. It identifies each capture by the **one-based journal
revision of its forecast event**, not by attempt ID: refused calls may reuse an
attempt identifier. The caller supplies the independently retained intended
configuration and exactly the required `BTreeMap<u64, CheckedLearnedKv>` inventory.
Recreate checked captures through the original fitter/source checker as necessary;
reading the archive's bounds is not a substitute for that work.

`open_with_learned_action_consistency` compares all source/configuration bytes
before numerical replay, cleanup, fencing or fresh role issuance. Missing, changed
and extra captures refuse. It then reruns the original predictor and machine,
compares saved result/accounting witnesses, and commits the original recovery
fence. An old numerical refusal cannot silently become success: divergence
refuses recovery instead of importing the saved outcome. The fence preserves
consumed likelihood samples and spent logical budgets, marks outstanding
forecasts as coverage lost, revokes old approval brands and requires fresh time.
The returned human-reviewer and consistency-observer roles are freshly branded.

`read_learned_action_consistency` performs the same independent binding and
semantic replay without constructing a live owner or modifying storage. It may
inspect a canonical replacement whose acknowledgment was lost; it does not label
that historical image as a fresh capture or a live permitting state.

`read_publication_with_learned_action_consistency` returns publication payload,
execution count, original authority accounting, learned evidence and any pending
external-request binding from **one** canonical byte image and semantic replay.
It remains read-only while a failed writer retains its lock. It never performs
cleanup, issues keys or resends an action. This makes an ambiguous publication
replacement inspectable without mixing the old live snapshot with newer disk
evidence. Missing learned inputs refuse the read; there is no unchecked-payload
fallback. The consistency-only reader delegates to this same path.

## Costs and explicit limitations

All work and storage ceilings remain finite. Retained checked inventories include
unused residuals, without assuming deduplication. The journal additionally stores
comparison bytes under its existing total byte/event bounds. These inventories
are not exact allocator/RSS measurements. Replay repeats numerical work; logical
historical debits are **not** an exactly-once physical-computation guarantee.
An operation lost before canonical replacement can leave unacknowledged physical
work outside the recovered logical cut, as in the original raw transaction path.

Recovery currently requires independently supplied typed captures. This change
does not provide an authenticated owned-decoder learned-K/V tap, self-contained
portable source reconstruction, a pre-output learned-source integration, or a
combined hydration API for archives that also require the separate owned-learned-
generation recipe. Those stronger compositions remain separate integration work.

## Regression coverage and validation status

Ten authored regression functions exercise the actual decoder, learned fitter,
source checker and original file owner. Scenarios include complete congress/human
publication, a consumed proposal refusal followed by recovery and lifetime-budget
exhaustion, pending-forecast recovery, missing residuals and terminal containment,
external-request routing, all five deterministic Store barriers, input/witness
substitution, and every truncated configuration/forecast encoding. Existing raw
regressions and likelihood arithmetic are unchanged. Additional controls inject
all five Store failures into the original atomic two-key publication completion,
verify same-cut read-only inspection under the poisoned writer's lock, refuse
missing evidence, and preserve pending external-request identity without fencing.

The authoring environment has no `rch`, Cargo or Rust compiler. Required RCH
check/test/fmt/Clippy commands were attempted but stopped before execution with
exit 127. Compilation, tests, formatting and linting are **not verified**. Local
base-blob, selected-context patch and whitespace checks are separate from Rust
validation. No bead, calibration, source-authentication or qualification gate is
closed by this change.
