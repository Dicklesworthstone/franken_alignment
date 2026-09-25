# Native text generation through supervised publication

## Capability and contract

The Linux `supervise_publication` executable now accepts an explicit
`--native-text RECIPE` option on `serve-create`. It creates the ORIGINAL
native-text-only durable stream, loads a bounded Llama/SafeTensors model and
native FA-BBPE tokenizer, runs compulsory activation monitoring on every prompt
and sampled token, and connects its completed generation to the original actor,
process-helper, separate human-review, publication and reconciliation workflow.
This is the existing L1/L3 numerical path feeding L5 ingress (FA-025/FA-107;
plan sections 8.7, 10.6, 11.2 and 17.1), not a new grant or actor verb.

Only a completely reviewed generation ending on a registered Control stop token
can produce a submission reference. The operator's stdout receives one ORIGINAL
Submit JSON document containing that reference, never the generated message,
logits, helper input or effect key. The actor must submit it over the configured
credential-checked socket. That request still needs original process helpers and
an independent human key; a quiet activation probe is not publication permission.

The original durable intent precedes computation. Each advancement acknowledges
at most one new-request token through the original cursor/RNG/budget reducer.
Independent stop is available before the first source read and between tokens.
Source evidence is refreshed before every advancement. The service records fresh
time again after source/control work and immediately before numerical admission;
an expired source lease cannot borrow the earlier pre-read tick. Deadline, hold,
source loss, partial output, TokenLimit and numerical failure cannot emit a source
reference. Error handling uses the original stop/drain and helper cleanup path.
Physical replay of earlier numerical history remains synchronous and may repeat;
this is not a hard-real-time stop bound or an exactly-once CPU claim.

## Invocation

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text RECIPE > native-submit.json
# In the actor process, after the complete reference line is available:
supervise_publication actor-submit ACTOR_PROFILE native-submit.json
# In the independent reviewer process:
supervise_publication review-peer REVIEWER_PROFILE REQUEST_ID
```

The CONFIG must explicitly have an empty initial payload, the registered source
and actual helper programs. Both peer profiles retain their existing strict
scope, clock, directory, credentials and finite resource checks. Stop uses the
existing `stop-peer REVIEWER_PROFILE REQUEST_ID` command. No default approval,
output substitution, implicit stream finish, credential override or legacy
publication fallback is introduced. The request deadline is fixed to the service
lifetime and is never renewed after generation. Source-reference output failure
stops the workflow; it does not send or retry the actor request.

## Recipe

```json
{
  "schema": "fa.native-text-service/1",
  "identity": {
    "tenant": 1, "model": 9, "model_generation": 1,
    "tokenizer_generation": 1, "profile_generation": 1
  },
  "context": 16, "cache_stream": 5, "generation": 7,
  "stream": {
    "id": 9, "generation": 1, "max_messages": 4,
    "max_message_bytes": 64, "max_total_bytes": 256
  },
  "prefix_controls": [256], "stop_tokens": [256],
  "max_new_tokens": 2, "max_output_bytes": 4,
  "scalar_products": 1099511627776, "sampling_entries": 16777216,
  "model_config": "model.json", "weights": "model.safetensors",
  "monitor": "monitor.json", "sampling": "sampling.json",
  "tokenizer": "tokenizer.bbpe", "prompt": "prompt.txt"
}
```

These illustrative dimensions and IDs describe the synthetic regression model,
not an arbitrary trained checkpoint. Relative paths resolve against the recipe.
The complete strict schema is checked before referred files open. All seven
inputs share a logical-byte allowance equal to the configured journal byte cap,
in addition to the original per-format bounds. This bounds retained input bytes,
not transient model allocations or complete journal size. Original constructors
negotiate model/monitor/sampler/tokenizer compatibility; no remote code or model
name lookup occurs. Tied-head checkpoints are explicitly refused because the
current durable decoder constructor has independent-head semantics. Raw prompt
bytes and explicit controls are retained; no chat template or BOS/EOS guess runs.
Full prompt plus requested continuation must fit the original context.

## Source and execution status

Five regression functions are authored, with actual native numerical computation,
source/journal files, actor/reviewer sockets and existing synthetic helper child
processes. They cover paired human approval/rejection, held generation, a quiet
but incomplete output, source loss, deadline after one acknowledged token, exact
aggregate-input limits, foreign model tenant, content stop rejection, schema
preflight before missing weights, unsupported modes and cross-role path collisions.
They are not evidence of trained detector quality or hostile-process isolation.

The existing `source_wire` module is now publicly reachable, while its admission
hooks remain crate/restricted private. This repairs the source visibility defect
in the earlier multi-peer executable's imports; it does not expose controller
handles or public replaceable admission callbacks.

RCH is unavailable here. The targeted test command failed BEFORE compilation
(exit 127); compilation, these five tests, rustfmt, Clippy and the full gate are
UNEXECUTED. Byte/whitespace review is not runtime qualification. No Beads are
closed and no historical execution receipt certifies this source addition.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --example supervise_publication native_
```

Current scope is one new native stream/request per service. Native `serve-open`,
checked-witness/multi-peer combinations and implicit numerical resume are not
accepted. The native stream's existing full-input, two-key and fresh-publication
checks remain mandatory even without the optional structured-witness extension.
The protected sink remains the original journal-as-publication reference sink;
no model-serving, package-registry, network-delivery or OS-isolation qualification
is claimed. File/clock authenticity and arithmetic-platform compatibility remain
operator assumptions. All original model, wire, journal and authority formats
and every existing test body are unchanged.
