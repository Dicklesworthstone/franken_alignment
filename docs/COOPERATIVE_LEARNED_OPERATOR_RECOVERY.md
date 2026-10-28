# Cooperative learned-publication recovery

## Contract and implementation status · 2026-10-12

The learned publication operator now has one-event recovery checkpoints for its
ordinary, required raw-residual and required owned-K/V configurations. The driver
uses the existing guarded recovery objects, original exclusive Store lock,
canonical bytes, independently supplied recipes, external floors and terminal
recovery reserve. It adds no alternate replay engine or mutable recovery state.

`ready_selected_with_checkpoints` checks once before opening the store, before
each original journal event, and again at Ready. The original test-facing
`open_selected_with_checkpoints` shares that replay; the live path selects its
original finalizer only after the Ready checkpoint. Its callback receives only optional historical progress, not a candidate
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

## Initial checkpoint source and tests

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

## Active command deadlines and Stop admission · updated 2026-10-28

The continuation, next-message and finish commands now connect those checkpoints
to the same deadline and authenticated stop endpoint used by generation and
publication. The command creates both before guarded recovery. Every checkpoint,
including the pre-open and final Ready boundaries, first checks the command's
logical and elapsed-time limits, then polls the existing kernel-credential
admission service. Recovery does not renew either limit or create another peer
admission budget. An unauthenticated candidate does not become a stop request;
the existing finite candidate quota and its refusal behavior remain in force.

An admitted Stop peer now remains bound to the same complete guarded replay.
At Ready, the cursor becomes a sealed terminal target before ordinary finish or
any live-role handoff. Only the peer's complete exact nonce-bound request applies
the original clock-independent admission Stop/Fence and receives the native
receipt. Connection alone, incomplete replay or failed guards cannot mutate
history. The original candidate quota, startup limits and peer session beginning
at kernel admission remain in force; failures cannot fall back to live startup.

Acknowledged history and pending liabilities survive lost replies and late
deadlines. Every selected history/source custody output is attempted independently
before the command reports terminal startup. No helper, next-source refresh,
effect clock, numerical continuation or artificial drain is introduced. See the
[authenticated recovery Stop contract](LEARNED_RECOVERY_STOP_PEER.md) for the
exact operator behavior, custody repair and authored regression coverage. The
new tests remain pending RCH execution; earlier verification records below are
historical and do not qualify this implementation.

For an explicit offline durable stop, use
`create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION`.
It consumes the same fully guarded replay directly into the original terminal
transaction, without an ordinary recovery fence or live-role handoff first.
See [offline learned stop recovery](LEARNED_STOP_RECOVERY.md).

Message and finish `--resume` use the same checkpoints with a separate elapsed
budget of `CONFIG.timing.runtime_ms`, beginning immediately before guarded open.
They do not consult the supplied live logical-clock function during replay,
persist a new clock observation, read live policy evidence, acquire a forecast,
or renew the retained request deadline. A completed terminal receipt still
settles entirely from its original record. These receipt-only commands have no
reviewer profile and do not create a stop endpoint.

These checks bound continuation at **event boundaries**. Initial file reading,
decoding and recipe binding, one original numerical replay event, and final
encoding/synchronization remain synchronous. A single such operation can exceed
the configured elapsed budget; there is no interruptible-kernel latency claim.
History-anchor selection, original guards and reserve, the single successful
fence, and final anchor-retention behavior are unchanged.

The integration's regression sources are `tests/replay.rs`,
`tests/recovery_control.rs` and `tests/receipt_replay_deadline.rs`. They compare
the cooperative result with original full-batch replay, inject expiry at every
checkpoint, and exercise actual Unix peer admission. Positive counterparts
retain numerical state and the original fence; cancellation counterparts retain
all canonical and staged bytes and release the writer lock. Seven new regression
functions are authored and independently source-reviewed. The actual continuation
entry is tested as well as its helpers; existing terminal message/finish tests
retain panic-on-live-time callbacks through the changed receipt path.

Six changed Rust files passed older, unqualified rustfmt syntax parsing only;
the parser output did not rewrite source and is not formatting or type-check
qualification. Both the learned example target and the full
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` dispatch
stopped before compilation because `rch` is absent (exit 127). The source snapshot
includes the complete parent tree and this integration; raw attempt logs are
outside it. No compile/test result is claimed. The scoped bead remains in
progress; the full FA-014 packet and its prerequisites remain open.
