# Implementation status · design revision 0.3

Current execution evidence: [receipt](artifacts/execution/2026-09-08-action-receipt.json).

## Qualified action/permit reference semantics

The complete frozen RCH gate passed **335 tests: 58 reference unit, 30 integration, 242 xtask and 5 doctests**, with zero failures, ignored tests or filtered tests, on `hz3` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-action-receipt.json) binds job `j-30004650170124324`, 135 reviewed source/build inputs and all four gate attempts. Final documentation and tracker updates are checked separately; the earlier receipts retain their original source boundaries.

FA-001 now freezes exact scope, target versions, payload, declared logical witnesses, policy epoch, deadline and units. Opaque instance-bound permits consume the existing rights ledger once; a permit from another authority refuses even with identical authorized state. A missing clock observation refuses preparation; an explicitly observed tick zero is valid. The initial unqualified implementation reproduced that clock defect with ten passing tests and one failing regression; [the complete original source/test patch](artifacts/execution/2026-09-08-action-clock-red-baseline-tests.patch) and raw run are retained. The final twelve public action tests cover binding changes, witness limits, expiry, revocation, lifecycle, accounting and unknown-effect liability, alongside eighteen existing integration tests.

Qualification retained three failed gate attempts: a test-helper Clippy warning, a worker missing its nightly formatter, and eighteen registry negatives with incomplete sandbox inputs after new roadmap artifact links. The warning and worker setup were repaired; sandboxes now copy the referenced real files, with exact negative assertions unchanged. Full FA-001 remains in progress: this batch supplies reference semantics, not admitted Asupersync purpose contexts, canonical wire compatibility, an authenticated issuer, a broker, a durable ledger or cold/transition cost measurements.

FA-052's existing five-file provenance results were independently reconciled and its missing changelog entry added. It is complete only as source intake. FA-053 is blocked: the pinned Asupersync native source includes an unconditional build script, mandatory unadmitted dependencies and native FFI paths even with default features disabled. No foundation has been admitted and no donor source was changed.

## Fresh review: corrected reference and operator boundaries

The complete frozen RCH gate passed **319 tests: 57 reference unit, 18 integration, 242 xtask and 2 doctests**, with zero failures, ignored tests or filtered tests, on `vmi1227854` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-fresh-review-receipt.json) binds the base, exact overlay, 133 source/build inputs and all retained attempts. The initial regression batch on unchanged `3001c52` implementation reproduced eleven failures with one passing control; [its exact test patch](artifacts/execution/2026-09-08-fresh-review-baseline-tests.patch) is retained separately from later added controls.

Empty reference domains can now close at sequence zero only with an explicit nonzero-generation caller-trusted marker; capacity, scope and post-close checks still apply. The admission gate refuses duplicate package IDs before identity collapse, and observation projection refuses duplicate resolve nodes or partially unreadable dependency kinds/targets. Document scanners exclude indented code and HTML comments while preserving fence boundaries. Named reference checks reject ignored tests and unqualified conditional declarations; this remains bounded source inspection, not proof that a declared test ran. The operator digest rejects non-regular paths before invoking the hash utility; its test covers a regular file and directory, not an executed FIFO hang experiment.

Two full-gate failures remain visible: a late diagnostic edit needed formatting, then an added test's local variable shadowed its helper. Both were repaired without relaxing assertions. The malformed README performance table was corrected and the old package digest/report explicitly labeled historical. Production behavior, full foundation admission, signing and release remain unimplemented; the previous DSR result has not been rerun on these changes.

At that fresh-review checkpoint, eight affected leaves or epics had been reopened; three scoped reference/operator leaves were reclosed, leaving five checker leaves/epics open. Its 336 live beads comprised 12 closed and 324 open. The later action batch independently reconciled the original checker criteria after FA-052 provenance completion and reclosed those five in dependency order. These are checker and source-intake results, with no production increment.

## Roadmap admission ordering verified

The reviewed roadmap now makes complete admission an explicit prerequisite of native types (FA-007) and the ATP adapter (FA-064). The source binding was updated only for that reviewed roadmap change. A fresh frozen RCH gate on `hz3`, job `j-30004650170124271`, passed all 298 tests under `nightly-2026-09-07`; [the inputs and raw log](artifacts/execution/2026-09-08-bridge-gate-inputs.json) identify the snapshot. Its tracker input precedes the final dependency import, which is checked separately. No native package was added or admitted.

## Ninth qualified batch: system-map and vocabulary checks

The complete isolated RCH gate passed **298 tests: 56 reference unit, 16 integration, 224 xtask and 2 doctests**, with zero failures, on `hz3` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-epoch9-receipt.json) binds 133 source/build inputs and the explicit overlay; [the passing log](artifacts/execution/2026-09-08-epoch9-attempt2.log) follows a retained Clippy refusal.

