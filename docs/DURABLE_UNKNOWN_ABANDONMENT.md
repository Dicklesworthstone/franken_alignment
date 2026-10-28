# Explicit durable unknown-delivery abandonment

`FileDelivery::abandon_unknown` and `FileOversight::abandon_unknown` persist the
original broker's trusted `Unknown -> IrrecoverablyUnknown` transition. This is
the bounded FA-014 reference implementation of plan sections 8.2-8.5 and
16.4/16.9, serving FI-A02/FI-A06 and FA-INV-005/006. The operator records an
irrecoverable liability; the operation does not establish an endpoint outcome.

## Authority and retry contract

The trusted supervisor supplies a `FileAbandonRequest` with a nonzero operation
and attempt plus the independently selected control sequence and authority
epoch. A new operation requires that exact current predecessor and an original
Unknown attempt. Dispatching, pre-dispatch and already settled attempts refuse.
Retention expiry alone still leaves Unknown and never selects abandonment.

The owner also requires its current journal revision. An exact retry of a
previously acknowledged operation returns the original `FileAbandonReceipt`
without another event, even after a recovery Fence advances the current epoch.
Changing any field under that operation ID refuses. A new operation cannot
rewrite an already irrecoverable attempt. A poisoned owner refuses both mutation
and receipt lookup until canonical recovery; an I/O error cannot prove that a
replacement did not become visible.

The receipt binds the complete original `FrozenAction`. Its `charged_units()`
are that action's original conservative charge, not a refund or available
balance. The receipt is not an `EndpointReceipt` and cannot be accepted as one.
No actor port acquires this supervisor-only mutation.

## What the transition preserves

The native reducer changes the selected disposition and the durable host removes
only that attempt's retained sendable envelope. The same owner therefore cannot
later publish that envelope. Other eligible requests keep their original path.
The operation neither contacts the endpoint nor clears a pending stream: a
message or Finish successor still cannot advance past the unresolved attempt.

Available, reserved and charged units; previously published payloads; execution
count; target version; clocks; policy; authority epoch; source observations; and
numerical continuation state keep their original meanings. In particular an
already executed but unreconciled effect remains visible and charged. A normal
trusted reconciliation before abandonment can still establish its real outcome.
Ordinary reconciliation cannot subsequently rewrite the terminal disposition.

Actor polling continues to return the existing Unknown outcome. A pending sweep
omits irrecoverable attempts, but Stop progress retains each in both `unresolved`
and `irrecoverable`; the domain is not drained. An empty sweep is never evidence
that the liability disappeared.

## Storage and compatibility

Base journal tag 18 carries only the four request integers. Its record occupies
37 bytes including framing, or 38 bytes inside the observed journal's Core
wrapper. Replay recomputes the receipt through the original reducer; imported
receipt data cannot invent a disposition. Repeated canonical abandonment events
are invalid even though an exact public API retry is read-only.

Existing bootstrap encodings, version domains and event tags are unchanged.
Histories without this event retain their existing bytes. Older readers reject
the unfamiliar event instead of ignoring it; a history containing abandonment
must stay with a reader that supports tag 18. No permissive downgrade is supplied.

Abandonment uses **ordinary journal capacity**. It cannot spend the existing
three-event, 50-byte Fence/Stop/StopProgress reserve. Exhausted ordinary capacity
refuses explicitly; the change does not reinterpret an existing reserve or
promise unlimited restarts. Both dimensions are checked before persistence.

## Guarded recovery without reopening work

`FileGuardedRecovery::finish_abandoned_with_anchor` and
`FileMediatedRecovery::finish_abandoned_with_anchor` consume the original private
recovery cursor after complete replay. They recheck the complete independently
selected guard inventory, effective policy, credentials and recovery floors.
Mediated recovery retains its original initial/current topology, availability
and prediction/evaluation requirements. Anchored entry points also require the
independently supplied history prefix; the operator below always selects an
anchored entry point. A specialised cursor never exports a weaker base cursor or
accepts a public replacement checker.

A fresh disposition compares the canonical image before mutation and again
before storage publication, then applies the same native abandonment event.
`FileAbandonedHistory` contains only the receipt, acknowledged snapshot and exact
history anchor. No live owner, permit or role can be extracted. No recovery
Fence, fresh effect-clock sample, source refresh, helper request or endpoint
operation is introduced. Future live recovery still requires its original Fence
and complete guards.

An exact retry returns the original receipt with the **current acknowledged**
snapshot and history anchor, which may include later events. It writes no new
event and remains possible after ordinary journal capacity is exhausted. Keep
the request's original sequence and epoch even when the current snapshot has
advanced. A failed native replacement returns no receipt or candidate anchor,
even if the new canonical image may already have become visible.

