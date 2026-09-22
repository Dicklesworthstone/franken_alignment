# Durable byte-exact monitored text generation

## Status

2026-09-22: unqualified source implementation. Eleven Rust regressions are
AUTHORED, NOT EXECUTED. The targeted command failed before compilation because
`rch` is unavailable (exit 127):

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
