# Supervised publication heartbeat acquisition

`FileSupervisedDriver::step_with_publication_heartbeat` connects the original
supervised driver to both the concrete witness-file source and the concrete
change-feed heartbeat reader. `step_from_files_with_publication_heartbeat` also
uses the existing sealed policy/committee file reader and its native source lease.
The APIs live in `driver::evidence::publication::heartbeat`; their report type is
`FileHeartbeatDriverReport`. Configure the required publication/change/freshness
profiles and bind the attempt's original witness source before using this path.

This is acquisition in the SAME driver and evidence provider chain, not a second
scheduler, actor command, permission mechanism or runtime. Ordinary existing APIs
are unchanged; choosing an older API cannot disable the configured gate, but does
not automatically acquire its heartbeat. The new entry points are the paths that
automate these concrete acquisitions.

At each original evidence boundary the existing PublicationProvider first
withdraws witness eligibility durably. The heartbeat provider then commits feed
withdrawal, reads the heartbeat file, and samples the trusted clock after that
read. Actual committee/policy capture and witness-file acquisition follow. The
original authorize/dispatch/first-publication operation samples/checks time again.
A read-time heartbeat status can therefore be valid while the later effect check
correctly refuses it as expired. No read-time status is accepted as a permit.

A normal Ready step with no automatic permit performs two independent heartbeat
acquisitions: one before authorization and one before dispatch. First publication
performs another. Workers that need no evidence, Idle, reconciliation, and already
resolved/expired first publications retain the original no-provider paths and do
not require heartbeat or witness files. Human approval remains separately owned;
this change neither approves requests nor supplies a replacement second key.

Read-time results are recorded in `report.heartbeats`; witness reads and the
original committee-source diagnostics remain in `report.publication`. Failed
heartbeat persistence is an outer journal error, with no invented successful
observation. A file error or acknowledged expired heartbeat is independently
recorded, not mislabeled as changed committee evidence. The actual committee and
witness captures still run; mandatory native feed gating then refuses the effect.
Consequently a temporary feed outage need not invalidate the immutable committee
review or lose an unspent automatic/human key. A retry acquires real new evidence
and retains the same reservation rather than authorizing twice.

An outage or expiry discovered after dispatch seals through the original endpoint;
only original receipt reconciliation settles nonexecution. An already executed
receipt still wins and its charge stays spent. A caught clock unwind after a read
leaves the durable owner unavailable, and the existing publication phase has already
retired its send path. No candidate effect is acknowledged through an outer storage
or replay error.

The three-reader path reuses the original FileProvider, including policy-source
identity, producer floors, durable observations and time leases. A heartbeat does
not renew the independent policy source, and a policy-source read does not renew
the producer timestamp in a stalled heartbeat file. The observations are sequential,
not an atomic snapshot across producers. Producers, coverage, filesystem control
and the shared elapsed-clock interpretation remain trusted host assumptions. The
heartbeat file transports coverage/time only; actual change records still arrive
through the existing notification API.

At most two heartbeat and two witness acquisitions occur in one driver step; a
three-reader step also keeps the original at-most-two committee file reads. Each
successful heartbeat acquisition adds its original two journal events. Full-prefix
byte/event/recovery-reserve limits remain in force. No performance benchmark or
production source-authentication guarantee is claimed.

`tests/file_driver_publication_heartbeat.rs` adds eight real helper-socket driver
regressions: normal acquisitions, stalled feed with unchanged committee evidence,
second-acquisition loss and same-permit retry, expiry during committee capture,
post-dispatch loss, clock unwind, receipt/deadline reader skipping, and composition
with a separately leased native policy source. These tests and the earlier native
and durable heartbeat tests remain UNEXECUTED pending the repository's required
RCH verification on the exact integrated revision. The current environment has no
rch, cargo or rustc; no passing compilation/format/Clippy/test claim is made.
