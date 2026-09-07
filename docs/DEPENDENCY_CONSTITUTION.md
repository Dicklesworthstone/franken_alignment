# Closed-universe dependency constitution

## Three different boundaries

**Product:** pure Rust, edition 2024, latest qualified nightly. Every FrankenAlignment crate forbids unsafe. No C/C++ FFI, libtorch, ONNX Runtime, Python, BLAS or foreign fallback. Asupersync is the only async/concurrency runtime. Native tensor and graph operations use admitted FrankenSuite code.

**Foundation closure:** membership in the FrankenSuite is not sufficient admission. The actual target/feature graph, build scripts, proc macros, native linking, downloaded runtime artifacts and unsafe sites must be reviewed. The strict “no unsafe written here” claim is separate from the dependency unsafe ledger. Even std/toolchain/OS have trust assumptions; the product never markets this as a proof that all executed machine code is memory-safe.

**Operator infrastructure:** DSR, Git, rustup, the linker/toolchain and local host automation run outside the product. DSR's Bash/yq/jq or local act does not make the product a Python/Bash application. Optional foreign comparison oracles run in isolated evaluation lanes and cannot be a required production service or quietly linked dependency.

## Admission procedure

Record package identity `(name, source URL, exact revision/version, checksum)`, target, feature set, direct reason, complete build/runtime closure, owner and decision. Inspect Cargo metadata with `--format-version 1`; inspect the resolved graph rather than merely the workspace-member list. Do not use `--no-deps` as a transitive audit. Include target-specific and build dependencies. A lockfile pins package resolution; it does not itself prove which features were activated, what a build script did, or what runtime bytes a loader downloaded.

Allowed foundation candidates are the explicit packages from Asupersync and the reviewed FrankenSuite repositories. Fundamental exceptions such as serde require individual rows. No external exceptions are currently admitted to the reference workspace. The name `ft-api` must not resolve to an unrelated registry package when the intended owned package is `frankentorch-api`.

Before admission, use an upstream feature split or a narrowly factored owned crate where practical. Preserve provenance and differential tests for any extraction. A duplicate in-house crypto implementation is not preferable to an existing audited foundation primitive merely to shrink a manifest; use the reviewed foundation interface and exact crypto profile, with no plaintext fallback.

## Concrete blockers found in the review

| Surface | Observed issue | Required route |
|---|---|---|
| FS MVCC | Mandatory compression/synchronization dependencies despite an empty default feature set | Split representation/codec/runtime concerns; choose admitted pure-Rust codecs and Asupersync-owned synchronization |
| Search/reranker | Heavy default/mandatory model-loader and support closure | Native-only feature closure and upstream factoring; verify no dynamic ONNX fallback |
| NumPy linalg | Default parallel activation plus mandatory helpers | Disable/factor Rayon; review small scalar helpers individually; no Python facade |
| Torch | Package names differ from directory names; capture ABI not implied by facade | Pin exact owned packages; explicit native model/tap/gradient bridge |
| ATP | Native and compatibility actor surfaces are distinct | Use native Asupersync bridge, not tokio compatibility |
| NetworkX | Dense cut and allocation-heavy generic views | Safe sparse representation plus explicit projection/cursor bridge and oracle tests |
| FMD | CLI can load assets; core can remain IO-free | Link core-only review profile; external active assets prohibited |

These are source-level admission blockers, not declarations that the donor repositories are unusable.

## Toolchain policy

`rust-toolchain.toml` tracks `nightly` for development. A release campaign must freeze a dated channel and full compiler identity across all hosts. Update locally with `rustup update nightly`, inspect `rustup show` and `rustc -Vv`, and run the full gates. Freeze compiler, Cargo, components, targets, linker/flags and source closure before a campaign. No mixed-nightly release.

## Current enforceable scope

The reference workspace contains exactly two dependency-free local packages. `xtask` rejects any lockfile outside this exact inventory. This intentionally crude guard is sound for this closed initial inventory, not a general parser or future package-policy engine. Expanding it requires implementing the complete target-feature/source admission gate first. The 2026-09-07 owned xtask additionally parses actual per-target Cargo metadata, rejects missing or malformed graph evidence, checks feature sets and build/proc-macro/link declarations, and confines actual manifest and target source paths. Its closed-inventory compiler unconditionally refuses external packages; metadata does not inspect all source includes, runtime downloads or build effects. The qualified epoch2 receipt is in `artifacts/execution/2026-09-07-epoch2-receipt.json`; it does not justify relaxing the lock guard. `release-check` refuses release because production qualification is absent.

The epoch4 gate additionally verifies `registry/source_snapshot.json`: every regular file under the two crates' source/test roots, the six named build inputs, and the six currently embedded registries must match its reviewed SHA-256 inventory exactly. Missing, changed, unlisted and symlinked inputs refuse. The manifest is operator data, outside its own input set; changing it is a review decision, not evidence that new source passed. Refresh it only after reviewing the selected source/input closure, then execute a new gate against that frozen selection. This is an enumerated current compile-input closure, not a generic Rust include resolver. Host SHA-256 tools are trusted operator infrastructure, and an outer freeze is required against concurrent mutation. [The retained receipt](../artifacts/execution/2026-09-07-epoch4-receipt.json) proves execution for that selected snapshot only.

Official tool contracts: [rustup toolchains](https://rust-lang.github.io/rustup/concepts/toolchains.html), [rustup overrides](https://rust-lang.github.io/rustup/overrides.html), and [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html). These explain tool behavior; the stricter admission policy above is this project's design.
