# Native text generation through supervised publication

Explicit native-archive and supported Hugging Face tokenizer JSON selection is
available in recipe versions `/3` and `/4`; see
[operator tokenizer intake](OPERATOR_TOKENIZER_INTAKE.md). Existing `/1` and `/2`
recipes retain their native-archive path meaning.

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
At the final stamped clock, the service explicitly calls the original
`capture_file_policy_state` gate before generation, resume or finish preparation;
clock acknowledgment alone does not establish source freshness.
Source-bound reopen now checkpoints between original historical events, as
described under [cooperative recovery](#cooperative-reopen-and-continue-stop).
Each event, new-work transaction and final persistence remains synchronous;
physical numerical replay may repeat across separate recovery attempts. This is
not a hard-real-time stop bound or an exactly-once CPU claim.

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

## Exact policy prechecks before submission

Select one bounded exact-policy disclosure alongside the native recipe:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text RECIPE --precheck-profile PRECHECK_PROFILE > native-submit.json
# Prepare native-precheck.json with the query wrapper described below.
supervise_publication actor-precheck ACTOR_PROFILE PRECHECK_PROFILE native-precheck.json
# Submit the original source reference separately, with independent review:
supervise_publication actor-submit ACTOR_PROFILE native-submit.json
```

The same selection is available on `serve-open`, `serve-create-checked` and
`serve-open-checked`; checked forms retain their required `WITNESS_PROFILE`
positional argument. Open forms can select one explicit continuation with
`--after-message PREVIOUS_REQUEST` or Finish with
`--finish-after-message PREVIOUS_REQUEST`. Finish uses the preceding confirmed
message's original recipe and performs no additional generation. The optional
independent operator remains available through the pair
`--operator-profile OPERATOR_PROFILE --tower-generation NONZERO`. Option pairs
may appear in any order after the positional arguments; duplicate or
incompatible modes are rejected before configuration and numerical assets open.
For example, a separately reviewed Finish with both optional endpoints uses:

```sh
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE \
  --native-text ORIGINAL_RECIPE --finish-after-message PREVIOUS_REQUEST \
  --precheck-profile PRECHECK_PROFILE --operator-profile OPERATOR_PROFILE \
  --tower-generation 41 > finish-submit.json
```

The [live precheck profile](LIVE_ACTOR_PRECHECK.md#supervisor-opt-in) must be
uncached `fa.actor-precheck-service/1` with
`policy_disclosure: "exact_predicates"`. Cached version two and full-policy
version three are unsupported for this native selection. Omitting the option
leaves the existing native service without this endpoint or additional disclosure
bootstrap. Request schedules, multi-peer, topology and governor combinations do
not acquire native precheck support through this option.

The actor command takes a **precheck query**, not the emitted Submit JSON. Build
`native-precheck.json` using the strict wire fields:

| Field | Value for the native query |
| --- | --- |
| `schema`, `operation` | `"fa.actor-precheck/1"`, `"precheck"` |
| `query` | An independently chosen nonzero decimal-string label |
| `target`, `payload_hex`, `units`, `expected_policy_epoch` | The original proposal fields from the emitted native Submit document |
| `deadline` | The emitted deadline or a shorter one |

Do not copy the Submit envelope's `version`, `operation: "submit"` or outer
`request` field into this query. The fixed 32-byte payload already binds the
selected source request, generation and generation revision, or explicit Finish.
The existing [actor-only command](LIVE_ACTOR_PRECHECK.md#actor-only-command)
authenticates the supervisor and validates the response against the independently
selected epoch and disclosure ID. That profile must name the actual owner epoch
after recovery; neither profile nor proposal is rewritten to repair a mismatch.
The native Submit already carries the actual epoch, rather than the learned
stream's zero-valued envelope marker.

Disclosure begins only after the original Message has completed its monitored
generation, or after explicit Finish preparation has checked the confirmed
prefix. The same native builders bind the actual cumulative frame, including all
prior messages, boundaries, target and full resource charge. Exact predicates
evaluate that action rather than the 32-byte source reference. A query cannot
replace bytes, change a Message to Finish, select another source identity or
extend the deadline. Generated output and predicate details stay private.

Bootstrap captures fresh registered evidence and clock state and can append
their original journal events. Queries perform no additional inference,
sampling, source reads, clock observation, journal changes, helper review or
human approval. Passing the query does not consume the one-use intake snapshot
or admit the actor request. Actual Submit reacquires fresh source evidence,
checks the original generation and cumulative frame, and still requires both
helper and independent human review keys. A later source change or checked
publication witness can therefore prevent the effect after a pass.

Before model loading, time observation or durable startup, canonical path checks
separate the precheck listener from the actor, reviewer, Stop and optional
operator endpoints, registered source and authority store. Existing paths are
never removed to make startup succeed. Independent Stop runs before disclosure
bootstrap and each live intake turn. The sole disclosure custodian retires
before review, on owner or lease/clock invalidation, on service exit, or after
terminal sidecar framing/transport failure. Revocation withdraws every port
copy's access to the finite grant. A sidecar-only failure does not invoke native
Stop/drain or grant a fresh allowance, and ordinary valid actor submission can
continue.

An already-recorded native Message or Finish cannot acquire a new disclosure on
open. Selecting precheck for that historical receipt workflow refuses after
exact canonical recovery identifies it, before source acquisition, new inference,
precheck listener creation or review. The ordinary recovery Fence remains part
of its original owner contract. Recover the receipt without the precheck option;
its old target, epoch, deadline and effect outcome remain unchanged.

## Independent live operator

Native services can select the same authenticated Tower operator profile used
by other publication services. The actor continues to use its original
source-only wire and independently submit the generated reference:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text RECIPE --operator-profile OPERATOR_PROFILE \
  --tower-generation 41 > native-submit.json
```

The two operator options must appear together. They are also accepted by the
checked create/open forms and explicit native continuation or Finish. Their
complete grammar is validated before configuration or numerical assets open;
duplicates, mixed modes and an absent, zero or malformed route generation
refuse. The generation identifies the Tower route, not the model, journal cut
or publication authority epoch. The existing
[live operator contract](LIVE_TOWER_OPERATOR.md) supplies the profile, peer
authentication, request codec and client commands.

The supervisor provisions the original finite grants on a restricted
[generated-text Tower mount](TOWER_PUBLICATION.md#operate-an-original-native-generated-text-stream).
An operator can inspect the actual live situation and original journal history,
diagnose recorded holds and request separately granted historical explanations
or policy comparisons. Explicitly selected receipt reconciliation, undispatched
request cancellation, Unknown abandonment and admission fencing retain their
original independent grants. Every selected control that reconstructs numerical
history must declare its separate cumulative `numerical_replay_events`
allowance. Ordinary target-ceiling narrowing remains unsupported. A read grant
without sufficient numerical work reports its original uncertainty; it cannot
borrow the generation budget.

One operator listener, authentication session and set of grants remain owned
through qualification, initial or resumed generation, actor intake, helper and
human review, publication and bounded final reply delivery. Existing Stop
checkpoints run first. The service then pumps the selected operator and observes
any acknowledged Fence through the original one-use Stop cleanup before further
source, token or effect work. Reconnecting does not restore spent queries,
operations or replay work, and operator disconnection does not itself issue Stop.
New-work transactions and independently budgeted operator history queries retain
their original synchronous work contracts. Cooperative reopen checkpoints do
not interrupt one of those operations or add a hard real-time preemption guarantee.

The operator endpoint must be distinct from actor, reviewer and Stop sockets,
the registered evidence source and the authority store, including aliases
through existing filesystem ancestors. Neither operator observations nor stdout
release unapproved generated text. Source refresh, native monitoring, the
original helpers, fresh independent human approval and both publication keys
remain required for an effect.

An open that resolves to an already-recorded source-linked request accepts only
observation operator grants. Selecting any mutation for that receipt workflow
refuses explicitly after exact canonical recovery identifies it and before
operator listener/grant creation. It is never silently downgraded. The historical
path opens no Stop/reviewer listener, starts no new generation or helpers and never
turns a failed receipt output into a new Stop. Its sidecar can only observe the
original history while the original bounded actor receipt exchange completes.

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

### Cooperative reopen and Continue Stop

All native open modes, including checked publication, use the source-bound
`FileGeneratedSourceRecovery` cursor. The service checks its existing logical
and wall deadline before opening the canonical image, before each one-event
replay quantum and again at `Ready`. It validates the fixed total and completed
predecessor on every step. A deadline failure before finalization releases the
private reconstruction without an ordinary recovery Fence or a partial live
owner. Initial image read/decode, each individual event and final
encoding/persistence remain synchronous, so the checkpoints bound event calls
between opportunities to observe control, not elapsed execution time.

Explicit `--after-message PREVIOUS_REQUEST` declares new continuation work. Its
independent authenticated Stop endpoint is therefore created before historical
numerical replay begins. The same existing candidate allowance, authenticated
peer/session state and deadline survive every replay checkpoint and, when no
peer selects the terminal path, continue into the normal live workflow. Recovery
does not create a second allowance or replace the session after reconstruction.
For example, the independent peer can use the existing command while this
service is replaying:

```sh
supervise_publication serve-open CONFIG NEXT_ACTOR_PROFILE REVIEWER_PROFILE \
  --native-text NEXT_RECIPE --after-message PREVIOUS_REQUEST > next-submit.json
# In the independent reviewer process, using NEXT_ACTOR_PROFILE's request ID:
supervise_publication stop-peer REVIEWER_PROFILE NEXT_REQUEST_ID
```

A pending authenticated peer selects the terminal branch, including at the
checkpoint after the last event. The complete original history and exact
model/tokenizer/source/publication/reserve contracts must still pass before the
sealed Stop target is available. The target consumes that same verified machine
and locked image; it does not reconstruct numerical history again or first
return a live owner. Only the complete original nonce-bound request can append
the original admission Stop/Fence pair. Source acquisition, helper/reviewer
launch, new generation and Submit output have not begun. Unknown liabilities
remain charged and endpoint drain remains separate.

Once a peer selects this terminal branch, silence, disconnection, malformed
input or deadline expiry cannot fall through to ordinary startup or cause an
unrequested Stop. Acknowledged Stop and a lost acknowledgment remain terminal;
the workflow does not retry new work or infer settlement. When no peer selects
the branch, `finish()` publishes the one ordinary recovery Fence over the same
verified machine, then normal fresh evidence/time and explicit resume apply.

The early Stop listener is limited to explicit Continue. Plain `serve-open` and
`--finish-after-message` can identify an already-recorded historical request,
so they use cooperative deadline checkpoints without an early Stop listener.
An actual new-work path creates its normal Stop control after classification.
Receipt-only opens retain their original absence of Stop/reviewer listeners and
new generation, and failed receipt output cannot issue Stop. Historical replay
is still required to verify their original canonical image.

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

### Cooperative native recovery — 2026-11-15

Ten library regression functions cover exact original replay/finalization,
immutable profile bindings, cancellation and stale quanta, failures and all
storage barriers, retained Unknown liability and authenticated terminal Stop.
Five service regression functions additionally cover one-event/deadline
checkpoints, first/middle/final-Ready Stop selection, a silent connected peer
without fallback, and explicit continuation Stop before missing live evidence
or helper programs can be used. Their actual numerical models and protocol
fixtures are synthetic; they do not establish detector quality or a latency
guarantee. Existing receipt-only tests retain the historical no-Stop contract.

These sources have independent static review. Targeted dated-rustfmt processing
and whitespace checks are recorded for the library; required RCH is unavailable,
so compilation, runtime execution, Clippy and full qualification of this
increment remain pending. No runtime pass or Beads closure is inferred from
authored tests or earlier execution records.

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

### Native exact-precheck integration

Library regressions exercise actual native generation followed by exact checks
and two-key Message/Finish publication; full cumulative frame byte/charge
denial; immutable source binding and shorter deadlines; finite queries,
revocation and owner replacement; incomplete generation; and fresh changed
source admission after a pass. Live service regressions cover actual native
creation with independent human approval/rejection, changed source after a pass,
Stop during a partial query, sidecar transport failure and endpoint alias
preflight. Parser regressions cover explicit open/checked continuation and
Finish selections and reject incompatible options. Synthetic model and helper
fixtures test contract enforcement, not trained detector quality.

Central Rust syntax checks passed for this increment. Compilation, Clippy and
runtime regressions remain pending the required remote verifier because `rch`
is absent in the editing environment. The historical execution statements below
remain unchanged; syntax review does not certify runtime behavior or close the
corresponding feature gate.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --test generated_actor_precheck
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --example supervise_publication native::tests::precheck_tests
```

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
