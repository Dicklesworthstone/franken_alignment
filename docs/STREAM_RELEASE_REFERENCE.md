# Cumulative, receipt-ordered streaming release

## Implemented capability and qualification

`PublicationEndpoint::new_stream` registers append-only complete-message output.
`DeliveryBroker` consumes that profile in its existing proposal, dispatch and
receipt paths. `OversightBroker::stream_message_spec` and `stream_finish_spec`
build unreviewed proposals from the broker's receipt-confirmed history. These
builders do not send bytes or create permits. There is no separate rights ledger.

This is safe, std-only reference functionality added on September 10, 2026.
Rust compilation, formatting, Clippy and the repository gate have NOT run in this
session. Cargo, rustc, rustfmt and the required RCH runner were unavailable.
No bead, production packet or source-snapshot qualification is closed. Existing
passing receipts cover other source, not this batch.

The implementation extends the existing in-memory endpoint, not a network or
filesystem adapter. Its consumer is the reference dispatcher deciding whether
it can disclose one complete reviewed unit without changing or overlooking what
was already disclosed. It does not establish host capture, helper honesty,
network authentication, storage durability, OS-crash recovery or real streaming
latency. Every public transition remains an atomic reference-model step.

## Complete units and cumulative context

The registered release unit is one nonempty complete UTF-8 message, or an explicit
Finish operation. Message boundaries are preserved independently of concatenated
text. UTF-8 completeness and bounded framing do not prove semantic harmlessness
or that a caller chose a good semantic boundary; those are the registered review
question's obligations. This endpoint never invokes tools. Tool-argument token
streams are not a supported effect profile and raw unframed requests refuse.

A release payload contains the profile identity and generation, frozen limits,
EVERY prior message with its byte length and boundary, and the new message or
Finish tag. Even identical concatenated prefixes with different message boundaries
are different histories. The endpoint compares that exact prior history against
its own actual visible state before releasing the new unit. It never takes the
actor's asserted offset, summary or reconstructed prefix as authoritative.

The complete frame is the action payload, so the existing exact-action and whole
helper-input bindings include it automatically. Exact policy operates on this
framed payload, not an implicitly extracted suffix. The helper's required action
frame includes the prior messages as well as the proposed addition. A caller
cannot satisfy this profile by retaining only a favorable isolated chunk.

The wire domain is FASOUT with version 1. Its 49-byte header contains two u64
identity fields, five u32 limit/count/byte fields, one operation tag, and one u32
new-message length, all integers big-endian. Each prior message adds a u32 length
followed by its bytes. Decoding is length-bounded before allocation, preserves
raw UTF-8 bytes without normalization, and rejects invalid tags, malformed UTF-8,
empty ordinary messages, inconsistent counts, truncation and trailing bytes.
`ReleaseFrame` is a borrowed inspection view, never a sendable envelope or permit.

The adapter contract version must identify these stream semantics. Ordinary
`PublicationEndpoint::new` retains its existing replace-payload behavior; an
attached endpoint has no mode-switch or profile-edit operation. Helper input
contracts and any enabled action-consistency calibration must be registered for
the framed stream action population. Reinterpreting a predictor for raw suffixes
or reusing a different action contract is not a semantics-preserving optimization.

## Live ordering and irrevocable disclosure

```text
receipt-confirmed prefix -> complete cumulative proposal
  -> exact policy / whole-input congress / normal authorization
  -> current action, evidence, optional activation/consistency and human checks
  -> one-use dispatch -> one outstanding stream obligation
  -> endpoint prefix/version check -> append or terminal nonexecution
  -> matching endpoint receipt -> advance the local confirmed prefix
```

The broker allows at most one unresolved dispatched stream operation. A status
miss, acknowledgment loss, expired retention or irrecoverably unknown outcome
cannot advance the prefix, free the stream slot, or admit a following message.
The endpoint may already be ahead of the local confirmed view; `stream_pending`
exposes that uncertainty. `stream_state` is not labelled as a fresh remote read.

