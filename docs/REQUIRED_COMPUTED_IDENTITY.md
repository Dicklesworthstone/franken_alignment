# Mandatory computed identity for the durable learned owner

FA-118; plan 7.10, 7.3 and 16.4; founding synthesis FS-07.

## Selected contract

`FileLearnedConfig::with_required_computed_identity()` makes original-model
computation mandatory for identity measurements in the durable owner. This closes
the manual-frame fallback for an explicitly selected deployment. The existing
passport, identity policy and separate observer role remain required. Configure
them in `FileGuardSet.identity` and use the existing guarded learned constructor.
The original identity matcher, inference engine, authority ledger and publication
algorithms are unchanged.

The requirement is part of the independently retained exact learned recipe,
using a disjoint `FALCID` wrapper around all preceding recipe bytes. An existing
recipe without the option keeps its original bytes and trusted-manual behavior.
Wrapper order is part of exact identity, as with the existing sidecar/stop modes.
The new mode preserves text, sidecar and automatic-stop settings. It does not add
an outer journal tag or change an existing numerical witness encoding.

Before the learned generator is installed, the original identity gate must exist
and must have no earlier challenges. A manually matched, pending, expired or
withdrawn challenge cannot be grandfathered into the stronger profile. Guarded
bootstrap validates this before creating storage, and the first canonical image
contains the complete configuration. There is no live disable or late-upgrade API.

## Measurements and effects

Use the original `begin_identity_check`, then the separate observer's
`observe_computed_learned`. The original challenge fixes the full passport and
the original learned owner supplies the model. Its independently observed
manifest, whole-roster budget, pre/post-computation times and exact comparison
witness keep their existing semantics. A matched measurement still requires
`apply_identity_check`; identity eligibility never substitutes for congress,
automatic authorization, human approval, dispatch or checked publication.

Both live transactions and full-history replay reject standalone manual manifest
and anchor records in this mode. This includes byte-identical genuine captures
submitted through the old manual ingress. Only the private original computed
adapter feeds its measured frames into the original identity transitions. The
legacy token-stepped runner journals manual observations and therefore is not a
supported measurement route for this profile; it refuses rather than silently
weakening the replay claim. Legacy profiles retain that interface. The cooperative
transaction below supports strict profiles without creating manual records.

Manifest mismatch, actual anchor mismatch, native expiry and admitted numerical
failure retain their distinct original outcomes. Explicit `identity_unavailable`
is still allowed. It withdraws eligibility, not the mandatory configuration.
No missing observation or failed computation can select the old manual route.
Inference itself is not stopped merely because identity eligibility is missing;
the existing effect gates remain the boundary. A local identity incident is not
proof that an already-dispatched effect did not execute.

## Recovery and scope

Every existing learned reader/open entry point must match the independently
supplied exact recipe before replay. A weaker recipe cannot read or reopen a
strict journal. Semantic replay enforces the ingress restriction itself rather
than trusting a constructor's earlier validation. Recovery preserves the selected
mode while withdrawing old roles and matching eligibility. Fresh computation may
measure a paused owner, but cannot resume it or revive old effect keys.

This removes a fallback, not the trusted-host assumptions. The observer's manifest
commitments, declared passport, clock, source provisioning and operating-system
boundary are still trusted. Matching finite anchor intervals does not authenticate
a serving host, validate every parameter, prove arbitrary substitution detection,
or establish detector quality. Computation and original history replay remain
synchronous and bounded, not preemptible; replay costs are separate from reported
single-measurement work.

## Authored verification and status

Eight dedicated integration regressions cover actual strict identity through
sidecar/congress/two-key publication; genuine manual frames accepted in the legacy
profile but refused in the strict profile; usable computed continuation after
that refusal; missing-guard and late-challenge admission; both sidecar-wrapper
orders; same-label parameter substitution; read/reopen downgrade rejection; fresh
roles and computation while paused after recovery; exact versus insufficient work
allowance; receipt-time expiry and its timely control; and independent manifest
mismatch without anchor computation. One compile-fail example excludes disabling
the mode. Existing tests are not changed.

