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

## Authored boundary tests

`learned_generation_stream.rs` exercises every cut across prompt, sampled
continuation and terminal status; exact equality with the existing byte encoder;
short/interrupted transport operations; exact and one-less bounds; header-only
refusals; truncation; trailing data; a different sampler recipe; original write
and flush errors; and a missing EOF certificate. Reconstructed continuations are
compared with the original generator, including cache/logit words, sampled
probability bits, sampler state and all numerical/telemetry spend and ceilings.

Rust compilation, formatting, Clippy and these tests are UNEXECUTED in this
preparation environment: cargo, rustc, rustfmt and RCH are unavailable. No bead,
release gate, portability claim or production control profile is closed.
