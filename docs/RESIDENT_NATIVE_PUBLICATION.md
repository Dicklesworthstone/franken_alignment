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
```

The optional publication profile retains the native service's existing simple
checked-publication contract. It applies to the same original owner and every
selected release. The command owns its complete grammar before reading files;
unsupported options are not consumed by another service.

A version-one sequence contains 2 through 16 entries. The first entry is a
Message. An optional Finish appears only once, last. Message followed directly
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
while using its own actor request ID.

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

This initial resident command creates a new native stream. Existing single-release
native creation, continuation, recovery and receipt commands keep their own
grammars. The resident command does not implicitly select recovery, operator,
precheck, topology or policy-governor services.

Regression sources exercise actual deterministic fixture inference, independently
authenticated actor/human exchanges, cumulative Message/Finish publication,
later rejection, independent Stop, fresh-source loss and checked witness failure.
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
