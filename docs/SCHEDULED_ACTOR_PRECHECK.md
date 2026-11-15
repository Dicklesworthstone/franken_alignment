# Precheck disclosure for a finite ordinary Tower schedule

An ordinary Tower creation can explicitly select a separate precheck lifetime
before each new request in its finite `--requests` schedule:

```sh
target/debug/examples/supervise_publication serve-create \
  /srv/fa/config.json /srv/fa/actor.json /srv/fa/reviewer.json \
  --tower-generation 11 --requests 7,8 \
  --precheck-schedule /srv/fa/precheck-schedule.json
```

A simple checked creation adds the original witness profile as the fourth
positional file:

```sh
target/debug/examples/supervise_publication serve-create-checked \
  /srv/fa/config.json /srv/fa/actor.json /srv/fa/reviewer.json \
  /srv/fa/witness.json --tower-generation 11 --requests 7,8 \
  --precheck-schedule /srv/fa/precheck-schedule.json
```

Both forms also accept the existing independent `--operator-profile` and
original creation `--topology-profile`. The original topology source and its
spent work follow the same owner through every final publication. The witness
profile must pass the existing `simple_limits` admission; richer producer, feed,
snapshot, wait and joint profiles remain unsupported in this composition.

## Explicit selection for every request

The new schedule is a closed, bounded document:

```json
{
  "schema": "fa.actor-precheck-schedule/1",
  "entries": [
    {
      "request": 7,
      "precheck_profile": "/srv/fa/precheck-7.json"
    },
    {
      "request": 8,
      "precheck_profile": null
    }
  ]
}
```

Every row uses the exact numeric request key from `--requests`, in the same order.
There must be 1 through 64 distinct, nonzero keys, and the first must be the actor
profile's request. Missing rows, reordered keys, unknown fields, duplicate keys,
noninteger keys and implicit defaults are refused. Profile paths must be
absolute file paths without parent traversal. The schedule is bounded to
512 KiB; each referenced service profile keeps its existing 16 KiB limit.

An explicit `null` selects no precheck bootstrap, source capture, cache, grant or
listener for that row. The ordinary original request path still needs its own
fresh evidence when the actor actually proposes an action.

A selected path contains an existing `fa.actor-precheck-service/1`, `/2` or
`/3` profile. Versions 1 and 2 disclose exact predicates; version 2 also selects
a bounded native cache. Version 3 selects full policy disclosure, its explicit
cache limits and the original complete leakage declaration. The schedule
neither changes these schemas nor infers a broader disclosure.

For example, a first-row exact-predicate profile can contain:

```json
{
  "schema": "fa.actor-precheck-service/1",
  "policy_disclosure": "exact_predicates",
  "socket": "/srv/fa/actors/precheck-7.sock",
  "expected_policy_epoch": 0,
  "disclosure_id": 81,
  "query_limit": 2,
  "exchange_limit": 8,
  "candidate_limit": 4,
  "connection_limit": 4,
  "runtime_ms": 15000
}
```

If request 8 also needs precheck, select a second complete profile with a distinct
socket and disclosure ID. Each selection is independent operator custody.
The existing actor-side precheck commands use the same actor identity and that
row's original service profile. The request key selects the service lifetime;
the ordinary native disclosure itself binds the original owner, scope and
revision, rather than embedding the schedule's numeric request key.

## Conserved bounds and original authority

For each selected row, the native precheck socket and the already authenticated
Tower actor session share the same finite native query grant. The service mounts
the original weak exact-policy port once for that row; it does not issue a second
disclosure or replace the actor session. Tower `precheck` and `capabilities`
queries therefore spend the same allowance as native precheck reads. A native
read can exhaust Tower reads, and a Tower read can exhaust native reads.

Even when a row explicitly selects full-policy profile version 3, its Tower
facade receives only that grant's exact-predicate projection. Full policy content
remains available only through the explicitly selected native protocol. Both
projections retain the same original query accounting and custodian.

Transport limits remain separate: the selected native wire profiles contribute
to the schedule's exchange, candidate and connection sums below; Tower traffic
spends the original actor session's own finite exchange and connection limits.
Neither transport can replenish native queries or the other's transport work.

All rows and all selected profiles are loaded and validated before the first
clock observation, source content read, listener or authority store. Profiles
are retained in memory; later handoffs do not reopen an edited profile file.
Checked arithmetic admits the total selected allowances across the whole
schedule:

| Allowance | Maximum sum across selected rows |
| --- | ---: |
| Native queries shared by both protocols | 128 |
| Precheck wire exchanges | 128 |
| Peer candidates | 1,024 |
| Authenticated connections | 1,024 |

Each individual profile must also satisfy its existing limits. All selected
disclosure IDs and precheck socket identities must be distinct. Unspent allowance
from one row is never borrowed by another, and reconnects, repeated query labels,
cached responses, retries and request handoffs do not replenish a grant.

Before startup, canonical existing ancestors are used to compare every selected
precheck socket against all future reviewer and Stop sockets, the actor and
operator sockets, the source path, the authority store subtree, and original
topology input and checkpoint output paths. This includes parent symlink aliases.
An existing precheck destination is refused; it is never removed to start a
service.

Each selected row starts after the preceding request's direct helper children
are confirmed reaped and the new request's independent Stop transport is
installed. Stop gets priority before bootstrap and before each precheck turn.
One new row bootstrap acquires one fresh source observation under the original
native source policy and retained accounting. Actual actor admission acquires
its own fresh source evidence; a passing precheck is not a helper vote, human
permit, publication witness or consequence authority. Source limits must cover
both bootstrap and actual admission work for selected rows, as well as the
original workflow's other required source observations.

The original actor session, original owner, cumulative consequence ledger and
global logical/wall deadline survive each handoff. A row's precheck runtime is
also clipped to that global deadline and its original observation lease. Every
query reads the immutable original projection: it does not read source content,
observe owner time, append journal events or start reviews.

An owner revision change immediately invalidates the native disclosure. Once
the original request is admitted, the service explicitly retires that row's
precheck before helper/human review. It also retires on Stop, errors and timeout
before ordinary stop/drain or terminal reply grace. Retirement revokes the sole
native custodian before the weak Tower facade is removed. Sidecar peer faults
retire that row's disclosure and read facade while the ordinary actor service
continues; they do not independently invoke the actor service's Stop.
The next explicitly selected row mounts only its own bounded grant. A null row
inherits no predecessor facade, and no handoff revives an old socket or restores
spent queries.

## Unsupported selections

`--precheck-schedule` requires `--tower-generation`, `--requests`, and an
ordinary plain or simple checked creation. It cannot be combined with:

- `serve-open`, `serve-open-checked` or `--guarded` recovery;
- `--governor-profile` or an operator policy-promotion selection;
- the single-request `--precheck-profile` option;
- richer checked publication profiles.

Existing native actor commands and learned resident sequence selection keep
their existing interfaces. Historical requests do not bootstrap a new disclosure
through this creation-only option.

The regression source covers two requests over one original actor connection,
all three disclosure profiles, alternating native/Tower query exhaustion,
exhausted reconnects, changed source and rejected
human approval after a pass, explicit null, Stop retirement, full schedule
admission, aggregate limits, later endpoint aliases and unsupported combinations.
It also covers positive CLI option ordering and the public checked creation CLI
with two prechecked publications, an independent operator and one cumulative
topology source/checkpoint.
This addition has source review only in the authoring environment: its executor
was disconnected and no remote compilation worker was available. The tests and
Rust build still require execution through the repository's remote validation
workflow; the authored cases are not a passing-test or deployment claim.
