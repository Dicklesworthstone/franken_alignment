# Checked publication witnesses with native Andon accounting

This source increment composes the existing plain publication-witness gate with
native evaluation/Andon accounting in one guarded owner. It does not add another
publication validator, evaluator, authority ledger, or journal format.

The checked Andon constructors publish the plain publication validation profile,
recovery reserve, evaluator protocol and native accounting configuration in the
same first canonical image. Recovery pins the independently retained publication
limits alongside the exact history anchor, native window inventory, evaluation
contract, guards and recovery floors. Omitting or changing those limits refuses
before historical execution or role release.

The runnable supervisor accepts the checked forms:

```sh
supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE \
  --andon ANDON_PROFILE --retain-history-anchor NEW_ANCHOR

supervise_publication submit-checked CONFIG SUBMIT_JSON WITNESS_PROFILE \
  --andon ANDON_PROFILE --guarded SUBMIT_RECOVERY_PROFILE
```

The existing publication profile remains authoritative for witness acquisition
and revalidation during live execution. Native accounting only records the
already-applied review/evaluation cases; it cannot replace publication evidence
or human approval.

This first composition deliberately accepts only the plain publication-witness
profile. Feed-backed, snapshot-fallback and joint-publication history modes refuse
through `simple_limits()` rather than silently discarding their stronger
freshness/history contracts. Native joint credibility may still be selected in
the Andon profile; that is a different committee-reweighting safeguard.

Four authored regressions cover coupled first-image creation/recovery, omission
and drift of the publication limits, refusal of a stronger history profile, and
closed checked-command grammar. Existing native publication regression bodies are
preserved except for the new explicit optional publication field in their helper.

Runtime qualification remains pending the repository's required RCH gate.
Source construction and Git-object checks are not compilation or execution
evidence.

## Scoped execution update (2026-10-24)

The original stronger-history fixture omitted the required feed for its waiting
schema and failed before reaching `simple_limits()`. It now decodes a complete
waiting profile, then requires the specific no-downgrade refusal, with its feed
still absent and no store created. All four original functions, including plain
checked creation/recovery and changed/omitted-limit refusals, passed within the
38-function combined run at `9a8cf8f0` on the qualified standard compiler and
default stack. See `IMPLEMENTATION_STATUS.md` for exact source and denominators.
The required canonical driver still refuses dependency admission; this scoped
result does not qualify a packet, release or stronger-profile composition.
