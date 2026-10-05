# Learned pending-step recovery and admission

Status: unqualified source, advancing FA-014 without closing its full packet.
This serves the existing effect-ledger and replay contracts in plan sections 8,
11 and 16. It changes no founding semantics, dependency or journal format.

## Ordinary capacity is distinct from terminal recovery space

A new learned inference intent now requires two ordinary event slots before it
can be persisted. The old physical-limit check counted the reserved terminal
Fence/Stop/StopProgress tail, although the original encoder correctly refused
using that tail for a numerical completion. With only one ordinary slot left,
that admitted an intent that the next completion could never append.

Completion also checks its one ordinary slot before replay or new numerical
work. An intervening transaction can still consume previously available space;
this check is not an escrow or a promise of eventual completion. The durable
intent remains a barrier when completion refuses. No older quiet prefix becomes
eligible, and the terminal recovery reserve is not enlarged or spent.

These checks cover event slots, not future witness bytes, physical disk space,
future restart attempts or hardware durability. Original canonical prefix byte
limits, independent recipe matching, numerical algorithms, authority fencing,
mandatory automatic/human keys and storage-failure poisoning remain unchanged.

## Verification and change record

Four new regression functions pair one-slot refusal with exactly-two-slot
original inference, both with and without a terminal reserve. They also exercise
intervening clock work, retained pending state, unchanged canonical bytes and
stale/poisoned owners. They use the existing original-model fixture and real
journal files; synthetic probe parameters are not detector-quality evidence.

The required targeted RCH command was attempted and failed before compilation
because `rch` is absent (exit 127). Rust compilation, test execution, rustfmt and
Clippy are UNEXECUTED. No historical receipt qualifies this change. Beads remain
unchanged because their required `br` CLI is unavailable; no gate is promoted.
