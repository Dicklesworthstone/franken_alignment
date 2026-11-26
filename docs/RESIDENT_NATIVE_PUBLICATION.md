# Resident native generated-text publication

`supervise_publication serve-native-generated` serves an explicitly bounded
sequence through one original native generated-text owner. The decoder, tokenizer,
KV/RNG state, stream, source custodian, journal and cumulative work survive each
successful release. The service does not reopen and replay the journal between
Messages.

Each entry still requires an independently authenticated actor to submit its
original source reference, fresh source evidence, original process-helper review
and a separate human approval. The supervisor never submits a reference on the
actor's behalf. Generated text remains private until the original publication
gate admits its cumulative frame.

This is a bounded reference service under plan sections 8.7, 11.2, 16.4/16.9 and
17.1 and 22.7, serving FA-017, FA-025, FA-026, FA-107 and FA-130. Compilation and runtime
qualification of this increment remain pending the required RCH verifier.

## Select the complete sequence

```sh
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --publication-profile WITNESS_PROFILE
supervise_publication serve-native-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST
supervise_publication serve-native-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST --credibility-activation CAPSULE
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --operator-profile OPERATOR_PROFILE --tower-generation NONZERO
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --operator-profile OPERATOR_PROFILE --tower-generation NONZERO \
  --governor-profile GOVERNOR_PROFILE
```

The optional publication profile retains the native service's existing simple
checked-publication contract. It applies to the same original owner and every
selected release. The command owns its complete grammar before reading files;
unsupported options are not consumed by another service.

For creation, a version-one sequence contains 2 through 16 entries. The first
entry is a Message. An optional Finish appears only once, last. Message followed directly
by Finish is valid, as are several Messages with no selected Finish. A sequence
that ends after a Message does not implicitly close the stream.

```json
{
  "schema": "fa.native-publication-sequence/1",
  "entries": [
    {
      "actor_profile": "actors/first.json",
      "recipe": "recipes/first.json",
      "release": "message"
    },
    {
      "actor_profile": "actors/second.json",
      "recipe": "recipes/second.json",
      "release": "message"
    },
    {
      "actor_profile": "actors/finish.json",
      "recipe": "recipes/second.json",
      "release": "finish"
    }
  ]
}
```

Paths may be absolute or relative to the sequence profile's directory. Empty,
NUL-containing and parent-traversal paths are refused. Each actor profile names
its own nonzero request ID and endpoint. Every selected request ID is distinct;
each Message selects a distinct native generation ID. Finish repeats the
immediately preceding Message's generation and complete generation request,
while using its own actor request ID. Explicit recovered suffixes may start
with a final Finish using the retained predecessor's exact original recipe.

All recipes and actor profiles are loaded before the first clock observation,
listener or durable owner. Existing strict native recipe loading admits the
complete model, tokenizer, monitoring, sampling and text-generation inputs.
Every entry must agree on the exact decoder configuration, tokenizer encoding
and stream profile. Model names or equal tensor shapes do not establish this
equality. Message prompts and explicit generation requests may differ within
those original contracts.

There are at most sixteen bounded native recipe loads. Each retains the existing
per-recipe asset ceiling, so startup asset reads are bounded by sixteen times
that ceiling. Equal later decoder configurations share the original immutable
buffers after compatibility validation. This avoids retaining duplicate model
buffers; it does not claim that each recipe's validation performs no model I/O.

Before live setup, endpoint validation includes all selected actor and reviewer
sockets, the retained Stop endpoint, the evidence source and authority store.
Canonical aliases cannot let a later listener replace an earlier endpoint or
an input needed by the original owner. No existing path is removed to make a
listener bind succeed.

## Exact policy prechecks for selected releases

Version two uses the same command and bounded sequence rules, while requiring
every entry to select `precheck_profile` as a path or explicitly use `null`.
Paths follow the same sequence-relative rules as actor and recipe paths.
Version one remains closed and does not accept this field.

```json
{
  "schema": "fa.native-publication-sequence/2",
  "entries": [
    {
      "actor_profile": "actors/first.json",
      "recipe": "recipes/first.json",
      "release": "message",
      "precheck_profile": "prechecks/first.json"
    },
    {
      "actor_profile": "actors/finish.json",
      "recipe": "recipes/first.json",
      "release": "finish",
      "precheck_profile": null
    }
  ]
}
```

Each selected disclosure uses the existing uncached
`fa.actor-precheck-service/1` exact-predicate profile and the existing
`actor-precheck` client. Cached and full-policy profiles are refused for this
generated-text selection. All disclosure profiles are read before any model
recipe. The complete schedule must fit the native aggregate query, exchange,
candidate and connection ceilings and use distinct disclosure IDs. Canonical
aliases against every future actor, reviewer, Stop and disclosure endpoint, the
evidence source, authority store and selected operator endpoint are refused
before model loading. A later release cannot create a fresh invocation-wide
allowance.

The sidecar starts only after the original Message completes or Finish is
prepared, with independent Stop already active. It evaluates that exact
cumulative frame and its charged units, rather than a caller-supplied replacement
payload. Query turns do not sample tokens, refresh source evidence or append
journal events. Each reconnect retains the same finite disclosure. The sidecar
is retired before independent review, request handoff and every error exit.

A passing precheck supplies neither publication approval nor a permit. The
actor must still submit the original source reference; actual admission
reacquires source evidence and retains the existing helper, human and final
publication checks. Original Stop and deadline checks surround bounded
precheck handling before actor mutation.

