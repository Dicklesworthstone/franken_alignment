# Policy changes during durable supervised execution

Consumer: FileSupervisedDriver coordinating the existing actor gateway, helper
workers, mandatory independent human key and publication owner. The driver now
exposes the same replace_policy operation as that owner, with no new journal
format, policy authority, control loop or actor-visible vocabulary.

## Govern the owner; derive cleanup from its live ledger

The operation checks any active job's original owner brand, then performs the
original durable policy update. It invokes existing helper maintenance after
success AND failure. A committed update cancels undispatched work in the original
ledger, so its sockets are released and owned child processes are requested to
stop before later fallible clock/provider calls. A stale update does not cancel a
healthy job. An ambiguous storage failure retires workers but returns no candidate
policy receipt or refund. Actual child exit still requires bounded reaping.

The driver retains its existing Stopped event for the cancelled job. It does not
rerun helpers, substitute saved inputs, or silently promote the new policy to
approval. The actor port continues projecting original durable request outcomes,
retaining exact keys and tombstones. Fresh admission requires a new key, a new
explicit supervisor snapshot, the current epoch and the original review path.

An exact retry of an OLD update can list OLD cancelled attempts. Those historical
receipt fields do not instruct the driver to cancel whatever job is active now.
Cleanup always reads the original current ledger. A newer review remains usable
when an operator retries an earlier update to recover its receipt.

Already dispatched work preserves its publication/reconciliation phase, original
human expiry and charged rights. In the legacy unguarded profile, no provider or
human role is reacquired for that admitted effect. The optional first-publication
guard, when configured before proposals, remains independently enforced. Its
concurrently implemented driver path takes a fresh input before first execution
and can seal an old-policy request instead of publishing. Historical execution
receipts win over later policy changes. Neither profile refunds a dispatched
charge merely because policy changed; the original endpoint outcome must first
be acknowledged. Use the independent stop/fence path to withdraw pending endpoint
execution. See FILE_SUPERVISED_PUBLICATION.md for the guarded driver's contract.

## Causal and failure scenarios

Five driver scenarios exercise interrupted socket review followed by fresh-epoch
publication; a retained reservation and separately approved human key; policy
change after unguarded dispatch; a refused update followed by successful original
review; and an exact historical retry while a newer review remains active.

The process scenario uses the actual test executable, inherited helper sockets
and original HelperClient. The parent waits for atomically written PID readiness
AFTER each worker receives its exact original input. In one arm a stale update
leaves those same children alive; releasing their synthetic verdicts completes
review and co-signed publication. In the other, a committed update requests their
termination, preserves the Stopped event without calling a clock/provider, and
requires those same PIDs to be reaped. No child exit is treated as a vote.

Two additional scenarios use the existing five storage barriers, not replacement
mock reducers. They distinguish errors before canonical rename from successful
rename followed by failed directory synchronization. In the latter case the update
and, where applicable, human-key withdrawal must recover together despite no
acknowledgment. The old faulted owner exposes neither a candidate receipt nor a
refund. Reopening resolves actual policy history while preserving charged effects;
exact retry cannot apply an acknowledged transition twice.

Two cross-feature scenarios exercise the retained publication guard under actual
policy replacement: old-policy first execution seals without a premature refund,
whereas an already executed effect remains executed and charged. Freshly reviewed
new-epoch work must still publish after an exact historical update retry. These
use the concurrently committed guard implementation unchanged; they do not add a
second publication path or claim authorship of that independent work.

Combined with the core policy increment: twenty-two scenarios across twenty-three
Rust test functions (one is the subprocess entry point), plus one compile-fail
example. These counts exclude the independently implemented guarded-driver tests.
All helper responses are explicit synthetic fixtures. None of this Rust has been
compiled, formatted or executed; the RCH command could not start because rch is
unavailable. Source and diff checks do not validate executable behavior.
Original semantic reducers, original assertions, dependencies, bootstrap bytes,
Beads statuses and historic execution evidence remain unchanged. Base journal
policy-update tag 14 is the explicit addition described in FILE_POLICY_UPDATES.md.

Storage is still operator-controlled and synchronous. These tests do not establish
power-loss survival, malicious rollback resistance, authenticated policy issuers,
independent watchdog operation or descendant/hostile-process containment.
