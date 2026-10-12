# Recovering an uncertain rights-journal batch

## Original consumer and contract

`FileJournal::commit_batch` can fail after complete records have reached disk.
Its original contract is intentionally not multi-record disk atomicity: strict
reopen accepts a complete prefix beyond the independent recovery head, preserves
its original rights accounting, and rejects a torn record without truncation.
A caller recovering a frozen group must determine which records survived before
considering further accounting transitions.

`FileJournal::batch_anchor()` captures the acknowledged identity, head and encoded
length BEFORE attempting a group. Retain it independently alongside the exact
original ordered events. `inspect_batch(anchor, events, limits)` checks the
surviving portion using the SAME locked file. It reads/replays the original prefix,
requires the exact head at the claimed length, validates the entire proposed
group through the existing batch encoder/reducer, and compares each persisted
suffix byte. The expected prefix's rights, length and head must also equal the
original owner's acknowledged state. A clean but later unrelated suffix refuses;
no partial or conflicting record is silently discarded.

`BatchStatus` reports the observed head, matching record count, missing record
count and missing encoded bytes. These are journal facts, not external-effect
receipts. Inspection does not append, synchronize, truncate, repair, dispatch,
reconcile or refund. It never installs its temporary prefix ledger as live state.
The original lock remains held. Once reads begin, a mismatch, read failure or
unwind quarantines the owner until strict reopen; the earlier acknowledged head
remains available as a minimum, while rights reads and writes refuse.

## Independent inputs and limitations

The caller must independently retain the original batch, not infer it from a
successful prefix. Events that never reached disk cannot be authenticated from
that file. Changing only the unwritten suffix cannot be detected by prefix
comparison; no claim to recover an unstored intention is made. BatchAnchor fields
are recovery input data, not a capability or a cryptographic proof. Original
run/policy identity and anchor trust remain caller obligations. The CRC chain
still detects accidental corruption only; stored suffix comparison is byte exact.

This is a bounded synchronous replay, not a constant-time lookup or wall-clock
bound. It retains the original prefix ledger, bounded complete batch encodings,
private candidate ledgers, one replay frame and an 8 KiB comparison buffer. All
original lifetime and per-batch frame/record/byte limits still apply to the WHOLE
group, including records already present. It neither raises journal limits nor
uses partial replay as a way to spend rights twice. Filesystem, trusted-directory
and cooperative exclusive-lock assumptions are unchanged.

## Status and validation

2026-10-12: added exact-group inspection and eight actual-file regression functions.
Cases cover every complete-record interruption boundary, a genesis batch, identical
accounting under a different stored event order, wrong/misaligned/later anchors,
late semantic and whole-group capacity failures, same-length corruption, changed
extent, unrelated history and exact v2 reserve-tail inspection. They use the original codec, reducer and file owner;
injected loss of acknowledgment is not a hardware power-cut qualification.

Required verification was attempted with:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal
```

It stopped before compilation: `rch: command not found` (exit 127). Rust tests,
compilation, rustfmt and Clippy remain UNEXECUTED. No production gate, calibrated
claim or bead is closed. No original assertion, event format, authority type,
dependency or external-effect path is replaced.
