# Current topology in the supervised publication driver

## Capability: October 14, 2026, unqualified source

The original FileSupervisedDriver now exposes explicit topology-aware concrete
file operations for socket/process review launch, completed review, independent
human approval requests, automatic reservation, dispatch and first publication.
They consume the existing FileTopologySource custodian and FileEvidenceSource
(or the original admitted EvidenceFile implementation); no caller-supplied
snapshot, graph verdict, policy reducer or endpoint replaces those owners.
This serves FA-072/FA-073, plan 15.2/15.6/17.1 and founding FI-A02/FI-A06.

`start_file_review_with_topology` and `start_file_process_review_with_topology`
validate the original request, input revision, job and worker roster before
reading topology. Only a successful original topology check allows policy input
capture and original helper admission. Failure cannot launch a worker or retry
through manual review. Their existing non-topology counterparts preserve the
same original body through a private, non-exported pre-capture hook.

`step_from_file_with_topology` decorates the ORIGINAL private evidence provider.
It checks topology before each policy capture, not before selecting the job
phase. In particular the second capture after reservation checks topology again
before native dispatch. `request_human_approval_from_file_with_topology` uses
the same original human-request implementation, with the same before/after
input-cut checks. The credentialed step variant still requires the original
separate credential and both native approval keys.

## Loss is not permission or an outcome

An acknowledged topology withdrawal produces missing evidence; original policy
capture does not run. During first publication this reaches the original
checked endpoint's nonexecution seal rather than accidentally skipping directly
to reconciliation. The charge stays until the original endpoint outcome is
reconciled. A failure to acknowledge the topology operation is an outer journal
error and cannot attempt publication afterward. The original fault/unwind and
query-only recovery behavior remain unchanged.

TopologyReport retains each actual topology Result alongside the unchanged
original operation report. A step can contain two topology and two policy
observations; approval and launch have at most one. Empty topology observations
mean the capture stage was not reached, not success. A successful graph check
cannot hide missing policy evidence, changed helper input, expiry or a stronger
configured guard. All original post-capture clock observations remain in place.

Reconciliation, idle work, already recorded endpoint outcomes and already
expired effects do not acquire fresh topology or spend its allowances. A later
source failure cannot erase an original executed receipt. Operation IDs advance
inside the same custodian used by explicit refresh/rebind; they cannot restart
per driver call. Exhausting that counter quarantines the original owner without
wrapping, reading files or returning a stale-cut success.

These are explicit supervisor APIs, not an automatically mandatory CLI profile,
background watcher, authenticated inventory or OS containment proof. The original
caller must provision and retain the source outside actor custody. No new
persisted flag, journal encoding, authority type, executor or dependency is added.
The source remains process-local and cannot attach to a recovered owner. Reads,
verification, historical reconstruction and synchronization remain synchronous;
no latency or throughput improvement is measured.

## Verification status

Eight authored regression functions exercise actual files, the original actor
gateway, original socket commit/reveal, human key, dispatch, publication and
reconciliation. Controls cover positive publication, stale launch, loss before
helper/policy admission, human requests, loss between the two dispatch captures,
loss after dispatch, valid graph with missing policy, post-capture expiry,
all five original storage barriers and operation-counter exhaustion. Socket
Allow ballots are test controls, not native helper-model or accuracy evidence.
Existing test bodies, authority/checker and endpoint reducers are unchanged.

The targeted and full gate must execute through the prescribed RCH path. In this
preparation environment the launcher is absent, so compilation and Rust tests
remain UNEXECUTED. Source/hash/whitespace checks are not Rust type, format,
Clippy, runtime, performance or production qualification. No beads are closed.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::mediation::source::tests::driver
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
