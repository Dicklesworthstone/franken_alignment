# Whole-input supervised publication

`fa.supervised-whole-input/1` connects the existing opaque whole-input publication
judgment to the runnable supervisor. It requires no synthetic structured snapshot
or dummy key witness. It uses the same coupled producer, actor gateway, helper
review, human approval, publication driver and durable accounting as the checked
structured workflow. No new authority, effect endpoint, journal format or runtime
is introduced.

## Explicit profile

```json
{
  "schema": "fa.supervised-whole-input/1",
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
  "requests": []
}
```

Replace illustrative identities and paths with the independently configured
producer and supervisor scope. The producer profile, feed bootstrap position and
clock must match exactly. This schema requires an explicit empty `requests`
array; existing `fa.supervised-witnesses/1`, `/2` and `/3` profiles still require
nonempty structured recipes. No existing malformed empty recipe becomes valid.

The producer's original `opaque` lane must contain a complete native
`ActualHelperInput`. Missing opaque input refuses before helper launch even when
a structured snapshot is available. A nonempty structured recipe still requires
its structured input and cannot fall back to an opaque-only source.

Use the existing commands with this profile:

```text
supervise_publication create-checked CONFIG SUBMIT_JSON WHOLE_INPUT_PROFILE
supervise_publication submit-checked CONFIG SUBMIT_JSON WHOLE_INPUT_PROFILE
supervise_publication resume-checked CONFIG ORIGINAL_SUBMIT_JSON WHOLE_INPUT_PROFILE
```

The [direct source adapter](PUBLICATION_SOURCE_DOCUMENT.md) can supply the producer
without implementing its binary wire format: set `structured` to explicit `null`
and provide the complete `opaque` object, then use `publication_producer
create-source` or `publish-source`. Both source lanes remain explicit; omitted
keys are not an unavailable-input shorthand.

## Original requirements, not a narrower explanation

Before any helper answer, the supervisor binds the entire original producer
packet to its actual frozen action through the original owner. It does not strip
additional present lanes. The opaque witness retains submitted bytes, profile
bytes, tokenizer/policy/model epochs, part kinds and boundaries, and omissions.
A helper explanation cannot select a smaller subset of that view.

At authorization, dispatch and first publication the native driver reacquires
current producer data. The native whole-input judgment performs the comparison
under its configured work budget; the native feed still requires complete change
coverage and fresh observations. A quiet new observation with unchanged input can
remain valid. Changed input, missing lanes, expired freshness or insufficient
comparison budget cannot become permission. Adding a new lane does not silently
change the original judgment's lane shape either.

This profile selects the recipe for a new review; it is not a new journal-wide
policy flag. Once an attempt is bound, its original native requirements remain
in the journal and cannot be narrowed by a later profile or current source image.
An exact recorded retry and explicit resume remain receipt/reconciliation-only,
including after the original deadline. Neither needs the original/current
producer file, helper evidence file or a new human offer. Retained budget and
execution history are not reset.

## Trust and verification limits

The producer's claim that an opaque view is the actual relevant helper input is
still an operator assertion. This integration does not prove that those bytes
were sent to a remote model, authenticate a producer, or calibrate a detector.
The separate original `FileEvidenceSource` path for committee input remains
mandatory; a producer packet does not replace that source or either approval key.
Protect complete input data from actor access as described in the producer docs.

Eight authored regression tests cover explicit schema selection, unchanged legacy
recipes, complete original lane retention, early missing-input refusal, the
nonempty structured requirement, ten post-dispatch unchanged/mutated cases,
comparison budget and freshness, source-free receipt recovery and exact retries,
and unrelated structured updates alongside opaque model-epoch changes. Existing
structured tests are unchanged apart from registering this new test module.

The tests are not execution evidence. The required RCH example-test and full
xtask invocations cannot start in this editing environment (`rch` absent, exit
127); Cargo, rustc and rustfmt are also absent. Compilation, formatting, Clippy
and all Rust execution remain unverified:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