Required targeted and full checks were attempted through
`RCH_REQUIRE_REMOTE=1 rch exec --`. Both stop before compilation with exit 127:
`rch` is absent; Cargo, rustc and rustfmt are also absent. These tests, Rust
compilation, formatting and Clippy remain UNEXECUTED. This environment contains
selected source, not a full checkout. Preimage hashes and whitespace checks are
not execution evidence. No Bead, qualification gate or production release is
closed by this source addition.

## Cooperative preparation and atomic computed commit

`FileIdentityObserver::begin_computed_learned` consumes the separate observer,
an already-begun challenge and the fixed original input. It returns
`identity::decoder::transaction::FileComputedIdentityRun`. Setup performs original
whole-roster admission and reconstructs one private machine from the acknowledged
journal; it executes no new stimulus token and writes nothing. Failed setup returns
the original observer. Reconstruction of prior numerical history is real,
synchronous work excluded from this new measurement's token-work report.

Each `advance_with_clock` enters at most one token through the SAME original
`DecoderIdentityProbe` used by synchronous execution and replay. Pre/post-token
clock checks reject expired or backwards receipt times. The complete acknowledged
owner revision is pinned: even an unrelated intervening transaction requires a
fresh preparation, rather than guessing which state can be merged. Foreign/stale
cursor calls do no work. Work is retained before post-compute clock callbacks.
An interrupted native token is distinguished from completed work; its admitted
original bound covers any unreported partial computation.

All measurements and the private manifest comparison stay private until canonical
commit. The original begun challenge already withdrew identity eligibility, and
observer custody prevents a manual or synchronous fallback while preparation is
active. An immediate private manifest mismatch is ready for commit without token
work; it is not presented as an acknowledged incident beforehand. Only completed
original measurement/containment results become externally observable.

`commit_with_clock` is a distinct operation after `ReadyToCommit`. It performs no
new stimulus inference, rechecks the pinned original owner, samples completion
time, and applies the original complete-roster comparison/withdrawal path. One
existing tag-6 journal record acknowledges the original result. Synchronous
execution, cooperative execution and replay share numerical stepping and final
comparison code; they share the unchanged witness codec too. Intermediate clock
samples are local admission checks, not a separately replayed clock trace.
A late final commit records original expiry withdrawal, never backdated success.
The original `apply_identity_check` and both publication keys remain separate.

An admitted error closes private computation and keeps its work and observer
custody. `cancel` discards the candidate and uses original `identity_unavailable`
only if this challenge's basis is still current. It cannot withdraw a successor
basis. The observer can be taken exactly once only after acknowledged commit or
successful cancellation. Storage or final-clock failure poisons the durable owner,
returns no candidate result and cannot release the observer for an older-state
retry; exclusive original recovery withdraws that old role and challenge.
Dropping unfinished preparation performs no hidden I/O, releases its private
machine and loses its observer; it does not install matching identity or settle
an external effect. The caller must explicitly withdraw or recover as appropriate.

This is process-local cooperative preparation, not durable token-by-token resume,
preemptive scheduling or a lifetime computation escrow. Cancellation costs remain
process-local; no crash-surviving cost claim is added. The original whole-roster
and journal bounds still apply, and full-history replay still recomputes the entire
record. Existing source, manifest, OS and clock assumptions remain unchanged.

