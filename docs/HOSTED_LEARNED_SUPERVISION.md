# Learned reset and automatic containment through supervision

The original actor supervision and connected/offline drivers now consume the
[paired learned reset](HOSTED_LEARNED_RESET.md). This keeps the broker's actual
numerical owner, authority ledger, mailbox, helper lifecycle and endpoint
reconciliation on the same path. It introduces no actor-visible reset command,
replacement model, second ledger or external send path.

## One mailbox before and after reset

`ActorSupervisor`, `SupervisedDriver` and `OfflineDriver` expose
`capture_hosted_learned_checkpoint` and `reset_hosted_learned` using the broker's
opaque checkpoint handle and typed reset request/receipt. They invoke the
original learned broker methods; the driver does not implement another reset.

The actor supervisor retains the original mailbox, request keys, exact payloads
and tickets. A restoring reset cancels queued requests from the abandoned
continuation without inventing ledger attempts. Accepted requests are projected
from the original ledger: only undispatched reservations can be refunded, while
dispatched/unknown requests remain outstanding. Fresh intake stays available
under the new authority epoch. Old requests are not rewritten or resubmitted.
Incident-threshold suspension instead closes intake.

An invalid preflight leaves queued requests and active review usable. An admitted
numerical failure may cause the original configured automatic stop; mailbox
synchronization still runs even when reset returns an error, so that actual stop
is visible to actor tickets. No failed reset creates a successful receipt.
Checkpoint capture also synchronizes an actual stop that its broker call
observes or services; a refused capture cannot leave that stopped job active.

The connected and offline driver wrappers release their active review and
retained permit only after a successful original reset. They run the existing
nonblocking child-maintenance path afterward. Maintenance also recognizes
ledger cancellation and an actual stop after a failure. Child cleanup does not
establish whether an external effect executed.

## Recovery with a detached endpoint

The offline driver now reaches the same paired learned checkpoint map and
original numerical owner while preserving its lack of a dispatch method. Reset
does not change endpoint storage, settle an effect, refund an unknown charge or
copy authority into another owner.

The original endpoint recovery capability can reopen its file, and reconnection
still requires the original dispatcher restart and acknowledged endpoint fence.
A late original receipt can reconcile an effect while disconnected. Fenced
reconnection can also resolve outstanding outcomes without rerunning a helper,
resending an effect or requiring a human-review role. The old actor tickets
remain the handles for these outcomes.
An acknowledged fence alone does not establish earlier nonexecution. A missing
endpoint record remains unknown and charged until the original expiry/seal or
receipt protocol supplies a terminal outcome.

## A learned trip reaches the endpoint barrier

`SupervisedDriver::advance_hosted_learned` delegates one original token step
through the existing hosted driver composition and returns
`HostedDriverStep<Rc<GenerationEvent>>`. Its three observations stay distinct:

- `inference` reports the actual original learned generation event or refusal.
- `synchronization` reports actor-mailbox projection.
- `containment` reports an attempted original endpoint stop sweep, if there is
  a configured automatic incident.

A held numerical token may therefore coexist with an endpoint clock or I/O
failure. The driver does not convert that into an acknowledged fence or refund.
The existing stop service checks both mutually exclusive owned numerical
profiles. It services the original learned stop handler when there is no
conventional-host incident, including a learned trip initiated through trusted
lower-level supervision.

The ordinary driver loop invokes this same service before helper I/O and new
work. The original local stop first closes admission and cancels undispatched
work. The driver then releases that job, synchronizes mailbox closure and
requests child cleanup before attempting a fresh clock observation and endpoint
fencing/reconciliation. A late clock refusal or storage failure leaves the
endpoint obligation visible for a later service call. Neither path sends another
effect or reruns congress. Completed automatic containment remains idempotently
visible; manual stops retain their existing separate API.

## Verification boundary

Seven supervision regression functions exercise original learned inference with a fresh empty actor,
real helper sockets and the original endpoint protocols. They cover successful
and refused reset, ticket continuity, current reviews, reserved versus unknown
charges, offline recovery, fresh post-reset publication and learned automatic
containment through typed advancement and the ordinary driver loop.
Together with the thirteen broker reset tests, twenty new regression functions
cover the paired reset and its supervision consumers. No executed test count is
implied by that source inventory.

Compilation, test execution, rustfmt and Clippy remain **UNEXECUTED** because the
required remote RCH command cannot launch. Source review and synthetic-model
controls are not detector, process-isolation or deployment qualification. This
adds in-process supervision; it does not journal a learned reset or recover a
lost broker process. Cumulative text-stream reset and independent attestation of
restart grade/pre-incident checkpoint selection remain outside this path.
