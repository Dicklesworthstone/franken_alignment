# Recover the original topology-enforced Tower service

The Linux `supervise_publication` service can reopen an independently retained
mediated owner and continue through its original Tower endpoint:

```sh
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation 41 --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE --operator-profile OPERATOR_PROFILE
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --requests 8,9 --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE --operator-profile OPERATOR_PROFILE
```

Option pairs may follow the positional files in any order. The service accepts
one request or an explicit `--requests` schedule of 1–64 distinct keys, with the
existing plain checked-publication profile and optional separate operator
endpoint. A schedule must start with the actor profile's request. Precheck sidecars,
multiple peers, generated/learned modes and stronger feed, wait, snapshot or joint
witness profiles keep their separate lifecycles. Unsupported combinations refuse
before service setup.

For ordinary campaign-governed recovery without topology, use the separately
selected [live governor recovery command](LIVE_POLICY_GOVERNOR.md#recover-the-governed-owner-with-fresh-independent-custody).
It requires `fa.guarded-submit/2` with full effective-policy and campaign
expectations. The mediated service described here does not provision a live
governor.

## Independent history and source custody

The existing `fa.guarded-submit/1` profile supplies an independent history anchor
and revision/control/authority floors. The topology document must select
`fa.mediated-submit/2`; manual v1 source-work minima cannot substitute for an exact
checkpoint. The [checkpoint operator contract](TOPOLOGY_CHECKPOINT_OPERATOR.md#subsequent-publication)
defines its `initial`, `source_checkpoint`, `next`, `operation` and optional
`retain_checkpoint` fields. The nested checkpoint anchor must equal the selected
history anchor and cover the complete canonical journal. Checkpoint floors can
only strengthen external minima.

Select new `retain_history_anchor` and `retain_checkpoint` destinations to retain
the next pair. Each output uses existing create-new custody rules outside the
authority store. Both outputs and the actor, human-review, Stop and operator
sockets must be canonically distinct, including parent-directory aliases. These
checks precede the clock, listeners and recovery mutation. Creation also checks
all selected endpoint classes before setup.
For a schedule, these checks include every future reviewer and Stop socket;
an output cannot reuse an endpoint merely because that request has not begun.

The original cursor checks source guard, effective policy, initial/current
topology, availability, publication requirements, reserve and all independent
floors. It holds the same exclusive owner while replay advances one original
event per checkpoint. The independent authenticated Stop listener is bound
before historical source loading and replay; each checkpoint observes the
original workflow deadline and polls that same listener. Elapsed checks remain
cooperative: synchronous reads, native transitions and persistence are not
preempted.

A verified pending Stop peer selects the sealed
[ordinary recovery Stop path](MEDIATED_RECOVERY_STOP_PEER.md) at Ready, before
ordinary Fence/Time/Sweep, current-source acquisition or live actor custody.
Complete replay, the exact source checkpoint, publication requirements and all
independent history floors remain mandatory before an offer. A silent or
malformed request terminates that original startup attempt without falling
through to live recovery; no successful Stop receipt or canonical mutation is
manufactured.

Use the existing independent reviewer command with the actor profile's first
selected request key (8 in this example):

```sh
supervise_publication stop-peer REVIEWER_PROFILE 8
```

The exact request applies the original atomic admission Stop/Fence pair without
an effect-clock read, reconciliation sweep, fresh source or new publication.
Unknown and dispatched liabilities retain their charges. Admission acknowledgment
does not attest endpoint drain, helper reaping or remote process termination.
Once acknowledged, both selected custody outputs are attempted independently:
the history anchor comes from the native result and the terminal source successor
comes only from the exact original checkpoint's `after_admission_stop`.
Lost replies, a late workflow deadline or one failed export cannot discard the
acknowledgment or suppress the other export. Startup then terminates without
returning a live owner.

When no Stop is pending at Ready, the original atomic Fence/Time/Sweep
acknowledges once. Its sealed
[checkpoint successor](TOPOLOGY_WORK_CHECKPOINT.md#ordinary-reconciled-checkpoint-successors)
retains the exact resulting history without another replay or current source
read. Every reconciliation outcome and remaining delivery stage is checked before
selecting old or new request work. Dispatching, Unknown and irrecoverably unknown
liabilities refuse further admission and retain their original charges. The same
Stop control, admitted peer, candidate allowance and session timing continue
through source acquisition, initial qualification and the first actor request;
recovery does not drop and rebind the listener. An explicit schedule carries
that original control into its first request before selecting later endpoints.

## Fresh publication and recorded observations

For a new request, both next graph and inventory generations must advance.
Source/checker limits remain identical; spent checks, reserved read bytes and
last source operation survive. The explicit operation must exceed retained/native
floors, with allowance remaining for initial and final checks. The recovered
observer rebinds the original source, which the same driver retains. Native
certification and a post-read clock precede intake. Current actor evidence,
helper congress, independent human approval, both publication keys and final
topology/witness checks still determine publication.

The actor supplies the actual target version, policy epoch, deadline and exact
proposal; recovery never rewrites them. The existing `--credibility-activation`
option applies to new work through its original path, and existing credibility
requirements remain enforced. Operator situation/explanation reads remain
observational.

All scheduled keys are inspected before choosing the recovery path. If any key
is new, one original source rebind and initial qualification serve the entire
schedule, including a retained prefix followed by new work. Source limits,
spent checks/bytes and operation floors are never renewed at handoff. Each new
effect still acquires fresh actor evidence, independent helper/human review,
both publication keys and a separate final topology/witness check. The next
request waits for terminal settlement and confirmed helper reaping. Actor
tickets, operator queries/cursors, connection/exchange limits and the original
service deadline remain cumulative.

A recorded request uses observation-only actor ingress. An exact repeated
proposal receives its original request observation through the existing codec.
This path never opens next topology/inventory, live actor evidence, helper
programs, witness captures or a credibility activation file. Historical initial
topology and independent custody inputs remain necessary. Actor traffic causes
no extra sweep, source refresh or effect, and a recorded transport timeout does
not automatically Stop the settled owner. The sealed successor can retain the
same spent work for another exact paired-custody recovery with missing live assets.
An entirely recorded schedule follows this observation-only path for every key,
even when live source files, witness captures and helper programs are absent.
Historical actor transport failure does not manufacture a Stop or refresh the
saved cut. Any genuinely unresolved old delivery still refuses the whole
schedule before admission, including a schedule beginning with that old request.

Independent authenticated Stop remains available against partial actor frames
and during reply grace. If Stop advances an already-live recorded owner's cut, the new
history is retained and export of the obsolete source checkpoint refuses. The
same limitation applies after ordinary finalization but before a new source is
bound. This differs from the sealed recovery Stop branch above, which owns the
exact pre-finalization cursor and can derive its terminal source successor.
No terminal successor is constructed from arbitrary snapshots.

## Failure, retention and verification

Selected acknowledged custody is retained before releasing the original driver,
including failures before binding and actual source work after binding. Before
binding, a same-cut sealed recovery checkpoint can be exported. After binding,
capture uses the actual original driver source and owner. Both independent
outputs are attempted once; one successful archive does not imply the other
succeeded. Unacknowledged or ambiguous state cannot produce an acknowledged
checkpoint, and export failure cannot authorize a resend. Keep prior archives
until their successors are acknowledged.

Retention precedes direct helper cleanup, which can still make the command fail;
a checkpoint does not attest reaping. The finite
`submit --guarded --topology-profile` command retains its own recorded-request
source-export restriction while sharing the same recovery helper.

The existing regressions cover both open grammars and option orders, actual
plain/checked second publication, cumulative source spending, stale
custody/inventory, exhausted source work, operator reads, final witness refusal,
repeated recorded recovery with missing live assets, genuinely unresolved charged
dispatch, independent Stop and archive/endpoint aliases. Four additional
real-socket regression functions compare plain, checked and scheduled recovery
Stop with the exact native byte oracle; reject silent/malformed peers and
independently wrong requirements; and preserve each custody output after a lost
reply, late deadline or failure of the other export. The recorded-Stop regression
now waits for a complete native actor response to establish its live boundary.

The new integration sources were reconstructed after the editing workspace
became unavailable and independently reviewed. After the workspace returned,
all eleven changed Rust files passed installed-rustfmt syntax processing; both
new test files were formatted and passed focused format checks, and
`git diff --check` passed. The renewed focused native/example RCH attempt still
exited 127 because the required runner was absent. Its raw log is retained in
`artifacts/execution/2026-11-11-mediated-stop-integration-verification.log`.
Compilation, Clippy, runtime execution and full qualification remain pending.
No bead or packet is closed. Modeled
topology completeness and trusted storage, clock and custody retain their
original deployment assumptions.
