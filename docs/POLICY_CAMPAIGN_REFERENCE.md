# Evidence-scoped exact-policy replay

`action::consequence::policy_campaign` compares a candidate exact policy with
retained proposal/review observations. `PolicyReplayReport::from_archives`
verifies each decision archive against its separately retained anchor before
comparison. Reports are immutable data, not effect permits or policy approvals.

This is a scoped prerequisite for plan 22.7 / FA-130, serving FI-A14 and FI-A18.
It does not close that full production packet. The old congress consequence is
retained as historical context; no helper is rerun, no changed-policy helper
answer is fabricated, and passing a candidate exact policy is labelled
`NewlyReviewable`, not permission or an assertion of harmlessness. Full congress
threshold replay, independently labelled campaign outcomes, signatures, durable
campaign publication and production qualification remain separate obligations.

## Candidate evaluation

The source action and original policy evaluation are checked again. Candidate
reads require an explicit observed key or a closed range containing the key.
An empty-range predicate needs complete retained coverage unless an observed
positive member already refutes it. Adjacent closed domains can compose; gaps
cannot. The maximum u64 key remains directly observable without an overflowing
successor. Required reads in unselected Boolean branches are checked too.

The original policy evaluator runs only after this coverage check. The sparse
reconstruction is never exposed as a complete provider database. New unsupported
reads produce `RequiresShadow` and identify the candidate node indices. Missing
evidence is not an empty-value substitute. Candidate resource-limit failure
refuses the comparison rather than producing a report for a convenient prefix.

Each row retains its action, original and candidate predicate evaluations,
observed semantic epoch, original consequence where available, and exact-policy
change class. Newly blocked and newly reviewable sets are explicit. Proposal and
later review observations remain different cases even for one action. Counts
are retained cases, not independent trials or population rate estimates.

The fixed ceilings are 5,120 cases and 32 MiB of retained action/witness variable
bytes, with smaller caller-selected limits. Repeated witness occurrences and
candidate witness copies count. These are logical data bounds, not allocator,
peak-memory, CPU-time or latency measurements. Policy/node bounds apply too.

## Verification

Eight numerical/reference unit tests accompany the kernel, covering observed
absence, unknown Boolean branches, closed-domain union, positive nonemptiness,
previously denied cases, maximum keys, input limits and baseline substitution.
They have NOT been compiled or executed in this session. Rust/RCH is unavailable;
no historical receipt is reused and no bead is closed. Public archive provenance
and supplied snapshot truth remain the original verifier's trust assumptions.
