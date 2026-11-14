# Explicit policy promotion for a learned publication history

This operator path connects an independently reviewed, complete retained-policy
comparison to the **original campaign governor and policy transition**. It is a
trusted offline governance operation for an existing, campaign-protected,
nonpredictive learned text-stream history. It changes policy; it issues no effect permit,
human-review key, numerical owner or governor handle.

The plain profile uses its independent history anchor. Topology-mediated
histories use the [paired source-custody extension](LEARNED_MEDIATED_POLICY_GOVERNANCE.md),
which retains the original graph requirements and source expenditure through
the same atomic promotion.

The read-only `--policy-replay` command remains descriptive. Running it does not
request, approve or promote a campaign. A separate operator selects
`--promote-policy`, an exact governor request and the unmodified report they
reviewed. The executable is a trusted local supervisor: access to its governance
invocation and input files must be provisioned separately from the actor,
helpers and human **effect** reviewer. This is not a new signed-identity or
remote governance authentication protocol.

New nonpredictive resident sequences can instead retain the original first-image
governor; independently anchored recovery can retain its freshly provisioned
successor. Both expose the [separately authenticated live endpoint](LIVE_POLICY_GOVERNOR.md#govern-a-new-resident-learned-sequence).
That selection requires both a governor Request/Approve numerical allowance and
an independent operator Promote allowance. The offline anchored operation below
keeps its original complete grammar and atomic three-event replacement.

## Fresh governor custody after learned recovery

The live recovery adapter can retain the fresh governor returned by the original
`FileGuardedLearnedRecovery` together with its recovered host and human reviewer.
It accepts only an explicitly selected nonpredictive, nonmediated learned stream
with an independently retained history anchor and exact existing campaign
requirements. A creation campaign selection cannot substitute for that inventory.
The complete original guard, effective-policy, credential, recovery-floor and
terminal-reserve checks still belong to the same native cursor.

Replay checks the command's original deadline and independent Stop transport at
every event and again at Ready. A pending authenticated Stop consumes that same
verified cursor through its original terminal path, retaining the acknowledged
anchor without issuing a live governor. Otherwise the original recovery fence
returns fresh custody once. No role getter, late guard installation, second
replay or reconstruction from a historical approval is used.

The native numerical history, confirmed effects, unresolved charges and campaign
spending survive recovery. Old governor handles and old approval keys cannot act
on the recovered owner. A new policy promotion still requires a new complete
campaign, independent approval and separately paid promotion; later publication
still needs fresh source evidence, native helper work and human approval.

This adapter and its regression sources advance FA-014 and FA-130. Source and
syntax review are separate from Rust compilation, execution and deployment
qualification. The live command composition is described in
[the governor service contract](LIVE_POLICY_GOVERNOR.md#recover-a-governed-learned-sequence).

## Provision the original guard

The history must already require campaigns with an independently retained exact
`cases`, `input_bytes` and `max_campaigns` inventory. New histories can install
that guard through the explicit first-image
[`--campaign-profile` workflow](LEARNED_CAMPAIGN_CREATION.md). Recovery selects
the same inventory through `fa.learned-recovery-policy/2`. Recovery cannot add,
omit, resize or disable the guard.

The original campaign count and retained-input bounds remain lifetime limits.
This workflow neither resets them nor replaces an exhausted campaign with a
caller-selected subset of cases.

## Review the complete original comparison

Prepare the existing strict `fa.policy-replay-request/1` candidate document:

```json
{
  "schema_version": "fa.policy-replay-request/1",
  "expected_policy_generation": 1,
  "limits": {
    "cases": 16,
    "input_bytes": 1048576,
    "output_bytes": 1048576
  },
  "candidate": {
    "generation": 2,
    "nodes": [{"op": "payload_at_most", "maximum": 256}]
  }
}
```

`cases` and `input_bytes` must equal the installed campaign guard for a later
promotion. `output_bytes` bounds complete canonical report encoding; an
exhausted report is never accepted as a prefix. The candidate above is only a
syntax example, not a recommendation to remove any existing predicate.

Run the original read-only path against an independently retained history
anchor:

```text
supervise_publication create-learned-generated CONFIG RECIPE \
  --resume --policy-replay CANDIDATE_JSON --history-anchor INPUT
```

Retain the **exact output bytes** as the reviewed report, including the final
newline. The report includes the original journal revision, control sequence,
authority epoch, execution/charge/reservation cut, full previous and candidate
policies, every retained current-policy proposal and review case, disposition
changes, newly reviewable cases and shadow requirements. Do not pretty-print,
truncate or substitute summary counts. No favorable case sample is admitted.

Reviewers must consider the report's actual scope: a newly reviewable action
would pass the candidate's exact predicates, but receives no old helper approval
or effect permission. Missing observations remain `requires_shadow`; accepting
relaxations cannot waive them.

## Select a separate governor operation

The closed request schema is `fa.learned-policy-promotion/1`:

```json
{
  "schema": "fa.learned-policy-promotion/1",
  "operation": "901",
  "expected_journal_revision": "123",
  "expected_control_sequence": "7",
  "expected_authority_epoch": "0",
  "candidate_request": "/operator/candidate-policy.json",
  "reviewed_report": "/operator/reviewed-policy.json",
  "accept_newly_reviewable": false
}
```

All four counters are mandatory canonical unsigned decimal strings. Operation
and journal revision are nonzero. The predecessor must be the exact original
report cut, not today's values substituted after a change. Both input paths
must be absolute normalized regular-file paths. The governor request,
candidate request and reviewed report must be retained outside the authority
store. Unknown or repeated fields, unsupported schemas, alternate encodings,
and implicit acceptance refuse.

`accept_newly_reviewable` is always explicit. Set it to `true` only as the
separate authorized governor's acceptance of the **actual complete newly
reviewable set**. The native governor still refuses any shadow-required case.

Invoke:

```text
supervise_publication create-learned-generated CONFIG RECIPE \
  --promote-policy GOVERNOR_REQUEST \
  --history-anchor INPUT --retain-history-anchor NEW_OUTPUT
```

This mode owns its complete argument grammar. It takes no reviewer profile,
actor profile, precheck, live source, helper credibility activation, generation
continuation, stream reset, receipt reconciliation or Stop
option. It admits the existing audit-only or explicitly functional learned
history under the scoped recovery reader. A mediated invocation additionally
requires `--topology-profile` with an exact v2 retained source checkpoint and
`--retain-topology-checkpoint` with a distinct new paired output. Predictive
histories remain unsupported; they cannot fall back to a plain recovery owner.

## Original transitions and durability

The sealed guarded cursor holds the original exclusive lock and canonical
bytes. It verifies the complete original numerical history with the exact
independent recipe, guard inventory, effective policy, credential epoch,
recovery floors, terminal reserve and retained prefix. It captures the selected
original policy corpus and cut **during that same replay**, before the named
original Request event on a retry or at the current Ready cut for a fresh
operation.

The operator encodes that complete original report using the existing renderer
and compares it byte for byte with the independently reviewed input. It checks
the original previous-policy generation and exact installed replay limits.
There is a separate deadline checkpoint after comparison and before mutation.

For a fresh operation, the private original machine executes:

1. `CampaignEvent::Request` for the exact original `PolicyUpdate`.
2. `CampaignEvent::Approve` through the original separately owned governor,
   including shadow and relaxation checks.
3. `CampaignEvent::Promote` through the original one-use promotion gate.

The three original events are encoded and published in **one canonical
replacement**. All three consume ordinary event and byte capacity; they cannot
spend the retained terminal reserve. No intermediate durable pending campaign
or reusable approval key escapes. Guard, policy, credential and floor checks
hold before preparation and again on the validated successor. Canonical bytes
are rechecked before cleanup and replacement.

The original policy transition determines cancellations and accounting. It
cancels only undispatched reservations, withdraws old effect approvals and
advances the original authority/control floors. Admitted and Unknown effects
stay charged; their original endpoint evidence is still required for
settlement. A formerly denied action is not resurrected. Future publication
continues to require fresh policy evidence, native helper review and an
independent human effect approval.

No helper is rerun for the campaign, no current policy-source file is read, no
new logical clock is acquired and no numerical inference resumes. Numerical
work needed to verify the original learned history still runs and keeps its
original replay/work limits. The finite elapsed allowance covers admission,
replay, report comparison and prewrite checkpoints; synchronous framing,
individual replay events and storage I/O are not preemptible time bounds.

## Acknowledgment, failed output and exact retries

The result schema is `fa.learned-policy-promotion-result/1`. It distinguishes
`promoted` from `already_promoted`, reports appended events, the original
reviewed revision, historical promoted generation, current effective policy
generation, current authority/accounting cut, cancelled attempts, explicit
relaxation acceptance, anchor retention and deadline status.

A fresh successful operation appends three events. An exact acknowledged retry
appends zero. The retry must match the original Request/Approve/Promote triple,
the complete original report, exact policy update and original acceptance bit.
A pending, rejected, revoked or partially recorded campaign cannot be converted
into an acknowledged promotion by retrying this command. An intervening event
at the fresh selected predecessor refuses rather than rebasing the request.

After acknowledgment, anchor export and result output are both attempted even
if one fails or the original deadline has elapsed. Those failures cannot undo
the policy transition. Preserve the governor request, candidate document,
reviewed report and old anchor, and retry with a **new** output path.

Recovery must independently select the **current effective policy**. After the
promotion, update the recovery-policy envelope to the exact acknowledged
candidate before retrying; an old expected policy must refuse. The original
governor request and reviewed report remain unchanged. If a later authorized
operation has changed policy again, the recovery expectation must select that
current policy; this operation's receipt remains historical and cannot
promote its older generation again.

## Source and qualification

Native APIs are `FileLearnedPolicyPromotionRequest`,
`FileLearnedPolicyPromotionReview`, and `FileLearnedPolicyPromotionHistory`,
with `require_policy_promotion`, `policy_promotion_review`, and
`finish_promoted_policy` on the original `FileGuardedLearnedRecovery`.
The immutable review is descriptive data; authenticating maintenance authority
and comparing independent reviewed expectations remain the trusted caller's
responsibility. Only the explicit finalizer can mutate policy, and it returns
no live role.

Implementation lives in the native learned recovery `policy_promotion.rs`
module and guarded adapter, and the learned operator `policy_promotion.rs`
module with its one-event recovery adapter. Regression sources cover actual
learned generation/native/human publication, reserved versus Unknown rights,
denied and shadow-required corpora, exact original-cut retries, all five
canonical storage barriers, ordinary event/byte reserve exhaustion, changed
report/intent/canonical bytes, explicit current-policy recovery, deadline cuts,
and failed report transport with successful anchor retention.

This is source implementation under FA-130 and plan §§22.2, 22.4 and 22.7
(FI-A14/FI-A18). It is not executed qualification, a policy-quality result,
independent operator authentication, population safety evidence, or completion
of the broader campaign/shadow deployment work. The repository's RCH-only
verification gate must run against the resulting revision before those tests
receive execution credit.
