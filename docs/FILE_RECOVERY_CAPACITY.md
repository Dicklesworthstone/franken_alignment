# Preserve journal capacity for terminal recovery

Consumer: FileDelivery and FileOversight hosts that must close and settle their
original effect domain when ordinary journal traffic reaches its declared bound.
This implements capacity admission at the existing effect/recovery boundary
(plan sections 8.3–8.6 and 16; FA-014). It adds no authority reducer, external
credential, runtime, filesystem sink, dependency or actor command.

## Failure mode and scope

The original bounded journal admits ordinary events up to the same event/byte
limits used by reopening fences and stop sweeps. Once exhausted, a writable owner
cannot append its stop, and opening cannot append the required recovery fence.
A bounded store must therefore allocate logical recovery space BEFORE new work,
not silently enlarge its limits, reset rights, truncate history or waive fencing.

`enable_recovery_reserve(revision, RecoveryReserve)` installs that allocation as
an explicit durable input, before work. The same shared admission implementation
checks every encoded history prefix in both persistent profiles. Canonical decode
re-encodes and checks these prefixes too: a framed imported history cannot hide
ordinary work inside its reserve. Installation is bounded, one-time and immutable.
It may follow initial clock observations or the full-input publication-guard
configuration, but not actor/operator work, refused recorded submissions, policy
updates, or an earlier recovery fence. Unconfigured legacy profiles are unchanged.

All previously written bytes count: bootstrap, record headers, initial marker,
clock events, reviews and recovery operations. Ordinary admission is limited to
`limits - reserve` in BOTH dimensions. Recovery can use the original full limits,
never a larger value. Only the original Fence, Stop and StopProgress inputs may
spend the tail. A PublishChecked operation is ordinary even when it might discover
nonexecution; it can execute and cannot be treated as an emergency-only query.
Human approvals, helper replies, policy updates, cancellation, routine clock reads
and routine status sweeps also cannot spend the protected tail.

The marker is base event tag 15 (bounded event count plus byte count), explicitly
admitted under the full-input journal's Core tag. Its original bootstrap, domain
bytes and older event encodings remain unchanged. Old readers reject this marker.
The marker itself changes no rights or native broker state; its only consumer is
the canonical journal's capacity-admission rule.

## Minimum terminal plan and finite recovery debt

`RecoveryReserve::terminal()` reserves three events and fifty encoded bytes.
This covers one full-input Fence (6 bytes), Stop (30 bytes) and StopProgress
(14 bytes), including record framing. The simpler format uses three fewer bytes.
The sweep input contains only its elapsed tick, not a serialized result map;
original endpoint outcomes and accounting are recomputed by the original reducer.
The same sweep can therefore settle the complete bounded pending set without
serializing an event per refund. Larger explicit reserves can cover additional
recovery attempts. Limits and encoding changes must update/test this byte bound.

The guarantee is logical space after a last ordinary admission, not successful
I/O, trustworthy time, endpoint retention or an unlimited number of crashes.
Each acknowledged recovery event consumes actual capacity. A crash after an
initial reopening and a subsequent stop may require ANOTHER reopening beyond the
minimum plan. Repeated recovery faults/reopens require a larger declared reserve.
No operation replenishes headroom, compacts away history or raises bootstrap caps.
A fully consumed store may retain only pure historical-read access; it cannot
silently skip its recovery fence to return a writable authority.

`journal_capacity()` reports the exact acknowledged encoded size, total remaining
capacity and ordinary remaining capacity to the trusted supervisor. It performs a
bounded re-encoding, not a filesystem free-space query or a latency measurement.
A storage-faulted owner refuses it because the canonical file might be newer.
Neither an available byte count nor a local stop is endpoint nonexecution evidence.

## Original effects and verification boundary

A tail-backed stop still refunds only undispatched reservations. Executed effects
remain charged; missing outcomes require the original endpoint seal/reconciler;
expired retention remains unknown and charged. Human withdrawal, request identity,
revocation floors, source checks and publication guarding keep their original
semantics. The reserve cannot approve an effect or resurrect a cancelled request.

Nine public owner scenarios and two admission unit tests cover both dimensions,
complete mixed-effect recovery at the event limit, exact fifty-byte full-input
framing, denied installation, legacy compatibility, unchanged guarded publication,
rejected approvals and publication at capacity, and retained expired liabilities.
Driver and real-storage/process cases are in FILE_CAPACITY_DRAIN.md.

The Rust source and tests have NOT been compiled, formatted, linted or executed.
The required RCH command failed to start with command-not-found (127). No earlier
execution receipt validates this increment, and no Beads or production gate is
closed. This remains operator-controlled Unix storage without disk preallocation,
anti-rollback protection, authenticated journal origin or hostile-host containment.
