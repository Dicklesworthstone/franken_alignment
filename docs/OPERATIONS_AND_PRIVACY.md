# Operations and privacy

This is a proposed runbook, not an operational service manual for an already deployed product.

## Health is a vector

The operational state is the `SituationReport` of plan §17.8, obtained by `fa situation`; every dashboard is a rendering of it. Expose enforcement, authority continuity, observation coverage, helper availability, calibration validity, evidence retention, and replay closure separately. A healthy renderer cannot compensate for a broken gate. A green CPU metric cannot conceal a missing trace interval. Report unsupported capabilities as unsupported, not unhealthy versions of features that do not exist.

## Failure handling

On required-observation failure, stop admitting the affected effects or select a preregistered narrower mode. On authority-state uncertainty, fence dispatch. On unknown remote outcome, retain its charge and status token. On lost artifact/key, mark the specific replay closure unavailable. On corrupted derived search/graph state, rebuild from committed sources; do not infer permission from an empty result.

Run shutdown closes admission, handles undispatched reservations, and reconciles already admitted effects. Cancellation is not proof of nonexecution. Physical adapters require their own safe-state controller and watchdog; generic process termination is not the runbook for a moving machine.

## Retention classes

| Class | Typical content | Retention policy |
|---|---|---|
| Ephemeral observation | Short raw capture ring, temporary helper input | Bounded by declared window; no retrospective full-run claim |
| Decision nucleus | Exact effect identity, policy, permit/outcome, evidence roots | Required for authority/accounting/reconciliation contract |
| Incident evidence | Authorized trace windows, artifacts, observations, tests | Explicit investigation/replay lease; minimize unrelated data |
| Research dataset | Approved, lineage-tracked traces or summaries | Separate permission, poisoning checks, held-out partition controls |
| Public capsule | Redacted reproducer and non-sensitive metadata | Export review before publication; no raw secret default |

Do not retain everything forever merely because storage is available. State the purpose and the replay grade that the retained objects can actually support. Deletion may weaken a replay promise; record that fact explicitly.

## Key and object lifecycle

Separate logical content, encryption domain, encoding profile, and physical placement identities. Use authenticated encryption and explicit nonce/AAD rules in production. Key erasure requires accounting for copies, wraps, recipients, and backups; removing one local key file is not evidence that every copy is gone.

An erasure-coded object needs authenticated reconstruction and failure-domain placement. Repair cannot restore an intentionally erased decryption key. Retention of a hash does not preserve access to deleted content and can itself leak information for low-entropy data.

## Policy and model changes

Promotion records contain the new generation, predecessor, authorized approver, compatibility/evaluation artifacts, reduced or expanded scope, and rollback plan. Shadow results do not automatically activate authority. A model or codec update invalidates incompatible cached evidence and calibration records.

A rollback is a new authorized transition to an earlier tested implementation. It must not roll back the authority epoch, permit-spending history, or lifetime statistical error allocations. Historical failures remain visible in the evaluation lineage.

## Incident procedure

1. Fence the affected authority domain and identify still-safe operations.
2. Preserve the authorized evidence closure and reconcile possibly executed actions.
3. Diagnose through independent replay/experiments, recording missing evidence and uncertainty.
4. Restore a tested profile under a new explicit activation, then add a regression case.

The order matters: a blind restart can destroy evidence and duplicate external effects. The response should not punish a helper merely for dissenting from a mistaken majority.

## Export and presentation

Reports reference exact evidence spans and distinguish observations from inferences. Render using already authorized assets without live network fetch, arbitrary local inclusion, or script execution. A sanitized report may omit necessary replay data; label its replay grade accordingly.

There is no default centralized upload of prompts, activations, or incident bundles. An optional shared corpus needs consent, tenant boundaries, lineage, poisoning quarantine, and a separate evaluation-contamination policy. The project should enable local deployment and local investigation without an external control service.
