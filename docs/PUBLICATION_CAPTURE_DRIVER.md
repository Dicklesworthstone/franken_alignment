# Source-bound evidence in the supervised driver

The concrete publication capture from `PUBLICATION_CAPTURE.md` now participates
in the ORIGINAL `FileSupervisedDriver`, rather than requiring a supervisor to
manually sequence three refresh operations around it. The additional adapter
uses the existing worker review, automatic authorization, human-key dispatch,
first publication and separate reconciliation phases. No second job state
machine, rights ledger, endpoint or worker protocol is introduced.

## Entry points

Bind the original source capture and witness requests on the same durable owner
using `bind_publication_file_source`, while the attempt is Reviewing and before
automatic authorization. An already completed committee review can be used;
binding a witness source does not replace or repeat that review. The underlying
owner must have enabled mandatory publication validation before the proposal.

`FileSupervisedDriver::step_with_publication_source` accepts a concrete
`PublicationInputFile`, the existing trusted clock, a committee evidence callback,
an optional independent human permit and an optional credential permit. The
callback cannot provide publication witness bytes; the adapter opens the concrete
file. Existing native policy-source requirements remain independent: supplying
cached committee evidence cannot renew a required policy-source lease.

`FileSupervisedDriver::step_from_files_with_publication_source` accepts the
existing sealed `EvidenceFile` reader plus the concrete publication-input reader,
clock and optional permits. Its inner adapter is the exact `FileProvider` used by
`step_from_file`, including configured durable source updates, generation checks,
lease renewal, refusal recording and read/withdrawal failure distinctions.
No new public provider trait or positive cached-input fallback is exposed.

These entry points require the active attempt's source to have been bound. They
do not silently create a weaker unbound profile. Existing driver APIs are
unchanged, but a source-bound owner's native gate still prevents a single capture
from authorizing multiple effect boundaries through those older APIs.

## Acquisition and effect order

Each time the existing state machine requests evidence, the adapter first commits
publication-input unavailability in the original canonical journal. ONLY THEN
does it invoke the committee callback or native file provider, validate its whole
committee input, and open the publication witness file. An invalid/missing
committee result returns with publication inputs still unavailable. A successful
witness read is action/source/generation-checked on the original replay candidate
before it can become current.

In the normal Ready step with an independent human permit, the driver captures
before authorization, then captures AGAIN before dispatch. The subsequent
first-publication step captures once more. The native gate consumes each
capture's eligibility at its own boundary. Original deadline/lease checks use
the clock after acquisition, so reading does not extend an old approval deadline.

The original publication phase is retired to reconciliation before calling any
provider. A caller-caught committee-provider panic therefore leaves durable
witness unavailability and cannot reuse an earlier successful capture through a
direct host publication call. A failed second Ready read retains any automatic
reservation already made; the original cancellation/review rules decide how to
resolve it, not a refund or a fabricated retry in this adapter.

Idle steps, receipt reconciliation and already resolved/expired publications
never invoke either evidence provider. Prior execution remains charged and can
be acknowledged even when both files are missing. A first-publication evidence
failure seals the original request; only the subsequent original receipt
reconciliation can refund nonexecution. Installation/storage failure instead
produces the driver's existing PublicationUnknown result and keeps the charge.

## Reports and limits

The return type is
`driver::evidence::publication::FilePublicationDriverReport<FileDriverEvent>`.
`reads` contains actual witness-file read results. Its nested `evidence` field is
the unchanged `FileEvidenceReport`, including committee observations, durable
source-update results and the original driver event/error. A successfully read
producer identity is NOT a successful installation or publication: inspect
`evidence.result`, including PublicationUnknown. Empty reads also occur when a
preflight or committee acquisition failed before the witness file was opened.
There are at most two capture attempts in one step, not an unbounded retry loop.

The two producer reads are sequential, not an atomic cross-file snapshot. The
operator still owns source provenance, producer correctness, file publication
and the clock. This profile does not authenticate a remote provider, sandbox an
actor-controlled directory, prove completeness, or establish a real-time read
bound. Existing bounded packet and journal limits remain mandatory.

`tests/file_driver_publication_capture.rs` adds eight integration tests using
real helper sockets and native human/driver operations. They cover the three
acquisition boundaries, live phantom versus unrelated updates, second-read and
post-dispatch loss, caught callback unwind, source-independent receipt/expiry
paths, successful reads with rejected installation, and the paired concrete-file
path retaining the native policy-source lease contract. Compilation, formatting,
Clippy and test execution are NOT claimed: the required RCH command cannot start
in this environment because `rch` is absent. FA-062 remains open for verification.
