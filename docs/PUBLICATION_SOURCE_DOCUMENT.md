# Direct source observations for the publication producer

The producer command accepts a bounded, explicit `fa.publication-source/1` JSON
observation. Operators no longer need to implement the native Rust binary wire
format before exercising structured witnesses and whole-input publication checks.
This adapter constructs the **existing** `FileWitnessInput`, `ActualHelperInput`
and `FilePublicationInputs` types; the original producer still owns expected
versions, input floors, change derivation, retries and durable replacement.

```text
publication_producer source-observation SOURCE_JSON
publication_producer create-source PROFILE SOURCE_JSON
publication_producer publish-source PROFILE SOURCE_JSON
```

`source-observation` emits the original sealed `fa.publication-observation/1`
packet document. The other two commands read one complete source document and
perform the same lifecycle as `create`/`publish`; neither path samples a new clock
or has a create-on-missing fallback. Source decoding and stateless native input validation precede opening the
producer store or cleaning its recovery staging files. Checks against stored
generations, timestamps and revision floors remain the original owner's job.

## Complete source document

See the executable test fixture at
[`crates/fa-reference/fixtures/publication_source.json`](../crates/fa-reference/fixtures/publication_source.json).
Its historical timestamp, identities, bytes and profile epochs are illustrative,
not real source freshness or authentication evidence.

The root requires exactly these fields:

| Field | Meaning |
| --- | --- |
| `schema` | Exactly `fa.publication-source/1`. |
| `expected_generation` | Original producer generation; zero for creation or exact recovery of an installed initial observation. |
| `observed_at_unix_ms` | Explicit time of this observation, never the retry time. |
| `structured` | Complete structured input object, or explicit `null` for unavailable. |
| `opaque` | Complete actual helper-input object, or explicit `null` for unavailable. |

Missing lanes are errors, not implicit nulls. A null lane records unavailable
source data and cannot remove already retained consumer requirements. Removing a
lane emits the native producer's conservative invalidation; restoring it cannot
reset the remembered structured revision/control/semantic floors.

Unsigned integers retain the full native u64 range and are not parsed as floating
point. Binary values are canonical lower-case hex; no Unicode normalization,
text replacement or explanation-based filtering is applied to submitted bytes.
Foreign schemas, unknown/duplicate fields, invalid hex and malformed native inputs
are rejected. There are no caller-supplied `changes`, heartbeat coverage or permit
fields.

### Structured lane

Required fields are `revision`, `control_cut`, `semantic_epoch`, `domain`,
`closure`, `admitted_close` and `entries`.

`domain` contains `domain_id`, `domain_epoch`, and `projection`. A projection key
contains `source`, `branch`, `projection` and `source_epoch`. These are the native
logical identities, not credentials or authentication tokens.

`entries` is an array of objects with `key`, `version`, and `value_hex`. All entries
are retained; duplicates fail the native snapshot constructor. Entry-count and
per-value-byte bounds are the original witness constants.

`closure` is one of:

```json
{"kind":"unknown"}
{"kind":"conservative_summary"}
{"kind":"closed","marker":{"key":{"source":40,"branch":4,"projection":7,"source_epoch":1},"final_sequence":1,"marker_generation":1}}
```

`admitted_close` is separately required: either `null`, or a full marker object
with `key`, `final_sequence` and `marker_generation`. It records the operator's
independently admitted closing observation. A marker for a different projection
is rejected instead of being silently ignored.

The adapter never copies the snapshot's closure claim into its admitted frontier.
It reconstructs only an explicitly supplied admitted marker, through the same
constant-span native frontier operations used for file replay. A closed assertion
with no admitted close cannot establish absence; an admitted marker with an
unknown/summary snapshot cannot establish absence either. Incompatible declared
and admitted markers remain incompatible and fail native witness capture; they
are not repaired or replaced with whichever version would pass.

**Both closing fields are trusted-operator assertions.** JSON parsing does not
prove the source was authenticated or fully observed. Access to these documents
and the producer directory must remain outside the actor's authority. A real
source adapter must supply its actual complete snapshot and independent admission
observations; EOF, an empty query result, or a caller-selected integer is not a
substitute.

### Whole-input opaque lane

Required fields are `submitted_hex`, `input_profile`, `parts` and `omissions`.

`input_profile` requires `profile_id`, `profile_hex`, `tokenizer_epoch`,
`policy_epoch`, and `model_epoch`. A changed model/profile epoch remains a changed
opaque dependency even when all visible submitted bytes are identical.

Each ordered part has `start`, `end`, and a `kind`. The supported kinds are
`question`, `prompt`, `instruction`, `delimiter`, `other`, `tool_schema` (also
requires `schema_id`), and `evidence` (also requires `source_id`, `transform_id`).
The native constructor requires an exact contiguous half-open partition of all
submitted bytes and exactly one question. Gaps, overlaps, empty parts, missing
bytes, and extra questions are errors; the adapter does not synthesize delimiters
or infer part locations from text.

Each omission has `domain_id` and `kind`: `closed_absent` also requires a nonzero
`trusted_closure_marker_id`; `gapped` requires `first_missing`; `unsupported` has
no additional fields; `redacted` requires `transform_id`. The native constructor
rejects duplicate omitted domains. Omission metadata is retained as supplied,
not treated as proof that omitted material could not affect the helper.

This is a full-input interchange adapter, **not** a model-provider client. The
operator must supply the exact bytes and metadata actually sent to the helper;
the command cannot establish that this occurred or that a helper was calibrated.
Any opaque byte, layout, omission or profile change remains a whole-input
invalidation. Helper explanations cannot narrow that dependency.

## Retrying across either input format

```sh
# Seal once and keep the observation; treat its embedded full input as sensitive.
publication_producer source-observation source.json > sealed.json
publication_producer create-source producer-profile.json source.json

# After lost output, either retry form checks the SAME already-installed image:
publication_producer publish-source producer-profile.json source.json
publication_producer publish producer-profile.json sealed.json
```

A successful native exact retry returns `already_current` without advancing
producer/input generations, duplicating changes or refreshing observation time.
A new timestamp or different native bytes for that same expected generation is a
conflict. Preserve the saved document until reconciliation; do not rewrite it with
"now" after a failed stdout delivery.

The producer bundle is still consumed by the unchanged
[`fa.supervised-witnesses/3` workflow](COUPLED_PRODUCER_COMMAND.md). It does not become
an actor-readable public feed: it contains full source and helper-input data.

## Verification status

Eleven additional Rust tests cover complete binary/layout/epoch preservation,
independent closure admission, incompatible/foreign markers, native key versus
whole-input invalidations, source loss and retained floors, cross-format retries
after lost stdout, malformed input before recovery cleanup, mandatory native
metadata, full-width counters/long closing prefixes, and resource boundaries.
Together with the original producer-command tests, this is eighteen authored tests.

They have **not been compiled or executed** in this editing environment. The
required RCH route cannot start because `rch` is missing; Cargo/rustc/rustfmt are
also unavailable. Use the existing repository check route, not a local bypass:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example publication_producer
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
