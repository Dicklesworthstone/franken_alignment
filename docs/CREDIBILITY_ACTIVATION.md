# Credibility activation at the native authority boundary

## Scope and status

FA-105 implementation increment, serving FI-A08/FI-A13/FI-A18 and plan sections 9.9, 9.4, 22.4. The L8 governed activation installs L3 derived weights into the existing L4 policy authority; no new rights ledger, runtime, agent verb or production format is introduced. The existing plan and founding concordance are unchanged. This is source implementation, not qualification or closure of the full FA-105 bead.

Previously the immutable promoted policy was usable only through a separate evaluation API. `PolicyAuthority::activate_credibility` now performs the original held-out promotion and installs its capped weights into ordinary bound review sessions. `propose`, positive review application, authorization, dispatch and the existing coupled publication recheck consume the same freshness guard. No public mutable controller or Congress accessor is added.

Activation names an operation, exact sequence/epoch predecessor, authority scope, policy generation, complete actor restart profile, deployment binding and stratum. Campaign positions must already belong to that authority's sequence domain; this mapping is a trusted operator assertion, not inferred from matching integers. The original evaluator identity, held-out manifest, cohort and label authenticity assumptions remain explicit.

The activation advances the existing control sequence and revocation epoch, cancelling all undispatched attempts. Only actually reserved, undispatched units are refunded. Dispatched, unknown and terminal effects retain their accounting. Frozen prior reviews and permits cannot survive activation. No suspension or capability ceiling is widened, and no exact-policy predicate or Congress threshold changes.

The history retains each immutable promoted snapshot under a 64-activation and 65,536 aggregate case/helper observation bound. Exact retries return their original receipt without advancing the epoch, refreshing age or reinstalling old policy. Conflicting operation reuse refuses. Refreshes preserve the admitted evidence requirements, declared strata and evaluator owner, and reject regressing evidence/helper/evaluator generations. A changed exact policy invalidates positive use. The existing host still rejects actor-profile replacement; normal actor progress under the same profile is permitted. Qualification remains bound to the complete original profile.

Evidence freshness is checked at the resulting activation sequence and at every subsequent positive boundary. Exact denials, cancellation, restrictive reviews and receipt-based settlement remain available when evidence is stale. Freshness is a control-sequence interval, not wall time or an authenticated provider-liveness measurement. Already dispatched independent external messages do not rewind.

## Evidence withdrawal and endpoint fencing

`PolicyAuthority::withdraw_credibility` is the explicit trusted notification that the admitted evidence is unavailable. It validates an operation and exact sequence/epoch predecessor, advances the original authority, cancels pending work and retains dispatched liabilities. Missing evidence does not become a new evaluator label or an accepted model-profile substitution. Each activation retains at most one withdrawal. Exact retries return their historical receipt; they cannot withdraw a later fresh activation. Restoring actor bytes, replaying an old activation or advancing time cannot reopen the lost qualification. A new explicit promotion must pass the original requirements.

`DeliveryBroker::activate_credibility` and `withdraw_credibility` expose these native transitions through the existing delivery owner. Broker-side overflow and stop-state failures are checked before changing the authority. Successful fresh transitions advance the dispatcher epoch and invalidate its fence acknowledgment. New sends require the original endpoint's acknowledgment of the new fence. Historical activation/withdrawal retries do not re-fence, acknowledge a fence, roll back a generation or change accounting.

A local transition alone does not stop an already sent envelope: it can still execute before the endpoint installs the fence. After installation, the old envelope is stale. A missing status still supplies no nonexecution proof. The original endpoint's seal returns its actual earlier execution receipt or seals nonexecution; only that receipt can settle the original charge. This same behavior is used with the existing memory and Unix filesystem publication endpoints. No alternate endpoint, request encoding or settlement path is introduced.

These APIs are not yet wired into durable `FileDelivery`/`FileOversight` activation replay or the supervisor CLI. The filesystem integration test covers the existing endpoint's payload, terminal receipts and fence, not whole-process recovery of credibility evidence or authority. No new production activation is claimed.

## Validation

Twenty-one new tests are authored: thirteen native-controller tests and eight public delivery integration tests, including one Unix filesystem test. Coverage includes working activation/review/dispatch, retained unknown liabilities, stale work, invalid bindings, censored labels, historical retries, expiry through real restrictive transitions, exact denial, immutable-profile rejection, policy replacement, evidence withdrawal, refresh constraints, overflow and suspension. Endpoint cases pair successful publication with stale-fence/expired-evidence refusals and distinguish execution before fence installation from rejection afterward.

Integration review caught an incorrect new test assumption that actor profiles could be replaced. The original host's rejection was preserved; the authored test now asserts that rejection and exercises explicit evidence withdrawal. The pre-existing controller test block is byte-for-byte unchanged, and all original delivery methods are preserved.

The required command `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` was attempted and exited 127: `rch` is not installed. `cargo`, `rustc` and `rustfmt` are also absent. Compilation, formatting, Clippy and all Rust tests remain UNEXECUTED. Baseline Git blob identity, uploaded source hashes and scoped whitespace checks were performed; these are not substitutes for the Rust gate. Durable credibility recovery, evaluator authentication, independently reviewed attack campaigns and production activation remain unqualified.
