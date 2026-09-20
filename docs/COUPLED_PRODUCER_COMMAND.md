# Live producer-backed checked supervision

`supervise_publication` accepts `fa.supervised-witnesses/3` in the existing
`create-checked`, `submit-checked` and receipt-only `resume-checked` commands.
It connects the native coupled publication producer to the real actor gateway,
helper processes, independent human reviewer and original publication driver.
No separate capture-owner journal, guessed attempt ID or manufactured action
frame is required.

## One producer bundle, two original readers

```json
{
  "schema": "fa.supervised-witnesses/3",
  "source": 91,
  "producer": {
    "path": "/srv/fa/source/producer/delivery.bin",
    "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5}
  },
  "feed": {
    "source": 41,
    "after": 0,
    "clock": "unix_milliseconds",
    "max_age_ms": 5000,
    "lookup": {"steps": 10000, "bytes": 1048576}
  },
  "limits": {"bindings": 8, "steps": 10000, "value_bytes": 1048576},
  "requests": [
    {"kind": "exact_value", "key": 0, "role": "subject"},
    {"kind": "absent_key", "key": 1}
  ]
}
```

Configure the real deployment's scope rather than copying the example numbers.
The entire producer profile (snapshot source, scope, feed source, clock domain and
bootstrap `after`) must match the canonical `PublicationProducerImage`. The
producer scope must match the independent supervisor scope even on recovery;
this check precedes journal creation or its recovery fence. `after` is the original
feed bootstrap position, not today's latest sequence.

Version 3 has **no** `original`, `current`, or separate feed `path` field. Supplying
any of them is rejected. Version 1 remains the original capture-only mode;
version 2 remains the original separately encoded capture/feed mode. Neither
legacy reader is taught to silently accept producer images.

Before the first helper launch, the native owner constructs a producer reader
bound to its actual original frozen action and attempt. The command reads one
original producer snapshot and binds its witness recipe through the existing
journal event. The prepared command retains only the action-bound live reader,
not current evidence. Subsequent authorize, dispatch and first-publication checks
reopen the SAME selected bundle through the existing snapshot and feed readers.
Source scope, original action binding, producer freshness, complete coverage and
witness validity are still checked by their original owners.

A producer update between those separate reads can produce a mismatch. It fails
closed; there is no cross-file repair, cached heartbeat, relabelled input cut or
fallback to the original image. Missing, stale or malformed producer data never
becomes positive evidence merely because a human approved the request.

## Existing journals and recovery

```text
supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE
supervise_publication proposal-next CONFIG REQUEST_ID PAYLOAD_FILE TTL_MS
supervise_publication submit-checked CONFIG SUBMIT_JSON WITNESS_PROFILE
supervise_publication resume-checked CONFIG ORIGINAL_SUBMIT_JSON WITNESS_PROFILE
```

The same version-3 profile can be used for successive new requests. Each new
accepted action gets its own original capture before its own helper and human
review; the producer need not know the next actor attempt. Rights are not
replenished, target versions are not rewritten, and the journal's original
recovery fence still applies. Exact recorded retries and explicit resume remain
receipt/reconciliation-only: neither producer data nor helper evidence is read,
and no new reviewer offer is created.

The producer bundle contains complete native input data. Its path must remain
in a trusted, access-controlled producer directory; it is not an actor-readable
public feed just because its feed projection can be extracted. This integration
is a synchronous Unix reference workflow, not a daemon, source authentication,
a sandbox, proof of source completeness or detector calibration.

## Verification

The new example tests cover strict profile selection, actual-owner action binding,
live reader replacement, foreign/missing sources, late unrelated versus invalidating
changes through real helper/reviewer sockets, and receipt-only recovery without
source access. They use synthetic helper verdicts, not calibrated models.
Run the repository gate using its required RCH route:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

This editing environment has no RCH or Rust toolchain; the gate could not start.
Compilation, Rust tests, formatting and Clippy remain unexecuted here.
