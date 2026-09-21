# Runnable supervision after producer-history retention loss

The existing `create-checked`, `submit-checked`, `resume-checked` and Linux
`serve-*-checked` paths accept the explicit `fa.supervised-witnesses/5` profile.
It selects the native exact-snapshot fallback from
`PUBLICATION_SNAPSHOT_FALLBACK.md`, rather than adding another execution path.
It is intended for current-state predicates, not judgments that require every
intervening event. Producer authenticity and completeness remain host assumptions.

Start with a version-three coupled-producer profile, change `schema` to
`fa.supervised-witnesses/5`, and add the REQUIRED top-level field:

```json
"history": "exact_current_snapshot"
```

All other fields retain their existing meaning: one producer bundle path and
complete scope, feed identity/bootstrap position, producer-clock lease, bounded
lookup and comparison budgets, and a nonempty structured witness recipe. Raw
original/current capture files and a separate feed path are not accepted in this
schema. A `max_retries` field is also rejected: this is exact validation, not an
extension of the version-four pre-dispatch waiting profile. Existing schemas
reject the new history field rather than silently enabling or ignoring it.

## One explicit policy, from initialization through recovery

Creation calls the original four-profile atomic initializer. The fallback is
present in the FIRST canonical image, before actor admission or helper launch.
It is not enabled in response to an error, and there is no retry through a weaker
profile. The native recovery reserve remains available for stop and settlement.

Recovery pins validation, feed, clock/freshness and fallback selection before
cleanup or a recovery write. A strict feed profile cannot open this journal, and
this profile cannot open a strict one. Both preparation APIs also compare the
stored fallback selection before reading a producer, obtaining time, withdrawing
eligibility or binding original evidence. An embedding caller cannot silently
prepare the same request under a different history policy.

The existing coherent original binder reads one actual bundle, acknowledges the
available feed suffix and heartbeat, then binds the exact original requirements.
Binding after retained-history eviction does not create current eligibility or
an automatic/human key. The original helper review and independent approval must
still run. Each subsequent authorization, dispatch and first-publication boundary
performs a new acquisition through the existing supervised driver.

## Lost history is not declared recovered

The contiguous change position remains at its actual known prefix. Feed status
and ordinary feed freshness continue to report Incomplete when a producer window
has evicted needed records. The fallback instead requires a fresh producer-time
observation and a source-bound snapshot at the EXACT latest observed feed head.
It does not invent notifications, make a partial history complete, or increase a
comparison budget. An expired producer observation cannot become fresh by reread.

The original validator rechecks every captured value, absence, empty range and
range-membership dependency, plus any opaque lane present in the original input.
A newly present forbidden key still refuses after dispatch even if its notice
has been evicted. Unrelated changes have a positive publication path. The native
policy-source lease, full committee inputs, human key, stop control, actor
cancellation, target identity and deadline remain independent obligations.

Publication uses original endpoint receipts and accounting. Missing policy
evidence after dispatch can seal nonexecution; ambiguous installation or storage
failure does not fabricate that receipt. Already executed effects stay charged.
Exact retained retries and receipt-only recovery do not reacquire source files,
start helper processes or replay the old effect. A stored fallback selection does
not survive recovery as an old live lease, job or sendable envelope.

## Verification scope

Eight authored Rust tests exercise strict schema controls, atomic selection and
bidirectional recovery pins, preparation before I/O, original binding after real
producer-derived retention loss, the complete runnable helper/reviewer workflow,
post-dispatch negative-versus-unrelated changes, lease-expiry neighbors and the
independent native policy source. Historical feed incompleteness and charged
executed outcomes are checked separately from successful publication.

These are enforcement fixtures with synthetic helper verdicts, not evidence of
model accuracy or real-world capture completeness. They have not run in this
editing environment: RCH and the Rust toolchain are unavailable. The required
exact-revision verification is
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`.
No compilation, formatting, Clippy, runtime-test pass, production qualification,
source-authentication or FA-061/062 closure is claimed by this increment.

## Whole-input-only snapshot supervision

`fa.supervised-whole-input/2` selects the SAME native history policy and requires
`"history":"exact_current_snapshot"`. It uses the same producer, feed, scope,
clock and budget fields as the version-five structured profile, but requires
`"requests":[]`. Preparation must retain an actual original opaque input. Missing
opaque data or a structured-only image cannot substitute for that whole view.
There is no invented key, helper-supplied read subset or cached positive capture.
The older `fa.supervised-whole-input/1` remains strict and rejects the history field.

The comparison includes submitted bytes, complete input-profile identity/bytes,
model/tokenizer/policy epochs, ordered parts and omissions under the original
budget. This mode neither recaptures the original verdict nor treats a changed
whole view as equivalent because its prose explanation names fewer dependencies.
All other original lanes that happen to be present remain bound as well.

A currently identical opaque view can pass after intermediate changes have left
producer retention. This certifies only its present equality under the original
witness contract, NOT that every intervening event was harmless or recovered.
Future history-sensitive witnesses must not use this fallback. A changed final
view still refuses even when committee-source bytes and human approval are
unchanged. Native sealing, uncertain outcomes and receipt-based accounting retain
their existing meanings.

Six further authored regressions cover explicit whole-mode admission, complete
opaque-only original retention, missing opaque input before helper launch,
end-to-end publication plus source-free resume and exact retry, a six-case late
retention-loss matrix (quiet/bytes/model/layout/omission/missing), and a zero-budget
negative paired with the same-input permitting case. Together the two runnable
increments add fourteen tests. All remain UNEXECUTED pending the exact-revision
RCH verifier; no runtime pass or qualification follows from source checks.
