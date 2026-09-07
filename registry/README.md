# Design registries

These are versioned planning inputs, not machine-certified production authority.

`invariants.json` records 30 exact obligations and scope/negative tests. `roadmap.json` sequences 102 packets. `foundation_audit.json` pins the new code review; `sources.json` retains founding/earlier research provenance. `dependency_policy.json` and `release_readiness.json` explicitly admit no production foundation closure or release yet. `claims.json`, `slo.json` and `operation_costs.json` retain the initial claim/cost model, strengthened by the normative v0.2 plan; any new production operation needs a registry row before activation.

Preparation validates parseability, IDs, dependency acyclicity, file/test-symbol references and selected cross-file consistency. Those results are recorded in the validation report. The included Rust xtask currently enforces only its documented initial dependency/source inventory and actual local compilation/test commands; it is not a complete JSON/schema/registry validator. Implement the full owned registry compiler before production activation.

No missing checker is a passed checker. Source-present reference tests are explicitly unexecuted in this package's preparation history.
