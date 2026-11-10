# Independently anchored historical review audit

The original read-only FileReviewReplayCursor can now require an exact external
FileHistoryAnchor covering the selected completed review. This serves FA-020,
plan 7.3, 9.2, 11.1 and 16.5, and founding FI-A06/FI-A14. The named consumer is
the offline publication-review command, not a new authority or storage owner.

## Exact input before original replay

`begin_read_review_replay_anchored` checks the independently supplied profile,
canonical path identity and original complete-prefix bytes before constructing
the replay machine. The anchor must include the selected Finish record. An
anchor covering only the opening, commits or reveals is insufficient to claim
which completed application was recorded. The same frozen image supplies all
later replay events; no second read can substitute a different suffix.

The `_anchored_with_learned_generation` and
`_anchored_with_owned_learned_consistency` readers then bind the independently
supplied original numerical and predictor recipes before the first historical
transition. Their existing binders retain recipe-owned versus standalone mode
rules. No runtime model, forecast, verdict or source result is trusted merely
because its archived bytes decode. All original numerical witnesses still check.

The original cursor must finish the ENTIRE suffix before returning the original
FileReviewReplay. Matching an anchored review cannot excuse an invalid later
transition. The ordinary unanchored APIs keep their original contract and bytes;
the new callers never retry them after an anchor failure.

## Native learned review completion

The same readers recognize the original learned-sidecar Finish record as well as
ordinary Finish. They replay the actual native transition and retain its original
completed archive, application inputs, snapshot and application result. A native
review therefore remains auditable after computed identity, source and numerical
preparation, without current helper files or new observations. Anchors still
cover the selected complete record, and the entire suffix must verify.

Effect tracing follows the actual native application receipt through the original
authorization, human-key, dispatch and reconciliation events. It does not infer
execution from a favorable review or synthesize an application result.

