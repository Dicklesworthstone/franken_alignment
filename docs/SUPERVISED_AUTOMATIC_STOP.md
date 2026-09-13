# Owned inference through automatic endpoint containment

Consumer: SupervisedDriver coordinating its original hosted decoder, actor
mailbox, helper children and PublicationEndpoint. This connects the opt-in policy
in HOSTED_AUTOMATIC_STOP.md to the existing endpoint fence and receipt protocol.
It is not an actor-visible command, a new runtime or a second authority ledger.

## One call, three independent results

The driver now exposes advance_hosted_forced and advance_hosted_sampled with an
explicit supervising clock callback. They invoke only the original broker-owned
numerical methods. HostedDriverStep separately reports the inference result,
mailbox synchronization and optional endpoint-containment result. A numerical
Held can therefore coexist with a failed clock observation or fence write,
without that combination being misreported as successful endpoint containment.
No inference result proposes, authorizes or dispatches an external effect.

For a configured trip, the original broker stops local admission before returning
from inference. The driver publishes queued/accepted request outcomes, releases
its now-cancelled active review/permit, and requests direct-child cleanup BEFORE
fallible clock or endpoint work. It samples the clock freshly, installs and
acknowledges the current endpoint fence, and invokes the original stop sweep.
Existing execution receipts win; missing requests are sealed before refunds;
expired retention remains unknown. No helper, fresh input view or human key is
required to settle an already-issued effect.

The sampler's draw and monitoring cost are not rewound by an endpoint failure.
Nor are held token IDs or raw logits returned. A retry services the same original
stop and effect obligations, not another inference, replacement permit or resend.
Calls rejected before computation do not create a numerical incident or cancel a
healthy helper review. A quiet step still needs the full existing congress and
permit path before any publication, including both keys in a configured profile.

## Existing scheduling and interrupted endpoints

The ordinary step/step_with_clock loop, including its file-evidence path, calls
service_hosted_stop before helper I/O or evidence acquisition. A lower-level
broker trip or an owner poisoned by a caught unwind is thus handled at the next
supervised turn. With no automatic trip there is no extra clock callback and the
original flow remains unchanged. A serviced trip returns DriverEvent::HostedStop
with the exact original StopSweep, including per-attempt failures. A failed clock
or fence returns the actual error, not Idle or a fabricated drained event.

service_hosted_stop is also explicit for a host progressing only containment.
None means no automatic incident; it is not a safety certificate. Completed
manual stops and intentional incident suspensions retain their existing APIs and
are not relabelled as numerical alarms. Retained trigger data names the original
local stop even if reconnection later advances the dispatcher epoch again.

File-endpoint failure can leave the stop latched and effects charged. The
existing detach/reopen/reconnect flow retains the original controller, numerical
owner, policy, mailbox and obligations. Once the original endpoint is recovered
and fenced, another normal driver step resolves its actual execution history.
There is no second publication, restored admission or fresh numerical draw.
A delayed envelope may execute BETWEEN local stop and endpoint acknowledgment;
that outcome must remain executed, not be retroactively refunded.

Cleanup is bounded polling through the existing direct-child owner. A local stop,
endpoint drain, and actual child exit are distinct: DriverStopProgress::quiesced
still requires all of them. Nothing runs while the supervising process is not
scheduled; no watchdog, process-death recovery or descendant containment is
claimed. Parameter/probe provenance and trained-detector utility remain external
requirements of the reference profile.

## Source coverage, not an executed qualification

Seven driver regression scenarios include full sampled-logit equivalence and
socket-reviewed publication in both key profiles, active/queued cancellation,
post-inference clock refusal with both possible race outcomes, ordinary-loop
containment without evidence or reviewer roles, retention expiry, file-fence
failure with offline recovery, and a quiet-review preflight control. One
additional subprocess scenario waits for child-written PID readiness, confirms
quiet inference leaves those children active, then verifies that a trip requests
termination and actual reaping even when the post-inference clock refuses.
Its subprocess entry point is not a separate scenario.

Combined with the broker increment this adds seventeen scenarios across eighteen
Rust test functions and one compile-fail example. All weights/probes/verdicts are
synthetic fixtures. Rust compilation, formatting, tests and RCH qualification
have NOT run in this editing environment. The required RCH command could not
start because rch is unavailable. Existing tests, manifests, dependency admission,
production gates, historic execution evidence and br-managed tasks are unchanged.
