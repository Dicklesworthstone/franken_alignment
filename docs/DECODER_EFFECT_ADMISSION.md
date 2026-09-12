# Compulsory monitored inference bound to the original effect controller

Consumer: OversightBroker, including ActorSupervisor and SupervisedDriver. This
joins FA-011/025 numerical monitoring with the existing FA-012/013 control/effect
path. It adds no executor, dependency, ledger, actor command, alternate congress,
or authority-bearing decoder output. Source and tests are uncompiled/unexecuted;
RCH qualification remains pending and no production task is closed.

## Admission and exact binding

Before any proposal, install `MonitoredDecoder::observation()` with
`enable_decoder_monitoring` and explicit `DecoderBindingLimits`. The exact owner,
full decoder profile, tokenization/model generations, stream and fixed monitor
generation stay pinned. Installing an empty source is allowed for bootstrap;
empty, computing, held, failed or closed sources cannot admit an effect.

Proposal admission captures the current original token IDs and final all-layer
quiet report directly from that source. No API accepts a copied caller-asserted
report in their place. The token IDs and next position must exactly equal the
original controller's ActorState, and declared tenant/model/tokenizer identities
must agree. Length equality alone is insufficient. The basis also freezes actor
revision and authority epoch. It never changes merely because helper inputs are
refreshed or another quiet report appears.

The existing broker checks that basis before starting review, applying a
permissive review, reserving authority and dispatching. Both automatic and
human-co-signed sends pass the same approval checks; human requests also use that
path. Current complete helper input, congress, policy witnesses, target checks,
source/topology gates, deadlines, fences and actual one-use permits remain
required. Numerical NoAlarm alone cannot reserve or send anything. The legacy
raw/tensor activation tripwire can coexist but cannot satisfy the decoder gate.

A further decoder step invalidates the old basis even when it is also quiet.
Owner loss, a monitor hold, or admitted numerical failure cannot leave a previous
quiet report eligible. Actor replacement invalidates old attempts even when its
tokens are identical. New quiet inference can support a NEW explicitly proposed
and reviewed action after the actor state is synchronized; it cannot refresh an
old permit. Cancelling an undispatched reservation uses the original ledger.

## Recovery and retention

A failed gate check performs no dispatch, consumes no human key and does not
refund a reservation. Completed restrictive reviews, explicit stopping, late
receipts and existing outcome reconciliation deliberately do not require a live
decoder or helper input. Unknown effects stay charged until terminal endpoint
evidence, not until a model becomes available again.

`decoder_evidence` exposes only the historical proposal basis.
`dispatched_decoder_evidence` first requires the original delivery record, so a
proposal or an issued permit cannot be mislabeled as a consumed dispatch basis.
The same immutable evidence survives endpoint reopening and source loss. It does
not enter publication requests, actor responses, or helper inputs implicitly.

Binding retention is cumulative: original token IDs and exact score words are
bounded, conservatively per accepted proposal even where immutable storage is
shared. Failed admission does not consume retained-evidence capacity; cancelling
or settling an accepted action does not free its historical basis for reuse.
Existing ledger bounds additionally cap record counts. These counters are not
allocator/RSS, elapsed-time, or model-quality measurements.

## Regression coverage and limits

Ten direct integration scenarios cover empty and wrong-token admission, complete
single/two-key publication, changed source during review and after reservation,
restrictive review application, lookalike source rejection, identical-token actor
replacement, fresh-action recovery, uncertain outcomes after source/reviewer loss,
retention limits and mismatched model generations. Three Unix driver scenarios
exercise actual numerical inference plus real socket helper I/O through both key
modes, decoder-loss refusal before reservation, and a file publication whose lost
acknowledgment is recovered after decoder loss and a fenced endpoint reopen.
Fixtures use deterministic parameters and helper verdicts, not trained detector
or helper qualification. No Rust scenario has run in this environment.

The source brand proves only which in-process numerical owner produced the
observations. The trusted host still supplies the ActorState and selects the
model, probe policy and action payload. This does not prove that an untrusted
actor generated the payload from that prefix, authenticate weights/probes,
validate supplied cache/sampler bytes, contain a hostile process, or bind a
physical publication's wall clock to a later model step. The final gate is at
DISPATCH: an already returned envelope remains subject to the existing endpoint
fence/stop protocol, not retroactive decoder revocation.

There is intentionally no disable or source-swap method. A held/dead owner cannot
be replaced to revive its original authority; qualified source/controller repair
is outside this increment. Observation snapshots retain the final report rather
than a full earlier-score trace. Existing gates, fixtures, manifests, lockfiles,
beads and historical execution evidence were not weakened or updated to claim
verification.
