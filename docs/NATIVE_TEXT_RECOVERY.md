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
