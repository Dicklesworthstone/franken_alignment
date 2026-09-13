# File-backed evidence across the durable supervised effect lifecycle

## Implemented consumer

`FileSupervisedDriver` now connects to the existing sealed `EvidenceFile` readers,
including the concrete bounded `FileEvidenceSource`. The original file format,
parser, scope checks, generation floor and full-input builder are reused; no new
provider parser, executor, policy reducer, authority ledger or dependency is added.
This extends FILE_SUPERVISED_PUBLICATION.md and FILE_EVIDENCE_REFERENCE.md at the
existing FA-012/014/018/062 boundaries. The original `FileReviewLaunch` type is
reused rather than introducing another worker-launch description.

The APIs are `start_file_review`, `start_file_process_review`,
`request_human_approval_from_file` and `step_from_file`. They remove the need for
an embedding host to construct file-derived committee inputs or manually relay
cached snapshots between those lifecycle phases. They are library integration,
not a standalone service, production `fa` command or authenticated source system.

## Capture and review

Start preflight checks the existing admitted request, original Reviewing state,
input revision, complete worker roster and available child slot before reading.
The actual file reader opens the operator-selected file anew. The capture must
be complete and must bind the action's complete scope and helper roster. The
original builder includes each member's selected context and its source/version
identity; private policy keys and other members' contexts are not automatically
sent to that worker. The original full-input manifest remains the judgment basis.

The driver starts the original socket or process review with this exact capture
and reference root. Its clock is sampled after the file read; the existing process
launcher additionally checks a fresh post-spawn clock. Launch does not perform
inference or invent a human decision. A failed spawn keeps the original started
child owner inside the driver, and its error retains the successful capture ID.

If a source read or validation fails, the driver records the original unavailable
input event before returning the source error. A failed durable withdrawal is
retained separately alongside that source error; it is not reported as success.
No new worker round is started in either case. Existing successful observations
or earlier journal operations are not advertised as rolled back.

## Human request, authorization and publication

The file-backed human-request method performs a fresh file capture, passes it
through the original eligibility-withdrawal and ready checks, samples time after
capture, and then freezes the original human request. It does not approve that
request or reserve automatic rights. The separate FileHumanReviewer or reviewer
transport must still issue the actual second key. The original one-request-per-
context rule is unchanged; restoring a file does not recreate a consumed review.

`step_from_file` uses the existing driver's evidence callbacks. There is one read
at completed review, up to two during dispatch (including the read AFTER automatic
reservation), and one at an unresolved live first publication when the original
publication guard is enabled. Historical outcomes and query-only reconciliation
perform no reads. Source failures remain in `FileEvidenceReport.observations`
even when the native result is a completed restrictive review, a sealed endpoint
outcome, or a failed journal operation. Those diagnostics do not grant authority.

The publication guard must still be explicitly enabled before proposals. These
methods do not silently upgrade legacy owners or disable an enabled guard. A
source change after dispatch is handled by the original guarded publication/seal
transaction; any charge is released only by original outcome reconciliation. A
failed second authorization read retains its already-reserved original permit.
No failure triggers an automatic helper rerun, new grant, source fallback or resend.

## Source scenarios and verification

Eight public file scenarios use actual immutable-version file writes/renames and
the existing helper client over real Unix socket pairs. They cover successful
file-to-helper-to-human-to-publication flow and private-view isolation; missing,
changed and incomplete human-request evidence; deletion between authorization
reads; final-publication source drift; a failed new launch invalidating earlier
review eligibility followed by a genuinely new review; combined source/storage
failure; preflight and cancelled-job read suppression; and post-read human expiry.

Two additional process scenarios launch the current test executable through the
original HelperProgram/HelperChildren code. They cover the successful file-backed
pipeline and partial spawning with both source identity and child custody retained.
A separate child entry function is inert without its explicit fixture environment.
The child inspects the original decoded context and selects a synthetic Allow:
this is not trained-model evaluation or evidence of helper independence.

The positive lifecycle asserts six concrete reader attempts: launch, completed
review, human request, two dispatch reads, and first publication. Reconciliation
then runs after the actual source file is removed, without a seventh read. This
is a specified test assertion, not an executed counter or performance result.

Attempted in the editing container:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_supervised_file_evidence --test file_supervised_file_processes
```

The command failed before compilation with `rch: command not found` (127).
Compilation, formatting, Clippy, tests and revision-bound qualification remain
unexecuted. No local compiler fallback, dependency admission, Beads closure or
production-gate activation occurred. Combined with the guarded-driver increment,
there are twenty new test functions including one subprocess entry fixture.

## Unchanged assumptions

The file producer declares completeness and publishes immutable versions by
atomic rename. Reader freshness means a new bounded file read, not proof of a
complete or freshly observed external world. Paths/ancestors, source identity,
human identity, clock correctness and worker isolation remain operator duties.
The reader's generation floor is process-local, not independently authenticated
or durable anti-rollback storage. Same-generation byte substitution remains an
error, and version changes conservatively change the helper's actual input.

Accepting a PolicyFileSource through its existing sealed reader interface does
not install that adapter's live PolicyStateWriter gate into FileOversight. No
such admission is claimed by this integration. Existing synchronous file/journal
latency, finite history limits, plaintext storage and cooperative-lock assumptions
remain. The actor still receives only its original restricted outcome projection.
