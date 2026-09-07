# Preparation validation · revision 0.2

## Scope

This report records preparation checks on the revised repository files. It does not report Rust compilation, rustfmt, Clippy or unit-test execution. `rustc`, `cargo` and `rustfmt` were not installed in the preparation environment. The 20 Rust test functions are source-present, not passed tests. No foundation was built, no detector was trained, and no production effect boundary or benchmark was exercised.

## Checks executed

The preparation used local Python standard-library processing and the available JSON Schema validator **outside the shipped repository**. It parsed JSON/TOML, checked unique work/invariant identities and the roadmap dependency DAG, verified referenced Rust test symbols and artifact paths, checked local Markdown paths/anchors, checked commit-ref/URL structure, and inspected the exact two-package dependency-free workspace. The three draft schema/example pairs were validated. The optional first-publication shell script was syntax checked with `bash -n`.

The raw check record is [static-validation.json](static-validation.json). The final record must contain no errors; a prior failed preparation pass was corrected rather than treated as success. Source-pin checking here checks the structure of the recorded commit URLs, not a fresh network audit or a proof that every donor file was read completely. Read scope is recorded in the source audit.

## What the local Rust gate still must establish

Run `cargo fmt --all` as an explicit preparation step, then `cargo run --locked -p xtask -- check` on the operator's machine. The gate checks formatting, compilation, Clippy and tests without modifying source. Review and commit formatting before a frozen DSR campaign. Compilation or test failures must be fixed and logged; this report is not a substitute for that run.

The initial lockfile guard is intentionally exact and narrow. It verifies only the two dependency-free reference packages, not a complete future foundation closure. `release-check` deliberately rejects a production release. Registry consistency and test-symbol existence do not establish invariant truth.

## Archive integrity

`SHA256SUMS` covers all package files except itself. The completed ZIP is reopened and every listed file is compared with its digest during preparation. This proves packaging consistency with these bytes, not authenticity under an operator signing key. No signing key or release artifact is fabricated. The original revision-0.1 archive remains separate and its Python test results do not validate revision 0.2.

## Revision 0.3 addendum (2026-09-06)

The revision 0.2 text above is retained verbatim as history. Revision 0.3 re-ran the static checks with the same method (Python standard library outside the shipped product) over the enlarged registries, the founding concordance, the preregistration ledger, the beads export and all local Markdown links; the record is [static-validation-0.3.json](static-validation-0.3.json) and contains no errors. Additional checks: plan subsection numbering is unique (the duplicate §7.5 of revision 0.2 was corrected); every invariant, hypothesis and revision 0.3 packet carries a founding-idea link; the concordance's section references resolve; the beads graph has no cycles and every bead has a description.

Rust execution is no longer "not run": the 20 reference tests passed on one operator host, and the complete `cargo run --locked -p xtask -- check` gate passed under `nightly-2026-08-31` after the documented `cargo fmt --all` preparation, which was the only source mutation. The gate failed under the host's rolling `nightly` because that toolchain directory lacks the rustfmt and clippy binaries; that host defect is recorded, not hidden. Logs with host, date, toolchain and command are under [execution/](execution/). One host on one date is quality-gate evidence for this source only, not production, release or safety evidence.

`SHA256SUMS` was regenerated over the tracked files of the revision 0.3 tree (excluding itself and the beads database sidecars). It remains a packaging-consistency digest list, not a signature.

The agent-legibility batch added checks that the system map's layers reference existing plan sections, invariants, packets and registered verbs, that every `fa <verb>` and address kind used in the README, plan, system map and agent guide is registered in `registry/vocabulary.json` and not retired, and that no bead body uses a retired verb.
