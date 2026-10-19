# Runnable offline fleet shutdown and resume

Unqualified source implementing the operator consumer of FA-014, plan
16.4/16.5/16.9 and FI-A02. The `shutdown_fleet` example drives the existing
registered-domain coordinator and cooperative recovery/resolution. It does not
introduce another stop reducer, rights ledger, runtime or journal format.

For supervisors still holding their live journal locks, use the authenticated
[online fleet Stop command](LIVE_FLEET_STOP.md). This offline coordinator
requires journal ownership and does not contact active stop endpoints.

## Invocation and independent inputs

From a built, qualified Unix executable:

```sh
shutdown_fleet create /custody/fleet.json
shutdown_fleet resume /custody/fleet.json
shutdown_fleet recover /custody/fleet.json
```

The closed `fa.shutdown-fleet/1` profile is operator-only configuration:

```json
{
  "schema_version": "fa.shutdown-fleet/1",
  "operation": "900",
  "coordinator": "/operator/fleet-stop",
  "minimum_revision": "0",
  "max_visits": 128,
  "max_head_bytes": 4194304,
  "coordinator_bytes": 16777216,
  "input_bytes": 16777216,
  "runtime_ms": 30000,
  "replay_events": 8,
  "members": [
    {
      "id": "1",
      "config": "/custody/domain-1.json",
      "history_anchor": "/custody/domain-1.anchor"
    },
    {
      "id": "2",
      "config": "/custody/domain-2.json",
      "history_anchor": "/custody/domain-2.anchor"
    }
  ]
}
```

Each CONFIG uses the original `supervise_publication` version-one format and
independently supplies its complete bootstrap and original absolute store path.
No helper program, source file or reviewer socket is opened: those CONFIG fields
are only parsed. Each archive is an independently retained ORIGINAL
`FileHistoryAnchor::write_to` image, not bytes copied from a current journal and
relabeled as custody. `FileShutdownDomain::from_history_anchor` checks its exact
profile/path/event-count binding. Original coordinator semantic replay still
validates the actual domain and newest retained prefix when visited.

All roster/config/anchor intake finishes before coordinator creation or member
intent. A missing domain remains registered; missing independent configuration or
custody instead refuses the whole command before any domain work. Archives and
configs must be private regular files, not final-component symlinks, and must lie
outside every domain and coordinator directory. Opened file identity and mode are
rechecked. The coordinator's existing parent must have its canonical spelling;
coordinator/domain directories cannot overlap. Custodian authenticity, controlled
parent directories, stable files and filesystem isolation remain assumptions;
these checks do not authenticate an adversarial administrator.

Keep the SAME original registration archives and plan limits when reopening the
coordinator. Its native journal already retains newer observed heads; replacing a
registration changes the plan and is rejected. Raise `minimum_revision` to an
independently retained known coordinator floor. This is a numeric floor, not an
exact anti-fork anchor for the coordinator; joint rollback remains outside this
profile. No argument is inferred from an untrusted domain's current counters.

## Three explicit operations

`create` exclusively creates a NEW coordinator, then performs one new recovery
pass. It requires minimum_revision zero and never falls back to opening an
existing directory. A lost creation acknowledgment is resolved explicitly, not by
silently starting a new operation.

`recover` opens the exact original coordinator/plan and requests one NEW bounded
visit for every fixed member. It is the explicit choice for further native stop
or drain work after a previous refusal or a stopped-but-undrained observation.

`resume` uses the existing native resume selector: latest eligible Entered or
Observed visits take passive resolution; other members take new recovery. A
retained-visit evidence refusal NEVER falls through into a stop retry. Old success
is reread, not counted as current success. Exact observed-cut refreshes add no
domain or coordinator records. Missing/unstopped retained evidence is reported
separately; use an explicit new recovery request for further native progress.

The full roster, registered clock bindings, visit allowance and revision growth
remain native admission rules. Busy/offline members or acknowledged native
refusals allow reachable members to proceed. A failed coordinator acknowledgment
stops the pass, with every later member explicitly NotVisited. A domain may have
already settled when its coordinator acknowledgment fails; neither that error nor
cancellation is proof of effect nonexecution.

## Deadline, clocks and output

One monotonic elapsed budget begins before profile intake and is checked during
intake, before coordinator acquisition, before each member and between bounded
original event quanta, including Ready before finalization. Expiry before a visit
leaves it NotVisited; expiry during new recovery acknowledges a native
Unavailable refusal without changing that domain. During passive resolution it
withdraws session success without rewriting the saved visit. Prior completed
members are still reported. A callback unwind is not swallowed or converted into
a successful observation; the original entered intent requires reopening.

The executable samples Unix-epoch milliseconds once for the pass; every CONFIG
must name the existing FAUNIXMS clock domain. Native resolution and already
drained rereads do not consume that supplied tick or append time. Replay does not
renew it, a deadline or a resource budget. Wall-clock rollback can still cause the
original native stale-time refusal. Initial decoding, a single original event,
final storage and output are synchronous; this is not a hard real-time deadline.

The JSON `fa.shutdown-fleet-result/1` lists EVERY member and its actual native
classification, including Refused, EvidenceRefused, Unacknowledged and NotVisited.
Observed members include original stop/charge/unresolved data. IDs and counters
are decimal strings; errors use fixed labels and native I/O stages, not evidence
bytes or raw diagnostics. No partial plan is labeled complete. A partial,
undrained or timed-out pass emits its report AND exits nonzero. Write or flush
failure reports the last acknowledged coordinator revision and never retries an
effect. Retain the independent floor and use explicit resume after lost output.

## Bounds and verification

At most 64 members and 512 native visits; all original journal, per-domain and
retained-head limits still apply. The manifest is at most 256 KiB, each reused
CONFIG at most 2 MiB, an anchor at most its original 16 MiB plus framing, and
`input_bytes` bounds cumulative file payload intake to at most 128 MiB. The
aggregate canonical anchor payload also stays within max_head_bytes. The
coordinator encoder separately bounds its duplicated plan/head evidence to its
16 MiB maximum; there is no silent truncation or evidence-capacity increase.
Counts bound logical inputs and output records, not peak allocator overhead.

Eight new operator tests use real CONFIG files, original anchors and native
coordinator/domain journals: successful create and exact resume; busy/missing
members plus a reachable control; every elapsed checkpoint; closed parsing and
private custody; plan/floor mismatch before cleanup; lost stdout/flush with exact
resume; original two-key reserved/sent/executed/unknown accounting; and an unwind
after actual durable intent with evidence-only resume. The six native registration
tests supply independent plan-equivalence and rollback controls. No existing test
assertion was weakened. The new zero-event fixture uses the native canonical
member path for Unix systems whose temporary directory has an alias.

Required exact-revision target tests and `cargo run --locked -p xtask -- check`
were attempted through `RCH_REQUIRE_REMOTE=1 rch exec`; the launcher is absent,
so no Rust compilation, test, formatting or Clippy pass is claimed. This worktree
contains selected files, not a full checkout. No FA-014 closure, production
activation, authenticated producer or simultaneous cross-domain stop is claimed.
