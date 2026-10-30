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
source refresh and witness caching, captured-cut observations, label-only
correlation, observable timing, no signed profile, no effect authority and required
ordinary review. These are compiled endpoint facts, not imported assertions.
Unsupported full disclosure or congress dry runs cannot silently masquerade as
this endpoint. The response cannot be decoded as a precheck pass or vice versa.

The original precheck and discovery codecs share one epistemic envelope decoder,
including strict query/epoch/basis correlation and refusal handling. The old
precheck format and its original parser limits are retained. A capabilities
response uses the same 1,024-byte ceiling with its own finite item allowance.
Maximum-width schema vectors fit in 256/1,024 bytes respectively.

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

Operator CLI integration remains separate. This is not the full signed E27
assurance-profile implementation, a deployment
attestation, an instance nonce, a source-renewal interface or witness caching.
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
These tests are authored, not executed: the required RCH test/check/Clippy commands
cannot start (`rch` absent, exit 127); formatting cannot start (`cargo` absent,
exit 127). No compilation, runtime success, qualification or bead closure is claimed.
