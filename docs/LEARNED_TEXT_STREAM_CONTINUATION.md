# Finite learned text stream continuation

The learned publication operator can publish more than one complete message from
the **same originally admitted finite generation**. Each message passes through
the original learned K/V monitor, source-only actor request gateway, native
sidecar review, human approval, endpoint publication, and receipt reconciliation.
An explicit finish remains a separately reviewed effect.

The feature is selected before the first canonical image by
`fa.learned-publication/4`. Versions `/1`, `/2`, and `/3` keep their existing
one-message interpretation. Changing a retained recipe to `/4` cannot upgrade an
existing store: the original independently supplied generation configuration must
still match the journal's exact bytes.

## Operator use

Admit the whole finite horizon, stream capacity, actor assets, sampling contract,
and numerical/telemetry ceilings in the first recipe. `/4` has the progressive
review fields used by `/2` and the checkpoint-source intake enabled by `/3`:
`sidecar.priority` and `native_review.round_ids` are explicit, including an empty
priority and one round ID for a one-round review.

The admitted policy-source observation allowance also spans all messages and
the final review. Native token-level review continues to acquire original policy
observations. Reopening or continuing cannot clear that retained history: a
tokenizer or review schedule that exhausts it causes a capacity refusal before
another request can be admitted, even when inference and stream space remain.

For example, a generation which samples `A, stop, B, stop` needs an original
`text.max_new_tokens` of at least four and a stream with at least two message
slots. The stop tokens count against that single original generation limit. The
model must actually produce both messages; no caller-supplied message is accepted.

After request `1` has a confirmed Executed message receipt:

```sh
supervise_publication create-learned-generated host.json first.json reviewers.json
supervise_publication create-learned-generated host.json second.json reviewers.json --after-message 1
supervise_publication create-learned-generated host.json finish.json reviewers.json --finish-after-message 2
```

In this example, `first.json`, `second.json`, and `finish.json` name requests `1`,
`2`, and `3`. Their complete actor generation and publication-stream contracts
must remain identical. Give each new review its own original sidecar object and
round IDs, as required by the existing retained-review owners. The next recipe's
request number is the publication identity bound by the continuation intent.

A later message can need more sidecar detail than an earlier one. Provision its
explicit progressive disclosure schedule and review allowance before that review
starts. An uncertain compressed probe must retain its uncertainty until the
original refinement path obtains the required residual; a prior message's native
approval cannot replace that work.

`--after-message` requires a distinct nonzero predecessor and the reviewer
profile. It cannot be combined with `--resume`, `--continue-generation`, or
`--finish-after-message`. There is no additional prompt, sampler seed, token
allowance, or budget option at this boundary.

## What can continue

The original durable owner prepares a continuation only when all of these facts
hold together:

- The exact independently supplied configuration explicitly enables finite
  message continuation.
- The predecessor is an original admitted **Message** request with a Confirmed
  Executed receipt at the current stream target version. Its full cumulative
  message boundaries equal the receipt-confirmed audience history.
- Published and confirmed stream views agree; no message or finish send is
  unresolved. Visible output alone is insufficient.
- The original generator finished at an accepted declared control stop, with
  valid complete UTF-8 output. It is not held, failed, suspended, or stopped.
- The original generation has remaining token/context/numerical allowance, and
  the stream has another message slot plus capacity for the configured complete
  output bound. The target version can represent the next append and finish.
- The successor request ID is fresh. A retry of an already retained identical
  continuation names its original request and predecessor exactly.

Preparation freezes the predecessor request, successor request, original actor
revision, and original numerical position. The same original journal records a
`ContinueMessage` input event before further inference. Commit rechecks the
predecessor and applies the existing source, clock, authority, and journal-capacity
admission rules.

Only `Finished(StopToken)` can become `Generating`. A `TokenLimit` completion does
not acquire another horizon, including when a lower-level text policy accepts it
as a complete message. Held or failed numerical work is retained and cannot be
turned into a shorter successful message by continuing.

## What the boundary changes

The original generator changes only its stop status. It retains its original
prompt, model, tokenizer, compression codec, probes, sampling policy and seed,
accepted-token history, K/V cache, random-word stream, and all accumulated work.
Its original `max_new_tokens` and numerical/telemetry ceilings remain unchanged.
No replacement numerical engine or independently reconstructed output is used.

