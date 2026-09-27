# Durable source-bound learned sidecar review

This connects the original learned sidecar to the original FileOversight journal
and two-key publication boundary (FA-113; plan 9.2, 10.3, 10.15 and 16.4).
It does not replace inference, the planner, the congress or the rights ledger.

## Required configuration and initial packet

`FileLearnedConfig::with_required_sidecar` consumes a trusted numeric or text
recipe and binds the mandatory original sidecar gate into its exact bytes.
Existing recipe bytes remain unchanged unless this option is selected. The new
wrapper is disjoint from both legacy numeric and native-text configurations.
Every existing learned recovery entry point must match the independently held
recipe before replay. A journal requiring sidecars cannot be opened with the
corresponding optional recipe. There is no live disable or late-enable method.

The existing guarded bootstrap can publish the requirement and learned recipe
in the first canonical image. After original generation and proposal,
`FileOversight::begin_learned_sidecar` records only bounded disclosure choices.
Replay calls the original broker's source-bound constructor on the exact learned
evidence already retained for that proposal; no caller-supplied source, second
compression or recapture is used. The original input recorder installs the
packet, cumulative input charge and mandatory revision marker. Only the existing
canonical replacement acknowledges the operation. A failed write returns no
packet and leaves the owner unavailable pending exclusive recovery.

`retained_learned_sidecar` returns acknowledged historical data, not permission
or a claim of freshness. Missing clocks, paused recovery and pending numerical
intents block new preparation. Source advancement and input withdrawal invalidate
permitting reviews through the original gates. A manual text-only or even
byte-identical packet cannot install the original provenance marker. Each attempt
gets only one original planner; withdrawal cannot refill its budget.

The existing durable commit/reveal, review application, human-key and final
publication APIs consume the packet without special permission shortcuts. Saved
packets survive as historical data, but recovery withdraws old approvals and
cancels undispatched attempts through the original fence. A fresh attempt still
needs a fresh original sidecar. Reconciliation of dispatched/unknown liabilities
remains independent of new review availability.

Learned-event subtag 4 records the new sidecar commands, with preparation at its
subtag 0. All previous learned and outer journal encodings remain unchanged.
The independently retained bootstrap, cooperative writer exclusion and original
source provenance remain assumptions. This is not cryptographic authentication,
remote effect recovery or a claim that the numerical probes detect harm.
Latest-position K/V remains latest-position evidence, not a full-prefix trace.

## Verification status

Eight integration tests and one compile-fail boundary are authored: original
packet into two-key publication, forged-equivalent/manual inputs, stale source,
requirement-preserving recovery and old-key rejection, withdrawn inputs,
invalid/underfunded preparation, pending numerical intents and legacy behavior.
The fixtures execute original model/codec/probe algorithms when run; helper
ballots are synthetic controls, not independent model judgments.

The targeted durable_learned_sidecar test and full xtask gate were attempted
through RCH_REQUIRE_REMOTE=1 rch exec. Both stopped before compilation because
rch is unavailable (exit 127). Cargo, rustc and rustfmt are also absent. Rust
compilation, tests, rustfmt and Clippy are UNEXECUTED. This preparation contains
selected source, not a complete checkout. No Bead or qualification gate is closed.
