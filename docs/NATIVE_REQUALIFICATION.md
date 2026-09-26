# Native generation requalification

## Capability and invariant

A recovered native decoder is paused, while the original recovery fence makes
previous held-out qualification stale. The decoder admission boundary now admits
exactly the existing `ActivateHeldOut` and `WithdrawHeldOut` governance events
while paused or holding an unfinished generation. Without this composition,
reactivation was rejected before the original credibility reducer could run;
accepted evidence loss could also poison an otherwise healthy pending owner.

The same original activation still validates scope, actor/committee identities,
policy, exact control/epoch predecessor, evidence age, influence caps and any
configured held-out joint policy. Both transitions use the existing rights and
endpoint fences. They neither advance inference, import a cursor, reset a draw,
replenish a generation budget, change its prompt nor clear the decoder pause.
A new clock and explicit resume remain separate requirements. Historical
activation and withdrawal retries remain receipt-only and cannot reinstate or
withdraw a later qualification.

This is not blanket admission of credibility events: enabling protocols,
assessment and weight promotion retain their existing decoder admission rules.
No wire tag, journal encoding, model constructor, numerical reducer or authority
algorithm changes. The native service is the intended consumer: it needs explicit
fresh qualification to continue already-qualified interrupted work without
restarting generation. This serves the existing credibility, replay and effect
boundary contracts (plan sections 9.9 and 11.2), not a new permission path.

## Tests and source status — 2026-09-26

Four regression functions use the original numerical fixture, original completed
reviews to establish control history, independently labeled synthetic campaigns,
and real canonical files. They cover recovered activation versus uninterrupted
numerics; live pending activation/withdrawal and historical retries; stale,
foreign and repeated-generation rejection plus exclusion of unrelated inference;
and storage failure at staging and directory acknowledgment during withdrawal.
No test edits the control sequence or treats wall-clock ticks as campaign
positions. The single-member fixture tests composition, not ensemble accuracy,
independence, evaluator authentication or production isolation.

The required fresh RCH gate attempt failed BEFORE compilation because `rch` is
not installed (exit 127). The preparation environment has no complete checkout.
The four new tests, Rust compilation, rustfmt and Clippy are UNEXECUTED. No prior
execution receipt qualifies these changes, and no Beads item is closed. This
source is not a newly qualified production capability.
