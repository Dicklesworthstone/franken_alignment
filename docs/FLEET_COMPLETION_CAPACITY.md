# Reserve registered terminal refusal capacity before domain work

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

This increment reserves a negative completion, not arbitrary successful head
growth. A later retained-domain image can still exceed the coordinator ceiling;
that positive-result preflight is separate from this refusal guarantee. Genuine
I/O errors remain unacknowledged when appropriate, and a caught unwind keeps the
entered owner unavailable. An in-memory sizing check does not guarantee storage
availability or durability. Existing live-owner and passive-resolution APIs keep
their original admission behavior.

Four authored regression functions exercise exact/one-byte-short budgets for
all five entry paths, original native successful controls, cancellation at every
fixture replay boundary, all five intent-write barriers and caught checkpoint
unwind. No existing assertions changed. Tests and the full xtask remain pending
exact-revision RCH execution; a successful commit is not qualification. FA-014
remains open. No dependency, runtime or effect authority is introduced.
