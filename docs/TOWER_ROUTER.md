# Shared snapshot and native publication routing

`fa_reference::tower::router::TowerRouter` gives an integrating Unix host one
request entry point for the existing reference snapshot/replay facade and an
optional original durable publication session. It consumes both facades rather
than copying their state. It introduces no journal, supervisor, ticket cache,
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

The two facades must belong to the same tenant. A branch-scoped snapshot cannot
acquire a live publication route. `TowerRouter::new(snapshot)` supports snapshot
reads and the original isolated replay operations without a production port.
For a native mount, configure any same-owner precheck disclosure on the original
`PublicationTower` before construction. Its separate custodian can still revoke
the grant, and the router's weak actor port cannot keep the supervisor alive.

Routes are fixed: there is no unmount, replacement or backend fallback API.
The original scope, mount generation, finite native allowances and acquired
tickets remain inside `PublicationTower`. The snapshot retains its own basis,
read allowance and journal. Their generations are not compared or converted.

`snapshot_address()` and `publication_address()` expose the registrations.
`publication_request_address(key)` names the original native request namespace;
knowing that address does not acquire a ticket. No new request-key parser exists
in the router.

## Admission and routing

Malformed identities are bounded and normalized before route lookup. Caller
tenant, plane and authority admission also precede route lookup. Denied callers
receive the same reference refusal shape whether or not a publication mount
exists; no native observation or snapshot read unit is spent.

For an admitted request, the first matching row determines its sole backend:

| Address | Route |
|---|---|
| Exact live run mount, ignoring requested generation | Original publication facade for every verb |
| The mounted session's `request-<run>-<principal>-<branch>-<authority>-` attempt namespace | Original publication facade, even with an invalid request-key suffix |
| Any other attempt ID starting with `request-` | Route-neutral missing-observation refusal; no snapshot lookup |
| Other registered addresses | Original reference facade |

Native routing wins over conflicting imported objects for every generation and
every verb, including an unsupported verb. Missing tickets, stale generations,
revoked disclosures, unavailable owners and output errors remain native results;
none cause a second attempt against the snapshot. `get` at the native run mount
does not become a live run report: the native profile only polls acquired
request addresses, so that request retains its original refusal.

An overlapping snapshot domain address is shadowed as well: `tail`, `situation`
or another unsupported native operation at that address refuses. Hosts needing
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
Actual review, human approval, dispatch, publication and reconciliation stay with
the separately retained original supervisor.

Each admitted native `get` polls that owner. Pending, uncertain and confirmed
outcomes can therefore change between queries without a router cache. A lost
acknowledgment does not reset expenditure or choose another backend. Exact
submission retries retain the native content binding and one-shot behavior.
Native calls and native refusals do not spend snapshot read units or append
snapshot events; snapshot work retains its existing independent bounds.

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

The implementation received independent source review and passed available
rolling-nightly syntax/format checks with child-module formatting disabled.
Those checks do not establish Rust type correctness or executed behavior.
The qualified nightly and RCH runner are unavailable in this session; compiler,
Clippy, runtime tests and the canonical gate remain pending. Existing dispatcher
and deployment acceptance remains open.
