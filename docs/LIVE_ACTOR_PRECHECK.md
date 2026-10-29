# Exact prechecks in the live actor service

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

The precheck option is final after ordinary positional arguments. Existing open
modes can also use their independent credibility activation option. Sequence,
multi-peer, receipt-only, topology, generated, learned and native-text combinations
are refused before another service can consume or ignore the option. An already
recorded selected request remains observation/reconciliation-only: requesting a
new disclosure for it is refused, without a source read or new grant. Its existing
open/recovery fence remains part of the ordinary owner contract.

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

This scoped service schema implements only explicit `exact_predicates`; omission
of the option disables disclosure. `none` and `full` are not alternate values
silently treated as exact disclosure. This is not the complete signed E27
assurance-profile schema or its production qualification.

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
before request bytes. Candidate attempts, admitted connections, entered frames
and native queries retain their original separate lifetime allowances across
reconnects. Complete malformed requests spend parser work, not a new source read;
lost replies never refund a completed evaluation.

Truncated/overlong frames, terminal transport failures and exhausted service
limits close only this endpoint and revoke its disclosure. They do not call the
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

The operator remains synchronous and cooperatively scheduled, not a new executor
or hard real-time service. Clock checks do not preempt filesystem, kernel or
evaluation work already entered. Source authenticity, deployment mediation,
full assurance-profile options and witness-sensitive caching remain separate.
The new source and tests are pending execution: required RCH commands cannot
start in the editing environment because `rch` is absent (exit 127). No compiled,
formatted, Clippy-clean, test-passing or production-qualified result is claimed.
