# Native generation to controlled publication

## Source addition and implementation status

The Linux reference `supervise_publication` command now has an explicit
`create-generated CONFIG GENERATION_RECIPE REVIEWER_PROFILE` mode. It connects the
original byte tokenizer, monitored sampled decoder, durable token progress and
native-source-only stream to the existing file-source, helper-process, independent
human-review, publication and reconciliation workflow. This is executable L5
composition serving FA-025/FA-107 and plan sections 8.7, 9.11 and 17.1. It adds no
runtime, authority reducer, model kernel, actor wire format or journal tag.

The actor does not provide output bytes. The command supplies only a retained
native generation reference to `FileGeneratedTextActorPort`. The original stream
requires complete prompt review, a monitored Control stop, the current numerical
predecessor and valid complete UTF-8. A quiet partial prefix, token limit, held
computation, failed admission or exhausted budget cannot become a message. The
original cumulative frame, not the small reference envelope, determines charging.

```
cargo run --locked -p fa-reference --example supervise_publication -- \
  create-generated CONFIG GENERATION_RECIPE REVIEWER_PROFILE

# Separate authorized reviewer process, using the recipe's request ID:
cargo run --locked -p fa-reference --example supervise_publication -- \
  review-peer REVIEWER_PROFILE REQUEST_ID
```

`CONFIG` is the existing supervisor file, with an empty `initial_payload_hex` and
budgets sized for the actual model, token history, source observations and review.
`REVIEWER_PROFILE` is the existing independent peer profile; there is no unchecked
human-channel fallback. `stop-peer REVIEWER_PROFILE REQUEST_ID` uses the SAME stop
endpoint throughout generation, helper review and the approval wait.

## Recipe

All six paths are explicit absolute local paths. No downloads, inferred tokenizer,
chat template, BOS/EOS, executable deserialization or checkpoint-code import occurs.
The original Llama config negotiator, SafeTensors constructor, native FA-BBPE/1 or
FA-BBPE/2 archive reader, monitor parser and sampling parser enforce their existing
contracts. The durable decoder currently requires an independent output head;
tied-head configurations refuse rather than silently selecting different semantics.

The following shape is illustrative; identities, token IDs, bounds and files must
match the operator's actual admitted model and deployment. This is not a bundled
trained model or a declaration that these dimensions/IDs are suitable for one.

```json
{
  "schema": "fa.generated-publication/1",
  "request": 1, "generation": 7, "ttl_ms": 100000,
  "model": {
    "identity": {"tenant": 1, "model": 9, "model_generation": 1,
      "tokenizer_generation": 1, "profile_generation": 1},
    "context": 16, "stream": 5
  },
  "publication_stream": {"id": 9, "generation": 1, "max_messages": 4,
    "max_message_bytes": 64, "max_total_bytes": 256},
  "binding": {"token_ids": 16, "score_words": 1024},
  "files": {
    "model_config": "/operator/model.json",
    "weights": "/operator/model.safetensors",
    "monitor": "/operator/monitor.json",
    "sampling": "/operator/sampling.json",
    "tokenizer": "/operator/tokenizer.bbpe",
    "prompt": "/operator/prompt.txt"
  },
  "text": {
    "prefix_controls": [256], "stop_tokens": [256], "max_new_tokens": 2,
    "max_output_bytes": 64, "scalar_products": 1000000,
    "sampling_entries": 1024,
    "tokenization": {"input_bytes": 65536, "pair_lookups": 196608,
      "heap_pops": 196608}
  }
}
```

Recipes have a 16 KiB cap, strict object/field inventories and bounded token arrays.
Each file uses the original bounded regular-file reader. The weight read cap also
intersects the admitted shape and journal byte limit. These are individual logical
bounds, not a process peak-memory measurement or a reservation of future journal
space. Model files and source/clock authenticity remain operator assumptions.

## Ordering, failure and non-disclosure

The independent stop socket is provisioned before the native store. The recovery
reserve is installed in the FIRST native-only stream image, before decoder/tokenizer
work; installing it later would violate its original cutoff. Fresh registered source
capture and a post-read clock precede the text intent and EVERY incremental native
token. The original source gate decides whether the lease remains valid. Stop is
checked between tokens, and one fixed deadline covers generation and publication:
expensive inference cannot buy a new approval lifetime.

No raw generation bytes are printed. Only the original restricted actor response
is written to stdout. A source-linked proposal is admitted only after the native
completed result exists; process helpers and a separate human key are then required.
Generation/source failures run the existing stop/drain path. Failed stdout delivery
after publication neither resends the effect nor changes its acknowledged result.

This version creates ONE generation and proposes ONE complete message. It does not
emit an inferred stream-finish frame, continue an existing inference session, retry
a failed generation or substitute ordinary caller text. Existing ordinary and
multi-peer publication commands are unchanged. The sink remains the original local
journal-backed publication profile, not a remote package registry.

## Validation limits

Eleven regression functions are authored: eight workflow/route cases and three
recipe cases. They cover actual synthetic-weight inference, complete source-linked
publication with separate approval versus rejection, monitor hold and token-limit
non-disclosure, missing source, exact post-read lease expiry versus a one-tick-inside
control, content-stop refusal, existing-store preservation, independent stop and
stdout failure after a committed effect. Recipe tests use actual local files and
exercise exact model/tokenizer binding plus strict syntax and unsupported sharing.

The required RCH attempt failed BEFORE compilation (`rch: command not found`,
exit 127). Compilation, all eleven tests, rustfmt, Clippy and the complete gate are
UNEXECUTED. No historical execution receipt qualifies these changes. Same-process
peer tests and synthetic helper choices do not establish trained detector quality,
OS isolation, model authenticity or an independent human evaluation. Native replay
and synchronous file I/O retain their existing cost; this is not a throughput,
Asupersync admission or hard-real-time result. No Beads or production gate is closed.
