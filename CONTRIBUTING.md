# Contributing

Begin with [AGENTS](AGENTS.md), the [plan](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md), and the relevant packet in [roadmap](registry/roadmap.json). Proposals should specify the problem, mechanism, source/proof assumptions, adversarial counterexample, measured cost and simplest correct baseline.

Production code is safe Rust in the closed dependency universe. Review all target/features and transitive dependencies. Native feature selection is not an exemption. The reference should stay simple and independent of the optimized implementation.

Prepare formatting explicitly, then run the local gate:

```bash
cargo fmt --all
cargo run --locked -p xtask -- check
```

DSR may run the same gate as a required local check. No hosted Actions completion is necessary or sufficient. Retain commands, exact source/toolchain/feature identities and raw output. Record planned or skipped tests honestly; do not use earlier Python results for the Rust source.

Research contributions need matched baselines and failure criteria. Performance contributions must preserve the declared result contract and include byte/memory/latency costs. Do not close a packet from code volume, static grep or a simulated example that excludes its real trust boundary.
