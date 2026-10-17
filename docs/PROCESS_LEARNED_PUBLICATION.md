# Process-backed learned publication

The `create-learned-generated` operator can run the original learned sidecar
congress through separately launched helper programs. The explicit
`fa.learned-publication/6` recipe selects this path. Recipes /1 through /5 retain
their in-process native evaluator and registered sidecar-probe behavior.

This serves the learned review/publication path in FA-018, FA-019, FA-107 and
FA-113, plan sections 9.1–9.3, 10.15 and 14.9, and founding ideas FI-A03, FI-A05,
FI-A14 and FI-I04. It does not complete those broader packets.

## What the original owners enforce

`FileSupervisedDriver::start_process_learned_sequence` joins an already admitted
actor request to `FileLearnedProcessReview`. The request, attempt, frozen action,
current learned sidecar, input revision, complete snapshot and journal revision
must agree. The consuming wrapper retains the same actor gateway and job.

The existing process sequence owns every durable Begin, socket, commitment,
reveal, refinement and direct child. A round is consumed before process launch.
The launcher's fresh post-launch clock check runs before any request bytes can be
sent; a late or panicking clock returns the original driver and recoverable
partial children. Failure does not create a replacement native or manual round.

Every subsequent quantum acquires current policy evidence and trusted time
through the original learned provider. Accepted refinement changes the job to the
new input revision. Later programs are fixed in advance and launch only after
the preceding round has produced its witnessed refinement **and every preceding
direct child has been confirmed reaped**. The complete schedule reserves its
bounded starts before the first launch. Future unstarted slots remain distinct
from observed process starts.

A finished permitting review can make the original driver ready for independent
human approval. It does not publish. The existing automatic permit, human permit,
current policy, exact learned input and publication checks remain necessary.
Child exit status is inspection data, never a vote, inference result or effect
receipt. Cancellation retains the original actor's unknown-effect accounting.

## Select the operator recipe

Keep the existing model, tokenizer, learned fit, monitor, generation, publication,
floor and sidecar fields. For version 6:

- Remove `files.native_roster` and the entire `native_review` object.
- Supply `files.forecast` explicitly as either `null` or the absolute path to an
  existing required-forecast artifact. A supplied forecast uses the original
  forecast admission, sampling and finish checks.
- Supply the finite ordered `process_review` object below.
- Keep `sidecar.priority` explicit. Its retained refinement ladder and cumulative
  disclosure budget must cover the complete declared round schedule.

The following is the new object, to insert into the otherwise complete recipe.
Executable paths, arguments, environment, member names and round IDs are
independently provisioned operator inputs.

```json
{
  "process_review": {
    "rounds": [
      {
        "id": 701,
        "members": {
          "alpha": {
            "executable": "/operator/bin/fa-native-helper",
            "directory": "/operator/helpers",
            "arguments": ["/operator/helpers/alpha-round-701.json"],
            "environment": {}
          },
          "beta": {
            "executable": "/operator/bin/fa-native-helper",
            "directory": "/operator/helpers",
            "arguments": ["/operator/helpers/beta-round-701.json"],
            "environment": {}
          }
        }
      }
    ],
    "polls": 4096,
    "input_bytes": 1048576,
    "salt_bytes": 32,
    "starts": 2
  }
}
```

The member keys in every round must exactly match the configured congress.
Round IDs are nonzero and unique, retain the order written in the array, and
match the sidecar round allowance. Input, salt, poll and aggregate start bounds
must fit the original caps. The aggregate starts must admit every member of every
future round; later cancellation does not refund that reservation.

The closed parser validates all program text and roster membership before model
asset ingestion. Executables and working directories must be absolute, normalized
paths. The original launcher preflights the complete live program schedule before
the first process Begin. It supplies a cleared, explicitly selected environment,
uses the executable directly, and passes each member only its private inherited
socket. Review input bytes travel through that socket rather than arguments.

Each helper must implement the existing full-input commit/reveal protocol.
The existing `fa-native-helper` can supply native monitored inference with its
independently provisioned manifest. Each round should have its own declared
manifest/salt and exact input profile, including the actual policy epoch after
any recovery. The supervisor does not rewrite worker identities, salt material,
model policy, sampling choices or epoch bindings to make a reply fit.

Version 6 retains the version-4 finite message continuation and explicit finish
semantics, including the original numerical lifetime and remaining work budgets.
A supplied forecast retains the version-5 required-forecast behavior. This
version change is explicit; the native-plus-probe behavior of /1–/5 is not
silently substituted by external protocol replies.

## Stop, failure and cleanup

The operator yields in short real intervals while waiting for external work,
then reacquires policy and time for the next protocol quantum. It does not sleep
straight to a commit deadline or invent elapsed time. The authenticated stop
service can cancel the process review and then apply the existing nonce-bound
stop through the original driver.

Terminal handoff retains the original process review, including child status,
disclosure history and any unresolved startup marker, while authority drains.
Bounded helper cleanup spends the remaining existing cleanup allowance.
`cleanup_pending` includes unreaped or uncertain process custody. A requested
kill, a dropped handle, or an absent owner is not reported as confirmed reaping.

Receipt-only recovery still loads the exact numerical recipe but performs no
process launch, executable preflight, live helper inference or policy-source
read. A missing helper executable does not make an already recorded receipt
unreadable. It cannot turn that receipt into a new admission or resend an effect.

## Scope and verification

This adapter uses the original Unix process launcher. It does not establish
statistical independence of helper judgments, authenticate a remote model,
sandbox descendant processes, revoke ambient OS credentials, or impose a
global inference/CPU quota. The starts limit counts admitted direct children.
Deployment containment remains part of the existing uncompleted host profile.

The public process-driver regressions exercise actual child processes, learned
generation, two-round refinement and predecessor reaping, source/clock failures,
cancellation, both publication keys and an original external publication.
Operator regressions exercise explicit /6 parsing, actual full-input replies,
human refusal, helper denial/early exit, cleanup and receipt-only recovery.

Tests are authored and source-reviewed. All twelve changed process-driver,
operator and test sources passed nonmutating syntax parsing by rustfmt from the
qualified nightly-2026-09-08 toolchain; their bytes were unchanged. This is not a
formatting or type check. Required remote-only RCH execution was unavailable in
the 2026-10-17 session; its daemon refusal prevented compilation (exit 103), and
the execution environment later restarted. Compilation, Clippy, runtime tests,
empirical detector results and production qualification remain open.
