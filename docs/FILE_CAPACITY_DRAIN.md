# Finish containment when ordinary journal admission is exhausted

Consumer: FileSupervisedDriver's operator responding to journal pressure under
an explicitly configured RecoveryReserve. This uses the original durable actor
owner, terminal stop, endpoint sweep and helper-child custodian. It does not add
a control loop or automatically treat every generic Limit error as an incident.

`stop_with_recovery_reserve(request, clock)` returns an outer error if no stop
receipt was acknowledged by this operation. After an acknowledged original stop,
it returns CapacityStop with that receipt and a SEPARATE drain result. A clock,
retention or storage failure cannot turn a completed stop into an alleged rollback,
or make an unacknowledged sweep refund visible through this wrapper.

The method validates the active job's exact owner and the reserve configuration.
A refused/stale stop preflight preserves healthy work. Once the original stop is
acknowledged, the driver releases its cancelled review and retained permit and
requests direct-child termination BEFORE invoking the clock. The original unknown
effects remain charged. The sweep needs no helper input, provider callback or new
human key. Direct-child termination is not called reaping or endpoint settlement.

An exact retry reuses the original StopReceipt rather than appending another stop
input. A conflicting operation/predecessor refuses. If current native progress is
already drained, the operation returns AlreadyDrained without a clock observation
or another journal event. Otherwise it runs the original progress_stop with the
new caller-supplied tick. This is particularly important after a visible sweep
whose directory synchronization failed: reopening must honor its actual outcome,
not spend an extra tail event merely to repeat it. No permissions are reconstructed
from a saved driver phase or receipt.

Four driver scenarios cover acknowledged stop followed by stale-clock failure,
retry without a second Stop, a real publication awaiting acknowledgment, rejected
preflight with permitted original publication, and stop-storage failure followed
by exclusive recreation of the original authority. The first scenario has multiple
related retry assertions, not independent accuracy samples.

Three five-barrier storage campaigns exercise failed local stopping and failed
endpoint draining, including successful rename followed by failed directory sync.
The old faulted owner exposes no candidate receipt, capacity reading or refund.
Recovered human withdrawal remains atomic with the native stop, and consumed human
keys stay consumed. Each campaign preserves original executed/missing distinctions.
These are deterministic source fault campaigns, not executed power-loss tests.

A separate malformed-history case appends a well-framed ordinary clock event into
reserved capacity, without using the admission encoder; canonical decoding must
refuse it. A same-prefix Fence is admitted and retains the original rights. No
saved balances or fabricated terminal outcomes are introduced into the fixture.

The process scenario uses the actual Rust test executable and original inherited-
socket HelperClient. The parent waits for each child's atomically written PID
marker AFTER it receives the real input. Its control arm permits those same
workers to complete review and co-signed publication. Its capacity arm exhausts
ordinary journal slots, requests the original stop, fails the following clock,
and requires the same children to be stopped/reaped before recovery completes.
Fixed helper verdicts are fixtures, not trained-model effectiveness evidence.

Combined source coverage: twenty scenarios in twenty-one Rust test functions,
including one inert subprocess entry point, plus one compile-fail example. All
original test bodies and assertion strengths are retained. The RCH runner is
absent; none of this Rust has been compiled, formatted, linted or executed here.
Source/patch integrity checks are not crash qualification. No dependency, original
reducer, bootstrap bytes, license, historical result or Beads status is changed.
The finite logical-reserve and operator-controlled-storage limits documented in
FILE_RECOVERY_CAPACITY.md remain in force.
