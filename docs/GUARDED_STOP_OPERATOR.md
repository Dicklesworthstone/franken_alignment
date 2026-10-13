# Guarded offline stop for ordinary source-backed publication

The existing Unix reference example accepts an explicit guarded recovery profile:

```text
supervise_publication recover-stop CONFIG OPERATION --guarded RECOVERY_PROFILE
```

This connects the original source-publication operator to
[direct cooperative guarded stopped recovery](GUARDED_STOP_RECOVERY.md). The
three-argument legacy `recover-stop CONFIG OPERATION` remains unchanged. The
new route never retries a failed guarded open through that legacy route.

## Independently supplied input

RECOVERY_PROFILE is a bounded JSON file, separate from the journal. For example:

```json
{
  "schema_version": "fa.guarded-stop/1",
  "minimum": {
    "journal_revision": 42,
    "control_sequence": 17,
    "authority_epoch": 2
  },
  "history_anchor": "/operator/custody/acknowledged.anchor",
  "retain_history_anchor": "/operator/custody/stopped.anchor"
}
```

The numbers are examples, not recommended defaults. All three recovery floors
and an independently retained input anchor are mandatory. Values of zero are
allowed only when explicitly supplied, never filled in from disk or omitted
fields. Retention output is optional. The parser rejects unknown/duplicate
fields, missing values, negative/fractional/overflowing numbers, relative paths,
an unsupported schema and oversized/deep input. The profile is limited to
16 KiB, 32 JSON items, depth four and 4096-byte strings.

CONFIG supplies the exact original authority scope, bootstrap policy, committee,
human-review policy and source identity. Version 1 selects exactly the original
ordinary source-publication guard set: mandatory publication revalidation and
the configured policy source, with no stream, decoder/stop policy, identity,
policy-campaign or credential guard. The original checker requires absence for
these unselected optional guards. Evaluated, predictive, mediated and learned
profiles keep their separate recovery contracts. This command is not a generic
loader that guesses a guard inventory from the journal. A changed effective
policy, source contract, independently retained floor or anchor fails closed.

## Execution and cancellation

Path/anchor admission does not create an authority store. The input and any new
output must be outside the canonical authority directory; aliases are resolved
before checking this boundary. Input uses the original bounded regular-file
loader and anchor decoder. Output must not exist, including as a dangling link;
exclusive mode-0600 file creation is repeated at export, then the file and its
parent directory are synchronized. A failed partial export is never overwritten
or silently treated as acknowledged custody.

One monotonic elapsed budget starts before custody admission. It is checked
before admission, before guarded open, before each original replay event and at
Ready. The original cursor runs one event at a time, holding the same exclusive
Store lock. Cancellation drops that cursor without selecting terminal time,
cleaning staging, appending a fence or issuing a role. No separate unlocked
`read_publication` pass is used to select the stop request. The verified cursor
selects the exact predecessor itself and consumes the original terminal
transaction directly, with the exact original recovery reserve.

Initial file/anchor decoding, one original event, guard validation, final
encoding and sync remain synchronous operations. These cooperative checkpoints
are not an interruptible-I/O or hard cancellation-latency guarantee.

## Acknowledgment and liabilities

A successful native stop is independent of archive/report output. The command
exports only its captured acknowledged anchor, never an owner reopened afterward
or a later disk image. It always attempts the original native stop JSON report
after export; an export error is combined with any report error or pending-drain
error. Stdout has no private anchor bytes, helper inputs or archive path. A
`stopped_drained` report alone does not acknowledge requested archive retention:
the command result must also succeed.

A drained retry with a new output path retains the identical anchor and does not
call the terminal clock, append a record or replace the canonical journal.
Original confirmed effects remain executed and charged. Expired unknown effects
remain pending/irrecoverable and charged, including in the exported successor.
A pending retry still needs fresh time and finite terminal capacity. No helper,
reviewer socket, current policy evidence, original submit document or credibility
capsule is read to complete this operation; configuration parsing does not start
its configured programs. No effect is resent.

Independent custodian authentication, the protection of CONFIG/RECOVERY_PROFILE,
advisory-lock cooperation and nonrollback of both journal and archive remain
operator assumptions. This is one original local reference publication sink,
not an OS-process halt, remote-provider settlement or production release claim.

## Qualification

The implementation and its regression sources are unqualified. Required target
and unchanged xtask-gate attempts stop before Cargo because `rch` is absent in
this environment. Rust compilation, tests, formatting qualification and Clippy
have not executed. FA-014 and all broader prerequisites remain open.

The new tests cover bounded parsing, absent live assets, actual command dispatch,
all elapsed replay cuts, independent source/floor/anchor refusal, custody-path
refusal, late export collisions, report/export errors, exact drained retries,
and preservation of confirmed/expired publication charges through the existing
native fixture workflow. Original legacy recovery and learned tests are unchanged.
This is scoped implementation of plan 16.4/16.5/16.9/25.4 and FI-A02, with no new
runtime, dependency, authority journal or agent-facing verb semantics.
