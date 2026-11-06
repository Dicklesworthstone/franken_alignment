# Bounded reference Tower operations

`fa_reference::tower` connects the existing object resolver, event journal,
situation fold and predicate explanations through one Rust request/response API.
It implements eight read operations, branch-only paired policy `replay`, and the
sixteen-field common envelope in
[`registry/vocabulary.json`](../registry/vocabulary.json). The vocabulary has
**38** registered verbs; the older dispatcher bead's title says 35.

The host-provisioned imports remain fixed. An enabled experiment can append its
actual outcome only to its owned branch journal; production reads remain
immutable. The [paired replay contract](TOWER_REPLAY.md) describes the native
baseline, interventions, separate source/branch bases and exact retries.
The facade does not implement
a `fa` command-line executable, a network service, request authentication,
durable subscription, native mutations or deployment qualification. The larger
FA-133/FA-134/FA-135/FA-136/FA-137/FA-138 contracts and dispatcher bead remain open.

The separate [durable publication profile](TOWER_PUBLICATION.md) shares the
request vocabulary and generic sixteen-field response envelope. It connects
native actor `propose` and request-local `get` using the original file owner,
without projecting its mutations into this imported snapshot journal.

## Connected operations

| Verb | Address and result | Required access |
|---|---|---|
| `get` | Imported object, including its complete identity and original basis | Subject tenant, plane and authority |
| `related` | Generation-bound relationships and each permitted target's original `Knowledge` | Subject admission, followed by each target's admission |
| `tail` | Domain journal page with actual events, causal sequence cursor and gaps | Operator and complete domain scope |
| `situation` | Domain fold with all items, gap notices and each item's producing event sequence | Operator and complete domain scope |
| `explain` | Imported predicate tree with all nodes, statuses, evidence references and bases | Complete domain scope and every evidence address |
| `why-held` | Unsatisfied predicate items and permitted remedies | Operator, complete domain scope and every evidence address |
| `next` | Ranked affordances within the host's allowed purposes and cost allowance | Operator, complete domain scope and every evidence address |
| `replay-plan` | Actual installed intervention registration, source lineage and execution requirements | Branch and source-family visibility; sensitive native metadata requires full domain scope |
| `replay` | Real baseline/treated policy report, explicitly labeled rehearsal | Operator, full scope, Experiment purpose, owned `ExperimentCx`, branch journal and exact request key |

Complete domain scope means the journal's tenant, all four resolver planes and
`Verify`, `ExactRead`, `Judge` and `Probe` authorities. `TowerAccess` is constructed
by the integrating host. Its grants and operator flag must never be accepted as
actor-supplied claims. This API checks those grants; it does not authenticate
their origin or manufacture proofs for an imported snapshot.

For a valid authorized request, every other registered verb returns
`CapabilityUnsupported` before spending a read unit or entering a backend.
Malformed and unauthorized requests fail their earlier admission checks.
Unknown and retired names fail `Verb::from_str`.
The facade never treats the reference proposal recorder as an executed effect.

## Provision, then query

1. Construct a `TowerBuilder` with a `DomainEventJournal`, the host's `Basis`,
   `TowerLimits` and an initial logical read allowance. The basis must identify
   the journal's actual final control sequence. The builder derives the domain
   address from its production run or experiment branch.
2. Add objects with their original bases, relationships and predicate trees.
   Object identity, tenant and generation must agree. An edge needs an existing
   source and binds its registration generation. An explanation must refer to
   an imported object with the same basis at the snapshot's control cut.
3. Optionally provision the explicit branch replay context and original history
   records as described in [TOWER_REPLAY.md](TOWER_REPLAY.md), then call `build()`.
   The resulting `ReferenceTower` owns its imports. Dispatch cannot replace
   objects or explanations; only an admitted experiment can append a branch event.
4. Construct `TowerRequest::new(operation_id, address, verb)` and, where useful,
   select `ReadArguments::Related`, `Tail` or `Next`. Call `dispatch` with the
   host-provisioned `TowerAccess` and serialize the response using `to_json()`.

For example, an integrating Rust host that already owns its journal, basis and
grants can obtain a situation response without changing the journal:

```rust
use fa_reference::event_journal::DomainEventJournal;
use fa_reference::observation_algebra::Basis;
use fa_reference::tower::{
    TowerAccess, TowerBuilder, TowerInstallError, TowerLimits,
    TowerRequest, TowerResponse, Verb,
};

fn read_situation(
    journal: DomainEventJournal,
    basis: Basis,
    access: &TowerAccess,
) -> Result<TowerResponse, TowerInstallError> {
    let mut tower = TowerBuilder::new(journal, basis, TowerLimits::default(), 8)?
        .build();
    let request = TowerRequest::new(
        "inspect-situation-1", tower.domain_address().clone(), Verb::Situation,
    );
    Ok(tower.dispatch(access, request))
}
```

The public regression fixture in
[`tower_dispatch.rs`](../crates/fa-reference/tests/tower_dispatch.rs) demonstrates
complete object, event, explanation and grant provisioning for all seven routes.

## One envelope, complete evidence

The JSON root contains `envelope` and `result`. The result is a typed
`Knowledge<TowerValue>` with a structured body. The envelope has exactly these
sixteen fields, using schema `fa.tower_response/1`:

