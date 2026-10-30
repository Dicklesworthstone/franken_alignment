# Recipe-bound learned publication situations

This extends the existing [publication situation](PUBLICATION_SITUATION.md)
consumer for FA-136 / plan 17.8 / FI-A02 and FI-A18. A learned publication can
now be inspected through its original numerical history without opening a
recovered owner, issuing roles or appending a recovery fence.

## Native selection

Use `FileOversight::begin_read_situation_with_learned_generation` with the
independently selected `FileLearnedConfig` for plain learned generation or a
recipe-owned raw pre-output predictor. For owned-K/V prediction, use
`begin_read_situation_with_owned_learned_consistency` and supply both the original
generator and `FileLearnedConsistencyConfig`. The returned `FileSituationReplay`
is the existing bounded cursor; advance it, then consume it with `finish`, or
use its synchronous `complete` convenience method.

Both entry points read one bounded canonical image. Optional independent history
custody checks the exact prefix of that same image before the original
`bind_history` or `bind_owned_history` routine binds runtime recipes. Neither
model dimensions nor a similar-looking configuration replace exact identity.
Missing, changed or mode-incompatible recipes fail at binding or original replay;
there is no retry through a plain or weaker reader. The original no-recipe
situation reader remains available for ordinary histories and cannot reconstruct
learned runtime recipes from saved assertions.

Every event, including numerical operations, predictive observations, unfinished
intents and suffix events, must validate through the original machine. Saved
numerical witnesses remain comparison material. Replay may recompute historical
model work in private memory; it does not advance the live sampler, create a new
forecast, finish a pending intent or perform a new production effect. Inference
cost is not bounded by event count alone: one original event remains synchronous.
Callers retain responsibility for cooperative elapsed checks around each step.

The result uses the original historical `Knowledge` basis, conserved accounting,
lifecycle ordering and immutable pagination. It exports no live model, sampler,
predictor, observation lease, role, helper session or permit. An unreconciled
publication remains unresolved even when its bytes are already visible; only
original receipt reconciliation changes its disposition. Reports captured before
that reconciliation do not silently follow later live state.

A matching history prefix does not authenticate a later suffix, and a recorded
clock is not current source eligibility. This is privileged supervisor inspection,
not an actor disclosure grant, production qualification or detector-quality claim.
The report still covers publication attempts/accounting rather than every
numerical work item or the complete fleet situation/planner contract.

## Verification status

Eight new integration regression functions exercise actual bounded learned-model
histories: every prompt/sample/stop cut, raw and owned pre-output predictions,
two-key dispatch and publication before/after reconciliation, wrong or absent
recipes, cancellation at every event boundary, anchored successors/rollback and
foreign custody, pending numerical intents, wrong bootstrap and malformed suffix.
The existing numerical fixture and original publication/review constructors are
used; synthetic weights and fixture ballots do not establish empirical safety.

These tests are authored, not executed in this editing environment. Targeted
RCH testing and the canonical check cannot start because `rch` is absent; Cargo
formatting also cannot start because `cargo` is absent (exit 127). Whitespace and
source-preservation review are not compilation, runtime tests or qualification.
No bead or full FA-136 acceptance condition is closed.
