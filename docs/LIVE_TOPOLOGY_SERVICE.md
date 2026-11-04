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
source after the service has completed without an execution, stop/drain, or
reply-grace failure. A failed service therefore cannot mint a recovery anchor
that is indistinguishable from a clean handoff. Successful retention still
preserves real spent checks, operation floor, source expectation and exact
history. Export failure does not undo acknowledged work and does not trigger an
effect retry.

This increment intentionally refuses topology-supervised `serve-open`, checked
publication profiles and credibility activation. Reopening must consume the
independently retained source/history checkpoint through the complete mediated
recovery path; silently using ordinary `serve-open` would weaken the selected
contract. No continuous watcher, OS-containment proof, new authority, journal
format, effect adapter or executor is added.
