# Reusing acknowledged learned replay state

Unqualified source implementation. Plan roots: 6.3 (owned work), 6.4 (ordered
control), 10.6 and 18.3 (replay/scheduling cost). This extends the existing
cooperative learned intent/completion path, not its numerical or authority laws.

## Move an acknowledged predecessor instead of rebuilding it

Both preparation types now offer `finish_with_continuation`. Intent completion
returns a `FileLearnedReplayContinuation`; numerical completion returns the
original nested numerical result together with one continuation. A continuation
is the former live Machine moved out only AFTER the original canonical write
acknowledges. The newly committed machine remains the sole live authority.
There is no clone, saved-tensor import, serialized-cache constructor or generic
prepared-event executor. The continuation owns no Store or file lock and exposes
no old reviewer, policy writer, broker, helper session, permit or model sample.

The retained machine represents the prefix BEFORE the just-committed event.
`verified_events()` exposes that prefix length only. `prepare_intent` and
`prepare_completion` consume the continuation and run the SAME live admission
checks as their empty-machine counterparts, including owner identity, exact
revision/position, pending operation, source interruption, pause and ordinary
capacity. Their resulting preparations must replay EVERY missing event through
the original reducer before finalization. They can use existing strict advance
or explicit catch-up; neither can finalize over an unseen journal tail.

A continued completion normally replays only its original Begin. The following
intent normally replays the immediately preceding witnessed Step, including
original numerical recomputation and witness comparison. Earlier verified
history is retained, not re-executed from zero at every stage. Intervening time,
cancellation, policy and receipt events are replayed rather than ignored.
No claim of elapsed-time speedup follows merely from this structural reduction.

## Acknowledgment, failure and recovery are unchanged

Intent finalization still acknowledges only Begin, with no next numerical step.
Completion still executes the original step and persists its original witness
before returning output. An acknowledged numerical Err is not a storage error
and cannot be hidden by a continuation. A held sample stays withheld. Either
write's storage failure returns NO continuation or candidate result and keeps
the live owner unavailable; original independently configured recovery decides
which canonical prefix became visible.

Dropping or unsuccessfully consuming a continuation only loses a performance
optimization. It does not undo an intent, refund an unknown effect, restore a
source, reset a work allowance or authorize a new request. A new/recovered owner
has a different identity and cannot adopt the continuation even at the same
path with the same configuration. Recovery continues to replay the entire
canonical journal, verify the independent recipe and fence old rights. No
continuation is persisted or used as a recovery trust root.

Ordinary finalizers retain their previous result types. Their implementation
shares the same numerical and persistence bodies with retaining finalizers;
there is no alternate numerical algorithm or file-replacement protocol.

## Cost bounds and tests

A continuation retains one additional original bounded Machine. This trades
memory retention for avoided historical recomputation. Callers should drop it
when done. Individual original events, encoding and storage writes remain
synchronous; whole-journal encoding and other transactions are not optimized by
this interface. There is no CPU quota, latency guarantee, parallel runtime,
physical-work refund or durable computation escrow.

Eight regression functions are authored against the original decoder and actual
journal files: all-token parity with an independent generator and cold original
replay; exact journal bytes; mandatory tail replay and stale finalization;
foreign/recovered ownership; original refusal precedence and capacity; both
writes at all five fault barriers; corrupt new numerical witnesses; actual
monitor hold; and executed/unknown effect preservation. Two compile-fail
examples deny copying and authority extraction. Synthetic models are causal
controls, not trained-detector effectiveness evidence.

Targeted RCH tests and the full required xtask gate could not launch because
`rch` is absent (exit 127). No local compiler fallback was used. Compilation,
tests, rustfmt and Clippy are unexecuted. Source checks do not qualify runtime
behavior, and no bead or acceptance gate is closed by this implementation.

## Existing synchronous calls reuse the continuation automatically

`begin_learned_step`, `complete_learned_step` and therefore
`advance_learned_generation` now use one private continuation slot on the
original FileOversight. Existing numeric, text and stream callers need no new
loop, callback, model or recipe. A successful intent installs its retired
predecessor; successful numerical completion replaces it with the next retired
predecessor. With consecutive steps and no other writes, each warm stage replays
one missing original event instead of the whole history. The first cold stage
still reconstructs the full original journal.

Other acknowledged transactions can extend that cache's tail. A later learned
call replays them all before finalization; it never applies cached state over
cancellation, revocation or source replacement. The cache does not advance or
hold back the live authority. An invalid request may discard the optimization,
not a pending intent or an effect obligation. Stale journal revisions and known
storage failures refuse before cache consumption. A replay error is returned
without automatically retrying a cold path or concealing divergent history.

`clear_learned_replay_cache` drops the additional process-local machine without
changing the journal, numerical state, source, epoch, pending operation or any
effect balance. Subsequent calls can reconstruct normally. The cache is never
serialized, restored, cloned or shared with a recovered owner. Recovery remains
cold and leaves the original generator paused. Explicit empty-machine
preparation APIs retain their previous behavior; their retaining finalizers let
a cooperative caller manage the same optimization explicitly.

This retains at most one additional cached Machine per host, including after
normal generation completes until cleared or the host is dropped. Unrelated
transactions and separately requested preparations can still allocate their own
private candidates; this is not an aggregate memory reservation. Whole-journal
encoding, file replacement, other transactions' replay and original inference
remain synchronous. No measured latency, throughput or asymptotic end-to-end
speedup is claimed without executed workload results.

Eight further regression functions cover the actual synchronous entry points,
verified one-event warm tails, byte-identical cold records, cache release during
a pending intent, explicit/cached interleaving, stale competing finalizers, both
writes at all five fault barriers, acknowledged numerical-budget failure, actor
cancellation, actual policy-file replacement and preservation of a refused
producer version. Policy tests pair refusal with a genuinely newer complete
observation and original successful intake, not a mocked permitting gate.
These tests, like the first eight, remain unexecuted because RCH is unavailable.
