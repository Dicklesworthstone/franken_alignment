# Bounded actor-side candidate screening

The exact-precheck client can screen explicitly supplied alternatives without
starting the supervisor workflow or submitting a candidate:

```text
supervise_publication actor-precheck --batch ACTOR_PROFILE PRECHECK_PROFILE QUERY_1_JSON QUERY_2_JSON ...
```

The original single-query command and its exit semantics are unchanged. Batch is
an explicit actor-only grammar owned before supervisor options are removed. It
uses the same public profiles and strict `fa.actor-precheck/1` request documents
as [the single-query client](LIVE_ACTOR_PRECHECK.md#actor-only-command).

## Stop at the first passing candidate

```text
supervise_publication actor-precheck --first-pass ACTOR_PROFILE PRECHECK_PROFILE QUERY_1_JSON QUERY_2_JSON ...
```

This explicit ordered-search mode prevalidates the **entire** input set using the
same worst-case bounds, then checks candidates in order. It stops successfully
only after an exact passing native response has been emitted and elapsed checks
have completed. Later candidates are not connected or sent, conserving remaining
server disclosure and connection allowances. The last emitted response identifies
the passing query; the client does not copy a proposal, mint a ticket or Submit it.
Known denials continue the search; an all-denied search exits nonzero. All other
refusals and failures stop just as in batch mode. An invalid later document is
still rejected before any query, even if an earlier candidate might have passed.

## Input and work bounds

A batch has 1–16 documents. Its count must fit the independently configured
query, exchange, candidate and connection limits. These are upper bounds, not a
reservation: another client may have consumed part of the server's remaining
allowance. All documents must decode, carry the independently expected policy
epoch and have distinct query labels **before any connection is opened**.
Malformed later input therefore cannot spend earlier candidates' server work.

Every input must be a bounded regular file under the existing no-symlink reader.
Aggregate original input is capped at four maximum native request frames; each
individual input retains the native frame/payload ceiling. Decoded requests are
retained, so a file changed after preflight cannot change a later transmitted
candidate. No private Config, evidence or journal is read.

Every candidate gets one intentional fresh connection authenticated by the
original native peer client before proposal bytes. This is not a retry or a
request to renew the server's grant. Each query label is sent at most once.
There is no fallback to the ordinary submission endpoint.

The entire command shares **one 4,096-call I/O allowance** and one elapsed
allowance beginning before profile reads. Byte ceilings are the selected number
of native maximum requests/responses (including framing and response probes).
Neither partial I/O, a response nor a new connection resets these allowances.
The elapsed allowance is the smaller profile runtime. Filesystem, connection
and stdout operations remain cooperative synchronous boundaries, not hard
preemptible deadlines.

## Results and stopping rules

Stdout contains one original native JSON response per completed query, in input
order, with no wrapper or fabricated result. Known denials are emitted and do
not stop screening. In `--batch` mode, a successful exit requires every candidate to have a Known
result and at least one exact `PrecheckPassed`. In `--first-pass` mode, later
candidates are intentionally not evaluated after the first pass. An all-denied
batch or search exits nonzero.

Unknown, Withheld, Stale or a native wire refusal is emitted unchanged, then
terminates the batch with a nonzero exit. Transport/authentication/input/binding
failures do not emit a fabricated response. Output failure, elapsed/work
exhaustion and a changed disclosed observation tick also stop immediately.
No remaining candidate is contacted and no previous candidate is resent.
A visible result prefix may remain on stdout after failure; server work already
spent is not refunded, and incomplete output is not a reason for automatic retry.

Every Known response must bind the selected disclosure and epoch. The batch also
rejects a change in `observed_at` before emitting that response. This detects
visibly different source cuts; it is not an authenticated service-instance nonce
and cannot detect reuse of identical disclosed labels after service replacement.
The original live sidecar still owns revision/lease checks and revocation.

A pass remains `review_required: true`: no permit, reservation, helper approval,
actor ticket, selected effect or automatic Submit is produced. Actual submission
must use fresh source acquisition and the ordinary review path. Batch querying
uses the existing exact-predicate disclosure and its declared inference leakage;
it adds no permission, policy evaluator, grant renewal or witness cache.

## Verification

Five focused regression functions cover grammar, independent limits, duplicate
labels/epoch binding, aggregate work exhaustion and disclosed-cut continuity.
Fourteen additional scripted-socket regression functions cover native output and
ordered stopping, all-denied/inconclusive results, full preflight, retained input,
lost replies, partial stdout, authentication/correlation, changed cuts and closed
executable dispatch. These are added tests, not evidence of execution. The editing
environment has neither the required `rch` runner nor a Rust toolchain; no
compilation, formatting, Clippy, runtime pass or qualification is claimed.
No bead or acceptance gate is closed.
