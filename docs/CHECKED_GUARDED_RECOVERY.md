# Exact publication profiles during guarded recovery

`FileGuardedRecovery::require_publication_profile` binds independently selected
publication gates to the original locked canonical image before replay. It
composes with the existing exact history anchor, full guard inventory, policy,
credential epoch, numeric floors and terminal-reserve requirement.

```rust,ignore
let mut recovery = FileOversight::begin_open_guarded_anchored(
    directory, bootstrap, &guards, &anchor,
)?.require_publication_profile(publication)?
 .require_recovery_reserve(RecoveryReserve::terminal())?;
// Drive the same original cursor, checking cancellation including at Ready.
let recovered = recovery.finish_reconciled(trusted_clock)?;
```

`FilePublicationRecoveryRequirements` explicitly selects optional witness
validation limits, an optional original `JointPublicationFeed`, and an optional
`HeldOutJointPolicy`. None requires absence. The feed pins its source, starting
cut, lookup steps/bytes, freshness clock domain/age and exact-snapshot fallback.
The joint policy pins the original policy identity, thresholds and budgets.
Duplicate configuration records and unselected gates refuse. Prefix routing is
the supported contract here; subtree routing is not treated as equivalent.

The selector consumes a cursor on mismatch or late selection without a clock,
fence, staging cleanup or role. Its event vector is already bounded, private and
immutable. Therefore the selection remains attached to the same image throughout
replay and all existing completion routes without another journal or stored
configuration. Successful selection is not successful replay: the complete
original reducer still checks ordering, observations, every suffix event and
all independent guards and floors. No source file is opened by this check.

This is configuration admission, not live authorization. Recovery still
withdraws original eligibility and approvals, and cannot revive a saved lease,
copy a permit or refund an unknown effect. `finish_reconciled` uses the existing
Fence/Time/Sweep transaction and ordinary capacity. The stopped completion keeps
its own terminal reserve and acknowledgment semantics. No existing constructor,
finish implementation, event format, dependency or actor command is changed.
The selector is explicit; older callers retain their existing admission contract.

Seven new native regression functions cover six supported compositions and
three replay quanta against the original full-replay/canonical-byte oracle;
absent gates and individual policy/budget mismatches; late selection; independent
floors and invalid suffixes; cancellation at every event cut; all five original
Store failure barriers and visible-cut retries; and subtree-versus-prefix
routing. Positive controls use original constructors and completion routes.

These tests have not executed in this environment. Required targeted tests and
the unchanged xtask gate were attempted through RCH_REQUIRE_REMOTE=1 rch exec,
but the launcher is absent and both stopped before Cargo (exit 127). Rust
compilation, test, formatting and Clippy qualification remain pending. Source
inspection and selected-file hash/patch checks are not execution evidence.
FA-014 and its broader prerequisites remain open. This scoped capability serves
plan 8, 16.4/16.5/16.9 and 17.1, and FI-A02, without claiming production safety,
independent custodian authentication, remote settlement or hard latency bounds.
