# Guarded clock-independent stop and explicit stopped drain

FA-014; plan 16.4, 16.5 and 25.4; serves FI-A02. The existing
`supervise_publication` executable now connects the original guarded terminal
phases to independent source-publication recovery requirements:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  stop-admission /operator/config.json 900 --guarded /operator/stop.json
cargo run --locked -p fa-reference --example supervise_publication -- \
  drain-stopped /operator/config.json 900 --guarded /operator/drain.json
```

## Selection and custody

The command accepts exactly CONFIG, OPERATION and one `--guarded PROFILE` pair.
Unknown, duplicate, mixed and missing options refuse before configuration intake.
An output-only `--retain-history-anchor` option cannot be combined with guarded
selection: use the existing profile's `retain_history_anchor` field instead.
Ordinary no-guard invocations and combined `recover-stop --guarded` remain
available with their previous native behavior. A selected guarded failure never
falls back to ordinary recovery.

Both existing `fa.guarded-stop/1` and `/2` profiles use the SAME private custody
admission, effective policy, complete source guard inventory, external floors,
independently retained anchor and terminal reserve as combined guarded recovery.
Version two additionally binds the complete initial/current topology and its
availability. The executable forwards only to its selected sealed original
cursor. There is no intermediate owner, second replay or alternate finalizer.
Learned reconstruction, prediction and evaluation remain separately selected
APIs; this route does not infer or drop their requirements.

Mediated admission uses requirements matching the pre-stop cut. Its subsequent
drain must independently describe the stopped topology availability, normally
`available:false`. Reusing an earlier permitting profile refuses. Changing the
availability declaration does not authenticate it: the original native checker
still compares the complete requirements with the replayed history.

## Distinct operations and acknowledgments

Admission performs the original Stop/Fence without acquiring logical time.
It uses the unchanged `fa.admission-stop/1` report, including actual fence state,
original liabilities and `drain_attempted:false`. Success establishes stopped
admission, not settlement of unknown effects. A matching existing stop is only
confirmed, with no new event or clock call.

Drain requires the existing matching operation. It invokes the original
one-event StopProgress only if needed, using a fallible current clock after
complete replay and canonical-history comparison. Already-drained receipts are
clock-free and append nothing, including at the journal ceiling. A new admission
stop followed by a needed drain uses the original two-plus-one event reserve.
The original drain renderer still returns failure for unresolved liabilities;
no new outcome, refund, publication retry or recovery fence is manufactured.

One monotonic elapsed budget begins in the actual command before CONFIG and
PROFILE intake. The same budget is checked through custody/topology ingestion,
every original replay event and Ready, and on both sides of a required drain
clock call. A slow clock cannot provide an eligible tick after expiry. A failed
or panicking clock cannot undo the earlier acknowledged admission stop.

After native acknowledgment, only the exact returned history anchor is eligible
for the existing create-new file/parent-synchronized retention. Native work,
archive retention and stdout remain separate acknowledgment boundaries. A late
output collision does not hide native results; retention and reporting failures
are combined. Lost stdout does not remove a retained archive or retry an effect.
The previous archive is never overwritten. Original filesystem-isolation and
independent-custodian assumptions remain; this is not an atomic multi-file
transaction or a hard-real-time shutdown guarantee.

## Verification status

Eight new regression functions use actual native file owners, original topology
parsing/certification and the real executable dispatcher. Coverage includes both
profile versions, source/clock-free stop, stopped topology admission, exact
three-event reserves and no-write retries, source/floor/topology mismatches,
rollback against an independently retained stopped prefix, foreign operations,
every elapsed checkpoint, failed/panicking/slow clocks, late archive collisions,
lost stdout, closed options and missing or malformed custody with positive
controls. Existing assertions and native reducers are unchanged.

The tests are authored, not executed. Preparation used an exact-hash selected-file
baseline, not a full checkout. Required RCH example tests and the unchanged xtask
gate cannot start in the editing environment because RCH and the Rust tools are
absent. Patch/source checks do not qualify compilation, formatting, Clippy or
runtime behavior. FA-014 and its full prerequisites remain open; no production
gate is activated by this source increment.
