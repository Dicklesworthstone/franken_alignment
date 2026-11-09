# Replay a candidate policy against the original retained corpus

On Unix, the native Tower publication profile connects `policy-replay` to
`FileOversight::replay_candidate_policy`. An explicitly authorized operator can
compare a candidate exact policy with every retained proposal and applied review
associated with the owner's current policy. The comparison includes exact
denials and keeps proposal and review observations as separate cases.

This is a historical experiment over the original owner's observations. It
evaluates the existing exact-policy engine without observing new evidence,
rerunning helpers, changing policy, creating a live governance campaign, or
issuing publication or promotion authority. The caller supplies the candidate;
the owning controller selects the complete corpus. A selected prefix cannot be
reported as a successful comparison.

The route is available through `PublicationTower::dispatch`,
`PublicationTower::exchange`, and a `TowerRouter` with that original publication
mount. The [live operator service](LIVE_TOWER_OPERATOR.md) can explicitly provision
the same grant. Ordinary actor connections and existing precheck, situation and
explanation grants do not acquire policy replay. The imported
[`ReferenceTower`](TOWER_REFERENCE.md) does not implement this native operation.

## Provision separate disclosure custody

The original supervisor issues a finite grant:

```rust
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, FileSupervisorPolicyReplayGrant,
};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::tower::publication::PublicationTower;

fn install_policy_replay(
    supervisor: &FileActorSupervisor<FileOversight>,
    tower: &mut PublicationTower,
) -> FileSupervisorPolicyReplayGrant {
    let (port, grant) = supervisor
        .policy_replay_port(
            8,
            ReplayLimits {
                cases: 128,
                input_bytes: 1_048_576,
            },
        )
        .expect("healthy original owner and bounded grant");
    tower
        .set_policy_replay(Some(port))
        .expect("Tower mounted on the exact original owner");
    grant
}
```

The embedding host must retain the returned
`FileSupervisorPolicyReplayGrant` for the intended service lifetime. It is
noncloneable; dropping it or consuming `revoke()` withdraws every weak port
clone and their shared cache. `FileSupervisorPolicyReplayPort` is cloneable,
but cloning or reinstalling it does not renew the allowance. The grant exposes
`remaining_queries()`, `replay_limits()` and `retained_bytes()` for its owner.

`set_policy_replay(Some(port))` checks the exact original owner allocation and
native scope. Equal tenant, run and other scope numbers on a different owner are
insufficient. Replacement first clears the previous installation, including
when the new port is rejected. `set_policy_replay(None)` removes this facade's
port without revoking another facade that holds a clone under the same live
custodian.

The grant and ports do not keep the original supervisor or its authority lock
alive. A closed or reopened owner cannot reuse the previous grant. Provisioning
itself reads no source or clock, captures no report, consumes no admission
snapshot, and changes no journal or governance authority.

This native Rust grant is privileged: `FilePolicyReplay::report()` contains the
original policies, action payloads and retained witness values. The Tower wire
projection intentionally discloses less, as specified below. A host must not
replace that projection with a debug rendering of the native report.

## Submit a bounded candidate

The request uses the original run mount, including its explicit nonzero route
registration generation. `campaign` and `expected_policy_generation` must be
nonzero. The envelope's `idempotency_key` must be the same canonical decimal
string as `campaign`.

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "compare-policy-91",
  "address": "fa://1/run/3@41",
  "verb": "policy-replay",
  "arguments": {
    "campaign": "91",
    "expected_policy_generation": "1",
    "candidate": {
      "generation": "2",
      "nodes": [
        {"op": "payload_at_most", "maximum": "128"}
      ]
    }
  },
  "idempotency_key": "91"
}
```

These identities illustrate a host configured for that exact mount and current
policy. The typed counterpart is:

```rust
use fa_reference::action::consequence::gate::containment::session::policy::{
    Policy, Predicate,
};
use fa_reference::tower::publication::{PublicationResponse, PublicationTower};
use fa_reference::tower::{ReadArguments, TowerAccess, TowerRequest, Verb};