## Offline operator command

The Unix reference executable maps this restricted disposition to the existing
`resolve` verb:

```sh
supervise_publication abandon-unknown \
  /operator/publication.json \
  /operator/abandon-request.json \
  --guarded /operator/abandon-recovery.json
```

The grammar is closed: `--guarded` is mandatory, and no shared submit, reviewer,
activation or recovery-mode flags are accepted. `CONFIG` supplies the original
source-publication policy and source guard requirements.

The request is a bounded 4 KiB JSON object. The IDs below are illustrative; supply
the independently selected original attempt and exact predecessor:

```json
{
  "schema_version": "fa.abandon-unknown/1",
  "operation": 1001,
  "attempt": 7,
  "expected_control_sequence": 23,
  "expected_authority_epoch": 4
}
```

All four values are unsigned integers. Operation and attempt must be nonzero.
Unknown or duplicate fields, missing fields and malformed values refuse. Retain
the request file and keep every request field unchanged for an exact retry.

The base recovery profile is a bounded 16 KiB object:

```json
{
  "schema_version": "fa.guarded-abandon/1",
  "minimum": {
    "journal_revision": 87,
    "control_sequence": 23,
    "authority_epoch": 4
  },
  "history_anchor": "/operator/custody/publication-before.anchor",
  "retain_history_anchor": "/operator/custody/publication-abandoned.anchor"
}
```

The example floors are illustrative lower bounds; select them independently.
They do not replace the exact request predecessor or the independently retained
history anchor. Both archive paths must be absolute and outside the authority
store. The output parent must already exist, and the output must be a new file.
Export uses exclusive creation with mode `0600`, file synchronization and
parent-directory synchronization. Existing archives are never replaced.

Mediated source publication selects version 2 and supplies the complete topology
requirements:

```json
{
  "schema_version": "fa.guarded-abandon/2",
  "minimum": {
    "journal_revision": 87,
    "control_sequence": 23,
    "authority_epoch": 4
  },
  "history_anchor": "/operator/custody/publication-before.anchor",
  "retain_history_anchor": "/operator/custody/publication-abandoned.anchor",
  "topology": {
    "initial_topology": "/operator/custody/initial-topology.json",
    "initial_inventory": "/operator/custody/initial-inventory.json",
    "current_topology": "/operator/custody/current-topology.json",
    "current_inventory": "/operator/custody/current-inventory.json",
    "available": false
  }
}
```

The topology and inventory files are independently retained requirements, with
absolute paths outside the authority store. `available` must explicitly match
the replayed state; both available and withdrawn topology states retain their
original checks. Loading these requirements does not certify topology for new
work. Custody retains the existing guarded recovery assumptions about trusted
parent directories; an archive is not a new authenticator of source origin.

Version 1 cannot silently ignore mediation. A stop, resume or submit profile
cannot select abandonment. This command supports the exact base or mediated
source-publication inventory; version 2 adds topology requirements only. It does
not expose the native mediated cursor's optional prediction/evaluation profiles,
infer learned or numerical requirements, or drop stronger guards to recover
their histories through this narrower command.

### Acknowledgment, reporting and retries

One monotonic elapsed budget begins before configuration, request and profile
reads. The command checks it throughout admission and event-by-event replay,
including the final Ready checkpoint. It reads the selected configuration,
request, recovery profile, independent custody files and canonical journal; it
does not acquire live source, helper or reviewer evidence or a fresh effect
clock. Its store must preserve the original terminal recovery reserve.
Abandonment still consumes only ordinary capacity.

After native acknowledgment, the command attempts **both** independent history
retention and the bounded JSON report, even if the deadline has elapsed or
retention fails. It checks elapsed time again after retention and reporting. A
later failure returns a nonzero result that explicitly names the acknowledged
revision; it cannot roll back the native disposition or refund the charge. Keep
the prior archive and select a fresh output path for an exact retry. A partial
or already written output file is never overwritten by that retry.

The `fa.abandon-unknown-result/1` report wraps the recorded disposition in a
`Knowledge` value with a bounded-model basis. Its nested `effect_outcome` remains
`Unknown { OutcomeUnknown }`. The report does not establish authentic origin,
source truth or endpoint nonexecution. It includes the acknowledged revision,
control sequence, authority epoch, total charge, selected action's charged
units, execution count, independent retention status and unresolved/irrecoverable
IDs. It exposes no action payload, source bytes, helper views or anchor contents.

If native storage returns an unconfirmed result, inspect canonical history
before choosing any different request. The original operation/request can be
retried through the same complete guards and independent history custody. A
successful retry acknowledges the existing disposition without resending the
effect or appending another event.

## Learned numerical histories

