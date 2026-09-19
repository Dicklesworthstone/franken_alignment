# Witness-checked publication command

The existing `supervise_publication` consumer now supports:

```sh
supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE
supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE --reviewer-profile REVIEWER_JSON
supervise_publication resume-checked CONFIG SUBMIT_JSON WITNESS_PROFILE
```

Build/run the existing example through the repository's required RCH toolchain.
The checked modes reuse the original actor gateway, exact helper roster and
process launcher, independent reviewer connection, native policy-source lease,
two-key dispatch, guarded publication, receipt reconciliation and child cleanup.
A configured `fa-native-helper` executable works through the same HelperProgram
entry as other provisioned helpers. No new inference, reducer or effect sink is
implemented by these commands.

## Explicit original requirements

`WITNESS_PROFILE` is a bounded strict JSON object. All fields are mandatory, and
unknown fields, duplicate JSON keys, duplicate witness identities, empty recipes,
relative source paths and unsupported schemas refuse. Example:

```json
{
  "schema": "fa.supervised-witnesses/1",
  "source": 91,
  "original": "/private/review/original-capture.bin",
  "current": "/private/review/current-capture.bin",
  "limits": {"bindings": 8, "steps": 10000, "value_bytes": 1048576},
  "requests": [
    {"kind": "exact_value", "key": 0, "role": "subject"},
    {"kind": "absent_key", "key": 1},
    {"kind": "empty_range", "start": 6, "end": 9},
    {"kind": "range_members", "start": 2, "end": 6}
  ]
}
```

Exact-value roles are `subject`, `policy_input`, and `predicate_input`. Intervals
are half-open `[start,end)`, with `start < end`; no successor of u64::MAX is
computed. Requests and bounds retain the existing native witness semantics.
Zero validation work is restrictive, not an implicit unlimited allowance.

Both capture files use the existing `FilePublicationCapture` format. The original
contains the exact attempt, complete controller-produced frozen action, producer
identity/generation and original structured image/closing observation. The capture
must be prepared against the actual proposal and policy-derived action, not just
its payload. The command accepts no fabricated digest or permission from a file.
The recipe is an operator assertion about the review's dependencies; it does not
prove a helper read a database or authenticate that database's contents.

Creation publishes the mandatory native validation profile in the first journal
image, before any actor proposal. After the actor gateway produces its action,
the original file is read and bound once BEFORE any helper is launched. It must
contain structured evidence; exact/negative/range requirements are captured by
the original witness engine. The current file is never substituted for a missing
original or used to redefine requirements after a vote is seen.

Following congress and independent human review, the command invokes the original
`step_from_files_with_publication_source`: native policy/committee and current
witness files are independently reread at authorization, dispatch and first
publication. The original owner withdraws eligibility before reads and validates
producer/action identity, generation floors, full input and witness requirements.
Current drift cannot be fixed by rebasing the original recipe. A post-dispatch
invalidation seals through the original endpoint, and only its receipt settles
nonexecution. Human consent is not a substitute for current evidence.

## Recovery and limits

`resume-checked` pins the original validation limits before replay cleanup/fencing,
then uses the existing receipt-only path. It never reads original/current/policy
sources, starts a helper, contacts a reviewer or reissues a dispatch. A changed
request still produces the original idempotency conflict. The legacy command
does not silently downgrade a stored checked profile; select the explicit mode.
Recovery does not authenticate newly named file paths or treat their absence as
proof of nonexecution. Only historical journal/endpoint outcomes are returned.

Reads remain sequential, not an atomic cross-file snapshot. Producer authenticity,
complete dependency modeling, stable protected directories and the quality of
helper judgments remain operator obligations. This source-only profile does not
assert a producer heartbeat or a complete notification feed. The current file
can be reread without proving its producer is still alive. No process sandbox,
remote transaction, model accuracy or production qualification is claimed.

Seven authored tests exercise strict profiles, the actual helper/reviewer process
path, valid publication, original absence versus current phantom/unrelated keys,
post-dispatch revalidation/settlement, missing or action-substituted originals,
missing current sources, and receipt-only recovery with mismatched-profile refusal.
Synthetic helper verdicts are explicitly fixtures. The RCH gate is unavailable in
the editing environment; Rust compilation, formatting, Clippy and these tests are
UNEXECUTED. Lexical, fixture-JSON and whitespace checks are not execution evidence.
