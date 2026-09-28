# Durable automatic learned containment

This connects the existing learned-host automatic stop to its durable numerical
recipe and original publication owner. It serves the plan's effect-boundary,
containment and recovery contracts (sections 8, 11.2 and 16.4). The detector,
stop reducer, journal transactions and endpoint settlement are unchanged.

## Bootstrap and recovery

`FileLearnedConfig::with_automatic_stop(HostedStopPolicy)` consumes an uninstalled
recipe. It freezes the original policy ID, generation and operation together with
ALL original model, codec, probe, sampler, prompt and resource configuration.
There is no live enable/disable/retune API. The original broker installs this
policy at zero tokens, before returning a usable durable owner.

The existing guarded bootstrap publishes the configured policy in the first
canonical image. Numeric, native-text, text-stream and required-sidecar settings
are preserved by the same original installation path. Each existing learned
open/read entry point must match the independently retained exact recipe before
replay. A missing policy or changed operation, generation or ID is not a weaker
recovery mode: it is a binding failure, before cleanup or role release.

The wrapper uses the disjoint `FALSTOP` versioned configuration domain inside the
existing bounded configuration blob. Old recipes without this option retain their
original bytes and manual-containment behavior. Outer journal tags and numerical
step witnesses are unchanged. Wrapper order is part of the exact recipe identity;
retain the selected complete recipe rather than reconstructing a different order
from its human-readable labels.

## Trigger, acknowledgment and obligations

Every original nonquiet monitor result can trigger the configured terminal stop:
alarms, threshold equality, unresolved evidence and monitoring-budget exhaustion
remain their distinct original causes. An admitted numerical or source-sync
failure remains an operational failure, not a fabricated detector alarm. Stale
preflight refusals are not admitted numerical failures. Quiet execution retains
its original numerical results and still requires congress and both effect keys.

The original durable learned step already completes local stop cleanup before
acknowledging its numerical result. Its existing exact witness binds the original
stop receipt, source, numerical state and costs. A committed trip pauses inference,
withdraws keys and cancels undispatched work through the original reducer. A
withheld candidate is not published. No extra journal event or second stop engine
is introduced by this configuration.

`learned_host_stop_policy` describes the configured mode. A missing policy is not
a safety claim. `learned_host_stop_incident` returns the first original incident
from a healthy, acknowledged owner. It performs no I/O or inference. An owner
whose canonical replacement failed refuses this getter rather than returning its
older RAM observation or an unacknowledged candidate incident.

The write-ahead numerical intent remains the barrier after failed completion.
Recovery with the exact recipe must resolve that same operation; it cannot erase
a pending trip by returning to the previous quiet prefix. A reconstructed trip
stays terminal. Its retained incident is historical, not fresh eligibility, and
recovery does not revive old keys.

A local stop is not endpoint settlement. Dispatched or unknown effects stay
charged, including across process recovery. The existing `progress_stop`,
reconciliation and sealing APIs obtain original endpoint outcomes. Only a real
nonexecution result can release its charge; sealing an already executed effect
cannot undo its publication. The journal remains the local publication sink,
not a remote-transaction or hardware-crash proof.

## Authored verification

Ten new integration tests use original tiny-model inference, learned compression,
exact probes and real local journals. They cover first-image configuration and
both sidecar-wrapper orders; quiet numerical parity through two-key publication;
a real alarm withdrawing existing keys in the acknowledged step; operational
budget failure with a manual-mode control; missing/changed recipe rejection
before cleanup; failed staging and exact pending-intent recovery; dispatched and
executed liabilities; stale preflight; native-text quiet/alarm behavior; and a
real unresolved probe with an otherwise matching retained-residual quiet control.
One compile-fail example rejects a live disable method. Coefficients and helper
ballots are synthetic controls, not empirical detector qualification.

Text-stream and other guarded compositions retain the existing installation
path but are not independently qualified by these tests. The source addition
proves no authentication, statistical detection rate, process isolation or
preemptive latency bound. It does not implement durable checkpoint/reset.

Targeted `cargo test --locked -p fa-reference --test durable_learned_stop` and
full `cargo run --locked -p xtask -- check` were attempted through
`RCH_REQUIRE_REMOTE=1 rch exec --`. Both stopped before compilation: `rch` is
absent (exit 127), as are Cargo, rustc and rustfmt. Compilation, Rust tests,
rustfmt and Clippy remain UNEXECUTED. Preparation contains selected source, not a
complete checkout. Source-integrity checks are not runtime validation. No Bead,
qualification gate or production release is closed.
