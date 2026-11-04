# Prepare, contain and restore a predictive learned stream

An explicitly selected owned K/V predictor can now accompany a real operator
workflow from fresh prompt preparation through containment, reset and ordinary
reviewed publication. The profile admits only the complete original prompt
before any cumulative sampling. It preserves the original model, predictor,
policy, audience and all work allowances throughout the sequence.

## Select the immutable reset profile

Keep the original `fa.learned-publication/5` recipe and all of its numerical,
sidecar, review and stream contracts. Its forecast file may use
`fa.learned-forecast/2`, whose one additional required field is:

```json
"reset": "owned_prompt_audit"
```

This value is supported only with `source.kind: "owned_learned_kv"`. The reader
pins the original owned predictor and per-message continuation before applying
the separate `FALKRFS\x01` stream restart recipe. Every recovery command must
independently select those exact bytes. A legacy `/1` artifact and a `/2`
artifact with `reset: "disabled"` retain the same original recipe bytes and
refusals. The new field cannot retrofit an existing journal.

The bootstrap Config must explicitly declare `functional_restart`. A declaration
does not qualify an external model or host. This workflow supports plain and
topology-backed histories. Mediated commands require the independently retained
source/history pairs and fresh certification described in the
[topology-protected predictive workflow](LEARNED_MEDIATED_PREDICTIVE_STREAM_RESET.md).
The examples below show plain custody. The create-only
preparation command has additional guard limits documented in
[prompt preparation](LEARNED_PROMPT_PREPARATION.md).

## Reach the unsampled prompt through the actual CLI

Use the exact `fa.learned-generation-checkpoint/1` profile from
[stream checkpoint control](LEARNED_STREAM_RESET_OPERATOR.md). Its position is
the selected original prompt length and `expected_sampled_draws` is `"0"`.

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --prepare-stream-checkpoint CHECKPOINT_PROFILE \
  --retain-history-anchor PREPARED_ANCHOR
```

This command uses the original cooperative numerical engine and source/time
checks. It acknowledges the paired checkpoint, retains its history and exits
before acquiring a forecast, drawing a token or submitting an actor request.
Its successful maintenance exit does not Stop or Fence the owner. The report
contains the actual checkpoint and journal coordinates needed for independently
selected control. It grants no effect key or reviewed message.

The existing `--checkpoint-before-generation` live option captures and then
continues generation in the same invocation. Successful publication changes
audience knowledge, and any sampled work makes this predictive reset profile
ineligible. Use the explicit preparation mode when a retained pre-output pause
is required.

## Narrow the original authority at that checkpoint

The capture sequence must precede the reset's first incident sequence. Applying
the registered `narrow` consequence provides that real containment transition
while preserving the predictor's policy generation:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --narrow-stream-checkpoint NARROW_REQUEST \
  --history-anchor PREPARED_ANCHOR --retain-history-anchor NARROWED_ANCHOR
```

The closed `fa.learned-stream-narrow/1` request supplies the complete original
checkpoint and current predecessor. All integers are canonical unsigned decimal
strings. The following values illustrate the shape; use the actual separately
retained preparation coordinates and independently selected target versions:

```json
{
  "schema": "fa.learned-stream-narrow/1",
  "expected_journal_revision": "12",
  "checkpoint": {
    "checkpoint": "71",
    "actor_revision": "2",
    "authority_epoch": "0",
    "control_sequence": "0",
    "stream": "5",
    "position": "2",
    "sampled_draws": "0"
  },
  "control": {
    "operation": "801",
    "expected_control_sequence": "0",
    "expected_actor_revision": "2",
    "expected_authority_epoch": "0",
    "binding": {
      "round": "801",
      "reducer_generation": "1",
      "evidence_root_hex": "0707070707070707070707070707070707070707070707070707070707070707"
    },
    "retained_targets": [
      {"adapter":"1","object":"1","contract_version":"1","expected_version":"1","generation":"1"},
      {"adapter":"1","object":"1","contract_version":"1","expected_version":"2","generation":"1"}
    ]
  }
}
```

The request is bounded to 256 KiB, depth 6, 16,384 JSON items and 64-byte strings.
The native target list is bounded by `MAX_ATTEMPTS`. Unknown/duplicate fields,
noncanonical counters, a different checkpoint, changed predecessor or different
publication endpoint refuse before canonical mutation. Only the explicit owned
prompt-audit recipe admits this mode.

