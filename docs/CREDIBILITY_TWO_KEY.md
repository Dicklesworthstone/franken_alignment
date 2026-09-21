# Held-out credibility in the full-input, two-key path

FA-105 increment; plan 9.9 and 9.11; FI-A08/FI-A13/FI-A18. The original L8
promotion feeds L3 weights and the existing L4/L5 authority. No new rights ledger,
agent verb, runtime or dependency is introduced.

`OversightBroker::activate_credibility(request, expected_contracts)` now delegates
to the original held-out authority transition. It checks the complete immutable
committee contract, not just member names. The association between evaluation
helper generations and that contract remains an explicit trusted assertion;
matching integer IDs does not authenticate models or qualify their inputs.

The existing owned-round evaluation and optional joint-replay promotion lane is
exclusive with this offline lane. Neither can be enabled to bypass the other's
requirements. New activation/withdrawal invalidates old empirical approvals and
advances the original authority and endpoint fence. Historical retries do neither.
The broker never manufactures a human revocation; retained human statuses remain
history, and old keys fail their original sequence/epoch checks. A fresh human key
and the original automatic permit are both still mandatory in two-key profiles.

Complete input equality, policy witnesses, expiry, topology/source/identity gates,
endpoint fence acknowledgment, and receipt-only settlement are unchanged.
Withdrawal does not refund dispatched or unknown effects. Independent endpoint
messages may execute before their new fence arrives; this is not atomic remote
revocation. No current qualification, human key or new effect is implied by an
inspection or an exact historical retry.

Eight authored native tests exercise permitted two-key publication, old-key and
missing-fence rejection, exact-contract mismatch, unknown outcomes, historical
retries, input substitution, mutually exclusive promotion lanes and expiry.
The required RCH xtask invocation exited 127 (rch unavailable). cargo, rustc and
rustfmt are absent; compilation, formatting, Clippy and Rust tests are UNEXECUTED.
This is source implementation, not FA-105 closure or production qualification.
