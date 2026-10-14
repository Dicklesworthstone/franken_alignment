# Durable actor commands with an explicit stream-intent contract

The existing Linux `actor-submit --durable` command gains an explicitly selected
stream-intent client profile. It uses the original `FilePeerClient`, write-ahead
batches, message-intent encoder/decoder and server `Knowledge` responses. It does
not add an effect adapter, a stream parser, a request allocator, a live-stream
server, or a protocol command.

## Configuration and compatibility

Version one (`fa.durable-actor-client/1`) keeps its original raw-submit binding
and accepted inputs. Version two requires the complete immutable stream contract:

```json
{
  "schema": "fa.durable-actor-client/2",
  "stream": {
    "id": 7,
    "generation": 1,
    "max_messages": 4,
    "max_message_bytes": 1024,
    "max_stream_bytes": 4096
  },
  "directory": "/private/stream-client-state",
  "server_identity": "independently-provisioned-stream-domain",
  "requests": 16,
  "payload_bytes": 1048576,
  "io_calls": 65536,
  "write_bytes": 67108864,
  "read_bytes": 67108864,
  "image_bytes": 16777216,
  "drive_steps": 16
}
```

The original actor-service profile supplies the scope, socket, expected server
credentials, exchange allowance and invocation timing. The version-two client
binding domain-separates stream intent from raw payload and includes all five
stream fields, the original full scope, clock domain, socket and independently
provisioned server identity. The existing peer wrapper binds UID/GID/optional
PID. A missing/partial stream contract, unknown field or unsupported schema
refuses instead of selecting raw mode. A version-one document cannot silently
acquire a stream field.

Changing the stream identity, generation or any limit cannot reopen the old
client store. Changing to the old raw profile also refuses. No migration or
rewriting of saved request bytes is performed. Use a separate, explicitly
created client domain instead of relabelling an old raw archive. Request selection
and batch size remain scheduling choices, not new domains or renewed allowances.
The original client image and server journal formats remain unchanged.

## Commands and original intent bytes

```
supervise_publication actor-submit ACTOR_PROFILE --durable STREAM_CLIENT_PROFILE create INTENT_SUBMIT_JSON
supervise_publication actor-submit ACTOR_PROFILE --durable STREAM_CLIENT_PROFILE submit MIN_REV INTENT_SUBMIT_JSON
supervise_publication actor-submit ACTOR_PROFILE --durable STREAM_CLIENT_PROFILE resume MIN_REV
supervise_publication actor-submit ACTOR_PROFILE --durable STREAM_CLIENT_PROFILE cancel MIN_REV
supervise_publication actor-submit ACTOR_PROFILE --durable STREAM_CLIENT_PROFILE inspect MIN_REV
```

The document is the existing `Command::Submit` encoding of
`encode_stream_proposal(stream_profile, &FileStreamProposal { ... })`. The actor
must still supply the original exact target, epoch and deadline. It supplies only
the new complete message or explicit finish, never a cumulative release frame.
The server derives cumulative review context and its full-frame effect charge.
Transport units remain exactly the intent's bytes, not the effect reservation.

`validate_stream_proposal` publicly exposes the existing decoder's pure validity
check without returning authority, observing a prefix or doing I/O/allocation.
The runnable command uses it before creating a client or connecting for any new
input. Wrong stream identities, malformed lengths/UTF-8, changed transport units,
raw payloads and cumulative release frames refuse. A valid shape is not evidence
that the target is fresh, the policy permits it, the budget remains available or
the effect will execute. The original server still makes every such decision.

For an existing stream client, the selected saved intent is also preflighted from
the canonical client image before socket checks or writer acquisition. After
opening, the selected intent is checked again before preparing socket work.
The preflight is not an atomic storage transaction; the original lock and exact
profile-binding checks still govern reopening. A corrupt replacement after
preflight may require failing after open, but is never transmitted as a valid
stream intent. Inspection and final reporting check the selected saved intent
as well. No stored command is rewritten or decoded against today's stream prefix.
This adds one bounded client-image read to stream submit/resume startup; it adds
no journal event, durable replacement or extra peer command. Version-one command
paths keep their previous read and write behavior.

Document-free resume uses the original saved intent and exact request key. It can
recover an old message after subsequent messages or finish when the receiver uses
the original stream reconstruction path. The separately provisioned receipt-only
stream server is permanently non-admitting; an arbitrary server cannot acquire
that property from a client's configuration. A normal admitting receiver still
has normal admission semantics. Keep endpoint identity and mode under independent
operator control.

Cancellation retains its original acknowledgment requirement and single Cancel
operation. An unsent reservation may be released by the server; a dispatched
unknown effect remains charged, and cancellation never creates stream finish.
The command does not broaden unacknowledged cancellation, perform automatic
reconnects, replace request IDs, or infer nonexecution from a lost response.
Inspection stays historical and read-only, with no request payload disclosure.

## Tests and limits

Seven new regression functions cover an independent original raw-binding vector;
all five stream-field substitutions with unchanged staging; malformed incoming
intents and cumulative-frame refusal before client creation; two Unicode messages
and explicit finish through original congress/human review/publication and
reconciliation for three batch sizes; actual command timeout followed by
document-free recovery after a later message; invalid saved intent refusal across
inspect/resume/cancel/submit; closed schemas; and cancellation before versus after
dispatch with unchanged unknown charges and no finish.

These are authored assertions, not passing executions. The targeted remote-only
RCH test and unchanged full gate were attempted in the editing runtime, where
`rch` is unavailable; no compilation, tests, rustfmt or Clippy pass is claimed.
The work is a reference integration, not trained-model calibration, executable
attestation, storage authentication, remote exactly-once effects, or a wall-time
bound on synchronous disk/socket operations. Existing assertions and cancellation
rules are unchanged. No dependencies, Beads states or qualification gates change.
