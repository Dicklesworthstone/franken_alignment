# Exact-history recovery of interrupted rights-journal batches

## Source status and scope

This advances the bounded journal recovery work under plan sections 16.4, 16.5,
16.9 and 18.3. The consumer is the existing `FileJournal` interrupted-group
inspection/resumption path. It remains rights accounting, not an effect adapter,
remote-receipt authenticator, deployment qualification or closure of FA-014.
No wire version, dependency, executor, authority reducer or limit is changed.

## Retain the original history and intent

Before attempting the original group, obtain `owner.history_anchor()` and retain
its immutable bytes independently alongside the exact ordered group events.
After an uncertain commit, drop the poisoned owner and use the original strict
anchored reopen, with independently supplied identity, lifetime limits and
explicit v2 recovery reserve where applicable. Torn records are not repaired.

`begin_inspect_batch_anchored(history, events, batch_limits)` consumes the same
healthy locked owner. Unlike the legacy `BatchAnchor`, the supplied `HistoryAnchor`
binds the complete pre-group bytes, not merely a sequence, extent and CRC-32.
Identity, lifetime limits, reserve and exact boundary are checked before file I/O.
The original cooperative replay compares bytes before mismatching data can enter
its decoder/reducer. The original whole-group preparation and stored-suffix/EOF
comparison follow under their existing independent polling budgets.

A successful `finish()` returns the original owner and `BatchStatus`, without
writing or synchronizing new accounting. It does not complete missing records.
An ordinary read-only inspection cannot later be upgraded through `resume()`.

## Explicitly complete only the missing suffix

`begin_resume_batch_anchored(history, events, batch_limits)` selects resumption
before inspection. After `Ready`, consume it with `resume()` to transfer the same
locked descriptor, original candidate and already encoded missing bytes into
`FileBatchCommit`. No intermediate owner, repeated prefix replay, re-encoding or
interleaved group is permitted by this handoff. A nonempty suffix uses the original
bounded writes and final synchronization. A fully acknowledged complete group
requires no new write or synchronization. Cancellation never rolls back bytes,
reissues an external operation or refunds an unknown obligation.

The original head-only APIs are unchanged and retain their explicitly weaker
accidental-corruption contract. Exact-history selection is explicit, not inferred
from a file, a journal header or a successful previous inspection. Taking a new
anchor after recovery is not a substitute for retaining the original pre-group
history and intent.

## Bounds and trust

The immutable prefix adds its encoded length to retained memory; clones share
storage. It is compared during the same budgeted prefix reads, without adding a
read pass to inspection. Anchor import and the preceding strict reopen remain
separate work. Original frame/group, event, read, prepare and compare bounds still
apply, including v2 terminal reserves. Logical bounds are not elapsed-time or
interruptible-I/O guarantees. Trusted directories, advisory locks and filesystem
synchronization remain assumptions; hostile same-size concurrent writes are not
covered. Independent retention does not authenticate an archive's producer or
freshness, and joint rollback of archive and journal is not detected.

## Regression mapping and change record

`crates/fa-journal/tests/anchored_batch_recovery.rs` contains eight authored
regressions. Positive controls cover every surviving group boundary for both
profiles at three quanta, original-writer byte/accounting parity, read-only
Unknown preservation, complete no-write retry and subsequent ordinary commits.
Causal negatives cover an actual equal-length CRC collision, cancellation and
exclusive locking at every inspection stage, independently changed v1 limits,
a bad unwritten final event, changed extent after Ready and poisoned owners.
The collision control explicitly demonstrates that legacy inspection admits the
other valid prefix before the exact-history path refuses it.

The required command was attempted:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal --test anchored_batch_recovery
```

It exited 127 before compilation because `rch` is absent. Rust compilation,
regression execution, rustfmt, Clippy and full qualification are not claimed.
Original assertions, wire encoders, reducers and writer loops are unchanged.
