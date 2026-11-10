# Original live journal metadata through the Tower

The registered `tail` read now reaches the original acknowledged
`FileOversight` owner through a separately granted operator capability. It
captures one ordered suffix of that owner's journal and returns bounded pages
of redacted metadata. It works with ordinary and learned native Tower mounts;
it does not reconstruct a second owner or run numerical replay.

This is scoped progress on FA-135/FA-134 and the Tower dispatcher. It does not
complete the planned cross-layer event schema, source-frontier accounting,
decision replay, or qualification. The existing snapshot `tail` retains its
separate `since_seq` contract.

## Provision the original capability

The original supervisor calls `journal_tail_port(queries, events_per_page)`
and retains the returned `FileSupervisorJournalTailGrant`. The port is installed
with `PublicationTower::set_journal_tail(Some(port))`. Only the supervisor can
issue this disclosure; actor requests, situation grants, precheck grants and
explanation grants cannot create it. Installation requires the same original
actor gateway. The native port additionally binds the original host incarnation,
so replacing its mutable host slot with an equal-scope owner cannot adopt a grant.

Queries must be in `1..=1024`; page size must be in `1..=64`. Capture is bounded
by the original journal ceiling of 4096 records, checked before allocation. The
host fixes these bounds once. Clones share the same remaining allowance, and
each grant retains at most one capture. Dropping or revoking the noncloneable
grant withdraws disclosure and all of its continuations. Ports and cursors
retain neither the supervisor nor the grant.

## Select an explicit original revision

