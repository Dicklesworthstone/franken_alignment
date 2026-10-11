# Owned-K/V forecasts drive the original checked text continuation

## Capability and source boundary

`FileConsistencyObserver::begin_owned_pre_output_request` now returns a checked
process-local generation handle after the original owned-K/V forecast transaction
acknowledges a complete prediction. It accepts only the existing owner, journal
revision, request, actor revision and position. It does not accept a source row,
model, probability, output token, clock override or effect key.

The existing shared machine check admits the original complete prompt boundary
or the exact acknowledged successor in an independently pinned
`with_forecast_text_stream_continuation()` recipe. That preceding integration
remains unchanged: original receipt, source-offset, stop-audit, numerical-history,
request, deadline and lifetime accounting checks are not replaced. Legacy recipes
are not upgraded and no journal, recipe encoding or numerical algorithm changes.

This is the owned-source counterpart of the existing raw pre-output handle. It
serves the FA-111/FI-A12 pre-output prediction path and the FA-113 native sidecar
consumer. Native committee inference and both publication keys remain separate.

## Typed outcomes and preserved evidence

The outer Result describes journal/preflight failure. An acknowledged attempt
returns `FileOwnedPreOutputStart` with one of three outcomes:

- `Ready` contains the non-cloneable `FileOwnedPreOutputForecast`.
- `Inconclusive` retains the full original learned forecast report, including its actual numerical work and uncertainty, but no generation handle.
- `Refused` records an acknowledged native error. The original host retains its accounting and coverage state.

The handle exposes the original learned prediction and complete acquisition
report as immutable historical observations. A `LearnedPrediction` is not a raw
`Prediction`; no raw frame is fabricated to fit the other accessor. The original
`FilePreOutputForecast` has a defaulted prediction type parameter so raw callers
keep their existing type and accessors. Its common generation/currentness methods
are independent of prediction representation. The owned handle composes that
same implementation with its genuine learned prediction and original report.

Each `advance` rechecks original owner identity, the requested journal revision,
the complete last acknowledged numerical state, retained request/deadline,
coverage, policy epoch and elapsed time before the original intent/completion
writes. It neither adopts another caller's generation step nor restarts a failed
or held source. The predictor's lifetime allowances are not replenished.

Dropping the handle leaves the unanswered durable forecast in place. It creates
no observation, refund, authority or new observer role. Pending-forecast recovery
keeps the original coverage-loss behavior. The handle cannot be deserialized,
cloned, moved to a different owner or converted into an effect permit.

## Native integrated controls

The existing durable finite-stream test target now additionally drives both raw
and owned handles through the original history-sensitive A/stop/B/stop actor.
The native helper reads each complete original sidecar packet, runs its own
monitored model, and commits/reveals its parsed verdict. Each message and finish
still requires a fresh human key and original receipt reconciliation. An original
uninterrupted generator compares accepted tokens, numerical work and telemetry.
Canonical replay retains numerical and lifetime likelihood history.

Additional native controls cover wrong successor requests, late reforecasting,
finish-versus-message binding, recovery before the next forecast and foreign
observer custody. The concurrent forecast implementation, its legacy refusal
test and its synthetic-congress regression module are preserved verbatim.
Tiny fixed model weights and neutral probabilities establish only contract
controls, not calibration, useful detector quality or helper independence.

## Verification and change record

Added the owned pre-output handle and typed acknowledged-start result; reused the
original continuation checks through a defaulted prediction type. Added native
end-to-end tests and two compile-fail boundaries. No new dependency, executor,
CLI verb, journal event or configuration encoding is introduced.

Required targeted and full RCH commands were attempted against the selected
source preparation. Both stop before compilation: `rch` is absent, exit 127.
Cargo, rustc and rustfmt are absent too. Rust compilation, tests, formatting and
Clippy remain UNEXECUTED. Selected-source exact preimage/blob and whitespace
checks are not runtime qualification. No Bead is closed and no historical
receipt or concurrent source review qualifies this increment.
