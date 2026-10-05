# Reusing learned history during ordinary control transactions

Source implementation, not execution-qualified. This extends the existing
acknowledged replay continuation under plan sections 6.3–6.5, 10.6 and 18.3.
The consumer is the original FileOversight transaction path used by clock
observations, actor intake, policy-source refresh, helper commit/reveal, human
approval, dispatch, checked publication and reconciliation.

## Existing cache, original reducer, original commit boundary

Once normal learned generation seeds its private replay cache, ordinary
transactions now consume that same cache and run every missing event through
the original Machine reducer. They then execute their new event under the
unchanged preflights and store protocol. Only an acknowledged canonical write
replaces the cache with the actual former live machine. Consecutive warm
transactions therefore replay one original tail event, rather than the entire
numerical history. No model, witness comparison, policy gate or journal tag is
replaced, and no second cache or public prepared-event executor is introduced.

The first cold path still reconstructs the full journal. Cold control operations
do not independently seed a cache: explicit cache clearing and cold numerical
preparation retain their previous behavior. Control updates can advance a warm
cache's prefix, but they cannot advance the numerical position unless the
original event itself does so. Independent explicit continuations still replay
all events after their own prefix before either learned finalizer can commit.

## Loss, failure, and recovery

Original live-origin, revision, source and admission checks still precede cache
use. Some evidence-loss operations deliberately invalidate the live owner before
replay; internal reconstruction permits this phase without clearing the latch.
Only the original successful acknowledgment clears that fault. Invalid cached
ownership, bad prefix bounds and a failed tail witness return an error without
retrying cold within the same call. Failed application discards the candidate,
not the original pending operation; failed storage exposes no candidate result.

Generic persist_candidate is deliberately unchanged. Cooperative learned
recovery temporarily owns an empty placeholder alongside a fully replayed
candidate. That placeholder is NOT an acknowledged predecessor and must never
seed a replay cache. Recovery remains cold, verifies the independently supplied
recipe and full canonical history, fences old keys, preserves unknown-effect
charges, and pauses inference. A new owner cannot adopt another owner's cache.

At most the existing one cached Machine is retained. The live owner alone owns
the store and effect authority. The cache remains opaque, process-local and
unserialized. clear_learned_replay_cache still changes no journal, source,
request, numerical budget, approval, liability or recovery semantics.

## Validation and bounds

Seven new regression functions use the original numerical and authority fixture,
including real journal replacements and cold original replay. They cover exact
records through review and both publication keys; cache release and recovery's
placeholder boundary; stale/source preflights and failed application; mismatched
private cache custody and corrupt unverified witnesses; all five write barriers;
fencing and reconciliation of executed versus unknown effects; and preservation
of the original pre-application loss latch. Synthetic models test mechanics, not
detector effectiveness. Two existing private cache-position assertions now
require advancement by the exact two committed control records, with additional
one-event-tail assertions; their numerical, policy and liability checks remain.

Targeted RCH tests and the full mandatory gate were attempted but could not
launch because rch is absent (exit 127). The full workspace is not available in
this environment. Compilation, tests, rustfmt and Clippy are unexecuted; no local
compiler fallback or qualification is claimed. This is a structural reduction
in historical reconstruction, not an executed latency/throughput benchmark.

Individual events, full-journal encoding and storage replacement remain
synchronous. Specialized transaction paths not explicitly using this helper can
still replay from scratch and extend an older cached prefix. There is no
aggregate memory quota, physical-computation escrow, cancellation-latency bound
or progress guarantee under unbounded concurrent control changes.
