# Cold actor intake from the registered durable source

Consumer: the existing durable actor supervisor and FileSupervisedDriver with an
explicitly enabled FileSourcePolicy. Review, approval and publication already
refresh that registered source through the existing file-provider adapter. This
increment completes the earlier boundary: obtaining the actor gateway's one-use
admission snapshot without an independently supplied cached Snapshot.

prepare_file_intake calls the original refresh_file_source, retains its complete
FileSourceError result in the existing FileEvidenceReport, and checks the original
capture lease at a fresh post-read tick. The acquisition tick precedes file I/O;
reading and persistence consume the fixed lease. A source read can commit while
intake preparation later refuses. Only acknowledged, still-eligible source data
reach the original set_snapshot operation. No action is proposed or approved.

The previous slot is cleared through host_mut BEFORE configuration/stop checks,
clock callbacks or I/O. Missing files, incomplete observations, rollback, capture
quota, stale time and failed journal writes cannot leave an older slot eligible.
Read and withdrawal failure remain separate diagnostics. A failed journal operation
cannot supply a candidate observation identity. The owner stays exclusively
borrowed during acquisition and time checks. The final handoff uses the exact
current journal revision and the original bounded, one-use snapshot validator.

The operation exists on FileActorSupervisor<FileOversight> for transport hosts and
on FileSupervisedDriver, which also polls existing helper cleanup after every
result. No new source registration, format, journal tag, input builder, authority
reducer or actor protocol is introduced. Unconfigured callers explicitly refuse
rather than gaining a weaker raw-file fallback. Stopped/suspended runs refuse
before source I/O. The fixed source and journal capacity still apply.

This is a synchronous preparation primitive. The existing port consumes the slot
once. A later privileged host mutation clears it, and the supervising host must
observe time at subsequent scheduling boundaries. The primitive does not create
an independent watchdog or make a saved clock current forever. Existing request
retries, ticket polling and cancellation keep their original semantics.

Seven regression functions cover cold entry through actual file reads, native
socket helpers, independent human approval and guarded publication; one-use
consumption; missing/incomplete replacement; exact lease expiry; producer rollback
through a fresh reader; compound read/withdrawal failure; absent configuration and
terminal stopping; capture quota and invalidation through host mutation. Permitted
controls accompany refusals. One compile-fail example rejects conversion of an
intake report into a FilePermit. Helper responses remain synthetic fixtures.

Rust compilation, tests, doctests, formatting and Clippy have NOT executed. The
required RCH command failed to start because rch is unavailable (exit 127). This
source is pending qualification; no Beads or production gate is closed. Existing
source/authority reducers and prior test bodies are preserved. The operator-owned
synchronous Unix storage, clock and producer-trust limits remain unchanged.
