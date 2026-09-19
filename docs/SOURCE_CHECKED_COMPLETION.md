# Source-acquired atomic completion

## Capability and consumer

`FileOversight::complete_publication_from_source` completes an already reviewed,
automatically authorized, independently human-approved, source-bound publication.
It composes the existing witness acquisition, native two-key dispatch, checked
publication and receipt reconciliation. Credential-guarded owners additionally
require their current process-local credential permit. There is no new authority,
endpoint, event tag or serialized permission type.

This closes a composition gap: source-bound attempts consume one fresh capture at
each validation boundary. A single pre-acquired observation cannot legitimately
serve both dispatch and first publication in a composite completion. This method
performs TWO actual acquisitions rather than marking one observation fresh twice.
Authorization remains a separate original operation with its own capture.

The API and data types are in `observed::publication::capture::completion`:
`CapturedCompletionKeys` borrows the original automatic/human keys and an optional
credential; `CapturedCompletionReport` retains actual file-read results, a callback
failure when present, and the result of the completed publication/settlement.
A successful read identity is not an acknowledged installation or a permission.

## Ordering and atomicity boundary

Before external provider or clock code runs, the owner commits the original
publication-input withdrawal. That is an acknowledged, non-permitting transition;
it is not rolled back by a later failure. Foreign keys, wrong source, stale journal
revision, missing required credentials and insufficient full-batch event capacity
are rejected before acquisition.

The completion then uses a private replay of the SAME owner and endpoint:

1. Capture actual committee/policy evidence, reread the concrete witness file,
   sample trusted time after capture, install that observation and dispatch using
   both original keys.
2. Withdraw candidate witness eligibility, recapture committee/policy evidence,
   reread the witness file again and sample time again. Validate at first publication.
3. Reconcile the original endpoint receipt and replace the original canonical file
   once with the entire effect/accounting cut.

The second provider call cannot inspect the candidate broker or receive its keys.
A concurrent read of the canonical publication during either provider sees only
the acknowledged withdrawal and undispatched reservation, not speculative effects.
On successful completion it sees the publication AND its settled original ledger.
The revision advances by eight for two successful acquisitions: one withdrawal
plus seven native events. A missing second acquisition omits its Captured event.

This is atomicity for the journal-as-publication sink, NOT atomicity across the
producer files, remote services or physical effects. The pre-read withdrawal is a
separate durable write. The two provider/file reads are sequential. Their contents,
producer authenticity, coverage and the clock remain operator assumptions.

## Failure behavior

Before any successful file read, ordinary read/provider failure leaves the witness
inputs withdrawn and the original reservation/keys intact. A real new acquisition
can retry that undispatched attempt; an old cached positive-input fallback cannot.

Once the first concrete read succeeds, installation, clock, capacity, replay or
storage failure leaves the live owner unavailable. A caller-caught unwind cannot
revive older producer data or expose the in-memory candidate. Recovery opens the
original canonical history and withdraws old sendable keys. Never infer success or
nonexecution from a storage error; a post-rename failure can expose the whole cut.

After native dispatch has been staged, ordinary second-provider/file unavailability
becomes missing evidence for the original publication check. It seals the request
and reconciles the original nonexecution receipt in the same final cut. Later
better evidence cannot rebase the reviewed judgment or reroll the sealed effect.
Detected producer equivocation/rollback instead remains an installation error,
not a fabricated successful sealing result. Original expiry takes precedence at
first publication, and cannot be extended by the time spent reading.

A credential is checked before acquisition and again before the credentialed native
publication event. Secret bytes do not enter events or the canonical journal. No
credentialed profile falls back to ordinary publication. Existing independent
policy/source/identity/consistency/mediation gates still run on the original machine.
The callback API does not renew a configured native policy-source lease; stale
native leases still refuse. Existing paired-file driver steps remain available
for hosts that require durable policy-source refreshes between transitions.

## Bounds and verification

Admission reserves the maximum eight additional events before acquisition; the
original journal encoder checks every subsequent prefix, including its byte and
recovery-capacity limits. Concrete files retain their original capture bounds and
there are at most two reads. Candidate replay and repeated encoding are additional
bounded work, not part of the witness comparison budget or a scalability claim.

`tests/source_checked_completion.rs` contains nine regression tests with positive
publication and unrelated-change controls, all structured dependency negatives,
source/provider loss, expiry, equivocation, callback unwind, event capacity and a
real staging-file failure. `tests/source_completion_credentials.rs` adds valid,
missing, foreign, rotated and revoked credential paths and secret nonretention.

All new Rust tests are authored but UNEXECUTED in this editing environment. RCH,
cargo and rustc are unavailable. The exact integrated revision still requires
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`. No production
activation, Bead closure, authenticated-provider guarantee or crash proof is claimed.
