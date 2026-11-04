# Select new recovery work without reducing the fleet

FA-014, plan 16.4/16.5/25.4. Full-roster `recover` spends a new durable visit on
every domain. `resume` passively inspects eligible saved visits, including those
that are stopped but undrained. Neither can explicitly drain one such member
while refreshing the other members' receipts at a nearly exhausted visit budget.

The original coordinator now provides `recover_selected_pass_with_clocks`,
`stop_selected_admission`, and `drain_selected_stopped_with_clocks`. Each takes
the COMPLETE independently registered bindings and a nonempty list of domains
selected for new work. Only those domains enter the original combined recovery,
clock-free admission stop, or stopped-only drain. The selection is not another
roster, runtime, journal, or authority object.

Every unselected member must have an eligible LATEST retained visit. It is
passively resolved during this pass, not accepted from a saved success flag. A
latest refusal, non-recovery visit or absent visit refuses the selection before
any callback or mutation; name that member explicitly to request new work.
Missing, changed or unstopped canonical evidence found during passive resolution
remains EvidenceRefused. It never falls through into a domain command. Selected
native refusals can leave other reachable members usable, but all members remain
in the result and in the stopped/drained denominator.

Selection and binding order are immaterial. Unknown/duplicate domains, incomplete
or foreign bindings, empty selection and invalid replay quanta refuse. Original
whole-pass capacity admission counts a new durable visit only for selected work,
one completion revision for a pending passive resolution, and no new revision
for an exact saved observation. EVERY member still consumes its original bounded
session attempt. This does not replenish either allowance or bypass completion
capacity, canonical custody, retained-prefix or independent-anchor checks.

Clock acquisition occurs only where the original selected terminal operation
needs it. The original checkpoint loop, cancellation, caught-unwind quarantine,
first unacknowledged-write stop and explicitly unvisited tail remain unchanged.
An explicit selection of a pending intent requests new recovery; it is not proof
that its earlier operation failed. Native exact-stop handling still determines
whether any record or clock is needed. No path resends a publication, imports a
role, refunds an unknown effect, or silently widens the caller's selection.

Eight regression functions cover original two-key executed and expired effects,
selective drain at the durable-visit ceiling versus an all-member capacity refusal,
combined/admission controls, complete admission, latest-refusal dominance,
missing and unstopped unselected receipts, every fixture cancellation checkpoint,
all five intent barriers, and a clock unwind followed by passive-only resume.
Existing tests and assertions are unchanged. The implementation shares the old
planner and the exact original execution loop; legacy APIs retain their selection.

These tests are authored, not executed. The editing environment lacks RCH and
Rust tools; exact-revision remote tests and the unchanged xtask gate remain
required. No production qualification or full FA-014 closure is claimed. Trusted
filesystem isolation, clock provenance, independent custody and actual roster
completeness remain external assumptions. Storage and single-event replay remain
synchronous, not hard-real-time or a simultaneous fleet-stop guarantee.

## Runnable selection

`shutdown_fleet recover PROFILE --recover-domain ID` selects this path. Repeat
the option for multiple members. Combine it with `--admission-only` or
`--drain-only` to select those original operations; otherwise it uses combined
recovery with per-domain terminal clocks, also explicitly selectable with
`--terminal-clocks`. The full original profile is still mandatory. For example:

```sh
cargo run --locked -p fa-reference --example shutdown_fleet -- recover /operator/fleet.json --drain-only --recover-domain 2
```

Selection is rejected with `create` or `resume`. Unknown/duplicate/missing IDs,
conflicting phases, unknown flags and selection flags placed in custody-value
slots refuse. IDs are checked against the prepared original plan before opening
the coordinator; complete latest-visit and remaining-capacity checks then run
through the native planner before any member command. No rejected selection
falls back to ordinary all-member recovery or an automatically enlarged budget.

Both existing roster formats and independent coordinator-anchor options use the
unchanged preparation and custody path. The existing cumulative elapsed budget
covers configuration, full-roster reads, selected terminal work and retention.
Clock failure, expiration and output/retention failure preserve their original
acknowledgment boundaries. Operators use ordinary passive resume after lost
output, not an automatic repeat of the selection.

The explicit route emits `fa.shutdown-fleet-result/4`, naming the requested phase
and sorted `selected_domains`. This field states requested new work, NOT that
it ran or succeeded. Every member still carries the original result, liabilities,
optional acquired tick and fence observation. An unselected missing receipt
keeps fleet success false even if selected settlement succeeds. Existing commands
without selection retain schemas one, two and three and their previous behavior.

Seven additional operator regression functions cover actual flag routing,
selective drain at the visit ceiling, strict options and custody-value slots,
unknown IDs before staged-file cleanup, missing unselected receipts, failed/slow
clocks, active-drain and combined controls, and retained independent custody
across lost stdout. Together with the native increment, fifteen regression
functions are authored; none is claimed executed in this editing environment.
