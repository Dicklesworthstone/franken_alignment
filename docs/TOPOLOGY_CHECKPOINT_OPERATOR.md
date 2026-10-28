# Operator retention of topology work and history

The existing topology-supervised `create` and `submit --guarded` commands now
have explicit version-2 topology profiles. They capture the actual selected
source's spent checks and read allowance together with acknowledged journal
history, instead of requiring a manually transcribed `source` counter object
for every subsequent publication. Version-1 profiles keep their existing meaning.
The concurrently added plain-witness `create-checked` topology bootstrap is
preserved and can also retain a checkpoint; the continuation command described
here still requires its original plain source-publication profile, not checked
witness recovery through a weaker gate.
The native source, journal, recovery, helper, human-review and publication gates
remain the authorities; this is their operator-facing composition.

## Initial creation

Use the existing explicit-archive command, with a version-2 topology selector:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- create CONFIG.json SUBMIT.json \
  --retain-history-anchor /operator/custody/history-0.anchor \
  --topology-profile /operator/custody/create-topology.json
```

```json
{
  "schema_version": "fa.mediated-create/2",
  "topology_path": "/operator/live/topology.json",
  "inventory_path": "/operator/live/inventory.json",
  "retain_checkpoint": "/operator/custody/source-0.bin"
}
```

The first canonical image and all initial/final checks are still the existing
[topology creation](TOPOLOGY_SUPERVISED_CREATION.md) workflow. The new selector
requires a fresh checkpoint output path. Before storage creation, admission
requires that its parent exists and resolves outside the authority store; paths
must be absolute without parent traversal. Existing output is never overwritten.
Keep the ordinary history archive and source checkpoint at distinct paths.

The bounded topology/inventory bytes loaded at preparation are retained until
completion. Capture compares them against the original source's entire expected
input, including unselected inventory metadata. It does NOT reread live files
at export and accidentally turn source loss/substitution into a new expectation.
An acknowledged withdrawal can therefore export its spent work and unavailable
coverage using the original documents. An uncertain/poisoned owner cannot export
an allegedly acknowledged checkpoint.

## Subsequent publication

Use the existing `submit CONFIG SUBMIT_JSON --guarded RECOVERY_PROFILE
--topology-profile TOPOLOGY_PROFILE` command with:

```json
{
  "schema_version": "fa.mediated-submit/2",
  "initial": {
    "topology_path": "/operator/custody/initial-topology.json",
    "inventory_path": "/operator/custody/initial-inventory.json"
  },
  "source_checkpoint": "/operator/custody/source-0.bin",
  "next": {
    "topology_path": "/operator/live/topology.json",
    "inventory_path": "/operator/live/inventory.json"
  },
  "operation": 3,
  "retain_checkpoint": "/operator/custody/source-1.bin"
}
```

`operation` is an explicit fresh source-rebind ID, greater than the retained
source floor and leaving IDs for the initial/final checks. The example assumes
the previous last operation is 2; do not copy it over a different history.
The next graph AND inventory generations must advance and source/checker limits
must remain identical. The source checkpoint supplies actual spent work, current
input and historical availability. Manual `source`, `current` or `available`
overrides are rejected by this version's closed schema.

The ordinary guarded recovery profile remains independently required. Its
`history_anchor` must identify exactly the SAME cut as the source checkpoint's
nested anchor. Its explicit control/authority/revision floors are strengthened
by the checkpoint, never replaced by smaller defaults. Its optional
`retain_history_anchor` should name a new output such as `history-1.anchor` so
both resulting archives can be used together for the next invocation.

This operator mode requires the canonical event count to match the saved source
checkpoint before replay. A valid but later journal can contain unretained
source work; an older checkpoint does not price that work. It refuses rather
than assuming zero extra expenditure. The separate native checkpoint recovery
helper has its own more selective suffix checks; this command deliberately keeps
the stricter exact-cut rule. Missing, corrupt, mixed-cut and stale input refuse
without falling back to version 1 or guessing counters from the journal.

After original mediated replay, the original Fence/Time/Sweep must acknowledge
and all prior liabilities must resolve before new work. The same native source
resumption, policy capture, initial certification, post-read clock, helper
congress, separate human approval, two-key dispatch, final topology refresh and
publication run as before. Capture/export occurs under that same owner/source,
before child cleanup releases custody. Every actually spent check remains spent,
including initial checks followed by human rejection. No actor field is rewritten.

For the generic `submit --guarded` command, an existing request remains
receipt-only: no next live input, helper, new human approval, source check or
publication is requested. Omit `retain_checkpoint` in that mode; it cannot claim
to capture a newly active source. The original history export can still occur.
Because recovery advances the journal, an older source checkpoint cannot be
reused indefinitely through that strict operator path. The learned explicit
receipt-settlement command has the separate paired-custody path below.

## Learned terminal recovery

The learned operator can retain a source checkpoint after offline
`--stop-admission`, `--drain-stopped` or combined `--recover-stop` with the
explicit `--retain-topology-checkpoint NEW_OUTPUT` option. This requires a
version-2 source checkpoint and paired `--retain-history-anchor NEW_OUTPUT`.
The native consumer validates an exact terminal successor of the saved cut,
preserves all original documents and source expenditure, and pairs them with
the acknowledged terminal anchor and floors. It does not create a live source
or read the profile's next files. The successor pair can be selected by a new
version-2 profile for later drain or retry. See the full
[terminal workflow](LEARNED_STOP_RECOVERY.md#separate-operator-phases).

The profile field `retain_checkpoint` remains a live-source output and is
rejected by historical recovery. The explicit `--retain-topology-checkpoint`
flag instead selects acknowledged custody in the learned maintenance modes that
support it. Version-1 manual minima cannot produce an exact source checkpoint.
Both terminal outputs must be new, distinct and outside the authority store
before any stop mutation occurs. Credential maintenance has its own
[paired-custody workflow](LEARNED_CREDENTIAL_LIFECYCLE.md).

If an acknowledged terminal write outlives one or both exports, explicitly add
`--recover-terminal-checkpoint` while selecting the original version-2 source
checkpoint and its matching original anchor. The separate terminal-only cursor
can recover across the same stop's Stop/Fence/StopProgress suffix, preserve
exact spent work, and retain a new pair. It rejects ordinary/source successors,
changed stop identity and mixed cuts, and exposes no live recovery operation.
The original exact-cut rule remains in force without this flag and for all
live and receipt workflows. See
[interrupted-export recovery](LEARNED_STOP_RECOVERY.md#recover-after-interrupted-checkpoint-export).

## Learned receipt reconciliation

The one-shot learned `--resume --reconcile-receipts` path can retain its exact
acknowledged source/history pair with `--retain-history-anchor NEW_OUTPUT` and
`--retain-topology-checkpoint NEW_OUTPUT`. Select a v2 `--topology-profile`
without `retain_checkpoint`, plus an independent `--history-anchor` matching
the checkpoint's nested anchor. Both outputs are required when retaining
topology custody. Passive `--receipts-only` rejects both output flags.

Admission checks the new, distinct outside-store paths before replay. The native
finalizer checks the complete selected source cut before its trusted clock or
Fence. After the original Fence/Time/Sweep acknowledges, the successor preserves
source documents, spent checks, read expenditure and last operation, while
recording the acknowledged floors and unavailable topology. It creates no live
source and does not read the profile's next files.

Both exports are attempted before a late deadline can withhold the response or
stdout can fail; neither export suppresses the other. Use the resulting pair
in a new v2 profile for subsequent exact recovery. No-output reconciliation
retains its earlier behavior and leaves the old v2 cut stale. Receipt settlement
has its own explicit `--recover-receipt-checkpoint` option; the terminal repair
flag does not apply. Select the original matching v2/source-history pair and
both new outputs with `--resume --reconcile-receipts`. The restricted original
cursor accepts only that unchanged cut or exactly one Fence/Time/Sweep successor.
An existing successor is fully replayed and confirmed with real synchronization
and cleanup, without another clock, event, replacement or source read. Partial,
extra or unrelated suffixes refuse. Ordinary exact-cut recovery remains strict.

See [retained topology receipt custody](LEARNED_RECEIPT_RECONCILIATION.md#retained-topology-receipt-custody)
for the paired-output command and next passive inspection, and
[explicit receipt repair](LEARNED_RECEIPT_RECONCILIATION.md#explicit-operator-repair-after-interrupted-exports)
for recovery after a lost export.

## Failure and cost boundaries

Each export uses private create-new mode, then file and parent synchronization.
The standalone history archive and source checkpoint are TWO independent writes,
not one atomic multi-file commit and not a transaction with publication. Both are
attempted, both failures are retained, and neither successful output implies the
other succeeded. A racing output or failed report cannot undo publication, erase
its charge, overwrite earlier custody, or trigger a resend. Keep the prior
archives until their successors are durably acknowledged. No source checkpoint
is produced when recovery/preparation cannot establish the required healthy cut.

For learned services, an acknowledged refusal or Stop can still leave a healthy
native owner with source expenditure to retain. A service error therefore does
not suppress its selected source output. The archive records the actual native
availability, including withdrawal after an uncertified cut or missing input;
it is neither successful publication nor fresh topology certification. The
original failure is preserved, checkpoint errors are appended and the separate
anchor export is attempted independently. A poisoned owner refuses capture
before either archive is created.

The creation envelope remains 16 KiB/depth 2; its item ceiling is 12 to admit the
four required fields. The continuation envelope remains 16 KiB/depth 4/64 items.
The native checkpoint keeps the original per-document and nested history limits;
see [the transport contract](TOPOLOGY_WORK_CHECKPOINT.md). Retained input bytes,
checkpoint parsing and synchronous journal encoding/copies are additional bounded
work. Export adds file and directory synchronization per checkpoint. This is not
a latency bound, memory optimization or throughput claim.

Independent custody is still a trust boundary, not authentication. Joint rollback,
forged archives and unretained process-local work after the last checkpoint are
not solved. The mode is not a crash-atomic global quota, continuous topology
watcher, mandatory-source deployment flag, or operating-system containment proof.
The configured operator remains responsible for complete recovery custody.

## Verification status

Eight regression functions plus one synthetic helper entry point are authored in
`workflow/continuation/checkpoint/tests.rs`. They use the actual top-level command,
helper processes and separate reviewer sockets to cover creation and two later
publications, exact paired archives, cumulative charges/work, receipt-only access
without live assets, final source loss, export races, exact exhaustion, forbidden
counter overrides, mixed history, newer canonical history, pre-creation refusal
and human rejection. Synthetic ballots are plumbing controls, not model-quality
or reviewer-authentication evidence. Existing test assertions are unchanged.

The initial remote-only attempt stopped before compilation because `rch` was
absent. On 2026-10-23 all eight regressions and their helper entry point pass
with the qualified nightly's official Cranelift backend in a disposable scoped
harness. Only unrelated libtest registration is changed; the original test
bodies, assertions, fixtures and production logic remain intact. Four new
learned terminal-checkpoint regressions also pass, including paired exports,
all predictor modes, output races and report failure. The complete unmodified
operator test source passes the standard compiler check. Full-harness code
generation hit the host memory limit, while the full standard xtask gate still
refuses the existing dependency graph. RCH, rustfmt, Clippy and release
qualification remain pending; no Beads or roadmap item is closed.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  --example supervise_publication workflow::continuation::checkpoint
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
