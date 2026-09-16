# Bounded exact witness refinement

Source addition: September 15, 2026. Plan sections 7.6–7.8 and 18.7; FA-060; FA-INV-021. This is reference-model functionality, not a production cache or dispatch integration.

`WitnessJudgment::begin_refinement` creates an allocation-free validation cursor over immutable borrowed judgment, snapshot and product-frontier inputs. `advance(RefinementBudget)` reports `StillValid`, `Invalidated` with the original dependency index, `NeedsRefinement`, or `Refused`. The eager `reuse_at` algorithm is unchanged and supplies the differential oracle.

The budget independently caps logical steps and value bytes compared. Work is charged before operations. A step is one basis check, witness dispatch, frontier check, point lookup, range advance, or comparison chunk; it is not a hardware-instruction or wall-time bound. Range iterators survive pauses. Byte comparisons can advance one byte at a time, so a small budget need not restart an entire large-value comparison. The `NeedsRefinement.minimum` value describes the NEXT atomic step, not the budget to finish the judgment.

Per-call and cumulative work remain available on budget exhaustion, incomplete frontiers, stale snapshots, and arithmetic refusal. Terminal observations are idempotent for the same borrowed inputs. No partial result can be converted to a permit. Immutably borrowing the whole input set prevents a paused cursor from resuming on a changed snapshot; the compile-fail example exercises that boundary.

Exact positive witnesses do not acquire a spurious negative-closure requirement. Absence and complete-range witnesses retain the original authenticated-prefix and exact closing-marker checks. A completed prefix is not a completed range: the cursor advances once beyond the last expected member to catch suffix phantoms. Domain, projection, semantic-epoch, revision and control-cut checks retain the eager validator's order.

The source includes nine unit tests: mixed witness mutation differentials under three budgets; zero/byte exhaustion; a last-byte change in an 8 KiB value; paused range suffix checks; incomplete/summary closure cost retention; exact maximum-key reads; stale/basis priorities; empty values/ranges; and checked-work overflow. A compile-fail doctest covers mutation while a session remains live.

## Verification and remaining boundary

Rust compilation, rustfmt, Clippy, unit tests and the doctest were NOT executed in the editing environment: Rust and RCH were unavailable. The original witness file was reconstructed and checked against its Git blob identity before the two-line module declaration was added. This source addition does not close FA-060, change Beads status, activate a production feature, or update any qualified test-count claim.

Still required: the repository's frozen-source RCH gate, independent review, cost/allocation measurements, the wider coarse-to-fine witness ladder, complete-tail invalidation indexing, and an explicitly qualified production reuse/publication integration. Caller-provided adapter-domain assertions remain assumptions; this module adds no authentication claim.
