# Deep foundation review · revision 0.2

The purpose of this review is to find mechanisms that change the architecture, not decorate it with library names. It distinguishes **observed source**, **proposed adaptation**, **missing bridge**, and **acceptance test**. Commit-pinned file URLs and read scopes are in [foundation_audit.json](../registry/foundation_audit.json). These are targeted deep dives, not claims to have compiled the donors or read every line of their very large repositories.

The two founding essays were read in full for revision 0.1 and remain the complete premise. There is no missing external post. The new round dives into ten ecosystem/build projects, including NetworkX and DSR, at fixed refs. Main-branch movement after each ref was captured is intentionally not folded into the review.

<a id="prototype-corrections"></a>
## Founding proposals: retain the ambition, sharpen the experiment

The external-helper essay proposes independent specialist observers, intermediate-activation observation, costly stop-the-line decisions, credibility-weighted congresses, rotation, salted commitment-style independence and resistance to persuasive self-defense. Its lasting contribution here is the separation between a powerful actor and the system deciding whether it may affect the world. The plan does not import the stronger philosophical claim that internal alignment is impossible; external control is useful without proving that claim.

The introspection essay proposes a sidecar latent state, grouped/layer-specific/unified compression, replayable thought graphs and self-coaching. Its illustrative KV extraction contains random placeholder tensors; some model attributes/shapes are architecture-specific, and MSE plus speculative compression ratios do not establish actual restart. Thus AuditTrace and RestartCheckpoint remain separate. Revision 0.2 adds exact receiver bases, residual escape paths, shape contracts and decision-sensitive measurements rather than translating the illustrative code literally.

The original source blob identities are retained in [sources.json](../registry/sources.json). No original compression experiment was reproduced in this work.

<a id="asupersync"></a>
## Asupersync: computation, evidence and transfer as one lifecycle

**Reviewed commit:** `9d5bc0b2d3767275a866feed5c3347bbf4a2de64`.

**Observed.** The source has native ATP object/stream/journal surfaces; stream states distinguish provisional, verified, final and invalidated conditions. The sparse journal distinguishes received, verified, written, repair-derived and committed chunks. The obligation machinery includes monotonicity reasoning; the Plan layer exposes compositional shapes; the decision crate belongs to an evidence/kernel/decision vocabulary. The syndrome module explicitly includes an offline-oracle path. These are more useful facts than a blanket assertion that “ATP is fast.”

**Architectural extraction.** First, an observation is a supervised computation with obligations, not an unowned asynchronous callback. Second, transmission can expose verified progressive evidence without conflating a prefix with a whole artifact. Third, the same receiver can reuse exact local blocks and request only the missing/refinement bytes. Fourth, monotone immutable facts may propagate independently, while live rights remain ordered. Fifth, observation compilation should target this runtime instead of introducing a second executor and hidden task lifecycle.

**Missing bridges.** Setters and enum names do not prove a legal state transition was enforced at admission. Stream sequence closure must be checked by the alignment adapter. Repair-derived bytes still require object authenticity verification. The shape Plan IR does not contain our policy/uncertainty/read-witness semantics. Wall-clock or host calls require recorded/Cx-controlled boundaries for lab replay. A tokio-compatible transfer actor is not admitted merely because other native ATP code exists.

**Most promising experiment.** Learn which verified refinement a specific receiver needs, then exploit ATP's resumable sparse transfer. Compare full objects, exact residuals and syndrome variants including all envelopes and feedback. The oracle variant must be labeled offline: it does not demonstrate that a live sender can know receiver entropy cheaply.

**Acceptance.** Lost middle chunk; duplicate frame; wrong encoding; stale base; decoder budget exhaustion; cancellation with outstanding host ownership; invalidation after partial review; crash after write but before committed receipt. No case may advance completeness or discard a live obligation incorrectly.

**Inspected source anchors:**

