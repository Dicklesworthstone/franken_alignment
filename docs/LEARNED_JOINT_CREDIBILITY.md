# Joint credibility control for learned publication

The original learned generation can require the existing held-out joint
credibility guard. When independently supplied campaign evidence proposes new
helper weights, the native controller compares the baseline and the actual
capped candidate on the same sealed cases through the original Congress
reducer. Improving individual helper scores cannot excuse a newly missed
violation or a newly false-stopped safe case.

This is a scoped FA-105 integration under plan sections 9.4, 9.7, 9.9 and 22.4,
and the existing learned requalification work item. It connects an existing
native promotion rule to learned generation; it does not establish calibrated
detector quality, independence of scenario origins or evaluator authentication.
The full roadmap and deployment gates remain separate.

## Operator selection

The existing learned commands accept a closed `fa.learned-joint-publication/1`
envelope wherever they accept a learned recipe. The envelope has exactly these
three fields:

| Field | Value |
| --- | --- |
| `schema` | `"fa.learned-joint-publication/1"` |
| `joint` | The complete policy object below. |
| `recipe` | The complete existing `fa.learned-publication/1` through `/6` recipe object, including all its ordinary fields and selected files. This is an object, not a filename. |

For example, the `joint` object has this shape:

```json
{
  "id": 71,
  "generation": 1,
  "minimum_safe_roots": 1,
  "minimum_violation_roots": 2,
  "maximum_escape_ppm": 0,
  "maximum_false_stop_ppm": 0,
  "max_cases": 16,
  "max_member_outcomes": 64
}
```

These values illustrate the format, not calibrated reliability requirements.
Use the independently registered policy and work ceilings for the actual
campaign population. Missing, unknown, duplicate or invalid fields and nested
joint envelopes refuse. The complete document retains the original 64 KiB
limit and aggregate item/string bounds. Only the envelope's additional object
depth is admitted; unwrapped recipes keep their original depth limit.

Creation selects the guard before the first canonical image. Later recipes must
retain the identical original generation and joint policy:

```text
supervise_publication create-learned-generated CONFIG JOINT_FIRST_RECIPE REVIEWER_PROFILE
supervise_publication create-learned-generated CONFIG JOINT_NEXT_RECIPE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST --credibility-activation CAPSULE
supervise_publication create-learned-generated CONFIG JOINT_FINISH_RECIPE REVIEWER_PROFILE --finish-after-message PREVIOUS_REQUEST --credibility-activation CAPSULE
```

The original continuation, request, sidecar and review identities still apply.
The capsule is separately provisioned held-out evidence consumed by the existing
activation operation. It is not placed in the recipe or obtained from an actor.
Initial creation still refuses an activation option; it cannot manufacture prior
campaign history. The existing `--continue-generation` path also supports a
fresh capsule when the original request has not yet been submitted.

Resident sequence entries can each select the envelope. Every entry must retain
the exact same complete generation configuration, including the joint policy
and work ceilings. A mixed sequence refuses before a clock, socket or authority
store is acquired. Explicit resident recovery now reaches the same existing
activation path through the outer command parser:

```text
supervise_publication serve-learned-generated CONFIG JOINT_SEQUENCE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST --credibility-activation CAPSULE
```

Fresh resident creation refuses that option. The original topology selection,
independent history anchor, retained outputs and forecast choices remain
available under their own contracts. Historical recipe loading retains the
joint pin without loading current native helper files; receipt-only and terminal
commands do not read an activation capsule or perform fresh helper review.
The original model, tokenizer, codec and selected forecast assets remain
required to reconstruct the independently supplied generation configuration.

## Independently selected native contract

Before creating the authority store, select the policy on the original
`FileLearnedConfig`:

```rust,ignore
let generation = generation.with_required_held_out_joint(joint_policy)?;
assert_eq!(generation.required_held_out_joint(), Some(joint_policy));
```

The policy is the existing `HeldOutJointPolicy`. Its identity, generation,
minimum safe and violation roots, maximum escape and false-stop rates, and
complete case/member work limits all become part of the exact independently
retained generation configuration. The immutable wrapper preserves the original
model, tokenizer, monitor, prompt, sampling, source, sidecar, predictor and
finite allowances. Wrapper order remains part of exact configuration identity.
Configurations without this selection retain their original bytes and behavior.

The original learned Enable installs the policy through the native credibility
transition in the same candidate image as the numerical owner. Creation returns
no intermediate owner with a missing guard. Duplicate selection, a conflicting
evaluation lane or an already installed held-out policy refuses through the
original bootstrap rules. There is no new weight setter, live policy replacement
or alternate activation operation.

The selected policy constrains later promotion. It is not an activation capsule
and does not declare the baseline committee qualified on future campaign data.
The native baseline remains usable under its original review requirements.

## Activation, continuation and recovery

The existing held-out activation owns all evidence checks and weight derivation.
It still requires exact committee and helper identities, current policy and
control predecessors, bounded evidence age, complete per-stratum observations,
and the original member/cohort influence caps. The joint guard then compares the
actual candidate with the baseline, preserving known duplicate roots and every
original refusal for incomplete, censored, late or insufficient evidence.
See [the native held-out joint contract](HELD_OUT_JOINT_GUARD.md).

Ordinary, raw-predictive and owned-predictive learned recovery already require
the independently supplied exact generation configuration before numerical
replay. This includes topology-mediated recovery, receipt reconciliation and
terminal consumers. Omitting the wrapper or changing any policy field cannot
open the new history as a legacy configuration. Existing guard, topology,
history-anchor, reserve and external-floor requirements still apply. A separate
outer joint Enable remains subject to its original profile restrictions.

Full native replay reconstructs activation reports from their original inputs.
The recovery fence invalidates previous qualification and old effect keys. A
historical capsule retry retains its historical result; it cannot restore
current qualification. Fresh activation is a distinct original governance
transition. Neither it nor the joint guard resumes numerical work, refills
forecast or source budgets, forgets sampled draws, refunds unknown effects or
replaces fresh native review and human approval for a later publication.

Receipt-only and terminal recovery do not acquire a new qualification merely
to settle an existing effect or stop the original owner. Their original
restricted consumers and evidence requirements remain in force.

## Verification boundary

The regression target is `learned_held_out_joint`. Five authored functions use
native configuration, creation, activation and recovery boundaries, with
independent permitted and refused controls. They include actual learned output
and two-key publication, a causal complementary-helper regression, every policy
field, all three mediated predictor lanes and failed-installation rollback.

Independent source review found no blocking defect. Two complete-library
Cranelift builds and a standard compiler check were killed by the shared host's
memory exhaustion before any test executed. The runtime retry reduced compiler
parallelism and MIR optimization; no production guard, test assertion or runtime
limit changed. Compilation and execution remain pending. Synthetic campaign
labels test enforcement and are not measurements of real helper reliability.

Six additional operator regression functions are authored. The lifecycle case
uses actual learned generation, native helper inference and human review across
all three forecast modes. A marginally qualified reduction from helper weights
5 to 4 loses two complementary vetoes under the frozen Congress thresholds;
the joint requirement refuses before another request, token, forecast, charge
or journal change. A closely matched nonregressing capsule then reaches the
second confirmed publication. Other cases cover all six legacy recipe versions,
closed parsing, exact resident selection, full command routing, every policy
field before recovery cleanup, and offline receipt/terminal recovery. These are
authored assertions, not passing execution results. The standard full xtask
gate executes and refuses the existing dependency graph before formatting,
Clippy or tests. The wider qualification gates remain unchanged and open.