The checker validates nine layers, 35 registered verbs, typed links, exact Knowledge variant sets, bounded command/address syntax and the checker-epic bead link. It distinguishes payload types and domain constructors from explicit Knowledge variants. Its 28 new negative/control tests do not implement a production command or a layer conformance suite. A full-gate injected retired command failed at the intended system-map check after earlier gates passed; the exact input and raw log are retained. The updated four-step DSR integration subsequently passed on unchanged clean commit `3d8fd5a7cb1bd25ddac1bbb69069ed6d9997039f`, including all 298 tests through RCH; [the integration record](artifacts/execution/2026-09-08-epoch9-gate-integration.json) retains the exact source binding and raw-log hashes. Later tracker and reality-check edits are outside that frozen-source result.

## Eighth qualified batch: runtime checkout and current declarations

The complete isolated RCH gate passed on `vmi1227854` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 196 xtask tests and 2 doctests; 270 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch8-receipt.json) binds 47 overlay files and 99 verified source/build inputs. [The raw log](artifacts/execution/2026-09-07-epoch8-code-attempt1.log) includes clean concordance and current-prose reports.

The gate now resolves the invocation's actual checkout; a cached binary cannot keep using a removed compile-time archive path. Current declarations are checked against registries and a named retained execution receipt, with bounded real-file reads, canonical receipt paths, symlink refusal and shared Markdown fence semantics. Historical counts remain historical. A receipt's fields are comparison inputs, not proof of execution by themselves.

The planted-concordance experiment stopped on the intended unknown invariant after earlier checks passed. The first DSR integration attempt is retained as failed: its second Cargo invocation exposed the stale archive path that this batch repairs. On 2026-09-08 UTC, the repaired committed revision `2e60924598f5283b8eeab5482087db87f154f106` passed all three required DSR checks, including the complete 270-test gate, with identical clean source identities before and after. A fresh injected-defect run on that same code refused the unknown invariant after source, admission and registry checks passed. [The integration record](artifacts/execution/2026-09-08-epoch8-gate-integration.json) links the exact raw logs and inputs. Production remains unimplemented, and mandatory Asupersync dependency/build/runtime admission remains incomplete.

## Seventh qualified batch: owned concordance and admission discrimination

The complete isolated RCH gate passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 174 xtask tests and 2 doctests; 248 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch7-receipt.json) binds the 40-file overlay and 67 verified source/build inputs. [The passing log](artifacts/execution/2026-09-07-epoch7-attempt4.log) contains the actual concordance report: zero missing/dangling findings for 231 headings, 40 invariants, 21 hypotheses and 143 packets. The four canonical-reference corrections from the sixth batch are now checked by execution.

The admission compiler compares full source/name/version identities and exact target, feature and dependency-edge scopes. Real Cargo observations feed the same evaluator as the unit twins. The prior three failures are retained: test compilation, Clippy, then three unchanged regressions that caught a real source/path bypass missed by source review. Root restored external-first/path checks and made every evaluator refusal visible; no negative test was weakened. Both required target inventories and actual child-process failure paths executed. No external package is admitted, FA-053/054 remain incomplete, and this is no production qualification.

The five founding-source immutable blob identities were independently revalidated; [the scoped result](artifacts/execution/2026-09-07-founding-source-results.tsv) does not promote donor/runtime evidence. DSR wiring and an injected-defect full-gate experiment remain outstanding.


## Sixth qualified batch: explicit founding concordance

The sixth isolated RCH gate passed all **217 tests** on `ovh-a` with the reviewed concordance and its source-manifest binding. [The receipt](artifacts/execution/2026-09-07-epoch6-receipt.json) retains the exact six-file overlay and separates execution from the independent source audit: 231 required heading keys, 40 invariants, 21 hypotheses and 143 packets have direct typed mappings. The 67 engineering additions remain labeled additions serving the founding ideas. The full owned concordance checker is the next task; these coverage counts are not yet its output. **Correction at 22:58 UTC:** the manual audit normalized four shorthand section references, so its zero-unknown claim was too broad under the canonical-key contract. The rules bead was reopened and those references corrected; the owned checker must execute before reclosure.

## Fifth qualified batch: closed initial admission contract

The fifth isolated RCH gate passed on `ovh-a` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 143 xtask tests and 2 doctests; 217 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch5-receipt.json) binds the nine selected overlay files and [raw execution log](artifacts/execution/2026-09-07-epoch5-attempt1.log). It verified 40 source/build inputs and recollected both required target inventories. Apple metadata collection remains distinct from Linux compilation and execution.

The two local rows now carry checked source identity, version, target coverage, reason, ownership, decision, build closure and scoped runtime review. The policy binds the source-manifest digest; missing requirements, silent target removal, permissive external flags and false closure declarations refuse. Dependency-edge diagnostics preserve declared kinds, targets and malformed evidence independently of input order. The schema leaf is qualified; the general compiler's target/source discrimination remains unfinished, and no external foundation or production feature is admitted. Concurrent concordance edits were excluded from this batch.

## Fourth qualified batch: reviewed source snapshots

