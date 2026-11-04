# Topology-supervised initial publication

## Operator entry point

The existing explicit-archive creation command can select topology supervision:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- create CONFIG.json SUBMIT.json \
  --retain-history-anchor /operator/custody/created.anchor \
  --topology-profile /operator/custody/create-topology.json
```

The original `--reviewer-profile REVIEWER.json` option can also be supplied. It
retains the original reviewer-admission requirements; topology declarations do
not authenticate a reviewer. The topology profile is a closed, bounded input:

```json
{
  "schema_version": "fa.mediated-create/1",
  "topology_path": "/operator/topology.json",
  "inventory_path": "/operator/inventory.json"
}
```

Those paths contain the original `fa.authority-topology/1` document and native
perimeter inventory. Paths must be absolute, distinct and without parent
traversal, and resolve outside the authority store. The existing regular-file
reader rejects final-component symlinks and oversized files. The native parser
retains the complete input objects, including metadata outside the selected
family. The graph's scope and target must match CONFIG exactly. Trusted parent
directories and independently provisioned input contents remain required.

This option requires `--retain-history-anchor`. Both `create` and
`create-checked` can select topology supervision. Checked composition admits only
the plain `fa.supervised-witnesses/1` profile; feed, waiting, snapshot and joint
profiles refuse before topology intake or store creation instead of losing their
additional guards. The unselected legacy paths remain unchanged. Initial
creation still cannot open an existing store or reinterpret recovery intent.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- create-checked CONFIG.json SUBMIT.json WITNESSES.json \
  --retain-history-anchor /operator/custody/created.anchor \
  --topology-profile /operator/custody/create-topology.json
```

The checked profile's original/current captures, immutable witness recipe and
validation budget pass to the SAME continuation and final publication gate.
They are not replaced by the topology cut. The original capture is bound before
helper launch; current witness inputs are reacquired at the existing evidence
boundaries. After dispatch, the original final-publication path acquires inputs,
refreshes the driver-owned topology, observes time, and runs the native guard.
Both predicates must hold; neither human approval nor a previously verified
cut can waive missing or changed witness evidence.

## One original owner from creation to publication

The command composes the [native reserved bootstrap](TOPOLOGY_OPERATOR_BOOTSTRAP.md)
with the existing creation/continuation workflow. Its FIRST canonical image
contains the mandatory original publication and policy-source guards, terminal
recovery reserve, uncertified native graph, and selected witness-validation
limits. There is no later witness-enablement write. It returns no intermediate
unguarded, witness-free or unreserved owner to the workflow. Invalid source/graph/profile
configuration does not become a weaker fallback.

The separately issued topology observer is consumed into the original
`FileTopologySource`, with fixed paths, complete expected input, lifetime
allowances and work. That same source is installed in `FileDriverTopology` before
any actor submission. After the original fresh policy capture, an explicit
native topology refresh must obtain a `Verified` cut before the original actor
gateway receives the proposal. A later clock observation checks the action and
workflow deadlines and advances the original source-lease clock; a slow topology
read cannot preserve eligibility by freezing the earlier time.

The remainder is the original helper-process review, independent human approval,
automatic/human two-key dispatch, publication and reconciliation workflow.
At first publication the existing driver-owned topology path refreshes the same
source AFTER policy/input acquisition and before the native final clock and
publication gate. A caller cannot omit it by choosing another stepping method.
Loss, substitution or exhausted source allowances reach the native withdrawal
and sealing behavior; failed persistence remains unconfirmed, not permission to
publish. No new permit, source getter, effect adapter or parallel reducer exists.

This is explicitly initial-admission and first-publication supervision, not a
continuous watcher or a promise to reread topology at every helper/human/dispatch
boundary. The driver must retain custody for its lifetime. This command does not
add a persisted mandatory-source flag or extend later live-recovery commands;
those commands cannot be assumed to preserve this selection without their own
explicit integration. The graph remains a declared perimeter, not an empirical
proof of operating-system containment or inventory completeness.

## Failure, custody and recovery

Existing create-only admission refuses an existing authority store. An initial
native cut refusal occurs before actor admission and retains the healthy
acknowledged history through the original archive path. After admission, the
original stop/drain and helper cleanup paths handle workflow failures. No error
or canceled task erases an external-effect obligation or resends the effect.

Archive retention runs under the same acknowledged owner, before child cleanup.
It uses the original private create-new output and file/directory synchronization.
A racing output file or lost output cannot undo publication or overwrite earlier
custody. A failure can leave an acknowledged bootstrap, hold, stop or publication
without a successfully exported archive: export is separate, not atomic with
every journal commit. Read the original response and retention error together.

For source-only creation, a retained archive is valid independent input to the explicit
[mediated offline-stop command](MEDIATED_STOP_OPERATOR.md). Supply the independently
retained initial/current graphs, actual pre-recovery availability, exact source
profile and floors. The stop path does not require live policy evidence or helper
programs. It preserves published charges and supports the original no-clock
already-drained retry. Creation does not infer those later requirements for the
operator or automatically restart a stopped domain. Checked creation retains the
witness guard in its archive as well; recovery must retain that exact profile.
This composition does not extend or qualify additional recovery command modes.

## Bounds and verification status

The envelope is bounded to 16 KiB, depth 2, 8 JSON items and 4096-byte strings.
Initial intake uses the original per-file topology and inventory bounds. It is
separate from the original source custodian's work: initial admission and final
publication each reserve `TOPOLOGY_REFRESH_READ_BYTES` through that same source.
A profile with fewer than two checks or insufficient read allowance for those
two checks refuses before storage creation. No allowance is reset at handoff.

One elapsed workflow budget spans preparation and execution, with checks around
initial file intake and after the initial topology refresh. Configuration,
proposal, reviewer-profile and topology-envelope parsing precede this budget.
Parsing, file reads, native replay/checking and storage barriers remain synchronous;
no kernel-latency bound, measured memory saving or throughput claim is made.

The original nine regression functions in `workflow/continuation/creation/topology_tests.rs`
exercise the actual top-level create command followed by offline mediated stop
and exact retry; real helper processes and human sockets; inventory loss after
dispatch; a bypass at initial admission; malformed/mismatching/insufficient input;
existing-store refusal; post-capture deadline expiry; human rejection; output
races; and closed command/profile selection. The original helper fixture returns
controlled ballots: it is enforcement plumbing, not a calibrated model or proof
of reviewer authenticity. Existing tests and native reducers remain unchanged.

Six further functions in `workflow/continuation/creation/checked_topology_tests.rs`
cover the first combined native image without capture reads; the actual checked
command with real child/reviewer plumbing; post-dispatch phantom insertion,
missing witness data, missing topology inventory and unrelated changes; human
rejection; refusal of stronger feed/wait profiles before clock or topology I/O;
and closed positional/option parsing. The existing regression assertions and
native algorithms are unchanged.

These tests are **authored, not executed evidence**. The editing environment has
no RCH launcher; required launches stop before compilation. Rust compilation,
rustfmt, Clippy and qualification remain pending. This advances the existing
plan sections 8, 16 and 17 and the mediation/publication integration, without
closing FA-014, FA-072/FA-073 or any production gate.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --example supervise_publication workflow::continuation::creation::topology_tests
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --example supervise_publication workflow::continuation::creation::checked_topology_tests
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
