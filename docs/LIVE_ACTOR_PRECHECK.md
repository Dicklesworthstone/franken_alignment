# Exact prechecks in the live actor service

The executable dispatches `actor-precheck` before any shared supervisor option
processing. Single-query, batch, first-pass, capability and policy modes all
retain their complete public-client grammar; a private or live-service option
cannot be stripped into an accepted query.

The Linux ordinary single-request publication service can explicitly provision a
separate exact-precheck endpoint. It uses the original source-backed disclosure,
bounded wire, peer authentication and one-shot client described in
[the native contract](ACTOR_EXACT_PRECHECK.md). No option means no endpoint and
no additional source/time bootstrap. This is an executable reference consumer
of the existing `precheck` vocabulary, not a new permission or policy evaluator.

## Supervisor opt-in

```text
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --precheck-profile PRECHECK_PROFILE
```

The ordinary precheck option is final after positional arguments. Existing open
modes can also use their independent credibility activation option. Ordinary
request schedules, multi-peer, receipt-only, native-transport topology and
standalone `create-generated` combinations remain unsupported. Native-text and
learned release selections have the explicit integrations below. An already
recorded selected request remains observation/reconciliation-only: requesting a
new disclosure for it is refused, without a source read or new grant. Its existing
open/recovery fence remains part of the ordinary owner contract.

Single learned actors and explicitly selected resident sequence entries have a
separate release-specific integration, described below. It never enables the
ordinary raw-proposal disclosure on a learned owner.