The fourth isolated RCH gate passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 125 xtask tests and 2 doctests; 199 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch4-receipt.json) identifies the exact base and four overlay files; [the raw log](artifacts/execution/2026-09-07-epoch4-attempt1.log) records verification of all 39 reviewed source/build inputs before the remaining gate commands. The selected snapshot used committed concordance bytes, excluding concurrent concordance work.

The verifier requires exact file-set and SHA-256 equality, includes the six embedded registries, refuses symlink ancestors and leaves, bounds directory traversal, and permits a second hash utility only when the first is absent. Real filesystem and subprocess regressions passed. It relies on trusted operator hash tools and an externally frozen checkout; it does not provide an atomic snapshot or authenticate those tools. Full admission rows, runtime closure and external admission remain unfinished. No bead was closed for this intermediate prerequisite.

## Third qualified batch: capped reduction and owned registry checks

The third isolated batch passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 reference integration tests, 102 xtask tests and 2 doctests passed; zero failed**. Qualified source is committed in `84eeca9`; [the receipt](artifacts/execution/2026-09-07-epoch3-receipt.json) identifies the exact base, nine overlay files and passing [remote log](artifacts/execution/2026-09-07-epoch3-attempt3.log). The preceding Clippy failure and real Cargo ancestor-workspace discovery failure are retained. The latter was fixed by explicitly binding metadata collection to the requested `Cargo.toml`; its test was not weakened.

The reference reducer clips empirical weights per member and cohort using name-independent floor-proportional allocation, discards fractional remainders, rejects actor statements and preserves exact disqualifier dominance. It grants no authority and does not implement credibility or helper selection. The owned xtask now checks registry IDs, DAGs, declared links, retained files and scoped Rust test declarations. It rejects lexical lookalikes and escaping symlinks, and checks the actual compiler-reported identity against the qualified Linux commit. Those checks do not authenticate operator binaries or prove the referenced invariants. At that batch, full founding-concordance validation, source-snapshot verification and general dependency admission remained unfinished.

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
| Founding-ideas concordance | `docs/FOUNDING_IDEAS.md` and `registry/founding_concordance.json`: 38 founding ideas, 9 syntheses, 8 labeled non-literal imports, 67 engineering additions | Traceability of the design, not evidence that any mechanism works |
| Agent-facing surface (tower, Knowledge wrapper, addresses, vocabulary, journal, situation, explain, affordances, rehearsal, annotations, handoffs, precheck) | Plan §6.7, §6.8, §17.2, §17.3, §17.8, §17.11, §17.12; `docs/SYSTEM_MAP.md`, `docs/AGENT_GUIDE.md`; packets FA-132 through FA-143; beads | No command exists; the only executable is the local gate driver |
| Founding essays | Both read in full again for revision 0.3; head commits recorded in `registry/sources.json` | Complete premise; no missing post |
| Ten donor/build project deep dives | Targeted code/manifest/plan review at fixed refs (revision 0.2) | Exact files/scopes recorded; not full repository audits or builds |
| Pure-Rust reference workspace | Source present, zero external packages, formatted with `cargo fmt --all` on 2026-09-06 | Selected logical semantics only; not a broker |
| Rust test functions | **58 unit + 30 integration reference tests, 242 xtask tests and 5 doctests passed remotely on 2026-09-08 UTC**; earlier runs remain dated below | Exact frozen batch above; not production evidence or credit for other work in progress |
| Rust local gate driver | **Complete reference gate PASS remotely on 2026-09-08 UTC** under `nightly-2026-09-07`; earlier local 2026-09-06 gate retained below | Initial inventory, registry-core and full concordance checks execute; external admission remains incomplete; new source requires a new run |
| Production release gate | Explicitly refuses release | No qualified broker/toolchain/target/signing closure |
| Machine-readable registries | 40 invariants, 21 hypotheses, 143 packets, 5 SLO targets, founding concordance (38 ideas, 9 syntheses, 67 engineering additions), preregistration ledger with no preregistered protocol yet, system map (9 layers) and vocabulary (26 nouns, 35 verbs, 12 reason codes) | Static structural checks are not proofs of their claims |
| Beads task graph | All 143 roadmap packets have full owners; 336 live beads: 18 closed, 316 open, one in progress and one blocked; `br ready` returns three tasks | FA-001 remains in progress; FA-053 is blocked. Provenance and checker closures do not qualify production. Earlier graph counts describe their dated checkpoints |
| Production control broker / persistence / containment | Not implemented | Reference code performs no external effects |
| Native foundation adapters | Planned, admission blockers recorded | A reviewed source file is not an integration |
| Trained helpers / codecs / signatures / surprise residual / rewind | Research and implementation plans | No measured safety, compression, detection or containment result |
| Receipts, assurance profiles, passports, autonomy ledger, canaries, risk-theater detector, formal anchors | Plan subsections, invariants FA-INV-035 through FA-INV-038, packets FA-117 through FA-131, beads | No verifier, no proof, no profile exists yet |
| Local DSR quality integration | All four required checks executed and passed on unchanged clean revision `3d8fd5a`, 2026-09-08 UTC; Cargo checks ran through RCH | Reference/operator gate only; no production build, signing or release profile |
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
