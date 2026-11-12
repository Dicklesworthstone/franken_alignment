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

## Authenticated Tower and live service

The registered `convene` verb connects only through a restricted original
control-operator mount with Permit purpose and its own same-owner
`FileSupervisorConvenePort`, installed with `set_convene`. Actor, ordinary
observer, legacy reconciliation, imported-snapshot and foreign-owner routes do
not gain this capability. Learned and generated-text service profiles reject
its selection, as do topology-mediated ordinary create/open/schedule profiles.

At the exact native request address, send `next` with
`{"action":"convene_held","round":"8"}`. All Tower integers are canonical
decimal strings. An actionable `convene-next` response contains the complete
generation-bound `convene` document; inspect and explicitly submit it. The
mutation requires the same canonical round idempotency key, an explicit nonzero
mount generation and its full eleven-field native predecessor. No caller clock,
source contents, helper recipe or claimed review verdict is accepted.

The closed response schemas are `fa.publication-convene-next/1` and
`fa.publication-convene/1`. Both bind the original request/round, registration,
query correlation and scope `original_held_review`. They explicitly report
`live_evidence_read`, `clock_read`, `effect_authority`, `payload_disclosed` and
`automatic_retry` as false. Planning returns one connected action with unknown
cost, information gain, ranking and current publication eligibility. Mutation
reports `queued_work_is_durable: false` and a known admission disposition:

```json
{"outcome":"pending","round_opened":false,"opened":null}
```

Only an actually completed native launch can report `outcome: "opened"` and
`round_opened: true`. Its `opened` object carries the actual journal revision,
input revision and start tick, all as decimal strings. Missing original request
knowledge is `Unknown(outcome_unknown)`; unavailable owner knowledge is
`Unknown(controller_unavailable)`. Neither is encoded as a successful plan.

Select the optional member in the existing independent operator profile:

```json
"convening": {
  "queries": 32,
  "operations": 2,
  "authority": "convene_held_review"
}
```

The ordinary Linux Tower service retains this original grant across requests
and reconnects. With it selected, an actual Hold waits within the original
action/service deadline, servicing the same independent Stop, operator and
actor endpoints. The fixed helper program/argument/environment recipe and
checked-publication preparation are retained from before the initial review.
Every successor still requires a separate explicit operator admission. Without
this selection, the existing post-Hold cancellation behavior is unchanged.

The service consumes one ticket only after old helper children are reaped,
builds a fresh bounded review window within the unchanged lifetime, captures
fresh evidence and launches the original helper protocol once. If a later
scheduled request prefers a round ID used by that successor, the bounded
original allocator chooses a free ID; external request identity is unchanged.
Operator retirement withdraws unconsumed work, and deadline or capture/launch
failure follows the original failure cleanup. Terminal replies cannot repeat
an already attempted automatic Stop/drain. A failed authenticated Stop
application keeps its original reply custody; a protocol-only failure still
requires the original automatic failure Stop.

Use `supervise_publication tower-operator-exchange OPERATOR_PROFILE JSON` to
send each selected document. The client validates exact scope, predecessor,
round/generation, complete returned action, envelope and disposition metadata
before emitting a reply or sending a later supplied document. It accepts
Pending as known admission, never as opened work, and cannot select or resend
an action on its own. These authenticated response checks are correlation and
protocol validation, not independent proof against a dishonest supervisor.

## Verification boundary

Nine authored public integration tests cover actual Hold and fresh independent
publication, same-byte evidence, absence of automatic or actor-triggered retry,
source loss with spent admission, abandonment, revocation and nonrenewal,
foreign/lost owners, complete predecessor drift before capture, finite shared
query/operation allowances, round-ID exclusion and a later failed review's
inability to borrow an old Hold. Their helper ballots are controlled fixtures,
not evidence about model reliability.

The connected increment adds five public Tower and five authenticated client
tests, five substantive service/profile tests plus one helper subprocess entry
fixture, and one causal Stop-control test. They cover exact codecs and sealed
responses; absent, actor and foreign grants; real Pending-to-Opened and fresh
two-key publication; malformed response suppression before later documents;
plain/checked live publication across two scheduled requests; unchanged-source
helper recovery without automatic retry; missing-source launch failure;
independent Stop and original deadline while held; nonrenewable profile bounds
and unsupported startup refusal; and one-time cleanup
through single-request and scheduled terminal replies. The protocol-only Stop
refusal test also asserts that automatic failure cleanup remains necessary.

Independent source review and qualified-nightly syntax parsing passed for the
changed native, Tower, client and service source/test files; new files are
formatted. Compilation and runtime of these increments remain
unverified: the existing library compiler was killed by the environment while
emitting metadata, before reaching integration targets. No whole-repository,
RCH, production or roadmap qualification is claimed.