A selected learned-sidecar `Refined` round has not attempted policy application.
The original `FileReviewReplay` API continues to return `Incomplete` for that
selected outcome. Callers can explicitly enable completed refinement audits with
`FileReviewReplayCursor::include_refinements()` before any event attempt, then
export the distinct `FileReviewCompletion::Refined` result after the whole suffix
verifies. Earlier refinement rounds still replay normally before a selected
`Applied` completion. See [completed refinement rounds](#completed-refinement-rounds).

## Claims and costs

An anchor is independently retained operator data, not a signature or evidence
that the source observation was truthful. Replacing both anchor and journal with
an old matching pair defeats this protection. A matching prefix does not
authenticate any unanchored suffix. The reviewed decision and its actual broker
application remain separate values. Neither is a new permission or a receipt
for an external effect.

Reading/decoding, prefix re-encoding/comparison, recipe binding and individual
original transitions remain synchronous and bounded by their original profiles.
At most one journal image is decoded for the audit. This is not a wall-clock,
throughput, hostile-filesystem or process-memory guarantee. No writer lock,
fence, staged-file cleanup, live sampling budget or privileged role is acquired.

## Source status

Six authored native regression functions cover three polling schedules, frozen
cuts and genuine successors, insufficient pre-Finish anchors, a valid same-counter
fork accepted by unanchored replay but rejected by anchored replay, invalid
unanchored suffixes, foreign paths, rollback and every event-cut cancellation.
The existing read-only extraction and authority tests are unchanged.

These Rust tests and compilation are UNEXECUTED in the preparation environment:
the required remote-only RCH launcher is absent. Source/hash/whitespace checks
are not runtime qualification. No bead is closed or existing execution receipt
reused for this increment. The original integrated gate remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::replay
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Offline learned operator command

The existing Linux reference operator now consumes these readers directly:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  create-learned-generated /operator/config.json /operator/recipe.json \
  --audit-review 101 /independent-custody/review.anchor
```

Despite the shared command family name, this explicit mode never creates or
recovers a publication owner. It rejects live continuation, review/stop peers,
credibility activation and successor-anchor export. The original Config parser
validates only the independently selected bootstrap data. The original recipe
loader runs with `require_native = false`: current policy-evidence contents,
live helper model files, native roster and peer sockets are not dependencies.
Primary model, tokenizer, fit, monitor and configured forecast assets still ARE
required, and they must reproduce the exact original recipe. No recipe is
inferred from the journal. The anchor is mandatory, a regular bounded original
archive outside the authority store, and is never advanced or overwritten.

One monotonic elapsed allowance from `config.timing.runtime_ms` covers config,
anchor and asset loading, historical replay and report construction. Checkpoints
run before file replay, before every original event, at Ready and before output.
Timeout discards the audit, not the live owner's authority or accounting. Initial
I/O, recipe construction, a single original event and output can block; the
checkpoints do not claim kernel-level or strict wall-clock preemption. The audit
uses no fresh logical-clock observation and cannot extend an action deadline.

Only a completely verified suffix produces a complete audit. Applied reviews
keep `fa.review-audit/1`; selected refinements use `fa.refinement-audit/1`. All integral
identifiers and counts are strings to retain full precision. A `known` wrapper
names the bounded historical-reference claim, independent prefix extent and
remaining unanchored suffix; authentic source origin and truth are not asserted.
The report includes the original policy trace (node, predicate kind, children,
truth and witness index), original per-member configured/admitted weights,
missing/abstained flags, phase counts/times and the actual application result.
A recorded Continue whose application refused remains refused. An applied denial
is a completed audit, not a failed command or a claimed external execution.

The report distinguishes historical execution counts at the review and journal
cuts and retains charged/reserved totals. These are the original journal-as-sink
projection, not authenticated receipts from a remote provider. It explicitly
carries `authority: none` and `external_receipt: not_verified`. It omits raw
payloads, helper inputs, model values and policy literals; member/cohort names
are byte-preserving UTF-8 hex, not unescaped text. This is a supervisor-only
summary, not an actor message, a portable full evidence archive or a production
assurance certificate. Independent investigation can still use the original
FileReviewReplay and its complete retained evidence.

Report writes are bounded BEFORE buffer growth to 64 KiB. No summary bytes are
emitted before complete verification and rendering. A partial stdout write or
flush failure is reported as output failure; it cannot rerun inference, dispatch
an effect, mutate the journal, remove staging or retry recovery.

Six additional authored operator regression functions use original loaded
primary models, native helper inference, sidecar views and two-key publication.
They cover ordinary/raw/owned forecast modes through the actual command router,
missing live assets, unchanged staging and live state, a real native denial,
checkpoint cancellation including Ready/pre-output, closed option/path admission,
output failure and a valid changed generation recipe. A seventh unit regression
checks exact/overflow output-buffer admission. Together with the six anchored
reader tests, thirteen regression functions are new. They remain UNEXECUTED:
operator and integrated RCH gate launches again stopped before compilation with
exit 127 because `rch` is absent. These controlled tiny models do not establish
learned detection quality, production containment or performance.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication audit
```

## 2026-10-24 execution update

All six existing operator audit functions now pass with actual native learned
completion support, including denial, all three forecast modes, wrong recipes,
output failure and cancellation. The computed-identity offline-history case and
the real refinement-to-later-application case pass too. The complete operator
test source passes standard qualified Cargo check. Runtime evidence uses standard
LLVM test harnesses with the official Cranelift native library; see the
[identity operator verification](LEARNED_IDENTITY_OPERATOR.md#verification) for
exact scope, failed attempts and the unchanged dependency-admission gate refusal.
This update does not claim execution of every earlier native audit test or full
release qualification.


## Completed refinement rounds

A helper congress may finish by asking the original learned-sidecar planner for
more evidence. Its exact residual purchase changes the retained input revision,
but it has not applied a policy consequence. This completed review is now
auditable without manufacturing a failed application receipt or a permission.

The same acknowledged-owner, canonical, anchored and independently recipe-bound
readers construct the original `FileReviewReplayCursor`. Before the first event
attempt, `include_refinements()` enables the additional completion kind. A late
opt-in refuses; default cursors keep their original refusal at a selected
refinement. The new `finish_completion()` and synchronous
`complete_completion()` expose `FileReviewCompletion`:

- `Applied(FileReviewReplay)` retains the existing archive and actual application
  result, including an application that refused.
- `Refined(FileRefinedReviewReplay)` retains the original pre-vote anchor,
  independently verified decision archive, exact refined K/V group, new input
  revision, and the state at completion and at the full journal cut. Its type
  has no application receipt.

The original machine still executes each historical transition once. The
refinement record comes from its actual `FileLearnedSidecarFinish::Refined`
result and must agree with the same one-shot archive captured from the original
session. No later input, helper response or caller-supplied group can replace
that record. Audit reconstruction does not purchase evidence in the live owner.

`finish_completion_with_effects()` additionally returns the existing
`ReviewEffectAudit` for the same attempt. A later applied review may authorize a
real dispatch; its round identity remains attached to that dispatch. Selecting
the earlier refinement cannot make `selected_review_dispatched()` true or
attribute the later approval to the refinement.

Every suffix transition must verify before either completion kind is exported.
An exact external anchor must cover the selected complete Finish; a matching
opening or vote prefix is insufficient. Recipe binding, acknowledged ending-state
comparison, cancellation, poisoned-owner refusal and numerical witness checks
are unchanged. `finish()`, `finish_with_effects()` and the legacy synchronous
review APIs still return only applied-review records.

### Qualification of this increment

Native regression sources exercise genuine learned generation and sidecar
refinement, original independent reviews, later two-key publication, frozen
cuts, anchors, suffix rejection, cancellation and compatibility. The execution
environment became unavailable during this session, before the new code was
written. These new Rust tests, compilation, rustfmt, Clippy and the canonical
RCH gate have not run. Independent source review and content-hash checks do not
qualify execution, detector quality, model alignment or production deployment.
No packet or bead is closed by this increment.


## Refinement reports through the existing operator command

The same explicit `--audit-review ROUND HISTORY_ANCHOR` mode above now accepts
a selected completed refinement. It opts in on the original anchored,
independently recipe-bound cursor before any event attempt. Original applied
reviews retain their existing `fa.review-audit/1` output and renderer.

A refinement returns `fa.refinement-audit/1` under the same historical
`Knowledge::Known` wrapper. The two schemas distinguish an original attempted
policy application from an original completed evidence refinement:

| Field in the refinement report | Meaning |
|---|---|
| `input_revision` | Input actually judged by the selected congress, before refinement. |
| `refinement.input_revision` | New original input revision created by the completed refinement. |
| `refinement.group` | Exact layer, key/value side, position and head whose residual was acquired. |
| `application.status` | `not_attempted`; no approval or failed application is invented. |
| `application_inputs` | `not_applicable`; the new input was not applied by this round. |
| `refinement.requires_fresh_review` | The refined input needed another independent review at this completion cut. |
| `attempt_effects.stage_at_review` and `stage_at_journal_cut` | Original selected-attempt stages at the two distinct historical cuts. |
| `attempt_effects.dispatch.authorization_round` | The actual later round associated with a dispatch, when one was recorded. |
| `attempt_effects.selected_review_dispatched` | Whether the original effect trace attributes that dispatch to the selected round; a refinement does not authorize it. |

The report also retains the original policy trace, congress membership and
missing/abstaining distinctions, review timing, independent anchor extent,
unanchored suffix extent, and verified/attempted replay work. Historical effect
counts remain separate from the selected-attempt trace. An absent dispatch or
an unrecorded terminal outcome does not establish nonexecution. A later review
may already have approved the refined input; `requires_fresh_review` describes
the selected refinement cut, not present eligibility.

The original 64 KiB report limit applies before buffer growth. Raw payloads,
helper input bytes, evidence values and reveal salts remain undisclosed. The
same deadline checks cover historical reconstruction and output preparation;
nothing is emitted before the full suffix verifies and the report fits.
Cancellation, wrong anchors/recipes, malformed history and output failure
produce no live recovery, helper call, publication retry or journal write.

The operator regression source exercises an actual native coarse refinement,
later independent publication, the new report schema, original applied output,
anchor/recipe refusal, cancellation and output failure. The new command path
has received independent source and format-literal review, but its Rust tests,
compilation, formatting, Clippy and required RCH gate remain unexecuted because
the execution environment is unavailable. The source review does not replace
those checks or close production qualification.
