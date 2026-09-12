# Paired reset through active and disconnected supervision

Consumer: ActorSupervisor, SupervisedDriver and OfflineDriver using the original
hosted numerical owner. This integrates HOSTED_DECODER_RESET.md with the existing
actor-ticket, helper-process and file-endpoint paths. It creates no new effect
adapter, authority ledger, background executor or actor-visible reset command.

## The same actor mailbox survives

ActorSupervisor::reset_hosted_decoder invokes the original paired reset while
retaining its existing mailbox. A restoring reset marks queued requests from the
abandoned continuation CancelledBeforeDispatch without manufacturing ledger
attempts. Accepted work is projected from the original ledger. Already sent
requests remain unknown or keep their terminal outcomes; no reset refunds them.
The original idempotency keys, exact payloads, conflict checks and tickets survive.
Fresh intake stays available but requires explicitly new requests with the new
authority epoch. An old request is not rewritten to that epoch or queued again.
If the original incident policy suspends instead of restoring, intake closes.
This is distinct from claiming an endpoint stop acknowledgment.

Invalid reset preflight does not cancel a valid queue or active review. An
admitted replay failure can withdraw numerical eligibility without changing
ledger dispositions, exactly as documented by the paired broker API. Neither
failure invents a successful reset or discards an outstanding reservation.

## Active reviews and owned children

SupervisedDriver releases its active review and retained permit only after a
successful original reset. The supervisor has already published the ledger's
outcomes, so no discarded permit is confused with an external nonexecution
receipt. Cleanup then uses the existing direct-child owner without a blocking
wait, replacement worker, repeated congress or blind effect resend.

Routine connected/offline cleanup also reads the ORIGINAL ledger's attempt
stage. Children for a job cancelled or made terminal by a lower-level trusted
reset are stopped even when the next clock update or reconnection fails. Cleanup
preserves the pending job long enough for a later successful driver step to emit
its existing Stopped event; it does not silently replace that event with Idle.
An explicit successful driver reset already has a reset receipt and releases its
job immediately. Terminal stop keeps its pre-existing release behavior. Child
exit is still not evidence of effect outcome. Use the supervisor/driver reset
entrypoint for immediate queued-ticket updates; a raw broker reset has no access
to the actor mailbox.

## Offline effects remain outstanding

OfflineDriver exposes the same paired capture/reset operations without adding a
send path. The original controller, checkpoint map and actor port stay alive.
The retained endpoint-recovery key can reopen the file endpoint, and the existing
reconnect operation still requires its new dispatcher fence. Reset alone does
not touch endpoint storage or settle anything. Reconciliation remains possible
without decoder progress, helper inputs or the human-reviewer role; historical
dispatched evidence remains attached to its original source stream.

## Verification boundary

Six added integration scenarios plus one subprocess entrypoint cover fresh
socket-helper publication in both key modes after reset; retained-permit cleanup;
invalid-reset positive control; real file publication with a lost acknowledgment,
offline numerical reset and fenced reopening; incident-driven intake closure; and
PID-identified child termination/reaping after a lower-level reset followed by a
backwards-clock refusal, preserving the original Stopped event. The fixtures use
synthetic weights and helper verdicts. Combined with HOSTED_DECODER_RESET.md this
increment has thirteen scenarios, fourteen Rust test functions and one
compile-fail example. None has been compiled or executed here. The required RCH
command could not start because rch is absent. Historical execution logs,
production qualifications, dependency manifests, original tests and br-managed
task states have not been changed.

This remains bounded full-prefix reference recomputation. It does not provide
whole-process authority recovery, hostile-process/descendant containment,
automatic checkpoint policy or trained-model/detector qualification.
