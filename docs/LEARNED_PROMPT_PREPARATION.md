# Prepare and retain an unsampled learned-stream prompt

`--prepare-stream-checkpoint` creates a fresh learned owner, completes its
original monitored prompt, captures the independently selected checkpoint, and
returns before obtaining a forecast or drawing output. The retained history is
an active, quiet prompt boundary. Successful preparation appends no Stop or
recovery Fence and admits no actor request.

This supplies a normal operator entry point for
[owned predictive stream containment](LEARNED_PREDICTIVE_STREAM_RESET.md): prepare,
narrow the original authority, begin the original reset, complete it, then resume
through fresh prediction and ordinary native and human review. An operator can
also explicitly continue a prepared prompt without selecting a reset. Preparation
itself grants neither reset permission nor either publication key.

## Inputs and command

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --prepare-stream-checkpoint CHECKPOINT_PROFILE \
  --retain-history-anchor NEW_OUTPUT \
  [--campaign-profile CAMPAIGN_PROFILE]
```

The command is fresh creation only. `CONFIG` must explicitly declare
`functional_restart`; an audit-only configuration is insufficient and no stronger
restart grade is inferred. `LEARNED_RECIPE` must select the original plain owned
K/V stream predictor and `fa.learned-forecast/2` with
`"reset":"owned_prompt_audit"`. The numerical recipe, tokenizer, prompt, fitted
codec, monitor, predictor, stream limits, policy-source contract and terminal
reserve are installed through their existing native constructors.

The checkpoint profile reuses the closed `fa.learned-generation-checkpoint/1`
schema:

```json
{
  "schema": "fa.learned-generation-checkpoint/1",
  "checkpoint": "71",
  "expected_position": "2",
  "expected_sampled_draws": "0"
}
```

All coordinates are canonical unsigned decimal strings. Checkpoint ID and
position must be nonzero. The position must equal the complete original prompt
under its selected tokenizer; it is not a character or byte count. The example
describes a two-token prompt. The owned prompt-audit recipe requires zero draws.
A wrong position refuses at the first actual output boundary before acquiring
any forecast or sampling intent. The command does not continue until a later
position happens to match.

`NEW_OUTPUT` is mandatory. Its parent must already exist, and it must name a new
file outside the authority store. Admission resolves the original parent and
rejects existing output paths. The actual export repeats create-new admission,
uses private file permissions, and synchronizes both file and parent directory.
The output is the exact native history anchor captured while the successful
owner remains healthy, not a newly opened or later fenced history.

An optional campaign profile uses the existing
[first-image campaign contract](LEARNED_CAMPAIGN_CREATION.md). Its full inventory
is installed before prompt work. Later recovery must independently select that
same inventory through its recovery-policy envelope; preparation does not create
a reusable governor or let a later command omit the guard.

## Supported scope

The complete grammar accepts only the checkpoint profile, new history output,
and optional campaign profile. Recovery flags, an input history anchor, actor or
reviewer service selection, live prechecks, topology, Finish, simultaneous live
checkpoint capture, duplicate flags and held-out activation are refused before
creation.

This bounded preparation path also refuses numerical recipes requiring computed
identity, checked final publication, credential custody or held-out joint
activation, and refuses nonzero recovery floors. It does not drop those
requirements to obtain a simpler owner. Raw predictors, legacy owned predictors,
nonpredictive streams, and topology-backed histories remain outside this mode.
An existing store is never reopened or replaced by preparation, including after
a previous preparation's report was lost.

No actor/reviewer socket or helper process is started. The original native
helper execution assets are loaded by the later live release command; they are
not needed to prepare a prompt. The complete numerical and predictor contracts
and all selected first-image guards are still admitted before creation.

## Original generation and acknowledgment

Preparation uses the same original generation scheduler as live publication.
Every prompt quantum retains the existing source freshness, logical deadline,
wall deadline and native intent/completion checks. Historical replay custody is
returned before capture. The checkpoint binds the actual actor revision,
authority epoch, control sequence, source stream and exact confirmed audience.

The scheduler returns a distinct maintenance outcome immediately after durable
capture. It cannot be interpreted as a completed message. No forecast, sampled
token, stream append, sidecar verdict, human approval, permit, dispatch or
publication is produced. The source actor state and numerical work remain in
the original journal; dropping the successful owner does not add a Fence.

The command emits `fa.learned-stream-preparation-result/1`. Its fields are:

| Field | Meaning |
| --- | --- |
| `outcome` | `prepared`: native capture was acknowledged |
| `checkpoint` | The independently selected checkpoint ID |
| `revision` | The exact acknowledged journal/anchor revision |
| `actor_revision`, `authority_epoch`, `control_sequence` | Original saved capture coordinates |
| `stream`, `position`, `sampled_draws` | Original saved numerical boundary |
| `sampling_attempts` | Cumulative original sampled work; zero for this profile |
| `executions`, `charged`, `reserved` | Current original external-effect ledger |
| `checkpoints`, `restart_attempts` | Original checkpoint and reset usage |
| `history_anchor_retained` | Whether the separately selected archive was fully retained |
| `deadline_reached` | Whether the original deadline had been observed at the reporting boundary |
| `effect_authority` | Always `not_issued` |

Numeric report fields are decimal strings, preserving the full unsigned range.
The report describes an observation; it does not authenticate an incident or
replace the retained binary anchor. Narrowing and reset requests remain
independently selected instructions and must match their actual original cuts.

Ordinary continuation can subsequently use:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE \
  --continue-generation \
  --history-anchor PREPARED_ANCHOR \
  --retain-history-anchor NEW_CONTINUED_ANCHOR
```

