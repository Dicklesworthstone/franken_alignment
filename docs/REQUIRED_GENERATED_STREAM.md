# Mandatory native-source message streams

## Capability and status

`FileOversight::create_generated_text_stream` creates the original complete-message
publication owner with an immutable requirement that every newly proposed message
come from `submit_decoder_text_message`. It installs the original monitored
numerical configuration and immutable tokenizer in the same first canonical
image. No input inference or effect occurs at startup. Eight new
`generated_message_required_` regression tests are authored, not executed.

The consumer is a supervisor operating a hosted native actor whose external
messages must be its exact acknowledged generated output. The prior source-linked
submission API remains useful in mixed workflows, but ordinary callers could
still submit arbitrary message bytes on those streams. This new explicit mode
makes source linkage mandatory without changing the legacy `create_stream`
contract or granting a new actor capability. It serves the complete-message
release, exact input/identity and one-directional effect-boundary contracts in
plan sections 8.7, 17.7 and 25.1. It does not close a production work packet.

## Original execution and authority

A source-required stream uses first-event tag 32, `GeneratedStreamBootstrap`.
All previous tags and encodings retain their meanings. The native stream
bootstrap still constructs the original endpoint and publication guard, and
its first-event restriction prevents either retrofitting or disabling this mode.
There is no live setter. `generated_text_stream_required` reports the last
acknowledged mode and refuses after a storage fault.

The original machine checks the canonical event's source before expanding a
source-linked message into the original request submission. That derived input
passes through the same decoder, consistency, policy and request reducer,
including stop cleanup and request projection. There is no callback, imported
report, alternate authority book or caller-supplied message in this expansion.
The source expansion now occurs exactly once per application rather than
recursively refreshing the same request book twice.

Direct `propose`, ordinary `submit_request`, and the existing actor gateways
cannot submit message bytes, even bytes equal to the model output. The live
check occurs before replay/prediction accounting, while semantic replay enforces
the same rule. A rejected unlinked input has no acknowledged request; it cannot
consume a pending forecast or leave a speculative admission. Exact ordinary
retries of an already source-linked request can read its current status, but
cannot change the original source or cause another effect.

Native submission still requires the fully reviewed original prompt, a natural
monitored control-token stop, exact nonempty UTF-8 and the current numerical
predecessor. Cancellation, incomplete sampling and invalid UTF-8 cannot be
repaired by supplying replacement message text. Congress, the separately held
human key and final publication checks remain mandatory. Generation itself,
proposal admission and a quiet monitor are not publication permissions.

A finish frame can use the original ordinary proposal path because it adds no
message. The original frame parser and cumulative-prefix validator, policy,
congress and human-key machinery still decide it. It cannot append a message,
finish an unreviewed generation, omit already confirmed context or claim an
external audience received anything. Existing obligations and their receipts
remain independently settleable.

## Qualification

Tests pair native/legacy admission, direct/actor-gateway refusals, the actual
modeled two-key publication path, cumulative finish admission, source-link
substitution during replay, bootstrap immutability, exact event limits,
cancellation/batched partial output, hand-built tag-32 bytes and pre-action
forecast conservation. They use the original numerical implementation with
synthetic weights and actual temporary journal files. They do not demonstrate
trained-model accuracy, OS isolation, hardware power-loss durability or remote
recipient delivery.

The targeted RCH attempt failed before compilation because `rch` is unavailable
(exit 127). Compilation, tests, rustfmt, Clippy and the full gate are UNEXECUTED.
No local compilation fallback was used. Source whitespace and patch/hash checks
are tracked separately and do not qualify runtime behavior.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generated_message_required_
```

Old readers reject tag 32 rather than treating its mandatory-source mode as a
legacy stream. Existing stream-shape readers accept either bootstrap, replaying
and retaining the actual mode; they do not independently require native-only
semantics. No dependencies, new actor verbs or remote endpoints are added.


## Recovery and reserved termination capacity

`open_generated_text_stream_anchored` pins the mandatory-source bootstrap in
addition to the original exact stream, model, monitor, sampler, tokenizer,
base guard set, current policy, credential state and external history anchor.
A fully valid legacy stream with its own matching anchor is still not accepted
as this stronger contract. Mode checking precedes numerical replay. There is one
read image, one original replay, one original fence and no returned role before
acknowledged storage. Reopening preserves the native cursor, spent draws and the
source requirement, while withdrawing clock readiness and pausing inference.

This specialized entry point is the base guarded profile. Additional evaluated,
predictive and mediated configurations still use their existing composed
anchored openers, which preserve the mode when their retained anchor covers the
bootstrap. Those entry points do not independently assert native-only mode.
Ordinary stream-shape readers/openers also retain the replayed requirement;
none converts a source-required owner into a legacy caller-text owner.

`create_generated_text_stream_with_reserve` additionally installs the ORIGINAL
`RecoveryReserve` before decoder/tokenizer work in that same first image. This
ordering matters because the existing encoder forbids installing a reserve
after work starts. The simple constructor retains its explicit unreserved mode.
No old event classification or reserve arithmetic is changed. The reserve is
not a new allowance or reserved filesystem space: only the original fencing,
terminal-stop and stop-progress records can use it, and used space is never
replenished. Repeated recoveries, including visible but unacknowledged fences,
can exhaust the finite tail; ordinary generation cancellation is not promoted
into a new recovery-budget class.

Seven further `generated_message_required_` tests cover every acknowledged
cursor boundary, current source admission after explicit resume, legacy-mode
refusal with a valid anchor, all five original Store failure barriers, valid
mode substitution against an independently retained anchor, guard mismatch,
generic reopening without a mode downgrade, exact retry after recovery,
existing obligation reconciliation, and reserved-capacity bootstrap/termination.
The capacity test fills the ordinary event allowance, refuses another source
submission, and then uses exactly the reserved Fence/Stop/StopProgress tail.
A forged finish with unpublished prior context is also paired with a permitted
native cumulative finish in the first test set. None of those assertions has
executed; synthetic model weights and injected Store failures are not evidence
of trained-model accuracy or hardware power-loss behavior.

All FIFTEEN new tests, compilation, rustfmt, Clippy and the full gate remain
UNEXECUTED. The final changed-source RCH attempt again failed before compilation
because the runner is missing (exit 127). No broad bead or production gate is
closed, and no passing-test claim is made.
