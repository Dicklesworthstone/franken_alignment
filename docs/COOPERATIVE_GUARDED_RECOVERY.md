# Single-pass recovery of the original guarded publication owner

## Capability and scope

The base `FileOversight` guarded owner now has `begin_open_guarded` and
`begin_open_guarded_anchored`. Both return `FileGuardedRecovery`, which executes
at most a caller-selected number of original journal transitions per poll.
The existing `open_guarded` and `open_guarded_anchored` drive this same cursor.
Their public return types and guard/role contracts are unchanged.

This serves the existing FA-014 recovery and FA-107 one-way custody contracts
(plan 8, 9.3, 16 and 18; founding FI-A02/FI-A16). The consumer is the original
composed publication owner, not the separate reference rights journal. It adds
no event format, authority, dependency, executor, command or deployment profile.
Specialized learned, evaluated, predictive and mediated recovery entry points
retain their independent contracts; this base path cannot drop those gates.

Previously these guarded entry points replayed the journal to validate the
independent requirements, then sent Fence through a transaction that reconstructed
history again. Now each historical transition runs once. Finalization applies
one original Fence to the verified machine and uses the existing
`persist_candidate` canonical-replacement and poison boundary. There is no second
policy reducer, imported snapshot, effect executor or replacement authority owner.
No throughput or latency result is claimed.

## Frozen inputs and withheld authority

The original Store lock, canonical bytes, decoded events and independently
supplied guard inventory remain owned by the cursor. The complete existing
framing and decoder-configuration checks run before any historical transition.
The anchored entry point also checks the original exact-prefix anchor before
replay. A matching prefix does not excuse an invalid suffix. Full guard,
effective-policy, credential-epoch and external-floor checks precede Ready.

A poll names its verified-event predecessor. Stale or invalid quanta do no work;
each attempted original transition is counted before calling the reducer.
A returned error is sticky. An unwind marks the private machine Interrupted,
including when an event already changed private state. Neither can be resumed
as a successful prefix. Progress exports counts and status, never tokens, a
mutable machine, keys, source eligibility or a role bundle.

Ready is not a live owner. Drop/cancel at any point, including Ready, releases
the lock without cleaning staging evidence or writing a fence. Successful
finish rechecks the exact canonical image, then acknowledges the original
recovery fence before issuing the owner and separately held roles. Saved clock
and identity/source eligibility stay withdrawn; old keys cannot be used. Unknown
or dispatched effects remain original reconciliation obligations, not refunds
or invitations to repeat a publication.

## Capacity and operation boundaries

`require_recovery_reserve` can require the exact original reserve before the
first event attempt. Missing, changed or late selection consumes the cursor and
refuses without writes. It cannot install or replenish a reserve. The original
journal decoder and encoder enforce its placement and cumulative limits.

Fence capacity, encoding and retained-cut comparison precede staging cleanup.
A full unreserved journal is not repaired or silently given extra room. A
reserved journal can spend its existing Fence allowance after ordinary capacity
is exhausted. All synchronization and interrupted-replacement rules remain in
the original Store; failed finalization returns neither owner nor roles.

Construction still reads/decodes the bounded complete image synchronously.
An event can contain a whole original numerical operation. Requirements checks,
encoding, the final file comparison, synchronization and fencing remain
synchronous. Event quanta are logical scheduling boundaries, not CPU, memory,
physical I/O or wall-clock limits. Canonical-cut comparison does not authenticate
a malicious storage writer, and the existing lock assumes cooperating writers.

## Source status and change record

Added the sealed recovery cursor and connected both original synchronous
consumers. Ten new native regression functions cover original-machine/encoder
parity, both entry points and three poll schedules, invalid polls, early finish,
all event-cut cancellations with real staged bytes, independent requirements,
invalid suffixes, equal-counter forks, changed canonical images, all five Store
replacement barriers, post-application unwinds, reserve preservation and full
unreserved journals. Three compile-fail examples protect owner/role withholding
and one-use finalization. Existing test bodies and assertion semantics are intact.

These Rust tests and examples are authored but UNEXECUTED. The preparation
runtime has no `rch`, `cargo`, `rustc` or `rustfmt`. Required RCH launches stop
before compilation. Hash/whitespace checks do not qualify Rust behavior, crash
resilience or production readiness. No bead is closed and no prior receipt is
used to qualify this source. Run the original integrated gate:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference guarded
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Original effect and two-key composition controls

Three additional native regression functions use the actual FileOversight action,
full-input congress, independent human key, checked-publication and reconciliation
paths. They exercise eight combinations of four lifecycle cuts and two polling
schedules, from approved-but-unsent to receipt-confirmed publication. The original
full Machine replay plus Fence supplies the independent result oracle. Old keys
and old reviewer custody refuse; a fresh original review and new human role can
successfully publish after recovery. Neither an unknown charge nor a visible
unacknowledged effect is forgotten or reexecuted.

All five original publication replacement barriers are injected before recovery.
The canonical endpoint outcome, not the failed caller's stale RAM image, decides
whether reconciliation observes execution or an unresolved liability. Missing
current input at a newly recovered owner's real final gate still seals the effect
without execution; the near-identical complete-input case publishes. Only original
receipt reconciliation refunds proved nonexecution, and later better evidence
cannot reroll a sealed outcome. The journal's read projection equals the live
acknowledged cut after each completed scenario.

The manual congress ballots are explicit synthetic controls, not native helper
inference or evidence of detection quality. File I/O uses the existing original
Store; injected errors are not power-cut experiments. These tests are authored
but unexecuted under the same RCH limitation above. No production qualification
or new authority semantics follows from their existence.
