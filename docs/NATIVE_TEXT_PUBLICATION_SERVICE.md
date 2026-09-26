# Native text generation through supervised publication

## Capability and contract

The Linux `supervise_publication` executable now accepts an explicit
`--native-text RECIPE` option on `serve-create` and explicit `serve-open`
recovery. The `serve-create-checked` and `serve-open-checked` forms additionally
require a witness profile; see [checked native publication](NATIVE_CHECKED_PUBLICATION.md).
Creation establishes the ORIGINAL
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
lifetime for a first proposal. A recorded request's deadline is never renewed.
Source-reference output failure stops new-work workflows; it does not send or
retry the actor request. Receipt-only recovery does not issue Stop on output
failure, because doing so could affect unrelated later work.

## Explicit recovery

```sh
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text RECIPE > recovered-submit.json
```

Keep the original recipe, model/tokenizer files and registered profile. Recovery
uses the [exact native bootstrap opener](NATIVE_TEXT_RECOVERY.md), verifies the
stored source policy and reserve, and compares every original text-request field,
including prompt bytes and all budgets. It never falls back to create, introduces
a new generation ID, replaces a prompt, replenishes work or changes the sampler.
A journal without the original text intent refuses rather than starting over.
Original stale socket paths are not deleted automatically.

For a pending generation, the explicit open command obtains fresh source and time,
services independent stop, and invokes the original resume on the reconstructed
predecessor. It then shares creation's one-token/control/source loop. A completed
but unsubmitted generation must have its original Control stop and still match
the current numerical position/revision. Another outstanding generation, even
before its first token, cannot be skipped. Held, cancelled, failed and TokenLimit
results are not restart candidates. Completion still only emits an unapproved
source reference; authenticated actor submission, helpers and independent human
approval remain mandatory before any first publication.

For an already-recorded source-linked actor request, recovery instead emits the
EXACT original Submit document, including its old target, policy epoch and
deadline, and performs only original outcome reconciliation and actor observation.
It does not refresh evidence, resume inference, launch helpers, open a review/stop
listener, request a new human key or retry publication. An expired original
request can therefore retrieve its retained outcome without becoming a new effect.
The new service lifetime bounds receipt delivery, not the old permit. A failed
receipt output neither resends the effect nor issues a new Stop consequence.

A first submission after interrupted, unsubmitted generation uses a new explicit
service lifetime, but this grants no authority and renews no recorded request or
permit. An unsubmitted reference emitted by an earlier process remains unapproved;
the first accepted actor request fixes the durable source identity. Recovery is
not an authenticated latest-head or anti-rollback guarantee. Existing anchored
and guarded-role deployments still require their separately composed contracts.

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
name lookup occurs. The model configuration's explicit `tie_word_embeddings`
declaration now selects the original independent or tied-head loader through
[the durable head contract](TIED_HEAD_DURABLE_DECODER.md). An omitted head is
accepted only for declared tying; a physically stored head must match embedding
bits exactly. Recovery pins this interpretation even for numerically equivalent
archives. Raw prompt bytes and explicit controls are retained; no chat template
or BOS/EOS guess runs.
Full prompt plus requested continuation must fit the original context.

## Source and execution status

### Creation baseline

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

Current scope is one native stream/request per service, either creation or the
explicit recovery above, including the explicit checked-witness forms. Multi-peer,
joint-held-out and implicit numerical-resume combinations remain unsupported.
The native stream's existing full-input, two-key and fresh-publication
checks remain mandatory even without the optional structured-witness extension.
The protected sink remains the original journal-as-publication reference sink;
no model-serving, package-registry, network-delivery or OS-isolation qualification
is claimed. File/clock authenticity and arithmetic-platform compatibility remain
operator assumptions. All original model, wire, journal and authority formats
remain unchanged.

### Recovery change record — 2026-09-25

Seven additional regression functions exercise actual original computation and
canonical files, partial and completed-but-unsubmitted continuation with both
human outcomes, exact recipe mismatches, missing intent, held/limited work, source
loss, expired published-request retries without helper programs or evidence,
broken receipt output, and preservation of a later zero-token intent. They use
the existing synthetic model and same-process credential/helper fixtures, not a
trained detector or hostile-process isolation test. The existing unsupported-mode
test now rejects `serve-open-checked` instead of newly supported `serve-open`;
its other cases and the four creation regression functions are unchanged.

These seven tests and the five exact-bootstrap recovery tests are **UNEXECUTED**.
A fresh required RCH gate attempt again failed before compilation (`rch` missing,
exit 127). No Rust compilation, rustfmt, Clippy or test pass is claimed. No Beads
closure or production qualification follows from source or whitespace review.

### Checked-native integration — 2026-09-25

The checked forms install native and witness contracts atomically and pass the
selected capture/producer profile into the original preparation, helper review,
human approval and publication driver. Missing or changed evidence cannot
fallback to the unchecked route. Checked receipt recovery remains source-free.
Five new service regression functions exercise producer-backed creation and
interrupted continuation, both human decisions, late phantom/disjoint changes
under both history policies, expired receipts with source files removed, missing
producers and strict scope/argument preflight. Six library tests cover the
bootstrap/recovery composition. All eleven remain **UNEXECUTED**: RCH is absent,
and fresh gate/test invocations stopped before compilation with exit 127. The
earlier test bodies are unchanged; their missing-witness-profile checked forms
remain invalid and are not implicit requests for an unchecked mode.

### Tied-head model serving — 2026-09-25

Both native-service recipes and the separate `create-generated` recipe now pass
the original Llama configuration's output-head declaration into durable model
construction. No recipe flag guesses or overrides it. The legacy independent
format remains unchanged; tied models use decoder-event subtag 12 and older
readers refuse that new tag. All monitor, source, witness, actor and human
requirements stay in the original workflow.

Four new native-service regression functions use real tied synthetic weights,
including checked creation and partial/completed generation recovery with both
human decisions, exact input bounds, contradictory physical heads, receipt-only
recovery and mode-only substitution, plus held/limited nonpublication. The
separate generated-recipe consumer adds a numerical test and replaces its old
blanket-tied-refusal case with paired missing-weights/real-tied-weights checks;
foreign-tenant preflight remains tested. Its actor port import now uses the
existing public re-export rather than the private implementation module.

These five added tests, the six durable tests and the adapted recipe test are
UNEXECUTED. Fresh targeted and full RCH commands failed before compilation
because `rch` is absent (exit 127). No Rust test, formatting or Clippy pass and
no production or trained-checkpoint qualification is claimed.
