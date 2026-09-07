# Design registries

These are versioned planning inputs, not machine-certified production authority.

`invariants.json` records 38 exact obligations and scope/negative tests. `roadmap.json` sequences 131 packets. `founding_concordance.json` is the normative map from the two founding essays (38 ideas, 9 syntheses, 18 engineering additions, 8 non-literal imports) to sections, invariants, hypotheses and packets. `experiments.json` is the preregistration ledger (no protocol is preregistered yet). `foundation_audit.json` pins the code review; `sources.json` retains founding/earlier research provenance with head-commit pins. `dependency_policy.json` and `release_readiness.json` explicitly admit no production foundation closure or release yet; the latter records the single local quality-gate execution of 2026-09-06. `claims.json`, `slo.json` and `operation_costs.json` retain the claim/cost model; any new production operation needs a registry row before activation.

Preparation validates parseability, IDs, dependency acyclicity, file/test-symbol references and selected cross-file consistency. Those results are recorded in the validation report. The included Rust xtask currently enforces only its documented initial dependency/source inventory and actual local compilation/test commands; it is not a complete JSON/schema/registry validator. Implement the full owned registry compiler before production activation.

No missing checker is a passed checker. Source-present reference tests are explicitly unexecuted in this package's preparation history.
