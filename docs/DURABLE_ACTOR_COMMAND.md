# Runnable durable actor submission and recovery

The Linux `supervise_publication actor-submit` command can now use the original
`FilePeerClient` and its bounded write-ahead transport instead of keeping all
request identity in the short-lived command process. This is an opt-in raw-submit
client of the existing live actor service, not a new server, effect adapter,
review mechanism or authority ledger. Existing one-shot and sequence syntax is
unchanged.

```
cargo run --locked -p fa-reference --example supervise_publication -- \
  actor-submit ACTOR_PROFILE --durable CLIENT_PROFILE create SUBMIT_JSON

cargo run --locked -p fa-reference --example supervise_publication -- \
  actor-submit ACTOR_PROFILE --durable CLIENT_PROFILE submit MIN_REV SUBMIT_JSON

cargo run --locked -p fa-reference --example supervise_publication -- \
  actor-submit ACTOR_PROFILE --durable CLIENT_PROFILE resume MIN_REV

cargo run --locked -p fa-reference --example supervise_publication -- \
  actor-submit ACTOR_PROFILE --durable CLIENT_PROFILE inspect MIN_REV
```

`create` exclusively creates a new private client directory and never falls back
to opening an existing one. `submit` uses an existing directory and requires an
independently retained minimum revision. Both require the original bounded Submit
document to match the selected actor-profile request before creating/opening the
client. `resume` takes **no document**: it explicitly retries that request's exact
saved submission to reacquire the original server ticket. An absent key refuses
before connection. Target, payload, policy epoch and deadline are never edited,
and a restart never refreshes the lifetime request/exchange/I/O allowance. The
same client can later accept another explicitly selected request: request numbers
are not baked into the server identity.

The original actor profile still supplies scope, socket, expected supervisor
UID/GID/optional PID, exchange limit and invocation/poll timing. The independent
client profile has a closed schema with no defaults:

```json
{
  "schema": "fa.durable-actor-client/1",
  "directory": "/private/actor-client-state",
  "server_identity": "independently-provisioned-server-domain-and-generation",
  "requests": 16,
  "payload_bytes": 1048576,
  "io_calls": 65536,
  "write_bytes": 67108864,
  "read_bytes": 67108864,
  "image_bytes": 16777216,
  "drive_steps": 16
}
```

The full scope, named clock domain, raw-submit profile, socket path and independent
server identity are length-framed into the original recovery binding. The original
peer wrapper adds the exact supervisor credential policy. All native constructors
still validate limits. A changed audience, service identity, socket, peer policy
or lifetime limit refuses recovery before cleanup. Drive quantum and per-invocation
monotonic timing are scheduling choices, not renewed lifetime allowances. The
client directory must be separate from every server publication directory; this
actor-only command has no private server profile from which to discover those.

A live invocation verifies the protected socket directory and actual connected
peer before protocol work. It uses the original durable preparation and drive
operations: acknowledge exact intent before I/O, acknowledge results before
returning them, and retain conservative reservations after ambiguous interruptions.
It polls only Pending and OutcomeUnknown responses, on the same connection and
under the same finite invocation/I/O budgets. ControllerUnavailable, Withheld,
stale/absent responses, errors and timeouts do not cause automatic resubmission,
reconnection, cancellation, a new key, or a different endpoint. Endpoint execution
and liability accounting remain entirely in the original server.

A monotonic budget starts before reading the profiles and is checked before
storage/connection work, preparation, each batch and every subsequent poll. Initial
file I/O, Unix connect and one native batch are synchronous and may overrun it;
this is cooperative scheduling, not preemption or a bound on disk/kernel latency.
A terminal response received and durably recorded within an entered batch remains
reportable even if that batch took longer than expected. Each new explicit command
has its own invocation budget, but the stored lifetime I/O budget and original
request deadline are not renewed.

`inspect` needs only the two public configuration files and canonical client image.
It neither checks the live socket directory nor acquires a writer, removes staged
bytes, connects, polls or reads a supervisor/evidence file. It works beside a locked
or faulted client. Reports are tagged `historical`, retain the original nested
wire Knowledge response, and distinguish an unrecorded response from Known
nonexecution. Accounted work and pending reservations are reported separately;
accounted work may include conservative expenditure retained from an earlier crash.
No request payload is printed. Output failure does not erase the saved response or
send another command. A canonical result newer than the last acknowledgment is
not rewritten merely because output or completion acknowledgment was lost.

Six new regression functions use original Unix transports, client stores and the
native full-input congress/two-key publication path. They cover three drive quanta,
OutcomeUnknown until reconciliation, actual command timeout and document-free
resume after server execution, offline inspection beside a lock and staging file,
output failure, exact domain/limit recovery, command/JSON preflight and deadlines
before preparation versus before first I/O. Their source does not establish model
calibration, network authentication, executable attestation, equal-counter fork
authenticity or real-world exactly-once effects. No existing assertions change.

The required remote-only targeted test and unchanged `xtask` gate are attempted,
but `rch` is unavailable in the editing runtime. Compilation, rustfmt, Clippy and
Rust test execution remain unverified. No dependency, server/client journal format,
wire verb, Beads status or production qualification gate is changed.
