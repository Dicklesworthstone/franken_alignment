# Durable file observations feeding the original policy-state gate

Consumer: `FileOversight`, the coupled full-input/two-key publication host. This
implements a reference source-capture connection at the existing FA-014/017/062
boundaries; it does not admit a production runtime, provider or storage foundation.

## One source, one original gate

`enable_file_source` registers an immutable `FileSourcePolicy` before any proposal
(including actor admission attempts). It creates the ORIGINAL leased
`PolicyStateCapture`/`PolicyStateWriter` through `OversightBroker`, and enables the
existing first-publication guard if needed. No duplicate policy evaluator or rights
ledger exists. The mode cannot be disabled, widened, switched to another source or
installed as a fallback after an action was reviewed.

`refresh_file_source` reads through the existing sealed `EvidenceFile` interface.
Only concrete original file readers can implement that interface. The exact parsed
`EvidenceSnapshot`, including source identity, scope, producer generation, semantic
epoch, policy values and per-helper contexts, is journaled using its existing file
representation. No file is reopened when historical transitions replay.

Every accepted refresh records a NEW original full-image event and closes it under
the fixed freshness policy. Even an identical healthy reread consumes a source event:
renewal cannot be achieved by relabeling an earlier closure. `observed_at` must be a
trusted read-start tick in the existing process-independent clock domain. Read,
parse and persistence latency consume, rather than extend, the resulting lease.
The caller still advances controller time before subsequent permission checks.
This bounds the age of the local read, not the age or truth of the producer's world
observation. Source authenticity and physical clock fidelity remain assumptions.

The original native gate rejects unmatched, incomplete, expired or unavailable
policy snapshots at proposal/review/authorization/dispatch. Input installation and
key-producing operations also check that the full helper inputs equal the original
builder's output for the retained source. Manually supplying a different context
with otherwise valid policy keys cannot bypass the source binding. Original
full-input/congress and human two-key checks remain mandatory.

## Durable floors and failure

Producer generation and semantic epoch cannot move backward, including after all
old readers and owners are dropped. Same-generation content substitution refuses.
A correctly scoped, well-formed newer producer image becomes a retained floor even
when incomplete or rejected by the stricter native capture limits. An earlier
complete image is not its recovery fallback. Malformed bytes that cannot decode
supply no trustworthy generation; they withdraw eligibility without inventing one.

Source drift, outage and rejected observations withdraw the original capture,
invalidate original per-action input approvals, and revoke pending/approved human
keys. They do NOT cancel an action or refund a reservation or unknown effect.
Restoring bytes can establish a new source observation but cannot restore the old
review or second key. Identical healthy rereads preserve the unchanged full-input
review while renewing only the original observation lease.

Native record/closure errors are COMMITTED refusal results. Source quota exhaustion
therefore retains the original poisoned capture and cannot reset its allowance by
reopening. New source incarnations/replacement are not implemented by this profile.
The existing native event, entry, value, image and retained-byte limits all apply;
large files are refused rather than truncated into an apparently complete source.

A read can also succeed when the journal cannot encode/allocate/store its event.
An explicit live `interrupted` latch then blocks new reviews, grants and publication,
even if historical capture metadata still reports a closure. This includes prewrite
capacity refusal, without misreporting it as an I/O error. Clock, cancellation,
withdrawal, stop and receipt settlement remain available under their original rules.
A later refresh must first COMMIT source withdrawal; it cannot hide the interruption
by rereading earlier bytes. Real storage failure retains the original stronger
unavailable-owner behavior. Both reader and withdrawal errors remain reportable.

Recovery reconstructs the original writer/gate and producer floors from the journal,
then withdraws the source as part of the existing durable recovery fence BEFORE
returning the owner. A saved closure never becomes current simply because its old
tick still looks recent. Neither a copied complete flag nor a new reader supplies
a new observation. Existing executed/nonexecuted receipts still reconcile without
source files, and old sendable envelopes remain unavailable.

Floors bind ACKNOWLEDGED journal history, not an independent anti-rollback anchor.
Unacknowledged writes may or may not be visible on reopening; unavailable owners
never return their speculative captures. Same-path malicious journal rollback,
forged storage and dishonest operators remain outside this plaintext reference
profile. Dropping the standalone file-reader object is not a live withdrawal event;
the host observes availability at refreshes and enforces its fixed lease meanwhile.

## Interface and execution status

Public APIs: `enable_file_source`, `refresh_file_source`, `withdraw_file_source`,
`file_source_required`, `file_source_status`. Capture results are data, not permits.
The independently existing durable driver can continue using explicitly refreshed
captures; automatic source-gate integration at its file-read boundaries is separate.

Ten public test functions exercise real immutable-version file replacements, the
original exact policy/congress/human/endpoint path, expiry and renewal, unavailable
sources, complete-flag/context substitution, owner and reader replacement, retained
incomplete-version floors, native capture poisoning, source/storage failures,
preproposal installation, and prewrite capacity refusal followed by explicit safe
recovery. Each refusal has a permitted control where meaningful. No original
assertion, test body or evaluator is weakened.

Attempted in this editing environment:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_source_gate
```

The command failed before compilation: `rch: command not found`, exit 127. All new
Rust, tests, formatting, Clippy and revision-bound qualification remain unexecuted.
No Beads packet or production gate is closed. The journal remains bounded to the
original 4,096 events/16 MiB maxima and still rewrites/replays complete history.
Native source-event limits can be stricter. These byte bounds are not total RSS,
I/O latency, or production performance results.
