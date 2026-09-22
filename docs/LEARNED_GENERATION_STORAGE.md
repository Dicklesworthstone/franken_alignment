# Learned-generation archive transport and recovery

## Stream transport (source addition, not an execution qualification)

The stream APIs extend the existing portable V1 learned-generation archive and
consume the original token-recomputation verifier. They support plan 11.1, 11.2,
11.7 and the evidence retention/recovery boundary in section 16. They do not
change the archive's bytes, the numerical engine, the monitor roster or authority.

`GenerationCheckpoint::write_archive_to` preflights the complete recipe, state,
position and total-byte bounds before emitting any bytes. It uses the same
encoder as `encode_archive`, through a checked sink, then flushes the supplied
writer. A returned `ArchiveWrite` counts logical emitted bytes; it is not a
filesystem durability or authenticity claim. A failed writer can contain a
partial prefix and must not be published as a checkpoint.

`GenerationArchive::read_archive_from` admits the fixed header before consuming
its body, counts the independently supplied intended recipe, then compares that
recipe directly against the input stream. It retains only the bounded state
buffer rather than a second archive-sized copy of model/codebook parameters.
The input must end exactly after the declared state. Interrupted operations are
retried; WouldBlock and I/O failure cannot stand in for EOF. Deadlines and
nonblocking scheduling belong to the caller. Failed streams are partially
consumed and must be discarded, not resumed at an invented frame boundary.

Successful import still returns untrusted replay expectations. Call `replay` or
`begin_replay` on that archive to run the original learned generator and verify
all saved state before obtaining a reconstructed owner. The independently
constructed intended recipe is never advanced or reset during import. An empty
prefix still binds the entire recipe, including unused model parameters.

## Existing file APIs now consume bounded streams

`GenerationCheckpoint::save_archive_new` sizes the complete archive before any
file creation, then runs the original encoder through a buffered writer. The
shared file helpers preserve exclusive creation, Unix mode 0600 (subject to
umask), and a successful file sync before returning the encoded byte count.
A failed write or flush is not retried by dropping the buffered writer. Errors
can still leave partial or full files for inspection; these are not successful
saves. File synchronization does not certify durable directory publication.

`ReplayableGeneration::read_archive_file` retains the original regular-file and
file-size checks, then runs the streaming recipe comparison through a buffered
reader. This removes the whole-archive input allocation; only the admitted
state image is retained along with the fixed I/O buffer. Buffered reads can
prefetch bytes beyond the current logical comparison. Truncated files remain
format errors; actual I/O failures retain their operation and error kind.

`begin_replay_file` and `replay_file` still delegate to the original numerical
verifier. The imported expectations no longer depend on the pathname once
reading succeeds, so deleting it cannot redirect or substitute a later replay.
The operator must control the file and all its ancestors and publish immutable
bytes. These checks are not a hostile-path or concurrent-writer sandbox. The
separate Unix `FileGeneration` write-ahead owner and recovery floors are unchanged;
portable interchange does not interpret or bypass their interrupted markers.

## Authored boundary tests

`learned_generation_stream.rs` exercises every cut across prompt, sampled
continuation and terminal status; exact equality with the existing byte encoder;
short/interrupted transport operations; exact and one-less bounds; header-only
refusals; truncation; trailing data; a different sampler recipe; original write
and flush errors; a missing EOF certificate; tampered state; and repeated holds.
Reconstructed continuations are compared with the original generator, including
cache/logit words, sampled probability bits, sampler state and all numerical and
telemetry spend and ceilings.

`learned_generation_file_stream.rs` adds three real-file regression tests for
all continuation cuts, original-byte parity, exact bounds, private exclusive
creation, invalid-export zero creation, replay after path deletion, truncated
and trailing data, independent-recipe mismatch, unchanged original owners,
symlink refusal and replay that repeats the original monitor hold. The existing
portable archive tests, including the fresh-process continuation test, remain
unchanged.

Rust compilation, formatting, Clippy and these tests are UNEXECUTED in this
preparation environment: cargo, rustc, rustfmt and RCH are unavailable. No bead,
release gate, portability claim or production control profile is closed.
