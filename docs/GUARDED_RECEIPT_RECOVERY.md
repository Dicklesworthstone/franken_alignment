# Guarded receipt recovery for the publication operator

The existing Unix reference example supports independently anchored recovery of
an existing actor request without starting another publication:

```text
supervise_publication resume CONFIG SUBMIT_JSON --guarded RECOVERY_PROFILE
```

The unflagged `resume`, checked modes, and both stop recovery modes retain their
original behavior. No failed guarded read, replay or reconciliation falls back
to an unguarded opener. This is a reference implementation awaiting execution
qualification, not a new production release or remote effect adapter.

## Separate, explicit operator intent

RECOVERY_PROFILE uses its own closed schema, not the stop profile schema:

```json
{
  "schema_version": "fa.guarded-resume/1",
  "minimum": {
    "journal_revision": 42,
    "control_sequence": 17,
    "authority_epoch": 2
  },
  "history_anchor": "/operator/custody/acknowledged.anchor",
  "retain_history_anchor": "/operator/custody/recovered.anchor"
}
```

Numbers and paths are illustrative, not defaults. All three numeric floors and
the independently retained input anchor are mandatory; output retention is
optional. A stop profile cannot be parsed as a resume profile or passed to the
resume consumer. Shared bounded parsing and archive custody preserve the stop
profile's original schema, limits, refusal behavior and exclusive writer.
The parser limits input to 16 KiB, depth four, 32 items and 4096-byte strings.
Unknown/duplicate/missing fields, invalid numbers and relative paths refuse.

CONFIG pins the ordinary source-backed publication scope, initial profile,
effective policy, committee and source contract. This version has no configured
stream, decoder, identity, campaign, credential or predictor guard: absence is
checked by the original complete guard inventory, not inferred from disk. The
original terminal reserve is mandatory. Other recovery profiles are not silently
admitted. A checked witness profile is not supported by this command; the
post-recovery compatibility check refuses it without forwarding actor input.
That compatibility refusal can follow an acknowledged recovery fence; it is not
a claim that the journal stayed unchanged. No stronger publication check is
bypassed or new action admitted.

## One owner, one complete replay, original receipts only

One elapsed-time budget is checked before custody admission, before opening,
before each original replay event and again at Ready. The same exclusive cursor
validates the retained prefix, full suffix, independent guard inventory and
floors. Cancellation releases the cursor without cleanup, clock acquisition or
journal changes. `FileGuardedRecovery::require_existing_request` then checks the
request's existence on that same completed history before any recovery fence.
An unknown key cannot turn receipt recovery into a new submission.

The cursor completes through `finish_reconciled`: exactly the original
`Fence -> Time -> Sweep` sequence in one acknowledged replacement. Sweep examines
all outstanding deliveries in the original domain, not only the selected actor
request. It queries original endpoint outcomes without resending effects.
Expired or otherwise unresolved results remain charged and are explicitly
reported as command errors, even when the selected request was already executed.
The retained ledger is also inspected: absence of an already irrecoverable
delivery from a fresh sweep map cannot be treated as evidence of settlement.
The clock is called once for completion; a saved request deadline is never
extended. There is no second historical replay or post-completion driver sweep.

The existing actor gateway receives the unmodified original submit document only
after existing-request validation and recovery acknowledgment. It checks exact
proposal equality, reconstructs a read-only ticket and renders its original
`Knowledge` response. A conflicting proposal produces the original idempotency
conflict, never another admission. Such a conflict may be reported after the
recovery transaction has committed; it does not undo that transaction.
No admission snapshot, helper process, reviewer socket, new approval, evidence
refresh or effect dispatch is supplied by this path. Returned native roles are
dropped, and the recovered owner does not escape the command.

## Archive and output failures do not undo acknowledgment

Input and output must be outside the authority directory. Output must be a new
private file; create-new mode 0600, bounded encoding, file sync and parent sync
are shared with guarded stop retention. An appearing or partial output is not
overwritten. Requested retention captures the acknowledged original owner under
the input-prefix check before actor-response rendering, never a reopened owner
or a later disk image. The actor response contains no archive or private evidence.

Every post-acknowledgment failure names the recovery revision and says no effect
was resent. Unresolved sweep results, failed anchor retention and failed output
are combined rather than hiding one another. The original actor response is
attempted even if requested archive retention failed. As with existing resume,
only an executed actor outcome with no outstanding command failures exits
successfully; canceled, denied, unknown and conflicting requests are not reported
as executed. A JSON response alone does not acknowledge archive retention.

Retry with the original request and an independently retained anchor, selecting
a new output path when needed. Live recovery always consumes another original
fence/time/sweep and calls the clock again, unlike an already-drained stop's
zero-event retry. Time and Sweep cannot borrow the emergency stop reserve;
insufficient ordinary capacity refuses the live transaction without spending a
preliminary fence. The direct stopped path remains the terminal alternative.

## Scope and verification

File decoding, each original reducer call and synchronization are synchronous;
event checkpoints do not establish a hard cancellation-latency bound. Trusted
clock/storage behavior, advisory-lock cooperation and independent nonrollback
custody remain assumptions. Anchors do not authenticate custodians, and rolling
back both archive and journal defeats the continuity assumption.

Nine new operator regression functions use the actual source publication
workflow, actor wire, synthetic helper processes, reviewer protocol and native
journal. Coverage includes strict schema separation; successful executed receipt
recovery and successor retention; missing/malformed requests and missing stores;
original idempotency conflicts; all elapsed checkpoints; independent floor,
source, anchor and custody refusal; combined export/output failure; expired
unknown charges; and actual CLI dispatch without live evidence or qualification.
One additional native test pairs missing/incomplete request refusal with original
recorded admission and successful guarded reconciliation. Existing tests remain.

These tests are authored but unexecuted. Targeted and full-gate RCH dispatches
stop at missing `rch` before Cargo in this environment. Rust compilation, tests,
formatting and Clippy remain unqualified; no bead or FA-014 acceptance is closed.
This serves plan 16.4/16.5/16.9 and 17.1, FI-A02, with the same journal format,
actor verbs, original authority semantics and dependency set.
