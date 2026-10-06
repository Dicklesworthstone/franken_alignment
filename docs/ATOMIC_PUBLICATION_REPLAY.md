# Acknowledged replay reuse in atomic publication

Source implementation, not execution-qualified. The ordinary transaction reuse
already on main is preserved. This completes the fixed publication-cut part of
the pending transaction-replay patch without replacing that implementation or
moving cache retention into generic persistence/recovery.

`commit_publication_cut` now consumes the same private `replay_control_candidate`
used by ordinary controls. An existing learned replay cache supplies its actual
acknowledged predecessor; every missing original event is still executed by the
original reducer. A cold owner remains cold, including startup and cooperative
recovery. There is no imported state, public event executor or second cache.

The original fixed cuts, all source/key/capacity preflights, original event
encoding and replacement failure boundaries are unchanged. In particular,
Time -> Dispatch -> PublishChecked -> Reconcile still has one canonical
acknowledgment. No intermediate approval consumption, outcome or refund leaves
that operation. Storage failure returns neither a new result nor a continuation.

After successful replacement, a warm cut retains the actual former live machine
with the journal length BEFORE the entire batch. All four publication records,
or both query-only settlement records, must therefore be replayed before a later
preparation can become Ready. An incomplete or stale preparation cannot finalize.
Discarding the preparation loses only the optimization; ordinary cold controls
do not silently seed it again. A later original learned step can warm it normally.

Query-only settlement preserves a live source-interruption latch and an unknown
dispatch charge. Neither replay reuse nor cancellation proves nonexecution.
Recovery still replays the independent recipe and canonical history, fences old
rights and pauses generation. Foreign human keys still refuse before cache use.

Four authored regression functions cover exact original four-event state and
journal bytes, required batch-tail replay and premature finish, all five write
barriers and old-or-complete recovery, source-interrupted unknown settlement,
and foreign-key refusal before consuming the optimization. They use the existing
numerical/congress/two-key fixture; no existing test assertions are changed.
Synthetic parameters are controls, not detector-effectiveness evidence.

Required targeted RCH and full xtask startup attempts stopped before compilation:
`rch` is absent (exit 127), and the full workspace is not mounted here. Tests,
compilation, rustfmt and Clippy remain unexecuted. No local compiler fallback,
performance measurement, qualification or bead closure is claimed. Whole-journal
encoding, each original numerical event and filesystem replacement remain
synchronous, and the existing additional-machine memory tradeoff remains.

## Policy-file regressions retained from the pending patch

Three additional regression functions use actual operator files and the original
learned-text configuration. They compare the reused state with independent
canonical decode/bind/replay, check expiring leases and fresh renewal, preserve
a newer refused producer version across later transactions and recovery, and
exercise all five policy-write barriers. A genuinely newer complete observation
is the positive control for subsequent original request admission. The tests
preserve the current cold-owner rule after a failed proposal consumes the cache;
losing the optimization does not lose source history or restore authority.
Together with the batch cases, seven functions were ported from the pending
bundle. These are source scenarios only; no execution result is claimed.
