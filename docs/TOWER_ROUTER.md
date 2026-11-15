# Shared snapshot and native publication routing

`fa_reference::tower::router::TowerRouter` gives an integrating Unix host one
request entry point for the existing reference snapshot/replay facade and up to
64 original durable publication sessions in the same tenant. It consumes the
facades rather than copying their state. It introduces no journal, supervisor, ticket cache,
permit, source observation, or conversion between native and global clocks.

The consumer is the host already provisioning `ReferenceTower`,
`PublicationTower` and authenticated caller grants. This closes the need for
that host to choose a backend for each request, including the unsafe fallback
case where a lost live owner could expose a stale imported object instead.
It does not implement the entire registered vocabulary, a `fa` executable,
network authentication, live global situation reconstruction or signed receipts.

`exchange(access, document)` accepts the same requests through the strict,
bounded [JSON request codec](TOWER_REQUEST.md). The whole document is validated
before route lookup. A malformed request receives a normalized route-independent
error without spending either facade's allowance or echoing partial input.

## Provision the original owners once

```rust
use fa_reference::tower::{ReferenceTower, TowerAccess, TowerInstallError, TowerRequest};
use fa_reference::tower::publication::PublicationTower;
use fa_reference::tower::router::{RoutedResponse, TowerRouter};

fn connect(
    snapshot: ReferenceTower,
    publication: PublicationTower,
) -> Result<TowerRouter, TowerInstallError> {
    TowerRouter::with_publication(snapshot, publication)
}

fn request(
    router: &mut TowerRouter,
    access: &TowerAccess,
    request: TowerRequest,
) -> RoutedResponse {
    router.dispatch(access, request)
}
```

For multiple runs, call
`TowerRouter::with_publications(snapshot, Vec<PublicationTower>)`. The existing
single-owner constructor and snapshot-only constructor retain their behavior.

Every facade must belong to the snapshot's tenant. A branch-scoped snapshot cannot
acquire a live publication route. `TowerRouter::new(snapshot)` supports snapshot
reads and the original isolated replay operations without a production port.
For a native mount, configure any same-owner precheck disclosure or separately
granted supervisor inspection port on the original `PublicationTower` before
construction. Their separate custodians can still revoke the grants, and the
router's weak ports cannot keep the supervisor alive.

The host must authenticate the caller and select each original session in that
caller's audience before construction. `TowerAccess` grants tenant/plane/purpose
access; it does not authenticate a principal for each run. A host cannot treat
same-tenant membership alone as permission to mount another principal's owner.
Each mounted native facade still requires its original, separately retained
grants for operator disclosure or control. The router adds no such grants.

`with_publications` accepts zero through `MAX_PUBLICATION_MOUNTS` (64) facades.
An empty set is equivalent to a snapshot-only router, including for a branch.
The constructor rejects excess mounts, foreign tenants and any duplicate
tenant/run registration before serving a request or reading a native owner.
Generation, principal, branch or authority differences do not make two mounts
of the same run unambiguous: the run address itself omits those scope fields,
so such aliases are rejected. Duplicate detection is independent of input
order and the owners' current availability.

Routes are fixed: there is no unmount, replacement or backend fallback API.
The original scope, mount generation, finite native allowances and acquired
tickets remain inside `PublicationTower`. The snapshot retains its own basis,
read allowance and journal. Their generations are not compared or converted.

`snapshot_address()` exposes the imported registration and
`publication_addresses()` enumerates the native registrations in canonical
address order. These methods return host configuration, not observed liveness.
The legacy `publication_address()` returns a registration only for exactly one
mount, and `publication_request_address(key)` preserves its single-owner behavior.
With multiple mounts that naming method returns `InvalidAddress`; it never
silently picks the first run. Use
`publication_request_address_at(&exact_registration, key)` to name a request
under an explicit native owner. A missing or changed generation refuses, and a
foreign registration cannot resolve. Knowing an address does not acquire a
ticket. No new request-key parser exists in the router.

## Admission and routing

Malformed identities are bounded and normalized before route lookup. Caller
tenant, plane and authority admission also precede route lookup. Denied callers
receive the same reference refusal shape whether or not a publication mount
exists; no native observation or snapshot read unit is spent.

For an admitted request, the first matching row determines its sole backend:

| Address | Route |
|---|---|
| Exact live run mount, ignoring requested generation | That original publication facade for every verb |
| A mounted session's `request-<run>-<principal>-<branch>-<authority>-` attempt namespace | That original publication facade, even with an invalid request-key suffix |
| Any other attempt ID starting with `request-` | Route-neutral missing-observation refusal; no snapshot lookup |
| Other registered addresses | Original reference facade |

