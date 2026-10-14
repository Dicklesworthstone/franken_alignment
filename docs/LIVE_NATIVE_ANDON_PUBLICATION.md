# Native accounting on live publication paths

Plan sections 9.9, 16–18; scoped continuation of credibility-ledger-dc0.4.
Source implementation, not execution qualification. No bead or gate is closed.

## Reserved first image and reconciled recovery

`FileOversight::create_andon_guarded_reserved` prepares the original guard set,
explicit recovery reserve, native evaluation protocol and Andon configuration
before returning an owner. The original reserve is installed immediately after
the first publication/stream bootstrap event, before guard events count as work.
Invalid reserve inputs fail before storage creation. Canonical encoding still
checks every prefix and the actual path-dependent byte limit before publication;
a later creation failure can leave an unacknowledged directory, never live roles.
The existing unreserved constructor and its bytes remain unchanged.

`FileAndonRecovery::finish_reconciled` uses the same original Fence/Time/Sweep
completion as base guarded recovery after checking the full independently held
Andon configuration, windows, protocol, history and floors. It returns the owner,
separate evaluated roles, and every original reconciliation result. The caller
must inspect outcomes AND remaining unknown/irrecoverable liabilities before
admitting new work. Returning an owner does not certify settlement.

The entire verified history is used once. The fixed original transaction owns
capacity checks, exact canonical comparison before clock and before cleanup,
replacement barriers and role publication. No new reducer, journal encoding,
effect resend or saved-score import exists. The trusted clock is called only
after verification; unwinding drops the locked cursor without issuing roles.
Ordinary Time/Sweep cannot spend a terminal-only reserve. This API bounds no
wall time and does not authenticate clocks or evaluators.

The consumer is the explicitly selected native-accounting publication workflow:
new domains need the same terminal capacity required by existing publication;
recovered domains need both native evaluation custody and settlement of prior
liabilities. Neither is replaced by a less constrained legacy opener.

## Verification

Seven authored native regressions cover reserved/legacy first-image controls,
invalid reserve admission and all original bootstrap barriers, complete original
replay parity for unexecuted/dispatched/published/expired effects, fresh evaluator
continuation, incomplete/changed canonical refusal, terminal-only capacity, and
all five completion barriers. Original base reconciliation tests and fixtures
are unchanged; the same original two-key fixture is reused for actual effects.

Required targeted tests and the unchanged xtask gate were attempted through
`RCH_REQUIRE_REMOTE=1 rch exec`. Both exited 127 before Cargo because rch is not
installed. Rust compilation, test execution, rustfmt and Clippy are unverified.
Selected-file source checks do not establish runtime or production correctness.