The learned operator accepts the same closed `fa.abandon-unknown/1` request
through its original recipe and independently retained history:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --abandon-unknown REQUEST_JSON \
  --history-anchor INPUT_ANCHOR \
  --retain-history-anchor NEW_OUTPUT_ANCHOR
```

The complete command grammar is checked before options are removed or files are
opened. Both history paths are mandatory. The command cannot combine abandonment
with resume, receipt reconciliation, Stop, credential rotation or revocation,
reviewer/actor service, generation continuation or credibility activation.

Historical recipe loading preserves the exact original model, tokenizer,
numerical budgets, learned monitor, raw or owned predictor, computed identity,
checked-publication policy and independently selected current credential epoch.
It does not load live helper rosters, provider secrets or current evidence. One
original guarded cursor replays the complete journal in one-event quanta, with
elapsed-budget checks before opening, at every event and at Ready. The consuming
native finalizer appends only the selected ordinary-capacity `AbandonUnknown`
event, or confirms an exact historical retry. It does not issue a recovery
Fence, acquire effect time, advance numerical work, refund exposure or expose a
live owner or sendable envelope. Dispatching and already confirmed attempts
remain ineligible; a timeout is not sufficient evidence for abandonment.

### Paired topology custody

An originally mediated learned history additionally requires its original v2
`fa.mediated-submit/2` profile and a separate new source-checkpoint output:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --abandon-unknown REQUEST_JSON \
  --history-anchor INPUT_ANCHOR \
  --retain-history-anchor NEW_OUTPUT_ANCHOR \
  --topology-profile RETAINED_TOPOLOGY_PROFILE \
  --retain-topology-checkpoint NEW_OUTPUT_CHECKPOINT
```

The profile's retained checkpoint and the independent history anchor must name
the same original cut. Manual v1 source minima cannot establish this complete
custody. The native checkpoint finalizer verifies the original topology,
availability and floors before cleanup or mutation. The acknowledged successor
copies the exact retained topology and inventory documents, source work and last
source operation, while advancing the paired history anchor and its floors.
Availability remains unchanged because abandonment performs no recovery Fence
or source refresh. No current or next live topology document is read.

If acknowledgment succeeds but either output fails, retain the original input
pair and retry the exact original request with fresh output paths. This
restricted consumer permits either the saved exact cut or exactly one matching
`AbandonUnknown` successor after it. An unrelated suffix, changed operation,
changed predecessor or mismatched input pair refuses before staging cleanup.
Ordinary live and receipt recovery retain their existing exact source-extent
requirement; they cannot reuse this interrupted-export allowance.

### Learned acknowledgment and reporting

After acknowledgment, both independently requested exports are attempted before
the final stdout report, even if a deadline expires or either export fails.
Outputs use the existing outside-store, create-new, private-file and sync
boundaries. Existing outputs are never overwritten. A late deadline, archive
failure or stdout failure returns an error naming the current acknowledged
revision and directs an exact retry; it cannot erase the native disposition.

The bounded `fa.learned-abandon-unknown-result/1` report includes separate
`history_anchor_retained` and `topology_checkpoint_retained` results (the latter
is null when no topology was selected), the native charged liability and
unresolved/irrecoverable IDs. The disposition has a bounded-model `Knowledge`
basis and `authority: "none"`; the effect outcome remains
`Unknown { OutcomeUnknown }`, including when the original endpoint execution is
already present but was never confirmed to the actor. No private model, source,
action payload, provider credential or archive bytes are reported.

## Validation boundary

The authored base and two-key regression cases exercise original dispatches,
state and predecessor refusals, exact retries, retained execution, actor and
stream behavior, canonical encoding/replay, ordinary capacity and all five
native replacement fault barriers.

Nine additional guarded regression functions target native byte/state
equivalence across replay quanta, complete requirement checks after Ready,
invalid intents and non-Unknown stages, partial and interrupted replay,
independently anchored forks and suffixes, terminal reserve preservation, exact
retry at physical capacity, all five replacement barriers, and mediated
topology/availability requirements.

Twelve operator regression functions target closed request/profile parsing
including the mediated schema, command routing, a real expired Unknown attempt,
required independent output custody, source/floor/predecessor refusals,
pre-acknowledgment and post-acknowledgment elapsed checkpoints, stdout write/flush
failure, output-file races, exact retry and recovery without live evidence/helper
inputs. These are authored tests, not execution results. Fault injection is not
a physical power cut or proof about a remote endpoint.

The preparation environment has no RCH executable or configured worker. The
pinned nightly-2026-09-08 rustfmt parser provides syntax/format checking only; no
local compilation fallback is used. Compilation, runtime tests, Clippy, canonical
admission and full FA-014 completion remain pending. The existing dependency
admission guard is unchanged, and the scoped bead remains in progress.
