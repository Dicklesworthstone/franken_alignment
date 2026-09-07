# Implementation status · design revision 0.3

## Latest qualified batch: capped reduction and owned registry checks

The third isolated batch passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 reference integration tests, 102 xtask tests and 2 doctests passed; zero failed**. Qualified source is committed in `84eeca9`; [the receipt](artifacts/execution/2026-09-07-epoch3-receipt.json) identifies the exact base, nine overlay files and passing [remote log](artifacts/execution/2026-09-07-epoch3-attempt3.log). The preceding Clippy failure and real Cargo ancestor-workspace discovery failure are retained. The latter was fixed by explicitly binding metadata collection to the requested `Cargo.toml`; its test was not weakened.

The reference reducer clips empirical weights per member and cohort using name-independent floor-proportional allocation, discards fractional remainders, rejects actor statements and preserves exact disqualifier dominance. It grants no authority and does not implement credibility or helper selection. The owned xtask now checks registry IDs, DAGs, declared links, retained files and scoped Rust test declarations. It rejects lexical lookalikes and escaping symlinks, and checks the actual compiler-reported identity against the qualified Linux commit. Those checks do not authenticate operator binaries or prove the referenced invariants. Full founding-concordance validation, source-snapshot verification and general dependency admission remain separate unfinished work.

## Second qualified batch: full-input witnesses, frontiers and initial admission

