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

This option requires `--retain-history-anchor` and plain `create`. It cannot be
used to open an existing store, reinterpret a recovery request, or silently
replace a checked-witness, learned, stream, credential or evaluation profile.
`create-checked` with topology refuses; its earlier meaning remains unchanged.
The unselected legacy creation path remains the original implementation.

## One original owner from creation to publication

The command composes the [native reserved bootstrap](TOPOLOGY_OPERATOR_BOOTSTRAP.md)
with the existing creation/continuation workflow. Its FIRST canonical image
contains the mandatory original publication and policy-source guards, terminal
recovery reserve, and uncertified native graph. It returns no intermediate
unguarded or unreserved owner to the workflow. Invalid source/graph/profile
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

A retained creation archive is valid independent input to the explicit
[mediated offline-stop command](MEDIATED_STOP_OPERATOR.md). Supply the independently
retained initial/current graphs, actual pre-recovery availability, exact source
profile and floors. The stop path does not require live policy evidence or helper
programs. It preserves published charges and supports the original no-clock
already-drained retry. Creation does not infer those later requirements for the
operator or automatically restart a stopped domain.

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

Nine new regression functions in `workflow/continuation/creation/topology_tests.rs`
exercise the actual top-level create command followed by offline mediated stop
and exact retry; real helper processes and human sockets; inventory loss after
dispatch; a bypass at initial admission; malformed/mismatching/insufficient input;
existing-store refusal; post-capture deadline expiry; human rejection; output
races; and closed command/profile selection. The original helper fixture returns
controlled ballots: it is enforcement plumbing, not a calibrated model or proof
of reviewer authenticity. Existing tests and native reducers remain unchanged.

These tests are **authored, not executed evidence**. The editing environment has
no RCH launcher; required launches stop before compilation. Rust compilation,
rustfmt, Clippy and qualification remain pending. This advances the existing
plan sections 8, 16 and 17 and the mediation/publication integration, without
closing FA-014, FA-072/FA-073 or any production gate.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --example supervise_publication workflow::continuation::creation::topology_tests
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
