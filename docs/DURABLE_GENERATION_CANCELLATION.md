# Durable generation cancellation

## Capability and status

2026-09-23: source implementation of `FileOversight::cancel_decoder_generation`.
Eight `generation_cancel_` regression tests are authored, not executed. The RCH
attempt failed before compilation (`rch: command not found`, exit 127). Rust
compilation, tests, rustfmt, Clippy and the full repository gate remain unexecuted.
Source hashes and whitespace checks are separate from execution qualification.
No broader bead or production gate is closed.

The consumer is the existing supervisor of a durable monitored decoder. An
unfinished generation previously blocked every new numerical request and reset
until it completed, or the whole authority domain was permanently stopped.
The supervisor can now cancel one request at an acknowledged token boundary,
retain its exact released prefix and work, and later explicitly resume the SAME
numerical state under a new request. This serves FA-014/FA-025 and plan sections
6.3, 11.2 and 14.9; it does not add an actor control or change effect settlement.

## One original record, no rewind

The input-only `CancelGeneration { id, revision }` decoder event uses tag 11.
All preexisting tags and encodings remain unchanged. Semantic replay validates
the exact pending identity, generation revision and numerical predecessor and
reconstructs the cancelled report from the ORIGINAL cursor, not caller-supplied
output, counters or a serialized cursor. The cancellation itself computes no new
token. The existing file transaction still replays prior history; this is not a
claim of zero CPU, constant-time cancellation or exactly-once physical inference.

The original result book retains the command and conservative requested-step
reservation. The new `GenerationFinish::Cancelled` reports termination without
asserting full prompt review, a natural stop token or a usable helper verdict.
Start/end positions, actual reviewed prompt count, released tokens, admitted
numerical/sampling work and last review remain unchanged. An unstarted intent
has zero work and zero reviewed prompt tokens, including when numerical context
or budget admission has not run. Native helper evaluation continues to require
a fully reviewed prompt and `StopToken`; cancellation cannot manufacture a vote.

The cache, PRNG and lifetime monitor/numerical counters remain on the original
owner. No effect reservation, charge, unknown outcome, incident counter, policy
or authority epoch is cancelled, refunded or rewritten. Existing in-memory
session cancellation still destroys its numerical owner; it gains no escape
hatch for returning an unfinished decoder.

## Stop first, resume explicitly

A successful new cancellation clears only its pending generation cursor and
pauses the decoder while withdrawing clock readiness. A fresh trusted clock
observation and the existing explicit `resume_decoder` operation are required
before new work. All original guards, holds/failures, source interruption,
pending forecasts and permanent-stop rules remain in force. Cancellation is
allowed without acquiring a fresh clock or evidence source, including after
recovery; it does not itself acknowledge or repair an interrupted source.

Active cancellation requires the CURRENT journal and generation revisions.
A stale cancellation returns an error rather than the read-only progress retry
used by ordinary advancement. Once terminal, exact ID retries return the
original result (whether Cancelled, Held, Failed, StopToken or another finish)
without a write or pause. Thus a lost reply for an older request cannot cancel a
newer active generation. Future generation revisions refuse even on terminal
requests. Existing command retries preserve all original input/budget fields.

One ordinary canonical replacement acknowledges the transition. Old and new cuts
never contain an intermediate refund or extra token. The existing byte/event
ceilings apply; logical recovery capacity and physical disk space are not
created. Storage failure exposes no candidate output and makes the owner
unavailable until exclusive recovery. Successful cancellation is not a remote
or descendant-process termination receipt.

## Authored tests

The first eight regressions use the unchanged original synthetic-weight decoder
fixtures and actual journal files. They exercise unstarted, partial-prompt and
partial-output boundaries; exact state/work conservation; stale and missing
identities; cancelled-result retries while another request advances; preserved
holds/stops/admission failures; explicit resume; cancellation before numerical
admission; retained history reservations; exact/one-under event capacity; and
semantic replay rejection of duplicate or mismatched cancellation records.
Positive native generation controls remain beside the negative cases.

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generation_cancel_
```

These tests are source assertions, not demonstrated runtime behavior. They do not
qualify trained-model fidelity, host isolation, power-loss durability, external
source authenticity, actor-facing API compatibility or production deployment.

## Byte-exact text and crash recovery

`FileOversight::cancel_decoder_text` delegates to the same numerical cancellation
and returns the existing `FileTextGenerationProgress`. It requires an original
text intent; a bare numerical ID cannot acquire a tokenizer or text receipt
retroactively. Exact tokenizer/command matching and output-capacity preparation
precede the mutation. After acknowledgment, only the existing released IDs are
decoded into that reserved capacity. No additional sample, newline, EOS, Unicode
repair or output-sink write occurs. The cancellation can be read through the
existing progress, result, read-only canonical-image and anchored recovery APIs.

Eight more tests cover original byte spans/control IDs and prompt review counts,
reopening before and after cancellation without a current clock, every original
Store barrier at unstarted/prefill/output boundaries, split UTF-8 with subsequent
explicit continuation from the SAME cache, modeled source-reader interruption,
bare-ID and stale-text refusal, exact-history guarded recovery, manual tag-11
bytes and malformed/truncated cancellation records. The source interruption case
uses the existing private transient fault seam, not a claim of live capture.
The Store seam distinguishes visibility from acknowledgment; it is not physical
power-loss or remote-process evidence. An old or complete cancellation image must
retain identical numerical state and output, never an extra token or refund.

Source review corrected the first capacity fixture's startup event count:
`FileOversight::create` writes an empty history; decoder enable, tokenizer and
Time make three events, not four. The exact/one-under cancellation assertion is
unchanged; the independently bounded profiles now reach the intended boundary.
No production predicate or existing test outside this new feature was weakened.

All SIXTEEN authored tests, Rust compilation, rustfmt, Clippy and the complete
gate remain UNEXECUTED. The second targeted attempt also failed before
compilation (`rch: command not found`, exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generation_cancel_
```

The added public enum variant requires exhaustive downstream `GenerationFinish`
consumers to handle cancellation explicitly. Old journal bytes retain their
meaning; old readers reject decoder tag 11 rather than silently treating a
cancelled request as completed inference. No native in-memory cursor-cancellation
API, authority format, dependency, actor verb or production qualification is added.
