# Source-bound exact-precheck capability discovery

`FileActorPrecheckPort::capabilities(query, expected_policy_epoch)` describes the
explicitly granted exact-precheck endpoint using the original `Knowledge` wrapper
and `FilePrecheckBasis`. Ordinary actor ports cannot self-grant this interface.
Discovery is not an exact-policy pass, admission, helper approval or effect key.

A Known result reports the remaining **shared** native query allowance after
charging this read. It is not a reservation. Discovery, exact evaluations, clones,
repeated labels and entered invalid queries spend the same finite allowance.
The same original-owner borrow, storage/source eligibility, revision binding and
custodian revocation protect both operations. Stale expected epochs retain Stale;
unavailable owners retain Unknown; revoked grants retain Withheld. None of these
reads refreshes a source, observes a clock, consumes the admission snapshot or
writes a journal event. Results describe the captured observation tick only.

The bounded codec is exposed under `precheck_wire::discovery`. Requests use
`fa.actor-precheck-capabilities/1`, operation `capabilities`, and decimal-string
`query` and `expected_policy_epoch` fields. The 256-byte input ceiling, duplicate
key rejection and closed fields prohibit embedded proposals, supervisor options
or caller-selected disclosure levels. Query zero is invalid; epoch zero is valid.

Responses use the separate `fa.actor-precheck-capabilities-response/1` schema and
`informational: true`. Known values explicitly declare `exact_predicates` /
`exact_policy`, withheld policy contents/helper feedback/source values, unavailable
source refresh, captured-cut observations, label-only
correlation, observable timing, no signed profile, no effect authority and required
ordinary review. These are compiled endpoint facts, not imported assertions.
Unsupported full disclosure or congress dry runs cannot silently masquerade as
this endpoint. The response cannot be decoded as a precheck pass or vice versa.

The original precheck and discovery codecs share one epistemic envelope decoder,
including strict query/epoch/basis correlation and refusal handling. The old
precheck format and its original parser limits are retained. A capabilities
response uses the same 1,024-byte ceiling with its own finite item allowance.
Maximum-width schema vectors fit in 256/1,024 bytes respectively.

Witness caching is disabled by default (`witness_cache: "unavailable"`). An
explicitly cached native grant, including the version-two live service profile,
reports `"exact_witnesses"` instead. The current decoder accepts exactly these
two spellings; older decoders fail closed on the new spelling. This reports
selection, not a hit or reserved work. Discovery never populates the cache or
resets query allowances. See [the cache contract](ACTOR_PRECHECK_CACHE.md).

## Authenticated live transport

The original `FilePrecheckWire::exchange_frame` dispatches both precheck and
capabilities documents through one strict parse and the same entered-frame
allowance. Its original typed `exchange` remains precheck-only. Malformed,
unclassifiable and exhausted-wire refusals retain the original error envelope;
a discovery client rejects a different schema rather than inventing a result.
Valid discovery replies, including native query exhaustion, use their own schema.

The original newline channel, Unix transport and authenticated peer session now
carry either operation. No second listener, grant or replenishable allowance is
created. Fragmentation, one-response write/flush backpressure and explicit
reconnection preserve both original counters. The ordinary live sidecar's source
lease, owner-revision, elapsed and stop checkpoints protect discovery too.

On Linux, `PrecheckCapabilitiesClient::connect_capabilities` uses the same
one-shot authenticated I/O engine as `PrecheckPeerClient::connect`. Constructors
pin the response codec; a capabilities response cannot become a precheck pass.
The independently configured epoch must match the request, and kernel peer
credentials are verified before any request bytes. The same byte/frame/I/O
budgets, trailing-data probe and terminal closure apply. No reconnect, retry or
source renewal occurs, even when a response was lost after charging server work.
The host still owns connection establishment and cumulative elapsed/work bounds.

## Actor-only operator command

```text
supervise_publication actor-precheck --capabilities ACTOR_PROFILE PRECHECK_PROFILE QUERY
```

`QUERY` is a nonzero canonical decimal u64 label, not an actor request ID or an
input pathname. The closed grammar is checked before any files or shared
supervisor options can be consumed. Only the two existing public connection
profiles are read. The command opens no proposal, private Config, evidence or
journal. It connects only to the already provisioned precheck endpoint; it cannot
start that endpoint or grant itself disclosure. The existing supervisor
`--precheck-profile` opt-in and its private source bootstrap are still required.

One authenticated native discovery exchange uses the independently configured
policy epoch. A Known response must also bind the selected disclosure ID before
stdout. Output is the original newline-terminated native response, with no
wrapper or generated capability assertions. Known information returns zero,
including a count of zero remaining queries; this is not a successful policy
precheck. Valid Unknown, Withheld, Stale and native refusals are printed unchanged
and return nonzero. Authentication, correlation, framing and transport failures
print no fabricated response and never fall back to Submit or precheck.

One lifetime work allowance caps the command at 4,096 socket I/O attempts,
257 request bytes and 1,026 response/probe bytes. The elapsed allowance begins
before reading profiles and uses the smaller configured runtime. It is checked
around native drives, connection, file and output boundaries. Synchronous file,
connect and stdout operations are cooperative, not preemptible deadlines.
A failed or late output operation can leave partial or complete response bytes;
it does not cause a retry. Discovering capabilities spends the original server
query/frame/connection allowances: it does not reserve work for a later batch.

## Verification and remaining scope

This is not the full signed E27 assurance-profile implementation, a deployment
attestation, an instance nonce, a source-renewal interface or cached congress review.
The disclosed remaining count and timing can reveal other use of the same grant;
credential/scope separation remains the operator's responsibility.

Fourteen regression functions and one compile-fail example cover real source-backed
owners, unchanged durable/source/request state, later ordinary review, shared
exhaustion, revocation/unavailability, strict codecs, full-width IDs, refusal of
stronger claims and byte compatibility with the original precheck outcomes.
The transport functions add mixed-verb accounting, parser-only refusals,
fragmentation/flush barriers, actual authenticated one-byte drives, preserved
ordinary review, no bytes before authentication, lost-reply accounting, response
correlation/type separation and malformed/truncated/extra response boundaries.
Eight operator regression functions additionally cover public-profile-only
reads, unchanged native output and exit semantics, selected disclosure binding,
strict top-level grammar and full-width labels, authentication-before-bytes,
malformed/lost replies, lifetime I/O/elapsed exhaustion and partial output.
These 22 tests are authored, not executed: the required RCH test/check/Clippy commands
cannot start (`rch` absent, exit 127); formatting cannot start (`cargo` absent,
exit 127). No compilation, runtime success, qualification or bead closure is claimed.