At the run mount, use the normal six-field `fa.tower_request/1` document:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "journal-1",
  "address": "fa://7/run/8@3",
  "verb": "tail",
  "arguments": {"after_revision": "0"},
  "idempotency_key": null
}
```

Every integer is a canonical unsigned decimal string. `after_revision` is an
original journal ordinal, not a native control sequence, registration generation
or timestamp. A continuation has exactly `{"cursor":"17"}` as its arguments;
the cursor must be the next handle returned by this facade's original grant.
The caller cannot choose a page size, offset, capture boundary or disclosure.
Native `tail` rejects `null` and the snapshot's `since_seq` arguments. The
snapshot dispatcher refuses native journal arguments without consuming its
read allowance. Both typed and JSON entry points enforce these distinctions.

The first request captures exactly `(after_revision, basis.journal_revision]`.
Later pages use that same immutable cut even if the live owner appends events.
Starting at the acknowledged end returns a complete empty suffix. Starting
beyond it spends one accepted query and returns Unknown, never an asserted
absence of future history. A new capture retires the old continuation.

## Read a sealed historical result

The sixteen-field Tower envelope contains a `journal-tail` result with schema
`fa.publication-journal-tail/1`. Its closed wrapper binds the original run
registration and exact query address, marks the result `historical: true`,
states `current_eligibility: "not_observed"`, and names the disclosure
`redacted_journal_events`. The original operation ID and optional idempotency label are also sealed.
Public envelope edits cannot relabel the native result, including an Unknown
or error. An idempotency label correlates a read; it does not refund or replay
spent native query work.

A Known result carries the original scope, journal revision, control sequence
and recorded elapsed observation in `FileSituationBasis`. Its origin is
`acknowledged_owner`, with no synthetic checkpoint anchor. Global Tower control
sequences, source frontiers and costs retain their existing Unknown semantics.
Recorded time is an original observation; tail never samples a clock.

The value contains `after_revision`, `offset`, `through_revision`,
`enumeration_complete`, `continuation` and `events`. The lower bound stays fixed;
offset counts rows already returned, and through-revision ends this page.
Each row contains exactly `revision`, `kind`, `attempt`, `request` and `round`.
Optional IDs are present only when directly recorded by that event. Human
approval identifiers are not relabeled as actor request IDs. All 68 event-kind
labels are closed and cover the acknowledged native event variants, including
learned generation and helper-sidecar operations.

Labels describe recorded operations. A `publish` row is not an execution
receipt, and a `reconcile` row does not prove settlement. The existing original
outcome and explanation capabilities own those observations. Payloads, source
bytes, model state, policy literals, human keys, helper identities, votes and
salts are absent from both typed metadata and JSON. The redacted stream cannot
replay the authority machine.

`enumeration_complete` is true only when every row through the captured cut has
been returned. Exhausting a grant mid-capture returns no further cursor and
leaves completion false. Missing continuation alone never means complete.

## Fail without renewing work or authority

Operator role, tenant, mount and registration checks precede native work.
Foreign, forged, replaced and consumed cursors cannot skip rows or renew quota.
Every accepted capture or continuation spends one query before fallible work;
output failure does not refund it. A failed encoding never emits a partial
Known result or preserves a facade cursor that could retry consumed work.

Owner loss, conflicting owner borrow, storage failure and host replacement
invalidate retained progress. No disk fallback or newer capture substitutes for
the selected history. An absent capability, revoked grant, exhausted quota or
unavailable owner is explicit rather than a fabricated empty journal. Reads
preserve pending admission snapshots, canonical journal bytes, numerical state,
publication state, charged Unknown effects and every authority key.

## Authenticated operator service and finite command

On Linux, the existing ordinary, learned and resident operator services can
select this capability in their `fa.tower-operator-service/1` profile:

```json
"journal_tail": {
  "queries": 128,
  "events_per_page": 32,
  "disclosure": "redacted_journal_events"
}
```

This is an optional member of the existing profile, not a complete profile.
Its integers use the profile's JSON-number convention. Omission or null keeps
Tail disabled; unknown fields, noninteger or excessive bounds and other
disclosure markers refuse before setup. A Tail-only endpoint can set inspection
and explanation counts/replay to zero, keep explanation disclosure `"none"`,
and omit the other optional capabilities. It requires no numerical replay grant.

The service installs the capability from the existing original supervisor and
keeps the grant, cursor and remaining allowance across authenticated reconnects
and scheduled or resident handoffs. Retirement drops disclosure custody. Actor,
operator and independent human-review/Stop endpoints remain separately bound.

With a verified executable and configured operator endpoint, run:

```sh
supervise_publication tower-tail OPERATOR_PROFILE fa://1/run/3@41 --after-revision 0 --max-pages 128
```

The run address must carry the actual nonzero registration generation. The
command opens one authenticated connection and follows one native capture. It
caps requests by the smallest of caller max-pages, selected Tail queries and
the original transport exchange allowance. Existing cumulative byte, I/O and
elapsed bounds also apply. No reconnect, retry, fresh capture or hidden
follow-polling is performed. To observe later history, explicitly start another
capture after the previous captured end using remaining service work.

Every response is checked before output and before any subsequent request:
closed schema and event labels, original registration/query, complete scope,
fixed native basis, initial revision, contiguous offsets/rows, completion and
nonrepeated cursors must agree. The command flushes each validated complete JSON
response as one output line. Success means explicit complete enumeration of the
captured suffix. A valid partial prefix remains output if a bound, missing
continuation, Unknown, Withheld or native refusal stops the command, which
returns a failure exit status. Check that status before treating the prefix as
complete. Output failure cannot spend another native query.

`tower-operator-exchange OPERATOR_PROFILE REQUEST_JSON` also validates native
Tail responses for explicit request documents. It remains an explicit document
exchange; only `tower-tail` performs bounded continuation over a captured suffix.

## Verification scope

Native and public Tower regressions exercise real publication and learned
decoding, coherent pagination during owner advancement, custody and quota
failures, malformed requests, sealed response tampering, output refusal,
redaction and retained Unknown liability. See
`crates/fa-reference/src/action/consequence/delivery/persistent/requests/actor/journal_tail/tests.rs`
and `crates/fa-reference/tests/tower_journal_tail.rs`.

Sixteen additional service/client tests exercise strict profile selection,
original grant custody across reconnects, real authenticated pagination,
altered or substituted captures, finite caller/profile/transport/native bounds,
wrong peers, owner/output loss, and learned publication before and after its
independent human approval. They are under the example's operator and client
`journal_tail` test modules. Their source also passes the installed formatter's
syntax check; execution remains pending for the same environment limitation.

This session has no configured RCH executable or worker and lacks the qualified
`nightly-2026-09-08` toolchain. Source review and permitted syntax-only checks do
not establish compilation or runtime success. The implementation remains open
for required RCH verification; no bead or full roadmap packet is closed.
