# Preserve acknowledged coordinator history during a live session

FA-014; plan 16.4, 16.5 and 25.4. The independently retained coordinator
anchor protects reopening. It does not by itself prevent a live owner from
replacing an out-of-band changed canonical file using its earlier in-memory
visits. The original coordinator now retains the exact bytes acknowledged by
creation, a successful replacement, or verified open, and compares canonical
storage with that image at the following original operation boundaries.

## Native consumers

Every coordinator commit checks the complete last acknowledged image immediately
before the original Store replacement. Intent commits therefore refuse before
domain acquisition, live-owner mutation, clocks or replay callbacks. Completion
commits check again after the native operation. A changed canonical file or
failed comparison read returns an outer, unacknowledged error and leaves the
owner unavailable until explicit verified reopening. Restoring the old bytes
does not make that same in-memory owner usable again.

The completion check cannot undo native work. A clock or replay callback may
coincide with a coordinator replacement after the intent was acknowledged; the
domain can still complete its original Stop/Fence/StopProgress transaction
before coordinator acknowledgment refuses. The observed domain outcome must be
resolved from native evidence, not inferred from this error or repeated. Sent,
executed and unresolved liabilities keep the original accounting.

Passive retained-visit resolution performs no intent write and may perform no
completion write. It consequently checks coordinator custody before domain
reading and before returning its completed or refused observation. Cancellation
also checks before clearing unavailability. A failed domain read can remain a
session-only negative result when coordinator custody is unchanged; lost
coordinator custody cannot be cleared by that refusal or cancellation. Pending
resolution and exact-cut refresh still use the original visit rules and never
issue a replacement domain command.

Creation and opening establish the initial comparison bytes; only a fully
acknowledged replacement advances them. A failed final directory sync can leave
new canonical bytes visible without advancing this owner's comparison image,
revision or report. The owner remains quarantined. Explicit reopen validates
the actual visible file, with the existing independent anchor/floor when supplied,
before cleanup. Opening compares its captured file once more after decoding and
anchor validation, before confirming and cleaning staging.

## Compatibility, costs and assumptions

No coordinator, plan, domain, anchor or actor-wire encoding changes. Neither
capacity limits, session/visit allowances, historical replay nor permission
rules are enlarged. Ordinary reads of a report or historical anchor remain
observations of the owner's last acknowledged cut, not continuous disk monitoring.

The owner retains at most one additional canonical image, bounded by its existing
max_bytes. Each comparison is a bounded full-file read and exact byte comparison;
this prioritizes detected replacement over throughput. There is no content hash,
checkpoint import, new effect executor or restoration of live authority.

These checks reject replacements visible at the comparison boundary. They are
NOT an atomic filesystem compare-and-swap and cannot prevent a non-cooperating
writer from racing after the final comparison, swapping a lock inode, restoring
identical bytes, or jointly rolling back all independently retained custody.
Trusted filesystem isolation, cooperating writer locks and separate anti-rollback
custody remain required. Read, decode and storage operations remain synchronous;
no hard-real-time shutdown guarantee is added.

## Verification status

Seven authored regression functions use the original native journals, fixed
Store failure barriers and two-key effects. Coverage includes valid rollback and
equal-revision forks through seven new-work routes, all five intent/completion
storage barriers, late coordinator replacement with each native liability class,
passive finish/cancel for pending and recorded visits, missing coordinator data
before domain intake, healthy read-refusal/cancellation/refresh controls, and an
unacknowledged first fleet member with an explicitly unvisited tail.

Existing tests and assertions are unchanged. These Rust tests have not executed.
This increment is prepared in a selected-file worktree, not a full repository
checkout. Exact-revision remote RCH tests and the unchanged xtask gate remain
required; source/hash/patch checks are not build, formatting, Clippy or runtime
qualification. FA-014 and its full prerequisites remain open.
