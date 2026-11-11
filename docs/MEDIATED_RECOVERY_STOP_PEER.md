# Authenticated Stop during ordinary mediated recovery

## Native custody

On Linux, `FileMediatedRecovery::into_recovery_stop(operation)` transfers a
fully verified ordinary mediated recovery into the existing sealed
`FileRecoveryStop` transport target. It retains the original exclusive writer
lock, canonical image, native machine and independently selected requirements.
It exposes neither a live owner nor human, topology, prediction or evaluation
roles.

The same original cursor must reach `Ready`. Its complete guard inventory,
effective policy, initial and current topology, availability, credentials,
optional prediction/evaluation and external recovery floors are rechecked.
Explicit publication-profile selection remains bound when selected, and an
existing terminal recovery reserve is mandatory. The target cannot install a
reserve, repair an invalid history or substitute a weaker unmediated recovery.

The operation is nonzero and, for a previously stopped history, must match the
original Stop operation. Selection alone makes no write. The existing
`VerifiedReviewerSocket::into_recovery_stop_connection` consumes the sealed
target only after kernel peer admission and the complete original nonce,
audience, scope, clock-domain and operation-bound Stop request.

## Acknowledgment and uncertainty

The original `finish_admission_stop` consumer applies the same Stop/Fence pair
in one canonical replacement. It acquires no effect clock, issues no live role,
refreshes no source, sweeps no delivery and sends no effect. Only undispatched
reservations follow the native Stop cancellation rules; dispatched and Unknown
liabilities stay charged. Admission acknowledgment is distinct from endpoint
drain or remote process termination.

Partial replay, incompatible requirements, missing reserve, a changed canonical
image, a silent peer or an invalid request cannot yield a successful native
acknowledgment. Before application, dropping the target or connection releases
the original lock without changing canonical or staging bytes. The existing
connection retains an acknowledged native result before reply encoding and
delivery; a lost reply does not erase that result or permit another application.

An explicit exact-operation retry can confirm the original stopped history
without another event, clock or replacement. A different operation cannot take
over the old acknowledgment. Independent retained history remains necessary;
this protocol is not an anti-rollback authority.

For an independently retained topology-source checkpoint, the existing
`TopologyCheckpoint::after_admission_stop` constructs the terminal successor
from the exact matching history. Source expenditure and operation floors remain
unchanged. An arbitrary snapshot cannot replace that paired custody.

## Ordinary live Tower recovery

The Linux `serve-open` and `serve-open-checked` commands with
`--tower-generation`, `--guarded` and a `fa.mediated-submit/2` topology profile
now bind this independent Stop listener before historical loading and replay.
An explicit `--requests` schedule uses the first selected request's original
endpoint. Existing `stop-peer REVIEWER_PROFILE REQUEST` selects the same
authenticated protocol; it needs no private supervisor configuration.

The service checks for a verified peer at every cooperative replay boundary,
including Ready. Pending Stop consumes the original cursor through this sealed
path; otherwise the original live finalizer runs and the same control, gateway
and driver continue into current-source checks and actor service. Candidate
allowance, pending peer and session timing are not renewed at handoff.
Synchronous I/O and native transitions remain cooperative, not preemptible.

After a native recovery acknowledgment, the service independently retains the
selected history anchor and exact terminal source checkpoint. A lost reply, late
deadline or one failed export cannot skip the other output or retry the native
Stop. An already-live Stop still refuses an obsolete saved source cut when it
lacks the separately bound source needed for capture; it retains acknowledged
history instead of inventing a source successor. See
[live recovery](LIVE_TOWER_RECOVERY.md) for commands and custody boundaries.

## Scope and qualification

This extends the existing recovery protocol to the ordinary mediated cursor;
it adds no wire tag, journal event, agent verb or dependency. It advances
FA-014/FA-073 and the existing FA-120 recovery acknowledgment path under plan
sections 8.5, 8.11, 15.2, 16.4 and 16.9, serving FI-A02/FI-A06. Full packet
acceptance, deployment mediation and runtime qualification remain open.

Six public regressions in `crates/fa-reference/tests/mediated_recovery_stop_peer.rs`
cover authenticated application, incomplete and mismatched recovery, reserve and
publication requirements, malformed and silent requests, canonical replacement,
and lost replies with exact retry. Independent source review and an initial
installed-rustfmt syntax pass completed before the editing workspace became
unavailable. After restoration, all eleven changed Rust files passed fresh
syntax processing; both new test files were formatted and passed focused
format checks, and `git diff --check` passed. Required RCH test attempts before
and after workspace restoration exited 127 because `rch` is absent; compilation,
runtime tests, Clippy and the full qualification gate remain unexecuted.

Four further real-socket regressions in the live service cover plain, checked
and scheduled recovery, malformed peers, independent requirement mismatches and
post-acknowledgment failures of transport, time and either custody export. Those
integration sources were reconstructed after workspace loss, independently
reviewed and syntax-checked after restoration. This establishes no compiler or
runtime pass; the actual checks and runner failure are retained in
`artifacts/execution/2026-11-11-mediated-stop-integration-verification.log`.

The original contracts remain in [cooperative mediated recovery](COOPERATIVE_MEDIATED_RECOVERY.md),
[recovery Stop transport](LEARNED_RECOVERY_STOP_PEER.md) and
[topology checkpoint custody](TOPOLOGY_WORK_CHECKPOINT.md).
