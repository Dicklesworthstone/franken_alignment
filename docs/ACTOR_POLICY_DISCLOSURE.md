# Explicit full exact-policy disclosure

FA-142 / plan §17.8: full disclosure exposes the deterministic policy definition,
including literal bytes, only after the supervisor explicitly grants a distinct
`FileActorPolicyPort`. It does not expose empirical review, helper identities,
source snapshot values, dynamic consequences, or a permit. This is a scoped
reference implementation, not the complete signed E27 assurance profile.

## Native capability

`disclose_full_precheck(revision, disclosure, queries, optional_cache)` uses the
original source/revision checks and returns a full-policy port and the original
revocation custodian. Existing `disclose_exact_precheck` and cached exact grants
are unchanged; an ordinary actor or exact-only port cannot upgrade itself.
`prechecks()` deliberately narrows full access to an ordinary exact-check handle.
All resulting handles share one original finite query allowance and revocation.

`policy(query, expected_policy_epoch)` returns the original `Knowledge` wrapper
with the captured grant's basis. It spends a query before entered validation or
cloning, without evaluating a proposal, reading a file or clock, consuming an
admission snapshot, running a helper, or appending a journal event. Busy, missing,
stale, revoked and source-ineligible owners retain the original refusal behavior.
A cache remains bound to its original owner; reading a policy does not evaluate
or populate it. A subsequent ordinary Submit still performs ordinary review.

The returned definition is the exact original `Policy`: its generation and all
predicate nodes, including unselected Boolean arms. This is not an inventory of
every dynamic admission condition. Current stop/narrowing consequences, resource
ledger contents, source values and empirical review remain private. Possessing
the definition or evaluating it against caller-supplied data cannot authorize an
effect. Revocation prevents later reads but cannot retract bytes already disclosed.

## Declared leakage

| Surface | Full-policy capability | Exact-only capability |
| --- | --- | --- |
| Deterministic definition | Complete graph, generation, keys, target literals and literal bytes | No definition access |
| Source observations | Withheld; policy literals may themselves contain sensitive operator data | Withheld |
| Helper identities, votes, empirical review, consequences | Withheld | Withheld |
| Timing, correlation, allowance use | Observable; same finite shared grant | Existing behavior |
| Effect authority | None; ordinary admission and review still required | None |

Operators must review literal contents before selecting full disclosure. This
explicit choice can reveal secrets embedded in a policy. No renderer executes
policy text, shell commands, helper requests or imported claims.

## Bounded representation

The closed `fa.actor-policy/1` request uses operation `get`, a nonzero canonical
decimal-string query, and an independently expected policy epoch (zero allowed).
Its ceiling is 256 bytes. The separate `fa.actor-policy-response/1` informational
response cannot be decoded as a precheck pass. It binds query, epoch, disclosure
and observation time through the same original epistemic envelope implementation.

A definition encodes every predicate using closed op names, exact decimal-string
identifiers/limits/edges, and lower-case hex literals. Decoding invokes the
original `Policy::new` graph validator: cycles, unreachable nodes, forward edges,
invalid ranges and original node/edge/literal/read limits cannot be bypassed.
The 196,608-byte response ceiling covers the complete admitted policy, including
64 KiB of hex-encoded literals; no truncation or lossy summary substitutes for it.
Legacy precheck and capability envelopes retain their original 1,024-byte bounds.

## Authenticated mixed transport

`FilePrecheckWire::with_policy` consumes the distinct full-policy port. The
ordinary constructor remains exact-only: a valid policy request receives
Withheld, not an implicit upgrade. Both paths use the original owner eligibility
checks and charge entered native reads; malformed frames spend only parser work.
Full policy, exact evaluation and capability discovery share one finite wire and
grant. Reconnection cannot replenish either. The legacy typed `exchange` stays
precheck-only even when its containing wire has full access.

On Linux, `PrecheckPolicyClient::connect_policy` uses the same authenticated
one-shot I/O engine with a separately selected response codec. Authentication and
independent epoch validation happen before request bytes. Only the full codec
allocates the 196,608-byte receive ceiling; old clients retain 1,024 bytes. The
channel also allocates its larger bounded reply storage only for a full wire.
Complete writes and flushes still precede further frames. Partial I/O, queued
trailing data, EOF, transport failure and terminal closure retain their original
semantics. The host still owns connection establishment and lifetime deadlines.

Discovery on a full port/wire reports the coherent pair `policy_disclosure: full`
and `policy_contents: available`; a deliberately narrowed exact handle reports
the original exact-only pair. All other secrecy and no-authority facts remain
unchanged. Older discovery decoders fail closed on the new pair. The declared
endpoint does not assert that a later query has reserved capacity or a live lease.

## Verification status

Ten new regression functions and one compile-fail example cover real source-backed
read-only behavior, ordinary submission review, owner/source revocation, shared
budgets and cache binding, every predicate, full-width IDs, maximum policy sizes,
closed grammars, invalid graph forms, private-field refusal and protocol separation.
They are authored tests, not executed evidence. The editing environment lacks
`rch`, `cargo` and a Rust toolchain; compilation, formatting, runtime tests and
qualification remain pending. No bead or acceptance gate is closed.

Seven additional real-owner/framing/authenticated-transport functions cover
exact-versus-full access, mixed operations, parser charging, flush backpressure,
lost output, ordinary review, owner changes, authentication and epoch binding.
Three scripted shared-engine functions cover maximum literal responses, the
exact byte ceiling, partial I/O without resend, malformed boundaries and
unchanged legacy response limits. These ten additional functions are also
unexecuted; no socket runtime or throughput result is claimed.
