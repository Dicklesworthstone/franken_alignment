# Sequential actor service

Plan sections 8, 9.11, 14.9 and 17: repeated actor work through the existing
full-input, two-key authority, with bounded intake and retained obligations.

## Native handoff (2026-09-21)

`drive_peer_sequence_from_file` admits fresh evidence acquisition only for the
operator-selected current request. The bounded completed prefix is checked
against the original journal. Earlier exact/conflicting retries still compare
original bytes, without reading a source or sampling time. Future keys refuse
before intake. `drive_peer_sequence_observe` acquires no source and cannot admit
new work. Both use the original authenticated connection, tickets, codec and
lifetime transport limits; no authority or durable request is stored in a second
queue. The maximum schedule has 64 distinct nonzero keys.

`retire_completed_request` checks the original terminal disposition, retires the
matching local job and confirms direct-child reaping before another cohort can
start. Pending review, dispatched and unknown states refuse. It neither cancels
work nor reconciles an effect, resets a budget, restarts a dispatcher, reads
evidence or grants authority. False means keep the owner and poll cleanup again.

Six authored native socket/file regressions cover successive keys on one owner,
source-free historical retries, future-key refusal, nonterminal handoff refusal,
foreign/malformed schedules, unchanged connection accounting and exact/one-over
schedule bounds. Existing assertions and fixtures remain unchanged.

The native handoff landed first; the executable integration is described below.
The required RCH xtask attempt returned exit 127 because `rch` is unavailable.
Compilation, Rust tests, formatting and Clippy are UNEXECUTED. No production
qualification, performance claim or Bead closure is made. Authenticated Linux
peer credentials do not establish executable integrity or evaluator correctness.

## Integrated live service (2026-09-21)

The original `supervise_publication` executable now accepts a service-only,
explicit sequential schedule. Existing single-request invocations and the actor
wire/profile formats remain unchanged:

```text
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --requests 7,8,9
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --requests 7,8,9
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --requests 7,8,9
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --requests 7,8,9
```

The first key must equal the actor profile's `request`. Keys are nonzero, distinct
canonical decimal integers, bounded to 64. The schedule is in execution order,
not numeric order. Supply `--requests` after all positionals; the existing
`--credibility-activation EVIDENCE_FILE` option may also accompany open modes.
Actor, reviewer and stop socket collisions are checked for the whole schedule
before the publication store or actor socket is created. A rejected schedule
cannot silently fall back to a single request.

Each new request uses the same locked host, actor gateway and peer session.
Transport candidate/connection/exchange limits, total rights, journal capacity,
recovery reserve and the service's logical/wall deadline survive every handoff.
There is no per-request rebootstrap or renewal. The next request cannot start
until the previous original disposition is terminal and its direct helper
children are confirmed reaped. A missing source or exhausted qualification is
not replaced by the preceding request's evidence. An unresolved dispatched
request cannot be skipped or called cancelled to advance the schedule.

An actor submits one request, polls for its original terminal result, then sends
the next selected request using the same authenticated connection or an allowed
reconnect. It supplies the actual current target version and policy epoch; the
service never rewrites a stale proposal. Normal actor-submit clients may use
matching per-request actor profiles on the same socket. There is no pipelining:
an early future Submit is withheld rather than queued or admitted speculatively.
Earlier exact retries stay available during subsequent work and final bounded
reply grace. Conflicting retries retain the original byte-equality refusal.

Every publication runs the original concrete source acquisition, fresh isolated
helper round, independent human approval, dispatch and receipt reconciliation.
The checked mode additionally runs the original producer/witness guard on each
request's first publication. Only bounded immutable executable configurations
are cloned to launch the next fresh cohort; children, sockets, inputs, votes,
keys and permits are not cloned. Explicit actor cancellation can terminate one
request without terminating unrelated authority. Independent operator stop is
available while waiting for every new request and throughout its review and
publication checkpoints, using the same stop listener through intake/execution.
It remains cooperative, not a blocked-I/O preemption or hardware watchdog claim.

Opening a recorded schedule first applies the original recovery fence. Existing
keys are reconciliation/observation-only and do not read a source, launch helpers,
seek another human approval or open a supplied credibility capsule. The first
new request may perform the single explicitly supplied activation. Every later
new request checks its current qualification; no renewal or fallback is implicit.
Recorded effects remain spent and historical retries cannot become new effects.

Three option-parser tests and eight additional real socket/file/child-helper
regressions are authored for this integration. They cover two publications on
one connection; cancellation followed by publication; cumulative-budget refusal;
complete historical retries with unavailable evidence; all-schedule preflight;
independent stop before the second admission; second-source loss; and paired
unchanged/changed checked input after the second real durable dispatch. The
last pair preserves the first publication's spent units. Earlier regression
bodies and all original qualification/stop/checked-publication guards are retained.

The required RCH example-test and full xtask invocations could not start because
`rch` is unavailable (exit 127); cargo, rustc and rustfmt are also absent.
Compilation, Rust test execution, formatting and Clippy remain UNEXECUTED. These
are implemented source paths and authored regressions, not a qualified release,
measured throughput, model-performance result or closed roadmap packet.

## Executed sequence boundary (2026-10-24)

The original 18 service, parser and client entries now execute together with
seven peer-credential/readiness entries: **25 passed, zero failed/ignored,
707 filtered** on qualified standard RCH worker `vmi1149989`, default stack,
at `e5b141b0` plus four-path overlay
`517572c0fd12069b86fc3626083ac9c15f2ebdf0c376410d33a785536312f02b`.
These Rust bytes are committed in `d218b0b5`. The original 18-entry baseline
was ten passed/eight failed; the intermediate attempt was sixteen passed/two
failed. Both failed attempts are retained. This later execution does not erase
the original exit-127 history above.

The fixtures now poll the native intermediate `Unknown(OutcomeUnknown)` within
their original deadline, fund their complete payloads while retaining seven-unit
charges, and wait for actual reviewer listen readiness before checking the same
kernel peer and audience. The test-only wait retries only `NotFound` and
`ConnectionRefused`; it neither retries credential failures nor removes sockets.
A stale-socket timeout preserves the original inode beside a permitted peer.

Plan §7.3 reserves effect rights at **Authorize**, after review, rather than at
proposal admission. The cumulative-budget test therefore completes both reviews:
totals ten and thirteen permit only the first seven-unit effect and cancel the
second before dispatch; total fourteen permits both. The original ten-unit case
retains its exact seven charged, three available and zero reserved assertions.
The test checks the actual authorization error and terminal journal disposition;
its former `NotAdmitted` expectation was incorrect and has been replaced.
No production reservation rule, bound, wire format or deadline changed.

This is bounded integration-test repair, not a new runtime feature or full
qualification. The canonical driver still refuses the initial dependency guard;
complete operator, dependency and Clippy qualification remain open. See
`IMPLEMENTATION_STATUS.md` for the current execution inventory.
