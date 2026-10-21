# Resolve pending fleet receipts within their acknowledged capacity

FA-014; plan 16.4, 16.5 and 25.4. A native domain can complete its stop before
its coordinator acknowledges the result. On reopening, passive resolution must
retain the observed successor history. Previously that path could install the
observed head in memory, then discover during encoding that the completed image
could not fit. This quarantined the coordinator and stopped the fleet pass even
though no domain write or coordinator replacement had been attempted.

## Existing resolution, earlier capacity admission

For an Entered visit, the original encoder now sizes its Observed completion with
all existing visits and retained domain heads. Subtracting the retained payload
size yields the exact fixed overhead; the remaining pinned capacity supplies a
temporary aggregate head ceiling to the original canonical reader. No estimate,
placeholder receipt or new history is persisted. An impossible fixed overhead
refuses before domain I/O. An oversized acquired image refuses before decoding
or replaying domain events, under the existing complete-roster history limits.

A capacity refusal uses the existing session-only evidence refusal. It leaves
the durable Entered visit, retained heads, revision, canonical files and staging
untouched. The attempted read remains counted, its domain loses current-session
success, and the temporary ceiling is cleared. A fleet resume can then inspect
other members that fit; the refused member remains in the success denominator.
No stopped domain is commanded again and no uncertainty is converted to a refund.

An exact-capacity completion still requires complete original semantic replay,
matching stop identity, retained-prefix validation, unchanged coordinator custody
and the original durable acknowledgment. Existing Observed exact-cut refreshes
write nothing and do not reserve space for a nonexistent new completion. They
therefore continue to work at the coordinator byte and durable-visit ceilings.

Canonical-custody conflict takes precedence over an ordinary sizing/read refusal;
it cannot be cleared by this path. Genuine allocation, callback interruption or
storage failure still leaves the original uncertainty/quarantine semantics in
force. A successful size check is not a durable receipt. Explicit reopen and
native evidence resolution remain necessary after ambiguous persistence.

## Compatibility and verification boundary

No public API, wire format, pinned limit, effect rule or historical reducer is
changed. The new ceiling is session-only, is reset by the original completion or
next admission, and is absent after reopening. Complete roster/clock/visit checks,
original result rendering and all existing tests remain unchanged.

Five authored regression functions cover exact and one-byte-short completion,
fixed-overhead refusal before a missing domain, custody-conflict precedence,
a mixed two-member fleet with successful larger-budget controls, cancellation
and dropped reads, no-write refresh at exact capacity, independently anchored
successors, and all five acknowledgment write barriers. The fixtures execute
original stop operations, not caller-asserted receipts. Source-only validation
is not Rust execution: targeted tests and the unchanged xtask gate remain required
through RCH against the exact published revision. FA-014 remains open; filesystem
isolation and independent history custody remain assumptions.
