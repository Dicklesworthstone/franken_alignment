# Native stream continuation

## New-message preparation

`FileOversight::prepare_decoder_text_continuation(after_request, generation, request)`
prepares a new native message after a specific receipt-confirmed publication. It
returns the existing `FileTextGenerationCommand`, not a permit or executable
owner. `begin_decoder_text_continuation(revision, after_request, command)` rechecks
the same cut and delegates to the original durable text-intent transaction.

The predecessor must be a source-linked native request with the original
`Executed` receipt and `Confirmed` disposition. Its complete frame, including
all previous message boundaries, must equal the current confirmed stream; the
published and confirmed cuts must agree. The stream must be unfinished and have
no pending publication. A model result alone, cancelled request, older message,
lost publication acknowledgment or ordinary caller-text stream cannot qualify.

The numerical actor revision and position must still equal the predecessor's
actual completed native result. No pending generation may be skipped. The new
ID must be unused. The original tokenizer compiles the additional raw prompt;
the whole prompt plus requested continuation must fit the remaining context.
The declared maximum output must fit the remaining stream byte allowance and
message limit. The original numerical and monitoring budgets still control
execution; preparation does not guarantee sufficient runtime evidence or work.

Preparation is read-only and works while recovery is paused. Beginning still
requires original qualification, source/time admission and explicit resume.
No reset, prompt reconstruction, implicit chat template, BOS/EOS inference,
parameter replacement or sampler reseeding occurs. The additional prompt is
appended to the existing numerical context, not to a newly initialized model.
Old monitoring and numerical spend survives; the new request's explicit work
budget does not replenish a previous request or a lifetime ceiling.

Every new publication is still admitted through `submit_decoder_text_message`.
Its original stream builder includes the entire confirmed message history and
charges the complete frame. Helpers, an independent human key and all selected
witness/freshness/joint gates remain necessary. Producing the next text is not
publishing it. No stream finish is inferred from model EOS or process exit.

## Crash and retry boundary

Continuation is explicit **new-intent initiation**. Once the text intent has
committed, repeat its original `begin_decoder_text` command or use existing
native recovery. Calling continuation preparation with that generation again
refuses instead of creating another request or rebasing its predecessor.
The original journal retains the exact command/predecessor and its progress;
this convenience API does not introduce a separate continuation-edge format,
idempotency book, reset mechanism or automatic recovery loop.

## Source and verification

2026-09-26: added preparation and rechecked initiation plus five regression
functions. Tests use the original synthetic model, actual numerical methods,
canonical-file operations, congress and independent reviewer role. They cover
two-message history and full-frame charging, missing/unreconciled/cancelled
publication, paused recovery, duplicate and outstanding generations, stale
prepared cuts, content stop rejection and remaining context/disclosure limits.
The change implements complete-message continuation under plan §8.7 and the
existing recovery contracts, not a new authority rule or production adapter.

The fresh required `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask
-- check` attempt stopped before compilation (`rch` unavailable, exit 127).
These five tests, Rust compilation, rustfmt and Clippy are UNEXECUTED. Local files
are selected-source preparation, not a complete checkout. No prior execution
receipt qualifies this addition and no Beads item is closed.
