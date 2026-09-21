# Credibility activation at the native authority boundary

## Scope and status

FA-105 implementation increment, serving FI-A08/FI-A13/FI-A18 and plan sections 9.9, 9.4, 22.4. The L8 governed activation installs L3 derived weights into the existing L4 policy authority; no new rights ledger, runtime, agent verb or production format is introduced. The existing plan and founding concordance are unchanged. This is source implementation, not qualification or closure of the full FA-105 bead.

Previously the immutable promoted policy was usable only through a separate evaluation API. `PolicyAuthority::activate_credibility` now performs the original held-out promotion and installs its capped weights into ordinary bound review sessions. `propose`, positive review application, authorization, dispatch and the existing coupled publication recheck consume the same freshness guard. No public mutable controller or Congress accessor is added.

Activation names an operation, exact sequence/epoch predecessor, authority scope, policy generation, complete actor restart profile, deployment binding and stratum. Campaign positions must already belong to that authority's sequence domain; this mapping is a trusted operator assertion, not inferred from matching integers. The original evaluator identity, held-out manifest, cohort and label authenticity assumptions remain explicit.

The activation advances the existing control sequence and revocation epoch, cancelling all undispatched attempts. Only actually reserved, undispatched units are refunded. Dispatched, unknown and terminal effects retain their accounting. Frozen prior reviews and permits cannot survive activation. No suspension or capability ceiling is widened, and no exact-policy predicate or Congress threshold changes.

The history retains each immutable promoted snapshot under a 64-activation and 65,536 aggregate case/helper observation bound. Exact retries return their original receipt without advancing the epoch, refreshing age or reinstalling old policy. Conflicting operation reuse refuses. Refreshes preserve the admitted evidence requirements, declared strata and evaluator owner, and reject regressing evidence/helper/evaluator generations. A changed exact policy or actor profile invalidates positive use; restoring the old profile or replaying the old activation does not clear invalidation. Normal actor progress under the same profile is permitted.

Evidence freshness is checked at the resulting activation sequence and at every subsequent positive boundary. Exact denials, cancellation, restrictive reviews and receipt-based settlement remain available when evidence is stale. Freshness is a control-sequence interval, not wall time or an authenticated provider-liveness measurement. Already dispatched independent external messages do not rewind.

## Validation

Eleven new native-controller tests cover working activation/dispatch, immutable stale work, conserved unknown liabilities, invalid bindings, censored labels, historical retries, expiry through real restrictive transitions, exact denial, profile restoration, policy replacement, refresh constraints, overflow and suspension. Existing controller tests are byte-for-byte unchanged.

The required command `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` was attempted and exited 127: `rch` is not installed. `cargo`, `rustc` and `rustfmt` are also absent. Compilation, formatting, Clippy and all Rust tests remain UNEXECUTED. Baseline Git blob identity, uploaded source hashes and scoped whitespace checks were performed; these are not substitutes for the Rust gate. Durable credibility recovery, evaluator authentication, independently reviewed attack campaigns and production activation remain unqualified.
