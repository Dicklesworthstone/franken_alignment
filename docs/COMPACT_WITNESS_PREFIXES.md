# Contiguous observation batches and witness replay

`ProductFrontiers::accept_contiguous(key, stage, first, last)` records the complete
inclusive interval under the existing caller-supplied observation contract. It is
not a declaration that only its endpoints were observed. An interval must touch
the known contiguous prefix; missing predecessors refuse without changing state.
Every other stage, projection, source epoch and closing marker remains independent.
A terminal closing marker still rejects extension and must be recorded separately.

For admitted intervals the result equals ascending calls to the original single
position operation, including already observed pending successors. Runtime work
depends on bounded pending entries, not the size of the numerical sequence span.
There is no new allocator, runtime, authentication primitive or authority type.
The ordinary out-of-order max-gap bound is unchanged.

The original publication witness replay now uses this primitive rather than
looping over every saved sequence. Reconstructing its one closed projection has
no pending entries, so prefix reconstruction uses constant work independent of
its terminal number. This commit preserves the legacy 4,096 wire-profile limit
and all existing packet bytes; larger-prefix packet support is separate.

Six native regressions cover all 128 small pending patterns and their interval
neighbors, terminal u64::MAX, missing predecessors, explicit closure, independent
stages/projections, empty domains and unchanged capacity/gap rules. These Rust
tests are authored but unexecuted. RCH/cargo/rustc are unavailable in the editing
environment. Exact-revision compilation, formatting, Clippy and runtime execution
remain required; no production activation or Bead closure is claimed.
