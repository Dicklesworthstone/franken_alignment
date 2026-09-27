# Durable release of original learned text

## Original consumers, one journal

`FileLearnedConfig::new_text_stream` and
`FileOversight::create_with_learned_text_stream` connect the source-bound text
stream to the existing durable learned-generation and two-key publication paths.
This serves plan 8.2, 8.7 and 8.8: a completed message, its external publication,
its receipt-confirmed audience history and its explicit finish are different
transitions. None can be inferred from an earlier quiet token or a timeout.

The independently supplied configuration includes the exact original model,
tokenizer, fitted codec, monitor roster, prompt, sampling/completion policies,
resource ceilings AND the stream profile with all audience limits. Its binding
uses a new explicit `FALSTRM/1` domain inside the existing bounded configuration
blob. Existing numeric and raw-text recipe encodings and journal event tags are
unchanged. No parser imports models, tokenizer tables, KV, RNG or output from
this blob. Ordinary recovery must match the complete independently supplied
configuration before the original engine reconstructs its observations.

The first canonical image already contains the original StreamBootstrap and
Learned Enable operations. Original replay validates both before creating the
store. There is no acknowledged stream-only intermediate image. The installer
checks the actual audience profile against the pinned recipe before any token
replay, then uses the original broker-owned text-stream constructor. Empty
initial history, output capacity, canonical-file replacement and storage-error
semantics remain those of the existing implementations. An incomplete create
can leave storage resources; it does not expose a usable generic bootstrap.

## Recovery and immutable audience history

`open_with_learned_text_stream` selects this configuration kind and invokes the
existing learned-generation recovery. It performs the original exact recipe
match, original numerical/authority replay, cleanup and recovery fence. It does
not restore old permits or human approvals. The reconstructed generator remains
paused until separately observed current time and explicit numerical resume.
An interrupted write-ahead token intent remains a barrier until the original
matching outcome is acknowledged. Holds, failed work and exhausted original
telemetry allowances remain in the reconstructed state.

The message and finish proposal methods borrow the SAME crate-private original
stream builder and then call the existing durable `propose` transaction. They
accept no payload, target replacement, inferred prefix or smaller byte charge.
Generic proposals still reach the source-bound gate, so they cannot evade this
profile. No proposal/approval mutation happens outside the journal.

`read_stream_publication_with_learned_generation` replays one canonical image
and returns both the published and receipt-confirmed stream views, plus pending
status. It does not acquire a writer, clean staging, fence, resume, observe time,
create a permit or return an executable generation. It is historical inspection,
not independently authenticated delivery or a fresh source observation.

## The post-dispatch publication boundary

The durable publisher revalidates AFTER the original dispatch made the stream
pending. Ordinary new-message builders must still refuse every pending send.
The text action check therefore has one narrow additional path: the pending ID
must equal this exact already-dispatched attempt and an original delivery record
must exist. It then recomputes the exact original frame from the still-confirmed
audience history and completed live generated text, including boundaries,
target/version, scope, epoch, expiry and full-frame charge.

This is a revalidation of an existing obligation, not a new-send capability.
The original source/actor check, whole-input approval, consumed human key,
policy witnesses and endpoint validation all remain conjunctive. Missing input
or a late witness change seals nonexecution through the original publisher;
it does not authorize the frame just because dispatch previously happened.
Only original receipt reconciliation changes the charge or confirmed history.

An executed but unacknowledged append blocks new append AND finish. After the
original receipt confirms it, a separately reviewed finish can be proposed.
An unknown nonexecuted append or finish remains charged until original sealing
and receipt processing establish its actual disposition; only then can a new
reviewed attempt retry that operation. Published text is never removed, and a
cancelled or nonexecuted finish does not declare a completed audience stream.
Completed message and finish replays add no new logical generation step or RNG
draw. The ordinary journal replay DOES recompute historical numerical work.

## Authored checks and bounded claims

Eleven new integration test functions use the original nonzero-attention
fixture, byte BPE, fitted codec, monitored generation, real canonical files,
original congress, independent reviewer role and endpoint. They cover every
prompt/sample/terminal recovery cut; atomic first-image contract; exact stream
profile substitution; independently wrong actual bootstrap; exact capacity;
stale and generic-payload refusal; unfinished token intents; failed or
undecodable output; old-key fencing; published-versus-confirmed append/finish
cuts; unknown charges and positive sealing/retry; and missing/changed final
publication evidence with a nearby successful control. Existing source,
publication, stream and durable-text tests are not modified or weakened.

All new tests, compilation, rustfmt and Clippy are UNEXECUTED here. Required
remote-only targeted and full RCH checks stop before compilation because `rch`
is unavailable. Selected-source and Git hash checks are not a complete checkout,
runtime evidence, detector qualification or production approval. No Bead or
roadmap packet is closed.

This profile still owns ONE generation/message and its finish, not multi-turn
conversation generation or native CLI wiring. Its helper ballots in tests are
synthetic controls, not empirical safety measurements. Reconstruction and
whole-journal encoding/copying have their original costs; no constant-time
recovery, zero-copy claim, global inference escrow or measured speedup follows.