That separate live command performs its original recovery Fence, fresh source
and clock observations, explicit numerical resume, fresh forecast, monitored
sampling, native helper review and independent human approval. It retains spent
prompt work and cannot reuse preparation as a publication approval. When
campaigns were installed, the recipe in this command must carry their exact
independent recovery expectation.

## Failure and lost acknowledgment

An admission failure creates no owner. After creation, a prompt, source,
deadline or checkpoint-boundary failure uses the original Stop/drain path and
attempts to retain its resulting history. Admitted numerical work is preserved;
missing evidence or an incorrect boundary does not authorize sampling. A
terminally stopped preparation is not relabeled as a reusable prompt cut.

After successful capture, archive, report or late-deadline failure is different:
the checkpoint is already acknowledged. Retention and report delivery are both
attempted, and the failure message identifies acknowledgment. These later
failures do not issue Stop, add a Fence, repeat prompt inference or discard the
checkpoint. A destination created concurrently is left unchanged.

If report delivery fails but the anchor was retained, preserve that original
archive and inspect the acknowledged checkpoint through the original recovery
interfaces before selecting a next instruction. Re-running this fresh command
refuses the existing store and cannot replace its history.

If the initial anchor export itself fails, the checkpoint remains in the
original store, but independent retained custody may be unavailable. The command
does not invent an input archive from the mutable store or claim a partially
written destination as successful custody. Preserve the store and any successful
output, inspect the actual acknowledgment, and retain independent evidence before
choosing another operation. No automatic retry can silently solve a missing
first archive.

## Costs and verification scope

Prompt inference, source checks, monitoring and numerical journal operations
use their existing lifetime limits. Cooperative quanta bound historical reducer
calls; they are not a wall-time bound on individual numerical or filesystem
operations. Original checkpoint capture still performs its synchronous bounded
historical verification and serialization. This mode makes no new latency,
calibration, restart secrecy or deployment containment claim.

Regression sources in
`examples/supervise_publication/workflow/actor_service/learned/tests/checkpoint_prepare.rs`
exercise real prompt preparation, exact campaign guards, zero-event capture
recovery, later native and human publication, closed admission, wrong boundaries,
missing source evidence, and post-acknowledgment export/report failures. The
predictive stream-reset tests use this same production command before narrowing
and original reset. These additions require the repository's RCH verification
gate; this implementation session did not execute Rust compilation or tests.
