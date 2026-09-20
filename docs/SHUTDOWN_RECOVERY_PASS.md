# Bounded recovery of the full registered roster

## Supervisor entrypoint

`FileShutdownCoordinator::recover_registered_pass(revision, clocks)` drives one
original `recover_and_drain` visit for every member of the coordinator's fixed
`FileShutdownPlan`. `FileShutdownClock` binds a domain ID, its registered clock
domain, and a trusted operator-supplied elapsed tick. The method returns an
immutable `FileShutdownRecoveryPass` with every member's outcome, its operation,
and initial/final acknowledged coordinator revisions.

This is the operator-side consumer of the existing two-key recovery path. It does
not introduce a second broker, effect ledger, endpoint or importer of saved rights.
The independently supplied profiles required by plan decoding remain mandatory;
the driver does not invent trusted profiles from journal/archive contents.

## Admission and progress

Before changing any coordinator or domain file, the driver checks the exact
predecessor revision, full roster, duplicate IDs, clock-domain bindings, total
remaining durable visits, bounded current-session attempts and revision overflow.
Caller order is irrelevant; native visits run in stable domain-ID order. A list
that omits an offline member refuses instead of changing the denominator.

Each visit is still the existing intent -> native recovery -> completion path.
Every native journal prefix and retained comparison head keeps its existing
byte/event budget, original source/identity/consistency restrictions and history
floor. No extra publication is dispatched and no unknown effect is refunded.
Already drained domains follow the existing canonical-read path, without another
native stop/fence/time event. Actual operation counts are bounded by the roster;
this is synchronous cooperative supervision, not an unattended worker.

An acknowledged native refusal (including a busy/missing domain or stale local
clock) is recorded as `Refused` and the pass continues to the next member. A
coordinator intent or completion failure is `Unacknowledged`: the domain might
already have changed, so the pass stops and later members remain `NotVisited`.
No error becomes a nonexecution receipt or a successful drain.

The outer `Result` distinguishes admission from execution: an outer error means
no pass began; after it begins, the returned pass includes individual successes,
refusals and any unacknowledged write. `all_observed_stopped()` and
`all_observed_drained()` inspect only this pass's original observations. Earlier
session successes cannot make an unvisited or failed member look complete.
These predicates describe separate local cuts, not a simultaneous fleet snapshot.

## Restart and finite budgets

Existing per-member intents and completions remain the durable resume evidence.
The driver does not create a second pass journal or restore current success from
old bytes. Reopen the original coordinator after an unacknowledged write. A
retained shutdown visit whose native stop exists can be resolved or refreshed
through [resolve_shutdown_visit](SHUTDOWN_VISIT_RESOLUTION.md), including when its
last allowed visit was spent. That evidence-only operation never retries a command.

The pass does not reserve filesystem capacity, enlarge byte ceilings, steal live
locks, kill processes or solve a journal that is full without a recovery reserve.
A coordinator byte-limit failure can still follow a successful domain shutdown;
it is reported as unacknowledged with remaining members unvisited. No distributed
atomicity or complete mediation of remote/OS effects is claimed.

## Implementation and verification status

Added the driver, exact clock binding, immutable per-pass reports and eight Rust
regression tests. Together with visit resolution this increment has fifteen new
unit tests plus one compile-fail capability-boundary example. Shared test setup
uses the existing native profile and actual canonical files, not synthetic member
outcomes or helper inference.

Cases cover multiple clock domains and unsorted input; exact visit capacity;
busy/missing members followed by a reachable member; malformed roster/clock
bindings; all five intent-storage barriers; a completed native shutdown followed
by a coordinator byte-limit failure; stale per-domain clocks; fresh confirmation
at the visit ceiling after restart; and refusal to reuse an earlier pass's success.

The required RCH gate cannot start here (`rch: command not found`, exit 127), and
Rust/Cargo/rustfmt are absent. These Rust tests, the compile-fail example,
compilation, formatting and Clippy remain **unexecuted**. Source hashes and local
lexical/delimiter/whitespace/patch-application checks do not qualify runtime behavior.
No dependency, domain/coordinator wire format, production gate or Bead closure is
changed by this work.
