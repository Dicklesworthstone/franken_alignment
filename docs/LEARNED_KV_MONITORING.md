# Learned KV evidence and exact probe refinement

## Implementation status and purpose

Source implementation, pending Rust execution. This increment connects the learned KV codec to the existing exact linear-probe engine. It implements the local numerical part of plan 10.3, 10.4, 10.9, 10.12 and 10.15: a learned representation may be useful, but its reconstruction MSE is not a certificate that a rare detector-relevant feature survived. The relevant founding synthesis is the alignment essay's activation observers and the introspection essay's economical sidecar. No plan, qualification gate, authority rule or founding premise is changed.

`probe::learned::CheckedLearnedKv` checks a concrete `LearnedKvImage` against a supplied original `ModelKvImage`. Complete source descriptors must agree. It visits actual source coordinates rather than accepting the learned codec's reported error. These source-relative envelopes and optional residuals occupy L2; the existing registered probe's resulting interval is L3. Caller-supplied source provenance remains an assumption. No new command, production policy, network protocol admission or permit conversion is introduced.

## Exact envelopes without optimistic floating error estimates

Finite binary32 bit patterns are mapped to an ordered integer space, retaining distinct signed zeros. For each layer, side, absolute token position and stored head, the checker records the maximum integer distance between an original coordinate and its actual learned reconstruction. At evaluation, saturating integer addition/subtraction around the reconstructed word yields finite binary32 endpoints enclosing every original coordinate in that group. These are ordered-word radii, not absolute-error norms, average errors or a claimed Lipschitz constant. A radius can be very conservative when a group crosses signs or exponent ranges.

`LinearProbe::evaluate_learned` evaluates those endpoints through the existing 576-bit exact signed-product accumulator. Weight signs select the correct interval endpoints. Zero-weight coordinates do not reconstruct values. Bias and threshold are included exactly. The outcomes remain `CertifiedAlarm`, `CertifiedQuiet`, `NeedsRefinement` and `AtThreshold`; equality never becomes quiet. The certificate is for the registered finite linear expression over the supplied original values, not detector sensitivity, model intent or permission to execute an effect.

An observation retains its actual checked context and a snapshot of the refinement view. Later materialization in another view cannot retroactively change the evidence used by an earlier score. Matching numeric IDs does not replace source checking. In particular, two source images with identical descriptors but different scalar bytes cause a fresh source-relative envelope and residual computation, not reuse of one image's MSE report.

## Exact escape blocks and explicit retention

Retention is chosen before checking: no residuals, all groups, or an explicit set of groups. The compact learned image remains unchanged. Retained groups contain sparse canonical `(channel, original_bits XOR reconstructed_bits)` records; unchanged coordinates have no record. Exact recovery preserves signed zeros and subnormal words. Residuals may be larger than the original input, so their storage is counted explicitly rather than hidden behind the latent size.

`encode_base()` emits the complete learned image, including its codebook/training metadata, plus the full envelope/availability manifest. `encode()` also emits every retained residual. `CheckedKvReport` distinguishes these actual lengths and both passes' source-coordinate and reconstruction-product work. No original source arrays are retained. Views may independently materialize selected exact groups; their derived arrays are separately counted.

`verify_residual` compares supplied bytes against the locally retained source-checked expected block. The resulting opaque block is tied to the exact checked context. `LearnedKvView::refine` requires that owner, an expected view revision, and sufficient byte/value/reconstruction budgets. It decodes the actual XOR records, checks indices, ordering, finite values and source-envelope consistency, then publishes the entire group. Corrupt bytes, foreign checked owners, duplicate promotion, stale revisions and exhausted budgets refuse without changing the view. A missing unretained residual remains missing.

This is local source-relative verification, not a remote authenticated receiver or a cryptographic proof. Exported blocks have no standalone unchecked import-to-evidence constructor. Cross-process arithmetic compatibility, authentication, ATP transport and independent source provenance require their own admitted profiles. No refined view can become a live capture, exact restart checkpoint or permit.

## Source acceptance scenarios

Ten public test functions in `tests/learned_probe.rs` cover erased rare-signal ambiguity followed by a real exact alarm; cheap quiet and threshold equality; a positive 2^-298 score surviving enormous cancelling products; signed-zero/subnormal word recovery; 96 generated mixed-sign exact-score enclosure cases; corrupt/foreign/stale/duplicate and exact/one-below refinement controls; actual full export sizing and source-check budgets; selective stored-head/side/position refinement; same-descriptor different-value source checking; and source lifetimes with independent view snapshots. One unit test covers ordered finite endpoints and nonfinite refusal. Two compile-fail examples reject source-frame and permit relabeling. Existing probe arithmetic and tests are unchanged except for the new module declaration.

An independent Python check executed 4,096 eight-coordinate cases with arbitrary finite binary32 original/reconstructed values and mixed-sign weights. Fraction arithmetic confirmed all 4,096 score enclosures, 32,768 coordinate enclosures and 32,768 XOR round trips. It also checked the exact 2^-298 cancellation margin and the false-quiet counterexample produced by discarding the radius around an erased 0.25 signal. This is evidence for the mathematical construction only, not Rust source execution or integration qualification.

The attempted command `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_probe` stopped before compilation: `rch: command not found`, exit 127. Rust tests, formatting, Clippy and the full revision-bound gate remain unexecuted. No Beads task or production gate is closed.

## Changelog for this increment

Added locally source-checked learned KV envelopes, optional sparse exact residual retention, canonical exports, revision-bound selective refinement, exact linear-probe evaluation and positive/negative source tests. No dependency, executor, original codec, inference engine, effect authority or existing mathematical accumulator was changed.
