# Serve saved stream intents under an exact recovery contract

The Linux publication example adds this explicitly selected baseline:

```
supervise_publication serve-open CONFIG ACTOR_PROFILE --stream-recovery RECOVERY_PROFILE --receipts-only
```

It uses `FileStreamActorRecoveryPort`, not the raw proposal recovery port. An
actor resends its original complete-message intent, not the current cumulative
release frame. Existing message and finish requests recover their original
outcomes even after the confirmed stream has advanced or finished. Missing keys
remain Withheld; no new message or finish is admitted. The existing wire/client
codecs and exact request keys remain unchanged.

## Independent recovery input

The required recovery document has a closed schema, with no inferred defaults:

```json
{
  "schema": "fa.stream-receipt-recovery/1",
  "stream": {
    "id": 7,
    "generation": 1,
    "max_messages": 4,
    "max_message_bytes": 1024,
    "max_stream_bytes": 4096
  },
  "registered_source": true,
  "minimum": {
    "journal_revision": 0,
    "control_sequence": 0,
    "authority_epoch": 0
  },
  "history_anchor": "/independent-custodian/stream.history"
}
```

Supply actual independently retained minima, not the illustrative zero floors.
The anchor is the original `FileHistoryAnchor` archive retained outside the
publication directory; it is sensitive supervisor data, never an actor payload.
Canonical path checks reject an authority-local anchor. Location checks do not
authenticate a custodian, protect against joint rollback or replace filesystem
isolation. No new archive format, custodian writer or automatic anchor promotion
is added by the service.

CONFIG supplies the original authority scope, effective policy, source policy
when selected, and journal limits. The document pins the entire stream profile.
`registered_source: true` requires CONFIG's exact registered-source policy;
`false` requires source-gate absence. All other baseline guard fields require
absence, including decoder/automatic-stop, identity, campaigns and credential.
The original guarded checker rejects unexpected prediction, evaluation and
mediation contracts; there is no numerical, learned, evaluated or mediated
fallback. Such deployments must continue using their matching original recovery
interfaces. First-publication guarding remains mandatory, and the exact original
terminal recovery reserve must already be present. This command does not create
or upgrade a deployment and does not accept a witness-profile or reviewer flag.
Existing raw and witness-checked receipt commands keep their previous behavior.

## Recovery and service use the original engines

One monotonic invocation budget begins before configuration ingestion. Exact
anchor, decoder/configuration and reserve checks precede original event execution.
The original guarded cursor advances one event at a time, with a deadline check
at every boundary including Ready. A cancelled/failed replay leaves canonical
and staged files untouched and issues no roles. Binding the socket after verified
replay but before the single Fence makes a collision a pre-fence refusal. The
original bound-socket custody never removes an unrelated stale path.

Successful recovery acknowledges the original Fence and discards all returned
approval roles. Expiry afterwards cannot undo that mutation. The selected stream
recovery port then uses the SAME bounded peer-credential/accept/read/write/reconnect
loop as raw receipt service. One service window and the original candidate,
connection and exchange limits apply across all peers and reconnections. There
is no second protocol parser, outcome reducer, source read, helper process,
reviewer channel, logical-clock acquisition, inference or reconciliation.

Only unsent reservations can be released by original recovery or explicit Cancel.
Already published messages remain published, unresolved messages remain charged
and outcome-unknown, and a closed connection does not finish a stream. Native
reconciliation is a separate supervisor operation after the exclusive receipt
owner has been released. Reports count transport work, not effect success.
The caller must provision the correct stream service and server identity; an
arbitrary admitting endpoint does not become non-admitting by client intent.

Initial files, one event, Fence persistence and socket operations are synchronous
and may overrun a checkpoint. These are cooperative deadlines, not preemptive
cancellation, an independent watchdog or a disk/kernel latency guarantee. No
source readiness or new approval is restored by serving an old receipt.

Regression source uses actual original owners and Unix sockets for old message
and finish recovery, missing keys, published-but-unreconciled history, exact
configuration/anchor refusals, staging preservation, every pre-fence deadline and
expiry after Fence, protected socket collisions and closed document/flag parsing.
These are authored assertions, not executed results. RCH is unavailable in the
editing runtime; compilation, rustfmt, Clippy and tests remain unverified. No
pre-existing assertions, dependencies, wire commands, journal encodings, Beads
statuses or qualification gates change. The two patches advance the reference
FA-107/FA-014 stream recovery path, not full packet or production qualification.
