# Publication change-feed heartbeats (FA-061 / FA-062)

## Required profile and original consumer

A contiguous but stalled feed is not evidence of current coverage. The optional
`PublicationFreshnessPolicy` adds a named elapsed-clock domain and a fixed positive
maximum age to the ORIGINAL publication change gate. Authorization, ordinary and
two-key dispatch, and durable first publication all consume that same check.
No heartbeat, copied status, or notification grants authority or skips the original
structured/opaque final validation. Unconfigured legacy profiles retain their
previous explicit lack of a freshness lease.

`FileOversight::create_with_publication_change_freshness(directory, profile,
validation, changes, freshness)` installs all three profiles in the FIRST canonical
image. No heartbeat is installed and no effect is eligible merely because creation
succeeded. Individual enable operations are also available before proposals for
existing bootstrap workflows. There is no disable, clock replacement, age increase,
producer reset or late upgrade of an already proposed attempt.

`open_with_publication_change_freshness` takes the same independently expected
profiles. It checks validation limits, change source/initial sequence/lookup budget,
and clock/age BEFORE replay, cleanup or recovery writes. Ordinary open also replays
the stored lease; omitting the extra pin does not disable it. The freshness clock
must equal the original FileDeliveryProfile clock domain on both live admission
and canonical encoding/decoding.

## Producer contract and acquisition

`PublicationHeartbeat` carries source, clock domain, generation, complete-through
sequence and `produced_at: ElapsedTick`. The producer updates its generation whenever
any field changes, emits a new immutable file, and atomically renames it over the
configured path. Its observation time must be in the independently agreed elapsed
clock domain, not a filesystem modification time or a consumer's reread time.
`to_bytes`/`from_bytes` use the fixed 48-byte, versioned `FAPHBT01` format on Unix.

`PublicationHeartbeatFile`, in `observed::publication::capture::heartbeat`, has a
fixed path and source, with no reader override or positive cache. Every acquisition
reopens the file and checks regular-file type, size, and detected path/handle
identity/metadata changes. Symlink leaves, truncated/overlong input, wrong versions
and foreign sources refuse. Paths are at most 4,096 bytes and reads are limited to
48 bytes plus one sentinel. These checks are NOT an adversarial filesystem sandbox;
the producer and its directories remain operator-owned.

Call `refresh_publication_heartbeat(revision, &reader, clock)` to first commit feed
unavailability in the original journal, then read, sample trusted time AFTER the
read, and acknowledge the new observation. There is no public durable API accepting
a caller-cached positive heartbeat in place of this actual read. The native
in-memory broker still has its explicitly trusted observation interface.

`Ok(Ok(status))` means the observation was acknowledged, not that it permits work:
inspect `status.eligibility`. A stale/future/mismatched/incomplete observation is
retained restrictively. `Ok(Err(file_error))` means the read failed after the durable
withdrawal. An outer JournalError means no candidate installation was acknowledged.
After a successful read, clock unwind, invalid clock movement, allocation, replay,
capacity or storage failure leaves the live owner unavailable pending recovery.

## Time, generation and missing-tail rules

Eligibility requires `produced_at <= current_tick < produced_at + max_age_ticks`,
with checked addition. Re-reading an unchanged generation can never move that
expiry. A future observation cannot become eligible merely as time passes: it
must be acquired again. Generation, observation-time and producer-coverage floors
cannot regress. Same-generation different contents create a retained conflict;
a later copy of the old quiet file cannot rehabilitate that generation. A newer,
monotonic producer observation is required.

The heartbeat must match the exact contiguous change prefix. Seeing a producer
head beyond the local prefix records the higher observed frontier WITHOUT filling
missing records. Original contiguous change notifications must repair the tail;
those repairs retain their all-observation withdrawals, including at final repair.
Repair alone cannot activate a previously refused heartbeat. Notifications that
advance the feed also require a heartbeat for that newer complete prefix.

This file contains a heartbeat, not the missing notification records. The host
still delivers actual changes through `record_publication_change`. Counters and
clock labels do not authenticate that those records cover every real world change.
Heartbeat reads and witness/policy reads are sequential, not a cross-source atomic
snapshot. Existing source, credential, identity, policy and effect-deadline checks
remain independent requirements.

## Rights, recovery and bounds

Feed unavailability does not spend either approval key or refund a reservation.
An expiry before dispatch refuses the original dispatch. An expiry first noticed
after dispatch seals through the original endpoint; only its receipt reconciliation
settles nonexecution. An already executed/sealed outcome and original execution
deadline precede new feed checks. Feed loss never changes an execution into a refund.

Acquisition is bound to the ORIGINAL dispatcher epoch. Recovery/fencing invalidates
a saved heartbeat even inside its time window, without inventing another epoch or
rights ledger. Producer floors/conflicts and missing tails replay, but old sendable
keys are withdrawn by the existing recovery path. A genuine new read can establish
current coverage only while the producer's ORIGINAL expiry still permits it.

The native freshness state is constant-size. Each successful durable refresh adds
two original journal events (withdrawal and observation); failures may retain only
the withdrawal. Original event/byte and recovery-reserve limits apply to every
prefix. The profile uses subtag 7 within existing publication event 30; earlier
encodings are unchanged and older readers reject the new subtag. Inspection and
replay work remain bounded by the enclosing history limits, not by the witness
comparison budget. No CPU-time, allocator/RSS or production throughput claim is made.

## Verification status

Eight native unit tests cover lease boundaries, generation conflict, future ticks,
missing-tail repair, epoch changes and overflow. Eleven real-file/committee/two-key
tests cover authorization, stalled-file dispatch, post-dispatch sealing, recovery,
failed reads/installation, caught clock unwind, packet limits and receipt precedence.
Three additional file tests cover atomic bootstrap and independently pinned reopen.
These Rust tests remain UNEXECUTED in this editing environment. The required
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` could not start
because rch is absent (exit 127); cargo/rustc are absent too. Compiler, format, Clippy
and runtime verification are still required on the exact integrated revision.
FA-061/062 remain open; no production activation or authenticated-source claim.
