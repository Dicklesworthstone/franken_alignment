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

## Native executable

Start the next message explicitly on an existing stream:

```sh
supervise_publication serve-open CONFIG NEXT_ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text NEXT_RECIPE --after-message PREVIOUS_REQUEST

supervise_publication serve-open-checked CONFIG NEXT_ACTOR_PROFILE \
  REVIEWER_PROFILE WITNESS_PROFILE --native-text NEXT_RECIPE \
  --after-message PREVIOUS_REQUEST
```

Use a new actor request ID and a new recipe generation ID, preserving the exact
stream, model, tokenizer and selected publication/joint policies. The prompt in
NEXT_RECIPE is additional input, not a replacement for the retained context.
Both native recipe versions (single-file or explicitly sharded) use the same
continuation path. The one new message still follows the original actor Submit,
helper, independent human-review and final-publication checks. Stdout contains
only the restricted submission reference, not unapproved generated text.

The final `--after-message` option selects fresh initiation only. It never
falls back to creation or adopts an existing generation. After the intent has
committed, recover with the same command, recipe and actor profile **without**
`--after-message`. Ordinary native open still refuses a missing intent. A
recorded request returns its original document and outcome with no re-generation
or re-publication; an expired deadline is not replaced by a new effect deadline.
An already published but unreconciled predecessor must first be reconciled via
its own original receipt path before starting another message.

The existing `--credibility-activation` option composes with native open when
requalification is needed. Its original independent evidence/predecessor checks
and stop-before-read behavior are unchanged. After qualification, continuation
obtains fresh source/time, services independent stop, explicitly resumes, and
rechecks the prepared cut before the original new-intent transaction. Token
advancement then shares the original one-token/source/control loop. A crash
between resume and the new intent does not create a generation; a crash after
intent commitment uses ordinary recovery, not a fresh ID or refreshed budget.

One invocation serves one explicitly selected next message. This is not an
autonomous conversation scheduler, implicit chat template, context-window
extension or automatic continuation past rejected/held work. Stream closing
remains an explicit separately reviewed effect.

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

The executable integration adds six regression functions and a synthetic helper
entrypoint. Its helper protocol checks each actual post-recovery policy epoch.
Tests cover two-message publication with both human outcomes under unchecked,
witness-checked and joint-policy bootstraps; interrupted new-intent recovery;
exact second-message receipts with live sources removed; late witness changes;
missing-intent/source-loss refusal; and strict public option/qualification routing.
The original numerical, source-acquisition, intake and recovery bodies remain
unchanged, as do previous test bodies. The targeted `native_continuation_` test
command and a new complete gate attempt both stopped before compilation because
`rch` is absent (127). All eleven new regressions and the synthetic entrypoint
remain UNEXECUTED; source/whitespace/hash checks are not runtime qualification.
