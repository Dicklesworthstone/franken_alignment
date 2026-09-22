# Exact-history guarded recovery

## Status

Unqualified source additions, 2026-09-22. Fifteen Rust regression tests are authored
but UNEXECUTED: eight initial real-file recovery tests, four archive-framing tests
and three real-file custodian/advancement tests. Both targeted RCH attempts failed
before execution because `rch` is unavailable (exit 127). The complete target is:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference guarded::anchored
```

No Rust, rustfmt, Clippy, performance or production qualification follows from
these changes. Existing execution receipts are historical. No bead is closed.

## Capability and boundary

`FileOversight::history_anchor` captures the healthy owner's acknowledged canonical
journal. `open_guarded_anchored` requires that exact prefix before replaying recovery.
The independently held `FileRecoveryRequirements` still pins the full gate inventory,
effective policy, credential epoch and counter floors. The anchor never replaces
these requirements or confers an effect key.

The existing counter floors cannot distinguish different valid histories having
the same counters. This API additionally compares the exact original canonical
encoding of the retained prefix, including profile and storage path identity.
Re-encoding the prefix rather than comparing whole successor files permits genuine
append-only progress despite the changed event-count header. A matching prefix does
not excuse an invalid suffix: all native transition and guard checks still run.
The same exclusive Store and decoded event sequence feed comparison and replay.
Failure occurs before cleanup, a recovery write, or role provisioning. Success uses
the original fence and withdraws saved approvals, clock and source eligibility.

The original append-only journal format is unchanged. No dependency, custom digest,
cryptographic proof, copied authority, new event or actor-facing command is added.
This is the base composed guarded recovery profile; more specialized evaluated,
predictive and mediated configurations remain explicitly rejected rather than
silently downgraded. The journal remains the sole actual publication sink.

The anchor must be retained outside the rollbackable journal by the trusted
operator. A stale or attacker-replaced anchor does not protect newer history.
The unanchored suffix and the original observation authenticity remain separate
trust assumptions. Anchors contain sensitive original journal data; Debug is
redacted, and they are not actor-visible artifacts. This is neither hardware
anti-rollback nor a signature/authenticated transcript nor remote-provider recovery.
It serves the plan's effect-history/recovery and non-rewindable authority contracts
(FA-014, FA-INV-005/006/014), without closing those broader packets.

## Independent custodian transport and advancement

`FileHistoryAnchor::write_to` exports to a borrowed writer. `read_trusted` imports
from a finite, independently trusted archive; parsing alone does not validate or
authenticate the embedded journal. Recovery still compares every original byte.
The caller supplies an explicit total size bound, no larger than
`MAX_HISTORY_ANCHOR_BYTES` (the original 16 MiB journal bound plus a 24-byte header).

The version-one header is eight domain bytes `FAHANC` followed by zero and one,
then little-endian u64 revision and canonical-payload length. The exact original
canonical journal bytes follow. There are no optional fields or trailing bytes.
Revision is bounded by the original journal event cap. Payload length and the
caller budget are checked before allocating the payload. The reader performs one
extra byte probe for EOF and propagates I/O faults, including truncated input.
Oversized declarations, unknown domain/version, and trailing data refuse.

The writer preflights the full output before writing and uses no second whole-frame
buffer. An I/O failure can leave partial output and never acknowledges an anchor.
Flush, file sync, directory sync, atomic replacement, permissions and custody are
explicitly the caller's responsibility. This API does not invent an independent
trusted filesystem or claim that success from Write implies durable storage.
Blocking-reader deadlines likewise belong to the caller.

`FileOversight::history_anchor_after` checks the previously retained prefix before
returning a newer acknowledged anchor. Equal-cut calls are idempotent. A foreign or
divergent owner, including one intentionally opened through a weaker existing
recovery path, cannot replace that prefix. The method writes neither the journal
nor the custodian's storage. Retain the old anchor until the custodian has durably
acknowledged the new one; failure is not permission to erase the old requirement.

## Regression coverage

Real canonical-file tests pair exact and appended-history recovery with a distinct,
semantically valid equal-counter fork; that fork first passes the old guard/floor
checks. Additional controls cover rollback below the anchor, foreign storage
identity, independent guard and higher-floor rejection, invalid suffixes, every
original fence storage barrier, and a poisoned owner refusing to anchor old RAM.
Successful recovery never restores a saved clock observation as current evidence.

Transport tests use independently written manual header bytes, exact/one-under
budgets, oversized count/length declarations, every truncation of a small frame,
trailing data, fragmented/interrupted reads and a failed writer with no implicit
flush/retry. Those tiny payloads are intentionally not real journals and receive
no recovery credit. Separate real-file tests export/import an actual owner anchor,
reopen without the old owner, reject changed inner bytes and a false revision,
and prevent advancement from a rolled-back owner. They are not fresh-process,
authentication, OS anti-rollback, or performance qualification results.
