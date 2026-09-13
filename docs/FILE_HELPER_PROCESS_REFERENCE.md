# Executable helpers on the durable full-input publication path

## Connected consumer

`FileOversight::begin_helper_processes` returns `FileHelperProcesses`: the existing
FileHelperPool plus the original `HelperChildren` owner. The underlying
`launch_helpers`, executable validation, environment handling, inherited Unix
socket, worker wire, `HelperClient`, bounded socket-step implementation and child
reaper are unchanged. No shell, alternative executor, inference fallback, verdict
reducer or new dependency is introduced.

This extends FILE_HELPER_TRANSPORT_REFERENCE.md at the existing reference
FA-018/019 and FA-014 boundaries. The operator supplies the complete program roster,
actual input already recorded in the native broker, expected predecessor/input
revision, deadlines, limits and a trusted clock callback. Helper command arguments
and environment are not constructed from actor proposals or evidence bytes.

## Admission, launch and time

Before spawning, the adapter checks the expected revision, entire roster and
input revision, records a current clock observation as needed, and validates the
original worker limits and request encodings. It then starts and PERSISTS the
original full-input review before invoking the existing process launcher. The
worker round is leased immediately: partial launch cannot enable a manual-vote,
replacement-worker or reduced-roster alternative for that round.

The original launcher validates program metadata and aggregate text bounds,
starts the direct children, and returns every successfully started child on
partial OS failure. The durable adapter preserves that ownership, requests stop,
and returns it in `FileHelperProcessError`. No input bytes have been transmitted
at this point. A later explicit new round is distinct work, never an automatic
retry of a failed helper invocation.

After the complete roster is spawned, a fresh callback observation is admitted
through the original durable clock operation. The post-launch observation must
still precede the ORIGINAL commit cutoff. Slow spawning does not extend a review
window. Post-launch time, setup or persistence failure returns the child owner
for cleanup and no usable review pool. No request bytes are sent until the caller
successfully receives and pumps the pool.

`HelperRoundAdmission` distinguishes NotStarted, Committed and Unknown for this
launch. A failure of the initial native begin's storage barrier is Unknown, not a
fictional assertion that the round was absent. A later launch failure preserves
the already-committed round. Earlier clock observations can remain committed even
when the round never starts; launch is not advertised as an atomic OS operation.

## Progress, cancellation and cleanup

The wrapper pumps the existing durable worker transport, polling direct-child
cleanup before and after progress. A successfully exited process without an
accepted reveal remains missing evidence. Exit status is never promoted into a
vote; buffered socket bytes still pass through the original framing and session
checks before the helper is marked complete.

Once a slot completes, fails or closes, its direct child is requested to stop and
polled through the original bounded reaper. The same cleanup continues after a
journal fault, review refusal, cancellation or completed review. Cancellation is
checked against the ORIGINAL action state before further helper I/O. Terminal
stop and owner recovery already remove the native sessions. No helper failure
or cleanup result refunds an automatic reservation or an unknown external effect.

`stop_workers` withdraws only worker transport and requests child termination.
`reap`, `process_statuses`, `all_reaped` and `into_children` let the host retain and
complete cleanup independently of evidence files, review permission or a healthy
journal. A termination request is not proof of process exit. Hosts must retain
and poll child owners until reaped; Drop remains the original bounded best effort,
not a blocking wait or hidden background reaper.

A completed Allow review still requires the original automatic reservation AND
separately held human key for dispatch. The worker wrapper has no permission,
publication or reviewer accessor. It does not issue or refresh keys, repeat an
effect, or treat review completion as publication acknowledgment.

## Source scenarios and verification

Six new public scenarios launch the current test executable as an explicitly
synthetic helper over its actual inherited full-duplex socket. They cover:
complete helper-to-full-input-to-human-key publication; successful process exit
without a vote; partial spawn returning the started child owner; the commit cutoff
crossed during launch; post-launch storage failure with child cleanup and owner
recovery; and cancelling the original action before worker I/O. One separate child
entry function is inert unless its explicit fixture environment is supplied.
The helper inspects its decoded input and chooses a synthetic test verdict; these
are not trained model evaluations or evidence of helper independence.

Two internal tests use real worker socket frames and the existing five journal
barriers (stage, write, file sync, rename, directory sync). A failed commitment
transaction must not emit a reveal request even if rename made the commitment
visible. A failed finished-review transaction must not return a decision even if
its canonical replacement is visible. Both distinguish the acknowledged owner
from the actual disk cut and verify that recovery cannot revive review permission.
These tests inject API errors, not machine power cuts or hostile filesystem faults.

Across the two increments there are seventeen new scenario/test functions, one
subprocess entry fixture and one compile-fail example. Original worker tests and
semantic reducers are preserved. The shared change extracts the existing slot
coordinator; both in-memory and durable paths still use the native session logic.

The following invocation was attempted and failed before compilation with
`rch: command not found`, exit 127:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_helper_transport --test file_helper_processes
```

The new Rust, child-process scenarios, socket tests, storage fault campaigns,
doctests, formatting and Clippy remain UNEXECUTED in this editing session. The
internal target also requires the remote verifier:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib action::consequence::delivery::persistent::observed::helpers::tests
```

No Beads packet or production gate is closed. Historical execution does not
qualify these revisions, and no local compiler fallback was used.

## Unchanged boundaries

Spawn, journal replay and filesystem sync are synchronous operations, not bounded
physical latency. Original finite program, environment, input, response and journal
caps remain; encoded bytes are not total RSS or process CPU/memory limits. The
launcher has no descendant-tree containment, executable attestation or OS sandbox.
Children can retain ambient operating-system access unless the operator isolates
them separately. Clearing the environment and assigning private sockets do not
remove all filesystem, descriptor, user or network authority. stdout/stderr remain
host-private diagnostics, never the actor or voting protocol.

Operator clock correctness, plaintext trusted storage, cooperative locking,
hostile-namespace exclusions, absent cryptographic helper authentication and absent
independent anti-rollback protection remain explicit. The reference integration
does not constitute Asupersync/native foundation admission or production safety
qualification. No trained model or detector was run during this session.
