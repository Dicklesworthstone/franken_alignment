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

## Operator command

The existing registered `verify` verb now has a standalone `rights-journal`
profile in xtask. It reads real files through the independent verifier:

```bash
cargo run --locked -p xtask -- verify rights-journal \
  --context expected.json --journal rights.farj --anchor retained-prefix.farj
```

A built `xtask` binary accepts the same arguments from any working directory.
This read operation does not discover a source checkout, inspect its tracker,
run the repository qualification gate, or open a live authority owner.
Relative paths are relative to the caller's working directory. Build and
qualification continue to follow the repository's separate RCH requirements.

All three flags are mandatory and may appear in any order. Duplicate flags,
unknown flags, missing values and extra positional arguments are errors.
Paths retain their native operating-system representation. Inputs must be
regular files; the command rejects symbolic-link inputs and detects identical
journal/anchor paths and, on Unix, hard-link aliases using opened file identity.
Two distinct files with equal bytes remain valid: the command cannot prove
that their custody was actually independent. Supply frozen retained inputs;
file metadata checks are not a concurrent-filesystem security boundary.

The context file is strict JSON of at most 4 KiB. Every field shown here is
required; duplicate or unknown fields, malformed identities and non-integer
numbers are rejected. This example selects v1 and matches the existing
one-revocation golden fixture's independent context:

```json
{
  "schema_version": "fa.rights_verification_context/1",
  "run": "0101010101010101010101010101010101010101010101010101010101010101",
  "policy": "0202020202020202020202020202020202020202020202020202020202020202",
  "total_units": 100,
  "limits": {
    "max_frame_bytes": 1048576,
    "max_records": 100000,
    "max_total_bytes": 134217728
  },
  "recovery": null
}
```

The two identities are exactly 32 bytes encoded as 64 hexadecimal characters.
`recovery: null` selects v1. A `recovery` object with exactly `records` and
`bytes` selects v2; its limits and reserve must equal the independently selected
native header configuration. No expected identity, budget, capacity or format
is inferred from the journal. The complete context is validated before the
journal and anchor are opened.

The command writes one deterministic JSON response to stdout and uses a nonzero
exit status on failure. Schema `fa.rights_verification_result/1` contains all fourteen accounting
fields, separate prefix/final heads and byte lengths, the expected context and
all ten existing assurance properties. Checked observations retain `Known`
status under this bounded native profile; unestablished properties retain
`Unknown`. Native journal coordinates are never promoted to global Tower
generation, control sequence or source frontiers. This standalone report is
not the complete Tower response or a signed control receipt.

A failed verification includes its original error kind, input, byte offset and
record sequence when available. It emits no successful partial accounting.
Input acquisition and syntax failures have distinct structured errors. Neither
success nor failure repairs, truncates, appends to, or retries an effect from
either input. Repeating the command performs a new observational verification.

The command and thirteen binary integration tests are implemented in source.
Independent source review and available rolling-nightly syntax/format checks
completed; compilation, Clippy, runtime tests and the qualified gate remain
pending in the environment described in `IMPLEMENTATION_STATUS.md`.

## What the profile establishes

The FARJ verifier has its own parser, CRC implementation and transition reducer.
That library imports neither fa-reference nor fa-journal, including through
source inclusion. The command's context parser uses xtask's existing shared
strict-JSON syntax layer, which contains no journal, decision, policy or
authority algorithms. Neither path reuses the producer's decision algorithms
to make a differential agree. Keeping this first executable family in xtask does
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
map and still-reserved effects. The two streams are read once. Every underlying
read attempt also spends a finite shared allowance, including interrupted
reads, errors and end-of-file probes. The default `verify` allowance is
`2 * max_total_bytes + 2 + 1024`: one-byte progress on both maximum-sized inputs,
two EOF probes, and a finite additional interruption allowance. Repeated
interruptions cannot spin forever without exhausting this work budget.

An integrating host can use `verify_with_read_budget` and a caller-owned
`ReadBudget` to choose a smaller allowance or conserve it across retries.
`spent_calls()` and `remaining_calls()` expose actual attempted reads; no
success, refusal or failed read refunds them. Exhaustion returns
`read_budget_exhausted` before the next underlying call. Invalid expected
context still fails before spending any read allowance. The budget does not
measure physical storage I/O, and a single caller-supplied Read may block;
these are byte, allocation, read-attempt and transition-work bounds, not an
elapsed-time guarantee. Errors identify
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