The original gate intersects the current target ceiling, revokes the authority
epoch and old keys, and cancels only undispatched reservations. Admitted and
Unknown effects retain their charges. Actor state, policy and predictor
generations, numerical work, likelihood evidence, source/audit allowances and
the incident count remain unchanged. One ordinary journal event records the
request and full recomputed witness. It cannot spend the terminal reserve.
For a mediated history, narrowing also withdraws the accepted topology cut
while preserving its graph and source expenditure. An old certification cannot
be used at the new authority epoch.

Each target includes `expected_version`. A Message followed by Finish needs
both exact target versions; two Messages followed by Finish need three. Every
version must be selected in both narrowing and the later reset. The reset
intersects the current ceiling again and cannot recover a version omitted
earlier. An empty retained set is complete containment.

The result uses `fa.learned-stream-control-result/1` with `outcome: "narrowed"`
or `"already_narrowed"`. `first_incident_sequence` is the **original narrowing
receipt's sequence**, even when a historical retry observes a later current
`control_sequence`. The trusted supervisor's selected evidence binding records
the operation; it does not authenticate a detector or prove model misconduct.

## Begin and complete the separately selected reset

Build the existing complete `fa.learned-stream-reset-recovery/1` request from
[the reset contract](LEARNED_STREAM_RESET_OPERATOR.md#request-format). Select a
fresh reset operation and binding, the same captured checkpoint, the actual
current actor/control/authority predecessor, the independently retained exact
targets and a finite per-audit restart allowance. Set `first_incident_sequence`
to the original narrowing receipt's sequence.

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --begin-stream-reset RESET_REQUEST \
  --history-anchor NARROWED_ANCHOR --retain-history-anchor PENDING_ANCHOR

supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --complete-stream-reset RESET_REQUEST \
  --history-anchor PENDING_ANCHOR --retain-history-anchor RESTORED_ANCHOR
```

Beginning records the original intent without a Fence. Complete that exact
intent before any ordinary reopen can interrupt it. Completion performs the
original checkpoint audit and a second complete audit of the actual restored
successor cache. Both allowances are reserved before either audit; failed work
remains charged. The successor audit feeds only the owned predictor. Ordinary
learned source evidence remains Empty until new accepted numerical work.

Completion and its ordinary recovery Fence are published in one canonical
replacement. It preserves the world, confirmed audience, target ceiling,
revocation floor and lifetime budgets. Only the reset increments incident
accounting. It leaves the owner paused and does not open publication endpoints.

## Continue through the original review and effect path

```text
supervise_publication create-learned-generated CONFIG NEXT_REQUEST_RECIPE \
  REVIEWER_PROFILE --continue-generation \
  --history-anchor RESTORED_ANCHOR --retain-history-anchor PUBLICATION_ANCHOR
```

Keep the original numerical and forecast recipe. Preparation admitted no actor
request, so its unused original request ID may be used for this first release.
The ordinary command obtains fresh source/time, resumes the original numerical
owner and acquires a new owned forecast before the first sampled-token intent.
Mandatory native review, independent authenticated human approval, one-use
authorization, dispatch and reconciliation all remain required. Human rejection
still prevents publication. Further Message and Finish requests require their
original receipt-bound continuation and separately retained target versions.

## Failure and retry boundaries

Narrowing, offline capture and reset initiation acknowledge immutable results
without returning a live owner or roles. For a plain history, an exact original
request and anchor can be retried with a new output destination; the same
operation appends zero events, including after later publication. A mediated
history additionally seals the exact source/history pair: its unchanged cut or
single matching selected event can repair a lost export, but unrelated later
work requires newer independently retained custody. A changed binding, ceiling or
predecessor under the same ID refuses. Report and anchor failures do not undo an
acknowledged consequence. Completed reset retries preserve both audits and the
incident count and append only the ordinary recovery Fence.

Fresh preparation is create-only. Its acknowledged prompt cannot be regenerated
by rerunning the command, and a failed initial archive export does not silently
create independent custody from today's store. Preserve the store, successful
archive and original report. See the preparation contract for the exact failure
outcomes.

Pending/lost forecasts, crossed likelihood evidence, unreported work, any
cumulative sampling, changed audience or terminal Stop remain ineligible for
this reset path. Post-sampling reset needs a separate same-origin statistical
contract. Native and operator regression sources cover the real lifecycle,
negative custody/recipe cases, accounting and lost acknowledgment; compilation,
runtime execution, calibration and deployment qualification remain pending.
