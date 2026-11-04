# Terminal recovery of a mediated publication domain

## Implemented boundary

`FileMediatedRecovery` can complete directly into the original permanently
stopped domain. This serves the operator whose live mediated supervisor is gone
and whose journal may have exhausted ordinary capacity. It implements the
existing recovery/nonresurrection and stop contracts (plan sections 8, 16.4,
16.9 and 17.1; FA-014 with the FA-072/FA-073 mediation requirements), without
promoting any packet or deployment qualification.

After `begin_open_mediated_guarded` or its exact-history-anchored variant, replay
uses the same private cursor, original transitions, exclusive Store and event
quanta. `stop_request(operation)` is available only after complete replay and
full independent requirement validation. It selects the current native stop
predecessor, or the exact request of a matching retained stop. Selection performs
no file I/O and does not acknowledge a stop.

`finish_stopped(request, clock)` returns only the existing
`FileStoppedRecovery`. `finish_stopped_with_anchor` also returns its exact
acknowledged `FileHistoryAnchor`, through the existing `FileStoppedHistory`.
Neither returns an owner, human reviewer, topology observer or approval key.

Both operations recheck the complete mediated requirements: original and current
graphs, pre-recovery availability, optional prediction and evaluation, guard
inventory, effective policy, credential epoch and independently supplied floors.
They do not call the weaker base-profile checker or infer requirements from disk.
Learned histories still require their distinct independently loaded recipes.

## One original terminal transaction

The shared implementation consumes the verified reconstruction through the
original **Stop -> Fence -> StopProgress** canonical replacement. It performs no
preliminary ordinary recovery fence, second replay, new topology observation,
helper inference, human approval or external resend. The original Stop/fence
withdraws old admission and coverage; the native endpoint alone determines
settlement. Unknown or expired outcomes remain liabilities, never inferred
refunds. A stopped/drained domain can still retain charges for executed effects.

Use `require_recovery_reserve(RecoveryReserve::terminal())` before replay when
that exact independently selected reserve was provisioned originally. No reserve
is installed, enlarged, reset or replenished by recovery. All original event,
byte, encoding and source-admission checks remain in force. Terminal completion
can use the reserved three-event/fifty-byte tail when ordinary live recovery
cannot fit its Time/Sweep records.

A matching already-drained stop confirms the unchanged canonical image without a
new clock, event or replacement, including at the exact event limit. The supplied
requirements must describe the post-stop image (including withdrawn topology
availability), not silently reuse the earlier availability assertion. A different
operation or substituted original predecessor refuses even on this no-clock path.

Incomplete/failed/interrupted replay, invalid intent and mismatching requirements
produce no completion. The existing canonical comparison precedes the clock; the
original terminal finalizer rechecks the image after preflight and before staging
cleanup/replacement. A clock unwind or stale tick produces no receipt. Storage
failure can leave a terminal image visible even though no receipt was returned;
recover the actual image and retry the same original stop operation.

## Tests and limits

`guarded/recovery/mediated/stopped_tests.rs` contains nine new regression
functions, using the existing full-input/two-key fixture and original Machine as
the transition oracle. They cover all four original disposition cases under both
anchor selections and three replay quanta, exhausted ordinary space, exact
no-clock retries, stale requests, both final requirement checks, failed clocks,
canonical changes, all five native replacement barriers and failed/interrupted
replay. A compile-fail example keeps a stopped observation distinct from an owner.

These are authored regressions, **not executed evidence**. The remote-only RCH
launcher is unavailable in the editing environment; compilation, Rust tests,
rustfmt, Clippy and the integrated gate remain unexecuted. Re-run on a complete
checkout with the prescribed qualified toolchain:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib guarded::recovery::mediated::stopped::tests
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

This is cooperative event scheduling, not a kernel-I/O or elapsed-time bound.
Topology completeness, storage, clocks and independent archive custody retain
all original trust assumptions. It does not authenticate a topology, kill another
process, prove hardware durability, protect joint journal/anchor rollback, or
provide remote exactly-once effects. External anchor export is separate from the
acknowledged stop; keep the earlier archive until new custody is acknowledged.