The second isolated batch passed the complete gate remotely on `hz4` under `nightly-2026-09-07` (rustc `5a2be9f5f075d31e3ca5526b5b029881ce441253`): **44 reference unit tests, 16 reference integration tests, 69 xtask tests, one positive doctest and one private-field compile-fail doctest passed; zero failed**. The driver actually collected `cargo metadata --locked --offline --filter-platform` for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`, checked their exact initial inventory, and resolved workspace/manifest/target paths against the real filesystem. Compilation and tests ran on Linux; collecting an Apple target graph is not Apple execution.

[Exact source hashes, command and scope](artifacts/execution/2026-09-07-epoch2-receipt.json) bind [the passing log](artifacts/execution/2026-09-07-epoch2-attempt2.log). [The preceding Clippy failure](artifacts/execution/2026-09-07-epoch2-attempt1.log) is retained; the fix preserved the same refusal semantics and suppressed no warning.

Opaque witnesses now retain all supplied bytes, profile, spans and omission semantics behind validated private construction. Product frontiers keep source/branch/projection/epoch and stages separate, requiring an explicit caller-trusted closing marker for closure. These remain reference models: no provider capture, authentication, durable frontier or production authority. The new bounded JSON reader and metadata checks strengthen the initial two-package gate; full admission-row/source/build/runtime closure and external positive cases remain unfinished. The exact lockfile guard remains unchanged. The reducer and registry compiler were excluded from this batch.

## First reference batch on 2026-09-07

The first code batch adds a frozen commit–reveal transcript model and a containment-reset rights model. The final complete gate passed remotely on `ovh-a` (`x86_64-unknown-linux-gnu`) using explicit `nightly-2026-09-07`, rustc `5a2be9f5f075d31e3ca5526b5b029881ce441253`: **34 reference tests passed, zero failed**, plus formatting, workspace compilation and Clippy with warnings denied. Eight new tests cover the round; six cover reset. Source was formatted before the isolated RCH transfer; exact base, overlay hashes, command and boundaries are in [the execution receipt](artifacts/execution/2026-09-07-reference-epoch-1-receipt.json), with [the raw gate log](artifacts/execution/2026-09-07-reference-epoch-1-final.log). An earlier pass on an older nightly and a worker-pressure refusal are retained alongside them.

The round uses a deliberately non-cryptographic FNV comparison and proves no cryptographic property. Reset preserves spent rights, epochs and dispatched/unknown liabilities while incrementing an in-memory incident counter; it does not restore an actor, persist the counter or enforce escalation. FA-056 remains incomplete, and production FA-108 remains planned. The separate admission/parser work in progress was excluded from this frozen batch and receives no execution credit from it. The 2026-09-06 and revision 0.2 history below remains unchanged.

| Surface | Actual status | Claim boundary |
|---|---|---|
| Integrated comprehensive architecture | Revision 0.3 draft, founding essays as the spine | Requirements and research hypotheses, not deployed features |
| Founding-ideas concordance | `docs/FOUNDING_IDEAS.md` and `registry/founding_concordance.json`: 38 founding ideas, 9 syntheses, 8 labeled non-literal imports, 25 engineering additions | Traceability of the design, not evidence that any mechanism works |
| Agent-facing surface (tower, Knowledge wrapper, addresses, vocabulary, journal, situation, explain, affordances, rehearsal, annotations, handoffs, precheck) | Plan §6.7, §6.8, §17.2, §17.3, §17.8, §17.11, §17.12; `docs/SYSTEM_MAP.md`, `docs/AGENT_GUIDE.md`; packets FA-132 through FA-143; beads | No command exists; the only executable is the local gate driver |
| Founding essays | Both read in full again for revision 0.3; head commits recorded in `registry/sources.json` | Complete premise; no missing post |
| Ten donor/build project deep dives | Targeted code/manifest/plan review at fixed refs (revision 0.2) | Exact files/scopes recorded; not full repository audits or builds |
| Pure-Rust reference workspace | Source present, zero external packages, formatted with `cargo fmt --all` on 2026-09-06 | Selected logical semantics only; not a broker |
| Rust test functions | **56 unit + 16 integration reference tests, 102 xtask tests and 2 doctests passed remotely on 2026-09-07**; earlier runs remain dated below | Exact frozen batch above; not production evidence or credit for other work in progress |
| Rust local gate driver | **Complete reference gate PASS remotely on 2026-09-07** under `nightly-2026-09-07`; earlier local 2026-09-06 gate retained below | Initial inventory and registry-core checks execute; general admission and full concordance validation remain incomplete; new source requires a new run |
| Production release gate | Explicitly refuses release | No qualified broker/toolchain/target/signing closure |
| Machine-readable registries | 40 invariants, 21 hypotheses, 143 packets, 5 SLO targets, founding concordance (38 ideas, 9 syntheses, 25 engineering additions), preregistration ledger with no preregistered protocol yet, system map (9 layers) and vocabulary (26 nouns, 35 verbs, 12 reason codes) | Static structural checks are not proofs of their claims |
| Beads task graph | `.beads/` initialized 2026-09-06; the 2026-09-07 campaign is implementing reference extensions and missing prerequisites | Completion requires exact acceptance and new execution evidence; the original graph omits many foundational roadmap packets |
| Production control broker / persistence / containment | Not implemented | Reference code performs no external effects |
| Native foundation adapters | Planned, admission blockers recorded | A reviewed source file is not an integration |
| Trained helpers / codecs / signatures / surprise residual / rewind | Research and implementation plans | No measured safety, compression, detection or containment result |
| Receipts, assurance profiles, passports, autonomy ledger, canaries, risk-theater detector, formal anchors | Plan subsections, invariants FA-INV-035 through FA-INV-038, packets FA-117 through FA-131, beads | No verifier, no proof, no profile exists yet |
| Local DSR quality integration | Registry fragment and command contract supplied | Not executed through DSR on the user's hosts |
| Public repository mutation | The operator committed and pushed earlier revision 0.3 batches during the working session; later changes remain in the local working tree until committed | This document does not track remote state |

## Execution facts recorded on 2026-09-06

All logs are under [`artifacts/execution/`](artifacts/execution/). Each records host, date, toolchain identity and the exact command.

| Log | Command | Toolchain | Result |
|---|---|---|---|
| `2026-09-06-cargo-test.log` | `cargo test --workspace --all-targets --frozen` | rolling `nightly` (rustc c54751567 2026-08-22) | 20 passed, 0 failed |
| `2026-09-06-xtask-check.log` | `cargo run --locked -p xtask -- check` | rolling `nightly` | FAIL at `cargo fmt --all --check`: this host's rolling nightly directory lacks the `rustfmt` and `clippy` binaries although rustup reports the components installed |
| `2026-09-06-xtask-check-nightly-2026-08-31.log` | `cargo fmt --all --check` (18 diffs), then `cargo fmt --all`, then the gate | `nightly-2026-08-31` | Formatting applied; gate reached `cargo check` and was refused by the host's remote-build offload wrapper (no admissible workers) |
| `2026-09-06-xtask-check-nightly-2026-08-31-local.log` | `cargo run --locked -p xtask -- check` with the offload wrapper bypassed | `nightly-2026-08-31` (rustc 908501772 2026-08-30) | **PASS local_reference_gate**: lockfile inventory, source inventory (required files and the crate-level `unsafe` prohibition), `rustc -Vv`, `fmt --check`, `check`, `clippy -D warnings`, `test` (20 passed) |

What these facts do and do not establish:

- The reference source compiles, is Clippy-clean under `-D warnings`, is formatted, and its 20 tests pass on one aarch64 macOS host with one dated nightly. The formatting change is the only source mutation and is the documented preparation step.
- The gate did **not** pass under the rolling `nightly` alias on this host, because that toolchain directory is missing components; that is a host-environment defect, recorded rather than hidden. `rust-toolchain.toml` still tracks `nightly`; a release campaign freezes a dated identity anyway (plan §2.2).
- The historical fact stands: the revision 0.2 preparation environment did not compile this workspace. That prose is retained in the validation report and is not rewritten.
- Nothing here is production, release or safety evidence. Packets FA-003 and FA-004 remain open until the gate runs under the rolling nightly with its declared components and the registry checks move into the owned `xtask`.

## Founding-idea coverage

| Founding idea family | Mechanism in plan | Reference-model check | Production status |
|---|---|---|---|
| Effect gate, one-shot permits, conserved rights (FI-A01, FI-A02) | §8 | `effect_binding_and_one_shot_dispatch`, `rights_unknown_cannot_be_refunded_by_cancel`, `revocation_fences_reserved_not_history`, `reserve_budget_and_duplicate_fail_closed` and `resolved_non_effect_returns_rights_once`; executed 2026-09-06 | planned |
| Congress: independent votes, commit–reveal, consequences, credibility, escalation reports (FI-A06 through FI-A09, FI-A13 through FI-A16) | §9.2 through §9.10 | Nine bounded commit–reveal unit tests plus public-API framing/replay tests executed 2026-09-07; non-cryptographic comparison only. Twelve capped empirical reducer tests also passed in epoch3. Consequence lattice and credibility remain unimplemented | planned |
| Activation channel, sidecar-to-congress, certified margins (FI-A05, FI-I04, FI-I06) | §10 | `integer_probe_requests_refinement_at_margin`, `integer_probe_bound_matches_exhaustive_small_errors`; executed 2026-09-06 | planned |
| Elicitation, signatures, honeypots, surprise (FI-A10 through FI-A12) | §12.6, §12.7 | none; research packets FA-109 through FA-111 | planned |
| Rewind as containment (FI-A09, FI-A16, FI-I03) | §11.10 | Six bounded reset tests executed 2026-09-07: rights, spent units, epochs, dispatched/unknown effects, counter monotonicity and overflow. No host restore or persistent escalation | planned |
| Thought graph, practice, strategies (FI-I09, FI-I15 through FI-I20) | §11.4, §11.5, §11.9 | none; experiment-plane packet FA-045 | planned |
| Risk-theater governance (FI-A18) | §9.6, §9.9, §22.4 | none; FA-INV-032 has no checker yet | planned |

The previous Python reference and hosted-runner workflow remain removed. Their historical results do not validate the Rust source. Some original reference coverage, including complete congress/statistical utilities, must be ported and revalidated under packet FA-056; the Rust reference is not labeled feature-equivalent.

Preparation results for revision 0.2 are in [artifacts/VALIDATION_REPORT.md](artifacts/VALIDATION_REPORT.md), with a revision 0.3 addendum. Production invariant statuses remain planned and safety waivers forbidden.
