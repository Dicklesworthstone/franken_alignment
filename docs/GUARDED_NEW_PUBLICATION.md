# Anchored continuation for new source-backed publications

The existing Unix publication example accepts explicit independently guarded
continuation for a new actor request:

```text
supervise_publication submit CONFIG SUBMIT_JSON --guarded RECOVERY_PROFILE
supervise_publication submit CONFIG SUBMIT_JSON --guarded RECOVERY_PROFILE --reviewer-profile REVIEWER_JSON
```

The existing optional `--credibility-activation EVIDENCE_FILE` retains its native
new-request semantics. Exact old request retries do not read that file. This is
unqualified reference source, not a production activation or remote adapter.

## Independent intent and history

The new-work profile has a separate closed schema. A stop or receipt-recovery
profile is not a request to admit new work:

```json
{
  "schema_version": "fa.guarded-submit/1",
  "minimum": {
    "journal_revision": 42,
    "control_sequence": 17,
    "authority_epoch": 2
  },
  "history_anchor": "/operator/custody/previous.anchor",
  "retain_history_anchor": "/operator/custody/next.anchor"
}
```

Numbers and paths are examples, not defaults. All three floors and the input
anchor are mandatory. Output retention is optional. Parsing reuses the original
16 KiB/depth-four/32-item/4096-byte-string limits and rejects missing, duplicate
or unknown fields, invalid integers, relative paths and unsupported schemas.
The existing bounded archive loader and exclusive mode-0600 writer are shared
with stop/receipt recovery. Input/output must be outside the authority store;
outputs must be new files, including at the actual write boundary. File and
parent-directory synchronization precede successful retention. Never discard
an old independent archive merely because a new output path exists.

CONFIG independently binds the original source, bootstrap policy, authority
scope, helper contracts and human-review policy. Version 1 selects the ordinary
source-publication guard set and exact terminal reserve. Unselected optional
stream, decoder, identity, campaign, credential, evaluated or predictive contracts
are not silently accepted. The guarded opener never falls back to an unguarded
one. The original post-recovery witness-profile compatibility check still
refuses checked publication histories before fresh intake or forwarding actor
bytes; that refusal may follow an acknowledged recovery transaction. This flag
is not a replacement for the explicit checked or learned workflows.

## Original recovery and positive continuation

After bounded configuration/document parsing, one start Instant and logical
runtime deadline cover custody admission, original replay and the existing live
workflow. Elapsed checks occur before custody admission, before guarded open,
before each original event, and distinctly at Ready. Complete exact-prefix and
suffix verification, independent guards and floors precede recovery completion.
No additional unlocked history read selects the work. The same locked cursor
finishes through the original `Fence -> Time -> Sweep` replacement, withdrawing
old eligibility and keys before returning the owner and separate reviewer.

This new-work command refuses further admission if either the original sweep
or retained ledger still reports outstanding deliveries. A previously
irrecoverable delivery omitted from a new sweep cannot disappear from that
check. Confirmed effects remain charged; unknown effects do not become refunds.
Use the receipt-only command to inspect existing outcomes without new-work
intent. Time and Sweep remain ordinary-capacity operations, not consumers of
the emergency stop reserve; recovery does not enlarge any allowance.

Both guarded and legacy submissions now use one shared continuation body.
A genuinely missing original request needs a fresh policy-source read, the
original helper congress and a fresh human key before publication. The optional
peer profile remains subject to its existing host and transport checks. An
existing key remains query-only: exact bytes recover its original ticket/result,
while a conflict remains an idempotency conflict. No admission snapshot is
installed for an existing key, and no evidence, helpers or human offer is used
for its retry. The caller must supply the real post-recovery policy epoch and
current target version for new work. The command never rewrites either field,
the payload, units or original deadline to make a proposal pass.

## Acknowledgment and failure handling

On ordinary returned success/error paths, archive retention runs exactly once
while the original acknowledged owner is held, before child cleanup and output. It runs on a healthy post-recovery
preparation failure too, so a missing current source can leave a retainable
withdrawal/recovery cut rather than losing all successor evidence. A poisoned
owner cannot export its old memory state. Requested retention checks continuity
against the original input anchor and never reopens a later owner or disk image.

Source, review or publication failures preserve the existing stop/drain and
helper-cleanup behavior. A late archive-path collision cannot erase an executed
actor result or overwrite the competing file. Workflow/retention/output errors
are combined. Stdout contains only the original actor wire response, not private
history or paths. Only confirmed execution with no retained failures and no
unreaped children exits successfully. Failure to print or retain a response does
not undo execution or authorize a resend. Retry the same exact request with a
new output path; a live recovery has its real three-event/clock cost, unlike a
completed stopped retry. Process termination or an unwinding callback can leave
no new archive; retain the old independent anchor until a new export succeeds. Failure before the new request is actually forwarded
leaves it unadmitted; a corrected proposal then needs the next real epoch.

This is one original local journal-as-publication sink. File I/O, guard checks,
individual reducer operations, helper steps and synchronization are synchronous;
cooperative checkpoints are not hard latency bounds. Trusted clocks, helper
provenance, peer authentication, protected configuration/custody directories,
advisory-lock cooperation and independent anti-rollback remain assumptions.
No added runtime, dependency, journal format or actor verb is introduced.

## Regression sources and qualification

Eleven new regression functions reuse the original continuation fixture and its
real helper-process, reviewer-socket, actor-wire and native journal pathways.
They cover successive new publications with fresh keys and exact retained
budgets; expired retries and byte conflicts without live assets; missing fresh
source plus corrected publication; unmodified stale epochs; cancellation at
every replay boundary; independent source/floor/anchor refusal; late export
collisions; strict schema/flag separation and missing stores; repeated
irrecoverable liabilities; actual command publication/receipt retry; and combined
export/output failure. Original continuation and qualification tests are retained.
Synthetic helper ballots establish enforcement cases, not statistical calibration.

Rust compilation and these tests have not executed in this environment. The
required RCH target and unchanged xtask gate are attempted without a local
fallback; a missing launcher is not a pass. Exact upstream blob identity,
selected-file patch application and whitespace checks do not establish type
correctness, runtime safety or release qualification. No Beads acceptance or
full FA-014 prerequisite is closed. This scoped integration serves plan 8,
16.4/16.5/16.9 and 17.1, and FI-A02, on the original implementation boundaries.