| Fields | Meaning |
|---|---|
| `schema_version`, `operation_id`, `address`, `idempotency_key` | Response schema and bounded request identity |
| `status`, `reason_code` | Operation outcome and a registered reason code |
| `generation`, `control_seq`, `source_frontiers` | Typed observations of the relevant basis or frontier |
| `missing`, `would_change_this` | Addresses identifying missing observations or reconsideration targets |
| `limits` | Static import, query and response bounds |
| `budgets_remaining`, `realized_cost` | Typed logical read accounting, disclosed only to an operator with full domain scope |
| `affordances` | Complete permitted affordances: verb, target, required purpose, cost, expected information gain, changed addresses and predicate node link |
| `retry_safe` | Whether retrying this read is free of authority effects; it can still spend another read unit |

Every nested safety-relevant value uses the shared
[Knowledge JSON encoder](KNOWLEDGE_JSON_REFERENCE.md). `Known` and `Stale`
retain identity, all epochs, claim class and each independent proof state;
`Pending` retains its actual required frontiers; `Absent` retains its closed
domain and terminal marker. `Unknown` and `Withheld` remain explicit. A tree
containing an unknown predicate is not flattened into a successful boolean.

The bodies contain complete object fields, related-target observations, journal
events, situation items, predicate nodes and affordances. They are not debug
strings or count summaries. Journal events retain their existing canonical
encoding. The outer envelope does not upgrade an inner proof or uncertainty.

## Generation, scope and accounting boundaries

Source admission precedes lookup and graph expansion. Stale, future and missing
source generations keep their resolver outcomes. Hidden target addresses,
relation metadata and target logs are excluded before disclosure. A partial
caller can read permitted objects and relationships without receiving the global
snapshot's generation, frontier or accounting as a substitute for the object's
original basis. Resolver responses without source-frontier evidence report it
as `Unknown`.

`tail` and `situation` require the exact domain address and apply generation checks.
Situation items retain the control sequence of their producing event even when
later unrelated events advance the snapshot. Their imported generation and
epoch context come from the host's snapshot basis. Gaps preserve uncertainty and
downgrade a declared-complete snapshot proof to `Gapped`.

Imported predicate trees are reconstructed and checked against their declared
node count and depth. IDs must be unique and bounded. A nested `Known` predicate
must match the explanation's generation, policy/model/tokenizer/codec epochs and
control cut; its independent proof states and subject/question/profile identity
are retained. A hidden evidence address withholds the entire explanation
surface. `next` intersects its request cost cap with the host cap, and
`why-held` applies the same permitted set to its embedded suggestions.

`replay-plan` inspects the installed branch artifact without verifying its
historical source. Full-domain readers need neither operator status nor an
Experiment purpose to inspect metadata. Partial callers receive withheld native
metadata, and all plan assurance properties remain Unknown. Planning never reads
or writes the replay result cache. See [the planning contract](TOWER_REPLAY.md#inspect-the-plan-before-running-an-experiment).

One admitted read spends one logical read unit, including an unknown result or
an oversized response. Malformed requests, unsupported verbs, denied access and
pre-admission limit failures do not spend a unit. These units measure this
facade's query allowance; they are not inference, elapsed-time or byte costs.
Read retries do not append authority events, but there is no result cache or
read-accounting deduplication keyed by `idempotency_key`.
Paired replay has its own required exact-request key and bounded result cache;
an exact replay retry spends no additional unit or branch event.

## Resource bounds

| Resource | Default | Maximum configurable value |
|---|---:|---:|
| Imported objects | 256 | 1,024 |
| Imported edges | 512 | 4,096 |
| Journal events | 512 | 4,096 |
| Explanations | 64 | 256 |
| Events per tail page | 64 | 128 |
| Encoded response | 256 KiB | 1 MiB |
| Logical read units | 256 | 1,024 |

Additional fixed bounds cover address and operation identity lengths, event
bytes, source frontiers, evidence references, predicate node IDs and trees
(256 nodes, 16 levels). Regressing source frontiers and inconsistent snapshot
metadata are refused during import. The encoder also checks output and nesting
bounds. If a complete response cannot fit, dispatch returns explicit uncertainty
with an error reason and preserves the spent read charge; it never truncates a
value while labeling it `Known`.

## Executed verification and remaining gates

The replay-plan extension passed all **82 selected public regressions** and the
no-Permit compile-fail contract. The
[planning receipt](../artifacts/execution/2026-11-05-tower-replay-plan-receipt.json)
binds this result to the changed source and records the canonical gate stopping
at the unchanged source-snapshot binding failure.

The subsequent paired-replay extension passed the qualified compiler's library
check and all **77 selected public regressions**, including its nine genuine
oversight integration functions. The
[replay receipt](../artifacts/execution/2026-11-05-tower-replay-receipt.json)
retains source hashes, compiler settings, the failed memory-limit attempt and the
corrected production-read fixture attempt. Independent source review and changed
source formatting are complete. Full repository and deployment gates remain open.

On 2026-11-05 the qualified `nightly-2026-09-08` compiler executed **68 passing
public regressions**: Tower (14), resolver (10), agent projections (15),
Knowledge JSON (12), journal boundaries (9) and proposal retries (8). There were
no failures or ignored tests. The
[execution receipt](../artifacts/execution/2026-11-05-tower-read-receipt.json)
records commands, compiler identity and the exact changed-source hashes.

`xtask check` passed the reference lock, source inventory and compiler identity
checks, then stopped at `snapshot_binding_mismatch`: the declared source
manifest hash differs from the manifest's actual digest. Later full-gate checks
did not execute. This scoped local result does not establish remote execution,
native deployment qualification, all 38 connected verbs or completion of the
dispatcher and canonical snapshot beads.