The four ordinary single-request modes also accept an explicit
`--tower-generation GENERATION` alongside `--precheck-profile`, in either option
order after the positional arguments. This changes the actor submission socket
to Tower while keeping the precheck socket, profile, original grant and lifecycle
described here. The same authenticated Tower session also receives the explicitly
selected exact-precheck port: its `precheck` and `capabilities` reads share the
native sidecar's one finite query allowance. Either protocol can perform the
read before an explicit `tower-exchange` proposal. Reconnecting, changing
protocols or retaining the Tower socket cannot renew disclosure. Even when the
sidecar has a full-policy profile, Tower receives only exact checks and
capability data; policy definitions and literals remain unavailable through that
mount. Retirement revokes both views without replacing the Tower session or its
retained request tickets. Actual Tower submission still reacquires source
evidence and performs both reviews. The request-sequence option remains
incompatible with precheck disclosure.
See [the Tower composition](TOWER_PEER.md#check-exact-policy-before-a-tower-proposal).

Explicit single-request Tower `serve-create` and `serve-create-checked` can also
select `--topology-profile TOPOLOGY_PROFILE`, using the existing simple witness
profile for the checked form. The original topology cut must be certified
before disclosure is exposed. Precheck requests and reconnects share one finite
grant and do not refresh topology. Actual submission reacquires policy evidence,
requires fresh helper/human review and both keys, and performs the original
final topology check. Uncached, cached and full-policy disclosure remain
available under their existing leakage contracts. Stop retains priority over
both ingress sockets.

Guarded single-new-request Tower recovery supports the same disclosure on
`serve-open` and `serve-open-checked`:

```text
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation GENERATION --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE --precheck-profile PRECHECK_PROFILE
```

The checked form includes its ordinary `WITNESS_PROFILE` positional argument.
Both forms can additionally select an independent `--operator-profile`. Recovery
requires the exact `fa.mediated-submit/2` checkpoint and original guarded history
custody. It preserves cumulative source work, acknowledges the original recovery
Fence, rebinds the independently selected next topology and inventory, and checks
the current cut before exposing a fresh disclosure. The configured policy epoch
must match the actual recovered owner after those transitions. Neither the
precheck nor the actor proposal is silently rebased. A recorded request refuses
a new disclosure and retains its acknowledged history/checkpoint without opening
live topology, inventory, evidence or witness inputs. Independent Stop can win
during replay before any new source or precheck bootstrap.

Recovery archives, precheck/actor/reviewer/Stop/operator endpoints, topology and
inventory inputs, the evidence source and authority store are checked for the
relevant canonical path collisions before recovery starts. An alias through a
different parent spelling does not permit the precheck listener to replace
custody. Actual submission and publication retain their fresh evidence, helper,
human, final topology and checked-witness gates. Ordinary schedules and
live-governor compositions remain incompatible with precheck disclosure.

The actor and precheck profiles are public connection context. The private
supervisor Config, evidence, journal and independent reviewer roles must not be
provisioned to the actor. Identity, scope and polling interval come only from the
independently selected ACTOR_PROFILE; a precheck request cannot override them.

```json
{
  "schema": "fa.actor-precheck-service/1",
  "policy_disclosure": "exact_predicates",
  "socket": "/srv/fa-actors/precheck-7.sock",
  "expected_policy_epoch": 0,
  "disclosure_id": 81,
  "query_limit": 16,
  "exchange_limit": 32,
  "candidate_limit": 16,
  "connection_limit": 8,
  "runtime_ms": 10000
}
```

The independently configured expected epoch is not inferred from a request or
server reply; zero is valid. An opened owner may have advanced its epoch through
the original recovery fence. The operator must select the resulting context;
the service does not rewrite either profile or proposal to make them agree.

The strict schema rejects unknown/duplicate fields and requires a nonzero
disclosure ID, 1–128 queries and entered frames, 1–1024 accepted candidates,
1–candidate_limit admitted connections, and a 1–3,600,000 millisecond lifetime.
The socket must be an absolute normalized path of at most 100 bytes, distinct
from actor/reviewer/stop sockets. Its parent must be supervisor-owned and not
group/other writable; the new socket is mode 0660. Existing files, sockets and
symlinks are never removed to make bind succeed. Cleanup checks inode/device.

The version-one schema shown here requires explicit `exact_predicates`; omission
of the option disables disclosure. Ordinary source-backed services also accept
explicit version two for bounded witness-sensitive caching and version three for
full-policy disclosure under its required leakage declaration. A mismatched
schema/disclosure is refused, never silently downgraded. Native and learned
release-specific services accept only the uncached version-one profile. These
schemas are not the complete signed E27 assurance profile or its production
qualification.

## Learned actor releases

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE \
  --actor-profile ACTOR_PROFILE --precheck-profile PRECHECK_PROFILE
```

The same option is available with an explicit generation continuation,
`--after-message PREVIOUS_REQUEST`, or `--finish-after-message PREVIOUS_REQUEST`.
An actor profile is required. Historical, receipt, maintenance and actorless
commands cannot acquire this disclosure. The complete grammar is checked before
any incompatible option can be stripped into another mode.

A [resident learned sequence](RESIDENT_LEARNED_PUBLICATION.md#exact-prechecks-for-independently-selected-releases)
selects the same capability per entry through
`fa.learned-publication-sequence/2`. Each entry requires an explicit
`precheck_profile` path or `null`; there is no invocation-wide precheck flag.
Version one remains unchanged. Every future selection and canonical endpoint
collision is checked before startup, and each selected actual owner epoch is
checked before that entry begins numerical work. The grant belongs only to the
original selected release and retires before its review. Reconnects cannot
replenish that grant, and a null entry cannot inherit an earlier disclosure.

Select the uncached `fa.actor-precheck-service/1` profile shown above. Learned
release disclosure does not accept the cached or full-policy service schemas.
The supervisor binds the original selected request and release kind, waits for
actual generation or explicit Finish preparation, then captures one fresh policy
source and exposes a bounded authenticated endpoint while waiting for the actor.

The actor checks the same canonical release intent produced by
`FileLearnedTextStreamActorPort::encode_release`, using the independently selected
current policy epoch for the **precheck** request. The ordinary Submit intent
keeps its original zero-valued envelope marker. A shorter deadline is permitted;
a different request, release kind, payload, target marker, units or an extended
deadline is rejected. The precheck query label remains independent of the
selected release request ID.

The native owner evaluates its actual frozen action: full cumulative stream
frame, confirmed prior messages, current message or Finish, actual target,
actual policy epoch and complete units. It does not evaluate the actor's 17-byte
intent as if that were the publication. The generated bytes and predicate trace
remain private. For example, an earlier message can make the cumulative frame
exceed policy limits even when the new message alone would fit.

A query neither admits the release nor reads source files, advances time,
changes the journal, starts helpers or issues a human/effect permit. The original
Submit path reacquires source evidence and performs normal admission and both
reviews. A source change after a pass can still deny publication. Owner mutation,
expiry, Stop or exhausted disclosure work retires the endpoint; malformed peer
traffic cannot invoke the outer Stop/drain path. The endpoint is retired before
the original release enters review. A recovered owner requires its actual new
policy epoch, with no automatic repair of the actor's independent context.

## Native generated Message and Finish

The native text service explicitly composes `--precheck-profile` with its
original `--native-text` selection:

```text
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --native-text RECIPE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --native-text RECIPE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --native-text RECIPE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --native-text RECIPE --precheck-profile PRECHECK_PROFILE
```

Use the uncached `fa.actor-precheck-service/1` profile with
`policy_disclosure: "exact_predicates"`. Versions two and three, cache selection
and full-policy disclosure are unsupported for native generated releases. Open
forms can additionally select one `--after-message PREVIOUS_REQUEST` continuation
or `--finish-after-message PREVIOUS_REQUEST`. Finish retains the preceding
message's original recipe. The independent operator may be selected with the
pair `--operator-profile OPERATOR_PROFILE --tower-generation GENERATION`; both
are required and the route generation must be nonzero. These complete option
pairs may be ordered after the positional arguments. Duplicate, conflicting,
scheduled, multi-peer, topology and governor selections refuse before startup.

The selected native generation must actually complete with its original reviewed
prompt, monitored Control stop and current numerical predecessor. Explicit
Finish uses the receipt-confirmed stream prefix and advances no inference.
Only then does the supervisor capture a fresh registered source and disclose the
one selected original action. The actor still receives the original Submit
document containing a fixed 32-byte source intent; native precheck adds no raw
output or arbitrary-message submission path.

Construct a separate strict `fa.actor-precheck/1` query using the emitted Submit
document's original `target`, `payload_hex`, `units`, `deadline` and
`expected_policy_epoch`, with `operation: "precheck"` and an independently chosen
`query` label. Do not pass the Submit document itself to `actor-precheck`. The
query wrapper carries neither the Submit `version` nor its outer `request`
field; the source intent already binds the selected request. A query may shorten
the selected deadline. It cannot change the source request, generation/revision,
Message/Finish kind, target or fixed intent units. The independent precheck
profile must select the actual current owner epoch, including recovery changes;
unlike the learned release envelope, the native Submit epoch is already that
actual epoch.

The policy check evaluates the actual cumulative release frame and its full
charge, including all confirmed prior messages and the selected Message or
Finish. It never treats the 32-byte reference as the publication payload or
resource charge. Querying performs no additional inference, sampling, source or
clock observation, journal mutation, helper review or human approval. Bootstrap
can append the original source/time observations; queries cannot repeat it.

The original Submit must still acquire fresh source evidence and obtain both
helper and independent human review keys. A changed source can deny submission
after a pass, and checked forms retain their final witness checks. An already
recorded request cannot acquire this new disclosure during native recovery.
Endpoint/source/store aliases, including the optional operator endpoint, are
checked canonically before model loading, clock access or durable startup.
Independent Stop runs before bootstrap and each intake turn. The shared sidecar
lifecycle revokes the grant before review, on owner mutation, clock rollback,
lease expiry or service exit, and on terminal framing/transport faults.
Sidecar-only faults do not invoke the native workflow's Stop/drain or prevent a
later valid actor submission.
See [the native service](NATIVE_TEXT_PUBLICATION_SERVICE.md#exact-policy-prechecks-before-submission)
for the actor workflow and its separate source-reference file.

## Actor-only command

```text
supervise_publication actor-precheck ACTOR_PROFILE PRECHECK_PROFILE PRECHECK_JSON
```

The command accepts exactly these three files and no supervisor options. The
top-level dispatcher preserves that grammar before it can remove a credibility,
durability, topology or other option. It opens neither private Config nor the
evidence/journal. A request is the existing strict `fa.actor-precheck/1` document,
not a Submit command; its query label does not have to equal the ordinary actor
request ID. For the sample target in the publication fixture:

```json
{
  "schema": "fa.actor-precheck/1",
  "operation": "precheck",
  "query": "9001",
  "target": {
    "adapter": "10",
    "object": "11",
    "contract_version": "1",
    "expected_version": "1",
    "generation": "1"
  },
  "payload_hex": "6e6574776f726b",
  "units": "7",
  "deadline": "100000",
  "expected_policy_epoch": "0"
}
```

These are illustrative fields, not current deployment clocks or target IDs.
Use an independently chosen current target, epoch and deadline in the deployed
clock domain. Full-width IDs are decimal strings in the request, while the
strict public profiles use JSON integer fields. The command does not repair a
stale request, select another target or synthesize a deadline.

It validates the request's epoch against the public precheck profile, connects
only to the separate precheck socket and authenticates the connected supervisor
before sending proposal bytes. The original native client binds query, epoch
and closed response schema; the command additionally requires every Known result
to name the selected `disclosure_id` before printing it.

There is exactly one native exchange, without polling a new query, reconnecting
or retrying. Lifetime work is capped at 4,096 socket I/O attempts, one native
maximum request plus newline, and one response ceiling plus newline and a single
trailing-data probe byte. Partial I/O, Interrupted and WouldBlock spend the same
remaining lifetime allowance. One elapsed allowance starts before profile reads
and uses the smaller of the actor and precheck runtimes.

Stdout is only the original newline-terminated native response. A zero exit
status requires `Known(PrecheckPassed)` and still means `review_required: true`,
not admission or permission. A valid denial, Unknown, Withheld, Stale or native
wire refusal is printed unchanged and returns nonzero. Authentication, input,
binding and transport failures before output do not print a fabricated result.
The query may nevertheless have spent server work before a reply was lost.

File reads, pathname connection and stdout writes are synchronous cooperative
boundaries, as in the existing operator, not preemptible deadlines. Time is
checked around them and native drives. A failed or late-returning stdout write
can leave a partial or complete response visible despite a nonzero exit. Treat
that as an output/transport failure, not a reason to resend automatically.

## One bootstrap, no query-triggered renewal

After owner/deployment checks and independent stop setup, an initial stop
checkpoint runs before source acquisition. The supervisor then explicitly calls
the original `prepare_file_intake` once, acknowledges the source and clock,
checks the post-read lease and actual acknowledged clock, and retains one native
disclosure custodian for that exact owner revision. This operator bootstrap can
append ordinary source/time events. It is not a precheck request, and no actor
frame can ask for another bootstrap.

Every sidecar scheduling turn checks the original revision, half-open source
lease, monotonic scheduler-clock floor and elapsed allowance before any accept
or drive. Its lifetime is bounded by the source lease, profile runtime and
containing workflow deadline. A clock rollback, owner change or expiry
permanently retires the sidecar; a later clock or connection cannot revive it.
Results still describe their captured `observed_at`, not an assertion about a
future submission or an unobserved producer change.

At most one candidate is accepted per turn, and the native drive enters at most
one frame with bounded socket work. Kernel UID/GID and optional PID checks happen
before request bytes are read. Candidate attempts, admitted connections, entered frames
and native queries retain their original separate lifetime allowances across
reconnects. Complete malformed requests spend parser work, not a new source read;
lost replies never refund a completed evaluation.

Truncated/overlong frames, terminal transport failures and exhausted frame,
candidate or connection limits close only this endpoint and revoke its disclosure.
Native query exhaustion returns `Capacity` under the remaining frame/service
bounds; it does not allocate another allowance. These failures do not call the
ordinary workflow's stop/drain, create a request, cancel work or append a journal
event. Aggregate diagnostics contain retirement reason and counters, not proposal
content. Independent stop is serviced first on each ordinary intake turn.

The original source-aware Submit path is unchanged: it reads the source again,
consumes its own one-use admission snapshot, then performs ordinary exact
admission, helper review and independent human approval. A source that changes
after a passing precheck can therefore deny the actual submission. The sidecar
retires before review begins and on every service exit, including reply grace.
Checked modes keep all of their later publication-witness checks.

## Declared leakage and authority boundary

The service schema selects the native disclosure contract and its fixed leakage
boundary. It does not measure away the residual channels:

| Surface | Disclosed | Not disclosed or conferred |
| --- | --- | --- |
| Exact result | Coarse pass or denial; repeated bounded queries can infer policy constraints | Predicate trace/literals, source values, helpers or votes |
| Correlation | Query/disclosure labels, policy epoch and captured observation tick | Internal control sequence, actor ticket or reserved request ID |
| Refusal | Closed malformed/capacity/unavailable/withheld/stale states | Private failure paths or fallback authority |
| Timing and closure | Connection lifetime, response timing and whether disclosure remains available | No claim of timing-channel elimination |
| Passed result | Native `precheck: true` and `review_required: true` | Permit, reservation, approval or effect execution |

SO_PEERCRED identifies the connection peer's kernel credentials, not executable
identity, a human or the later holder of a transferred descriptor. Namespace,
file and credential separation remain deployment responsibilities.

## Verification boundary

Component regressions use real durable owners and authenticated local sockets;
service regressions exercise a precheck followed by the original helper/human
publication workflow, changed source before actual Submit, and malformed
precheck traffic followed by successful ordinary execution. They also cover
strict configuration, no-clobber paths, finite work, lost responses, lease/clock
expiry and unchanged durable state during queries.

The operator-client increment adds fourteen client/dispatch regression functions
plus two real client-to-sidecar integration functions. They check authentication
before bytes, complete strict/correlated output, independent disclosure binding,
lifetime work/elapsed bounds, no resend on loss/output failure, unchanged owner
state, and fresh ordinary admission after a real successful operator precheck.

The native generated increment adds original inference/Message/Finish coverage,
full-frame byte and charge checks, immutable source-intent binding, finite
disclosure/revocation, source changes after a pass, incomplete generation and
typed epoch mismatch. Same-session Tower and guarded topology
recovery regressions cover shared work across protocols, retained tickets,
exact-only narrowing, fresh final checks, cumulative recovery custody and
independent Stop priority. Fixture model outputs and synthetic reviewer decisions
test these contracts; they do not establish trained detector quality.

The operator remains synchronous and cooperatively scheduled, not a new executor
or hard real-time service. Clock checks do not preempt filesystem, kernel or
evaluation work already entered. Source authenticity, deployment mediation and
full assurance-profile qualification remain separate. Central Rust syntax checks
passed for the latest increment. Compilation, Clippy and runtime tests remain
pending the required RCH verifier: its commands cannot start in the editing
environment because `rch` is absent (exit 127). Syntax and source review do not
claim a test-passing or production-qualified result.
