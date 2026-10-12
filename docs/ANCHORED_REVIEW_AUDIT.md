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
