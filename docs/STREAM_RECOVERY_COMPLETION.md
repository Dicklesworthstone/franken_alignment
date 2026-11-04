# Preserve original stream recovery outcomes

The anchored stream receipt service retains the original PendingReconciliations
returned by Fence/Time/Sweep and a separate projection of every Dispatching,
Unknown and IrrecoverablyUnknown ledger entry at that acknowledged recovery cut.
An empty sweep cannot erase an irrecoverable liability omitted by later sweeps.
The report contains native outcomes, revision and charged units, not payloads,
source evidence or a new outcome evaluator.

The selected actor still receives only the existing Knowledge response. Service
continues through its original bounded, credential-checked session even when an
unrelated delivery remains unresolved. After serving receipts, reconcile mode
returns an operator error if any native sweep result is unresolved/refused or any
retained stage remains unresolved. It does not cancel a valid selected receipt,
refund a liability, retry a sweep or recreate an effect. Receipt-only mode records
that no sweep was attempted; it is not converted into a settlement requirement.

Post-recovery timeout and peer-service errors include the acknowledged recovery
cut and original outcomes, distinguishing them from pre-commit startup failure.
The diagnostic identifies its historical revision rather than claiming that it
represents a newer faulted owner. The mode summary says reconciliation_attempted,
not reconciled. No actor wire bytes or authority journal encoding change.

Retention expiry leaves the original action Unknown and charged. A later native
sweep visits it again and returns RetentionExpired; it does not perform explicit
abandonment or produce an empty sweep. The original broker's separate trusted
abandonment operation is now exposed by the [durable owners](DURABLE_UNKNOWN_ABANDONMENT.md),
with an explicit operation, attempt and authority predecessor. It is never
selected by this receipt service or by expiry. Its irrecoverable disposition
stays charged and unresolved in the completion report even when later sweeps
omit it.

Three native integration functions cover a real two-key confirmed request beside
an expired dispatched tail, receipt delivery before operator failure, repeated
Unknown/RetentionExpired accounting, the matching published-tail control,
receipt-only no-sweep behavior, and late timeout after acknowledgment. Two
explicitly synthetic diagnostic controls exercise native-refusal rendering and
empty-sweep reports with retained Unknown or IrrecoverablyUnknown. The latter
pairs each refusal with an otherwise identical completed report that retains a
nonzero execution charge. These pure inputs create no endpoint history and are
not evidence of a persisted irrecoverable transition.

At preparation, compilation and test execution were pending the required RCH
gate. Full FA-014 closure and remote exactly-once assurance remain unqualified.

On 2026-10-24, exact source `a985f9cd` passed all four original stream-completion
functions and the new pure diagnostic unit within the 63-function combined
selection, zero failed/ignored. The native functions retain valid receipt
delivery beside actual repeated expired-tail liabilities; the two synthetic
controls remain report-only evidence. All-target compilation passed, while
Clippy and canonical dependency admission failed. Full qualification stays open.
