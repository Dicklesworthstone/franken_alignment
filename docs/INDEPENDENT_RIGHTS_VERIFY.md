# Independent rights-journal verification

This is a bounded L6 verification profile for the actual FARJ v1/v2
rights-accounting journal. The public API lives in the already admitted,
dependency-free xtask package:

~~~rust
use xtask::independent_verify::{
    ExpectedContext, Limits, RecoveryReserve, verify,
};

// These values and the retained prefix come from independent caller custody,
// not from decoding the history that is about to be verified.
let expected = ExpectedContext {
    run: independently_selected_run,
    policy: independently_selected_policy,
    total_units: independently_selected_budget,
    limits: independently_selected_limits,
    recovery: independently_selected_recovery_reserve,
};
let report = verify(journal_reader, retained_prefix_reader, expected)?;
~~~

This API performs an observation. It never dispatches, repairs a journal,
changes rights, issues a permit or admits a live owner. Repeating a read of the
same inputs is valid. A failed read exports an error, not a successful partial
accounting report.

## What the profile establishes

The verifier has its own parser, CRC implementation and transition reducer.
Production code imports neither fa-reference nor fa-journal, including through
source inclusion. It does not reuse the producer's decision algorithms to
make a differential agree. Keeping this first executable family in xtask does
not admit another package or dependency; plan §6.2 explicitly allows logical
families to precede separate crates.

The caller must provide the complete independently selected run and policy
identities, total units, limits and optional recovery reserve. All-zero identities
are invalid. Supplying the reserve explicitly selects v2; omitting it selects
v1. v2 binds every encoded limit and reserve value exactly. v1 has no encoded
limits, so its limits remain an external verification constraint.

The second reader contains a mandatory, independently retained exact canonical
prefix. It must include the complete header and end at a complete validated
record boundary. The verifier compares bytes as it streams; a matching head or
CRC alone is insufficient. A valid suffix after the prefix is fully checked.
A truncated, malformed or illegal suffix prevents a successful report even if
the prefix itself matches.

The report separates the retained prefix's byte length and head from the final
image's length and head. It reports all accounting buckets, reservation counts,
the native rights epoch and incident counter. These native values are not
global Tower clocks.

### Independent accounting rules

The reducer checks the original law rather than trusting a supplied
"conjuncts passed" flag:

- Reserve IDs remain occupied permanently, including after settlement or abort.
  ID zero and total budget zero are valid; every actual reservation must have
  positive units and nonempty UTF-8 principal and target.
- Dispatch consumes a Reserved item once, in its original current epoch, and
  must match the entire original effect: principal, target, payload and units.
- MarkUnknown requires a dispatched effect. The units remain held.
- AbortBeforeDispatch refunds only Reserved work, including stale reservations
  that have not dispatched.
- Reconcile accepts only Dispatched or Unknown items. Its trusted historical
  boolean either charges spent units or refunds availability. The format does
  not authenticate that boolean's real-world truth.
- RevokeEpoch increases the epoch without releasing any held amount.
- ResetToCheckpoint requires every kept ID to exist, including terminal IDs,
  and a canonical strictly increasing set. It aborts only omitted Reserved work,
  preserves Dispatched and Unknown liabilities, increments the incident
  counter, and does not change the epoch.
- At every accepted transition, available + spent + reserved + dispatched +
  unknown equals the original total, with checked arithmetic.

v2 reserves an independently selected part of the lifetime byte and record
capacity for recovery. Only revoke, mark-unknown, abort and reconcile may enter
that tail. Reserve, dispatch and reset remain ordinary work. Every record
consumes the same cumulative counters; refunds and resets never replenish
journal capacity.

## What remains Unknown

Successful verification establishes consistency of the supplied byte image
under this bounded reference accounting law. It does **not** establish:

| Property | Reason |
|---|---|
| Authenticated producer or operator | FARJ uses CRC32, not a signature or MAC. |
| Independent anchor custody or freshness | The caller supplies the prefix; the verifier cannot establish its provenance. |
| Latest acknowledged or complete world history | A newer unseen suffix, or joint rollback of both inputs, is not detectable. |
| Actual external occurrence | A Dispatch record is a historical input, not proof of physical execution. |
| Truth of reconciliation | Reconcile carries a trusted boolean rather than authenticated outcome evidence. |
| Complete permit law | Issuer validity, evidence prerequisites, full closure, capture coverage and receipt signatures are absent from this format. |
| Future retention or global Tower state | The verifier owns no storage lease, global generation or control-sequence observation. |

An exact prefix defeats equal-CRC history substitution only for the bytes
actually retained. It does not turn CRC into cryptography or prove that the
retained bytes were independently obtained.

The broader control-receipt requirements in plan §17.9 and §25.1, FA-126, and
the Tower Verify backend remain open. This profile cannot be used as a signed
receipt or execution-admission result.

## Resource and failure boundaries

The profile is bounded to 128 MiB total configured bytes, one million configured
records, and frame bodies from 13 bytes through 1 MiB. Limits are checked before
frame allocation. Invalid booleans, UTF-8, event tags, lengths, sequence numbers,
CRC predecessors, trailing fields and unordered checkpoint sets reject.

The reducer retains an ID map for permanent duplicate prevention and an index
of active Reserved IDs. Exact effect bytes are released after dispatch or
abortion. Ordinary transitions use logarithmic map operations rather than
rescanning every historical reservation. Reset scans only active reservations:
removed entries are charged once; retained entries must appear explicitly in
the input keep set. This avoids repeated scans of all terminal history.

The whole image and whole prefix are never retained in memory. Work and storage
remain bounded by the selected record/byte profile, one bounded frame, the ID
map and still-reserved effects. The two streams are read once. A caller-supplied Read may block; these are byte,
allocation and transition-work bounds, not an elapsed-time guarantee. Errors identify
the input, byte offset and record sequence where available; no error repairs,
truncates, retries a dispatch or refunds an unknown liability.

## Native compatibility tests and qualification

The differential test harness compiles the exact production logical core and
journal core, then uses the original Journal constructors, append, and replay.
The source seams use include! at the original crate root: original production
definitions move without semantic edits, and existing root test blocks retain
their locations. Unix file-owner adapters remain in the original full crate.

The fixture producer is test-only. It is never part of the verifier's production
dependency closure. This validates canonical native Journal bytes and reference
Rights outcomes when executed; it does not exercise FileJournal locks, fsync,
crash recovery, endpoint execution or the full workspace.

Eight core unit tests and six native integration tests are authored. The native
producer defines 51 scenarios: 12 accepted and 39 rejected. The cases include
both formats, complete native lifecycles, conservation,
explicit recovery capacity, exact anchors and valid suffixes. Adversarial
histories recompute their CRCs so semantic negatives reach the intended reducer
boundary. The existing plumless/buckeroo collision supplies a causal control:
equal native heads, different exact history bytes.

**Qualification at introduction:** source review completed; Rust formatting,
compilation, Clippy and runtime tests remain pending because the execution
environment disconnected during implementation. No current-revision test pass,
complete gate pass, bead closure or production assurance is claimed.
