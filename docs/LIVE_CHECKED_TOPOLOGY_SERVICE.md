# Live actor service with topology and publication witnesses

The Linux reference publication example composes the existing authority-topology
source with plain final-publication witnesses on its live actor endpoint:

```text
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --topology-profile TOPOLOGY_PROFILE
```

The original plain `serve-create ... --topology-profile ...` form is unchanged.
The checked form requires a distinct positional witness profile before the
`--topology-profile` option. Both forms use the existing actor protocol, peer
credentials, helper processes, independent human-review channel and original
publication driver; there is no alternate effect executor or outcome reducer.

## Two independent controls on the same publication

The topology profile is the original `fa.mediated-create/1` or
`fa.mediated-create/2` document. Its graph, inventory, external input custody,
source budget, initial certification and final automatic refresh retain their
existing semantics. Selecting version 2 retains a topology-source checkpoint
under the existing healthy-service-only output rule.

The witness profile for this composition is `fa.supervised-witnesses/1`. It
names the original and current capture files, a bounded nonempty witness recipe,
and native publication limits. The original witness constructors bind the
reviewed action and its required values, absences or range membership. The
existing final publication path rechecks the current capture rather than treating
an earlier helper or human approval as proof that the inputs remain unchanged.

Both controls are selected before actor intake. The SAME native mediated
bootstrap includes the source policy, topology, publication validation limits
and terminal reserve in the first canonical image. It does not create a plain
host and attempt to enable the missing witness guard later. An immutable
deployment check confirms the requested publication limits, source policy,
reserve, publication guard and configured clock before actor admission.

Passing one control cannot replace the other. A changed required witness or a
missing topology source can block publication after dispatch even though the
initial graph was certified and human approval was obtained. An unrelated
current-capture change may still pass the original semantic witness checks.
Human rejection remains authoritative even when both controls pass. Existing
stop, unresolved-outcome accounting, reply grace and helper cleanup are unchanged.

## Explicit limits, no profile downgrade

Feed-backed, waiting, snapshot-fallback and joint publication profiles carry
additional native contracts. The currently available mediated bootstrap cannot
preserve them by copying only `PublicationLimits`. The service uses the existing
plain-profile selector and refuses those combinations before logical-clock
acquisition, topology reads, socket binding or journal creation. It does not
retry with a weaker profile.

Likewise this change does not enable topology-supervised `serve-open` or
`serve-open-checked`. Reopening requires the original independent mediated
history and source-work recovery contract; a creation profile cannot stand in
for that contract. Sequence, multi-peer, native-text, credibility-activation and
receipt-mode flags are not admitted by the closed topology-creation grammar.

A malformed or unreadable witness profile is a startup error. It cannot select
the plain endpoint. An existing socket path is not replaced. No new wire command,
journal tag, dependency, credential capability or statistical qualification is
introduced.

## Regression scope and verification status

Seven regression functions in
`examples/supervise_publication/workflow/actor_service/topology/tests.rs` cover
closed argument roles and outer routing; exact nonmutating deployment checks;
real live actor/helper/human exchanges; post-dispatch witness and topology
failures; unrelated-change allowance; human rejection; the actual command's file
admission; initial bypass and socket collisions; stronger-profile refusal before
clock or source access; and rejection of checked topology recovery and malformed
witness inputs. Synthetic helper ballots exercise enforcement plumbing, not
model accuracy, host isolation or custodian authenticity.

These tests are authored and source-reviewed, not executed. In the editing
runtime the targeted example tests, full `xtask check`, rustfmt and Clippy were
attempted through `RCH_REQUIRE_REMOTE=1 rch exec -- ...`. Each failed with exit
127 because `rch` was missing, before compilation or test execution. No build,
formatting, Clippy, runtime or release pass is claimed. Existing qualification
records, acceptance gates and Beads statuses are unchanged.

## Scoped execution update (2026-10-24)

All seven original functions now execute successfully within the 38-function
combined selection at `9a8cf8f0`, using the qualified standard compiler/default
stack. This includes actual actor/helper/human sockets, permitted publication,
human rejection, witness/topology loss, unrelated-change allowance and original
startup refusals. The outer-routing fixture now expects the learned grammar
diagnostic for learned-owned topology arguments, while every original direct
topology refusal retains its exact diagnostic. No production routing changed.
The earlier exit-127 attempts remain historical failures. Exact source, counts
and the unchanged canonical dependency-admission refusal are recorded in
`IMPLEMENTATION_STATUS.md`; complete project qualification remains open.
