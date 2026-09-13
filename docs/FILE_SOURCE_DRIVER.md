# Durable source observations in the supervised driver

## Connected behavior

The existing `FileSupervisedDriver` file APIs now feed an enabled `FileOversight`
file source through `refresh_file_source`, rather than reading evidence beside
its live gate. This includes `start_file_review`, `start_file_process_review`,
`step_from_file`, and `request_human_approval_from_file`. The same original driver
still controls review, authorization, publication and reconciliation; its internal
provider adapter is private and cannot be supplied by an actor or external
implementation. The public callback API remains unchanged.

Each configured read records the complete observation through the existing journal
and native leased state capture. Reconstructing a file reader no longer bypasses
the journal's producer generation, semantic epoch and exact-byte checks at these
boundaries. Equal successful rereads create new native observation events and
closures without revoking an unchanged review. They do not widen the fixed lease.
A read-start clock sample precedes the file operation; the original consumer clock
sample follows reading and persistence. Time spent in either consumes the lease.

The six capture boundaries in the full successful path are review launch,
completion of helper I/O, creation of a human request, both sides of automatic
authorization, and first publication. An independently configured host requires
its initial bootstrap observation before actor admission; the driver never enables
a source implicitly. No worker polling pass that has not completed review performs
an extra file read. Reconciliation, expired or historically resolved endpoint
outcomes, and query-only recovery do not read or renew a source. Unconfigured
legacy file profiles retain their previous raw-read behavior and clock sequence.

Every actual configured reread consumes a new original capture event and journal
space, including unchanged bytes. Source-operation count is not latency, RSS or
physical I/O accounting. No budget, reserved recovery tail or generation limit is
increased by this integration.

## Exact review-launch predecessor

Before source I/O or a clock callback, launch checks the original request's stage,
caller-supplied input revision and complete helper roster. It holds the existing
owner exclusively through the capture. An admitted source change can withdraw
previous inputs and advance their revision. Only that capture's resulting revision
is passed to the original review start, which records the new actual helper inputs.
A stale caller is not silently rebased onto a newer input revision.

No helper is spawned or receives evidence until capture has succeeded. A failed
source operation cannot start a native review, and a completed capture does not
claim that later socket/process setup succeeded. The original start operation
samples time after capture and rechecks the leased source. At its expiry, the
observation remains historical but cannot start a review. A failed setup before
native begin does not consume the round; a later original process-start failure
retains its already-started round and all direct-child cleanup obligations.

Both sockets and executable helpers use the same launch preparation. Partial
spawn retains the successfully admitted observation identity in its existing
process-error wrapper, and the original driver owns the started children until
reaped. Neither a source failure nor a process exit becomes an Allow vote.

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

`FileSourceReviewError::DurableSource` preserves the same native source failures
at launch. Existing legacy-source, socket and process errors remain distinct.
Exhaustive matches over this enum must account for the new variant. No public
callback signature, authority operation, evidence-file schema or journal event
encoding was changed by these driver increments.

## Source scenarios and verification

The first increment adds seven public test functions using the existing socket
worker/client fixture and real files. They pair expired old observations with
successful fresh-read publication; compare unchanged generic-callback refusal
against real-file renewal; reconstruct a reader below the durable generation
floor; delete the file specifically after automatic authorization; retain both
read and withdrawal failures; persist post-dispatch source drift before sealing;
and compare permission immediately before versus at the read-start lease expiry.

The second increment adds six launch tests: changed source after an explicitly
recorded predecessor, stale caller rejected before I/O/time, global source
withdrawal on missing file, fresh-reader rollback, lease expiry before native
start, and the native capture quota. Two additional process scenarios plus one
inert subprocess-entry fixture exercise actual inherited sockets: source capture
before spawning, complete two-key publication across expired earlier leases,
per-helper context isolation, source loss before launch, and partial spawn with
retained source identity/child ownership. There are sixteen new test functions
in total, including that entry fixture. The executable helper is explicitly
synthetic; it is not a trained model or independent harmfulness judgment.

2026-09-13: the targeted RCH invocation for the first test target and then the
combined invocation below both failed before compilation with
`rch: command not found`, exit 127. New Rust compilation, tests, formatting,
Clippy and revision-bound qualification remain unexecuted. No Beads or production
gate is closed. Source construction is not a passing runtime result.

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_driver_source_gate --test file_driver_source_launch --test file_source_process_launch
```

This remains the synchronous Unix reference profile. Whole-history replay/write
cost, native capture and journal limits, operator clock/producer fidelity,
plaintext storage and absent independent anti-rollback/authentication remain
unchanged. These source tests do not establish trained-helper reliability,
production throughput, wall-clock bounds or hostile-host protection.
