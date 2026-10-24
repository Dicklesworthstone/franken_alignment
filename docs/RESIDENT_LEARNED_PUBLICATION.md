# Resident learned publication sequence

`supervise_publication serve-learned-generated` serves a finite, independently
selected sequence of learned Messages and an optional final Finish through one
original supervised owner. It keeps the authority journal locked, retains the
same admitted numerical generation, forecast observer and topology source, and
requires fresh actor ingress, native or process review and independent human
approval for each release.

This is scoped implementation toward FA-027 and plan sections 8.5, 8.7, 9.3,
9.11, 11.2 and 16.4. Compilation and execution qualification remain pending.
The full roadmap packet, including long-horizon and isolated-branch acceptance,
remains open.

## Command and selected entries

From a qualified Linux build:

```text
supervise_publication serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE
supervise_publication serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE --continue-generation
supervise_publication serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST
```

The optional invocation-wide selections are `--topology-profile PATH`,
`--history-anchor INPUT` and `--retain-history-anchor NEW_OUTPUT`. Explicit
live recovery may additionally select `--credibility-activation CAPSULE`
under the existing qualification contract. Recovery selection applies once,
before entry one. Creation requires a fresh authority store; existing state
must pass the original exact recovery requirements and predecessor checks.

```json
{
  "schema": "fa.learned-publication-sequence/1",
  "entries": [
    {
      "recipe": "/operator/message-1.json",
      "actor_profile": "/operator/actor-1.json",
      "release": "message"
    },
    {
      "recipe": "/operator/message-2.json",
      "actor_profile": "/operator/actor-2.json",
      "release": "message"
    },
    {
      "recipe": "/operator/finish-3.json",
      "actor_profile": "/operator/actor-3.json",
      "release": "finish"
    }
  ]
}
```

The manifest contains 2 through 16 entries. Each selects its own existing
[learned publication recipe](LEARNED_OPERATOR_PUBLICATION.md) and mandatory
[authenticated actor profile](LIVE_LEARNED_ACTOR.md). Recipes must admit finite
continuation, as supported by versions 4, 5 and 6. The first entry is a Message.
Finish is optional and can appear only once, last. Ending after a Message
retains an open confirmed stream; it does not imply Finish.

Every recipe must encode the exact same complete original generation and
stream. The comparison uses the original encoded configuration bytes. Prompt,
model, tokenizer, monitor, predictor and finite allowances cannot change at a
message boundary. Later equal immutable generation configurations share the
first admitted configuration; each helper roster retains its own selected
review configuration.

Request IDs, sidecar object IDs and native/process round IDs must be distinct
and nonzero. Each actor profile must select its recipe's exact request and
complete supervisor audience. Actor sockets must be distinct and cannot overlap
any selected human-review or Stop endpoint. Each entry selects exactly one
original native or process review schedule. The independent reviewer profile
and kernel credential checks remain mandatory.

Only the first entry can select recovery floors, the history anchor, topology
custody or checkpoint outputs. Secondary entries with those selections refuse
before startup. Unknown fields, missing actor profiles, changed generation,
duplicate identities and misplaced Finish also refuse during admission.

The manifest is bounded to 64 KiB. All entries and their original independently
selected assets are admitted before the first clock observation, socket bind
or authority-store startup. Each recipe retains its existing 64 MiB aggregate
asset limit, so the sequence admits at most 1 GiB of recipe assets plus bounded
profile documents. Synchronous file intake is not a preemptible service phase.

## One original owner across message boundaries

Entries may select the
[joint learned qualification envelope](LEARNED_JOINT_CREDIBILITY.md) around their
original recipes. The exact joint policy and both work ceilings must match across
every entry, alongside the existing numerical, source and predictor contracts.
The outer command now forwards `--credibility-activation` for explicit resident
recovery; fresh resident creation still refuses it before startup. The selected
native guard checks the actual capped helper weights against the original
baseline before any successful requalification can admit further work.

The outer [computed-identity envelope](LEARNED_IDENTITY_OPERATOR.md) can require
a fresh original-model measurement for every entry. All entries retain the same
complete passport and policy, distinct check IDs, strictly increasing measurement
sequences and the original check allowance. These requirements are admitted before
runtime startup. The invocation retains its unique observer and completed or
cancelled run history; expired or interrupted measurements cannot admit publication.

The invocation creates or recovers one original host and releases one actor
gateway, driver, human reviewer and, when required, forecast observer. Its
topology custodian stays attached through numerical preparation, native or
process congress and final publication. Intermediate messages do not reopen
the authority journal or introduce a recovery Fence or fresh qualification.

For every entry, the ordinary learned request cycle still:

1. Checks the current source, deadline and authenticated Stop service.
2. Completes the original required forecast before sampling, or the separate
   forecast required for Finish.
3. Prepares the admitted original numerical continuation and accepts the
   selected authenticated actor release with its own deadline.
