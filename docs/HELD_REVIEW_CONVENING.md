# Explicit continuation of an ordinary held review

This connects the plan's named-evidence `HoldEffect` recovery (§9.8) to the
original ordinary file supervisor. A successful Hold leaves its original action
in Reviewing. The driver still retires the completed job and never starts another
congress by itself. A separately selected, finite supervisor capability can
request one explicit fresh round for that same held action. Queue admission is
not review completion, human approval, durable work, or permission to publish.

## Original custody and finite admission

`FileActorSupervisor<FileOversight>::convene_port(queries, operations)` returns a
weak `FileSupervisorConvenePort` and its nonclone
`FileSupervisorConveneGrant`. Select it once on the original owner, before
exposing the operator port. Queries and operations are nonzero and bounded by
`MAX_CONVENE_QUERIES` (1024) and `MAX_CONVENE_OPERATIONS` (128). Reconnects,
clones and replacement facades share those same allowances. Dropping the grant
withdraws every port and queued or taken ticket; the same owner cannot select
another grant to refill it. Neither weak handle keeps the owner alive.

`next(request, round)` spends one query and reads the original acknowledged
state without source I/O, helper work or a clock sample. An actionable plan binds
the complete effect scope, external request and generation, original attempt,
last applied held round, journal revision, control sequence, authority epoch,
input revision, recorded elapsed tick and unchanged action deadline. The chosen
successor round is globally unused in the original broker and grant.

Eligibility requires an ordinary, unmediated, non-generated owner, no active
review session, an unsuspended and unstopped Reviewing action, and an actually
applied Reviewing-to-Reviewing `HoldEffect` at the current control cut. The latest
native Begin for that attempt must be that successful held round. A later
failed review cannot borrow an older Hold. Recorded time must still precede the
original deadline; this historical check does not replace launch-time clocks.
Authorized actions, recovered/fenced histories, learned or topology-mediated
reviews and unfinished congresses do not use this continuation.

`convene(FileConveneRequest)` compares that exact predecessor again and admits
at most one pending operation. It spends one original operation slot and reserves
the successor ID in a bounded process-local set, but changes no native authority
or journal. A repeated identical request observes the original operation;
another predecessor or request using its round ID is an idempotency conflict.
Rejected admission does not spend an operation. An admitted operation is never
refunded, including source failure, launch failure or abandonment.

## A queued request is not an opened round

The receipt reports one of four historical admission dispositions:

| Outcome | Meaning |
| --- | --- |
| `Pending` | The original finite grant admitted the request; no source or helper work is claimed. |
| `Opened` | The original driver captured evidence, acknowledged native Begin and successfully launched the selected helper round. Metadata names its actual journal/input revisions and start tick. |
| `Failed` | The one attempted handoff failed. Committed observations, a possible native Begin and retained child cleanup remain real; there is no automatic retry or refund. |
| `Abandoned` | A taken one-use ticket was dropped without completing its launch. Its operation and round ID stay spent. |

An exact retry can observe the same operation progressing from Pending to its
terminal disposition. It never enqueues a second operation or starts a round.
These records disappear with their original live grant. They are not a second
durable ledger and do not claim recovery of queued work after restart. The
actual source capture and Begin use the existing native events and replay.

## One-use handoff through the original driver

`FileSupervisedDriver::take_pending_convene(request)` returns a sealed,
nonclone `FileConveneTicket` only when the driver has no job and every direct
helper child has been reaped. It reads neither evidence nor time. The ticket
cannot be reconstructed from a plan, receipt, serialized request or public basis.

`start_file_convened_review` and `start_file_convened_process_review` check the
ticket's original owner, current grant, exact held predecessor, request, round,
input revision and bounded review window before capture. They then use the
existing checked file launchers to read a fresh complete source and start the
original helper protocol. No old transcript or helper vote is reused. The
host preserves the original fixed helper recipe, source configuration and
overall deadline; neither the port nor ticket chooses a helper program.

Fresh evidence may have the same bytes, for example when independently operated
helpers recover from an outage. Availability changes, edits, actor polling and
another Hold never request a successor. Each successor still needs explicit
independent admission, consumes its original cumulative operation allowance and
creates a new frozen round while all previous review history remains available.

A completed Continue only reaches the existing independent human-review path.
It does not publish in that step, reuse human approval or mint an effect key.
Original authorization, fresh source checks, two-key dispatch, liability
accounting and deadline checks remain in force. Stop and changed control/input
predecessors invalidate an unconsumed ticket before source/clock work.

Hosts use `next_review_round(preferred)` for initial and successor round
selection. Its finite scan excludes every native started round and every
admitted successor ID, even after grant revocation, failed launch or abandoned
handoff. External request numbers therefore cannot collide with an earlier
successor's native review ID. The allocator observes; it does not reserve a
round or expand either native or grant capacity.

## Verification boundary

Nine authored public integration tests cover actual Hold and fresh independent
publication, same-byte evidence, absence of automatic or actor-triggered retry,
source loss with spent admission, abandonment, revocation and nonrenewal,
foreign/lost owners, complete predecessor drift before capture, finite shared
query/operation allowances, round-ID exclusion and a later failed review's
inability to borrow an old Hold. Their helper ballots are controlled fixtures,
not evidence about model reliability.

Independent source review and qualified-nightly syntax parsing passed for the
native source and test files. Compilation and runtime of this increment remain
unverified: the existing library compiler was killed by the environment while
emitting metadata, before reaching integration targets. No whole-repository,
RCH, production or roadmap qualification is claimed.
