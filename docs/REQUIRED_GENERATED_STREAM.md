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
