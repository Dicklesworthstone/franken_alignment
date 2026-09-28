# Read-only discovery of durable learned reset outcomes

`FileOversight::read_learned_reset_result` adds the missing inspection path for a
lost reset-completion reply. It serves the durable recovery and containment work
in FA-014/FA-108 without opening a sending owner or appending a recovery fence.
The original write-ahead reset and checkpoint algorithms are unchanged.

## Read the original canonical cut, not a supplied receipt

The caller supplies the directory, original FileOversightProfile, independently
retained exact FileLearnedConfig, and nonzero reset operation ID. The reader uses
the existing bounded canonical-file reader and journal decoder. It checks the
complete recipe before running any original numerical history, then replays ALL
events and their exact witnesses, including those after the requested reset.
It does not stop checking once it finds a plausible completion.

The returned FileLearnedResetSnapshot exposes its journal revision and operation
alongside exactly one FileLearnedResetRecord:

- NotRecorded: the operation has no intent in the completely verified cut.
- Pending: the original intent exists without a completion. Its interrupted bit
  reports recorded containment, not whether a process is currently executing it.
- Completed: the exact original intent and recomputed native result. An inner
  error remains the original failed/refused reset, not a successful restore.

A pending reset is never executed by this API. Existing completed resets are
recomputed by the original semantic replay and compared, not deserialized into
an unchecked owner. No caller-supplied cache, receipt, approval or judgment can
replace those checks. A changed recipe or corrupt event anywhere refuses the
entire read; it does not produce NotRecorded or a partially verified result.

## Lost replies, locked writers and full journals

The reader takes no writer lock, installs no fence, confirms no clock, issues no
reviewer or checkpoint handle, and never writes or cleans up staged bytes. It
works while the existing writer is alive or faulted and when event capacity
cannot fit another recovery fence. The existing canonical replacement protocol
provides the captured file cut; a concurrent writer can advance afterward.

After a completion's directory-sync acknowledgment is lost, canonical bytes may
contain that completion. A read describes those visible bytes; it does NOT issue
a replacement durability acknowledgment. After an earlier storage failure, the
canonical cut can still contain only the original intent even if staging holds
more bytes. Staging is never promoted by inspection. Absence from this cut is
not proof that no unreported numerical work ran or that any effect did not run.

This snapshot cannot resume generation, settle an unknown effect, refund rights,
clear an incident, supply a human key, or create a permit. Those operations retain
the original writable recovery, containment and endpoint protocols.

## Implementation and verification status

Added the bounded original-replay reader, immutable projection and seven runtime
regression sources plus a compile-fail authority boundary. The sources cover live
writer inspection, intent-only and interrupted cuts, all five completion-storage
faults, exact recipe and witness verification, corrupt suffixes, full journals,
recorded native failure, and unchanged outstanding effect accounting.

Compilation, tests, rustfmt and Clippy are UNEXECUTED in this preparation
environment: required remote-only RCH cannot launch. Source checks and synthetic
fixtures are not runtime, detector, journal-authenticity or release qualification.
This path performs synchronous whole-history numerical replay with its original
resource limits; it does not claim low latency, process isolation or cross-host
floating-point fidelity.
