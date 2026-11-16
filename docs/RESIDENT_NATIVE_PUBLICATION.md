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
17.1, serving FA-017, FA-025, FA-026 and FA-107. Compilation and runtime
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

Use suffix recovery at the original confirmed-Message boundary. A pending or
partly generated release must first use the existing single-release recovery
path; this mode cannot skip its outstanding numerical or publication work.

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
native opener. It does not add an independent latest-head or anti-rollback
anchor. Historical replay remains synchronous within each original event; the
service's checkpoints do not establish a hard real-time Stop bound.

## Explicit recovery requalification

A resident suffix can select `--credibility-activation CAPSULE` together with
`--after-message PREVIOUS_REQUEST`. This composes with the existing publication
and independent operator selections. Creation rejects the option before
configuration reads or clocks; an actor submission cannot supply the capsule.

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
selected operator endpoint before recipe/model reads. The original bounded file
reader still runs at the controlled qualification boundary. These path checks
do not establish a hostile-filesystem isolation guarantee.

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

The resident command creates a new native stream or explicitly selects a new
remaining suffix with `--after-message`. Existing single-release native
creation, continuation, pending-generation recovery and receipt commands keep
their own grammars. The resident command does not implicitly select recovery,
credibility activation, topology or policy-governor services. Requalification
requires its explicit capsule option and a selected recovered suffix. The independent
operator requires its explicit paired selection, and release-specific prechecks
require explicit version-two manifest rows.

Regression sources exercise actual deterministic fixture inference, independently
authenticated actor/human exchanges, cumulative Message/Finish publication,
later rejection, independent Stop, fresh-source loss and checked witness failure.
Recovery cases cover new suffix publication, singleton Finish without inference,
original replay Stop and rejection of historical or incompatible selections.
Profile cases pair admitted sequences with incompatible identities, duplicate
keys, invalid release ordering, finite limits and canonical path collisions.
These tests address contract enforcement; synthetic model and helper fixtures
do not establish trained detector quality or hostile-process containment.

The required execution gate is:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Source review, grammar parsing and targeted formatting are separate from that
execution gate. No compilation, runtime or full qualification result is claimed
while the required RCH runner is unavailable.
