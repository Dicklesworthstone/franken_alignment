# Native publication producer command

`publication_producer` drives the original `FilePublicationProducer` owner. It
creates and advances the single coupled snapshot/change-feed bundle consumed by
`fa.supervised-witnesses/3` checked supervision. There is no new journal format,
separate change-list writer, actor authority, helper approval or effect executor.

## Independently configured producer identity

```json
{
  "schema": "fa.publication-producer/1",
  "directory": "/srv/fa/source/producer",
  "source": 91,
  "feed": 41,
  "after": 0,
  "clock": "unix_milliseconds",
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "minimum_generation": 1
}
```

Use the real deployment's scope. The supervisor's version-3 `producer.path` is
this producer directory's `delivery.bin`; its snapshot source, scope, feed source,
bootstrap `after` and clock must match. See [checked supervision](COUPLED_PRODUCER_COMMAND.md).
The directory must not exist for creation, and its parent must already exist.
The original Store creates and locks a private Unix directory; the command does
not relax its permissions or use an unprotected replacement implementation.

`minimum_generation` is an independently retained lower bound on the producer's
heartbeat/observation generation, not its snapshot generation or feed sequence.
Raise it from independently acknowledged history when protection against rollback
is required. Keeping the floor only beside a rollbackable bundle does not protect
against whole-directory rollback. `after` always remains the initial bootstrap
sequence; do not replace it with the latest feed head.

## Seal a real observation, then publish it

The adapter supplies `INPUT_PACKET` as the canonical bytes of the existing
`FilePublicationInputs::to_bytes()` format. This preserves complete structured
snapshot data, admitted closure provenance and any complete opaque input. A
capture packet or a whole producer image is not an input packet. The command
validates the native packet instead of synthesizing a closing frontier, assuming
EOF means completeness, or allowing a caller-selected notification list.

```text
publication_producer observation EXPECTED_GENERATION OBSERVED_AT_UNIX_MS INPUT_PACKET
publication_producer create PROFILE OBSERVATION_JSON
publication_producer publish PROFILE OBSERVATION_JSON
publication_producer inspect PROFILE
```

For example, after a trusted source adapter has exported `inputs.bin` and recorded
its actual observation time (the following time is illustrative, not a current
freshness claim):

```sh
publication_producer observation 0 1789875000000 inputs.bin > initial.json
publication_producer create producer-profile.json initial.json

# A NEW actual observation, using the preceding acknowledged generation:
publication_producer observation 1 1789875001000 next-inputs.bin > successor.json
publication_producer publish producer-profile.json successor.json
publication_producer inspect producer-profile.json
```

The builder emits a bounded `fa.publication-observation/1` JSON document containing
`expected_generation`, `observed_at_unix_ms` and canonical lower-case `inputs_hex`.
It embeds the exact native input bytes rather than retaining a mutable file path.
The document binds original producer time to the submitted data for exact retries.
The command never samples its own clock to make replayed data look newly observed.
It cannot establish that an operator-supplied timestamp or packet is authentic;
source acquisition, timestamp provenance and completeness remain adapter/host
obligations, and the consumer still owns freshness admission.

The existing native owner derives key/all-domain invalidations from the complete
old/new images and atomically replaces snapshot plus feed through the same
lock/stage/fsync/rename barriers. Equal newly observed inputs may advance heartbeat
generation while retaining snapshot generation and emitting no key changes.
An explicitly absent lane remains absent: it does not remove a consumer's existing
witness requirements. All packet/document reads are bounded; detected replacement,
symlinks, special files, duplicate/unknown JSON fields and malformed native bytes
are refused. These filesystem checks are not a sandbox against a hostile kernel.

## Recovery and lost output

`create` requires expected generation zero and minimum generation one. It never
opens an existing directory as a fallback. `publish` opens only an existing,
matching producer and applies the original expected-generation operation.

After lost stdout or another ambiguous acknowledgement, repeat **publish with the
same saved observation document**. This also reconciles an already installed
initial observation with expected generation zero. The native owner's exact retry
returns `already_current` only after verifying actual canonical bytes; it neither
advances counters nor extends time. Changed bytes or observation time for that
same operation conflict. A missing producer, foreign profile, rollback below the
independent floor or stale expected generation does not trigger bootstrap, a
weaker reader, time relabelling or automatic replacement.

The whole observation is decoded before recovery opening/cleanup. After successful
storage, a failed stdout write or flush does not roll back or repeat publication.
Keep the saved observation document until its result has been reconciled.

`inspect` calls the native read-only `FilePublicationProducer::read_image` API.
It works while a cooperating writer owns the lock, checks the independent profile
and generation floor, and never cleans pending files, confirms durability or
refreshes time. Its `historical` report omits raw structured/opaque input bytes.
Generation, sequence and time counters are decimal **strings** in diagnostic
output to preserve full u64 precision for non-Rust consumers.

## Build and verification

Use the repository's required remote Cargo route:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo build --locked -p fa-reference --example publication_producer
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example publication_producer
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Seven authored tests cover sealed binary observations/full-width counters, complete
native updates and quiet heartbeats, exact retries and lost stdout, read-only
inspection under a held writer lock with pending bytes, profile/floor mismatch,
malformed input before cleanup, missing-store refusal and bounded non-symlink reads.
This editing environment lacks RCH and Rust tools; compilation, Rust tests,
formatting and Clippy remain unexecuted here. No production qualification follows
from these authored tests.
