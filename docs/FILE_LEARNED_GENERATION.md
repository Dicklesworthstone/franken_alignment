# File-owned learned generation

Plan 11.1, 11.2 and 11.6; the existing learned numerical replay baseline now has
an explicit Unix file owner. Its consumer is `FileGeneration` in
`sampling::replay::archive::file`. This is L7 numerical continuation, NOT recovery
of a production actor, effect journal, authorization, fresh evidence or detector
qualification. It adds no dependency, executor, model algorithm or weight update.

## Accepted output follows durable state

`FileGeneration::create(directory, original, limits)` accepts only an unadvanced
`ReplayableGeneration`, captures its original archive, and publishes it before
returning the file owner. The directory must be new. Creation cannot adopt a
held or already advanced run. Model, fitted-codec, probes, prompt, sampling and
telemetry budgets remain the original recipe, not writable configuration.

`advance(expected_revision, expected_position)` validates the predecessor before
performing I/O. It then replaces the canonical file with a small interrupted-step
marker and synchronizes it BEFORE invoking the original learned generator. A
quiet, accepted step is checkpointed with the existing archive encoder and
published with file-sync, rename and directory-sync. Only then is its original
`GenerationEvent` returned. Every successful step increments position and revision
once. These receipts describe numerical checkpoints, not external effects.

An alarm, budget exhaustion, computation/encoding failure or interrupted step
cannot recover the shorter quiet prefix: its canonical marker already forbids
continuation. A returned native held status is an observation, not a resumable
checkpoint. The marker deliberately does not reconstruct the held state or its
unreported partial work. Recovery of such a stream is unavailable; no retry or
reset method clears it. A new experiment is a separate operator decision and
still cannot acquire production rights or a new independent evidence origin.

The live owner closes before the first write, including across caught unwinding.
If an initial marker write fails before computation, the previous committed image
may still be valid. If final synchronization loses acknowledgment, the newer
complete image may be present. In either case that owner stays unavailable: only
exclusive reopening of actual canonical bytes determines what can be recovered.
Staged bytes alone are never a checkpoint and never trigger inference or a resend.

## Reopening and independent requirements

`FileGeneration::open(directory, &intended, limits, replay_budget, minimum)` takes
an independently constructed original recipe and explicit minimum revision and
position. It acquires the same cooperating lock, checks bounds, framing, directory
binding and floors, compares the complete intended recipe, and delegates to the
original `GenerationReplay`. Every prompt/sample and learned audit runs again;
exact token/sample/cache/logit/RNG/status/work comparisons must finish before an
owner is returned or leftover staging is cleaned. Saved work does not refill
original run budgets. Replay work is additional, and terminal stops stay terminal.

`generation()` exposes only the healthy owner's immutable original numerical
view. There is no mutable-inner-owner, unchecked-import, force-token, reset or
permit accessor. `last_commit()` is historical even when a newer checkpoint may
have reached disk. The caller must retain acknowledgment floors independently.

The new outer file profile uses `FALGF\0\0\x01`, revision, position, a committed
or interrupted tag, length-framed canonical directory bytes and the unchanged
original archive. Committed revision equals position; interrupted revision equals
position plus one and contains no archive. There are no serialized success scores,
effect balances or permits. Paths and outer overhead are independently bounded.
Private directories/files use modes 0700/0600, immediate symlinks and linked files
are refused, and the reader checks metadata identity and a hard read ceiling.

Cooperating Unix locks, operator-controlled directories/ancestors, atomic rename,
working synchronization and honest persistent storage remain assumptions. These
are not encryption, hostile-filesystem isolation, anti-fork authentication or
exactly-once output delivery. Equal-counter forks and stale independently retained
floors are not detected. The full recipe/state archive is sensitive and rewritten
per accepted step; no throughput, small-checkpoint or peak-memory claim follows.

## Validation status

Seven Rust tests and two compile-fail boundaries are authored. The tests use the
actual tiny decoder, fitted codec, original learned monitors and real temporary
files. They cover every prompt/sample/terminal cut, exact continuation and spent
budgets, exclusive ownership, stale requests, independent recipe/floor/state
mismatches before cleanup, all five storage barriers during both intent and
successor publication, a real monitor alarm, aggregate telemetry exhaustion,
strict framing and bounds. The near-identical quiet monitor accepts the token
that its alarm variant holds. Existing replay/archive assertions are unchanged.

The required RCH xtask command was attempted and exited 127 because `rch` is not
available. Cargo, rustc and rustfmt are absent too. Compilation, Rust tests,
doctests, formatting and Clippy are UNEXECUTED. Source hashes and non-compiling
screens are not verification. No packet or production qualification is closed.
