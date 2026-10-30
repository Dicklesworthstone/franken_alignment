# Explicit policy definitions in the live actor service

The existing ordinary single-request `--precheck-profile` service can opt into
full deterministic policy-definition reads. This uses the original source-bound
owner, one cache, one finite grant, original authenticated transport and original
stop/revision/lease/deadline guards. Without the option, disclosure remains
disabled; v1/v2 profiles remain exact-only. This is the scoped native contract in
[ACTOR_POLICY_DISCLOSURE.md](ACTOR_POLICY_DISCLOSURE.md), not a signed E27 profile
or a claim that the operator is production-qualified.

## Supervisor selection and declared leakage

Use the existing serve-create, serve-open, serve-create-checked or
serve-open-checked command with a final `--precheck-profile PROFILE` option.
The new `fa.actor-precheck-service/3` schema requires full disclosure, explicit
cache limits, and a complete closed leakage declaration:

```json
{
  "schema": "fa.actor-precheck-service/3",
  "policy_disclosure": "full",
  "socket": "/srv/fa-actors/precheck-7.sock",
  "expected_policy_epoch": 0,
  "disclosure_id": 81,
  "query_limit": 16,
  "exchange_limit": 32,
  "candidate_limit": 16,
  "connection_limit": 8,
  "runtime_ms": 10000,
  "cache_entries": 16,
  "cache_bytes": 262144,
  "leakage": {
    "policy_structure": "disclosed",
    "policy_literals": "disclosed",
    "source_values": "withheld",
    "helper_feedback": "withheld",
    "consequences": "withheld",
    "timing": "observable",
    "query_usage": "observable"
  }
}
```

These numbers and paths are examples, not current deployment identities or
clocks. Select the independently expected live epoch and current endpoint.
The existing public actor profile supplies peer identity, scope and polling.
Profile version three never infers missing leakage/cache fields, accepts weaker
leakage claims, upgrades v1/v2, or silently falls back after grant setup failure.
It uses the same cache entry/byte and query/frame/connection/runtime bounds.
The native full port can be uncached, but this operator profile deliberately
requires explicit bounded cache configuration.

**Full includes literal bytes.** Operators must review those bytes for secrets
before granting the profile. The definition can disclose keys, target literals,
policy generation and the complete Boolean graph. It does not include the actual
source snapshot, helper roster, votes, empirical review, dynamic consequences or
resource ledger. A literal may itself contain sensitive operator data; the
withheld-source claim does not sanitize literals. Revocation cannot retract
bytes already delivered. Timing, policy size and shared query use remain visible.

## Actor-only command

```text
supervise_publication actor-precheck --policy ACTOR_PROFILE PRECHECK_PROFILE QUERY
```

`QUERY` is a nonzero canonical decimal u64 label, not a request ID or proposal
file. The closed grammar is validated before profile reads or shared supervisor
option removal. Only the two public connection profiles are opened; no private
Config, evidence, journal or proposal is read. The selected profile must explicitly
permit full disclosure. A full public profile on the client cannot upgrade an
independently configured exact-only server: the native server returns Withheld.

The command sends one authenticated `get` query through the original one-shot
client. The independent epoch, query and closed response type are checked by the
native codec; every Known result must also match the selected disclosure before
stdout. Stdout is the original native JSON response plus one newline, not a
rendered summary, permit or local evaluation. Known definition returns zero;
valid Unknown/Withheld/Stale/native refusals render unchanged and return nonzero.
Malformed, wrong-type, uncorrelated, truncated, overlong and trailing responses
produce no fabricated policy or success. There is no retry, reconnect, grant
renewal, fallback to exact precheck or automatic Submit.

One elapsed allowance starts before reading profiles and uses their smaller
runtime. One cumulative socket allowance is 4,096 I/O attempts, 257 request bytes
and 196,610 response/framing/probe bytes. Partial I/O cannot reset it. File reads,
pathname connection and stdout are synchronous cooperative boundaries, not hard
preemption. A failed or late stdout operation can leave a partial/complete
response visible with a nonzero exit; this does not cause a resend.

Full reads, discovery and exact prechecks share the server's original query and
frame budgets. An informational result does not reserve work for a later query.
Discovery reports actual native full selection; a public profile alone enables
nothing. Exact precheck, batch and first-pass clients still receive the original
bounded coarse results. Their ordinary admission/review requirements are unchanged.

Actual Submit always acquires fresh evidence. Unchanged evidence still enters
helper/human review; changed evidence may deny a proposal after the actor read
the policy. Any original-owner transition invalidates the old grant and retires
the sidecar. Source lease expiry, clock rollback, wall expiry and supervisor exit
remain terminal. No policy read appends a journal event or runs a helper.

## Verification boundary

Eight service/profile regression functions cover strict leakage declarations,
actual actor-command-to-service definition reads (including the maximum literal
size), unchanged owner/source state, an exact-only server refusing the same client,
fresh-source admission versus denial, pre-bootstrap limit refusal,
lease/clock/wall/supervisor retirement, partial stdout without resend, and
peer/disclosure mismatch.
Four actor-command functions cover closed top-level grammar and full-width labels,
unchanged refusal output, disclosure binding before output and cumulative I/O or
elapsed exhaustion with exactly one transmitted request.

These twelve functions are authored, not executed. Required native/example and
canonical RCH checks could not start because `rch` is absent (exit 127); formatting
could not start because `cargo` is absent (exit 127). Whitespace, original-source
hash checks and independent JSON fixture checks are not compilation or runtime
evidence. No bead, production qualification or complete FA-142 acceptance is
closed. Signed profiles, deployment qualification and the general assurance
profile's explicit `none` representation remain outside this increment; omitting
the sidecar option still grants no disclosure.
