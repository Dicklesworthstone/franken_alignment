# Live file evidence for the original mediation boundary

## Source capability (unqualified source, October 14, 2026)

`FileMediationObserver::bind_topology_source` consumes the separately held native
observer and binds a `FileTopologySource` to that exact live owner, two fixed
absolute paths and an independently supplied `LoadedAuthorityTopology`. The
source uses the original closed topology and perimeter-inventory loaders. The
initial graph must equal the owner's graph; binding performs no reads and grants
no certificate. No observer getter, cloned role or writable-owner alias escapes.

`refresh(host, revision, operation)` reopens both bounded regular files and
compares the COMPLETE loaded object. Unselected family metadata, route profiles,
provenance, proposed gates and declared limits cannot disappear into graph-only
equality. Only a matching input reaches the original durable independent cut
checker. A checksum, parser result or source object is never a permit.

Missing, malformed, oversized, substituted or changed files withdraw coverage
through the original `FileMediationUpdate { next: None }`. The original reducer
revokes approvals and refunds only undispatched reservations; dispatched or
unknown effects stay charged until the original endpoint supplies an outcome.
A bypass, unreachable sink or checker refusal is retained as the actual native
check result, then coverage is withdrawn rather than preserving an older cut.

The source reserves one check and both maximum file sizes plus EOF probes before
reading. Read/parse failures do not refund this allowance. Exhausting either
fixed lifetime dimension also withdraws coverage without another read. Work is
logical source admission, not physical I/O, wall time or process memory. The
checker's own work and journal reconstruction are additional native costs.

## Loss, interruption and replacement

An unwind guard quarantines the original owner across input acquisition,
comparison, native checking and withdrawal. A storage failure preserves the
original storage failure; failure before native persistence still leaves the
owner unavailable. `last_refusal` and `last_check` retain the source cause and
acknowledged checker result separately from a failed withdrawal. No stale live
certificate survives a failed source operation as a usable fallback.

Restoring identical old files does NOT reinstate withdrawn coverage. `rebind`
requires explicitly provisioned newer graph AND inventory generations, using the
original topology update before installing the new expectation. The next fresh
check must still succeed. Paths, source budgets, spent work and checker allowance
cannot be renewed through rebind. An exact acknowledged rebind retry returns its
original receipt without another epoch change; it does not revive later loss.

Caller predecessor/owner mistakes refuse before reads or revocation. Source
operations use increasing nonzero IDs within this custodian's lifetime. The
original durable topology update additionally rejects conflicting operation IDs.
The custodian keeps no live-owner borrow or file lock. Owner shutdown/recovery
changes the issuer, so an old custodian cannot act on the recovered owner.

## Boundary and usage

This serves FA-072/FA-073, plan 15.2 and 15.6, and founding FI-A02/FI-A06. The
consumer is the existing publication mediation observer, not another authority
or a background watcher. The supervisor must schedule fresh checks at the
relevant effect boundaries. Existing APIs are not silently changed into a new
mandatory deployment profile. Source custody/work is process-local; recovery
requires independently provisioning a new custodian for the new owner. Neither
source limits nor unselected inventory metadata acquire a new journal encoding.

The directory, provisioning authority and filesystem are trusted. Regular-file,
identity, extent and metadata checks detect ordinary replacement during one read;
they do not establish a hostile-filesystem or atomic two-file snapshot guarantee.
The files' claims about real routes and functioning enforcers remain declarations.
A fresh graph certificate proves only the original conditional graph property.
No source refresh authenticates current helper/policy inputs, grants either
publication key, relaxes a stronger configured guard, repairs history, or retries
an external effect. Reads, parsing, original verification and storage remain
synchronous. No throughput or latency improvement is claimed.

## Validation

Nine authored regression functions use original locked owners and real canonical
replacement, full-input congress review, both approval keys and endpoint results.
They cover successful publication, missing/malformed/oversized/symlinked inputs,
metadata-only changes, loss after dispatch, newer explicit recovery with fresh
publication, both source budgets, bypass/unreachable/checker exhaustion, stale
callers, wrong owners, read unwinds and all five native storage barriers. Manual
ballots are controls, not helper inference or detector-quality evidence. One
compile-fail example forbids cloning observer custody.

Rust tests and compilation are UNEXECUTED: the prescribed remote-only RCH launcher
is absent in this environment. Source hashes, whitespace checks and a lexical
screen cannot qualify Rust syntax, type checking, formatting, Clippy or runtime
behavior. No bead or production gate is closed by this source increment.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::mediation::source
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