Every profile independently selects its expected policy epoch. A new owner's
initial epoch is zero, so a nonzero first disclosure is refused before recipe
reads, clocks, listeners or authority-store creation. Correcting that selection
can therefore retry creation without recovering an accidentally created store.
Recovery retains its separate original-owner check after any recovery Fence,
before live gateway setup. When explicit recovery requalification is selected,
the first disclosure must name its single activation successor epoch; that
selection is checked before the capsule read and again against the actual owner
after activation. No selected expectation is rewritten. Each later disclosure is checked when its release begins,
before qualification, source refresh or numerical work. A later row may
explicitly name the epoch resulting from an independently authorized narrowing
operation. Neither recovery nor a query rewrites a stale expectation to make it
pass. Historical recovery changes retain their existing meaning: a rejected
stale suffix may already include its original recovery Fence.

The optional independent operator can be selected with either manifest version.
Its original service, grants and deadline remain resident while each separately
selected disclosure retires at its own admission boundary.

## One owner, independently approved releases

For the first Message, the original native bootstrap and monitored generation
path establish the source-bound stream. Later Messages use the existing
`prepare_decoder_text_continuation` and `begin_decoder_text_continuation`
operations against the preceding confirmed request. They retain the original
numerical predecessor, cumulative spending and stream prefix. No second live
decoder, replacement source or new authority ledger is constructed.

After generation completes on its monitored control token, the supervisor emits
one unchanged actor Submit document to stdout and flushes it. Each line contains
the fixed-size source intent and original proposal fields, not generated output.
The actor uses its selected profile with the existing `actor-submit` command.
The admitted original source intent must exactly match that selected reference;
a different valid Message or Finish cannot replace it under the same request ID.
The service then drives the existing fresh-source, process-helper and independent
human-review workflow. Checked publication still obtains its final witness inputs
at the original publication boundary.

A successor is eligible only when the preceding request has both the original
`Confirmed` disposition and an `Executed` outcome. The original driver's
`retire_completed_request` must then confirm that its direct helper children are
reaped before another actor listener is bound or another generation starts.
Denied, cancelled, held, unexecuted and unknown outcomes cannot stand in for a
confirmed published prefix.

The independently admitted helper-program configuration is retained as an
immutable template. A fresh bounded set of helpers can start for the next
request only after the previous review's retirement. No helper process, review
transcript, vote or human key transfers to that next request.

Explicit Finish uses the original confirmed-prefix builder and performs no
sampling. It is still a separate effect requiring its own actor submission,
helper review, independent human approval and any selected final witness check.

## Recover an explicit remaining sequence

`--after-message PREVIOUS_REQUEST` selects an existing confirmed native Message
and a new suffix of 1 through 16 entries. The same option can accompany
`--publication-profile WITNESS_PROFILE`, in either order; each option appears
once. Omitting it continues to select creation. A recovered suffix may consist
only of Finish, using the preceding Message's original generation and complete
recipe; this closes the original prefix without sampling another token.

The service binds the first suffix actor and its independent Stop before replay.
It uses the existing cooperative native opener, checking Stop and the first
entry's original logical and wall deadlines between historical events and at
Ready. An authenticated Stop during replay uses the original terminal recovery
path; it cannot fall through into live generation or publication. Ordinary
successful recovery appends one original recovery Fence and returns a fresh
reviewer role. That same owner, numerical state and Stop session then serve the
complete suffix, without reopening between its entries.

