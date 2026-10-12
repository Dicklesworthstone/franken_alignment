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

Only a completely verified suffix produces `fa.review-audit/1` JSON. All integral
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
