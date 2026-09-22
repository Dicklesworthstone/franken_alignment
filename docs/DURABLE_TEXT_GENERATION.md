# Durable byte-exact monitored text generation

## Status

2026-09-22: unqualified source implementation. Twenty Rust regressions are
AUTHORED, NOT EXECUTED: eleven whole-request cases and nine incremental cases.
Both targeted attempts failed before compilation because `rch` is unavailable
(exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference decoder::text
```

Rust compilation, rustfmt, Clippy and the complete gate remain unexecuted.
Historical receipts do not qualify this source. No production bead is closed.
This advances the FA-025 original-input/capture and FA-014 recovery contracts,
not foundation admission, provider authentication or production authorization.

## One text identity, one original numerical owner

`FileOversight::enable_decoder_tokenizer` retains the exact native ByteBpe archive
against the already installed FileDecoderConfig. It requires a fresh ready
numerical state and no actor requests or pending generation. There is no tokenizer
replacement or reset-to-a-different-tokenizer operation. A tokenizer imported via
the existing strict Hugging Face subset uses the same native archive thereafter;
this change does not broaden that importer's supported transformations.

`FileTextGenerationCommand` retains original prompt bytes, explicit prefix and
stop IDs, tokenization/generation budgets, output capacity and original numerical
predecessor. `begin_decoder_text` compiles the input through the SAME preparation
function as TextDecoder and commits the text plus derived original-ID intent in
one journal cut. It calls the original generation reservation reducer, not a
parallel pending-work ledger. No new-request token computes at this boundary.

`generate_decoder_text` delegates execution, comparison witnesses, held output,
RNG advancement and result persistence to the existing generate_decoder path.
There are normally two cuts: text intent, then the ORIGINAL numerical result.
Output is decoded only from that result's released IDs after acknowledgment;
native refusal, Held, Failed and BudgetExhausted remain distinct. Invalid UTF-8
stays exact bytes, not replacement characters. A returned text report is neither
a helper judgment nor authority to publish. The effect ledger is unchanged.

Exact completed retries are read-only, even with stale revision/clock arguments.
Changing any text request field conflicts, including a budget that would produce
the same token IDs. Bare-ID generation cannot be retroactively labeled as text.
Original-ID numerical entry points remain monitored and retain their own receipt
shape; only an admitted text intent supports a text receipt for that ID.

## Recovery and limits

`open_with_text_decoder` checks independently supplied exact model, monitor,
sampler AND tokenizer bytes before numerical replay, cleanup or a recovery write.
The same exclusive Store supplies the input for every check. The original fence
withdraws old approvals and pauses inference. A pending text intent remains
pending; fresh time and explicit original decoder resume are still required.
An acknowledged result can be read with decoder_text_generation without inference.
The opener has the existing numeric-owner/human-reviewer scope, not the separate
composed guarded-role inventory, predictive contract or history-anchor profile.

Tokenizer archives are capped at 8 MiB. Retained original prompt/control/stop
bytes across all text intents are capped at 2 MiB; completed or refused requests
do not refund this bound. The original 128-generation and requested-step caps,
whole-prompt numerical admission, output-capacity admission and canonical journal
bounds still apply. Tokenizer installation consumes ordinary journal capacity;
install any intended recovery reserve beforehand. These are logical retained-data
bounds, not total allocations, physical replay cost, disk reservation or latency.
Shared preparation may run again during admission, replay and historical reads;
per-prompt tokenization counts are not a cumulative physical-work measurement.

New decoder tags 9 (native tokenizer archive) and 10 (text intent) extend the
existing family without changing tags 0 through 8 or numerical result witnesses.
Older readers refuse the new tags. No Cargo dependency, imported cache, unchecked
numerical state, alternate sampler, effect key, source-authentication claim or
external publication transaction is introduced. Journals include sensitive raw
input; the new Debug implementations omit prompt and output content.

## Authored regressions

The tests construct real SafeTensors-backed analytic weights, original residual
monitors and the original sampler inside a real locked journal. They compare
manual prompt IDs with output bytes, exact cache/sampler replay bytes and work.
They cover complete/pending restart, equal-profile tokenizer substitution, exact
and conflicting retries, native admission refusals, withheld alarm tokens,
control stops, invalid UTF-8, source interruption and one-under/exact event slots.
Both intent and result writes exercise all five original storage failure barriers:
no speculative output may escape; recovered logical draws follow only the actual
canonical cut. Codec tests retain existing tags and reject truncated new records.
These are test assertions awaiting execution, not measured production guarantees.


## Resumable bytes and read-only inspection

`advance_decoder_text` acknowledges at most one original monitored token using
advance_decoder_generation. No second cursor, numeric owner, budget or journal
event is added. The already retained text command is recompiled and checked
against its original numerical intent; output capacity is allocated before the
native advancement. Source, clock, native witness, first-step capacity and
poisoning rules still apply. A partial cursor cannot return to whole-request
execution from its old predecessor; after recovery it requires the original
explicit fresh-clock resume and continues with its retained budget.

`FileTextGenerationProgress` retains the ORIGINAL FileGenerationProgress and
cumulative bytes. Pending empty output, a native admission refusal, a stop token
and a held/failed finish remain distinct. A refused numerical result produces
an error from bytes(), not a successful empty string. An unexpected decode
failure keeps the acknowledged numerical progress/counters accessible rather
than discarding committed work. The progress is supervisor evidence, not a
completed-report fabrication or publication permission.

Old generation revisions return current progress without another token or
write. `delta_from` extracts only bytes after an explicit request-local byte
cursor and returns their exact range; retrying after retaining that range's end
returns an empty suffix. Cursors ahead of a snapshot refuse without arithmetic
wrap. Byte boundaries inside a Unicode character or merged token are valid;
utf8() remains strict for the whole prefix. A consumer owns its actual delivery,
idempotency and durable cursor acknowledgment. No exactly-once external-output
claim follows from this API, and a generation revision is not a byte offset.

`pending_decoder_text` discovers a pending original text intent. None does not
rule out a pending bare-ID request; the original pending_decoder_generation API
retains that distinction. `decoder_text_progress` projects the healthy owner's
acknowledged cut without inference or source/clock operations.

`read_decoder_text_progress` reads the complete canonical image and uses the
same private exact model/monitor/sampler/tokenizer pin as open_with_text_decoder
BEFORE numerical replay. It returns no lock, mutable owner, fresh role, cleanup,
fence or resume. Its snapshot explicitly describes a canonical image: it may be
newer than the selected generation, and a visible post-rename image does not
establish that an earlier failed directory sync was acknowledged. Subsequent
work still requires exclusive recovery. Model/tokenizer identity and stored
observations remain independently trusted inputs, not authenticated data.

The nine additional authored regressions exercise whole/incremental cache and
work parity, stale-response byte cursors, all five request-progress boundaries
across reopen, actual sampled UTF-8 fragments, merged-token partial cursors,
monitor holds, stop controls, native budget refusal, source interruption,
non-text ID exclusion, exact/one-under first-step event slots, all five original
storage barriers immediately before output acknowledgment, a post-restart exhausted
sampling allowance, strict configuration
pins and corrupted native progress witnesses. They use the original real
SafeTensors/monitor/sampler/journal paths with analytic fixture parameters, not
pretrained-model or production evidence. All tests remain unexecuted.
