# Bounded monitored generation

## Status and scope

Development implementation; compilation, formatting, Clippy and execution qualification are pending. No existing execution receipt qualifies this source. The implementation adds no dependencies, runtime, tokenizer, model import format, actor command, effect capability or persistent event format.

Consumers are trusted numerical integrations using `MonitoredSampledDecoder::generate` and enforcement integrations using `OversightBroker::generate_hosted`. Both use the original decoder, sampler and compulsory all-layer residual monitor. This is CPU reference inference over the already admitted explicit decoder profile, not qualification of a trained model or deployment perimeter.

## Request contract

`GenerationRequest` contains additional original prompt token IDs, a maximum continuation length, explicit stop token IDs, and one cumulative product/sampling admission budget. It never decodes and re-tokenizes input. An empty prompt is valid only after a previously reviewed prefix. Zero continuation tokens performs monitored prefill. At most 4,096 prompt-plus-continuation steps and 256 unique stop IDs are admitted; the full prospective request must fit the actual model context.

The original whole-prefix cost estimate must fit the numerical budget before any inference. An underfunded supplied prompt is refused without leaving a newly computed quiet partial prefix. This preflight is not a second charge: each admitted step is counted once. A monitor can still hold or a numerical operation can fail during prefill; actual work is not rolled back. Continuation may end early when its remaining cumulative product or sampling allowance is exhausted.

Each sampled stop token undergoes its own complete residual review before it can stop generation. A held token takes precedence over termination. A quiet stop token consumes its context position and random draw but is omitted from returned output. It remains in the actual decoder history and in evidence later captured for an effect proposal.

## Results and failure semantics

`GenerationReport` retains only reviewed continuation IDs, starting and resulting positions, requested and reviewed prompt counts, the last applicable review, admission work, and a finish classification: token limit, stop token, request-budget exhaustion, monitor hold, or failure. Debug output omits token contents. Admitted product and vocabulary counts are not FLOPs, latency, allocated bytes or wall-clock resource bounds.

Initial malformed IDs, duplicate stops, stale predecessors, context overflow and insufficient full-prompt numerical allowance refuse before inference. After execution begins, a report may contain an already reviewed prefix and a held or failed termination. A computed-but-held sampled token retains its original draw and private model state. No held token, unreviewed logits, sampler word or fabricated permit appears in the report. A failed review clears the report's last-review projection rather than presenting an older quiet review as the failed computation's evidence.

Request-budget exhaustion is not relabelled as a monitoring alarm. Supplying a new budget for a later invocation does not replenish the monitor's fixed cumulative lifetime allowance. These synchronous APIs do not implement transport streaming, stop strings, wall-clock cancellation, automatic retries or durable generation requests.

## Enforcement-owned execution

`generate_hosted` requires the current actor revision and numerical position, preflights the complete prospective cache/sampler size and revision range, and calls the original hosted step APIs for every token. Those APIs retain consistency checks, synchronize real actor/cache/sampler state, and execute the installed automatic-stop policy. A sampled alarm can therefore suspend the original authority and cancel its undispatched work; the driver does not introduce a separate containment ledger.

Further generation invalidates historical proposal eligibility through the existing decoder-observation and actor-revision checks. Frozen historical evidence is not rewritten. New proposals capture the actual updated token history, including suppressed quiet stop tokens. Numerical output does not replace congress review, policy witnesses, two-key approval, a permit, or endpoint reconciliation. This path performs no external publication and supplies no proof of nonexecution.

## Change record and acceptance targets

The first increment added bounded prompt-to-completion execution and eight source regression tests. The second connected the shared driver to the enforcement-owned numerical host and added seven host tests. The third added complete-prompt numerical admission, independent 75/76-product boundary controls, and host evidence checks for refusal versus complete prefill; it also moved the driver into its own module. There are 17 new unit tests plus one compile-fail authority-separation example. They have not been executed in the development session.

Tests exercise comparison with the original stepwise sampler, stop-token suppression and actual-history retention, malformed input, stale revisions, context and cumulative budget boundaries, held draws, post-computation errors, real actor-state synchronization, old-proposal invalidation, mandatory congress approval, native automatic suspension, and manual suspension. A green gate would establish these declared reference cases, not statistical detector quality, provider authenticity, OS isolation, serving performance or crash durability.

The repository's required verifier remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

The original packet and Beads acceptance conditions remain unchanged and are not closed by this implementation note.
