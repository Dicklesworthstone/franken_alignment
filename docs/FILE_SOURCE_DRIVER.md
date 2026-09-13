# Durable source observations in the supervised driver

## Connected behavior

The existing `FileSupervisedDriver::step_from_file` and
`request_human_approval_from_file` now feed an enabled `FileOversight` file source
through `refresh_file_source`, rather than reading evidence beside its live gate.
The same original driver still controls review, authorization, publication and
reconciliation; its internal provider adapter is private and cannot be supplied by
an actor or external implementation. The public callback API remains unchanged.

Each configured read records the complete observation through the existing journal
and native leased state capture. Reconstructing a file reader no longer bypasses
the journal's producer generation, semantic epoch and exact-byte checks at these
boundaries. Equal successful rereads create new native observation events and
closures without revoking an unchanged review. They do not widen the fixed lease.
A read-start clock sample precedes the file operation; the original consumer clock
sample follows reading and persistence. Time spent in either consumes the lease.

The actual boundaries are completion of helper I/O, creation of a human request,
both sides of automatic authorization, and first publication. No worker polling
pass that has not completed review performs an extra file read. Reconciliation,
expired or historically resolved endpoint outcomes, and query-only recovery do
not read or renew a source. Independently bootstrapped legacy file profiles retain
the previous raw-read behavior and callback clock sequence.

## Refusals versus persistence errors

A committed source refusal is an unavailable observation, never the last good
snapshot. It can feed the original restrictive review or endpoint sealing path.
Source withdrawal affects the original shared gate and all dependent inputs and
human keys; it does not cancel or refund effect obligations.

A failed journal operation is an outer failure. It cannot become an ordinary
missing helper vote, permitting snapshot, candidate endpoint receipt or refund.
The unchanged first-publication path retires its send phase before the provider
runs, so a failure there leaves only reconciliation, not an automatic reread and
resend. A successful source observation can still be rejected by later action,
input, authority, human-key or expiry checks.

`FileEvidenceReport` retains its `observations` field and adds `source_updates`:
one native source-operation result per configured capture attempt, empty for
legacy profiles. Successful entries identify the admitted observation, not an
effect. Errors preserve `FileSourceError`, including both an I/O read failure and
a failed withdrawal. A journal failure cannot invent an observation identity;
its full error lives in `source_updates` and the outer operation result. Native
preflight failure can occur before a physical read, so source-update count must
not be mislabeled as the reader's I/O count. Construction or exhaustive
struct-destructuring of this report must account for the new public field.

## Scope and verification

The first increment adds seven public test functions using the existing socket
worker/client fixture and real files. They pair expired old observations with
successful fresh-read publication; compare unchanged generic-callback refusal
against real-file renewal; reconstruct a reader below the durable generation
floor; delete the file specifically after automatic authorization; retain both
read and withdrawal failures; persist post-dispatch source drift before sealing;
and compare permission immediately before versus at the read-start lease expiry.

Review-launch reads are still the existing raw-read path in this increment. The
subsequent launch integration must use the same native source operation without
silently adopting a concurrently changed caller predecessor. No source policy is
enabled automatically, and no callback is declared a trusted observation adapter.

2026-09-13: the targeted command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_driver_source_gate`
was attempted and failed before compilation with `rch: command not found`, exit
127. New Rust compilation, tests, formatting, Clippy and revision-bound
qualification remain unexecuted. No Beads or production gate is closed.

This remains the synchronous Unix reference profile. Whole-history replay/write
cost, native capture and journal limits, operator clock/producer fidelity,
plaintext storage and absent independent anti-rollback/authentication remain
unchanged. These source tests do not establish trained-helper reliability,
production throughput, wall-clock bounds or hostile-host protection.
