# Read-only policy replay operator

`supervise_publication policy-replay CONFIG CANDIDATE_JSON` compares a candidate
exact policy with every retained proposal and applied review belonging to the
effective policy in one canonical publication journal image. It includes exact
denials and later reviews as separate cases. The owning native controller selects
the complete corpus; the operator cannot select favorable archives, filter out
denials, or return a successful prefix after a replay limit is reached.

```sh
cargo run -p fa-reference --example supervise_publication -- \
  policy-replay /operator/supervisor.json \
  crates/fa-reference/fixtures/policy_replay_candidate.json
```

The supervisor file is the original store profile. Its initial policy and all
other native profile bindings must match the journal even if a later policy is
currently effective. The candidate's `expected_policy_generation` identifies
that effective predecessor. Candidate generation must strictly advance it. This
command reads an existing journal; use the normal publication workflow first to
produce retained proposals. An empty current-policy corpus is unavailable.

## Candidate request

The committed fixture illustrates a stricter payload limit for the ordinary
supervised-publication fixture:

```json
{
  "schema_version": "fa.policy-replay-request/1",
  "expected_policy_generation": 1,
  "limits": {
    "cases": 128,
    "input_bytes": 1048576,
    "output_bytes": 4194304
  },
  "candidate": {
    "generation": 2,
    "nodes": [
      {"op": "exact_value", "key": 7, "value_hex": "6f6b"},
      {"op": "payload_at_most", "maximum": 256},
      {"op": "units_at_most", "maximum": 512},
      {"op": "all", "children": [0, 1, 2]}
    ]
  }
}
```

All fields are required. Unknown fields, duplicate JSON keys, extra command
options, unsupported schema versions, fractional/negative integers, invalid
policy graphs, uppercase or odd-length hexadecimal, and non-regular/symlink
candidate files are rejected. Candidate files are at most 2 MiB, JSON nesting is
at most 12, item count is at most 16,384, and individual strings are at most 1 MiB.
Policy admission uses the same parser and native `Policy::new` validation as the
supervisor configuration: topologically ordered nodes, all reachable from the
last/root node, native node/edge/literal/read bounds, no executable callbacks.

Supported node forms are:

| `op` | Other fields |
| --- | --- |
| `target_is` | `target`: `adapter`, `object`, `contract_version`, `expected_version`, `generation` |
| `payload_is` | `value_hex` |
| `payload_at_most` | `maximum` |
| `units_at_most` | `maximum` |
| `exact_value` | `key`, `value_hex` |
| `absent` | `key` |
| `empty_range` | `start`, `end` |
| `all`, `any` | `children`: preceding node indices |
| `not` | `child`: preceding node index |

All limits must be positive. `cases` is at most 5,120. `input_bytes` is at most
32 MiB and counts native retained action payload and witness occurrences,
including repeated observations and candidate witnesses; it is not a peak-memory
measurement. `output_bytes` is at most 64 MiB and caps the staged complete JSON
encoding. The original supervisor journal-byte/event limits also apply before
native reconstruction. No limit produces a partial successful corpus.

## Report semantics

Successful stdout is one `fa.policy-replay/1` JSON document, followed by a newline.
Its `knowledge` wrapper identifies a `known` result with a `bounded_model` basis,
scope `retained_exact_policy_corpus`, provenance `operator_trusted_local_journal`,
and a `single_canonical_image` cut. The value contains:

- Historical journal revision, control sequence, authority epoch, executions,
  charged and reserved units at that cut.
- Complete previous and candidate policy trees, original replay limits, retained
  input-byte charge, and case count.
- Every proposal and applied review case, exact action scope/target/payload,
  original and candidate predicate traces, original review consequence where one
  exists, and snapshot semantic epoch. Proposal cases have a null original
  review consequence because they are not helper review decisions.
- Explicit `newly_blocked` and `newly_reviewable` case-ID sets, per-case deltas,
  `requires_shadow`, and exact missing node indices for unsupported observations.

The four deltas mean:

| Delta | Meaning |
| --- | --- |
| `unchanged` | Original and candidate exact predicates have the same result on the retained observations. |
| `newly_blocked` | Original exact predicates passed; candidate exact predicates fail. |
| `newly_reviewable` | Original exact predicates failed; candidate exact predicates pass. This is the relaxation set. |
| `requires_shadow` | Retained observations cannot establish every candidate read; a separate prospective shadow observation would be required. |

A key omitted from the original retained witnesses is not evidence that the key
was absent. A new `absent`, `exact_value` or `empty_range` predicate can therefore
require shadow data even when the original full snapshot happened to contain the
answer. Missing observations in an `any` branch remain unsupported even if
another branch would pass. Such candidate evaluations carry `kind: unknown`,
`reason_code: MissingRequiredObservation`, and the missing node indices. This
command does not perform the prospective shadow run.

`newly_reviewable` means only that the exact candidate predicates pass for that
historical action and observation. It transfers no helper vote, human approval,
dispatch key or policy-promotion permission. The output explicitly reports
`authority: not_issued`, `policy_changed: false`, `helpers_rerun: false`,
`live_evidence_read: false`, and `replay_grade: decision_replay`. It is neither a
model rerun nor an estimate of safety on unseen actions.

## Read and failure boundary

The command uses `FileOversight::read_policy_replay`, so it can operate while the
original writer still owns its lock. It reads one canonical image without
acquiring the writer, applying recovery, advancing authority epochs, deleting
pending files, refreshing evidence, contacting helpers, or issuing campaigns.
It leaves reservations, charges, unknown deliveries, publication state and live
permits untouched. New decisions appended after the captured image are outside
the historical cut; rerun the command to compare a newer image.

This read checks native journal/profile validity. It does not authenticate the
journal's external origin or prove that a retained local image is the latest
durable history. The report makes that provenance limitation explicit. It
cannot substitute for the independent custody and exact predecessor checks of a
live promotion flow. Unsupported native journal profiles remain unavailable;
there is no weakened replay or compatibility fallback.

One elapsed budget starts before configuration ingestion, using the original
`timing.runtime_ms`. Checkpoints follow configuration, candidate ingestion,
native replay, and encoding. Reads and native replay are synchronous bounded
operations: the checkpoints do not promise to interrupt a single operation at
the deadline. Failure, timeout, stale predecessor, empty corpus, corruption or
resource exhaustion before output emits no report. Encoding is fully staged
within `output_bytes` before the first stdout byte. An output-device failure can
still interrupt transport of that complete encoding; it changes no authority or
effect state.

The executable tests exercise real durable proposal/review histories, positive
relaxation and tightening, unobserved-branch shadow requirements, writer-lock
coexistence without evidence/helper files, reopen replay, byte-preserving
no-mutation checks, invalid/truncated/wrong-profile histories, strict request
admission, all closed predicate encodings, resource exhaustion, deadline phases,
and output failure. No live policy promotion is exposed by this command; native
promotion remains governed by [DURABLE_POLICY_CAMPAIGN_GUARD.md](DURABLE_POLICY_CAMPAIGN_GUARD.md).