The existing required pre-output forecast modes currently define a forecast at
the original prompt boundary. They do not define forecasts at later message
boundaries. Combining either required forecast mode with finite message
continuation is therefore refused before startup, in either configuration order
and through independent forecast installation; the `/4` operator does not select
those modes.

The original learned text binding records the current offset into that same
generated-token history. Text decoding for the next message starts at this offset;
the earlier message and its actual control stop remain in the original evidence
prefix. Only the new message's actual final control stop is suppressed from its
visible text. The stream's audience basis advances to the original confirmed
predecessor. Message lengths and boundaries continue to come from the existing
stream frame builder.

The continuation event creates no publication key, effect reservation, receipt,
new funding, or new actor epoch. The source-only gateway binds the next Message
to the successor request retained in that event. The original generic request
admission also enforces this binding. An exact retry of an earlier submitted
request still returns that request's original status and frame.

The next message must independently earn its native congress authorization and
human approval under current policy. Earlier helper conclusions and human keys
cannot authorize it. Cumulative output frames retain their full original byte
charge. A finish after the last confirmed message adds no inference or sampler
draw, but still requires its own request, fresh review, human key, and receipt.

## Recovery and limits

Reopening replays the same original numerical inputs and continuation events.
It restores the message offset and audience basis at their exact recorded cuts,
then applies the existing recovery fence. It does not reset source history,
observation caps, evidence floors, random draws, numerical work, or charged effects.

If a crash occurs before the successor request is submitted, rerun the same
successor recipe with the same `--after-message` predecessor. An already retained
identical continuation is a read-only retry: it neither resets the offset nor
reactivates a terminal generation. The existing `--continue-generation` path can
also resume that retained numerical work when its request ID matches the saved
continuation. Fresh source and explicit numerical resume still precede further
work; an original pending token intent must receive its original outcome.

Once the successor request exists, use its ordinary `--resume` receipt path.
For a finish, retain the finish predecessor as well:

```sh
supervise_publication create-learned-generated host.json second.json --resume
supervise_publication create-learned-generated host.json finish.json --resume --finish-after-message 2
```

Unknown execution stays an original reconciliation obligation. No continuation
interprets missing acknowledgment as nonexecution or sends the earlier message
again. A terminal refusal or cancellation stays attached to its original request;
the continuation does not silently substitute another request identity.

Stream checkpoint/reset remains refused by the existing original host. This
feature does not reopen a stream checkpoint, append a new prompt, replenish a
budget, or support indefinite conversation. It gives the admitted model the
remaining portion of one finite generation, with a separate review and effect
boundary for each complete message.

## Compatibility and executable evidence

On qualified `nightly-2026-09-08`, all 24 public durable learned-stream tests
pass, including nine new continuation controls, and all three loaded-operator
continuation tests pass. All 22 selected forecast/reset and mediated-recovery
compatibility tests also pass. The broader 41-test learned-operator selection
passes 39 tests; its two existing Unix-listener tests fail at listener creation
with environment EPERM. Neither is skipped or replaced by another transport.

The opt-in configuration wrapper is `FALMULT\x01` around the previous exact
configuration bytes. Existing constructors leave the option false and preserve
their prior bytes. Existing learned journal tags `0` through `5` are unchanged;
the new continuation uses tag `6` with the four exact bound integers. Recovery
still requires an independently reconstructed matching configuration.

The public `learned_text_stream_durable` integration target includes continuation
tests using real attention weights and actual monitored sampling. They cover two
messages and finish, comparison with an uninterrupted original numerical run,
receipt-versus-visibility distinctions, stale predecessors, exact configuration
identity, the last generation unit, a real monitor hold in the second message,
crash cuts, pending-token cooperative replay, source-only and generic request
binding, and terminal token-limit completion. Their congress ballots are explicit
synthetic controls.

The executable's `learned::tests::continuation` module separately loads `/4`
assets, runs the original native reviewers for both messages and finish, requires
fresh human keys, buys real progressive residuals when later-message probes are
uncertain, checks cumulative receipt boundaries and charges, and recovers
the terminal finish without live helper or policy inputs. These numerical
fixtures establish execution and refusal behavior; they do not measure the
quality of an alignment detector or claim complete deployment qualification.
