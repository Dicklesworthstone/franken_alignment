# Cooperative learned-publication recovery

## Contract and implementation status · 2026-10-12

The learned publication operator now has one-event recovery checkpoints for its
ordinary, required raw-residual and required owned-K/V configurations. The driver
uses the existing guarded recovery objects, original exclusive Store lock,
canonical bytes, independently supplied recipes, external floors and terminal
recovery reserve. It adds no alternate replay engine or mutable recovery state.

`open_selected_with_checkpoints` checks once before opening the store, before
each original journal event, and again at Ready before invoking the original
finish. Its callback receives only optional historical progress, not a candidate
machine, source writer, human key or consistency observer. Rejecting a checkpoint
or unwinding drops the recovery object without staged-file cleanup, fencing,
role issuance or publication. A subsequent explicit attempt must reopen and
revalidate the original history; there is no automatic restart or work refund.

Successful completion still executes the original guard and floor checks, exact
canonical-cut comparison, cleanup and acknowledged fence. Learned inference is
paused, old approval keys remain revoked, and unresolved forecasts retain their
spent evidence and lost coverage. Completed receipt recovery retains its old
no-new-clock/no-new-forecast contract. Recipe and journal formats are unchanged.

One event can include a complete synchronous numerical step. Framing, immutable
recipe admission and final persistence also remain synchronous. These are event
boundaries, not an interruptible kernel, wall-clock latency bound or resource
preallocation guarantee. Supervisor checkpoint code must not mutate the source
store behind the exclusive owner.

## Source and tests

The implementation is in the learned operator's `recovery.rs` and
`recovery/cooperative.rs`. Seven driver tests cover exact ordering, every cancel
cut including Ready, empty histories, original reducer failure, invalid progress,
interrupted state and final persistence failure. Three loaded-owner tests in
`tests/recovery_quanta.rs` exercise actual numerical reconstruction in all three
modes, unchanged canonical/staged bytes on cancellation, lock release on unwind,
unchanged external-floor enforcement, numerical parity and unrepaired pending
forecast coverage on successful recovery. Fixture weights and probabilities are
controlled inputs, not calibration evidence.

These tests are authored, not executed. The required command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication`
stopped before compilation because `rch` is absent (exit 127). This environment
also lacks Cargo, rustc and rustfmt. No Rust compile, test, formatting, Clippy,
workspace or production qualification is claimed.

This is scoped implementation work for the recovery obligations of FA-014 and
the forecast-publication integration in FA-111, under plan sections 8, 11 and
16.4. It preserves the existing L5 effect boundary and L4 authority rules; no
new agent verb, evidence type, runtime dependency or founding-plan semantics is
introduced. The full packets and their prerequisites remain open. Beads were
read but not modified because the required `br` command is unavailable.
