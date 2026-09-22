# Portable learned-generation recovery integration

Plan 11.1–11.3 and 11.6; numerical L7 evidence through the original learned
replay owner. This integrates the previously prepared recovery increment with
the portable archive core that independently landed on main. It does not replace
that implementation, introduce a second encoding or change existing APIs.

## One published archive format

The existing `FALGA\0\0\x01` header, complete recipe binding, 352-byte fixed state
layout and `ArchiveLimits::state` remain unchanged. The original `binding`,
`state` and `wire` modules remain authoritative, as do `GenerationArchive::decode`,
`begin_replay` and `replay`. The unpublished alternative encoding from the source
bundle is intentionally not introduced.

`GenerationCheckpoint::archive_layout` exposes the same bounded count pass used
by `encode_archive`, before allocating the exported buffer. `ArchiveLayout`
reports total, recipe and state bytes and accepted positions. The archive's
`layout` accessor reports those same fields after the original decoder has
validated the complete framing and independently supplied recipe.

`ReplayableGeneration::decode_archive` delegates directly to the original
`GenerationArchive::decode`. The intended owner supplies the immutable recipe;
this read does not advance or reset it, including when it is held. No imported
weight, cache, sampler, status, score or work counter is installed in the replay
candidate. The original complete verifier is still required to expose a new run.

## Prepared regression coverage

Six integration tests cover all checkpoint cuts with fresh native construction,
comparison against the original sampled engine, exact and one-less resource
limits, every truncated prefix, every recipe-byte substitution at an empty cut,
plausible state mutations, spent telemetry, terminal stops and a later alarm.
The independent header and state offsets target the format already published on
main, not the source bundle's abandoned encoding.

One private test constructs seventeen valid alternate native recipes, including
same-ID unused parameters, signed-zero probe coefficients, future thresholds,
retention, lineage, prompt, sampling and budget changes. Every substitution must
refuse next to a permitted original control. Existing archive and replay tests
are retained unchanged; these new tests do not substitute for them.

## Validation and limits

The required targeted and full RCH invocations were retried during publication;
both stopped with exit 127 because `rch` is unavailable. Compilation, Rust tests,
doctests, rustfmt and Clippy are UNEXECUTED. Baseline blob identities, prepared
source hashes and scoped patch checks are not execution or qualification gates.
No bead, production restart or detector qualification is closed by this change.
The synthetic model and freshly fitted test codec establish no model-safety or
cross-hardware fidelity claim.

## File-to-process recovery

`GenerationCheckpoint::save_archive_new` validates and encodes before creating
anything, then reuses the original sampled-archive exclusive writer. Existing
files and symlinks are not overwritten; Unix creation starts mode 0600 subject
to umask. A write or file-sync failure can leave partial or complete bytes and
is not reported as success. This is not atomic directory publication or a
power-cut durability guarantee.

`ReplayableGeneration::read_archive_file` reuses the original bounded
regular-file reader and the same complete recipe decoder above.
`begin_replay_file` returns only the original bounded verification cursor;
`replay_file` exposes a new numerical owner only after complete verification.
File-shape/recipe errors remain distinct from recomputation failures through
the existing `ArchiveFileError::Format` and `ArchiveFileError::Replay` variants.
The cursor retains the read image; changing or removing the path later cannot
change its expected bytes. No reopening, resend or unguarded restoration occurs.

The shared byte-I/O helpers are restricted to the sampling subtree. Both archive
callers validate their complete native limits first. The original sampled-file
APIs, their error variants and their partial-I/O tests remain unchanged. Operators
must control the file and ancestors and provision immutable inputs; this is not
a hostile-path race sandbox, authenticated storage or anti-rollback protection.

Three additional behavioral tests cover exclusive creation, unchanged old files,
Unix permissions, invalid-export zero creation, bounded reads, malformed files,
symlink rejection, replay mismatch, and snapshot ownership after path removal.
A fresh-process fixture rebuilds the model, fitted codec and monitors, replays a
saved sampled prefix, continues it, and exports bytes for comparison with an
uninterrupted original run. The child fixture itself is not a passing capability
claim; it is only exercised by the parent test. All additional Rust remains
UNEXECUTED under the RCH limitation above.

These interchange APIs are separate from the concurrent Unix
`archive::file::FileGeneration` write-ahead owner. They neither interpret nor
bypass its interrupted markers, exact directory binding or external floors.
That owner's implementation and tests are preserved unchanged.
