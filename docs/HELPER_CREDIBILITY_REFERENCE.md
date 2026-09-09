# Independent-label helper credibility and fenced weight updates

## Implemented consumer

`OversightBroker` can enable a frozen `EvaluationProtocol` once, before any
proposal or round, and return a separate `IndependentEvaluator` handle. The
actual oversight/controller/endpoint path consumes this accounting through
`promote_credibility`; this is not a disconnected scoring report or another
parallel rights ledger. Existing unconfigured callers retain their fixed roster
and weights, and have no public raw weight setter.

The protocol names a domain, stratum and period, minimum independent benign and
violation origins, observed precision/recall floors, a false-positive ceiling,
and a false-stop budget measured in benign stopped rounds. Those settings cannot
be changed or reset by the actor, checkpoint restoration or weight promotion.
Helper contracts, questions, model/tokenizer profiles and cohorts are unchanged
by this weight-only profile; full helper replacement is not implemented.

## Independent labels and complete denominators

Every started round is tracked. An abandoned or unapplied current-policy round
remains pending and blocks promotion, rather than disappearing when its votes
are inconvenient. Successful review application extracts actual accepted member
outcomes from the existing replayable transcript. Missing and abstaining helpers
remain in the complete roster. Fallible preparation occurs before the control
transition, so evidence-accounting failure cannot leave a partially applied
review. Refused reviews do not become labelled successful transitions.

An evaluation ticket identifies the recorded round, attempt, exact-policy and
congress generations, evidence root and actual member verdicts. Only the
separately returned evaluator handle can issue a matching `SealedAssessment`.
It supplies independently owned ground truth, a retained evidence identity and
an original-scenario identity. No method obtains ground truth from a majority,
an actor explanation, or whether an endpoint happened to execute.

Unlabelled and censored cases block qualification. A censored assessment may be
resolved once, retaining both records. Final truth and origin cannot be silently
rewritten; identical repeated labels are idempotent. Conflicting final truths
for one origin in one policy generation refuse. Corrections of final labels,
new calibration campaigns and period rollover need future explicit contracts;
there is no history-clearing workaround in this implementation.

Repeated scenarios count once in empirical denominators. Replicated outcomes
are combined pessimistically: a violation is detected only if every retained
replica alerted, and a benign origin is a true negative only if every replica
allowed it. Missing/abstaining outcomes therefore count as misses on violations
and conservative false positives on benign origins. Separate missing and abstain
counts remain visible. Repetition cannot increase the independent sample size
or replace an inconvenient replica with a better one.

Counts are scoped to the exact-policy generation. Old records survive policy
replacement but do not qualify a changed policy's question. The false-stop
budget counts actual benign stopped rounds, including repeated origins and old
policy generations within the fixed period. Exhaustion exposes a calibration
incident and blocks weight updates; it never loosens requirements, drops a
helper or blocks cancellation and endpoint reconciliation.

## Weight selection and live fencing

All roster members must meet both independently labelled denominator minima and
the registered quality floors. Quiet helpers with excellent false-alarm rates
but poor attack recall cannot qualify. There is no manual requested weight.
The deterministic bounded rule is:

`weight = 1 + floor((per_member_cap - 1) * precision * recall)`.

Rates use exact integer cross-products; undefined denominators are not success.
The existing member/cohort reducer caps still apply. No roster, cohort, threshold,
minimum committee coverage, exact policy, target ceiling or budget is changed.
This is one explicit reference scoring policy, not a statistically optimal rule.
Lead-time credit, helper-hunting qualification and general model rotation remain
outside this implemented profile; elapsed ticks are not mislabelled as measured
control-sequence lead time.

Promotion checks the expected evaluation revision, control sequence and
revocation epoch. It stages the original ledger's accounting, advances the
congress generation, control sequence and revocation epoch, and cancels all old
undispatched attempts. Only actual reservations are refunded. Dispatched,
unknown and committed liabilities keep their dispositions. Old permits, bound
reviews and empirical approvals cannot carry through the change. New work must
capture new-epoch inputs and receive fresh review and authorization.

`CredibilityPromotion` retains the report and actual control transition, including
old/new congress configurations, cancelled attempts and refunded units. A
suspended run is not resumed and a narrowed target ceiling is not widened.
Checkpoint reset retains labels, assessor identity, incident status and promotion
history. At most 512 evaluation cases and 64 weight changes are retained.

## Integration repair in this batch

The preceding oversight source used nonexistent input/view APIs (`InputPart`,
`OmissionKind`, `ViewSpec`, `RedactionBinding`) and string profile fields absent
from the actual underlying modules. The packet implementation and its existing
positive/negative fixtures now use `SubmittedPart`, `ByteSpan`, `Omission`,
`AuthorizationProjection`, `EvidencePartView` and the actual numeric profile
identities plus profile bytes. Tenant checks inspect every projected original;
truncation/redaction refusals use the actual retained fields. The first ordered
Other part is the exact action frame; no nonexistent ordinal field is assumed.
The existing endpoint assertion now reads `DispatchEnvelope::request()`.
No compatibility stub or replacement underlying input model was introduced.

Logical committee-byte accounting now counts the actual retained payload,
read-witness values, submitted buffers, profile buffers and member names. It does
not invent a second submitted-byte copy or count numeric identities as strings.
This is a bound on retained variable data, not measured allocator or peak memory.

## Verification and limits

This batch adds nine credibility-kernel tests and eight public integration tests.
They pair qualified weight updates and fresh endpoint publication with quiet
helpers, repeated origins, missing/abstaining outcomes, censored/delayed labels,
abandoned rounds, foreign evaluator handles, stale metric revisions, false-stop
exhaustion, old permits, unknown liabilities, reset and policy-generation changes.
The existing 24 oversight tests were ported to the actual data APIs without
removing their positive or negative scenarios.

**Execution pending.** Cargo, rustc, rustfmt and RCH are absent in this session.
No Rust compilation, tests, Clippy, formatting gate or source-snapshot qualification
has been executed. Source review found the API mismatch; this is not a report of
a compiler run. Existing execution receipts do not qualify these new files. Beads
remain open, and the production FA-105/FA-114 obligations are not claimed closed.

The evaluator capability is a process-local role boundary, not a cryptographic
signature or proof of institutional independence. Correct labels, evidence
identities, origin lineage and the declared campaign sampling remain trusted
inputs. Observed rates are not population confidence bounds; this does not prove
held-out sampling, benign/attack prevalence, helper honesty or joint escape rate.
No inference engine, authenticated service, durable credibility storage, cross-run
aggregation or production admission is supplied. Founding roots are FI-A08,
FI-A09 and FI-A13; the relevant contracts are plan 9.4-9.6, 9.9 and FA-INV-032.