fn compare_policy(
    tower: &mut PublicationTower,
    authenticated_access: &TowerAccess,
) -> PublicationResponse {
    let mut request = TowerRequest::new(
        "compare-policy-91",
        tower.mount_address().clone(),
        Verb::PolicyReplay,
    );
    request.arguments = ReadArguments::PolicyReplay {
        campaign: 91,
        expected_policy_generation: 1,
        candidate: Policy::new(2, vec![Predicate::PayloadAtMost(128)])
            .expect("valid bounded policy"),
    };
    request.idempotency_key = Some("91".to_owned());
    tower.dispatch(authenticated_access, request)
}
```

The authenticated access must permit the subject tenant, ControlLedger plane
and Verify authority, set the operator role, and include `PurposeTag::Experiment`.
Those grants come from the host. Neither the JSON candidate nor the campaign
number supplies them. This route requires no `Permit` purpose and grants no
effect authority. It does not require a branch `ExperimentCx`: it calls the
native retained-policy comparison without creating or appending a branch journal.
The separate [paired branch replay](TOWER_REPLAY.md) retains its own context,
intervention and authority requirements.

The candidate's generation must exceed `expected_policy_generation`; the native
port rejects an equal or older generation before spending a campaign. The
expected generation is a policy generation, distinct from the policy epoch,
route registration generation, journal revision and control sequence. If the
actual current policy generation differs, a new admitted campaign returns and
retains a native `Stale` observation.

### Closed policy graph syntax

The candidate object has exactly `generation` and `nodes`. Every integer,
including node indices and target fields, is an unsigned canonical decimal
string. Literal bytes use even-length lowercase hexadecimal; an empty literal
is `""`. Duplicate keys, unknown operators, extra fields and numeric JSON values
are rejected by the [request codec](TOWER_REQUEST.md).

| `op` | Other required fields |
|---|---|
| `target_is` | `target` with `adapter`, `object`, `contract_version`, `expected_version`, `generation` |
| `payload_is` | `value_hex` |
| `payload_at_most` | `maximum` |
| `units_at_most` | `maximum` |
| `exact_value` | `key`, `value_hex` |
| `absent` | `key` |
| `empty_range` | `start`, `end` |
| `not` | `child` |
| `all`, `any` | Nonempty `children` array of node indices |

The final node is the root. Every child must refer to an earlier node, and every
node must be reachable from the root. The existing `Policy::new` constructor
validates the graph, target identities, range endpoints and native limits.
Parsing constructs policy data only; it cannot select historical cases, import
caller evidence, create a grant or promote the candidate.

## Interpret the complete comparison

The original controller selects all retained proposal records and applied
reviews bound to its current policy object. Historical records from a previous
policy are not silently relabeled as current-policy observations. A proposal
case is identified by its native attempt. A review case additionally retains
its original round and control sequence. External actor request keys are not
substituted for those native identities.

For each case, the engine reconstructs only the keys and closed ranges covered
by retained witnesses. It verifies the original policy's evaluation against
that reconstructed evidence before comparing the candidate. Unsupported
candidate reads remain missing observations. In particular, an unobserved key
cannot become proved absence, even if another branch of a candidate disjunction
would satisfy the expression.

| Case delta | Meaning |
|---|---|
| `unchanged` | The candidate's exact result matches the original result |
| `newly_blocked` | The original exact predicates passed and the candidate's fail |
| `newly_reviewable` | The candidate's exact predicates pass where the original predicates failed; no helper approval is transferred |
| `requires_shadow` | Retained observations cannot fully evaluate the candidate; its missing node indices remain explicit |

`requires_shadow` at report level is true when any case needs more observations.
The report retains separate `newly_reviewable` and `newly_blocked` case lists.
The original review consequence remains historical metadata and is not a
candidate helper vote. An empty or unreconstructable corpus returns Unknown,
not a vacuously successful comparison. Corpus limits refuse the whole report.

The route does not consume a proposal admission snapshot, reserve rights, change
an existing request ticket, approve a review, call an endpoint, advance a clock,
refresh a source, write a journal event or create a governance promotion key.
Original publication still requires its independent current checks and its
original automatic and human keys.

## Native basis and sealed Tower registration

`FilePolicyReplayResult` is a native actor `Knowledge` result or `ActorError`.
A Known `FilePolicyReplay` exposes `basis()`, `report()`, `limits()` and
`remaining_queries()`. Its sealed basis retains:

| Field | Coordinate |
|---|---|
| `scope` | Original complete native scope |
| `query` | Campaign identity within this grant |
| `journal_revision` | Original acknowledged journal cut |
| `control_sequence` | Original controller sequence at that cut |
| `policy_epoch` | Original ledger policy epoch |
| `policy_generation` | Original current policy generation |
| `recorded_at` | Previously recorded elapsed tick, or `None` |

A new exact denial can advance the journal without changing the control sequence
or policy generation. The revision therefore remains a separate coordinate.
The returned tick is historical and does not observe the current clock.

The Tower result is `PublicationResult::PolicyReplay(LinkedPolicyReplay)`.
This public wrapper has private fields and exposes only `native()`,
`registration()` and `campaign()`. It seals the native result to the exact Tower
registration address, including its generation, and campaign that produced the
response. The encoder rejects changes to the public envelope's subject,
registration generation or idempotency key, including for native errors and
uncertain results. A scalar campaign or copied response cannot construct a new
linked native result.

## Wire projection and uncertainty

The response retains the common sixteen-field `fa.tower_response/1` envelope.
Its outer result kind is `policy-replay`; the native value uses schema
`fa.publication-policy-replay/1` and scope `retained_current_policy_corpus`.
`historical` and `rehearsal` are true. `policy_changed`, `helpers_rerun`,
`live_evidence_read`, `effect_authority`, `promotion_authority`,
`payload_disclosed` and `witness_values_disclosed` are false.
`current_eligibility` is `not_observed`.

A Known body's `knowledge.basis` retains the native coordinates above, with
`campaign` as the wire name of `query` and `origin: "acknowledged_owner"`.
Its `knowledge.value` contains both complete policy graphs, case count and input
byte charge, all case deltas, the complete case array, configured replay limits,
and `remaining_campaigns_at_capture`.

Each case discloses its identity, semantic epoch, original consequence, original
and candidate traces, and action target, units, payload length, policy epoch and
deadline. A known trace includes every node result, witness index and witness
count. An unavailable candidate trace instead identifies
`missing_retained_observations` and `missing_nodes`. Numeric report coordinates
are decimal strings.

Policy literals are disclosed under the explicit grant. Action payload bytes,
actual witness values, helper input bytes, full source snapshots and journal
paths are omitted. Omission does not permit dropping cases or replacing their
traces with counts. Native coordinates and grant-local accounting do not become
global Tower observations: envelope generation, control sequence, source
frontiers, budgets, realized cost and uncomputed affordances remain Unknown,
or Withheld on an unauthorized request.

| Condition | Result and accounting |
|---|---|
| Missing installed replay port | `CapabilityUnsupported`; no native campaign spent |
| Missing operator role or Experiment purpose | `DeniedByPolicy` or `PurposeMismatch`; no native campaign spent |
| Wrong registration generation | `StaleGeneration`; no native campaign spent |
| Revoked custodian | Native Withheld requiring `supervisor_policy_replay`; no new campaign spent |
| Lost, mutably borrowed or faulted original owner | Native Unknown / `LowerLayerUnverified`; no new campaign spent and no cached disclosure |
| New campaign with mismatched current policy generation | Native Stale / `StaleGeneration`; one campaign spent and result retained |
| Empty or unreconstructable corpus after admission | Native Unknown / `LowerLayerUnverified`; one campaign spent and result retained |
| Complete corpus exceeds work or retained-result limits | Native `Capacity` / `BudgetExhausted`; admitted work stays spent, no prefix report |
| Same campaign with different valid expected generation or candidate | Native `IdempotencyConflict` / `IntegrityFailure`; original entry unchanged |

Malformed native query/candidate identities fail before spending. Other syntax,
scope and typed-admission failures retain the common Tower refusal behavior.
An unknown result never proves absence, nonexecution or permission to publish.

## Exact retries, lifetime and bounds

Campaign identity is local to the original live grant. The first admitted call
stores its complete expected generation, whole candidate policy, result and
original basis. Every port clone and facade using that grant shares the same
allowance and cache. Changing `operation_id` changes response correlation only.

An exact retry returns that first result without spending another campaign,
even after later proposals, denials, clock observations or policy changes.
Stale, unavailable-corpus and capacity results also remain bound to their first
admitted campaign. A new campaign identity is required to request a new
comparison, and it spends the remaining allowance. Cached results expose the
remaining count at their original capture, not a promise of the current balance.

Every call checks live grant custody and healthy original-owner availability
before looking up the cache. A cached success cannot be disclosed after
revocation, while the owner is mutably borrowed, after a storage fault, or after
owner loss. The cache is process-local and is not a durable recovery credential.
A restart or newly issued grant has a different campaign namespace; callers must
not describe a newly recomputed result as a continuation of the old capture.

| Resource | Bound |
|---|---|
| Admitted campaigns per grant | Host selects 1–64 |
| Cases per complete comparison | Host selects 1–5,120 |
| Logical input bytes per comparison | Host selects one byte through 32 MiB |
| Shared retained request/result bytes | Fixed 8 MiB per grant |
| Policy graph | 1–128 nodes, at most 512 edges and 64 state-reading nodes |
| Combined policy literal bytes | At most 65,536 |
| Request document | 256 KiB, depth 8, 2,048 JSON items, 128 KiB per decoded string |
| Native Tower response | Host selects 16 KiB–1 MiB; default 16 KiB |

Replay input bytes count action payload and required/original/candidate witness
values, including repeated retained occurrences. Cache retention separately
charges policy nodes, edges, literals, action and witness occurrences, traces,
case data and fixed map metadata. These are logical limits, not measurements of
peak allocator memory, runtime or helper inference. The request's structural
limit may reject a valid native graph before any replay work.

A request that cannot fit the remaining allowance or retained-request budget
does not run. Once admitted, work is charged once before the original replay;
failure does not refund it. If the complete resulting report cannot fit the
remaining cache budget, it is discarded whole and a charged capacity result is
retained. The service never silently evicts an earlier campaign to retry it.

Output limits are separate from native replay and cache limits. If encoding a
complete report exceeds the Tower response bound, the facade returns a bounded
`BudgetExhausted` refusal without a partial policy or case list. The native
campaign stays retained and charged. An exact retry through an explicitly
configured larger same-owner facade can encode that existing report without
replaying it or spending another campaign. All custody and original registration
checks still apply to each newly issued linked response.

## Enumerate a complete comparison in bounded pages

A complete retained comparison can exceed the largest single Tower response.
Hosts can separately opt into page disclosure by issuing
`policy_replay_port_with_pages(queries, limits, page_queries)` instead of the
original constructor. `page_queries` must be 1 through 1,024 and is shared by
all ports and facades holding that grant. The original constructor keeps paging
disabled. The live operator exposes the same opt-in as `policy_replay.page_queries`;
see [operator setup](LIVE_TOWER_OPERATOR.md).

Add the exact boolean `"paged": true` to the complete initial request's
`arguments`. Its typed form is `ReadArguments::PolicyReplayPaged { campaign,
expected_policy_generation, candidate }`. It runs or retrieves the same complete
native comparison; it never selects a smaller replay corpus. A legacy v1 request
and a paged request with the same campaign and candidate share that original
comparison and native campaign charge.

The native Rust port exposes `replay_candidate_policy_page` and
`continue_policy_replay`. Its sealed `FilePolicyReplayPage` exposes `original`,
`basis`, `summary`, case offsets, whole cases and the native continuation. The
grant exposes `remaining_pages`, `pages_enabled` and `retained_page_bytes` in
addition to its independent campaign accounting.

The first response is a header containing both original policy graphs, the
original basis and complete-corpus summary, with no cases. To follow its
`next_cursor`, send the same run address, campaign and matching idempotency key
with exactly these arguments:

```json
{"campaign":"91","cursor":"17"}
```

The typed continuation is `ReadArguments::PolicyReplayContinue { campaign,
cursor }`. The cursor shown is illustrative; use only the nonzero canonical
identifier returned by the original facade. Every subsequent page contains the
next contiguous group of at most 16 whole cases. It does not repeat the policy
graphs. The native report is shared through an immutable `Rc`, and its summary
is computed once when captured. Traversal neither copies another whole corpus
nor reruns its policy comparison.

### Page schema and completion

Paged replies use `fa.publication-policy-replay/2` under the same outer
`policy-replay` result and sixteen-field envelope. Historical and authority
flags, native uncertainty states, payload/witness omissions and exact trace
representation are unchanged. A Known `knowledge.value` has:

| Field | Meaning |
|---|---|
| `page.kind` | `header` for the initial response, then `cases` |
| `page.request_cursor` | Null for the header; the requested continuation identity thereafter |
| `page.offset`, `page.count`, `page.total` | Contiguous case coordinates in the complete original corpus |
| `page.enumeration_complete` | Whether every original case has now been enumerated |
| `page.next_cursor` | An issued native continuation, or null |
| `page.remaining_pages_at_capture` | Shared allowance immediately after this page was first admitted |
| `summary` | Original case/input counts, delta counts, shadow flag, replay limits and campaign allowance at capture |
| `previous_policy`, `candidate_policy` | Whole graphs on the header, null on case pages |
| `cases` | Empty on the header; at most 16 complete original cases thereafter |
| `newly_reviewable`, `newly_blocked` | Case IDs for those deltas on this page only |

Every numeric coordinate uses the existing canonical decimal string convention.
The complete delta totals live in `summary.delta_counts`. The summary and native
basis remain identical across the whole traversal, including after the healthy
owner advances. A Known page proves delivery of that section only. A null
continuation with `enumeration_complete: false` explicitly reports incomplete
traversal; it is not an empty or successful complete comparison.

### Charges, custody and retries

Each newly admitted header or continuation spends one page observation. A new
header also spends the original campaign allowance if that comparison has not
already been captured. A native stale, unknown or admitted capacity result stays
charged and cached. Exact header and continuation retries return their original
observation, next cursor and historical remaining allowance without another
page or campaign charge, even when no new allowance remains. Shared callers may
spend intervening allowance; a returned count reserves no future page.

The native cursor is sealed to the exact owner, grant, campaign and captured
report. Its scalar wire identifier cannot construct that handle. The facade
retains only handles that it actually issued in an encodable response, and a
replaced installation clears them. Process-wide nonreused identifiers prevent
same-process grant replacement from rebinding an old number. Original grant
custody and healthy owner availability are checked before every cached read.
Revocation, owner loss, conflicting borrow or storage failure suppress disclosure.

A different facade with a clone of the same live grant must first issue the
original header and, if needed, its cached continuation chain to import those
handles. Copying a scalar cursor to an unprepared facade cannot skip this step.
The wrapper `LinkedPolicyReplayPage` seals every response, including uncertainty,
to its original registration, campaign and requested cursor. Restarted owners
cannot reconstruct this process-local capture or continue it from a wire number.

Page metadata has its own finite bound, exposed by
`MAX_POLICY_REPLAY_PAGE_RETAINED_BYTES`; it does not enlarge the original 8 MiB
campaign cache. There are at most 1,024 admitted page entries per grant. A maximum
5,120-case comparison needs one header plus 320 case pages if neither allowance
nor output capacity stops traversal. Complete comparisons that exceed native
work or retention bounds still refuse whole before enumeration.

Each header or case page must fit the selected response allowance. An oversized
page returns the ordinary bounded output refusal without a partial graph, trace
or case. The original native page remains charged and cached. An authorized
same-owner facade with enough response capacity can reissue the original header
and cached chain without another native charge. The live operator accepts
`response_bytes` from 16 KiB through 1 MiB to accommodate large policy literals
and case traces.

### Authenticated complete-report consumer

The Linux `supervise_publication` example provides:

```text
tower-policy-replay OPERATOR_PROFILE PAGED_TOWER_REQUEST_JSON --max-pages N
```

It requires an explicit paged initial document and a finite page cap. It uses the
original credential checks, connection, lifetime and cumulative I/O bounds. It
validates each complete response before writing its original JSON line or
following a cursor. It performs no reconnect, retry or new campaign on failure.
A native refusal, incomplete allowance, exhausted client cap, malformed page or
failed output ends unsuccessfully and sends no next continuation.

The reusable `tower::policy_replay_client::PolicyReplayCapture` validates the
trusted native scope, request correlation, registration, historical basis,
policies, complete summary, contiguous offsets, unique case and cursor
identities, exact trace shapes and page-local delta lists. It commits progress
only after the whole page passes. It retains the two bounded policy graphs,
summary and seen identities, not action payloads, witness values or a second
case corpus. `progress().complete` becomes true only after validated totals and
all original cases agree.

## Executed coverage and remaining qualification

The automatic consumer increment adds ten public consumer and eight
CLI/service regression functions. Its unchanged consumer modules and complete
23-function public integration target typechecked in a focused composition with
the already-compiled native pagination library. The current full-library build
and full example test-harness metadata attempt were killed at the shared memory
limit; the focused composition's runtime link could not resolve the cached
split-metadata artifact. These results provide no new consumer runtime or full
repository qualification. The core runtime evidence below retains its original
source scope. The canonical gate verified the current 2,355-file source closure,
compiler/admission inventories and concordance, then stopped at three existing
playbook coverage omissions before later build, Clippy and test stages.

The native pagination increment was compiled with qualified
`nightly-2026-09-08` and Cranelift on 2026-11-09. All **13 public page integration
tests** in `tower_policy_replay_pages.rs` passed, with none ignored. This includes
complete 82-case traversal after owner advancement, original-key publication,
shared exact retries, incomplete allowances, custody boundaries, unchanged
journals and whole-page output refusal followed by larger-facade recovery.
The first run exposed a disconnected policy graph in one fixture; its 128 nodes
were connected to their final root without changing the tested budgets or
assertions, and the complete focused rerun passed. Six native unit regression
functions have source/format review but have not executed. This evidence does
not replace the full repository or remote qualification gate.

On 2026-11-09, the qualified `nightly-2026-09-08` compiler with the Cranelift
backend built and executed all **12 public integration tests** in
[`tower_policy_replay.rs`](../crates/fa-reference/tests/tower_policy_replay.rs):
12 passed, none failed and none were ignored. They exercise the actual native
corpus, both policy-delta directions, an unchanged journal and policy followed
by real publication with the original two keys, strict codec, withheld evidence,
cache and owner lifetime, bounded failures, response rebinding and unified
routing without snapshot fallback.

The initial focused run found an oversized fixture payload before the replay
route was reached. The corrected fixture fits its original 16-unit proposal
allowance; no native budget or assertion was relaxed. The passing rerun is a
focused local execution result. It does not establish a full repository gate,
remote RCH execution, deployment qualification, every registered Tower verb or
completion of the unified dispatcher bead.

The implementation lives in the original
[native grant and cache](../crates/fa-reference/src/action/consequence/delivery/persistent/requests/actor/policy_replay.rs),
[Tower adapter](../crates/fa-reference/src/tower/publication/policy_replay.rs),
[policy request codec](../crates/fa-reference/src/tower/request/policy.rs), and
[bounded wire projection](../crates/fa-reference/src/tower/wire/publication/policy_replay.rs).