- [`ATP_DOD_CHECKLIST.md`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/ATP_DOD_CHECKLIST.md) — requested lines 1-240.
- [`src/net/atp/object/mod.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/net/atp/object/mod.rs) — requested lines 1-220.
- [`src/atp/mod.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/atp/mod.rs) — requested lines 1-300.
- [`src/atp/stream_object.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/atp/stream_object.rs) — requested lines 1-240.
- [`src/atp/slepian_wolf.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/atp/slepian_wolf.rs) — requested lines 1-230.
- [`src/atp/journal/mod.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/atp/journal/mod.rs) — requested lines 1-220.
- [`src/atp/journal/chunk_bitmap.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/atp/journal/chunk_bitmap.rs) — requested lines 1-210.
- [`src/plan/mod.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/plan/mod.rs) — requested lines 1-250.
- [`src/obligation/mod.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/obligation/mod.rs) — requested lines 1-200.
- [`src/obligation/calm.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/src/obligation/calm.rs) — requested lines 1-200.
- [`franken_decision/src/lib.rs`](https://github.com/Dicklesworthstone/asupersync/blob/9d5bc0b2d3767275a866feed5c3347bbf4a2de64/franken_decision/src/lib.rs) — requested lines 1-200.

<a id="frankensqlite"></a>
## FrankenSQLite: epistemic MVCC and final-effect validation

**Reviewed commit:** `cbb2852c75973811548ac89b0b16943b63453c6e`.

**Observed.** Witness refinement exposes multiple granularities and preserves conflict/uncertainty on resource exhaustion. Deterministic rebase distinguishes read/write and structural/epoch conditions. Cell-level MVCC has explicit boundaries against structural operations. History compression and Foata-related helpers are present, but the inspected history path still materializes full pages; the witness VOI field is not evidence of an already integrated optimal scheduler.

**Architectural extraction.** Give each judgment a snapshot and read witnesses. Reuse it only while those witnesses remain valid, independently of current permit authorization. Distinguish exact inputs, missing-key/range predicates, derived query supports and global semantic epochs. An opaque helper witnesses its entire submitted packet; it cannot shrink its own dependencies by explanation. This makes the cache correct before making it clever.

The second extraction is final-effect validation. A review of a path or high-level intent cannot authorize a different resolved object. Replay only deterministic pure subplans whose laws are registered; rerun opaque judgment when its input changed. A publication race leaves historical evidence, not a silently rebased permissive verdict.

**Missing bridges.** The database's connection-level SSI wiring cannot be replaced by lower-level transaction types that do not collect its dependency semantics. Source-level witness helpers still need query-specific soundness. A heuristic commutativity class sufficient for one donor use is not a proof for arbitrary alignment traces: disjoint writes with cross-reads can change results. This is a transfer-of-contract caution, not an allegation that the donor's live commit path admits that example.

**Acceptance.** Insert a new member into a reviewed-empty range, alter an input transform/model epoch, mutate an unrelated key, and exhaust refinement budget. Require exact agreement with always-recompute. Measure false invalidations and helper calls only after the stale-permission count is zero on the named campaign.

**Inspected source anchors:**

- [`crates/fsqlite-mvcc/src/lib.rs`](https://github.com/Dicklesworthstone/frankensqlite/blob/cbb2852c75973811548ac89b0b16943b63453c6e/crates/fsqlite-mvcc/src/lib.rs) — requested lines 1-250.
- [`crates/fsqlite-mvcc/src/witness_refinement.rs`](https://github.com/Dicklesworthstone/frankensqlite/blob/cbb2852c75973811548ac89b0b16943b63453c6e/crates/fsqlite-mvcc/src/witness_refinement.rs) — requested lines 1-260.
- [`crates/fsqlite-mvcc/src/deterministic_rebase.rs`](https://github.com/Dicklesworthstone/frankensqlite/blob/cbb2852c75973811548ac89b0b16943b63453c6e/crates/fsqlite-mvcc/src/deterministic_rebase.rs) — requested lines 1-220.
- [`crates/fsqlite-mvcc/src/history_compression.rs`](https://github.com/Dicklesworthstone/frankensqlite/blob/cbb2852c75973811548ac89b0b16943b63453c6e/crates/fsqlite-mvcc/src/history_compression.rs) — requested lines 1-220 and 330-540.
- [`crates/fsqlite-mvcc/src/cell_mvcc_boundary.rs`](https://github.com/Dicklesworthstone/frankensqlite/blob/cbb2852c75973811548ac89b0b16943b63453c6e/crates/fsqlite-mvcc/src/cell_mvcc_boundary.rs) — requested lines 1-220.

<a id="frankenfs"></a>
## FrankenFS: shared bytes, publication frontiers and repair economics

**Reviewed commit:** `260833046b1e7bc01a51fb8aa9e8f2d96118a8a2`.

**Observed.** MVCC has structural sharing and explicit merge classification; plain writes can downgrade an optimistic merge claim. Compression distinguishes shared/full/identical representations and codec forms. Safe aligned-buffer construction is possible using owned storage rather than unsafe allocation tricks. Repair has local/global coding surfaces and a best-effort owner lease. The inspected manifest contains mandatory compression and synchronization dependencies even when default features are empty.

**Architectural extraction.** Model capture, visibility and durable evidence as separate frontiers. Share identical state bytes across counterfactual branches while retaining distinct event identities and independent rights. Bound delta-chain depth and base retention. Price local repair separately from global failure-domain durability, and avoid paying for stacked coding at every layer without a reason.

The aligned-storage lesson is especially important for the user's performance constraint: memory safety does not force pointer-chasing layouts or copies at every abstraction. A safe owner can overallocate bounded slack, expose a checked aligned slice, and retain the original allocation. Whether that wins for capture/codec workloads is a benchmark, not a reason to introduce unsafe.

**Missing bridges.** A repair ownership file with TTL and reread logic is not the fence for permission minting, destructive GC or cross-process authority. A filesystem snapshot does not rewind a network request. A repair-derived block is not trusted until verified. Native zstd paths cannot be imported under the pure-Rust rule just because their containing crate is owned.

**Acceptance.** Identical payload/different event, abandoned branch with retained base, deletion while replay is pinned, duplicate maintenance ownership, corrupt local parity, insufficient global redundancy and process death during publication. Preserve authority and historical truth, even when the optimized representation falls back to a full object.

**Inspected source anchors:**

- [`crates/ffs-mvcc/src/lib.rs`](https://github.com/Dicklesworthstone/frankenfs/blob/260833046b1e7bc01a51fb8aa9e8f2d96118a8a2/crates/ffs-mvcc/src/lib.rs) — requested lines 1-230.
- [`crates/ffs-repair/src/ownership.rs`](https://github.com/Dicklesworthstone/frankenfs/blob/260833046b1e7bc01a51fb8aa9e8f2d96118a8a2/crates/ffs-repair/src/ownership.rs) — requested lines 1-240.
- [`crates/ffs-repair/src/lrc.rs`](https://github.com/Dicklesworthstone/frankenfs/blob/260833046b1e7bc01a51fb8aa9e8f2d96118a8a2/crates/ffs-repair/src/lrc.rs) — requested lines 1-220.
- [`crates/ffs-block/src/lib.rs`](https://github.com/Dicklesworthstone/frankenfs/blob/260833046b1e7bc01a51fb8aa9e8f2d96118a8a2/crates/ffs-block/src/lib.rs) — requested lines 1-230.
- [`crates/ffs-mvcc/src/compression.rs`](https://github.com/Dicklesworthstone/frankenfs/blob/260833046b1e7bc01a51fb8aa9e8f2d96118a8a2/crates/ffs-mvcc/src/compression.rs) — requested lines 1-230.
- [`crates/ffs-mvcc/Cargo.toml`](https://github.com/Dicklesworthstone/frankenfs/blob/260833046b1e7bc01a51fb8aa9e8f2d96118a8a2/crates/ffs-mvcc/Cargo.toml) — full manifest.

<a id="frankensearch"></a>
## FrankenSearch: generational evidence discovery, not approximate authority

**Reviewed commit:** `b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb`.

**Observed.** Generation types separate producer, input, embedding space, storage and artifact identities. Two-tier fusion/refinement code contains RRF/MMR, rank comparison and decision accounting. Filter code can intentionally admit items whose relevance metadata is absent. Workspace/reranker manifests show that selecting a native model does not by itself remove the entire foreign dependency graph. The workspace names the owned Torch package explicitly; an unrelated similarly named crate must not be substituted.

**Architectural extraction.** Treat an incident/strategy search result as a candidate observation with a precise producer generation. Reuse search's identity discipline for taps, latents, decoders and helper input transforms. Carry initial/refined status, real budget and staleness into the evidence planner. Relevance ranking can prioritize scarce investigation resources; it never replaces complete enumeration for an authorization predicate.

**Missing bridges.** Deny-on-unknown access control must run before candidates are disclosed or expanded. Ordinary relevance filters are not that boundary. Rank agreement and a stable top-k do not mean the result is correct or exhaustive. A stored vector's dimension does not identify its model space. Reindexing/publishing a new producer generation must invalidate or explicitly retain matching dependent evidence views.

**Acceptance.** Cross-tenant metadata missing, model swap with identical dimensions, truncated candidate population, a late conflicting exact object, stale tail and offline model loading. Discovery may remain useful under degradation, but ProofComplete must fail or fall back to the exact authorized domain.

**Inspected source anchors:**

- [`crates/frankensearch-core/src/generation.rs`](https://github.com/Dicklesworthstone/frankensearch/blob/b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb/crates/frankensearch-core/src/generation.rs) — requested lines 1-230.
- [`crates/frankensearch-core/src/filter.rs`](https://github.com/Dicklesworthstone/frankensearch/blob/b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb/crates/frankensearch-core/src/filter.rs) — requested lines 1-250.
- [`crates/frankensearch-core/src/decision_plane.rs`](https://github.com/Dicklesworthstone/frankensearch/blob/b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb/crates/frankensearch-core/src/decision_plane.rs) — requested lines 1-220.
- [`Cargo.toml`](https://github.com/Dicklesworthstone/frankensearch/blob/b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb/Cargo.toml) — requested lines 1-180.
- [`crates/frankensearch-rerank/Cargo.toml`](https://github.com/Dicklesworthstone/frankensearch/blob/b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb/crates/frankensearch-rerank/Cargo.toml) — full manifest.
- [`crates/frankensearch-fusion/src/lib.rs`](https://github.com/Dicklesworthstone/frankensearch/blob/b2f7cc07170de1e5b1d20b82a0d56e428a5bbebb/crates/frankensearch-fusion/src/lib.rs) — requested lines 1-240.

<a id="franken_markdown"></a>
## FrankenMarkdown: commit the evidence view, not merely the source file

**Reviewed commit:** `2698ad7d7562468e9c1728be08ce1ea1e9023629`.

**Observed.** The verify surface includes a context-bundle format with length-prefixed commitments to source/path/transform/text, plus proof routines. Source spans and diagnostic abstractions are explicit; some nested spans can be coarse. The bounded diff engine caps expensive LCS work and has a fallback. The library core can render from supplied bytes without performing IO.

**Architectural extraction.** A helper review is reproducible only if we know the exact transformed bytes it received. Build an EvidenceViewManifest binding originals, scope filtering, redaction, ordering, truncation, tokenizer/window transform and final input. Bind independent commitments to that view. A beautifully rendered report is then a projection of verifiable evidence, not a substitute for it.

A useful operational consequence is disagreement localization. When two reviewers disagree, first compare view identities and transformations; do not assume they evaluated the same evidence because they were pointed at the same document URL. Textual diffs can help a human inspect the difference while the manifest remains the exact identity source.

**Missing bridges.** The presence of Merkle routines does not mean every context-bundle version embeds all inclusion proofs. Do not claim exact source attribution from coarse spans. Rendering must use the IO-free profile, escape attacker content and refuse remote active assets. A semantic-preservation proof cannot be inferred from a small text diff.

**Acceptance.** Truncation removes the decisive clause, redaction differs by purpose, asset URL tries an external fetch, Unicode/window transformations differ and a renderer upgrade changes submitted input. Make the difference visible and invalidate only the claims whose contract was affected.

**Inspected source anchors:**

- [`src/verify.rs`](https://github.com/Dicklesworthstone/franken_markdown/blob/2698ad7d7562468e9c1728be08ce1ea1e9023629/src/verify.rs) — requested lines 1-260.
- [`src/span.rs`](https://github.com/Dicklesworthstone/franken_markdown/blob/2698ad7d7562468e9c1728be08ce1ea1e9023629/src/span.rs) — requested lines 1-260.
- [`src/diff.rs`](https://github.com/Dicklesworthstone/franken_markdown/blob/2698ad7d7562468e9c1728be08ce1ea1e9023629/src/diff.rs) — requested lines 1-235.

<a id="frankengraphdb"></a>
## FrankenGraphDB: canonical state, derived graphs and incremental truth maintenance

**Reviewed commit:** `117dce95cf5f71eac22f95831a8073cf0506105b`.

**Observed in the plan.** The comprehensive specification makes all canonical control commands—not merely user transactions—part of recovery. It separates role-bearing snapshots, schema/policy epochs, escrow custody, typed strong/conditional/weak references, and logical/cipher/encoding/placement identities. Its Z-set algebra models retractions and registered derivatives, and its verification doctrine distinguishes exact invariants from statistics and operation-cost targets.

**Architectural extraction.** An alignment system should not have one database for permissions and a different mutable “truth” for investigations. Use one authority nucleus and reproducible graph projections with exact frontiers. The graph can maintain which findings remain supported, which permissions depend on changed facts and which paths connect individually innocuous actions into a concerning workflow. It can also retain different experimental branches without duplicating their immutable bytes.

Support and deletion are central, not edge cases. A finding with two independent supporting paths should survive deletion of one, but a negative conclusion cannot become true because a retraction was lost. Model/codec/policy changes also affect standing query meaning and must enter the update algebra or force a rebuild.

**Missing bridges.** This is design reuse, not certification that all planned GraphDB functionality is already live. Current graph view adapters may materialize strings or ordinals; zero-copy requires matching physical/API semantics. Branch and encryption/retention semantics require their own tests. Adding GraphDB alongside fsqlite does not justify two independent authoritative permission ledgers.

**Acceptance.** Compare incremental and batch results after every closed mutation frontier, including deletions, recursion boundaries and schema changes. Restore with unknown effects and an old revocation floor. Attempt to reclaim a codec base still named by a replay lease. Demand typed unavailable outcomes rather than invented historical completeness.

**Inspected source anchors:**

- [`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENGRAPHDB.md`](https://github.com/Dicklesworthstone/frankengraphdb/blob/117dce95cf5f71eac22f95831a8073cf0506105b/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENGRAPHDB.md) — selected contiguous sections: constitution/foundation/data-model/authority/retention plus incremental Z-set and verification/convergence sections; not all 3867 lines.

<a id="franken_networkx"></a>
## FrankenNetworkX: dominators, cuts and typed projection certificates

**Reviewed commit:** `823cdd0268e05dce40a1a3d0647076b3136cd568`.

**Observed.** The algorithm source exposes immediate dominators, dominance frontiers and minimum-cut computation with deterministic ordering and witness metadata. The inspected cut implementation uses dense residual state and Edmonds–Karp. The generic graph view uses string-oriented identity and usize neighbor rows. Runtime tie policies and complexity witnesses are explicit, but the existence of a witness struct is not a proof that the reported bound is tight.

**Architectural extraction.** Use a complete overapproximating authority graph to ask whether every modeled path to an effect crosses an enforcement boundary. Dominators identify a single chokepoint; cuts handle sets of gates. A small independent checker removes the proposed cut and tests reachability, so it need not trust the optimized solver. SCCs and reachability can also help inspect delegation cycles and cross-agent composition under their declared projection semantics.

The larger opportunity is placement: choose which boundaries to instrument deeply and where cheap exact telemetry covers many paths. This is a constrained optimization/reliability experiment, not a theorem that a small number of monitors can read arbitrary intentions. Cohort graphs can expose common-failure concentrations, while incident graphs preserve uncertain evidence separately from exact authority edges.

**Missing bridges.** Current dense residual storage must not be carried into a sparse million-node deployment. Budget projection and solver working sets. Preserve multiedges, directions and capacities explicitly; a generic default capacity is not an authority cost. A true cut in an incomplete graph gives no guarantee about omitted operating-system channels.

**Acceptance.** Exact small graph differentials, forbidden bypass insertion, stale topology certificate, parallel edges, omitted credential paths, an intentionally false claimed cut, and memory exhaustion before allocation. Benchmark sparse implementations against this independently verified semantics before claiming scalable coverage.

**Inspected source anchors:**

- [`crates/fnx-algorithms/src/lib.rs`](https://github.com/Dicklesworthstone/franken_networkx/blob/823cdd0268e05dce40a1a3d0647076b3136cd568/crates/fnx-algorithms/src/lib.rs) — opening declarations/GraphView and function bodies immediate_dominators, dominance_frontiers, minimum_cut, compute_minimum_cut_edmonds_karp; not all 85980 lines.
- [`crates/fnx-runtime/src/lib.rs`](https://github.com/Dicklesworthstone/franken_networkx/blob/823cdd0268e05dce40a1a3d0647076b3136cd568/crates/fnx-runtime/src/lib.rs) — requested lines 1-230.

<a id="franken_numpy"></a>
## FrankenNumPy: shape calculus and bounded numerical evidence

**Reviewed commit:** `bb4cc2aabac8239a3ed00057b257e627847b8d1a`.

**Observed.** The ndarray foundation uses checked shape/stride/broadcast semantics and explicit failures. Native linear-algebra declarations include factorization/solve infrastructure and blocked/TSQR-related surfaces. This review did not audit every TSQR implementation body or reproduce performance. The linalg manifest includes mandatory small dependencies and default optional-parallel activation. Its Python facade's broad reachability cannot be equated with a fully native implementation of every function.

**Architectural extraction.** Treat the tensor layout as part of the capture identity. Validate dimensions, strides, aliasing permissions and byte counts before receiving untrusted state. Use native linear algebra for whitening, sketches, codec baselines and conditioning diagnostics. Prefer algorithms that avoid materializing the full layer×token activation universe merely to compress it.

The most valuable numerical proof is modest and exact: a known reconstruction norm bound and a linear probe's dual norm bound can certify preservation of that probe's sign. This can permit progressive transfer to stop before full reconstruction for a particular question. It says nothing about an unseen probe or global model alignment.

**Missing bridges.** Remove/disable a second scheduler and review every mandatory helper through the dependency policy. Do not import PyO3 or numpy fallback. Exact error envelopes require conservative floating-point treatment; average measured MSE is not a worst-case bound. Blocked factorization can be attractive without already being qualified for this workload.

**Acceptance.** Overflowed shapes, negative/aliased strides, nonfinite inputs, ill-conditioned solves, reproducibility under the selected numeric profile, sketch degeneracy and reconstruction-bound violations. Compare simpler projections as well as sophisticated methods at matched actual compute/memory cost.

**Inspected source anchors:**

- [`crates/fnp-ndarray/src/lib.rs`](https://github.com/Dicklesworthstone/franken_numpy/blob/bb4cc2aabac8239a3ed00057b257e627847b8d1a/crates/fnp-ndarray/src/lib.rs) — requested lines 1-215.
- [`crates/fnp-linalg/src/lib.rs`](https://github.com/Dicklesworthstone/franken_numpy/blob/bb4cc2aabac8239a3ed00057b257e627847b8d1a/crates/fnp-linalg/src/lib.rs) — requested lines 1-240; TSQR names observed, full kernel body not audited.
- [`crates/fnp-linalg/Cargo.toml`](https://github.com/Dicklesworthstone/franken_numpy/blob/bb4cc2aabac8239a3ed00057b257e627847b8d1a/crates/fnp-linalg/Cargo.toml) — full manifest.

<a id="frankentorch"></a>
## FrankenTorch: deterministic derivatives for monitoring and compression

**Reviewed commit:** `d7514b1254a369fc9bf720aaf03dce8c30f7109b`.

**Observed.** The reviewed autograd source declares safe Rust, explicit node types, backward options and deterministic gradient-execution machinery. Backward method bodies were inspected; searches did not establish a public JVP or saved-version API under the searched spellings. Those features therefore remain required bridges where needed. The earlier runtime-ledger review found intentional middle-entry eviction under a memory cap; that diagnostic ledger is not a complete alignment audit history.

**Architectural extraction.** Train small native monitors/codecs and use derivatives to estimate which activation coordinates most affect a registered probe. Couple that estimate to progressive refinement or loss weighting. Record gradient/numeric/model identities so results are meaningful after a kernel or model upgrade. Sharing this substrate avoids a separate Python/libtorch stack inside the control system.

Local derivatives are advisory for nonlinear probes unless a bound over the entire reconstruction region is available. They cannot certify that an edited hidden state remains on a meaningful manifold or that a restart is faithful. For experimental interventions, require exact tap ABI, shapes, model/cache layout and a registered continuation evaluator.

**Missing bridges.** A tensor/autograd facade is not an instrumented full transformer host. Capture and restart need actual token/cache/position/RNG state and host parity tests. A new gradient product must be implemented and checked rather than cited from an absent API. Bounded runtime logs feed, but do not replace, the separate durable evidence plane.

**Acceptance.** Backward determinism in its numeric profile, wrong tap layout, alias mutation, omitted restart component, unknown host kernel nondeterminism, and intervention outcome differences. Keep these tests separate from claims about trained detector quality.

**Inspected source anchors:**

- [`crates/ft-autograd/src/lib.rs`](https://github.com/Dicklesworthstone/frankentorch/blob/d7514b1254a369fc9bf720aaf03dce8c30f7109b/crates/ft-autograd/src/lib.rs) — opening types and backward method bodies; not all 38862 lines; searches did not establish a public jvp/version_counter API.

<a id="doodlestein_self_releaser"></a>
## DSR: locally executed proof closure and exact release publication

**Reviewed commit:** `f3d047d3ee09d17f993782436726beaf2925d4c9`.

**Observed.** Quality gates distinguish missing configuration, dry-run planning and actual execution, retain command/log/source information and have required checks that survive ordinary skip flags. Build/release code has strict source/tag/target/asset contracts and draft-first publication. The quality registry and build/release per-tool config are different files. Ordinary release checksum verification can be a limited spot-check, whereas strict signed publication has stronger inventory verification.

**Architectural extraction.** Treat a release as another evidence-bound effect. First freeze clean source/dependency roots and a resolved nightly, then execute every required local lane, form an exact artifact manifest, independently verify it, and only then explicitly publish. Build logs and receipts live outside the immutable source tree. No hosted CI completion is needed; local act/native-host execution is simply a transport for the same checks.

**Missing bridges.** A dry run cannot generate a passed receipt. A hash of the porcelain dirty-path listing alone does not commit changed file contents; enforce clean immutable snapshots or full content fingerprints. Build resume and release resume are not interchangeable capabilities. Warning-only checks after publication cannot satisfy a required prepublication smoke test. Re-drafting after ambiguous publication is incident containment, not proof of zero exposure.

FrankenAlignment will not commit fake signing keys, hostnames, rule IDs or a release-enabled configuration for a product with no production binary. The included quality fragment is actionable; the production build/release profile remains explicitly pending accepted targets, host paths and operator key material. This avoids turning a prospective plan into a false release claim.

**Acceptance.** Missing required check, moving source, mixed-nightly builders, one failed target, stale resume, ambiguous draft creation, mismatched tag/asset digest, incomplete downloaded verification and publication uncertainty. A release receipt states exactly which property was tested on which host.

**Inspected source anchors:**

- [`README.md`](https://github.com/Dicklesworthstone/doodlestein_self_releaser/blob/f3d047d3ee09d17f993782436726beaf2925d4c9/README.md) — requested lines 1-760.
- [`dsr`](https://github.com/Dicklesworthstone/doodlestein_self_releaser/blob/f3d047d3ee09d17f993782436726beaf2925d4c9/dsr) — large script response: inspected build/release/verification control paths; not a line-by-line audit of all commands.
- [`src/quality_gates.sh`](https://github.com/Dicklesworthstone/doodlestein_self_releaser/blob/f3d047d3ee09d17f993782436726beaf2925d4c9/src/quality_gates.sh) — requested lines 1-215.

## Synthesis: where the ideas reinforce one another

Judgment witnesses avoid repeating work; exact receiver bases avoid repeating transfer; incremental graph supports avoid repeating global analysis; structural sharing avoids copying experiments; view commitments make the remaining work reproducible. Authority cuts identify where the whole system must actually mediate effects. The control nucleus orders only the nonmonotone decisions, while fact movement and physical optimizations can exploit Asupersync's concurrency. DSR then requires the implementation's release to satisfy an analogous evidence discipline.

These mechanisms share one set of identities, scopes, frontiers and assumptions. They are not a promise of multiplicative speedups or immunity to a smarter adversary. Their value is that a failure in one experimental mechanism need not invalidate exact authority enforcement, and an improvement can be adopted without rewriting the meaning of the evidence.


## 2026-09-07 founding-source identity revalidation

The five `F-*` rows in `registry/sources.json` were revalidated at their recorded commits. GitHub returned the two alignment files at the pinned commit; the local introspection repository supplied its three pinned Git objects. For each file, the resolved blob ID and a fresh operator `git hash-object --stdin` calculation matched the recorded blob ID. Root independently repeated all five byte/hash comparisons. [Exact commands](../artifacts/execution/2026-09-07-founding-source-commands.txt) and [results](../artifacts/execution/2026-09-07-founding-source-results.tsv) retain that bounded evidence. This verifies immutable identities and preserves the documented earlier read scopes and complete two-repository premise; fetching and hashing are not a new full-text reading, donor audit, build or benchmark. No source pin was moved to latest main.

## FA-001 action/permit reference profile · 2026-09-08

`crates/fa-reference/src/action.rs` extends the existing reference `Rights` ledger without adding a dependency. Scope, resolved target, frozen action and elapsed tick model L0 identity data; reference authority, permit, lifecycle state and inspection model L4 ordered rights. Trusted outcome inputs model L1 observations about effects, not verified broker receipts. None is a new production agent-facing command or response.

The supported profile binds nonzero logical identities, one exact scope, an already resolved existing resource with an explicit resource version, adapter contract version and generation, bounded payload bytes, ordered declared logical read witnesses, current policy epoch, observed elapsed deadline and integer budget units. Equality compares the complete structural value. There is no wire canonicalization or cryptographic digest. Witness declarations must match the supplied logical judgment before reservation and still validate against the supplied snapshot at dispatch. An explicitly empty declaration models no logical dependencies; it does not prove that a real policy or opaque model input has no dependencies.

The trusted caller bootstraps a reference domain and supplies target resolution, time observations, complete logical snapshots and reconciliation facts. Module inspection finds only in-memory standard-library values and calls into the existing reference ledger; no process, filesystem, network, downloaded artifact, runtime or external effect is introduced. The authority and permit are not cloneable; private instance identity prevents another reference ledger with identical public scope from consuming a permit. This is process-local encapsulation, not issuer authentication or containment of arbitrary code in the same process. Unknown effects stay charged; only explicit trusted nonexecution reconciliation refunds them. Irrecoverable unknown is terminal and remains charged.

The first action batch left the original FA-001 transition-table and cold/transition measurement obligations unfinished. The subsequent qualified batch executes the public state/method table and eleven fixed release cost cases; [its receipt](../artifacts/execution/2026-09-08-semantics-receipt.json) records the actual boundary and limitations. Canonical transport, admitted Asupersync purpose contexts and production integration belong to later packets; they are explicit nonclaims of FA-001's original semantics-only contract. Source review alone is not a passing gate.

The registry negative-test sandbox now copies the real reference integration tests and implementation-status file alongside source and artifacts. These became roadmap result references in this batch; omitting them from the fixture produced unrelated missing-file findings in eighteen negative tests. Checker behavior and exact causal assertions are unchanged.

## FA-002 declared perimeter reference profile · 2026-09-08

`crates/fa-reference/src/perimeter.rs` models the L5 effect-route inventory with L0 scope identifiers. It is a pure in-memory declaration model, not a new production command or epistemic response. It validates exact scope/family/route identities, the four declared trust-domain labels, bounded threat classifications, credential holders, bypass dispositions and residual nonclaims. Immutable `route_for` preserves threat and bypass fields; the mediation convenience lookup returns only a declaration, never permission or prevention evidence.

Its conservative family profile rejects any actor-direct credential when any route claims brokered mediation. A mixed credential profile is unsupported; callers must not hide credentials or rename overlapping effects to evade this rule. Unmodelled bypasses refuse in every mediation mode. Modeled residual bypasses are representable for observation/cooperative declarations and remain uncovered. Count, per-string and aggregate text limits execute before retaining the supplied records; they do not bound allocations already made by the caller.

No filesystem, process, network, runtime, dependency or external effect was added. The inventory does not yet provide the complete proposed effect catalogue, actor-visible credential surfaces, attested/profile assumptions, completeness basis or input loader required by full FA-002. Unit and independent public integration tests qualify only the bounded declaration/lookup seam. The retained one-guard counterexample demonstrates the unmodelled-bypass regression; it does not exercise a real deployment bypass.

## Pinned Asupersync admission blocker clarification · 2026-09-08

A further read-only review used Git objects at the existing pin `9d5bc0b2d3767275a866feed5c3347bbf4a2de64`, not the donor checkout's moving HEAD. The manifest identifies `asupersync 0.4.10`. Disabling defaults omits optional macros but does not remove the unconditional build script, mandatory fundamental/native dependencies or the non-WASM process module. The build script can invoke Git; the process module locally permits unsafe code and calls native libc operations. Root `deny(unsafe_code)` is therefore not a dependency-wide unsafe prohibition. These paths block the requested native profile under the present admission constitution; no external row, exception, package or lockfile change was made.

The factored `franken-kernel 0.4.9` has a smaller static surface, but still requires complete serde/derive closure review and cannot replace the Asupersync runtime. This was source inspection, not a resolved per-target Cargo inventory, donor build, runtime artifact audit or admission. FA-053 retains the blocker until upstream factoring and individually reviewed closure decisions supply an admissible profile.

### 2026-09-08 full-input and frontier reference review

Reviewed `full_input.rs`, `product_frontier.rs` and their new public integration tests. Whole-view equality now includes required tokenizer/policy/model epochs. A confounded omission-position test was corrected to vary only that position; a missing omission-presence negative was added. Review rejected an unnecessary marker accessor after confirming the marker is Copy, and corrected a mistaken conflation of terminal sequence zero with marker generation zero. Prefix closure semantics were preserved. The [373-test RCH receipt](../artifacts/execution/2026-09-08-input-frontier-receipt.json) retains the actual tokenizer-neutralization failure and source identities. No provider or marker authentication is inferred.

### 2026-09-08 seeded-history and draft-format reference review

Reviewed `history.rs`, `canonical_json.rs`, shared `strict_json.rs` and the independent public tests/manual fixtures. The initial replay scheduler's low-bit alternation was replaced by mixed output, and its regression now requires more than the two schedules that the old code produced. Snapshot and aggregate byte bounds supplement event counts. Public mutable format fields and accidental nonempty-array requirements were corrected; missing schema-version diagnostics distinguish absence from wrong type. Parser resource-limit precedence and canonical numeric narrowing remain explicit. The existing “Canonical evidence identities” addition (FI-A06, §7.1, FA-005) covers this reference format; production identities/authentication remain later work. [Executed evidence](../artifacts/execution/2026-09-08-history-format-receipt.json) retains 406 passing tests, independent planted schedule/canonical failures and bounded descriptive costs. A source-sharing move first preserved the parser body, followed by the separately reviewed coordinate repair; no package edge, executor or native dependency was added.

### 2026-09-08: independent format properties and prospective compiler qualification

The property review caught a shared-implementation oracle: decode/encode agreement alone could hide a shared lossy or noncanonical encoder. The repaired tests compare to independently generated original syntax and inspect raw key order and outside-string whitespace. Invalid enum, type, digest and required-array cases have explicit negative twins. Separate fresh-process release tests measure first decode; heaptrack attributes real allocation stacks with a same-binary zero-test baseline and retains its older compiler boundary. [The new qualification](../artifacts/execution/2026-09-08-history-format-qualification-receipt.json) records a real rolling resolution, 412-test gate and identical dated freeze, preserving the original roadmap condition and historical compiler receipts.
