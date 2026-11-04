# Admit registered terminal completion capacity before domain work

FA-014, plan 16.4/25.4. A coordinator can have room for a visit intent but not
its eventual refusal. Starting such a visit creates avoidable unacknowledged
bookkeeping even when no domain operation was possible. Registered offline
terminal recovery now admits both the intent and the largest permitted negative
completion through the original coordinator encoder before starting a visit.

This applies to supplied-time, cooperative, terminal-clock, admission-only and
stopped-drain recovery, including their existing fleet/operator consumers. An
underfunded request refuses before a session attempt, durable intent, domain open,
clock or checkpoint. The coordinator remains healthy and its bytes and revision
are unchanged. The scratch maximum-size diagnostic is sizing data only: no such
placeholder is persisted, and actual refusals still come from native execution.

The capacity pin is unchanged and never enlarged. No bytes, attempts or rights
are refunded. Exact original codec admission accounts for path, plan, existing
visits, current retained heads and the selected legacy/new intent version.
No coordinator or domain format changes; historical replay does not perform this
new admission, and no stored success is restored as current evidence.

Genuine I/O errors remain unacknowledged when appropriate, and a caught unwind
keeps the entered owner unavailable. An in-memory sizing check does not guarantee
storage availability or durability. Existing live-owner and passive-resolution
APIs keep their original admission behavior.

Four authored regression functions exercise exact/one-byte-short budgets for
all five entry paths, original native successful controls, cancellation at every
fixture replay boundary, all five intent-write barriers and caught checkpoint
unwind. No existing assertions changed. Tests and the full xtask remain pending
exact-revision RCH execution; a successful commit is not qualification. FA-014
remains open. No dependency, runtime or effect authority is introduced.

## Bound successful retained history before the native write

The registered entry also encodes an Observed completion with the existing
retained domain heads. Removing their byte count gives the exact non-head
encoding size for that visit: the original format has fixed-width revisions,
counts and lengths, and no other visit can enter during this operation. The
remaining pinned coordinator capacity becomes a session-only aggregate head
ceiling, additionally constrained by the unchanged independently registered plan.

The original canonical-acquisition and projected-terminal-head checks consume
that ceiling before any native Stop/Fence/StopProgress write. A valid but too-large
successor, or a terminal tail that would make success impossible to acknowledge,
returns the real Limit refusal through the capacity reserved at entry. Canonical
and staged domain bytes remain untouched; no speculative head becomes retained
history. Exact-limit successful controls still use the original native operation
and coordinator acknowledgment. The entire roster's retained heads and all prior
visit diagnostics count, not just the current domain.

The ceiling is cleared at completion or the next admitted campaign entry, is
absent on reopening, and is never encoded into a plan, coordinator or domain.
It neither enlarges a pinned limit nor replenishes a visit/reserve. Existing
passive exact-cut refreshes remain write-free, including at capacity. Historical
replay and live-owner visits keep their original decisions. Allocation or I/O
failure can still follow successful preflight and remains a separate uncertainty
boundary requiring actual native evidence, never an assumed nonexecution receipt.

Five additional regression functions cover exact/one-byte-short successful heads
through all five registered entry paths, oversized acquisition before replay,
multiple retained domains, ephemeral ceiling isolation and anchored reopening,
and all five coordinator-completion write barriers after actual native settlement.
All earlier tests and assertions are retained unchanged. These
nine total new regression functions remain unexecuted until the exact-revision
remote gate; the current environment lacks rch and the Rust tools.
