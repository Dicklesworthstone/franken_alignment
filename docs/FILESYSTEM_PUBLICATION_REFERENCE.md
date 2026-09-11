# Filesystem-backed publication and endpoint recovery

## Implemented consumer

`PublicationEndpoint::create_file_publication` and `create_file_stream` select a Unix local-filesystem profile for the existing endpoint. The same `DeliveryBroker` and `OversightBroker` accept it without an alternate dispatch path or a mutable inner-controller accessor. Ordinary exact policy, complete helper-input review, original permits, optional human keys, streaming rules and dispatcher fences still apply. Bootstrap creates a NEW operator-owned directory, never an existing directory or an arbitrary overwrite target.

The publication is the payload obtained from `PublicationEndpoint::read_file_publication(directory)`. Its complete canonical representation is `publication.bin`: it contains the initial endpoint contract and the retained successful, non-idempotent endpoint operations. The temporary `publication.pending` file is NOT a published resource and must not be served to an audience. No separate raw-payload file can become inconsistent with its receipt record. All names are fixed inside the provisioned directory.

Each mutation runs the original endpoint operation against a private staged copy of endpoint data. It then writes the next bounded representation to a new staging file, synchronizes that file, renames it to the canonical publication, and synchronizes the directory. Only after those calls succeed does it return the original operation's receipt or fence acknowledgment. Initial directory creation also synchronizes its parent. A previously recorded terminal result or an identical clock/fence operation is idempotent and does not write another record or consume another mutation slot.

This persists endpoint data, NOT the action-authority ledger. Staging copies no actor, permit pool, production credential, policy controller or live budget. No Cargo dependency, unsafe code, foreign runtime or executor was added. The implementation is synchronous and Unix-only; it does not silently fall back to memory-only publication when filesystem setup fails.

## Failures cannot become nonexecution receipts

Storage failures return the existing `Error::Incomplete` at endpoint mutation boundaries and retain a typed `StorageFailure` in `file_storage_status`, including the I/O stage and error kind. Constructor/recovery errors use `FilePublicationError`. No write, synchronization or rename error fabricates a terminal nonexecution result or refunds the broker's original charge.

A directory-sync failure can follow a successful rename. In that case the endpoint retains the potentially observed payload and terminal history but returns NO receipt, and it refuses further delivery/status operations until recovery. The status's `publication_visible` flag records that rename returned success; a false flag is not a nonexecution proof. An I/O error or an inconsistent storage result may leave recovery unavailable. Missing or truncated canonical files never fall back to an empty endpoint or a staging file.

A missing status remains `AwaitingResolution`, not a refund. `seal_unexecuted` and `resolve_expired` use the original endpoint operation and persist the resulting terminal record before returning it. A seal survives reopening and prevents a delayed message from publishing. If execution already occurred, its execution receipt wins. An expired second key cannot cause a recorded execution to become nonexecution. Expiry resolution still requires the existing retention and current dispatcher-fence conditions.

The original broker remains independently responsible for recording lost acknowledgment as Unknown. Its cancellation rules do not refund an unknown effect. It can later accept a matching recovered endpoint receipt through its existing reconciliation operation. Dropping, resetting, changing policy, or replacing an endpoint object never resets that broker's effect accounting.

## Surviving-controller recovery, not whole-process recovery

Creation returns a separately held, nonclonable `FileEndpointRecovery`. It retains the original process-local endpoint binding, directory, immutable initial contract, storage limits, visible-history revision and latest admitted clock observation. The controller and this handle must survive; neither can be reconstructed from the publication file by this API.

`reopen` first acquires the directory's exclusive `File::try_lock` owner lock, reads and checks the bounded canonical file, and replays every operation through the existing memory-only endpoint semantics. It checks the exact initial contract, limits and surviving revision floor, synchronizes the recovered canonical file and directory, and removes an inert leftover regular staging file. It does not install a partial staging file as an outcome. A second owner cannot open concurrently under this cooperating local lock profile, and closing the owner releases the OS-managed lock rather than requiring deletion of a stale boolean lockfile.

Persisted attachment prevents the recovered endpoint from being attached to a newly funded controller. Recovered receipts carry the ORIGINAL endpoint binding and requests, including the consumed human key's issuance/expiry data. Read-only inspection without the recovery handle returns only a `FilePublicationSnapshot`, not a receipt accepted by a live broker or a dispatch envelope.

A reopened endpoint always requires an explicit CURRENT `observe_time` before delivery, status, sealing or expiry resolution. A saved timestamp is historical data, not a fresh clock reading. Even a valid clock observation whose persistence fails or reaches a storage limit advances the surviving clock floor and invalidates current-clock eligibility. Reopening an older file cannot forget that observation or retry at an earlier time. Fence installation remains independently available when its own persistence can succeed.

