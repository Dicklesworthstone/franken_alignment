# Original request-to-review discovery

`PublicationTower` connects `related` on an original publication request to a
complete, bounded projection of that request's recorded reviews. An operator
can discover the actual round and follow an explicit `explain` request without
enumerating the entire run journal and manually joining its events.

This implements part of FA-134 and the unified dispatcher under plan §§6.7,
7.3, 9.2 and 17.2–17.8. It serves FI-A06/FI-A14 and FI-I11 by preserving the
original investigation trail. The metadata describes acknowledged history;
the independent explanation grant still owns decision replay and disclosure.

## Native custody and limits

The original `FileActorSupervisor<FileOversight>` provisions
`request_history_port(queries, max_history_events, max_reviews,
FileRequestHistoryDisclosure::RecordedRequestRelationships)`. It returns a
weak port and a noncloneable custodian. Every clone shares the original
allowance. Dropping the custodian revokes disclosure; dropping or replacing
the original owner makes the port unavailable. Equal numeric scope cannot
substitute a reopened owner.

Each entered `history(request)` query consumes one query before inspecting
the complete acknowledged event history. The hard ceilings are 1,024 queries,
4,096 history events and 2,048 completed reviews. Host-selected limits can be
smaller. Exceeding either scan or result capacity returns a refusal rather
than a truncated result. Failure or an output limit never refunds the query.

The scan reads original request mappings, journal events and already retained
learned-completion metadata. It does not reread storage, replay numerical
inference, consume admission snapshots, run helpers or read the clock. It
does not disclose payloads, helper votes, source contents or model inputs.

## Historical relationships

A `Known` result binds its original scope, request and request generation,
journal revision, control sequence, authority epoch and recorded elapsed
observation. These coordinates remain distinct. Request generation zero is
valid; it is unrelated to the nonzero Tower registration generation.

The value contains admission status, its original attempt and recorded stage
when admitted, the count of review starts without a completion at this cut,
and all completed reviews for the selected attempt. An incomplete start may
be abandoned; the count does not assert an active worker or current readiness.

Completed rows carry their actual round and original begin/completion record
revisions, in completion order. `application_attempted` includes a rejected
application. `evidence_refined` records a completed learned refinement; it
does not assert that a consequence was applied or an effect was permitted.
Learned kinds come from the original retained completion, never from the
permission to refine or a new model execution.

A known unadmitted request has no attempt, stage or reviews. A known admitted
request can have a genuinely empty completed-review set. An unobserved request
is `Unknown`, so missing evidence cannot become a complete empty history.

## Tower request and response

Install the port with `PublicationTower::set_request_history`. The installation
must belong to the facade's exact original owner. A rejected replacement
withdraws the former port. Existing intake, Tail, situation or explanation
grants do not enable this additional disclosure.

Send the existing `fa.tower_request/1` envelope with `verb: "related"`, the
original request's `fa://.../attempt/request-...` address, and null arguments.
The native profile does not accept the imported snapshot's relationship
filter. A generationless query resolves under the current registration; a
stale explicit registration refuses before spending work. Only a separately
authorized operator can discover native relationships.

The sixteen-field Tower envelope carries `result.kind: "request-history"`.
The result's `fa.publication-request-history/1` profile includes its sealed
registration and query address, explicit historical/disclosure markers, and
the original native Knowledge or bounded error. The encoder also checks the
original operation ID and optional idempotency label. A public envelope edit
cannot move a result or refusal to a different request, operation or label.

Every completed row carries a complete `explain_request` using the existing
review selector and the fully generation-pinned original request address.
`LinkedRequestHistory::explain_requests()` returns the same typed requests
without performing any work. Unknown, withheld or failed discovery cannot
produce an empty successful link set.

Execute a selected Explain only under its own disclosure and replay budget.
The operation may be unavailable or exhausted even when discovery succeeded.
Discovery never executes its suggested reads, renews their allowances or
creates an approval, permit, source custodian or effect authority.

All recorded reviews fit the response or the complete response is refused.
The facade never removes rows to make a report fit. Retrying is another
explicit native query, not a refund or automatic fallback to a snapshot.

## Authenticated operator use

The optional `request_history` member in `fa.tower-operator-service/1` selects
the three finite native limits and `recorded_request_relationships` disclosure.
Omission or null preserves existing profiles. The original service retains one
custodian across reconnects and ordinary, learned or resident handoffs; a new
request cannot renew the allowance. A history-only learned service requires no
numerical replay grant or Permit purpose.

The following is a complete explicit request for the example scope and Tower
registration; substitute the original request address returned by the service:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "discover-request-7",
  "address": "fa://1/attempt/request-3-2-4-5-7@41",
  "verb": "related",
  "arguments": null,
  "idempotency_key": null
}
```

Submit it through `supervise_publication tower-operator-exchange` using the
existing operator profile and explicit-document grammar. The client validates
the entire original scope, registration, coordinates, limits and completed
links before printing that response or sending the next supplied document.
It never follows links automatically. Its historical validation is not an
independent journal verification or a new source of authority.

## Verification scope

The public regression target is `tower_request_history`. It connects actual
ordinary review and learned refinement histories through discovery to the
existing explanation path, and pairs those controls with missing requests,
incomplete or unrelated rounds, finite work, custody loss, wrong owners,
scope/generation refusal, response tampering and output exhaustion.

Executed results and remaining repository qualification belong in the dated
implementation-status entry. Metadata discovery establishes no independent
receipt verification, current eligibility, production containment, complete
generic graph traversal or empirical alignment claim.