Twelve further regression functions cover exact synchronous/cooperative journal
byte parity at the same canonical path; one-token progress and unchanged live
numerical state; separate original installation and two-key publication; all
partial cancellation cuts and fresh-challenge requirements; exact setup budgets
with custody returned on refusal; stale/foreign and changed-source calls; retained
post-token clock failures; final expiry with a timely control; actual staging
failure and fresh paused recovery; original manifest/model mismatch containment;
real numerical overflow with a successful control; successor-safe cancellation
and corrupt-work-witness rejection; and final-clock interruption. Two compile-fail
examples exclude cloning or replacing the numerical owner. One first-increment
test was corrected to assert the legitimate two journal revision increments
separately from exact equality of numerical, pause and pending state. No existing
production gate or numerical assertion was weakened.

The targeted and full remote-only commands were attempted again after these
changes. Both stopped before compilation because `rch` is absent (exit 127).
The twenty regression functions and three compile-fail examples in this feature
series remain UNEXECUTED, as do Rust compilation, rustfmt and Clippy. Source-only
checks do not qualify the feature or close a Bead.

## Exact registration and reserved learned creation

`with_required_computed_identity_registration(FileIdentityRequirement)` selects
the same computed-only rule and pins the complete independently registered
passport and identity policy into the original learned recipe. Use it instead
of the older boolean-only modifier when historical readers must also verify the
expected registration. The `required_computed_identity_registration` accessor
returns that frozen requirement, never a measurement or qualification result.

The disjoint `FALCIR` wrapper reuses the original bounded identity-registration
codec, including the passport manifest, stimulus roster, exact anchor intervals
and every policy field. Existing unselected and boolean-only recipe bytes are
unchanged. Repeated or mixed computed-only modifiers refuse. Source, predictor,
sidecar and joint-credibility wrappers retain the registration; their complete
order remains part of exact recipe identity.

Native learned Enable requires the original identity gate, no prior challenge,
and an exact match with this registration before numerical installation. This
binds historical audits as well as live, receipt and terminal recovery: changing
or omitting independently expected registration cannot be hidden by an otherwise
matching model recipe. It adds no alternative identity matcher, observation
input, live policy replacement or permission to publish.

Three reserved creation adapters publish the original stream, terminal reserve,
complete guard inventory and learned configuration in one first canonical image:

- `create_guarded_with_learned_generation_reserved` for ordinary generation;
- `create_predictive_guarded_with_learned_generation_reserved` for raw prediction;
- `create_predictive_guarded_with_owned_learned_consistency_reserved` for owned
  K/V prediction.

Each validates the complete configuration before creating storage and returns
all separate original roles, including identity-observer custody and, where
selected, predictor custody. A recipe-owned source policy is installed exactly
once and must match the independent guard declaration. Predictor source mode,
configuration and work ceilings retain their original exact checks. These
constructors do not supply a source observation, matching identity, forecast,
committee vote or human approval.

The [learned operator identity envelope](LEARNED_IDENTITY_OPERATOR.md) selects
this complete registration for live publication, historical readers and resident
sequences, retaining original observer custody and cooperative measurement.

2026-10-23: all five new `learned_registered_identity` functions pass, covering
19 valid individual registration changes, legacy-byte compatibility, joint-policy
composition, bootstrap rollback, actual same-label parameter substitution with
its original two-key publication control, audit/recovery before a first identity
measurement, and all three reserved source/predictor/identity lanes. Independent
source review found no blocker, and the standard qualified compiler's library
check passes.

All twenty existing `durable_required_identity` functions also pass. Both complete
unchanged public targets execute with the qualified nightly's standard LLVM test
harnesses linked to its official-Cranelift native library; the new target also
passed as an ordinary Cranelift Cargo target. The first all-Cranelift existing
suite passed eighteen cases but let two intentional callback panics escape their
nested catches. A separate minimal unchanged catch control failed with Cranelift
and passed with LLVM, confirming the local backend limitation without modifying
any repository test, assertion, production rule or work bound. The full twenty
then passed under the LLVM harness. Both build modes use the same qualified
nightly, and runtime test threads use a 32 MiB stack. Failed attempts are retained.
This is scoped mixed-backend runtime evidence, not full LLVM library code
execution or full dependency/release qualification.
