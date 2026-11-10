# Fence one original publication domain

The native Tower `fence` profile connects the existing durable admission Stop
to the same request/response interface as publication and inspection. It is a
bounded L5 control operation under FA-120 and the unified dispatcher work
(FA-134/FA-138), serving FI-A02 and plan sections 8.5, 8.11, 16.4 and 17.3.
It adds no journal, replacement ledger, runtime, dependency or publication key.

## What an acknowledgment establishes

A successful native operation permanently closes admission in **one original
publication owner** and raises its authority revocation floor through the
existing Stop transition. It cancels and refunds only undispatched reservations.
Previously dispatched or uncertain effects retain their original charges.

The result carries the original Stop receipt and its native StopProgress at the
acknowledged ending cut. Endpoint fencing is still false at that cut: this
operation does not read a clock, drain or seal the endpoint, resend an effect,
request new evidence, run helper review or sample another model token.
Actor termination and fleet fencing remain explicit Unknown values.

The current owner has no resume operation. A caller must not use this profile
as the full distributed P8 playbook: it accepts neither an arbitrary floor nor a
fleet inventory, and supplies no distributed acknowledgment frontier. The
original supervisor retains its separate endpoint-drain and helper-cleanup
responsibilities. An acknowledged admission stop does not establish their
completion.

## Host custody and finite work

Only `FileActorSupervisor<FileOversight>` can issue a
`FileSupervisorFencePort` and its nonclone `FileSupervisorFenceGrant`.
The port holds weak references to both. Dropping or revoking the grant withholds
even a previously cached result; losing the original supervisor or observing its
storage-failure latch returns Unknown.

The owner must already have its terminal recovery reserve.
`fence_port(max_replay_events)` grants one operation over a bounded original
history. Decoder or action-predictor histories additionally require
`fence_port_with_numerical_budget(max_replay_events, numerical_replay_events)`.
The complete acknowledged history plus the proposed Stop event is reserved
before entering native replay. Warm replay never refunds this conservative
reservation. The original numerical recipes still enforce their own limits;
event counts are not CPU, memory, numerical-product or latency measurements.

A new operation binds four independent predecessor coordinates:

| Coordinate | Meaning |
|---|---|
| `scope` | Exact original tenant, principal, run, branch, authority and Effect purpose |
| `journal_revision` | Complete acknowledged native journal cut |
| `control_sequence` | Original authority ordering |
| `authority_epoch` | Original authority epoch before its revocation floor advances |

A stale or foreign predecessor refuses before spending the operation or
invalidating an unused admission snapshot. Once native work is entered, its
input is latched, the operation and applicable numerical work are spent, and
that snapshot is withdrawn. Storage failures and interrupted calls never
restore those allowances or cause a second native application.

Exact retries require the same original live grant and healthy owner. They
return the original result and historical cut without another Stop event,
even after later endpoint drain. Changed inputs under the same operation key
conflict. A new operation exhausts the one-operation grant; a replacement grant
cannot adopt a previous acknowledgment or append an idempotent duplicate Stop
to an already stopped owner. Recovery retains the original native Stop, while
grant-local retry disclosure is deliberately not reconstructed.

## Tower request and response

The host installs the matching original port with `PublicationTower::set_fence`.
A failed replacement withdraws the previous port. Matching public numeric scope
does not substitute for matching the actual original owner.

`new_control_operator` and `new_learned_control_operator` expose a restricted
operator facade. They grant no operation themselves and never expose actor
intake, cancellation, polling or precheck. Existing observation and
reconciliation constructors keep their original vocabularies. An installed
fence requires the authenticated host's operator role, Permit purpose and an
explicit exact run registration generation.

The request uses the existing `fa.tower_request/1` schema. The following is a
shape example; its numbers must be replaced by the independently observed
original predecessor and the selected registration:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "stop-original-domain",
  "address": "fa://1/run/3@41",
  "verb": "fence",
  "arguments": {
    "operation": "5009",
    "expected": {
      "scope": {
        "tenant": "1",
        "principal": "2",
        "run": "3",
        "branch": "4",
        "authority": "5",
        "purpose": "effect"
      },
      "journal_revision": "9",
      "control_sequence": "7",
      "authority_epoch": "0"
    }
  },
  "idempotency_key": "5009"
}
```

All integer identities use canonical decimal strings. Missing predecessor
fields, duplicate or extra keys, numeric aliases, mismatched operation keys,
foreign run addresses and absent registration generations are rejected by the
bounded closed parser before routing.

The response keeps exactly the existing sixteen envelope fields and adds
`result.kind = "fence"` with `fa.publication-fence/1`. Its sealed registration,
operation identity and expected predecessor cannot be relabeled through public
envelope fields. The original before/after cuts, cancelled attempt IDs,
refunded units, outstanding liabilities and raised floor are retained.
Global Tower clocks, measured costs and fleet/process state are not synthesized.
`retry_safe` remains false: there is no instruction to retry automatically
through a new owner or grant after a lost connection.

## Regression scope and qualification

The native regression sources cover reviewing and two-key-authorized work,
actual dispatched and executed effects, independent endpoint drain, original
retry results, every predecessor coordinate, foreign owners, grant revocation,
owner loss, reopening, full ordinary capacity, unavailable source evidence and
storage failure. Learned warm/cold histories and a raw action predictor require
their original independent numerical allowance and preserve their spent work.

Public Tower regressions exercise the same original owner through typed and
JSON requests, preserve unknown charges, reject authority and registration
mismatches, prevent actor intake through a control mount and reject response
relabeling. Fixed wire vectors retain full-width identities and strict syntax.

The new source received independent source review. Before these changes, two
qualified-nightly baseline library checks were terminated by SIGKILL without
compiler diagnostics. The execution workspace then became unavailable.
Consequently these new sources have **not** passed compilation, Rustfmt,
Clippy, runtime tests or the remote canonical gate in this session. The
source-snapshot inventory records exact authored bytes, not an execution claim.
No production activation, dispatcher-bead closure or full FA-120 completion is
claimed.
