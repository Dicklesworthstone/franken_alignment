# Contain and restore a topology-protected predictive stream

The Linux learned operator can prepare, narrow and restore an owned predictive
text stream while retaining the original topology source and its spent work.
This is the complete original prompt before the first sampled token. It uses
the same native checkpoint, narrowing, restart audits and lifetime prediction
process as [predictive stream reset](LEARNED_PREDICTIVE_STREAM_RESET.md), with
the additional independently retained topology/history pair required at every
offline step.

The useful path is fresh topology-backed preparation, checkpoint-bound
narrowing, reset initiation, reset completion, and then ordinary live
continuation under a newer graph and inventory, fresh native certification,
an owned forecast, native helper review and a separate human decision. A reset
or a topology checkpoint supplies none of those publication keys.

## Select the immutable numerical contract

Use an explicitly declared `functional_restart` host profile, a learned text
stream, the required owned K/V pre-output predictor and forecast artifact
`fa.learned-forecast/2` with:

```json
"reset": "owned_prompt_audit"
```

This remains the existing `with_owned_forecast_stream_restart_audit()` recipe.
The operator independently reloads the same model, tokenizer, monitor, fitted
codec, predictor and budgets for every command. Raw predictors, optional
predictors, legacy recipes, sampled histories, pending forecasts, lost coverage
and unreported predictor work remain ineligible. The lifetime likelihood process
is neither cleared nor rearmed.

Fresh preparation currently refuses computed identity, credentials,
checked-publication and held-out activation selections. Those stronger contracts
must have their own pre-output preparation integration; omitting them cannot
open a history that originally required them. An explicitly selected campaign
inventory remains part of native bootstrap and every later recovery guard.

## Prepare the original prompt and retain both archives

Select a topology creation profile with an explicit new checkpoint destination:

```json
{
  "schema_version": "fa.mediated-create/2",
  "topology_path": "/operator/initial-live-topology.json",
  "inventory_path": "/operator/initial-live-inventory.json",
  "retain_checkpoint": "/operator/custody/prepared.topology"
}
```

The topology and inventory are intentional creation inputs. They must bind the
configured scope and publication target and retain the original bounded source
and checker limits. The original topology owner performs its normal live source
check and cut certification before prompt inference reaches the selected
checkpoint. A graph with an uncovered bypass cannot become a successful
preparation result.

Use the existing closed `fa.learned-generation-checkpoint/1` selection. Its
position is the exact tokenized prompt length, and `expected_sampled_draws` is
`"0"`. Then run:

```sh
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --prepare-stream-checkpoint CHECKPOINT_PROFILE \
  --retain-history-anchor /operator/custody/prepared.anchor \
  --topology-profile CREATE_TOPOLOGY_PROFILE
```

Both outputs must be new files outside the authority store. Their normalized
paths must differ; this check happens before store creation. A v1 creation
profile without a topology output is insufficient for this maintenance mode.
An existing recovery profile cannot be substituted for creation inputs.

Successful preparation stops immediately after the original native capture,
before a forecast, random draw, actor request or effect. It does not append Stop
or Fence. The two exports describe that same unchanged acknowledged cut, and
the topology archive retains its original documents, source checks, reserved
read bytes and last operation. Retention does not reread the live documents or
obtain another source allowance.

The `fa.learned-stream-preparation-result/1` report now includes
`topology_checkpoint_retained`, alongside `history_anchor_retained`, the exact
checkpoint coordinates, sampled work and original accounting. Its value is
`null` for an unmediated preparation. Pre-capture errors use original Stop/drain
and attempt to retain both kinds of custody. Post-capture archive, report or
deadline failures preserve acknowledgment and do not repeat prompt inference.

The ordinary `--checkpoint-before-generation` option also requires explicit
paired outputs when topology is selected, and an exact v2 input pair when
continuing. That live command proceeds to forecasting and sampling after capture;
it does not promise a later prompt-only reset. Use preparation to retain the
unsampled maintenance boundary.

## Select an exact historical pair for each offline step

Capture, narrowing and reset initiation accept `--topology-profile` in their
existing complete command grammar. The recovery profile must be
`fa.mediated-submit/2`, with a retained topology checkpoint and a new output:

```json
{
  "schema_version": "fa.mediated-submit/2",
  "initial": {
    "topology_path": "/operator/retained/initial-topology.json",
    "inventory_path": "/operator/retained/initial-inventory.json"
  },
  "next": {
    "topology_path": "/operator/future/topology.json",
    "inventory_path": "/operator/future/inventory.json"
  },
  "operation": 10,
  "source_checkpoint": "/operator/custody/prepared.topology",
  "retain_checkpoint": "/operator/custody/narrowed.topology"
}
```

The original profile parser still requires a representable operation strictly
above retained source work. Offline control does not consume that operation,
load the `next` files, construct a live source, certify a cut or renew any
allowance. The `next` paths can therefore be unavailable during these offline
operations. The independently retained initial documents and selected checkpoint
remain required historical inputs.

The selected topology checkpoint's embedded history anchor must equal the
separately supplied `--history-anchor` input. Manual v1 minima do not establish
that equality. A changed initial graph, omitted topology guard, mismatched pair
or shared output path refuses before canonical mutation.

## Capture, narrow and initiate the reset