4. Runs the selected native or process review schedule and obtains a fresh
   independent human decision for that exact action.
5. Publishes only through the original driver and reports its original result.

The next actor listener is bound only after the predecessor has both an
original `Known(Executed)` response and a native `Confirmed` request with an
`Executed` resolution. The driver must retire its original direct children;
separately launched process reviewers must also be confirmed reaped. Their
histories remain owned through cleanup. Ambiguous or still-running work cannot
supply a successor slot.

Required forecasts retain their original consumed record and observer. Raw
forecasts and owned learned forecasts use their own native observation
accessors. Numerical draws, forecast expenditure, source checks, authority
charges and the admitted stream prefix accumulate across the sequence.
Individual request/helper quotas cannot refill those original lifetime
allowances.

This removes reopening and guarded authority-history recovery between entries.
The original cooperative numerical preparation still performs whatever replay
its existing stepper requires. The command does not admit arbitrary new prompts,
an unbounded conversation, branch credentials or replacement model assets.

## Deadlines, review and Stop

One global logical deadline and one monotonic wall deadline span the live
sequence. The runtime is bounded by CONFIG and every selected actor runtime.
Each request is additionally limited by its recipe TTL and actor runtime;
the accepted actor deadline can only tighten its action deadline. The first
entry's deadline includes startup and any initial replay. A later short TTL
retains a wall-time bound even if the logical clock stops advancing.

For each request, use the original `review-peer REVIEWER_PROFILE REQUEST`
client when its fresh review socket is announced. It displays the original
offer and requires the exact explicit decision and session nonce. Selecting a
sequence does not preapprove any human decision or submit any actor request.

The invocation retains one authenticated Stop endpoint and operation, named by
the **first recipe's request ID**, throughout every entry. If that ID is 1:

```text
supervise_publication stop-peer /operator/reviewer.json 1
```

The same operation can be selected in a [live fleet Stop roster](LIVE_FLEET_STOP.md).
Later request IDs never replace it. Operational failure during a later entry
also uses this original Stop operation and the existing reserved drain path.

Cancellation or another request-local nonexecuted result ends the sequence
without starting the next request. Unknown dispatches keep their original
charged liability. Earlier confirmed publications remain facts. Failed cleanup,
source retention, output or a lost acknowledgement cannot trigger an effect
retry or restore capacity.

## Retained evidence and results

Topology input and output use the existing
[mediated creation and continuation profiles](TOPOLOGY_CHECKPOINT_OPERATOR.md).
A mediated recovery requires the independently retained history anchor and its
matching source checkpoint. The invocation retains the selected final topology
checkpoint and history anchor exactly once, while the original driver still
owns its final acknowledged state. They cover all completed entries and
preserve cumulative source expenditure. Output paths must be distinct, fresh
and outside the authority store under their existing custody rules.

A refusal may still leave a healthy source whose final checkpoint can be
retained. Storage or export failures remain errors; an incomplete output is
not acknowledged evidence. Preserve the original independently retained inputs.

Each original actor receives its normal response service. At invocation end,
stdout emits the original encoded responses in entry order, one per line.
Success requires every selected entry to be confirmed Executed, no operational
failure and completed cleanup. A nonzero exit or lost stdout does not establish
that no effect occurred. Inspect the original request through the existing
[receipt service](LEARNED_RECEIPT_SERVICE.md) or recovery command; do not submit
a replacement effect merely because an acknowledgement is missing.

## Verification scope

Regression source drives original imported numerical generation, authenticated
actor and human peers, native congress and separately launched process fixtures.
It covers all three forecast modes, A/A/Finish continuity, native/process/native
review, cancellation, lifetime forecast exhaustion, an actual post-Dispatch
staging failure, the original Stop endpoint during request two, complete profile
admission, full on-disk command/topology/export composition, and a short request
deadline with a stalled logical clock. Closed manifest tests cover bounds,
missing independent actors, extra fields and early Finish.

Independent source review found and corrected the owned-forecast observation,
per-entry wall-deadline and fixed Stop-operation integration seams before
publication. All 17 changed Rust files in this wave passed nonmutating Rust 2024
syntax parsing from their exact candidate contents. These are authored
regressions and source checks, not execution receipts. The normal workspace
sandbox cannot start on the full device and the required remote-only RCH gate
has no available worker; no compilation, runtime, Clippy or production
qualification is claimed. Synthetic enforcement fixtures do not establish
model quality or empirical forecast calibration.

Implementation:
[`learned/sequence.rs`](../crates/fa-reference/examples/supervise_publication/workflow/actor_service/learned/sequence.rs),
[`sequence/profile.rs`](../crates/fa-reference/examples/supervise_publication/workflow/actor_service/learned/sequence/profile.rs),
[`tests/sequence.rs`](../crates/fa-reference/examples/supervise_publication/workflow/actor_service/learned/tests/sequence.rs).
