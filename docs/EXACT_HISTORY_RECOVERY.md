# Exact-history guarded recovery

## Status

Unqualified source addition, 2026-09-22. Eight Rust regression tests are authored
but UNEXECUTED. The targeted command `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test
--locked -p fa-reference guarded::anchored::tests` failed before execution because
`rch` is unavailable (exit 127). No Rust, rustfmt, Clippy, performance or production
qualification follows from this change. Existing execution receipts are historical.

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

The original append-only format is unchanged. No dependency, custom digest,
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

## Regression coverage

Real canonical-file tests pair exact and appended-history recovery with a distinct,
semantically valid equal-counter fork; that fork first passes the old guard/floor
checks. Additional controls cover rollback below the anchor, foreign storage
identity, independent guard and higher-floor rejection, invalid suffixes, every
original fence storage barrier, and a poisoned owner refusing to anchor old RAM.
Successful recovery never restores a saved clock observation as current evidence.
