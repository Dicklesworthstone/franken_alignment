# fa-journal

A versioned rights-transition journal over `fa-reference::Rights`, with strict
streaming replay and a Unix local-file adapter. No registry dependencies are
added. The reference crate remains free of this persistence implementation.

## What it does

The journal records reserve, dispatch, unknown outcome, pre-dispatch abort,
trusted reconciliation, epoch revocation, and checkpoint-reset transitions. All
transitions are checked by the existing reference model. Replay reconstructs
accounting; it never performs an external effect. Dispatched and unknown effects
remain liabilities across restart and reset, not automatically refunded funds.

`Journal` is an in-memory codec/replay state. `file::FileJournal` owns an exclusive
file lock and synchronizes each checked record before publishing its new state
and returning an acknowledged `Head`. Creation uses an exclusive new-file open,
0600 permissions, and synchronization of both file and parent directory. Recovery
also synchronizes recovered records before exposing their accounting state.

```rust,no_run
use fa_journal::{Event, Identity, Limits};
use fa_journal::file::FileJournal;
use fa_reference::Effect;

let identity = Identity {
    run: [1; 32],       // supply the actual externally established run identifier
    policy: [2; 32],    // supply the actual externally established policy identifier
    total_units: 100,
};
let mut journal = FileJournal::create("rights.wal", identity, Limits::default())?;
let minimum = journal.commit(&Event::Reserve {
    id: 1,
    effect: Effect {
        principal: "alice".into(),
        resolved_target: "ledger://one".into(),
        payload: b"operation".to_vec(),
        units: 40,
    },
})?;
// Persist `minimum` in the caller's independent, trusted recovery context.
drop(journal);
let recovered = FileJournal::open("rights.wal", identity, minimum, Limits::default())?;
assert_eq!(recovered.rights()?.available(), 60);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Failure and recovery contract

Validation failures do not write or change accounting. Once storage I/O starts,
any error or unwind poisons the writer: further commits and accounting reads are
refused. The attempted record may have persisted even though no acknowledgement
was delivered. Keep the last acknowledged head, drop the writer, and reopen with
that head as the minimum required prefix. A complete suffix is replayed, not
silently dropped. Do not blindly reissue a failed operation or refund its units.

A partial record/header, bad checksum, wrong sequence/chain, unknown event,
noncanonical payload, wrong run/policy/budget, invalid transition, exhausted
resource limit, or history shorter than the retained minimum fails closed.
Recovery never creates a missing journal, truncates a torn tail, repairs a record,
replays an external send, or synthesizes a successful receipt. Damaged files are
left unchanged for explicit investigation. There is intentionally no automatic
"discard the tail and continue" path.

A clean prefix truncation cannot be detected without a sufficiently recent
independent recovery anchor. The genesis head alone does not establish freshness.
The returned head is not an authorization capability or cryptographic proof.

## Scope and limits

CRC-32 protects against accidental corruption only. It does not authenticate a
writer, prevent an attacker from recomputing a chain, or verify remote receipts.
Identifiers, recovery anchors, and `Reconcile` inputs must come from trusted
external context. The adapter requires a trusted directory and a local filesystem
that honors exclusive file locking and file/directory synchronization. Advisory
locks are not a security boundary against a noncooperating writer. Distributed
storage, power-fault qualification, authenticated journals, cross-system atomicity,
and exactly-once external effects remain outside this implementation.

Defaults: 1 MiB maximum frame body, 100,000 records, 128 MiB encoded stream.
Recovery streams one bounded frame at a time and does not clone the accumulated
ledger per record. Live append prepares a cloned reference state for transactional
failure handling; this is not yet a high-throughput production ledger or compactor.

V1 header: eight-byte `FARJ\0\0\0\x01` magic, 32-byte run ID, 32-byte policy ID,
64-bit total units, then CRC-32 of the preceding 80 bytes. A record is a 32-bit
body length, 64-bit sequence, 32-bit prior checksum, tagged event payload, and
CRC-32 over length plus body. All integers are little-endian. Strings/payloads
are 32-bit length-prefixed, booleans are exactly 0/1, and checkpoint IDs are
strictly ascending. Unknown versions/events and trailing event bytes are rejected.

## Validation

The crate includes public replay regressions, an independent Python/zlib-generated
wire fixture, checksummed malformed-payload tests, and Unix file tests with partial
write, failed-sync, panic, writer-lock, rollback, and uncertain-dispatch scenarios.
These injections exercise error handling; they are not real power-loss tests.

Required execution in this repository:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal
RCH_REQUIRE_REMOTE=1 rch exec -- cargo clippy --locked -p fa-journal --all-targets -- -D warnings
cargo fmt --all -- --check
```

During this implementation session the required test command stopped before
compilation because `rch` was absent (exit 127). `cargo`, `rustc`, and `rustfmt`
were also absent. Rust compilation, tests, formatting, and Clippy are unverified.
Independent Python checks matched 258 CRC vectors against zlib and rejected all
105 truncations and 840 single-bit mutations of the 105-byte golden stream;
these checks are not execution of the Rust implementation.
