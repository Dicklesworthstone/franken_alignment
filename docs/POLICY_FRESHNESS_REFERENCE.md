# Live policy-observation freshness

Consumer: the existing DeliveryBroker policy-state prerequisite, forwarded through OversightBroker to the actor supervisor and supervised driver. This extends the observation-source and witness work in FA-017/FA-063/FA-058, plan sections 7, 8.3, 14 and 17.1. The producer remains a trusted observation adapter. A source's declared closed prefix is not evidence that observation is still running.

## Source and admission contract

`StateFreshness::new(max_age_ticks)` selects a positive, fixed maximum age in the controller's elapsed-clock domain. `enable_fresh_policy_state` installs that policy before any proposal. The returned original PolicyStateWriter must use `close_observed(through, marker_generation, observed_at)`. Untimed closure and untimed capture cannot bypass the selected profile. The explicitly untimed legacy profile remains available and makes no observation-age claim.

Every leased capture binds the complete original StateFrontier and the half-open interval `[observed_at, observed_at + max_age_ticks)`. Overflow refuses. The timestamp is the adapter's observation time, not the later completion time of unrelated computation. The consumer refuses a timestamp in its future and refuses at the exact expiry tick. Consumer clock checks retain a monotone floor even on expiry or missing evidence, so backdating a subsequent read cannot resurrect a previously eligible cut.

An exact repeated event or closure remains idempotent and cannot extend expiry. Replacing the timestamp on an existing closure refuses, and a new marker over the same observed prefix cannot renew it. A healthy adapter renews by recording newly observed state at a new sequence and closing it with a new marker; unchanged values may be recorded as a new complete Snapshot. All original gap, preimage, semantic-epoch, capacity, withdrawal and writer-lifetime rules still apply. Observation loss requires a new full snapshot after the loss cut. Time never repairs an incomplete prefix or a poisoned source.

The original delivery owner checks freshness at proposal, review creation, permissive review application, authorization and dispatch. It uses its own observed clock, not a timestamp supplied with an actor request. A valid renewed observation of unchanged state can reuse an otherwise-valid original permit without reserving twice. Changed policy values still pass through the original witness and policy checks. An expired observation cannot be substituted with the driver's historical snapshot.

Governed source replacement preserves the fixed maximum age and producer/consumer clock floors while starting the new generation without any eligible closure. It retains the existing cross-generation semantic floor, cancels undispatched attempts through the original ledger, and refunds only their reservations. Old writers affect only their retired source. There is no disable, widen or fallback method on the fresh profile.

A consumed capture and lease remain historical delivery evidence. Source expiry, withdrawal, poisoning or loss does not block restrictive reviews, cancellation, authentic endpoint-receipt acceptance, or reconciliation of already-dispatched obligations. It does not prove nonexecution or refund an unknown effect. This first increment enforces freshness at broker admission; endpoint-side expiry for delayed envelopes is not supplied by this increment.

## Verification boundary

The source adds eight capture-law test functions, four original actor/congress/dispatch integration functions, and a compile-fail construction example. Positive controls renew unchanged observations and reuse the original permit in both one-key and two-key profiles. Negative controls cover exact expiry, future timestamps, clock rollback, timestamp/marker laundering, unclosed tails, observation loss, poisoning, writer loss, arithmetic overflow and cross-generation time/policy retention. The recovery control accepts a real late execution receipt after source expiry and loss without changing its charged liability or historical capture.

These tests have not been compiled, executed, rustfmt-formatted or RCH-qualified in this environment. No Beads task or production gate is closed. No dependency or runtime is added, and historical execution receipts are not promoted. Source authenticity, truthful physical observation time, clock synchronization, unobserved external changes and production durability remain host contracts. Logical time comparisons do not establish a physical freshness guarantee without those contracts.
