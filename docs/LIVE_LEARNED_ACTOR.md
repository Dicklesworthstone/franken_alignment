# Live actor intake for the learned publication operator

## Command and scope

The Linux learned operator accepts an optional independently configured actor
endpoint in addition to the separate human reviewer:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE --actor-profile ACTOR_PROFILE
```

The same selection can accompany one existing live continuation choice:

```text
supervise_publication create-learned-generated CONFIG NEXT_RECIPE REVIEWER_PROFILE --actor-profile ACTOR_PROFILE --after-message PREVIOUS_REQUEST --history-anchor INPUT
supervise_publication create-learned-generated CONFIG FINISH_RECIPE REVIEWER_PROFILE --actor-profile ACTOR_PROFILE --finish-after-message PREVIOUS_REQUEST --history-anchor INPUT
```

`--continue-generation`, independent credibility requalification and successor
history retention retain their existing contracts. Only one continuation choice
is permitted. Receipt-only, offline stop and audit modes do not accept an
admitting actor profile. Their existing non-admitting commands remain separate.
Omitting the actor option keeps the original local source-only submission path.
This is one selected request per invocation, not an arbitrary multi-request
server or an alternate generation engine.

`ACTOR_PROFILE` uses the existing closed `fa.actor-service/1` schema. Its request
must equal `LEARNED_RECIPE.request`; scope and clock must match CONFIG. The actor,
human-review and authenticated-stop sockets must be distinct. Existing kernel
UID/GID and optional PID checks, supervisor-owned directory rules, socket
permissions, candidate/connection/exchange limits and bounded framing apply.
A collision does not remove an existing path. The actor socket is claimed before
the original store is created or recovered.

## Source-only admission on the original gateway

The operator still selects, loads and drives the original numerical model,
tokenizer, learned monitor and optional required predictor. Original prompt,
KV state, sampler, work limits, forecast obligations, finite continuation and
separate finish preparation are unchanged. The actor supplies none of those
inputs and cannot replace the completed message with caller-authored output.

Generation/finish preparation completes before actor frames can admit a request.
Thus numerical work may be spent even when a peer never connects or is rejected;
it remains original bounded work, not an effect or a refund entitlement. A
completed generation alone cannot substitute for an actor request in this mode.
The existing authenticated stop endpoint is still checked during generation.

The client sends the original source-only intent encoded by
`FileLearnedTextStreamActorPort::encode_release`, using the existing client:

```text
supervise_publication actor-submit ACTOR_PROFILE SOURCE_ONLY_SUBMIT_JSON
```

The operator constructs one `LearnedStreamAdmission` for the independently
selected request, Message/Finish kind and maximum deadline. The native scoped
peer method compares every canonical envelope field before source preflight,
new policy I/O or snapshot use. A different request is withheld; changed kind,
target sentinels, epoch, units or intent bytes are refused. No output-bearing
proposal, prompt edit or actor defense text is admitted.

A client may select a shorter absolute deadline, never a later one. Once the
original request is recorded, the operator obtains its local observation ticket
by an exact retry using the **accepted original deadline**, not the offered
maximum. That retry must leave the journal revision unchanged. The subsequent
review/publication lifetime is clamped to the accepted deadline; a deadline too
short for the complete native review schedule does not become a new allowance.
A refusal already retained under the request key is not retried with fresh input.

The active lifetime begins after configuration/asset intake, as in the original
learned command. It is bounded by the minimum of recipe TTL, CONFIG runtime and
actor-profile runtime, then further narrowed by the admitted deadline. Initial
asset ingestion, synchronous journal operations and individual numerical work
are not preemptively bounded by socket budgets.

## Observation and cancellation while review runs

After the first original request is recorded, the actor channel uses only
`drive_learned_text_stream_request_observe`. That method accepts no policy-file
reader or clock. A missing key cannot consume an unused supervisor snapshot or
admit new work. Exact retries, Poll and Cancel remain on the same original weak
gateway and request book; reconnecting does not generate another message or
repeat an effect.

This observation-only intake is serviced before and between native sidecar
quanta, while waiting for native commit/reveal phase deadlines, between external
helper-process quanta, during human review, and before publication steps. Review
itself still obtains the original fresh source and time through its own driver;
servicing an actor frame does not replace either observation. The original
native/process owner, roster, input-refinement history and child cleanup custody
remain intact. No second parser, outcome reducer, helper scheduler or publisher
is introduced.

Request-local actor cancellation retires the original review and uses the
original request cancellation law. It is not reported as a nonce-authenticated
run-wide Stop. Paid numerical work remains retained; dispatched/unknown effects
remain governed by original reconciliation and charge rules, not a local refund.
The independent authenticated stop channel keeps its existing priority and
terminal protocol. All checks are cooperative boundaries, not an interruptible
kernel, independent watchdog or hard stop-latency guarantee.

Native review, the original permitting receipt, independent human approval,
current-input validation and the existing publication/reconciliation steps still
control release. A peer cannot approve its own message, see private reviewer
inputs or turn an unknown outcome into execution evidence.

## Response and cleanup

After the workflow finishes, the same observation-only session can serve a
bounded final response window of `ACTOR_PROFILE.reply_ms` inside the remaining
original lifetime. It does not start a fresh runtime. Transport success is not
proof that a peer received or persisted its result; reaching the window limit
can leave the peer uncertain. The operator's stdout remains the original typed
request response. A transport error after execution is reported alongside that
response and never triggers another generation, review, reconciliation or effect.

The peer session is revoked before its socket is released. Existing anchor
retention and the original bounded cleanup of both ordinary and process-backed
helpers still run. Recovery of an uncertain receipt remains an explicit use of
the existing learned receipt commands with the original request and independently
selected contracts; there is no automatic fallback to a new request or run.

## Change and qualification record — 2026-10-17

This source increment connects the pre-existing FA-107 scoped learned peer APIs
to the actual operator, including plain learned, raw-residual and owned-K/V
predictor modes and the independently selected native or external-process review
backend. It extends an existing command and wire vocabulary, adds no dependency
or journal event, and does not alter numerical generation or publication policy.

Ten new regression functions cover the shared live operator runner with actual
imported models, original native reviews, helper subprocesses, separate human
sockets and the real authenticated actor client. They cover all three predictor
modes, shorter accepted deadlines, malformed-then-correct envelopes, reconnects,
request-local cancellation, process-backed publication and human rejection,
separate finish without new sampling, a non-admitting observation phase despite
an unused snapshot, no publication when the actor request is missing, foreign
profiles, protected socket collisions, credential rejection and closed argument
routing. The lower-level negative observation case has a near-identical positive
admission on the same generated state. Fixture weights, ballots and same-host
credentials do not qualify model accuracy, independent judgment or OS containment.

These assertions have **not executed**. The targeted peer-intake selection, the
broader learned-operator tests and the unchanged `xtask check` were attempted
through `RCH_REQUIRE_REMOTE=1 rch exec -- ...`. All three exited 127 before
compilation because `rch` is unavailable. This environment also has no Cargo,
rustc or rustfmt. The source received manual review, not Rust syntax, type,
formatting, Clippy, runtime or release qualification. Earlier execution receipts
do not validate this change. Broader FA-107/FA-014 and helper isolation acceptance
remain open, and unavailable `br` was not replaced with manual Beads rewrites.

Required qualification in a complete authorized checkout:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication workflow::actor_service::learned::tests::peer_intake -- --test-threads=1
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication workflow::actor_service::learned -- --test-threads=1
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
