# Capacity for revocation and settlement after ordinary journal exhaustion

This opt-in reference increment serves the original FA-013/014 fencing and
unknown-reconciliation obligations and the bounded durability contract of plan
16 and 18.3. The existing fa-journal owner is the consumer. Without a reserve,
ordinary records can fill its final byte or record slot and prevent later
revocation or reconciliation from being recorded. This increment preserves a
finite logical tail for those operations; it is not another authority ledger.

## Explicit immutable profile

`Journal::new_with_recovery_reserve(identity, limits, reserve)` selects version 2.
The 120-byte header contains the original identity, all three lifetime limits,
the two reserve dimensions and the checksum. Recovery requires the same exact
independently supplied profile. Version-1 `new`, `replay` and `begin_replay` keep
their old 84-byte header and original interpretation; they cannot silently open
a reserved journal. No operation installs a reserve late, infers one from disk,
widens it at recovery, migrates history or replenishes a consumed allowance.

V2 uses magic `FARJ\0\0\0\x02`. After the original run/policy/total fields are
max-frame u32, max-records u64, max-total-bytes u64, reserved-records u64 and
reserved-bytes u64, all little-endian, then CRC-32 of the preceding 116 bytes.
Event tags and payload encodings are unchanged. Chain checksums naturally bind
the new header. CRC remains accidental-corruption detection, not authentication.

## The same admission law at every path

Ordinary events (`Reserve`, `Dispatch`, `ResetToCheckpoint`) must fit below both
`max_records - reserved_records` and `max_total_bytes - reserved_bytes`.
`RevokeEpoch`, `MarkUnknown`, `AbortBeforeDispatch` and `Reconcile` can use the
remaining total capacity. Their original Rights state checks still apply: this
classification supplies no receipt, no effect permission and no new refund law.
In particular an unknown/dispatched effect cannot be aborted as unsent.

All events consume the same monotonic count and encoded length. Recovery records
written early also consume ordinary space; the reported ordinary and recovery
balances are not additive. Refunding effect units does not refund journal space.
Once ordinary space is exhausted it stays exhausted, including after reopening.
The reserved count and bytes are independent ceilings, not a guarantee to fit
arbitrary recovery work. Size the actual events: revoke is 21 bytes, unknown and
abort are 29 bytes, reconcile is 30 bytes. Even recovery ultimately refuses when
the finite total is full. This is not physical disk preallocation or a promise
of successful I/O, infinite recovery, power-loss resilience or production safety.

The original single/grouped append and both synchronous/cooperative replay paths
use one event-capacity check before their original Rights transition. Global
frame/lifetime bounds still precede payload allocation during replay. A valid
checksum or matching anchor cannot smuggle ordinary work into the reserved tail.
Grouped admission remains all-or-nothing in memory; complete unacknowledged disk
prefixes still retain their original liabilities under strict suffix recovery.

`capacity()` reports remaining logical space. It does not promise that the next
chosen transition is lawful, and does not expose a mutable ledger or reserve.

## Source validation status

Nine authored public regressions cover ordinary exhaustion followed by actual
revocation and settlement, exact/one-under byte tails, all 256 group partitions
of a nine-transition history, three replay schedules, invalid mixed groups,
checksummed illegal ordinary tails, changed profiles, every truncated prefix,
complete unknown suffixes, recovery-only profiles and an independently generated
v2 header vector. Legacy bytes and event encodings have paired controls.

The editing environment has no RCH launcher or Rust toolchain. Rust compilation,
these tests, rustfmt and Clippy are unexecuted. Whitespace/hash and independent
wire-vector checks do not qualify the Rust implementation. No bead is closed,
no original gate is weakened and no prior test receipt validates new source.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal --test recovery_reserve
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Original locked file owner

`FileJournal::create_with_recovery_reserve` writes the complete v2 header in its
first image using the original exclusive create and file/directory synchronization.
`begin_open_with_recovery_reserve` and `open_with_recovery_reserve` require the
same independent profile, use the original locked recovery cursor, and expose
no writer until strict replay and both synchronization barriers complete.
Legacy file entry points remain v1; there is no sniff-and-retry downgrade.

Single and grouped commits already consume the common event-admission law.
No new persistence or authority path is added. `FileJournal::capacity()` refuses
on a poisoned owner, rather than offering stale free-space numbers after an
uncertain write. A recovery-record write failure can leave no bytes, a complete
prefix or a partial record. Complete prefixes consume their recorded space and
retain their original unknown liabilities; partial records refuse without repair.
The advisory lock stays held until drop, even after failure or Ready.

Seven additional public file regressions cover exact first-image identity,
ordinary and byte-cap exhaustion followed by positive revocation/settlement,
reopening after each tail record, tiny-poll recovery, profile downgrade/widening,
header/Ready cancellation, invalid mixed groups, anchor rollback and the original
revoked-epoch rule paired with a valid new-epoch dispatch. Two private regressions
inject every one of the 51 byte cuts of a 50-byte recovery group and a full-write
unwind. The same poison, lock, independent-anchor and no-refund rules remain in
force. These nine additional Rust tests are also authored but UNEXECUTED; injected
failures are not power-loss experiments.