Native routing wins over conflicting imported objects for every generation and
every verb, including an unsupported verb. Missing tickets, stale generations,
revoked disclosures, unavailable owners and output errors remain native results;
none cause a second attempt against the snapshot. `get` at the native run mount
does not become a live run report: the native profile only polls acquired
request addresses, so that request retains its original refusal.

An overlapping snapshot domain address is shadowed as well. Native `situation`
requires its own installed supervisor inspection port and authenticated operator
role; it never substitutes the imported snapshot's report. `tail` and other
unsupported native operations at that address refuse. Hosts needing
separately accessible snapshot-domain reads must provision a distinct original
snapshot domain address. The router never renames an imported domain or presents
its historical evidence as the live owner's current state. Related snapshot
projections retain their original snapshot basis; following a native request
address performs a fresh native poll.

## Original authority, observations and retries

The existing native checks still govern `propose`: allowed Permit purpose,
exact explicit registration generation, original effect scope, canonical numeric
idempotency key, fresh admission snapshot and complete proposal shape. A successful
precheck creates no admission snapshot, request ticket or publication key.
`capabilities` uses the same original disclosure and shared query allowance;
its informational native response creates neither a grant nor a signed profile.
Actual review, human approval, dispatch, publication and reconciliation stay with
the separately retained original supervisor.

With an explicitly installed same-owner inspection grant, native `situation`
captures that owner's fresh acknowledged report. It preserves the original
native basis, full-capture accounting and explicit first-page completeness.
Query bounds and revocation remain shared by the original grant; caller scope
and the separate operator role are checked before any capture. Owner loss,
revocation or exhaustion never falls back to an imported situation. See the
[native inspection contract](TOWER_PUBLICATION.md#inspect-the-original-live-owners-situation).

Each admitted native `get` polls that owner. Pending, uncertain and confirmed
outcomes can therefore change between queries without a router cache. A lost
acknowledgment does not reset expenditure or choose another backend. Exact
submission retries retain the native content binding and one-shot behavior.
Native calls and native refusals do not spend snapshot read units or append
snapshot events; snapshot work retains its existing independent bounds.

Those properties also hold between native runs. Identical request, query,
round or operation numbers in distinct owners remain unrelated. A known
publication in one run cannot supply a ticket or settle uncertainty in another.
Precheck and journal-tail grants retain each owner's original finite allowance,
and a cursor from one owner cannot select or retire another owner's continuation.
Dropping an owner leaves its run and request namespaces reserved; the sibling
owners continue normally. Their journals, control sequences, costs and source
frontiers are never combined into a synthetic fleet-wide observation.

`RoutedResponse` preserves either the complete `TowerResponse` or the complete
`PublicationResponse`. Its `to_json()` directly delegates to the original
encoder, retaining exactly sixteen envelope fields, native result profiles,
response limits and nested Knowledge. There is no JSON round trip or synthesized
common `Basis`. Native global generation, control sequence, budgets, costs and
frontiers remain Unknown or Withheld exactly as in the native facade.

## Regression and qualification boundary

Six integration functions in `tests/tower_router.rs` pair real permitted work
with causal negative controls. The publication fixture uses the original
file-source `EvidenceSnapshot::inputs_for`, commit/reveal, automatic key, human
key, publication and reconciliation. Other controls prove that readable imported
decoys cannot replace native results, compare occupied/unoccupied-route refusals,
preserve admission and read allowances, retain routes after revocation/owner
drop, and prohibit branch/foreign-tenant live mounts.

The initial single-owner increment received independent source review and
syntax/format checks. On 2026-11-15, local `cargo check` typechecked the router
integration target and the publication example, and all twelve router tests
executed successfully (12 passed, 0 failed). The installed dated toolchain was
`nightly-2026-08-31`; Cranelift, single backend concurrency and unused-function
code-generation hints kept the executable build within this workspace's
resources. No repository compiler profile, admission gate or test assertion was
relaxed. Commands, captured test output and limitations are recorded in
`artifacts/execution/2026-11-15-router-stop-verification.log`.

That execution covered `8a1abac` plus this multi-owner router increment. The
subsequent native-precheck and service-composition commits on main were preserved;
the execution workspace became unavailable before the combined final tree could
be rerun. The repository's qualified nightly, RCH execution, Clippy and canonical
full gate remain unverified. Existing dispatcher and deployment acceptance stays
open.

The multi-owner increment adds six integration functions in
`tests/tower_router/multiple.rs`: two original source-backed publications using
the same request key; exact registration navigation; duplicate run, branch,
tenant and mount-bound rejection; acceptance at the exact 64-owner limit;
independent precheck quotas and journal continuations; and loss of one owner
without fallback or interference with its sibling. Publication controls use
actual durable native owners, source refresh, independently committed reviews,
two keys and reconciliation. They do not establish multi-process authentication,
cross-domain atomicity, detector quality or qualified deployment. The scoped
twelve-test execution above does not close the full dispatcher acceptance.