Two speculative proposals can be reviewed against the same current prefix, but
only one can dispatch before reconciliation. After a successful release, the
other proposal's old target version and old cumulative frame are stale. It needs
a new proposal and review, not a retargeted existing permit. Cancelling the old
undispatched reservation refunds that reservation, not the disclosed prefix.

Only a matching terminal endpoint receipt resolves an outstanding operation.
Executed evidence advances the local prefix and resource version. NotExecuted
keeps both unchanged. The existing sealing operation prevents every delayed
message under the sealed key; a lookup miss alone does not. If execution won the
race, sealing returns its Executed receipt and cannot manufacture a refund.
Repeated old receipts are idempotent and cannot clear a newer pending operation.

The audience receives only new message content; the cumulative frame's header and
prior context are not re-displayed. Endpoint `payload` is the concatenated visible
text, and `stream_view().messages()` retains its exact message boundaries. A valid
Finish advances the resource version and closes the stream without deleting text.
Finishing is itself reviewed and charged. Cancelling or sealing a proposed Finish
is not end-of-stream. A lost Finish acknowledgment stays pending until reconciled.

Actor reset, policy replacement and dispatcher restart preserve the stream's
actual and receipt-confirmed histories. They cannot erase disclosure, reopen a
confirmed Finish, or free an unknown stream slot. A dispatched old-epoch message
can still execute before a dispatcher fence reaches the endpoint; that real
outcome must be reconciled. Once the endpoint installs the new fence, old sends
refuse. Fencing, cancellation and reconciliation do not require current helper,
activation or human availability.

## Resource contract and costs

A profile caps each message at 4,096 bytes, total visible text at 32,768 bytes,
and message count at 127; configured limits may be smaller. The endpoint requires
capacity for at least the configured maximum successful messages plus Finish.
Failed deliveries also consume retained keys, and the existing lifetime action,
review, capture and human-request budgets can refuse work earlier. These maxima
are ceilings, not a promise that every combination fits every other budget.

For p prior messages, B prior visible bytes and N new bytes, frame size is exactly
49 + 4*p + B + N. An empty Finish uses N=0. The maximum frame is 33,325 bytes.
The builder charges all frame bytes, including repeated context and metadata,
through the original effect-unit budget. Actual newly disclosed bytes are smaller.
No refund of prior disclosures is inferred from that conservative overcount.

This baseline deliberately repeats full cumulative context. Total framing and
review input can therefore grow quadratically with the number of messages.
Broker and endpoint preview validated next states by bounded copying, not a
claimed zero-copy or constant-time fork. Existing action retention remains capped
at 2 MiB; structured history adds at most the visible-byte cap plus one native
`usize` boundary per message per retained view. `retained_stream_bytes` reports
that additional local variable storage, excluding receipts, allocator overhead,
endpoint state and external caller copies. No peak-memory or throughput benchmark
has been executed.

## Regression source and remaining production work

Six wire/history tests and seventeen public integration tests accompany the
change. They cover successful disclosure and Finish; unchanged-byte boundary
substitution; malformed and truncated frames; cumulative holds and exact denial;
preapproved sibling races; duplicate receipts during a later pending operation;
individual human keys; both execution-versus-sealing outcomes; both delayed-send
versus fence orderings; reset with pending disclosure; policy rotation; retention
expiry and retained terminal evidence; capture outage; capacity and irreversible
Finish. The tests are source only and have not been executed.

The suites are `stream_release.rs` and `stream_recovery.rs`, sharing explicit
public-API fixtures in `tests/support/stream_fixture.rs`. Their helper verdicts
and snapshots are controlled inputs, not measured inference or a real provider.
The original decision archive retains the framed action under its existing
policy-replay contract. It does not thereby verify actual audience disclosure,
endpoint durability, complete control-ledger accounting or all optional gates.

This implements the bounded reference release semantics of plan 8.7 and 8.8,
serving FI-A04 and the existing FA-012/FA-017/FA-050 obligations without closing
those production packets. Actual behavioral capture, real buffered release,
latency/leakage measurements, authenticated adapters, task-owned durable state
and the required independent execution campaign remain outstanding.
