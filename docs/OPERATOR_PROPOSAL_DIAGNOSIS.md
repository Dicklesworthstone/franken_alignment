# Explain an original durable proposal

The Unix `supervise_publication` reference executable can explain the original
admission of an external actor request and evaluate an optional replacement
payload against the **same historical exact policy**. This connects the original
anchored proposal reader to the operator; it does not open recovered authority,
renew observations, run a helper or resend a publication.

```sh
supervise_publication explain-proposal CONFIG REQUEST_ID RECOVERY_PROFILE
supervise_publication explain-proposal CONFIG REQUEST_ID RECOVERY_PROFILE --payload REPLACEMENT_FILE
```

`REQUEST_ID` is the nonzero durable actor key, not its internal attempt number.
The original request book resolves that distinction during one bounded replay.
A request rejected without a frozen action, or a direct proposal that merely has
the same number, yields no fictional explanation. A frozen hard-policy denial
can be explained even when it never reached a helper congress.

## Independent inputs and supported profile

`CONFIG` is the existing operator configuration. This command accepts exactly
the ordinary source-publication guard set: the configured source policy is
required; stream, decoder, decoder-stop, identity, campaign and credential
guards must be absent. The effective ending policy must equal the independently
supplied configured policy. Unsupported or mismatched histories refuse; there is
no unanchored, unguarded or learned-history fallback.

`RECOVERY_PROFILE` reuses the closed `fa.guarded-stop/1` independent input format:

```json
{
  "schema_version": "fa.guarded-stop/1",
  "minimum": {
    "journal_revision": 12,
    "control_sequence": 0,
    "authority_epoch": 0
  },
  "history_anchor": "/independent-custody/retained.anchor"
}
```

These numbers illustrate the shape, not acceptable floors for a particular run.
Use independently retained floors and an anchor produced by the original
acknowledged owner. The anchor must be an admitted regular file outside the
canonical authority directory. **`retain_history_anchor` is rejected**, not
ignored: an investigator's read does not acknowledge a new durable cut and must
not export one as though it did. Neither the archive nor staging files are
modified. Operating-system access controls remain the custody boundary.

The evidence file, helper executables and reviewer endpoint need not exist.
Configuration parsing validates their declared shapes but does not open them.
The writer lock may remain held by the original live owner; the investigation
reads one immutable captured image, not a live or necessarily latest snapshot.
The complete suffix and independent ending requirements must qualify before
any diagnostic data is returned.

## Predicate-tree result and hypothetical payloads

Output is one `fa.proposal-diagnosis/1` JSON object. The result separates the
original admission revision/state from the ending captured journal revision and
state. It includes the external key, original attempt, historical policy and
semantic generation, witness count, original and counterfactual truth, the
native predicate graph, and historical execution/accounting counters. Each node
has its original predicate arguments, child references, baseline truth,
counterfactual truth and original witness index. Integer identities and durable
counters are decimal strings; node indexes and bounded counts are JSON numbers.
Policy literal bytes use lowercase hexadecimal. Actual action and replacement
payload bytes are not printed.

`--payload` is the only intervention. Its bytes are bounded by both the original
action and experiment limits; an empty file means an explicit empty replacement.
Missing, oversized or unreadable files fail instead of silently requesting a
baseline result. Target, units, evidence keys, model state and world state are
not editable through this command. Every comparison starts from the same
historical admission, not the ending state or another counterfactual.

A `satisfied` result still has `purpose: experiment`, `permission: not_issued`
and `next_requirement: fresh_independent_review`. It neither overrides the real
denial nor predicts helper votes on changed inputs. A later cancellation or
execution does not rewrite the original admission explanation. Successful exit
means the requested diagnosis was delivered, not that an effect was permitted.

This is a privileged operator projection of the existing `explain` vocabulary
(plan §17; predicate trees, not persuasion). The payload comparison uses the
existing capability-free experiment layer (plan §12). Policy literals and
internal identities can be sensitive; do not expose this output or the private
configuration through an actor/helper interface. It does not claim to implement
the complete authorization-filtered production agent API.

## Work and interruption boundaries

One monotonic elapsed budget, using `CONFIG.timing.runtime_ms`, begins before
configuration reading. It is checked after input admission, before each original
historical event, at Ready, after native finalization/evaluation, and before
output. The command does not sample or append an authority clock. Each replay
pass executes one native event; there is no preliminary unlocked ID lookup or
second history replay. Cost is proportional to the bounded retained history plus
the native bounded policy evaluation, not the cost of a cached live explanation.

A checkpoint timeout emits no partial diagnosis and leaves canonical, staging
and anchor bytes unchanged. Read/decode, an individual event, formatting and
output writes remain synchronous: this is not a hard wall-clock bound on those
operations. A write or flush failure is an error, can leave partial stdout, and
cannot trigger effect retries or mutate a request. The operator must choose a
safe stdout destination; shell redirection is outside this command's custody.

## Source and verification scope

Implementation: `examples/supervise_publication/diagnosis.rs`, its native-tree
renderer and the original request-key `FileProposalRead`. The tests exercise
actual source events, durable request allocation, independent anchor files,
held writer locks, original denial versus hypothetical repair, unavailable live
assets, all elapsed checkpoints, malformed CLI/payload inputs, exact guard/floor
refusals, output failures and concurrent writer progress. Predicate serialization
covers every original operator and binary literals.

This increment is authored source with unexecuted Rust tests. The required
remote-only targeted example and full `xtask` commands must pass against this
revision before qualification. No roadmap packet, production gate or detector
performance claim is closed by these fixtures.
