# Recovering stopped publication domains without a surviving owner

## Consumer and API

The privileged `FileShutdownCoordinator` now supports
`recover_and_drain(revision, domain, at)`. The domain must already belong to its
independently supplied, fixed `FileShutdownPlan`; there is no arbitrary path,
replacement profile, imported outcome or reviewer argument. This completes the
existing shutdown coordinator's local crashed-owner path rather than introducing
a second authority ledger. It uses the original L4/L5 terminal transitions and
[mandatory two-key host recovery](TWO_KEY_TERMINAL_RECOVERY.md), serving plan
sections 8.3, 8.4, 8.11 and 16.4 and FA-006's recovery ordering obligations.

A visit durably records coordinator intent before opening the domain. The
original cooperating Store lock must be acquired; busy/offline domains refuse,
and no lock is stolen. While holding that lock, the coordinator checks the exact
registered profile and newest acknowledged complete history prefix. A rollback
or same-length valid substitution refuses before native mutation. The original
passive reader's unacknowledged numerical-bootstrap restrictions also apply.

Only then can the private original owner perform Stop, Fence and StopProgress
in one canonical replacement. Stop preconditions are derived from that validated
original controller, not from an imported receipt or an invented post-fence epoch.
Every prefix passes the native event/byte reserve checks. The entire resulting
comparison head must also fit the coordinator's fixed evidence budget before the
domain changes. No live owner, human reviewer or effect key is returned.

## Retry and failure semantics

The domain journal and coordinator journal are **separate** stores. There is no
distributed atomicity claim. Coordinator intent, native terminal acknowledgment,
and coordinator completion are distinct durable boundaries:

- A failed intent write never starts domain recovery.
- A native refusal is returned as an inner error only after the coordinator
  durably records it. A native storage error may still leave the complete native
  terminal image visible; it is not a nonexecution receipt.
- An outer completion error can follow a successful native shutdown. Reopening
  the coordinator preserves pending visits but restores no current success.
  Fresh canonical inspection/recovery resolves what actually reached the domain.

An already drained canonical stop for this operation is confirmed without
appending another domain stop, fence or clock event. Historical replay stays in
memory; synchronization and legitimate staged-file cleanup can still occur.
This path works even when the original three-event recovery tail is completely
spent. A stale supplied tick is irrelevant to this read-only observation, which
makes no new time claim. It cannot return a writable owner. Each retry still
consumes the coordinator's finite visit/evidence allowance.

Otherwise `at` must be a fresh trusted observation in the registered domain's
clock system. Executed effects remain charged; expired unresolved liabilities
remain explicit; only the original endpoint's nonexecution evidence permits
refunds. No publication is resent. Every registered member remains in the report,
including busy, absent and refused members. A recorded failure in one domain does
not prevent visiting others, nor does another domain's success hide the failure.

## Format compatibility and boundaries

Legacy coordinator histories retain `FASHCOO\x01` and their original encoding.
A `RecoverStopped` visit requires `FASHCOO\x02` with explicit operation tag 3.
The decoder rejects a downgraded header or a noncanonical upgraded legacy image;
old readers reject the new header. Exact plan, path, capacity, revision floor,
latest acknowledged heads and pending-visit accounting remain mandatory. The
domain journal, native event format and authority rules do not change.

These are operator-controlled local canonical files, not remote endpoints or an
OS watchdog. The operation does not kill a live actor process, authenticate
storage, prove a simultaneous fleet stop, recover inaccessible machines or detect
rollback of all independent control records together. Use the live-owner advance
path when a cooperating domain still holds its lock. Exhausted unreserved domain
journals and exhausted coordinator capacity are not silently enlarged.

## Status and change log

2026-09-20 UTC: added the real coordinator recovery path, explicit versioned visit
encoding and eleven Rust regression tests, alongside eleven host terminal tests
in the preceding commit. New cases cover a crashed guarded domain with reserved
work and an exactly spent tail; busy/missing members; rollback and valid history
substitution; comparison-head byte neighbors; interruption between store commits;
all five intent, native and completion storage barriers; version downgrade;
conflicting stop identity; and stale/unknown coordinator requests. They use
actual canonical files and original reducers, with deterministic fault injection,
not hardware power-cut or native-helper inference evidence.

Required verification was attempted again:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It exited 127 because `rch` is absent; Rust, Cargo and rustfmt are unavailable.
Lexical/delimiter and whitespace screens passed over the fetched partial source
workspace. Those screens do not compile Rust or execute these tests. Compilation,
formatting, Clippy and runtime tests remain **unexecuted**; no production gate,
historical test count, dependency admission or Bead closure is claimed.
