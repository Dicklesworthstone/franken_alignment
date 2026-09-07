# FrankenAlignment

**Evidence-carrying control for powerful agents. Pure Rust. Structured concurrency. Versioned judgment. Economical introspection.**

> **Design revision 0.2, September 6, 2026.** This repository specifies a general monitoring, interpretation, experimentation and control toolbox. It contains a comprehensive plan, pinned source audits, machine-readable design registries and a small safe-Rust reference workspace. It is not a deployed alignment system. The Rust source has not yet been compiled in the preparation environment; [validation status](artifacts/VALIDATION_REPORT.md) separates static checks from unexecuted gates.

## The thesis

An agent's apparent cooperativeness is not an authority boundary. Neither is a helper's confident verdict. FrankenAlignment is designed to connect **what an agent observed, what it proposed, what independent observers actually saw, what justified permission, and what effect actually occurred** through one coherent evidence and authority model.

Its foundations are Jeffrey Emanuel's [external-helper alignment proposal](https://github.com/Dicklesworthstone/some_thoughts_on_ai_alignment) and [introspective compression/metacognition proposal](https://github.com/Dicklesworthstone/llm_introspective_compression_and_metacognition). Their combination suggests more than an output filter: an external control system with economical access to internal state, independent specialist judgment, replayable experiments and bounded authority.

**An observation is data. A judgment is evidence. A permit is authority. None silently becomes another.**

## What changed in revision 0.2

The [deep review](docs/SOURCE_REVIEW.md) goes into actual source and manifests across Asupersync, SQLite, FS, Search, Markdown, GraphDB, NetworkX, NumPy, Torch and DSR. It results in architectural changes, not merely a longer dependency list.

| Mechanism | What it enables |
|---|---|
| **Epistemic MVCC** | Reuse judgments while their exact input witnesses remain valid; invalidate on changed values, phantom insertions and semantic epochs |
| **Authority graph compilation** | Independently check that every modeled route to a protected effect crosses qualified enforcement; expose graph-completeness assumptions |
| **Progressive ATP evidence** | Verified prefixes, bounded sparse journals, exact receiver bases and refinement transfer without confusing partial evidence with complete review |
| **Observation-plan algebra** | Compile the question into authorized, bounded, costed operators; reject shortcuts such as top-k search to prove absence |
| **Incremental oversight** | Maintain fleet findings with checked deltas, retractions and support sets; compare every eligible view against batch semantics |
| **Decision-sensitive compression** | Preserve registered decisions rather than only average activation error; use exact bounds where available and raw/refinement fallbacks otherwise |
| **Evidence-view commitments** | Bind each helper's judgment to the transformed, redacted, windowed bytes it actually received |
| **Structurally shared experiments** | Share immutable state and delta history without copying live credentials or resurrecting spent rights |
| **Lawful partial-order replay** | Collapse irrelevant interleavings only under a sound read/write/resource/authority independence relation |
| **Local release evidence** | Run gates on operator machines via Cargo/DSR; bind source, nightly, dependencies, targets and artifact verification without hosted Actions |

These mechanisms are intended to reinforce one another: reuse computation, reuse transfer, update only affected findings, and preserve an inspectable reason for every permission. Their joint performance and security benefits remain experiment goals, not results already measured.

## Architecture

The **control plane** is small and exact: frozen effects, current policy, conserved rights, one-shot permits, dispatch and outcome reconciliation. The **evidence plane** is bounded and progressive: capture, authentication, storage/ATP, authorized views, probes, helpers and graph queries. The **experiment plane** has no production authority: snapshot, intervene, compare and propose independently evaluated improvements.

Asupersync supplies runtime/transfer/obligation foundations. The FrankenSuite supplies admitted storage, graph, retrieval, numerical and presentation surfaces. Every adapter has its own tested contract; no library name transfers a guarantee automatically.

## Constitution

The product and its reference/gate code are Rust, edition 2024, on the latest qualified nightly. Every FrankenAlignment crate forbids unsafe. Dependencies are a closed, source-pinned universe of Asupersync and admitted FrankenSuite packages, with only individually reviewed fundamental exceptions such as serde. Native features do not excuse unreviewed transitive dependencies. No libtorch, ONNX Runtime, Python, C codec fallback, BLAS FFI or second executor inside the product.

The exact initial reference workspace has **zero external packages**. Foundation integration is deliberately blocked until its target/feature closure is qualified. The [dependency constitution](docs/DEPENDENCY_CONSTITUTION.md) records concrete donor blockers and their upstream-factor/adaptation routes.

No required gate depends on GitHub-hosted Actions. [Local release](docs/LOCAL_RELEASE.md) uses the operator's own DSR/native/local-act infrastructure. Development follows nightly; release campaigns freeze a resolved dated compiler and source/dependency closure.

## Read the design

Start with the [comprehensive plan](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md) and [revision rationale](docs/REVISION_0_2.md). The [source audit](docs/SOURCE_REVIEW.md) links exact inspected files; [research agenda](docs/RESEARCH_AGENDA.md) states falsifiable experiments; [roadmap](registry/roadmap.json) orders implementation; [implementation status](IMPLEMENTATION_STATUS.md) distinguishes source, execution and production qualification.

## Inspect the reference locally

On a machine with rustup:

```bash
rustup update nightly
cargo fmt --all
cargo run --locked -p xtask -- check
```

`cargo fmt` is an explicit preparation step because rustfmt was not available when this package was generated. The gate itself never mutates source. It checks the exact initial dependency inventory, formatting, compilation, Clippy and tests. DSR can execute it as a required local check using [the registry fragment](release/dsr-quality.fragment.yaml).

The reference contains 20 test functions for selected logical semantics. They are **not reported as passing** in this draft. It does not call any external tool, implement persistence, train a monitor, verify cryptography or provide containment. `cargo xtask release-check` deliberately refuses a production release.

## What success would establish

The first real demonstration is one brokered effect whose exact evidence view, judgment dependencies, authority cut, permit and outcome can be inspected and replayed. Later gates add real capture, native codecs, independent helper congresses, counterfactual experiments and fleet-scale incremental analysis under the same semantics.

The objective is a measurable improvement in the safety–utility–cost frontier, not a declaration that an arbitrary model is universally aligned. Positive and negative research results are both first-class artifacts.

## License

The existing **MIT License with OpenAI/Anthropic Rider** is retained unchanged. See [LICENSE](LICENSE). This is a custom restricted license, not an unqualified permissive-MIT claim.
