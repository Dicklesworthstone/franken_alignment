# Native-text bootstrap recovery

## Contract and use

`FileOversight::open_generated_text_stream` and
`open_generated_text_stream_with_reserve` recover the existing native-text-only
stream without combining a generic stream opener with a separately checked
model. They are the writable counterparts of the original bootstrap
constructors. This implements the local recovery portion of plan sections 8.7,
11.2 and 16.4, serving FI-A04/FI-A16 through the existing L1/L4/L5 owners. It changes no plan semantics or authority rule.

Supply the independently retained oversight profile, exact stream profile,
`FileDecoderConfig` and `ByteBpe`. The no-reserve opener requires absence of a
reserve. The reserve opener requires exactly the original logical reserve;
it does not install or enlarge one during recovery.

The implementation acquires the original exclusive store, reads one bounded
canonical image, decodes it under the supplied oversight profile, and checks
that its first event is the exact `GeneratedStreamBootstrap`. An ordinary
`StreamBootstrap` refuses even with matching model, tokenizer and current text.
It also checks reserve presence, value and uniqueness. The existing text replay
validator then pins actual model/monitor/sampler and tokenizer bytes before
replaying the full numerical and authority history. Only after those checks
succeed does the opener confirm storage, clean original staging and create the
original owner/reviewer followed by the original recovery fence.

A successful open therefore returns a **paused** decoder. It neither generates
a token nor publishes a message. Acknowledged prompt, progress, work and random
state are reconstructed by the existing reducers; they are not caller inputs.
Fresh time and any required source evidence must be obtained through the
original APIs before explicit `resume_decoder`. A held or failed numerical
owner cannot be rerolled. Old action keys are fenced, not restored; generated
messages still need their original source admission, congress, independent
human review, publication and reconciliation.

This is a local reference-store bootstrap contract, not an authenticated latest
head, an anti-rollback mechanism, remote delivery, detector qualification or OS
isolation. Deployments requiring independently anchored recovery or composed
guarded roles must still use those separate contracts. Physical replay work can
repeat; this does not assert exactly-once CPU execution.

## Cooperative recovery of a source-bound owner

`FileOversight::begin_open_generated_text_stream_from_source` provides the
cooperative form of `open_generated_text_stream_from_source`. It returns a
`FileGeneratedSourceRecovery`, exported with its progress and status types from
`observed::stream::generated::checked::source`. The synchronous source opener
now completes this same cursor and finalizer. The two original stream-only
openers above retain their separate contracts.

The caller supplies the original oversight profile, complete decoder and
tokenizer, and independently selected `GeneratedSourceProfile`. Before the first
historical event, the cursor locks one original store, decodes one bounded
canonical image, and checks the exact generated-stream bootstrap, source
identity/current registered generation/limits/lease, reserve, selected
publication witnesses/feed/fallback and joint policy. Complete model, monitor,
sampler and tokenizer bytes remain pinned. Source replacements still execute
through the original governance reducer; matching a requested generation does
not import it as state. Beginning recovery reads no live evidence source,
executes no historical event and acknowledges no clock, cleanup, Fence or Stop.

| Operation | Boundary |
| --- | --- |
| `progress()` | Reports the frozen total and original event attempts/completions, with `Replaying`, `Ready`, `Interrupted`, `Failed` or `Cancelled` status. It exposes no candidate machine or live role. |
| `advance(expected_events, max_events)` | Requires the exact completed predecessor and a positive bounded quantum, then applies at most that many original events to the same private reconstruction. Stale/invalid polls spend no replay work. Failure or unwind prevents reuse of partial state. |
| `cancel()` or drop | Releases private replay and the lock without changing canonical or pending files. It does not issue Stop or imply that an external liability settled. |
| `finish()` | Requires complete original replay and all selected bindings, then applies and durably publishes one original recovery Fence before returning the owner and reviewer. It reuses the verified machine rather than replaying numerical history again to finalize. |
| `stop_request(operation)` | Selects the original no-clock Stop predecessor only at `Ready`. An existing Stop retains its exact operation/request; another operation refuses. |
| `finish_admission_stop(request)` | The privileged host explicitly consumes the same verified machine through the existing terminal Stop consumer. No live owner or reviewer escapes. |
| `into_recovery_stop(operation)` on Linux | Transfers the same store, canonical image and verified machine to the original authenticated peer Stop consumer. The already-recorded terminal reserve is mandatory and is neither installed nor replenished. |

Both finalization paths retain canonical-image equality checks before storage
cleanup and use the original storage acknowledgments. A changed image refuses;
a failed replacement returns neither live authority nor a Stop acknowledgment,
even if its candidate bytes have become visible. Partial replay never issues an
owner or terminal target. A successful ordinary finish preserves acknowledged
prompt, numerical/RNG spend and stream history while withdrawing old source/time
eligibility and keys. Fresh evidence/time and explicit resume remain required.

The terminal peer path waits for the complete original authenticated,
nonce-bound request. An offered operation, connection, silence or malformed
request cannot authorize Stop. A new acknowledged admission Stop appends only
the original Stop/Fence pair; an exact already-stopped retry appends nothing.
There is no ordinary recovery Fence before that pair, source acquisition,
settlement clock, drain sweep or new model work. Unknown effects remain charged
and a pending stream stays obstructed. Admission acknowledgment does not imply
drain, process termination or remote halt.