Preparation has already captured its checkpoint. An exact capture retry is
available with the complete original `fa.learned-stream-checkpoint/1` request:

```sh
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --capture-stream-checkpoint CAPTURE_REQUEST \
  --history-anchor PREPARED_ANCHOR \
  --retain-history-anchor CAPTURED_ANCHOR \
  --topology-profile CAPTURE_TOPOLOGY_PROFILE
```

Use the existing complete `fa.learned-stream-narrow/1` request to narrow the
captured prompt:

```sh
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --narrow-stream-checkpoint NARROW_REQUEST \
  --history-anchor PREPARED_ANCHOR \
  --retain-history-anchor NARROWED_ANCHOR \
  --topology-profile NARROW_TOPOLOGY_PROFILE
```

Narrowing checks the original actor/checkpoint, exact current journal, control
and authority predecessor, explicit evidence binding and selected versions of
the same target. It intersects the current target ceiling, withdraws old keys
and the accepted topology cut, and cancels/refunds only original undispatched
reservations. It preserves graph availability, spent source work, policy and
predictor generations, actor state, numerical work, Unknown liabilities and the
out-of-band reset incident count. A caller-supplied evidence binding is not an
authenticated incident detector.

The original narrowing receipt's `first_incident_sequence` supplies the later
frontier for the separately requested reset. Its current control sequence is
not substituted when an exact retry returns after later work. Select every
target version that later messages and Finish will need: the target ceiling
includes `expected_version`, and a later reset cannot restore a dropped right.

Use the newly retained pair to initiate the existing complete reset request:

```sh
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --begin-stream-reset RESET_REQUEST \
  --history-anchor NARROWED_ANCHOR \
  --retain-history-anchor PENDING_ANCHOR \
  --topology-profile BEGIN_TOPOLOGY_PROFILE
```

These commands consume one original guarded replay sealed to the selected
operation. Their successful return contains immutable control history and its
matching topology checkpoint, with no live owner, observer, human key or Fence.
The `fa.learned-stream-control-result/1` report includes separate retention
results for the history and topology archives.

## Complete the reset, then qualify live continuation

Complete the same reset using the pending pair:

```sh
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --complete-stream-reset RESET_REQUEST \
  --history-anchor PENDING_ANCHOR \
  --retain-history-anchor RESTORED_ANCHOR \
  --topology-profile COMPLETE_TOPOLOGY_PROFILE
```

The original sealed native finalizer performs the requested restart and its
recovery Fence atomically. It reserves both original audits, freshly audits the
retained cache and the actual restored successor cache, conserves all work and
budgets, and increments the incident count through the original containment
transition. Completion returns a paused owner only inside this bounded
maintenance operation; the operator exports its acknowledged result and drops
its roles. The resulting topology checkpoint preserves the same source work
and documents while recording unavailable topology after Fence.

An authenticated stop profile remains available on completion. A checked Stop
retains priority at the original Ready boundary. The existing
`--recover-stopped-stream-reset` command can confirm and re-export an already
recorded terminal cut under the same owned predictor and exact pair; it does not
complete a reset or issue live custody.

Live continuation separately selects a v2 topology recovery profile whose
`next` graph and inventory generations both advance and whose source limits
remain unchanged. Supply the restored history anchor and new paired output
paths to the ordinary `--continue-generation` command. Original recovery,
topology source binding, fresh cut certification, owned forecasting, numerical
generation, native review, human decision and final publication validation all
remain mandatory. An uncovered bypass in the newly selected graph cannot reach
a forecast or a human approval request.

## Acknowledgment and retry boundaries

Both archives and the report are attempted independently after native
acknowledgment, even when the original deadline has expired or one output fails.
Create-new output semantics never replace an existing archive or competing file.
An archive error is not permission to repeat inference, source acquisition,
narrowing, refund or a restart audit.

For capture, narrowing and reset initiation, an exact old pair admits its own
unchanged cut or the single matching selected event needed to repair a lost
export. The finalizer replays and compares the original witness. An unrelated
source, numerical, clock or Fence suffix cannot be silently covered by older
source-work accounting. Select the same original request and new output paths;
an exact retry appends zero events. After successful export, advance using the
newly matched pair.

Completed reset retries follow the existing exact reset-plus-Fence suffix law.
They retain the original numerical result, two audits, incident count and source
work; only the ordinary recovery Fence repeats. A fresh preparation cannot be
rerun against an existing store. If preparation itself loses source custody,
preserve the canonical store and any successful export; this interface does not
manufacture the missing topology-work archive from the journal alone.

## Evidence and limits

Eight operator regression functions exercise actual preparation, paired offline
capture and narrowing retries, reset initiation/completion, later fresh
topology/forecast/native/human publication, wrong pairs, changed graphs,
uncovered bypasses, explicit recipe/output admission and lost archive/report
recovery, including an archive race after the preparation owner is released.
The fixtures use imported native numerical assets and the original
file-backed authority and topology owners.

The edited Rust files have passed syntax inspection. Required remote RCH
compilation, formatting, Clippy and runtime execution remain pending. These
source regressions do not qualify a physical mediation perimeter, authenticate
the source documents or incident detector, establish calibrated predictions,
demonstrate reset secrecy, or close FA-108/FA-111 or H17. Predictive resets after
sampling remain outside this prompt-only capability.
