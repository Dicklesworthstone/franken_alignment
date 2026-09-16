# Exact closing-marker identity for witness evidence

September 15, 2026. This fixes the reference-model boundary shared by witness capture, eager reuse, bounded refinement and change-index admission. It changes no general prefix-query semantics or production qualification.

`ProductFrontiers::satisfies` intentionally answers a prefix-coverage question. With a recorded marker `(projection=P, generation=7, final_sequence=2)`, a requirement through 0 or 1 with the same generation can be satisfied. The old witness `closed_marker` helper used that prefix predicate alone, so a snapshot asserting a different, shorter terminal with the same projection and generation could pass a whole-domain completeness check. In particular, a zero prefix did not establish an empty terminal.

`ProductFrontiers::closing_marker` now exposes the exact admitted marker as a read-only observation. The shared witness helper requires equality of the whole marker before checking authenticated prefix coverage. Truncated, lengthened, missing, or mismatched markers return `Incomplete`; a correctly declared new marker still produces `ClosingFrontier` invalidation relative to an older witness. Exact positive-value witnesses retain their original lack of a negative-closure requirement.

Five public-API integration tests cover preserved general prefix semantics; all three negative/range capture kinds with shorter, zero and longer asserted terminals; eager and budgeted reuse under same-generation terminal substitution; complete-tail admission and fallback; and a genuinely empty terminal as a positive control. No existing test was weakened or removed.

Rust, rustfmt, Clippy, tests and doctests were not executed because Rust and RCH are unavailable in the editing environment. Source changes and regression cases are not a qualification receipt. Caller-provided closing observations still carry no cryptographic origin authentication, and all Beads and production gates remain unchanged.
