# Adaptive monitoring of source-checked learned KV

## Connected operation

`monitor::learned::LearnedRefinementMonitor` turns the existing source-checked learned-KV probe evaluator into an automatic bounded refinement loop. It is the plan 10.3/10.4 progressive-monitoring consumer, not a new codec, numerical accumulator, harmfulness classifier or effect authority. The original monitor validates the complete nonempty, unique-ID, common-profile probe roster. The original `LinearProbe::evaluate_learned` supplies every score interval and outcome; the original checked XOR residual implementation supplies exact promotions.

The initial pass evaluates every frozen probe on the checked coarse row. A certified alarm ends analysis as Alarm; exact threshold equality stays AtThreshold; NoAlarm requires every registered probe to certify quiet. NoAlarm concerns this row and these probes only. None of the report types converts to a permit, live capture, model state or qualified monitor certificate.

When an interval is unresolved, the monitor derives relevant groups from the actual registered coefficient words. Signed zero has no dependency; every nonzero coefficient, including subnormals, does. Already exact groups and groups whose source-checked radius is zero need no promotion. There is no floating-point importance cutoff or MSE-based shortcut. Candidate heads are ordered deterministically. The first retained candidate that fits the remaining budget is selected, including its complete dependent-probe reevaluation cost. Missing or unaffordable earlier candidates do not prevent trying another useful retained candidate.

Only unresolved probes dependent on the selected head are reevaluated. Previously certified quiet observations retain their original immutable refinement snapshot and are reused. Each exact promotion can occur once. The process stops on the original outcome rules, unavailable useful residuals, or budget exhaustion. Missing residuals are never regenerated from a hidden source or dropped from the problem definition. Reaching the end of an incomplete refinement path yields Unresolved, never NoAlarm.

## Resource contract

`LearnedMonitorBudget` independently limits encoded representation access, probe coordinates, learned reconstruction products, materialized scalar values and exact group promotions. It is frozen at construction. A shared remaining allowance can reduce but not enlarge it. Per-row retained refinement history also has a hard 128-promotion ceiling.

The base representation size is charged once before the first complete probe pass. Later charges include the entire selected residual block, original group reconstruction and only affected probe evaluations. Every such combined step is priced before refinement or scoring. Insufficient base/pass allowance returns BudgetExhausted with no observations or charged work. Insufficient later allowance preserves earlier observations without falsely claiming they resolve the remaining probes.

These are logical operation and exact representation-length accounts, not network-transfer measurements, a CPU/latency bound or peak RSS. Original source checking and ALL retained residual storage are separately budgeted by CheckedKvBudget before this monitor exists. A retained source object can own residuals that a particular analysis never uses. Source descriptors, codebook storage, observation metadata, map traversal, allocations, interval integer arithmetic and failed attempts must not be relabeled as free measured execution. The report retains the immutable checked evidence and numerical snapshots so its older observations remain inspectable after original source owners are dropped.

## Added source scenarios

Eight new Rust test functions cover automatic rare-signal alarm recovery while reusing an already quiet probe; coarse quiet and exact threshold equality without residual reads; missing residual holds; a missing earlier head with a later useful head; all five exact/one-below budget boundaries; the original exact-score oracle for a subnormal squared signal under huge cancellation; unchanged roster/profile validation under a zero allowance; and independent analyses after source-owner destruction. One compile-fail example prevents conversion of the monitor report into a permit. Existing probe arithmetic, XOR verification, scalar monitor tests and their assertions are unchanged.

The targeted command was attempted in this editing environment:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_monitor
```

It failed before compilation with `rch: command not found`, exit 127. Rust compilation, tests, formatting, Clippy and revision-bound qualification remain unexecuted. No Beads packet or production gate is closed. This reference implementation adds no dependency, unsafe block or substitute inference/runtime foundation.
