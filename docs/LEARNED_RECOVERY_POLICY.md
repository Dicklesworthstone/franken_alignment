# Independently expected policy and campaign guards during learned recovery

An acknowledged policy replacement changes the effective policy without
rewriting the store's bootstrap profile. Recovery must verify both facts:
the original Config still describes creation, while the current policy is the
one independently expected by the operator.

Learned recipes can now carry that full expected policy in an explicit outer
envelope. The envelope is comparison data for guarded recovery. Reading it does
not replace, promote, relax, or otherwise apply a policy.

## Envelope

The closed schema is `fa.learned-recovery-policy/1`:

```text
{
  "schema": "fa.learned-recovery-policy/1",
  "effective_policy": POLICY_OBJECT,
  "recipe": EXISTING_RECIPE_OBJECT
}
```

Replace `POLICY_OBJECT` and `EXISTING_RECIPE_OBJECT` with complete JSON objects.
The `effective_policy` object uses the existing Config policy grammar: a numeric
`generation` and the complete `nodes` array. For example, the following is a
complete policy object; it is appropriate only when it exactly matches the
operator's independently selected policy:

```json
{
  "generation": 2,
  "nodes": [
    { "op": "payload_at_most", "maximum": 512 }
  ]
}
```

The full policy is required. A generation number, digest, or path alone cannot
stand in for its predicate contents. Unknown fields, missing fields, malformed
predicates and unsupported schemas are rejected by the original closed parsers.

The `recipe` object is an existing admitted learned recipe, including its
existing credential, checked-publication, identity or joint wrappers when used.
The policy envelope is outermost and appears once. It does not alter the
canonical order or grammar of those inner wrappers. Input remains subject to
the existing bounded recipe and asset admission.

Existing recipes without this envelope continue to expect the exact bootstrap
policy from Config. There is no implicit lookup of the current policy in the
journal and no automatic upgrade when an old expectation refuses recovery.

### Histories with policy campaigns

Use `fa.learned-recovery-policy/2` when the original history has the native
policy-campaign guard enabled. This version requires the full effective policy
and the exact independently retained campaign configuration:

```text
{
  "schema": "fa.learned-recovery-policy/2",
  "effective_policy": POLICY_OBJECT,
  "campaigns": {
    "cases": 16,
    "input_bytes": 1048576,
    "max_campaigns": 8
  },
  "recipe": EXISTING_RECIPE_OBJECT
}
```

The three example limits must be replaced by the original guard's values; they
are not defaults. Each is a positive integer. Native upper bounds are 5,120
retained replay cases, 32 MiB of replay input and 64 campaigns. Missing, null,
duplicate, unknown, zero and over-limit fields refuse before model asset intake.
The envelope remains outermost and cannot be nested or reordered with `/1`.

Legacy recipes and `/1` require campaigns to be **absent**. `/2` requires an
enabled guard with exactly the selected limits; it does not accept an absent
guard, different limits, or a guard discovered from the journal. Even larger
selected limits are a mismatch, not a safe substitution. The selection is
comparison data only: it neither enables campaigns nor approves or promotes a
candidate policy, and the operator exposes no policy governor through it.

## What recovery verifies

The selected recipe retains the expected policy alongside its original
numerical/text contracts. The original guarded recovery requirements use that
full value as `effective_policy`, while keeping the original bootstrap profile,
guard inventory, credential binding, history floors and anchor checks.

The same canonical recovery cursor must replay the complete history and match
the independent expectation before reaching a usable Ready cut. A valid history
that reaches a different effective policy refuses before staging cleanup,
recovery Fence or live owner issuance. Matching only its generation is
insufficient: different predicates at the same generation also refuse.

The original command owns its finalization. The new envelope works through
the shared guarded recovery selection used by learned resume, guarded proposal
explanation and policy replay, credential maintenance, terminal operations and
fleet members. It does not select a weaker
owner or bypass any additional requirement of those operations. For example,
stream-reset recovery still requires its exact original intent and retained
anchor, and credential maintenance still preflights the exact requested
credential change.

The envelope admits histories with acknowledged direct native policy replacement
and, with `/2`, histories protected by the original policy-campaign guard,
including a policy changed through its replay/approval/promotion protocol.
Both use the same complete guarded replay, never a weaker open path. Its recovery
Fence preserves promoted policy and receipts while revoking outstanding campaign
approvals; a recipe cannot resurrect a lost promotion key. The numerical recipe,
source freshness and later independent effect review retain their original rules.

## Creation and resident sequences

A recovery expectation cannot change the policy used to create a new owner.
Both ordinary learned creation and topology creation reject an expected policy
that differs from the Config bootstrap before creating a journal. Supplying the
same full bootstrap policy explicitly is allowed.

The campaign selection is recovery-only. Every new-owner creation entry rejects
`/2`, including when its effective policy equals the bootstrap. It cannot install
an extra guard after creation or mint a new governor implicitly. Provision an
original campaign-enabled history through the native guarded interfaces and
retain its exact configuration independently before selecting recovery.

Every recipe in one resident learned sequence must select the same expected
effective policy. This equality covers the complete policy, including its
generation and predicate contents. A later recipe cannot silently change the
expectation established for the original owner. Wrap each member recipe as
needed; the sequence profile itself keeps its existing grammar.
Campaign selection is also identical across all entries, including absence and
each original limit. A successor cannot silently add, drop or resize the guard.

## Using a changed policy

Keep the original Config and all original numerical, credential and source
contracts. Independently retain the exact policy selected for recovery, then
wrap the existing learned recipe with that policy object. Use the resulting
recipe in an existing guarded recovery, proposal-explanation or policy-replay
command; no additional shared CLI flag is stripped or inferred.

The numerical audit and situation commands retain their existing read-only
replay contracts. They do not evaluate this recovery-specific field as an
additional policy assertion. Their output is not evidence that the selected
expected effective policy matched; use a guarded diagnostic or recovery path
for that check.

Continue to supply the independently retained history anchor and recovery floors
required by the selected operation. The envelope proves no prefix authenticity,
creates no fresh observation and supplies no effect authority. Its purpose is
to make explicit policy and guard expectations available to the original native
verifier. Plain, raw-predictive, owned-predictive and topology recovery, terminal
operations, credential maintenance and fleet members share that verifier input;
each operation still requires its own independently selected custody data.

## Verification

Authored regressions use genuine native policy replacement and journal replay.
They distinguish successful matching recovery from missing, altered or malformed
expectations and check that refusal leaves canonical and staged history
unchanged. Creation and resident-sequence cases protect the bootstrap boundary.

Source review does not qualify the change. Compilation, runtime tests and the
canonical gate remain pending on the repository's required RCH runner. Existing
roadmap acceptance and guard requirements remain in force.
