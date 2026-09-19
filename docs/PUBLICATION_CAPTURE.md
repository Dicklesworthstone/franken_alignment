# Concrete publication-evidence acquisition

This extends FA-062's original publication gate with a source-bound file profile.
It closes the acquisition gap between a host-supplied current snapshot and an
actual read at the authorization, dispatch and first-publication boundaries.
It is still a bounded reference profile, not a production adapter authenticity
or alignment claim. New Rust code and regression tests await RCH verification.

## Producer contract

`observed::publication::capture::FilePublicationCapture` encodes one producer
identity, a positive generation, one attempt, the complete `action_frame` of its
controller-produced `FrozenAction`, and the existing `FilePublicationInputs`.
The latter preserves the structured snapshot, independently admitted closure and
whole opaque helper input. The file contains the versioned `FAPCAP01` packet,
not JSON, a permit, or an asserted validation result.

A producer writes a new file and atomically renames it over the source path.
Increment the producer generation whenever ANY packet input changes, including
snapshot revision or control cut. Re-reading an unchanged generation is allowed
only with exactly equal reconstructed inputs. The durable owner rejects a lower
generation, same-generation equivocation, another producer, another attempt or
another frozen action. Original source identity and generation history survive
withdrawal and replay. This is per-attempt history, not a global producer service.

`PublicationInputFile::new(path, source)` fixes a reader selection and performs
no I/O. `read_capture()` reopens that file every time: no cached positive result
exists. It rejects symlinks/nonregular files, over-limit data, malformed packets,
trailing bytes and detected inode/metadata changes during reading. Both the path
and open handle are checked. The read is bounded to 4 MiB plus one sentinel byte;
allocation is bounded before parsing. Source paths are at most 4,096 bytes.

The operator must own the producer and its directories. Regular-file metadata
checks do not make a hostile filesystem race-free, authenticate a producer,
prove snapshot completeness, or guarantee wall-clock freshness. A blocking local
filesystem is also not a real-time I/O deadline. Do not treat this profile as a
network/provider adapter or as a sandbox for actor-controlled paths.

## Bind once, acquire before each boundary

Create a `FileOversight` with `create_with_publication_validation`. Propose the
action through the original owner and complete the original committee review.
Use the exact returned action to produce the original capture. Then call:

```text
bind_publication_file_source(journal_revision, attempt, original_capture, requests)
```

This atomically journals the original witness recipe AND the producer binding.
It binds only while Reviewing, before authorization. There is no replacement or
disable operation. The original capture establishes requirements and a producer
floor, not current eligibility. An ordinary `record_publication_inputs(Some(..))`
cannot inject a cached positive observation into this source-bound attempt.

Before authorization, dispatch and first publication, call:

```text
refresh_publication_from_file(journal_revision, attempt, &source)
```

This first commits `Inputs(None)` in the SAME canonical journal and ONLY THEN
opens the file. A successful read is action/source/generation-checked by semantic
replay before its current-input event is acknowledged. Native snapshot/control
high-water marks remain in force. One successful capture can support only one
of these three validation boundaries; the original gate consumes its freshness
when it checks. Reusing the authorization capture for dispatch refuses without
spending the original effect/human keys. Reusing a dispatch capture for first
publication seals the endpoint request instead of executing it.

The caller must sample the trusted elapsed tick after acquisition when invoking
the existing checked-publication operation. This sequencing and one-comparison
freshness are not a time-to-live or proof that a producer cannot change afterward.
All original policy, whole-committee, human, credential, deadline, revocation and
receipt checks remain conjunctive. No parallel rights ledger is introduced.

## Failure and recovery

The refresh result distinguishes two facts:

- `Ok(Err(FileCaptureError))`: reading/decoding failed, and the prior durable
  withdrawal remains in force. No positive fallback is available. Before dispatch,
  original unspent keys can proceed only after another successful read.
- `Err(JournalError)`: the owner could not acknowledge the acquisition transition.
  Capacity, encoding, replay or storage failure during positive installation
  quarantines the owner. In particular, a regressing/equivocating observation is
  not forgotten so the live owner can go back to an earlier quiet generation.

A panic after the initial withdrawal also leaves the canonical inputs unavailable.
Opening the journal does not read the producer file: it replays the ORIGINAL
recorded observations and then performs the original recovery fence. Source
bindings, generation floors and original witness requirements remain historical;
old sendable keys/envelopes are not restored. Old undispatched work is cancelled.

A source error after dispatch cannot refund a liability. The original publisher
seals on failed revalidation; only receipt reconciliation settles the charge.
Already executed/sealed outcomes and original deadline resolution take priority
over new source acquisition. Query/reconciliation does not require the file.

## Resource and evidence boundaries

Source binding capacity uses the original publication gate's at-most-16 lifetime
slots. Each source additionally retains one bounded full generation image so
same-generation equality does not rely on a lossy digest; unavailability does
not erase that image. Capture/prefix reconstruction, exact generation comparison
and journal replay add work beyond the existing validation comparison budget.
The native snapshot/whole-input limits and enclosing journal byte/event limits
remain mandatory. File observation events use new sub-tags of original journal
event 30; existing event encodings are unchanged and older readers reject the
new sub-tags rather than omitting the source contract.

`tests/file_publication_capture.rs` covers one-read-per-boundary behavior,
unchanged rereads, live phantoms versus unrelated updates, missing files,
producer rollback/equivocation, attempt/action substitution, cached-input bypass,
source-independent historical replay, malformed packets and filesystem bounds.
These tests are authored but unexecuted here. Required validation remains:

```bash
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

The current editing environment has no `rch`, `cargo` or `rustc`; no passing
compiler, test, formatting or Clippy result is claimed and FA-062 remains open.
