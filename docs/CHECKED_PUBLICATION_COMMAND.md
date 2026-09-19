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

## Required live change feed (version 2)

The same checked commands also accept `fa.supervised-witnesses/2`. This is an
explicit stronger profile, not an optional missing-file fallback. It requires
all version-1 fields PLUS this `feed` object:

```json
{
  "schema": "fa.supervised-witnesses/2",
  "source": 91,
  "original": "/private/review/original-capture.bin",
  "current": "/private/review/current-capture.bin",
  "limits": {"bindings": 8, "steps": 10000, "value_bytes": 1048576},
  "requests": [
    {"kind": "exact_value", "key": 0, "role": "subject"},
    {"kind": "absent_key", "key": 1},
    {"kind": "empty_range", "start": 6, "end": 9},
    {"kind": "range_members", "start": 2, "end": 6}
  ],
  "feed": {
    "source": 41,
    "path": "/private/review/feed.bin",
    "after": 0,
    "clock": "unix_milliseconds",
    "max_age_ms": 20,
    "lookup": {"steps": 10000, "bytes": 1048576}
  }
}
```

The feed source is independent of the witness producer. `after` is the operator's
original complete sequence cut; it is never inferred from the first file's window.
The clock is the supervisor's existing Unix-millisecond domain. Positive maximum
age, exact sequence coverage, producer generation and source identity retain the
original native semantics. Read time cannot renew a producer's old timestamp.
A zero lookup allowance remains conservative, never an instruction to ignore
notifications. Version 2 refuses an absent, null or malformed feed configuration;
version 1 refuses an unexpected feed field instead of silently ignoring it.

Creation uses `create_with_publication_change_freshness`, so validation, change
source/cut/lookup and clock/freshness policies occupy the FIRST canonical image.
No crash between individual enable calls can leave a successfully initialized
version-2 owner with one of those required gates missing. Native policy-source
and recovery-reserve bootstrap still precede actor admission as before.

At each authorization, dispatch and first-publication boundary the workflow now
uses `step_from_files_with_publication_feed`: catch up the concrete immutable feed
window FIRST, then obtain the current native policy/committee and witness inputs.
This order retains the current witness revision after indexed invalidation. It
does not create an atomic snapshot across producers or combine the driver phases
into one effect transaction. No source is polled to manufacture permission; the
original gate rechecks the complete requirements after all acquisitions.

The file is the existing `PublicationFeedBatch` binary format, containing its
contiguous retained changes and producer heartbeat. Gaps do not disappear merely
because a later heartbeat is current. A missing or expired feed cannot be replaced
by a valid witness image, a live policy source, two approval keys or an unchanged
committee answer. Conversely, a fresh feed cannot replace a missing native policy
source. Both producer obligations remain independently enforced.

Changes after dispatch retain native outcome precedence. An actual phantom in an
originally absent key/range prevents execution; unrelated changes still permit
exact revalidation. An ordinary lost final feed read can seal via the original
endpoint and settle its nonexecution receipt. Conflicting acknowledged history
quarantines the owner; no receipt is invented and the unresolved charge remains.
The command's original stop/drain path cannot refund an unknown effect locally.

Version-2 resume pins ALL three native policy tuples before replay cleanup or the
recovery fence. It reads no producer files, obtains no new keys, and launches no
helpers. Wrong source, initial cut, lookup budget, freshness or validation limits
returns no owner and makes no recovery write. Selecting the weaker version-1
profile against a stored feed-required owner also refuses: that detection follows
its native fence, so it may advance recovery, but never returns a downgraded owner.
Path strings are transport choices, not a new authenticated source identity.

Seven additional full-workflow tests cover initial profile atomicity, successful
catch-up, missing/gapped/expired feeds with a neighboring live control, post-send
phantoms versus unrelated changes, feed loss versus conflicting history, native
policy-source loss and producer-free recovery with exact profile pinning. The
fourteen tests across both slices are authored but UNEXECUTED pending RCH. They
use real native owners, files, launched helper fixtures and reviewer sockets; the
fixtures do not establish trained-model accuracy, source authentication, complete
real-world dependency capture, filesystem isolation or production qualification.
