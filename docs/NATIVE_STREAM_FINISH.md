# Explicit native stream completion

## Library contract

`FileOversight::prepare_decoder_text_finish(after_request, request, deadline)`
prepares the existing `FileStreamProposal` for an explicit finish after the latest
receipt-confirmed native message. It shares continuation's original confirmed-cut
validation: exact source linkage, Executed receipt, complete current message
boundaries, no unresolved publication or outstanding generation, unchanged
numerical predecessor, Ready monitor, and no Stop or suspension.

Finishing does not generate another token or append another message. It therefore
needs no unused message slot, content-byte allowance or numerical context position.
It is still a separately reviewed effect: the full cumulative frame is charged,
and the original source, helper, human, witness and publication requirements apply.
Timeout, cancellation and a partial/held generation are not substitutes for finish.
This implements the explicit closing portion of plan section 8.7 using the
existing L5 stream and actor vocabulary, not a new effect or journal format.

Preparation is read-only and can inspect a paused recovered owner. It neither
refreshes evidence nor clears the pause. Recheck it after original qualification,
source/clock acquisition and explicit resume, then submit the resulting identity
through `FileGeneratedTextActorPort::encode_finish` and the original actor wire.
No actor-provided prefix or output bytes are accepted by that port.

`decoder_text_finish_request(after_request, request)` instead reconstructs an
already recorded finish, including refused, cancelled and expired requests. It
uses the original request specification, predecessor receipt and exact full
message boundaries, never today's prefix, epoch or deadline. The previous native
request is not interchangeable with another one that happened to produce equal
text. This lookup creates no key, performs no reconciliation and retries no effect.
A storage-faulted owner refuses. Recovery must separately pin its full original
model, tokenizer and selected publication contracts as before.

## Source and validation status — 2026-09-26

The shared confirmed-cut helper preserves continuation's validation predicates;
all existing continuation tests are unchanged. Six new tests exercise original
synthetic inference and real canonical-file review/publication: full stream
capacity, unreconciled publication, a later zero-token intent, retained refusals
and cancellations, equal-text predecessor conflicts, paused recovery, subsequent
inference, and ambiguous storage failure. Fixture weights and ballots do not
qualify detectors, evaluator identity or hostile-process isolation.

UNEXECUTED: the fresh required RCH gate stopped before compilation because `rch`
is unavailable (exit 127). Rust compilation, tests, rustfmt and Clippy have not
run. Source/whitespace/hash inspection and historical gate results do not validate
new runtime behavior. No Beads item is closed, and no production qualification,
anti-rollback guarantee or remote-publication transaction is claimed.

## Native service integration

Both native `serve-open` and `serve-open-checked` accept the explicit final option
`--finish-after-message PREVIOUS_REQUEST`. It is mutually exclusive with
`--after-message`: one starts a new generated message, the other proposes a
non-message closing effect. Creation, actor and reviewer modes reject this option.

```sh
supervise_publication serve-open-checked \
  CONFIG FINISH_ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE \
  --native-text LAST_MESSAGE_RECIPE \
  --finish-after-message PREVIOUS_REQUEST > finish-submit.json

supervise_publication actor-submit FINISH_ACTOR_PROFILE finish-submit.json
supervise_publication review-peer REVIEWER_PROFILE FINISH_REQUEST_ID
```

Use a new finish request ID in the actor profile, but retain the previous native
message's recipe: its original generation ID, prompt, budgets, model and tokenizer.
No new generation recipe, empty prompt, artificial stop token or output override
is needed. Both existing single-file and sharded recipes pass through their
unchanged loaders. Preserve the original witness and joint-publication selection.

Before a first finish proposal, the service checks that exact original recipe
and the latest confirmed message cut. It services independent stop, obtains any
explicit `--credibility-activation`, refreshes source/time, invokes the original
explicit resume, and rechecks the cut before emitting only the fixed-size finish
reference. There is no new generation, prompt evaluation or sampled draw. As with
all existing durable transactions, replay may physically recompute old history;
this is not a constant-time close or an exactly-once CPU guarantee.

The authenticated actor must still submit the reference. The original process
helpers and separate human review receive the full cumulative finish frame, and
current witnesses are checked through final publication. Rejection or a late
witness conflict leaves the previously disclosed prefix intact and unclosed; it
does not retract text, infer completion, or mint a replacement permission.

Retry/recovery uses the same finish option, previous-message recipe, predecessor
and finish request ID. Once a finish request was recorded, the service emits its
EXACT original document and uses only original reconciliation and observation.
It does not renew its deadline, resume inference, read live producer/evidence or
activation files, start helpers, open a review/stop listener or repeat the effect.
Even a failed receipt output does not issue a new Stop against later work. An
unrecorded finish is still a first proposal and must pass the full current checks.

### Integration validation status — 2026-09-26

Six additional service regression functions plus a synthetic helper entrypoint
exercise first closure with both human decisions under baseline/witness/joint
bootstraps, exact expired receipts without live evidence, late phantom versus
disjoint changes, missing sources, changed original recipes, broken receipt
output, and strict option composition. The library gains a seventh regression
that consumes all sixteen original context positions while leaving message slots
available, then separately reviews/publishes finish without another position or
draw. Existing service tests and inference/source/socket implementations are
unchanged. Neither joint bootstrap nor synthetic helper behavior claims empirical
qualification.

The targeted service test command and full gate were attempted again through
`RCH_REQUIRE_REMOTE=1 rch exec`; both stopped before compilation because `rch` is
absent (127). All seven library and six service regressions remain UNEXECUTED,
as do compilation, rustfmt and Clippy. Source preparation is not a full checkout.
No Beads closure, dependency, actor wire format or journal encoding change.
