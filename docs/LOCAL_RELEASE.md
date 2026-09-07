# Local gates and DSR release contract

## Current status

The project is a design/reference package. There is no production executable to release. Rust source and 20 test functions are present, but compilation, rustfmt, Clippy and tests were **not run** in the preparation environment. `cargo xtask release-check` deliberately returns failure. No fake public key, release target, host credential or GitHub ruleset ID is committed.

## Local development commands

From the workspace, on a machine with rustup:

```bash
rustup update nightly
rustup show
rustc -Vv
cargo fmt --all
cargo run --locked -p xtask -- check
```

The formatting command is an explicit preparation mutation because rustfmt was unavailable when this source was generated. Review/commit any formatting changes before freezing the release source. The gate itself uses `fmt --check`, `check`, `clippy` and `test`, and never fixes source while claiming it was unchanged. DSR must execute the same gate against a frozen, clean source snapshot. The source inventory command alone does not run tests.

## Register the local quality gate

The fragment in [release/dsr-quality.fragment.yaml](../release/dsr-quality.fragment.yaml) belongs under the real DSR quality registry, normally `~/.config/dsr/repos.yaml`. Merge it into the existing registry; do not overwrite other projects. It uses the observed `tools`, `checks` and `required_checks` schema. Then:

```bash
dsr --json quality --tool franken_alignment --work-dir "$PWD"
```

`required_checks` are still executed when ordinary checks are skipped. Missing configuration, empty checks and dry runs cannot establish success. DSR's outer receipt captures command logs and source context; it does not turn the reference gate into production-readiness evidence.

DSR build/release reads a separate per-tool configuration under `~/.config/dsr/repos.d/`. Registering quality checks is not configuring native builds. No production build configuration is supplied here because the accepted binary/target/signing/host profile does not yet exist. This is a deliberate blocked gate, not an example with misleading placeholder secrets.

## Required production build contract

The future profile must name the repository/clean source root; exact release/tag identity; native/local-act hosts and target triples; feature sets; fully resolved nightly and toolchain identity; dependency closure; commands; private output directory; and exact primary asset names. DSR strict release configuration supports `release_contract`, `checksum_sidecar`, `exact_primary_assets`, `exact_additional_assets` and `minisign_public_key_file`; use them only with actual reviewed values. GitHub ruleset binding is an optional distribution governance control, not a hosted-runner dependency.

A strict build requires the complete target set, immutable clean sources and consistent resumed campaign identity. All build outputs/logs go outside the source closure. One failed target means no complete release manifest. A path-list hash is not a content commitment for an already dirty tree: require clean immutable roots or separately hash all relevant file contents. The compiler and dependency identities must be checked on every native host, not merely recorded on the controller.

## Required release ordering

`local source freeze -> dependency/toolchain admission -> required tests -> all-target build -> installation/upgrade checks -> exact signed manifest and proof closure -> explicit draft staging -> full remote verification -> final revalidation -> publish`.

When a production profile is qualified, the observed DSR command family is:

```text
dsr build franken_alignment --version <qualified-version> --parallel=2
dsr release franken_alignment <qualified-version> --draft --artifacts <sealed-output-dir> --no-dispatch
dsr release verify franken_alignment <qualified-version>
```

These are **future operational forms**, not runnable release instructions for draft 0.2. Do not use a generic hosted-CI fallback path, partial targets, or a broad ordinary checksum spot-check as the release proof. Build resume requires the exact original source/target/output identity; do not assume a similarly named release-resume path has equivalent implementation.

## Proof closure

The signed primary archive should contain the accepted source/dependency/toolchain identities, gate inventory, raw-log digests, benchmark manifests, feature readiness, artifact hashes and verification scope. Keeping the closure inside a signed primary simplifies exact upload/signature-set verification. Never sign a claim that a dry run executed, that a missing target passed, or that a prototype is a production broker.

The publication adapter verifies exact asset inventory, immutable remote asset identities and downloaded digest/signature bytes for every required target. Ambiguous publication yields an explicit unknown/exposure incident. Returning a release to draft can mitigate exposure but cannot prove it was never observable.

All checks run on operator machines. Optional GitHub workflow wrappers may be added for local act/native execution, but no hosted Actions status is a required input. This package intentionally contains no workflow until it can be tested in the actual local host profile.

Source basis: the fixed DSR ref and exact inspected files are listed in [SOURCE_REVIEW](SOURCE_REVIEW.md#doodlestein_self_releaser) and [foundation_audit.json](../registry/foundation_audit.json).
