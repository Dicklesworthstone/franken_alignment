# Offline fleet registration from retained original history

FA-014, plan 16.4/16.5/16.9 and FI-A02: an offline stop operator must assemble
its fixed roster without first recovering live authority or reading a current
journal and calling it independent custody.

`FileShutdownDomain::from_history_anchor(member, anchor)` imports the exact
history captured by the original `FileOversight::history_anchor` and transported
with `FileHistoryAnchor::write_to/read_trusted`. The caller independently supplies
the member ID, absolute original directory and complete bootstrap profile.
Original canonical decoding checks profile/path identity, fields, bounds and
event count before the registration copies retained bytes. Transport framing by
itself is insufficient. Zero-event original histories are valid.

No current-domain file access, cleanup, inference, clock, role or effect occurs.
An offline or busy member can therefore remain in the fixed denominator. The
original plan constructor retains all roster/duplicate/head-space limits, and
its encoded representation is identical to registration through a live owner.
The existing shutdown coordinator consumes the resulting plan; it is not a substitute
for the native coordinator's registered-prefix admission and complete semantic
replay. An anchor may be transport-valid yet contain semantically invalid history;
only original replay can establish that the visit is valid.

The coordinator retains newer acknowledged prefixes after visits. Missing,
substituted or rolled-back source histories cannot be silently substituted for
them. Original domain and coordinator acknowledgments remain separate, and an
unknown or executed effect cannot be refunded by assembling a plan or resolving
a visit. Archive authenticity, filesystem isolation and protection against joint
rollback of all custody remain operator assumptions.

Six regression functions use original two-key journals and the real coordinator:
live/offline encoding equivalence, unreachable-member retention and later success,
reserved/sent/executed/expired liabilities, mismatched profile/path/revision,
framing-only archives, exact roster/byte boundaries, initial empty histories, and
newest-anchor rollback refusal with a genuine-successor control. Existing tests
are unchanged. Compilation and Rust tests are unexecuted in this environment;
remote-only RCH qualification of the exact revision remains required. No packet
closure, production activation, new runtime, dependency or journal format.
