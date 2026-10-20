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

Four new regression functions include a real two-key confirmed request alongside
an expired dispatched tail, receipt delivery before operator failure, an empty
repeat sweep with retained irrecoverable debt, a matching published-tail control,
receipt-only no-sweep behavior, and late timeout after acknowledgment. An explicit
synthetic refusal tests only report rendering; it is not endpoint evidence.
Compilation and test execution remain pending the required RCH gate. This is
unqualified source, not full FA-014 closure or remote exactly-once assurance.
