# Sharded checkpoints through native supervised publication

The `--native-text` service now accepts the explicit `fa.native-text-service/2`
recipe. Its `weights` value is an index plus an operator-selected label/path map,
not an inferred directory, glob, model download or monolithic fallback:

```json
{
  "schema": "fa.native-text-service/2",
  "weights": {
    "index": "model.safetensors.index.json",
    "shards": {
      "model-00001-of-00002.safetensors": "weights/part-one.bin",
      "model-00002-of-00002.safetensors": "weights/part-two.bin"
    }
  }
}
```

This excerpt replaces only the schema and weights fields in a complete
[native recipe](NATIVE_TEXT_PUBLICATION_SERVICE.md). All other required fields,
model/head declarations, prompt/control IDs, budgets and stream limits are
unchanged. Paths are explicit operator inputs, absolute or relative to the
recipe. The index assigns tensors to the left-hand labels; it never chooses
the right-hand filesystem paths. Version 1 still requires a single text path
and cannot silently accept this object; version 2 requires the object and
cannot silently accept a single path.

Use the existing service commands without new actor verbs:

```sh
supervise_publication serve-create-checked \
  CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE \
  --native-text RECIPE > native-submit.json

supervise_publication serve-open-checked \
  CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE \
  --native-text RECIPE > recovered-submit.json
```

The corresponding `serve-create` and `serve-open` forms also accept version 2
under their original publication contracts. The separate `create-generated`
recipe remains single-file; this addition concerns the authenticated native
actor service and its checked/recovery forms.

## Loading, recovery and authority

The complete recipe is parsed before referred files are opened. The original
Llama configuration is negotiated before the index. The original complete
index/label validator runs **before any shard opens**, including exact inventory,
explicit tied-head mode and restricted literal source labels. Each physical
file is then read through the existing bounded regular-file reader; the final
original sharded model constructor verifies tensor locations, layouts, finite
values, total data sizes and tied-head equality. No source archive is rewritten.

One byte allowance covers the recipe, model config, index, every shard, monitor,
sampler, tokenizer and prompt. The original separate index, aggregate shard-set,
model and tokenizer caps also remain enforced. A later file receives only the
unspent allowance; changing the number of files cannot multiply the budget.
This is a retained-input bound, not measured peak memory or replay throughput.

The resulting [durable sharded configuration](DURABLE_SHARDED_MODELS.md) feeds
the unchanged numerical, source, actor-intake, witness, helper, human-review and
publication owners. Recovery pins exact physical inputs before replay/fencing.
Changing index whitespace or repackaging weights is a different configuration,
even with identical numerics. Interrupted generation resumes its original
intent and conserved budgets. An already-recorded request still uses its exact
original source reference and receipt-only path, including an expired deadline;
it does not rerun inference, re-review or publish twice. Independent model files
are still needed to supply the expected configuration at open; live evidence
and producer files are not needed for a recorded receipt.

Missing shards, inconsistent assignments, unsupported layouts, monitor holds
and token-limited outputs never select another loader or bypass approval. The
input service is not OS isolation, authentication of operator-controlled files,
a new anti-rollback mechanism, or qualification for unbounded pretrained models.
Existing dimension/parameter limits and the original synchronous replay costs
remain unchanged.

## Tests and execution status

Five new regression functions use physical fixture shards whose paths differ
from their index labels, actual original numerical computation, durable files,
synthetic helper processes and actor/reviewer sockets. They cover exact aggregate
admission versus one byte less, both head modes, legacy/version-2 separation,
index refusal before a deliberately missing shard, checked creation and partial
recovery with both human decisions, exact-index recovery mismatch, expired
receipt retrieval after removing live evidence/producer files and helpers, and
held/limited nonpublication. Existing native, checked, tied and recovery test
bodies are unchanged; no synthetic result is presented as detector accuracy.

**UNEXECUTED:** fresh targeted `native_sharded_` tests and the full `xtask` gate
were attempted through `RCH_REQUIRE_REMOTE=1 rch exec`. Both stopped before
compilation because `rch` is absent (exit 127). The five service tests and six
library tests are authored but unexecuted, as are Rust compilation, rustfmt and
Clippy. Hash/whitespace inspection does not supply runtime qualification. No
Beads closure, new dependency, production activation or historical-test credit
is claimed.
