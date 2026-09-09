# Receipt-gated delivery and dispatcher recovery

## Implemented scope

This is an in-memory reference implementation in
`crates/fa-reference/src/action/consequence/delivery.rs`, with the separately
owned endpoint and recovery implementation under `delivery/endpoint*`.
The consumer is an effect dispatcher deciding whether an interrupted publication
is still an unresolved liability, definitely executed, or definitely prevented.
It composes the existing exact-policy, congress, permit and containment path;
it does not introduce a second budget ledger.

**Verification pending.** These sources and their regression tests were added
on September 9, 2026. Cargo, rustc and RCH were unavailable in this session.
No compilation, rustfmt, Clippy, Rust tests or repository gate were executed.
Historical receipts do not qualify these changes and no bead was closed.

This is scoped reference progress toward the FA-012/FA-014 effect-boundary and
reconciliation obligations, not completion of those production packets. Their
FrankenSQLite, broker, foundation-admission and real endpoint prerequisites
remain mandatory. This implementation neither substitutes a storage backend
nor establishes persistence, OS crash recovery, network authentication or a
production exactly-once guarantee.

## End-to-end path

```text
DeliveryBroker owns PolicyAuthority
  -> exact policy / derived witnesses / independent congress
  -> normal authorization reserves rights
  -> endpoint fence acknowledged
  -> dispatch consumes the permit and charges the original rights
  -> opaque DispatchEnvelope with only execution-bearing fields
  -> separately owned PublicationEndpoint checks exact resource version
  -> keyed terminal receipt, or unresolved acknowledgment loss
  -> receipt-gated reconciliation in the same policy authority
```

The endpoint is a bounded compare-and-publish model with its own payload,
resource version, terminal key records and execution count. It is not an
always-success callback. It accepts only messages issued by its attached broker,
checks the registered adapter/object/contract/generation, enforces the declared
payload-byte bound, and checks the expected resource version at execution.
Publishing changes its payload and advances that version. A duplicate key returns
its existing terminal receipt rather than publishing again.

This profile treats resource units as an upper bound on payload bytes. It does
not interpret the same units as money, arbitrary invocation cost or physical
exposure. One endpoint serves one attached authority scope and one resource.
Other resources, non-idempotent services, distributed ownership and lease-based
multi-endpoint operation require separate contracts.

## Absence is not nonexecution

A lookup can return `AwaitingResolution` while the original message is still
queued. Neither that result, a timeout, nor retention expiry refunds the charge.
`reconcile_status` can only mark such an in-flight attempt Unknown.

`seal_unexecuted` is the stronger operation: it atomically records a terminal
nonexecution result **and blocks any future delivery under that exact key**.
If execution won the race, sealing returns the existing Executed receipt instead.
The local broker refunds only after accepting a matching terminal NotExecuted
receipt from the originally attached endpoint. A late original message then
sees the terminal seal, not a newly available effect slot.

New work after such a refund requires a fresh attempt, policy evaluation,
congress and permit. There is no automatic replay under a fresh key and no raw
TrustedOutcome interface on DeliveryBroker. An identical accepted receipt is
idempotent; changing its key, originating endpoint or execution-bearing fields
cannot settle another attempt. Receipt equality includes the endpoint instance,
not merely equal Rc payload values.

## Recovery and fencing

`restart_dispatcher` models interruption between public protocol operations,
with the control state and endpoint state surviving. It marks every outstanding
Dispatching attempt Unknown, preserves terminal and irrecoverably unknown
liabilities, increments the dispatcher incarnation and closes new sends until
the endpoint acknowledges its fence. It does not restore an authority snapshot
or manufacture a replacement budget.

Issuing a fence is not the same as delivering it. An old message can still
execute before the endpoint installs the new fence; that result remains real
and is reconciled. After installation, older-incarnation messages and sealing
requests refuse. Older acknowledgments cannot reopen a newer recovery, and old
fence requests cannot lower the endpoint's floor. Status reads can recover an
already-terminal outcome without re-executing the effect.

Reserved but undispatched attempts do not become spent merely because the
worker restarts. After the new fence acknowledgment they can dispatch through
the unchanged current-policy, expiry, evidence and one-use checks. Conversely,
already charged effects are never reconstructed as merely reserved work.

`pending_reconciliation` enumerates retained unresolved obligations as status
queries, not dispatch messages. Completed and irrecoverably unknown effects are
not automatically retried. Policy rotation and actor checkpoint reset continue
to preserve post-dispatch liabilities. A receipt for an already executed effect
can still be accepted after those control changes or after local evidence expiry.

## Status retention and bounded obligations

Broker and endpoint use explicitly supplied monotone ticks in the same declared
logical clock domain. This model does not establish clock synchronization or
translate real remote clock domains. Dispatch computes a checked retention end
before spending the permit. The action deadline and status-retention deadline
are distinct: an expired action may be definitively refused and sealed while
its status is still retainable. At the retention boundary a missing receipt
becomes unavailable, not proof that execution never happened.

An already retained terminal receipt remains terminal evidence after retention
expires; this does not create a new provider lookup guarantee. The reference
keeps all accepted key records and never recycles their identities. It bounds
entries to the configured cap, at most 128, and counts each local retained
action's payload and read-witness bytes against 2 MiB before issuing a message.
The endpoint has at least one reserved record slot per admitted local delivery,
so the new-delivery cap does not prevent sealing an existing outstanding key.
These are logical bounds, not allocator, latency or throughput measurements.

## Endpoint privacy boundary

`PublicationRequest` contains only schema version, scope, resolved target,
payload, policy epoch, deadline and units. It contains no read witnesses,
congress transcript, actor cache or checkpoint. Dispatch messages, status queries
and endpoint receipts use this projection. The full witnessed action and review
basis remain in the local controller. Matching every projected field back to
the locally retained action preserves exact effect identity without exporting
the controller's unrelated observations.

Opaque messages, queries, fence acknowledgments and receipts use process-local
issuer brands. They are not serialized credentials, hashes or signatures, and
this code does not authenticate an external service. A real adapter needs an
admitted authenticated transport and its own tested durable idempotency,
nonexecution-sealing, retention and fencing implementation. There is deliberately
no interface that labels arbitrary caller-provided outcome bytes verified.

## Regression source and limitations

Twenty test functions and one compile-fail doctest accompany this batch. The
public tests drive real reference policy/congress reviews through authorization,
dispatch, endpoint payload mutation and reconciliation. They cover duplicate
messages and receipts; acknowledgment loss after execution; missing status;
both execution-versus-sealing outcomes; remote version conflicts; retention
expiry; foreign endpoint evidence; all six orders of delayed delivery, fence
installation and sealing; interruption before and after dispatch; stale fences
and queries; capacity; policy replacement; actor reset; private witness exclusion;
and retention arithmetic overflow. Every permitted recovery has a near-identical
forbidden case. These tests have not been executed.

No instruction-level crash injection, kill/restart subprocess experiment,
fsync/barrier test, provider data-loss or rollback campaign, remote signing-key
validation, Asupersync task cancellation, or production adapter qualification is
claimed. Each public reference call is treated as an atomic model step. In
particular, the broker's in-memory insertion and the endpoint's payload/receipt
updates are not claimed crash-atomic on real storage. The production system still
needs a single durable authority transaction and actual provider contracts at
those boundaries.

The founding connection is FI-A02/FI-A06/FI-A16: external control must constrain
what happens, retain the evidence for what actually happened, and never confuse
actor rewind or missing observation with reversal of an external effect. The
relevant plan sections are 8.1–8.8, 11.10, 15.4 and 16.1.