Use `--after-message` recovery at the original confirmed-Message boundary. An
accounted service can instead explicitly select
[`--continue-generation`](#resume-interrupted-accounted-generation) for its
original pending or completed-but-unsubmitted first Message. Ordinary unaccounted
pending generation retains the existing single-release recovery path. No mode
can skip outstanding numerical or publication work.

The complete remaining selection must describe new work. All actor request IDs
must be absent from the retained request history, and every selected Message's
generation ID must be unused. The predecessor must satisfy the original native
confirmed-cut and continuation or Finish checks. A historical receipt, unknown
effect, earlier superseded Message or altered Finish recipe cannot authorize the
suffix. The original stream's remaining Message allowance also bounds the full
selection. These historical checks run after the opener's ordinary Fence; a
rejected selection does not imply that recovery left the journal byte-identical.
It emits no new source reference and starts no new numerical work or review.

Recovery preserves original numerical spending, retained effect charges,
confirmed prefix, source identity and complete model/tokenizer/publication
contracts. Fresh source evidence and an explicit original resume still precede
any new numerical work or Finish. Every new release receives fresh helper and
human approval at the recovered policy epoch. An old recorded request's deadline
or approval is never renewed. Existing stale credibility qualification refuses unless the operator
explicitly selects the recovery requalification described below; the command
never infers or renews a qualification.

This is the same source-bound native recovery contract as the single-release
native opener. Select an independently retained history archive below when
recovery must compare a known original prefix. Historical replay remains
synchronous within each original event; the service's checkpoints do not
establish a hard real-time Stop bound.

## Independently retained history

Creation and explicit suffix recovery accept
`--retain-history-anchor NEW_OUTPUT`. Recovery also accepts
`--history-anchor INPUT` together with `--after-message PREVIOUS_REQUEST`.
These selections compose with checked publication, per-release prechecks,
the independent operator and explicit credibility requalification. Accounted
recovery also accepts the explicit `--continue-generation` mode described
[below](#resume-interrupted-accounted-generation), under its separate supported
composition; it requires both input and new output history anchors.

```sh
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --retain-history-anchor anchors/prefix.bin

supervise_publication serve-native-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST \
  --history-anchor anchors/prefix.bin \
  --retain-history-anchor anchors/successor.bin
```

The selected input must be a private regular archive outside the authority
store. Its bounded contents are decoded and frozen before recipe/model reads
or any live clock. A later pathname edit cannot replace the retained selection.
Missing, malformed or incompatible input refuses; the command never substitutes
the current journal or falls back to unanchored recovery.

Under the original exclusive recovery lock, the anchored cursor compares that
exact retained prefix before historical numerical replay, staging cleanup or
Fence. A valid older journal, a same-counter divergent history or a different
store identity cannot satisfy the selection. A genuine append-only successor
still undergoes all original replay and source/model/tokenizer checks. Anchor
custody supplies no publication approval, fresh source evidence or credibility.

Input and output paths must be distinct from one another, the evidence source,
authority store, selected credibility capsule and every scheduled actor,
reviewer, Stop, precheck and operator endpoint. They must also remain separate
from CONFIG, SEQUENCE_PROFILE, REVIEWER_PROFILE, the selected publication and
operator profiles, and every manifest actor, recipe and precheck file.

The complete command input names are checked before any configuration read.
The manifest name is checked before it is opened; all of its selected input
names are checked before actor, precheck or model reads. Canonical resolution
includes symlinked parent directories and future files which do not exist yet.
Endpoint checks still run before recipes and again before live setup. The
output's parent directory must already exist, and its filename must be unused.

The same healthy original owner captures the acknowledged successor after the
sequence's work. When an input was selected, that capture must extend its exact
prefix. Output uses the existing bounded archive format, an exclusive new
private file, and file plus parent-directory synchronization. An existing archive
is never overwritten. Keep the previous independent archive until its successor
has been acknowledged.

An acknowledged create or recovery Fence is also retainable when a later
read-only startup check refuses the suffix. Authenticated terminal recovery
Stop instead supplies its original sealed acknowledgment, including when a
reply or later deadline check fails. Retention does not reopen a live owner or
replay the journal again. A poisoned final owner refuses export and cannot
silently substitute an earlier in-memory capture.

An export failure makes the command fail while preserving its original
publication or Stop outcome. It never resubmits a confirmed release or issues
another Stop to recover from an archive error. An incomplete output is not an
acknowledged checkpoint; preserve prior custody and select a new output name.

These archives establish continuity from the selected original prefix. They
do not authenticate the latest head, recover unseen later events or detect
rollback of both the journal and its independently retained archive. They
contain private original journal data and are not actor-facing output.

Two paired profile regressions exercise both input and output aliases for
every command role and selected manifest input. Independent output paths provide
positive controls, including loading a real compatible three-entry schedule
without creating an archive, owner or listener.

Two additional combined regressions run retained history through actual
one-time credibility activation and separately approved suffix releases. They
remove the capsule after its first use and check the final retained cut,
cumulative work, effect charges and fresh human sessions. Their negative case
restores a valid older journal while the capsule and evidence source are
missing: the stronger retained prefix must refuse first, with no replay Fence,
reference output or staging cleanup. These are authored and independently
reviewed sources; compilation, formatting, runtime tests, Clippy and required
RCH qualification remain pending.

A further paired regression covers stale qualification evidence and a deadline
immediately after a successful activation. Both refuse new generation and
publication while retaining the actual acknowledged failure/Stop history. The
selected prior archive remains unchanged, and sealed replay verifies the exported
successor without adding another recovery Fence. This coverage shares the
qualified-success fixtures above and retains the same pending execution boundary.

## Explicit recovery requalification

A resident suffix can select `--credibility-activation CAPSULE` together with
`--after-message PREVIOUS_REQUEST`. This composes with the existing publication
and independent operator selections, including the complete governed recovery
selection below. Creation rejects the option before configuration reads or
clocks; an actor submission cannot supply the capsule.

The original cooperative recovery and complete suffix-history admission run
first. The existing native qualification operation then reads and validates the
bounded capsule exactly once, under the same independent Stop and first-entry
deadline. Scope, original committee, current control predecessor, policy epoch,
evidence requirements, freshness and any retained joint policy remain the
native operation's responsibility. The file supplies evaluation evidence; it
does not authenticate an evaluator or import a score as permission.

A successful new activation advances the original authority epoch once.
Historical operation retries remain historical receipts and cannot qualify a
new live suffix. Any first precheck explicitly selects that resulting epoch:
the service checks the anticipated successor before reading the capsule and
rechecks the actual owner after activation. Later releases check retained
qualification and their own selected current epochs. They never reopen the
capsule or activate it again. A missing, stale or incompatible capsule cannot
fall back to the recovered qualification.

Canonical admission reserves the capsule name against the authority store,
evidence source, every scheduled actor/reviewer/Stop/precheck endpoint and the
selected operator and governor endpoints before recipe/model reads. The original bounded file
reader still runs at the controlled qualification boundary. These path checks
do not establish a hostile-filesystem isolation guarantee.

With the complete governed recovery selection below, the same capsule is also
reserved against the independent governor endpoint. The retained governor may
operate after qualification, but any previously approved campaign is withdrawn
by that transition. A later promotion needs a fresh campaign and approval under
the actual resulting epoch; neither service allowances nor campaign capacity
are renewed by qualification.

Requalification precedes new source refresh, numerical resume, generated
reference output and helper review. Stop and the original deadline are checked
both before and after the operation. Fresh evidence, the original generation
and cumulative work budgets, every helper review and every independent human
approval remain mandatory. A final Finish can be requalified and published
without additional inference. If a later approval fails, the previously
confirmed prefix remains published and no later entry starts.

Recovery and an accepted activation are durable transitions. A later refusal
does not roll them back or promise byte-identical history. Authored regressions
pair real Message/Message/Finish recovery with lost and historical evidence,
stale disclosure epochs, human refusal, source loss and Stop/deadline boundaries.
Compilation and runtime qualification remain pending the required RCH verifier.

## Recover a governed native sequence

Governed suffix recovery has an explicit complete selection:

```sh
supervise_publication serve-native-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST \
  --operator-profile OPERATOR_PROFILE --tower-generation NONZERO \
  --governor-profile GOVERNOR_PROFILE \
  --recovery-requirements RECOVERY_REQUIREMENTS \
  --history-anchor PREVIOUS_INDEPENDENT_ANCHOR \
  --retain-history-anchor NEW_OUTPUT_ANCHOR
```

The new output archive is optional. The other recovery selections above are
required together. An original checked-publication profile can also be selected
with `--publication-profile`; recovery must preserve its exact original witness
and held-out joint contracts. Topology selection and the single-release command
family remain outside this mode.

### Requalify the original helper congress after recovery

Add `--credibility-activation CAPSULE` to the complete governed recovery command
to select one independently supplied qualification for the recovered suffix.
A retained held-out joint publication profile is selected through the existing
`--publication-profile` wrapper; it is never projected to its marginal or
unchecked form. The recovery requirements and anchor still describe the exact
original history and current policy. Neither the capsule nor the governor
profile can supply a missing recovery expectation.

The original guarded replay and single recovery Fence run first. The native
activation then consumes the capsule once before the first new release, under
the same Stop service and deadline. Successful activation advances the authority
epoch once and withdraws old effect keys and campaign approvals. It retains the
fresh recovered human and governor roles, original numerical spending, confirmed
prefix and unresolved charges. Every later Message or Finish uses the retained
qualification; a handoff cannot reread the capsule or renew its allowance.

Successful credibility activation is not effect approval. New releases
still require their own source evidence, helper review, independent human key
and final publication checks. Governor custody remains usable for the original
policy-replay protocol, but a subsequent policy promotion invalidates the old
qualification. The service must refuse later publication until a separately
selected fresh qualification is available; it cannot transfer the previous
policy's credibility evidence into the new policy. This mode does not add a
live per-release activation channel.

The capsule must be separate from the governor socket and every other selected
custody path. These checks precede model reads or listener creation. Stale
capsules, stale first-release disclosure epochs, deadline or Stop after
activation, and human rejection do not acquire publication authority. An
acknowledged recovery/activation remains part of the retained failure history.
Creation still refuses credibility activation. It may install the original
held-out joint guard while using the unchanged baseline committee, as described
below. The new composition and regression sources require RCH compilation and
runtime qualification before execution credit is assigned.

The remaining sequence contains 1–16 genuinely new requests and may consist
only of the final Finish. A Finish preserves its confirmed Message predecessor
and exact recipe without another sampled draw. New Message generations retain
the original decoder, tokenizer, stream limits and cumulative numerical work.
Creation keeps its original 2–16-entry contract.

### Independently retained recovery requirements

The recovery file is bounded to 64 KiB and uses this closed schema. The values
below illustrate the format; supply the actual complete current policy and
independently retained campaign settings and history floors for the run.
Keep the original bootstrap policy in `CONFIG`; put the complete currently
promoted policy in `effective_policy`. Recovery uses those two policies for
different checks and must not replace the bootstrap policy with the current one.

```json
{
  "schema": "fa.native-recovery-requirements/1",
  "effective_policy": {
    "generation": 2,
    "nodes": [
      { "op": "payload_at_most", "maximum": 2048 }
    ]
  },
  "guards": {
    "campaigns": {
      "cases": 16,
      "input_bytes": 1048576,
      "max_campaigns": 4
    },
    "decoder_stop": null,
    "identity": null,
    "credential": null
  },
  "minimum": {
    "journal_revision": 70,
    "control_sequence": 9,
    "authority_epoch": 1
  }
}
```

Every field is mandatory. Duplicate or unknown fields, invalid campaign bounds
and non-null unsupported guards refuse. Explicit absence is a requirement,
never permission to drop a guard found on disk. The independently supplied
original configuration and recipes pin source, stream, decoder and tokenizer;
the file neither replaces those inputs nor infers expectations from the
journal. Campaign settings must also exactly match the independently selected
governor profile. Effective-policy predicates are compared as well as the
generation, so a same-generation policy substitution refuses.

The command reserves the requirements file against the authority store, evidence
source, every scheduled actor/human/Stop/precheck endpoint, governor and operator
endpoints, and selected input/output archives before model assets or live
custody are opened. Paths do not constitute a hostile-filesystem guarantee.

### Recovery and live role custody

The [guarded native cursor](GUARDED_NATIVE_RECOVERY.md) holds one exclusive
canonical image and the original numerical reconstruction. Its independently
retained anchor is checked before the first replay event. Complete guard,
current-policy and history-floor checks must succeed before the original
single recovery Fence returns fresh human and governor roles. A missing
selection cannot fall back to the unguarded native opener.

Replay uses the same one-event checkpoints and an additional Ready checkpoint
under the resident invocation's deadline and independently authenticated Stop.
A pending Stop consumes the same verified cursor through the original terminal
Stop path before any ordinary recovery Fence or live role is issued.
A deadline refusal during replay or at Ready, before either finalizer begins,
changes no canonical or staged bytes. A deadline or transport failure after an
acknowledged Stop or Fence does not undo that durable event.

The recovery Fence withdraws old roles, source/time eligibility and approval
keys. Confirmed output, numerical spending, retained campaign use and unresolved
effect charges remain. A later policy change still requires a fresh original
corpus replay, independent governor approval and separate operator promotion.
Every new publication needs fresh source evidence, helper review and human
approval.

The service captures the acknowledged recovery cut before fallible suffix
preflight or later operator/governor listener startup. Its existing retention
path also preserves acknowledged terminal Stop. Failure to publish a new archive
never retries an effect or
overwrites the prior independent archive. A post-Fence refusal can therefore
leave the acknowledged Fence in history; it does not roll back recovery.

## Independent live operator

Creation and explicit suffix recovery accept the paired
`--operator-profile OPERATOR_PROFILE --tower-generation NONZERO` selection,
alongside either of the existing publication and recovery options. The profile
uses the existing native generated-text operator contract. It must explicitly
select each observation or control grant; numerical reconciliation, cancellation,
abandonment, narrowing and Fence retain their original replay allowances.

One authenticated operator service is installed on the original generated-text
owner. Its native grants, registration, connection and exchange counters, and
deadline survive Message/Finish handoffs. A new entry does not recreate the
listener, renew a grant or replenish spent work. The effective operator lifetime
is bounded by its own profile, the first selected actor's runtime and the
original invocation deadline. Later entries cannot extend that lifetime.

The operator can inspect the original run and its requests while numerical work,
actor intake, independent review, reply grace and helper retirement proceed.
Separately selected native controls act on that same owner. A granted Fence is
observed through the original Stop cleanup before forward work continues.
Operator access never submits an actor's source reference or supplies either
publication approval. Transport failure retires the sidecar without creating a
replacement service or granting new authority.

The profile may also select bounded `convening` authority for an original held
Message or Finish. The same `next` / explicit `convene` exchange used by
[native single-request services](NATIVE_TEXT_PUBLICATION_SERVICE.md#explicit-continuation-after-hold)
requests one fresh review of that exact generated release. No later generation
starts while it is held. Its output, numerical spending, confirmed audience and
original deadline remain fixed; fresh helper review still needs an independent
human key before publication. The one original query/operation allowance spans
the entire sequence, including reconnects and later releases. An exhausted grant
does not mint another one. The same selection is available to governed sequences
and new unrecorded suffix work after recovery, with the original governor and
recovery checks. It cannot revive an old held predecessor or mutate a recorded
receipt. Omitting it preserves the original post-Hold cancellation behavior.

A Fence accepted before actor intake or during predecessor retirement retains
that selected entry's one unused reply interval. The same operator can finish
its response or answer an already queued exact retry before retirement. This
terminal interval starts no actor intake, numerical work, helpers or publication.
It uses the original invocation deadline, operator lifetime and spent grants;
ordinary per-entry reply handling consumes the interval instead of adding a
second one. An already attempted automatic Stop/drain is never repeated merely
to service a historical reply.

All selected actor identities and future actor/reviewer/Stop endpoints are
checked against the operator profile before model assets or live custody are
opened. Canonical path aliases cannot place the operator listener over a future
endpoint, the evidence source or the authority store. During suffix recovery,
independent Stop remains available throughout the cooperative replay; live
operator custody is installed only after the original owner has been recovered.

Use the existing `tower-operator-exchange` client with this profile and the exact
run registration. The complete profile, grant and response contracts are in
[native Tower publication](TOWER_PUBLICATION.md#operate-an-original-native-generated-text-stream).

## Independent live evaluation and Andon accounting

Fresh resident native creation accepts the paired
`--andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE` selection, with a
required new independently retained history-anchor output. The same original
evaluator observes and assesses applied reviews across the selected Messages
and optional Finish while the native owner retains its journal lock.

```sh
supervise_publication serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE \
  --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --retain-history-anchor /absolute/independent/native-final.anchor \
  --publication-profile WITNESS_PROFILE \
  --operator-profile OPERATOR_PROFILE --tower-generation 41
```

The publication and paired operator selections in this example are optional.
Publication accepts the existing simple witness profile. An operator that selects
the finite `governance_ledger` grant can read acknowledged credibility and Andon
accounting through Ledger; the evaluator remains a separate role and endpoint.
The sequence retains the ordinary creation rule of 2 through 16 entries, with
its own actor submission, fresh helper review and independent human approval
for every release.

`ANDON_PROFILE` accepts marginal `fa.native-andon-operator/1` or the explicitly
selected joint `fa.native-andon-operator/2` profile. Their examples are
`crates/fa-reference/fixtures/native_andon_operator.json` and
`crates/fa-reference/fixtures/joint_native_andon_operator.json`. Creation
requires identical initial/current windows and minimum evaluation revision zero. The
[atomic source-bound constructor](NATIVE_GENERATED_EVALUATION.md)
installs the exact source policy, decoder, tokenizer, generated stream, terminal
recovery reserve, selected witnesses, evaluation protocol and Andon accounting
in one first acknowledged image. With `/2`, it also installs the exact original
joint helper-weight policy in that same image before any evaluator is released. It returns the independent evaluator only
after that acknowledgment. Accounting cannot be attached after a proposal or
substituted for original publication checks.

`EVALUATOR_PROFILE` uses `fa.native-evaluator-peer/1`. Its complete scope and
deployment/evaluator audience must match the original accounting configuration.
Its supervisor credentials must match, and its evaluator credential policy must
be disjoint from every scheduled actor, the human reviewer/Stop and any selected
operator. The evaluator socket must remain distinct from all future service
endpoints, selected configuration/profile files, source and witness inputs,
recipe/model/tokenizer assets and the retained anchor output, and outside the
authority store. These checks precede numerical asset loading and live setup;
recursive recipe assets are checked before their own reads. A later recipe's
refusal may follow earlier bounded asset reads, but no clock, listener or owner
has been acquired. The service requires the configured Unix-millisecond clock
domain used by native assessment.

The separately authenticated evaluator uses the existing commands:

```sh
supervise_publication evaluator-case EVALUATOR_PROFILE APPLIED_ROUND
supervise_publication evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON
```

Use the Known case identity and its exact global evaluation revision to prepare
the assessment, with independently supplied origin, evidence and truth. A later
review or assessment can stale that predecessor. Neither command supplies a
human key or approves publication. The complete wire, custody and retry rules
are in [independent live evaluation](LIVE_ANDON_EVALUATION.md).

One original listener, evaluator role and finite candidate, I/O and byte allowance
survive successful Message handoffs and Finish review. A case read and its later
assessment each spend a candidate; rejected credentials and malformed requests
also spend the original allowance. Reconnects and later entries cannot replenish
it. The evaluator lifetime is bounded by its own profile, the first selected
actor's runtime and the original invocation deadline. Expiry or exhausted
transport retires that endpoint without renewing it or granting publication
authority. Independent Stop remains ahead of each bounded evaluator turn.

After forward work finishes, a healthy unstopped owner remains available for
independent evaluation under the remaining original endpoint and invocation
budgets. This includes a final helper Hold: the evaluator can discover its actual
case and supply a benign, violation or censored label even though no successor
entry may start. A case read and assessment spend two existing candidate slots.
This phase schedules no numerical work, helper launch, further publication or
replacement service. Original operator grants and actor receipts remain bounded
by their existing contracts. Actor receipt transport failure retires that intake
and retains the error while preserving the evaluator's remaining opportunity.
Stop, storage failure or a failed original control checkpoint ends new evaluation.

Only after that bounded opportunity ends does terminal reply handling begin.
Evaluator intake is finished: the role and listener are dropped, partial or
queued new requests cannot label a case, and only an already produced response
may drain. A failed Stop checkpoint also ends evaluator intake before fallback
reply work. The service fully retires the evaluator before capturing the final
original history anchor, so acknowledged assessment Time and label events are
included and no evaluator mutation can follow that retained cut. Archive custody
and export failures retain the [existing history contract](#independently-retained-history).

Creation and the explicitly anchored recovery below support marginal `/1` and
joint `/2` accounting. A joint history requires every field of its independently
selected original policy, including identity, generation, rate fractions and
replay ceilings. Omitting or changing that policy refuses before the recovery
Fence; the command cannot infer a weaker contract from disk.

With `/2`, the existing operator pair can retain an explicitly funded
`credibility_promotion` grant on the same numerical owner. Selecting that grant
without a joint policy or its numerical replay allowance refuses before startup.
The evaluator supplies the original independent labels; the operator supplies
only an operation and exact predecessor. One custodian retains its original
queries, replay funding and single entered mutation across all handoffs and
connections. See the
[resident native promotion selection](LIVE_CREDIBILITY_PROMOTION.md#select-a-jointly-evaluated-resident-native-service)
for complete response capacity and explicit operator requests. Version-two sequence rows may independently select the
existing exact-predicate precheck for each Message or Finish, or explicitly use
`null`. The original evaluator role and finite allowance survive these actor
disclosures and all later handoffs. Each disclosure keeps its own original query
grant, shares it with any selected actor Tower precheck, and retires before helper
and human review; a passing result supplies no publication key.

The complete precheck endpoint inventory is admitted with the evaluator,
actor, human, Stop and operator endpoints. Equality and ancestor/descendant
aliases against those endpoints, the authority store, source, accounting and
recovery inputs, selected profiles, recipe assets and retained histories refuse
before live startup. Pending-generation recovery is explicitly a recovery mode,
even without a confirmed Message predecessor. Its first selected disclosure
binds the actual post-Fence owner epoch before source acquisition or numerical
resume. Later stale expectations cannot initiate a successor.

Policy governance, topology, offline credibility activation and the separate
held-out joint publication guard still require their own compositions and are
refused. The newly supported Andon `/2` policy qualifies original helper-weight
updates; it does not supply a missing held-out publication contract. No selected guard
is silently removed. Single-release native and learned modes keep their separate
grammars.

Authored composition regressions cover actual fixture Message/Message/Finish
generation, a separate evaluator process, original-case observation and labels,
Ledger accounting, finite candidates across handoffs, a final helper Hold with
independent false-stop accounting, independent Stop and deadline exits, custody
aliases and the terminal reply barrier. They establish
test intent, not executed results or detector quality. Compilation, runtime
tests, Clippy and the required gate remain unqualified because remote-only RCH
is unavailable.

Four additional joint-service regression definitions cover actual generated
Messages and Finish with original helper processes, independent human keys and
separately authenticated labels; rejection of Finish's fresh key; missing joint
or numerical grant selection; and omitted or changed recovery-policy fields
paired with an exact resumed Finish. The evaluator/operator listeners and paid
query allowance remain original across handoffs. Their benign-only evidence
correctly remains marginally unqualified; these fixtures do not claim successful
weight promotion. All four changed Rust files passed scoped parsing/formatting
with the available 2026-08-30 rustfmt. Compilation and runtime execution remain
pending.

### Recover an accounted resident suffix

Select a confirmed original Message and the remaining sequence explicitly:

```sh
supervise_publication serve-native-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --after-message PREVIOUS_REQUEST \
  --andon CURRENT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --history-anchor /absolute/independent/original-native.anchor \
  --retain-history-anchor /absolute/independent/new-native.anchor \
  --evaluation-recovery RECOVERY_REQUIREMENTS
```

The existing optional simple publication profile and independent operator pair
can accompany this selection. Recovery requires all three explicit inputs:
the predecessor, independent input anchor and requirements file, together with
the paired accounting/evaluator profiles and a new independent retained output.
The remaining sequence contains 1 through 16 new releases. It may consist only
of Finish, using the confirmed predecessor's exact generation and recipe, with
no new token generation.

`RECOVERY_REQUIREMENTS` uses the same closed, 4 KiB
[`fa.native-evaluation-recovery/1` schema](LIVE_ANDON_EVALUATION.md#recover-an-evaluated-native-generated-service)
as the single-request service. The journal revision, control sequence and
authority-epoch floors are mandatory independently retained expectations.
`CURRENT_ANDON_PROFILE` separately supplies the original identity and protocol,
initial windows, complete current window inventory and minimum evaluation
revision. The current config, recipes and optional simple witness profile pin
the complete supported source, decoder, tokenizer, stream, publication and
unchanged policy contracts. Missing selected expectations cannot be inferred
from the journal being checked.

Single-request and resident accounted recovery share the same checked driver
over `FileAndonGeneratedSourceRecovery`. The original Stop and invocation
deadline are checked before open, at every one-event boundary and separately
at Ready. An authenticated Stop consumes that verified cursor through its
original terminal path before an ordinary Fence or fresh live role is issued.
The resident history owner retains the acknowledged terminal cut. Normal
recovery acknowledges one Fence and captures its successor before fallible
suffix admission or listener startup.

The recovered numerical owner, source custodian, pending evaluation cases,
accounting windows and liabilities then survive every Message/Finish handoff.
One fresh independent evaluator serves the entire suffix under its single
finite candidate, I/O, byte and lifetime allowance. It can discover and label a
retained historical case as well as a later review. A new invocation does not
replenish native numerical work or erase accounting, and a later release does
not issue another evaluator or renew its allowance.

Every actor request must still be genuinely new, and each selected Message
generation must be unused. The confirmed predecessor, remaining stream bounds
and Finish recipe retain their original checks. Every new effect needs fresh
source evidence, helper review and independent human approval. A post-Fence
refusal retains acknowledged recovery history without emitting a new source
reference or starting additional generation. The final retained archive includes
acknowledged late labels after evaluator authority has retired.

The requirements, accounting/evaluator profiles and frozen input anchor are
reserved against the store, retained output, future service endpoints and
selected recipe/model/source/witness inputs before numerical asset loading.
This mode resumes a confirmed Message boundary. The next section selects an
interrupted original generation instead. The authored positive and causal-negative
regression cases still require remote RCH compilation and runtime qualification
before acceptance can close.

### Resume interrupted accounted generation

Select `--continue-generation` when the first remaining Message already has an
original pending or completed-but-unsubmitted native text intent:

```sh
supervise_publication serve-native-generated CONFIG REMAINING_SEQUENCE REVIEWER_PROFILE \
  --continue-generation \
  --andon CURRENT_ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE \
  --history-anchor /absolute/independent/original-native.anchor \
  --retain-history-anchor /absolute/independent/new-native.anchor \
  --evaluation-recovery RECOVERY_REQUIREMENTS
```

This unary option takes no value and cannot be combined with `--after-message`.
Both accounting/evaluator profiles, an independently retained input anchor, a
new output anchor and the same closed recovery requirements are mandatory. The
optional simple witness profile, independent operator pair and explicit
version-two per-release exact prechecks remain available. Joint `/2` accounting
requires its exact original policy throughout this recovery, just as it does
for a confirmed-Message suffix. Governor, topology and offline credibility
activation retain their separate composition requirements.

The remaining sequence contains 1 through 16 entries and starts with a Message.
Its first recipe must identify the exact original text intent: generation,
prompt, stop token, token limit, work limit and original numerical command must
match the retained history. A pending command resumes from its actual KV/RNG
state and sampled prefix. A command already completed on its monitored stop
token can proceed to fresh submission and review without sampling again. A
failed, held, cancelled or limit-exhausted generation does not become a completed
Message by selecting this mode. An already recorded actor request cannot be
reused as the first entry.

After the original recovery Fence and before fresh source reads, numerical
work, review listeners or reference output, admission scans the entire remaining
sequence. Every actor request must be absent from retained history. Only the
first Message may name an existing generation; every later Message generation
must be unused. The original remaining stream allowance must cover all selected
Messages. A collision in a later entry therefore refuses the whole selection
before the first resumed release can publish. The acknowledged Fence remains
in the newly retained history on such a refusal.

The first Message uses the original single-request intent selector and resume
operation. It does not reconstruct a command from the current position or reset
cumulative work. Each later Message still requires the immediately preceding
confirmed receipt. Finish retains the last Message's exact recipe and performs
no sampling. A single resumed Message is a valid selection; a singleton Finish
requires `--after-message` instead. Ending after a Message does not implicitly
close the stream.

One recovered owner and one fresh independent evaluator serve the whole
selection. Original numerical spending, complete current accounting windows,
pending cases and liabilities survive; later handoffs do not replenish the
evaluator's finite allowance. Every effect requires fresh source evidence,
helper review and a separate human approval. A rejection of the resumed Message
retains its spent work and prevents all later entries from starting. Original
Stop and deadline remain active through replay, Ready, resumed generation and
all subsequent releases. An authenticated Stop at Ready retains the sampled
prefix and accounted terminal history before any ordinary recovery Fence or
fresh role is issued.

Five authored regression functions cover pending and completed-unsubmitted
first intents through Message/Finish, singleton continuation and human refusal,
changed intent and later historical-generation collisions, Stop at Ready, and
closed unary grammar. They still require the repository's remote compilation
and runtime qualification; source checks alone do not establish these runtime
outcomes.

## Independent policy governance during native creation

A new resident sequence can explicitly select `--governor-profile PROFILE`
together with its operator profile and Tower generation. The governor profile
supplies the exact campaign requirement installed atomically in the original
first journal image. No second campaign profile or later guard installation is
used. The governor and human reviewer are separate opaque roles returned only
after acknowledged creation. Ordinary native operator selection continues to
refuse policy promotion.

Both sides must explicitly budget numerical reconstruction: the governor selects
`numerical_replay`, and the operator's `policy_promotion` selects
`numerical_replay_events`. Their original replay allowances, campaign limits,
connection/exchange counts and deadlines remain independent and cumulative
through every Message/Finish handoff. The complete profile and protocol are in
[live policy governance](LIVE_POLICY_GOVERNOR.md#govern-a-new-resident-native-sequence).

Governor credentials must be disjoint from every selected actor, operator and
human reviewer. Admission reserves its canonical socket against the authority
store, evidence source, every future actor/reviewer/Stop/precheck endpoint,
selected configuration/profile files, publication witness inputs and retained
history output. All selected role profiles are checked before model loading.
For each recipe, its model, monitor, sampler, prompt, tokenizer, weight index and
physical shard paths are checked before that recipe's referenced asset reads.
Earlier compatible recipes may already have loaded assets when a later recipe
is refused; no clock, listener or authority store has been acquired then.

After an earlier publication provides the original retained policy corpus,
withhold the next actor Submit at the post-generation source-reference boundary.
The independent governor can Request and Approve the complete original replay
report, then the operator can Next and Promote. Campaign processing does not
reread source evidence or rerun helpers. Further numerical advancement stales a
campaign; the service never silently rebases it.

If promotion changes the policy epoch while the selected actor request is still
entirely absent, the service emits one fresh reference for that same completed
Message or confirmed-predecessor Finish. Only the expected policy epoch changes.
The full source payload, generation, target, release kind, deadline, scope and
units must remain identical. This projection consumes no inference, source
capture, permit or journal event. The independent actor must submit the new
reference itself; an earlier queued reference cannot authorize the new epoch.
Any recorded request disposition keeps its original intent and prevents refresh.

An old exact-precheck listener is retired at this boundary. It is not rebound,
and its query allowance is not replenished. The operator and governor remain
available after the original independent Stop check during generation, intake,
helper/human review and retirement. Promotion preserves completed numerical work,
the confirmed stream prefix and unresolved charges. Every next release still
requires fresh current evidence, original helper review, independent human
approval and its selected final publication checks.

This creation selection accepts the existing checked publication profiles,
including the complete `fa.supervised-joint-publication/1` envelope, and optional
`--retain-history-anchor NEW_OUTPUT`. The original atomic constructor installs
the exact joint policy with the source, decoder, stream, witness, reserve and
campaign guards. The unchanged baseline committee can review the first
Messages; installing the guard does not claim held-out qualification. The guard
constrains later credibility activation through its original joint reducer.

Fresh credibility activation remains refused. The initial control sequence
cannot admit the historical evidence required by that operation; creation
does not manufacture prior reviews or relabel future evidence. A later
independently recovered suffix can select a fresh capsule under the complete
recovery contract. Baseline publication and policy promotion still require
their original independent roles, fresh evidence and final witnesses. The
paired regression cases use real native Message/Message/Finish publication
around an independent policy campaign, and require a human rejection to
preserve only the confirmed prefix.

This creation selection refuses `--after-message`, `--history-anchor`, topology
selection and single-release commands. Retaining an archive does not select or
authorize governed native recovery. Select the complete independently supplied
current policy and campaign contract through the
[governed recovery command](#recover-a-governed-native-sequence).

## Stop, deadlines and failure

One authenticated Stop service remains owned for the entire invocation. Its
operation and socket use the first selected actor request:

```sh
supervise_publication stop-peer REVIEWER_PROFILE FIRST_REQUEST_ID
```

The original listener, candidate allowance and authenticated peer lifetime
survive request handoff. Each publication workflow receives a shared view of that
same Stop custody. Stop remains first during generation, intake, review, reply
grace and helper retirement; a partial actor frame cannot prevent servicing it.

The Config runtime bounds the entire invocation. An entry's actor runtime may
shorten its action deadline, but cannot restart the invocation's logical or wall
deadline. Numerical and stream work remain cumulative in the original owner.

A source, transport, output or review failure cannot cause automatic resubmission
or generation of a later entry. The original Stop/drain and bounded helper
cleanup paths retain unresolved liabilities. An earlier confirmed publication
remains a fact even when a later entry fails. Cleanup consumes the resident
driver once, after the sequence has completed or stopped.

## Scope and verification

The resident command creates a new native stream, explicitly selects a new
remaining suffix with `--after-message`, or resumes the first original accounted
intent with `--continue-generation`. Existing single-release native
creation, continuation, pending-generation recovery and receipt commands keep
their own grammars. The resident command does not implicitly select recovery,
credibility activation, topology or policy-governor services. History custody
requires its explicit input/output options. Requalification requires its explicit
capsule option and a selected recovered suffix. The independent
operator requires its explicit paired selection, and release-specific prechecks
require explicit version-two manifest rows. Native governance requires the
explicit creation or independently guarded recovery governor selection described above.
Live evaluation and Andon accounting require their paired selection and new
retained anchor. Accounted recovery additionally requires explicit
`--after-message` or `--continue-generation`, an independent input anchor and
the evaluation recovery requirements above. Exact prechecks still require their
explicit version-two manifest selections; accounting does not imply governance
or any additional disclosure.

Regression sources exercise actual deterministic fixture inference, independently
authenticated actor/human exchanges, cumulative Message/Finish publication,
later rejection, independent Stop, fresh-source loss and checked witness failure.
Recovery cases cover new suffix publication, singleton Finish without inference,
pending or completed-unsubmitted first Message continuation, original replay
Stop and rejection of historical or incompatible selections.
Retained-history cases cover actual prefix/suffix and Finish publication, frozen
input, rollback and divergent-history refusal, startup Fence retention, Ready
Stop and deadline boundaries, and output collision without effect retry.
Profile cases pair admitted sequences with incompatible identities, duplicate
keys, invalid release ordering, finite limits and canonical path collisions.
Governance cases use a separate authenticated governor process, retained-corpus
Request/Approve and actual operator promotion before further native publication.
They pair fresh source references with human rejection, altered release/deadline
submissions, cumulative campaign/replay limits and the original independent Stop.
These tests address contract enforcement; synthetic model and helper fixtures
do not establish trained detector quality or hostile-process containment.

The required execution gate is:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Source review, grammar parsing and targeted formatting are separate from that
execution gate. No compilation, runtime or full qualification result is claimed
while the required RCH runner is unavailable.
