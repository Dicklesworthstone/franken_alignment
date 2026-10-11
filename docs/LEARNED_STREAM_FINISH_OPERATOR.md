# Explicit finish for learned publication

The learned publication operator can close its receipt-confirmed message through
the original separately reviewed stream-finish effect. Closing retains the full
published prefix and every message boundary. It computes no new model token,
draws no sampler value, and does not append the message again. Cancellation or
timeout cannot substitute for this explicit finish.

This completes the end-marker path in plan §8.7 for the existing one-message
learned command. It uses the original source-only actor wire, learned sidecar
and native congress, separate human key, current-policy publication gate and
durable endpoint receipt. It adds no effect kind, journal format, permission
type, numerical engine or automatic conversation scheduler.

## Command forms

Provision a distinct finish recipe using the original message's numerical
recipe, then run:

```text
supervise_publication create-learned-generated CONFIG FINISH_RECIPE REVIEWER_PROFILE --finish-after-message PREVIOUS_REQUEST
supervise_publication create-learned-generated CONFIG FINISH_RECIPE --resume --finish-after-message PREVIOUS_REQUEST
```

`PREVIOUS_REQUEST` is the nonzero decimal request ID of the exact preceding
learned message. The finish recipe's `request` is a new nonzero ID. Equal IDs,
additional flags, repeated predecessor selections and composition with
`--continue-generation` refuse. Ordinary creation, generation continuation and
message receipt recovery retain their original forms.

The first form starts a new finish proposal in the existing authority store.
It never creates a missing store or adopts an unconfirmed message. Once the
finish request is recorded, use the second form to inspect or reconcile that
same request. A retained refusal remains that request's result; a new ID means
a new proposed effect, not an exact retry.

The independently provisioned reviewer selects the **finish request ID** with
the existing `review-peer REVIEWER_PROFILE FINISH_REQUEST` command. The same
`stop-peer REVIEWER_PROFILE FINISH_REQUEST` endpoint remains available during
live preparation, native review, human review and publication. Receipt-only
recovery opens no reviewer or stop listener. Stdout remains the restricted
original actor response; success requires `Known(Executed)` and completed cleanup.

## Exact recipe and fresh review

Retain the original primary model, tokenizer, prompt, sampler, fit binding and
archive, complete learned monitor, numerical and telemetry budgets, publication
stream profile and source contract. Guarded open pins the complete original
`FileLearnedConfig` before numerical replay, cleanup or recovery fencing.
Independent recovery floors and the original terminal reserve still apply.
A different prompt or model is not a finish recipe.

Select a new outer `request`, separately provisioned sidecar identity and complete
review schedule. Version 1 uses the new request ID as its native round ID.
Version 2 supports the same fixed retained-residual ladder as a message and
requires fresh, unused round IDs for every future round. A previous permitting
review cannot authorize the different finish action. Actual native inputs carry
the entire finish frame, including every earlier message boundary, plus the
original learned evidence and question.

The new finish has its own prospective effect deadline. Its complete native
schedule must fit configured runtime, recipe TTL and remaining lifetime after
preparation. Once recorded, the finish keeps its original deadline on recovery;
changing `ttl_ms` cannot extend it.

Existing source, journal, disclosure, native inference and poll ceilings are
unchanged. In particular, the policy source retains at most 1024 events across
the entire authority lifetime. Both message and finish reviews consume this
same history. Genuine tokenizer compression can reduce full-input token count;
omitting bytes, replacing history or increasing a poll budget cannot create
capacity. Cancelled numerical work remains spent in its original sequence.

Finish needs no unused message slot or actor context position. Its full frame
still receives an independent byte charge and both publication keys. Rejecting
or cancelling finish leaves the published prefix open and unchanged; it cannot
retract text or refund an earlier executed message.

## Predecessor and recovery

Before live preparation, the operator checks the exact message request, its
accepted `Executed` receipt and the cumulative stream state. The receipt must
advance the original target version by exactly one. All frame boundaries and
the added message must equal the latest confirmed stream. Published and
confirmed views must agree, with no pending effect, closed stream, Stop or
suspension. Original generation must have completed at its admitted monitored
control stop, with no pending numerical step or reset. This read-only check can
inspect a paused recovered owner.

Independent stop and fresh policy acquisition surround explicit original
resume. The same predecessor checks run again before source-only Finish
submission. No advance or reset method runs; numerical, sampler, compression
and monitoring work remain unchanged.

`FileOversight::learned_text_stream_finish_request(after_request, request)`
provides exact receipt lookup. It reads the original pre-policy request book,
including `NotAdmitted` requests with no `FrozenAction`. It compares the retained
finish target/version and every boundary with the exact predecessor's accepted
receipt, then returns only the original proposal identity and deadline. It
does not return a message, approval, reconciliation or publication key.

The command uses the original actor retry and existing reconciliation path.
It does not regenerate output, rebuild the frame against today's prefix, start
another helper round or obtain another human approval. An unconfirmed message
must first be reconciled through that message's original receipt path. Missing
receipts and lookup absence never imply execution or nonexecution.

Recovery still loads the exact primary numerical recipe and performs original
guarded replay and fencing. It reads neither native helper assets nor live
policy evidence. Terminal receipts need no current clock; unresolved dispatched
requests retain the original query/reconciliation behavior. Lost stdout causes
no automatic resend.

## Source and verification

The implementation is `learned/finish.rs`, with option routing and publication
in `learned.rs` and receipt recovery in `learned/recovery.rs`. The accessor lives
in the original learned text intake module. Existing event formats are unchanged.

All eight new example tests pass on qualified `nightly-2026-09-08`. They exercise
two actual native reviews and separate human roles, full stream capacity,
unchanged numerical and sampler work, complete finish inputs and full-frame
charging, both human decisions, unconfirmed-message and wrong-predecessor
refusals, retained pre-policy attempt-capacity refusal, original-deadline receipt
recovery without live inputs, late policy change, retained native cancellation
work, the exact deadline boundary, no-write recipe mismatch and closed command
options. The existing genuine ByteBpe fixture fits both reviews under the same
1024-event source ceiling; no limit is increased for these tests.

Synthetic models and direct original owners do not establish detector quality,
statistical independence or authenticated listener transport. The complete
learned-command selection passes 29 of 31 tests. Its two existing Unix-listener
tests still fail at actual listener creation with environment EPERM; neither is
skipped or replaced by a fallback.
