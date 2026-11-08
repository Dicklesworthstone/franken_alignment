# Topology-supervised live actor creation

`supervise_publication serve-create` can now select the existing
`fa.mediated-create/1` or `fa.mediated-create/2` topology profile:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference --example supervise_publication -- \
  serve-create CONFIG.json ACTOR.json REVIEWER.json --topology-profile TOPOLOGY.json
```

The selected path reuses the native bounded topology/inventory loader, reserved
mediated bootstrap and original `FileSupervisedDriver`. The first canonical
image contains the publication guard, policy-source guard, terminal recovery
reserve and uncertified graph. Before actor intake, the same source custodian is
installed in the driver, a real native source refresh must produce a verified
cut, and a later time observation is committed so a slow topology read cannot
freeze an earlier time boundary.

The normal socket peer checks, fresh policy capture, helper processes, separate
human approval, two-key dispatch, publication, reconciliation, stop/drain, reply
grace and child cleanup remain unchanged. First publication still executes the
driver's mandatory final topology refresh after evidence capture and before the
final publication clock.

Version 2 creation profiles retain their existing `retain_checkpoint` path.
The live service exports that checkpoint only from the actual driver-owned
source after execution, stop/drain and reply-grace processing complete without
error and native checkpoint validation succeeds. Helper cleanup runs afterward
and can still make the command fail. The retained checkpoint acknowledges
source/history custody; it does not attest that all direct children were reaped.
Successful retention preserves real spent checks, operation floor, source
expectation and exact history. Export failure does not undo acknowledged work
and does not trigger an effect retry.

The existing checked form is described in
[live topology and publication witnesses](LIVE_CHECKED_TOPOLOGY_SERVICE.md).
On Linux, both create forms also accept `--tower-generation GENERATION` to use
the common Tower request schema, with an optional separate `--operator-profile`.
The [Tower composition](TOWER_PEER.md#preserve-topology-enforcement-through-tower-admission)
uses this same native bootstrap, source custodian and final publication refresh.
It accepts the option pairs in any order after all positional files. Operator
read custody does not replace topology certification, and its socket cannot
alias a retained checkpoint destination.

The service still refuses topology-supervised ordinary `serve-open`, request
schedules, precheck disclosure and credibility activation. Reopening must consume the
independently retained source/history checkpoint through the complete mediated
recovery path; silently using ordinary `serve-open` would weaken the selected
contract. No continuous watcher, OS-containment proof, new authority, journal
format, effect adapter or executor is added.
