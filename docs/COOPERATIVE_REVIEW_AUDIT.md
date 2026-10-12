# Single-pass historical congress audit

## Actual consumer and boundary

The existing `FileOversight::review_replay` and `read_review_replay` now drive
`FileReviewReplayCursor`. Their original `FileReviewReplay` result and distinction
between a congress decision and its actual application are unchanged. This serves
FA-020 decision replay and the complete evidence-closure requirement (plan 7.3,
9.2, 11.1 and 16.4; founding FI-A06/FI-A14). It adds no authority, journal event,
format, external runtime, policy change or helper inference.

`begin_review_replay` freezes a healthy owner's acknowledged events and exact
ending snapshot. `begin_read_review_replay` reads one complete canonical image
without locking or recovering the writer. Both run the original Machine once per
historical event, including the selected Finish's admission, application and
request-refresh postlude. The previous disk path replayed the whole history and
then replayed the selected prefix again; extraction manually handled Finish.

The selected original ObservedSession has one private, crate-only archive slot.
Only its existing finish writes that slot from the actual produced review. The
cursor consumes the result after Machine::apply succeeds; it does not construct
an alternate session or import an archive as an answer. Normal live sessions do
not allocate an archive slot. No authority reducer consumes this observational
slot, and no public callback or setter exposes it.

## Complete-cut discipline

A progress report counts verified and attempted original events. Each advance
names the verified predecessor and limits event attempts. Invalid/stale polling
performs no work. A native error is sticky; an unwind leaves Interrupted, never
a resumable partially changed candidate. An inner Reviewed refusal is retained
as the real historical application result, not relabeled as an approved effect.

The archive and completed-review snapshot stay private until every suffix event
also verifies. Reaching the selected Finish is not sufficient to export. Live
inspection additionally compares the complete reconstructed snapshot with the
original acknowledged cut. A later writer update does not change that frozen
historical cut or turn it into current eligibility. Only finish returns a review
record; cancellation or drop changes no journal, pending file, owner, source,
clock, effect balance, role or key. An interrupted audit is not an effect refund.

No fence, staging cleanup, reexecution of an external effect or helper invocation
occurs. The original memory-only endpoint is the sole replay endpoint. A visible
but unacknowledged canonical image remains historical evidence, not recovered
rights. The original pre-vote anchor and archive verifier remain independently
usable; reconstructed anchors do not authenticate themselves.

## Cost and source status

Initial bounded file reading/decoding and event copying remain synchronous. One
event can contain a full original numerical operation. Archive creation,
verification, snapshot construction and individual reducers are not interrupted
inside their operations. Event counts are logical work, not CPU, wall-clock,
physical I/O, memory-allocation or cancellation-latency bounds. At most one
selected review is retained; events, machine state and complete evidence inputs
retain their original bounds. No measured speedup is claimed.

Eight new native regression functions compare all result fields with the retained
previous extraction algorithm and original complete Machine snapshots. They cover
successful/missing/changed whole inputs under three quanta and both input paths,
private completed prefixes, live writer growth, unchanged staging, cancellation
at every event boundary, invalid polling, sticky suffix failure, missing/unfinished
rounds and deadline-closed missing votes. The two existing replacement-fault and
invalid-suffix tests are unchanged. Two compile-fail examples prohibit early
archive and owner access. Ballots are controlled reference inputs, not measured
helper accuracy or independent evidence of detection quality.

Rust compilation, tests, rustfmt and Clippy are UNEXECUTED in this preparation
environment. The required remote-only RCH launcher is absent; attempts stop before
compilation. Selected-source hash/whitespace checks are not runtime qualification.
No bead is closed and no prior execution receipt qualifies the new source. The
original gate must run on the full integrated checkout:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::replay
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Learned and owned-K/V journals

`begin_read_review_replay_with_learned_generation` (and its synchronous `read_`
counterpart) binds the full independently supplied `FileLearnedConfig` using the
original `bind_history` before creating the replay machine. This includes the
required raw-residual or owned predictor when it belongs to that recipe. The
previous generic disk exporter cannot hydrate these runtime recipes; the new
reader does not infer them from the archive or open a privileged recovery owner.

`begin_read_review_replay_with_owned_learned_consistency` and its synchronous
counterpart separately bind generation and predictor through the original
`bind_owned_history`. Standalone versus recipe-owned modes retain their original
rules and supplied capture importers stay forbidden. Neither API has a raw-mode
fallback, arbitrary capture map, caller-provided result or runtime callback.
Changing a declared generation or predictor refuses before any historical event
runs, even when the original first output would be the same text.

The SAME cursor reconstructs original numerical generation, compression,
monitoring, forecast sources, sequential evidence and complete sidecar review
inputs. It physically recomputes audit work; no live sampling or likelihood budget
is replenished. The actual original congress finish supplies its archive; the
actual original broker supplies its application result. Successful export requires
the complete historical suffix, not merely an apparently consistent review.

Five additional public integration functions use the existing finite learned
model and original K/V/source, sidecar, two-key publication and reconciliation
paths. Both forecast representations produce A, B and an independently forecast
finish. Audit records match the live original review and publication cuts under
two quanta and both reader forms. Controls retain a frozen first-message cut
while the writer continues, read after owner drop without deleting staged bytes,
reject changed total-generation allowance despite identical first messages,
reject a changed independent owned predictor generation, and cancel during native
history without changing live numerical state. Generic unbound replay remains a
refusal. The probability table and congress ballots are explicit fixtures, not
learned detector validation. These five tests are authored but UNEXECUTED under
the same remote-only RCH limitation; no deployment or performance claim follows.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_text_stream_durable review_audit
```