The clock remains a caller-trusted logical elapsed clock. This implementation does not sample a serving host's monotonic clock, establish tick duration, or certify real-time deadlines across a potentially slow filesystem call. Native clock/cancellation/latency qualification remains separate. Restoring the whole process or operating after losing the recovery handle needs the planned durable authority and authenticated recovery design; creating a new handle from arbitrary file bytes is intentionally not provided.

## Stream and human-key behavior

A stream publication retains the complete audience history, message boundaries, actual resource version and Finish state. Reopening cannot discard a disclosed prefix or reopen a finished stream. The broker's receipt-confirmed view can legitimately lag a published file while an acknowledgment is lost. No next chunk dispatches through the existing streaming path while that obligation is unresolved.

An actor reset preserves unknown disclosures and Finish obligations. Endpoint recovery does not restore old policy floors or human keys. A human key consumed by the original dispatch remains consumed even when the endpoint later seals that dispatch without execution and its automatic resource charge is legitimately refunded.

## Storage assumptions, format and cost

The directory and its ancestors must be controlled by the operator, outside actor write authority. New directories request mode 0700 and new files 0600. The profile rejects obvious symlink/nonregular canonical and lock entries, but its path-based checks do not establish security against a hostile parent, malicious filesystem, changing mounts, forged host state or a process that ignores the advisory lock. Payloads and retained earlier requests are plaintext; these modes are not encryption or an export authorization.

The version-one `FAPUBFS` format retains scope, target identities, original payload bytes, dispatcher/clock operations, retention boundaries and human approval metadata. It stores no policy witnesses, helper packets, actor caches or checkpoint data. Parsing bounds lengths and counts before replay; it rejects unsupported tags, malformed scope/target fields, truncation, trailing bytes, inconsistent revisions, illegal transition order, duplicate no-op history events and invalid approval windows. All replay operations must succeed under the original endpoint implementation. No alternate reducer or outcome callback is used.

The format is NOT a cryptographic integrity mechanism. The surviving handle detects an older history cut and a different initial contract, not every adversarial edit to a semantically valid same-length payload. Read-only inspection has no independent anti-rollback anchor. Authentication, secure namespaces, durable key management, power-loss tests, filesystem-specific guarantees and independent real actor bypass tests remain production obligations. Successful `sync_all` calls are recorded API outcomes, not a universal claim about hardware behavior or every local/network filesystem.

The fixed ceilings are 4,096 retained non-idempotent mutations and a 16 MiB canonical file, with smaller configured limits. Existing endpoint delivery and payload limits still apply. A full snapshot is rewritten for each mutation; endpoint staging copies retained endpoint data, and replay visits the retained history. This has write amplification and is intended for bounded disposable publication, not an optimized long-running store. The byte bound is encoded-file size, not peak memory, allocation traffic, CPU, syscall count or latency. Idempotent operations and read-only inspection do not replenish quotas. Exhaustion can prevent new writes and fresh clock publication; no stale-clock fallback is supplied. Already held broker receipts and cancellation of eligible undispatched reservations remain governed by their original APIs.

## Qualification and change record

Nine storage-unit tests cover each named pre-write/publication/synchronization failure point, a real create-new failure, owner locking, retained revision floors, every truncation of a selected complete file, replay order, quotas, source-clock nonresurrection, second-key expiry, path refusal and file permissions. The injected failures are source-level barrier tests, not actual power cuts.

Eight public integration tests construct actual policy reviews and permits through OversightBroker and pair real file publication with lost-ack recovery, exact-policy/helper holds, definitive sealing, dispatcher restart, human-key expiry, prior-execution preservation, actor reset with an unknown Finish, refusal of a newly funded controller and storage failure without an automatic refund. Fixture helpers and snapshots are explicit test inputs, not a claim of live inference or authenticated provider capture. One compile-fail doctest rejects cloning the recovery handle.

These seventeen Rust test functions and the doctest have NOT been compiled or executed in this session. Cargo/rustc and a configured RCH runner are absent from the active environment. Formatting, Clippy and the full revision-bound repository gate remain pending. No bead or production gate is closed, and no previous execution receipt qualifies these additions.

This is scoped filesystem/data-plane progress toward FA-006, FA-012 and FA-014, serving the existing effect/reconciliation contracts in plan 8.3, 8.4 and 16.9. It does not replace the required Asupersync/FrankenSQLite authority transactions, foundation admission, authenticated provider boundary or real whole-system recovery campaigns. Existing assertions and execution gates are unchanged.
