# Supervised execution over the original durable two-key authority

Consumer: the trusted host integrating durable actor requests with real helper
sockets, independently issued human approval, and the original publication file.
This joins FILE_OVERSIGHT_ACTOR.md and FILE_HELPER_TRANSPORT_REFERENCE.md at the
plan's effect/control integration boundary (sections 8, 9 and 17; FA-049/107).
No authority reducer, journal format, effect sink, runtime or dependency is added.

## One owner, distinct transitions

FileOversight::into_supervised_driver moves the original exclusively locked host
into the existing FileActorSupervisor and returns its restricted port plus a
FileSupervisedDriver. The independently returned FileHumanReviewer stays outside
the driver. The original wire/channel/Unix transport can use the port unchanged.
An already-created actor gateway can instead move its supervisor into the driver.

The host explicitly selects a durably admitted request and supplies its exact
inputs, roster, round, deadlines and current policy snapshot. start_review records
those inputs and starts the original leased FileHelperPool. There is one active
review, because the original congress binds a common control predecessor. A failed
later setup can leave committed input/clock observations; it is not rollback.
Invalid roster/request/expected-input preflight cannot start helper I/O.

step_with_evidence executes one of four stages: helper review; authorization and
durable dispatch; publication; or acknowledgment through original reconciliation.
A completed Continue review cannot publish in that same call. Waiting for the
human key reserves no resources. A supplied key still goes through the original
issuer/attempt/expiry checks; no driver method approves a human request.

The driver retains the original automatic permit after a pre-dispatch refusal.
A later explicit independent human key can use that still-valid reservation;
authorization is not rerun to allocate another one. Source withdrawal or a changed
control predecessor cannot repair the old approval or silently rebase its permit.
Adverse or refused completed reviews leave no automatic re-review job. The operator
must explicitly choose any subsequent work; the driver never hunts for an Allow.

## Evidence and time

The same DriverEvidence value used by the in-memory driver is supplied by a
trusted observation callback. Review completion samples it AFTER helper I/O;
authorization samples it before reservation; final dispatch samples it AGAIN after
reservation. Every callback is followed by a new supervising clock sample. Equal
already-confirmed ticks do not add gratuitous clock records or extend deadlines.

A missing, incomplete, malformed or changed helper input withdraws the original
input eligibility through its durable transition. It never selects the saved input
as a substitute. Restrictive completed reviews may still apply with unavailable
current evidence; permissive ones cannot. Policy predicates and witnesses are still
checked by the original broker. Provider scope/authentication and coverage of the
outside world remain host assumptions, not guarantees supplied by a callback.

Publication and outcome reconciliation do not call the provider, rerun helpers or
require the human role. The exact dispatched envelope is already owned by the
original host. Before attempting publication the driver retires its publish phase.
Any failure goes to reconciliation, not another publish call, including when a
caller catches an unwind. Storage ambiguity keeps the original owner faulted.
A successful publication is still Unknown to the actor until its acknowledgment
passes the original reconciler. Executed effects remain charged; no local phase
can assert nonexecution or refund a charge.

The driver checks original request dispositions before fallible clock/provider
work. Actor cancellation or external fencing cannot leave an abandoned review
running. An externally dispatched request is reconciled rather than resent.
Local phase and scheduling hints are supervisor diagnostics, not actor Knowledge
values, credentials or persisted permissions. An idle step performs no I/O.

## Reference and verification boundary

Eight integration scenarios cover actual actor-wire admission, complete socket
reviews, mandatory independent approval, staged durable publication, redacted
acknowledgment, adverse-review termination, restrictive-versus-permissive source
loss, changes and failures after reservation, expiry followed by a new independent
key, foreign-key rejection and invalid-roster admission with a positive control.
The original decoder, helper, human, journal, gateway and endpoint tests are
unchanged. One compile-fail example prohibits reviewer extraction.

These are source scenarios, not executed results. The required command
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check could not start
because rch is absent. Rust compilation, formatting, Clippy and tests remain
unexecuted. No historical verification, production qualification or Beads state
is promoted. All helper judgments are synthetic fixtures.

This is the existing bounded operator-controlled Unix publication-file profile.
Journal work remains synchronous, not a socket-budget latency guarantee. Live
helper jobs are process-local; reopening the original host revokes old permissions
rather than resuming that job. Peer/helper/human authentication, hostile-process
containment, anti-rollback storage and durable hosted-decoder migration remain
separate requirements. No new actor command or automatic deployment is introduced.
