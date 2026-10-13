# Recover an independently retained fleet plan

The existing `shutdown_fleet` command now accepts `fa.shutdown-fleet/2` as an
explicit input-custody alternative. It reads the original `FileShutdownPlan`
archive instead of requiring a separate `FileHistoryAnchor` file per member.
This connects the earlier prepared retained-plan operator work to the command
that subsequently landed on main; it does not add a second fleet executable.
The original `create`, `recover` and `resume` modes and their reducer, clock,
checkpoint and acknowledgment semantics are unchanged.

## Profile

```json
{
  "schema_version": "fa.shutdown-fleet/2",
  "plan_archive": "/operator/custody/fleet-plan.bin",
  "operation": "900",
  "coordinator": "/operator/fleet-coordinator",
  "minimum_revision": "0",
  "max_visits": 16,
  "max_head_bytes": 4194304,
  "coordinator_bytes": 16777216,
  "input_bytes": 16777216,
  "runtime_ms": 10000,
  "replay_events": 1,
  "members": [
    { "id": "1", "config": "/operator/config/member-1.json" },
    { "id": "2", "config": "/operator/config/member-2.json" }
  ]
}
```

The archive is the exact output of `FileShutdownPlan::encode`, independently
retained by the operator. It is not a coordinator image or a collection of
successful-stop observations. Every original member configuration remains
mandatory, including unavailable domains. Member IDs and all original profiles
and paths must match the archive. The original decoder enforces the complete
roster and canonical embedded prefixes; decoding executes no historical events.
The separately selected operation, visit allowance and aggregate head allowance
must equal the archive's values. Saved counters or budgets are not new authority.

Version one still requires each member's `history_anchor`. Version two requires
`plan_archive` and forbids those per-member fields. Unknown versions, mixed
formats and missing custody refuse without fallback. Both formats can describe
the SAME native plan bytes, allowing an existing coordinator to resume from its
retained plan without manufacturing new per-member anchors. No native plan or
coordinator wire format changes.

Use the original command:

```sh
cargo run --locked -p fa-reference --example shutdown_fleet -- create /operator/fleet.json
cargo run --locked -p fa-reference --example shutdown_fleet -- resume /operator/fleet.json
cargo run --locked -p fa-reference --example shutdown_fleet -- recover /operator/fleet.json
```

`create` never opens an existing coordinator. `resume` resolves eligible latest
saved visits by passive inspection and only recovers members selected for new
work by the original selector. An unstopped or missing retained image is never
silently retried as a stop command. `recover` explicitly requests new visits,
subject to the existing allowance. Output failure does not resend any effect.

## Cost, custody and failure boundaries

Archive acquisition uses the same private regular-file, device/inode and size
checks as other operator inputs. Its actual bytes share `input_bytes` with the
manifest and all member configurations. The native maximum plan size, individual
journal bounds and aggregate retained-head bound remain unchanged. Custody must
lie outside every authority and coordinator directory; domains themselves need
not be reachable to admit the independent plan. The complete plan and every
configuration pass before any coordinator creation or domain operation.

The existing cumulative elapsed deadline is checked before and after archive
read and decode. Those operations, a replay event and final storage work remain
synchronous, not hard real-time quanta. Canonical-prefix comparison, numerical
bootstrap admission, exact receipt settlement, fixed-roster reporting and
separate domain/coordinator acknowledgments still use the original engine.

Operator authenticity, filesystem isolation, shared clock correctness and
independent retention remain assumptions. Loading an archive does not prove the
fleet is complete relative to the real world, that a member currently stopped,
or that joint rollback of all retained copies was prevented.

## Qualification

Eight new regression functions use actual native plans and coordinator/domain
files: whole-roster stop plus exact visit-ceiling resume; compatibility with the
per-anchor format; mixed completed/unstopped/unvisited crash states; independent
profile/roster/operation/budget pins; closed and malformed inputs; exact cumulative
input-budget neighbors; offline domains versus unavailable or co-located custody;
and every elapsed checkpoint plus failed output after native settlement.
Existing operator tests and assertions are unchanged.

The new source is unqualified. RCH-only targeted example and full xtask commands
were attempted but stopped before Cargo because `rch` is unavailable in the
editing environment. Rust compilation, tests, formatting and Clippy have not
executed. Selected-file source/hash/patch checks are not a full checkout or a
passing build. FA-014 and its original prerequisites remain open.
