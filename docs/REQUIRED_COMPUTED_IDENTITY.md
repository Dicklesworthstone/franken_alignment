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
weakening the replay claim. Legacy profiles retain that cooperative interface.

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
