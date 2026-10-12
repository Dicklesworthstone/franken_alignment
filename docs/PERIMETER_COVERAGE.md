# Scoped logical perimeter coverage

`fa_reference::perimeter::coverage` implements a bounded, non-vacuous audit of
barriers in the existing `Graph` model. It supports the authority-topology and
effect-perimeter work in plan §§4.4, 6 and 15. It does not activate a production
adapter, authenticate an inventory or confer effect authority.

A caller declares one scope, projection, semantic epoch and topology generation,
complete directed graph, source and sink sets, and eligible barrier sites. The
constructor refuses an incomplete declaration. Sources and sinks cannot be
barrier candidates: removing the actor or protected resource is not credited as
installing a mediator. Eligibility remains a caller assumption.

`audit` returns each sink separately: already unreachable, mediated with an
original path establishing non-vacuity, or bypassed with a concrete surviving
shortest path. Sorted adjacency and source IDs fix ties. Unreachable sinks are
not included in the active-sink denominator. With no active sinks, certification
refuses instead of turning absent traffic into a positive coverage claim.

`certify` additionally consumes the existing independent `Graph::cut_disconnects`
oracle. `verify` binds the complete normalized graph and declaration, not only a
generation number or short hash, and re-executes the checks. The opaque result
cannot be converted into a permit. It is a logical certificate, not cryptographic
attestation, a portable live-state format, or evidence of complete host mediation.

The original graph limits remain: 128 nodes and 16,384 supplied edges. An audit
performs two breadth-first traversals and retains at most 128 paths of at most
128 nodes. It adds no dependency or executor and makes no latency/RSS claim.

## Source qualification

Eight public integration tests cover paired cut/bypass cases, cycles, multiple
sources, vacuity, zero-edge paths, endpoint exclusion, incomplete declarations,
full certificate binding, deterministic witnesses and all 4,096 directed graphs
on four nodes against the original cut oracle. A compile-fail example prohibits
conversion of the result into a permit. These Rust tests are authored, not run
in this session: neither the mandated RCH launcher nor a Rust toolchain is
available. No workspace, production, detector-quality or bead-close claim is made.
