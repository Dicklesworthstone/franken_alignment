# Terminal recovery for the journal publication profile

This API contract applies only to `FileDelivery`: the canonical journal is the
publication sink, and candidate replay uses a memory-only endpoint. It is not a
remote-provider transaction, an Asupersync integration, or production qualification.

## Atomic stop and drain

`FileDelivery::stop_and_drain(revision, request, observed_tick)` composes the
original `Stop` and `StopProgress` events in one canonical replacement. It
permanently closes intake, withdraws unsent work, installs the current endpoint
fence and reconciles retained endpoint evidence. Executed and unresolved effects
stay charged. Cancellation, time and absent evidence do not authorize refunds.

Supply the current journal revision, an exact `StopRequest` and a fresh trusted
observation in the profile's clock domain. Every event prefix is checked by the
existing codec and capacity admission before publication. Both records may use
an explicitly installed recovery reserve; ordinary `Time` and `Sweep` records are
not required. This reserves logical journal capacity, not physical disk space.

The return value is acknowledged `FileStopSweep` data, not authority. Examine
its per-attempt outcomes and `progress.drained()`: successful persistence does
not imply all obligations were resolvable. Validation failure leaves canonical
state unchanged, including the local stop. Use the existing `request_stop` alone
when immediate local-stop acknowledgment is required without a trusted clock.
An ambiguous replacement returns no candidate result and poisons the owner
until exclusive recovery. Exact stop retries retain the original stop receipt
but still require a current journal predecessor for this advancing operation.

## Implementation status and change record

Added atomic terminal stop/drain with seven regressions covering permitted
cancellation, endpoint sealing, lost acknowledgments, stale inputs, ordinary
capacity exhaustion, retention expiry and five journal replacement failpoints.
The original event format and reducers are unchanged. No dependency was added.

Rust compilation, formatting, Clippy and tests are UNEXECUTED. The required
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` attempt
exited 127 because `rch` is unavailable; Rust and Cargo are also unavailable.
The retained patch applies to the complete fetched `requests.rs`, verified
against its Git blob identity, and passes whitespace/application checks.
Those checks do not constitute a complete-workspace build or runtime evidence.
No bead is closed and no production feature is activated by this change.