The quantum counts original event calls. It does not bound physical numerical
products, memory or elapsed time. Initial image read/decode, each individual
event, final encoding and persistence remain synchronous. The caller must check
its own deadline/control before each quantum and again at `Ready`; a checkpoint
does not interrupt an event already executing. This source-bound opener adds no
independent latest-head anchor, guarded-role qualification or anti-rollback
assurance beyond its original contract.

### Source and verification status — 2026-11-15

Ten regression functions are authored under `source/recovery/tests.rs` and its
Linux transport module. They cover all six source/publication selections,
actual interrupted native computation followed by fresh-source continuation,
exact original Fence bytes, every replay cut, stale/invalid quanta,
model/tokenizer/source/publication substitutions, failure/unwind latching,
canonical substitution, all five original storage barriers, retained Unknown
charges and stream obstruction, explicit authenticated Stop and zero-write
retry. Two compile-fail examples protect early-owner and consumed-cursor
boundaries. Synthetic weights test protocol invariants, not detector quality.

Independent source review, targeted dated-rustfmt processing and whitespace
checks are complete for this library increment. Compilation, runtime tests,
Clippy and the required full RCH gate remain pending because `rch` is unavailable.
No new runtime pass, Beads closure or production qualification is claimed. The
live service uses this cursor as described in the
[native publication contract](NATIVE_TEXT_PUBLICATION_SERVICE.md#cooperative-reopen-and-continue-stop).

## Independently retained prefixes for source-bound recovery

`begin_open_generated_text_stream_from_source_anchored` and
`open_generated_text_stream_from_source_anchored` accept the same source-bound
configuration plus an independently retained `FileHistoryAnchor`. They serve
FA-014 and plan sections 7.3, 8.5 and 16.5/16.9 without changing the ordinary
openers or their journal format.

Capture the prefix from the original acknowledged owner with `history_anchor()`
and retain it separately from the mutable authority store. The existing
`FileHistoryAnchor::write_to` / `read_trusted` framing can transport that private
archive. Importing an archive does not authenticate its origin; the operator
must select the independently retained copy. The canonical prefix includes
private evidence and is not an actor-facing artifact.

The anchored constructor compares the exact original encoded prefix, including
its oversight profile and canonical store identity, immediately after decoding
the exclusively locked image. This happens before machine construction or any
historical numerical event. An older image refuses with `Stale`; a divergent
equal-counter history or another store's prefix refuses with `Binding`.
Refusal leaves canonical and pending files unchanged. A genuine append-only
successor is accepted, including later numerical events, but its complete
suffix must still satisfy the original source, model, tokenizer, publication
and replay contracts.

Both forms use the existing `FileGeneratedSourceRecovery`. Cancellation, Ready,
the final canonical-image comparison, the single ordinary Fence and the
authenticated terminal Stop handoff retain their original boundaries. Recovery
preserves numerical spending, stream history and charged Unknown effects.
Neither the anchor nor its successful comparison supplies fresh source evidence,
current clock eligibility, helper judgment or human approval.

This establishes continuity with the independently selected prefix. It does
not establish an authenticated latest head, protect history newer than that
prefix, or prevent rollback if both journal and retained anchor are replaced.
Other guarded-role and deployment contracts remain separate.

Eight additional regression functions in `source/recovery/tests/anchored.rs`
pair actual resumed native computation under all six source/publication
selections with equal-counter forks, valid truncation, foreign stores, invalid
suffixes, changed independent inputs, Unknown-liability Stop/retry, cancellation
and canonical replacement after Ready. Independent source review is complete.
The execution environment is unavailable; compilation, Rust formatting, runtime
tests, Clippy and the required RCH gate have not run for this increment.

## Source status and change record — 2026-09-25

Added the two exact-bootstrap openers and their shared preflight to
`observed/stream/generated/required.rs`. Five tests in `required/tests.rs` use
the existing synthetic decoder weights and real canonical files. They cover
partial progress followed by explicit continuation versus uninterrupted
computation; pre-resume refusal with unchanged bytes; stream, model, tokenizer
and reserve substitution; retained reserves; ordinary-stream rejection paired
with its valid ordinary opener; held-generation receipt-only retries; and
strict duplicate-reserve/stream-limit preflight. A completed generated result
is explicitly checked not to imply publication.

The existing wire and journal encodings, numerical reducers, authority reducers,
dependencies, production activation rules and prior test bodies are unchanged.
The [native publication service](NATIVE_TEXT_PUBLICATION_SERVICE.md) now consumes
this opener in explicit `serve-open --native-text` recovery. It keeps unsubmitted
continuation separate from exact recorded-request receipt recovery, without
replacing the numerical or authority reducers. Its seven additional regression
functions are also unexecuted.

**UNEXECUTED:** the required fresh command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` failed
before compilation because `rch` is absent (exit 127). Rust compilation, the five
new tests, rustfmt, Clippy and the full gate have not run. Static inspection is
not execution evidence; historical gate receipts do not qualify these changes.
No Beads item is closed or modified.
