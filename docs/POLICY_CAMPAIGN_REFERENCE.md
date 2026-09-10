# Evidence-scoped exact-policy replay and governed promotion

## Implemented consumer

`OversightBroker::enable_policy_campaigns` freezes a campaign-required profile
before proposals and returns a separate `PolicyGovernor`. Once enabled, the
ordinary `replace_policy` method refuses. A policy change requires a complete
own-history campaign and an unrevoked, exact-candidate governor approval.
No disable or governor-recovery operation is exposed. Existing unconfigured
brokers retain their explicitly trusted direct-governance behavior.

`request_policy_campaign` draws its corpus from the original controller, not a
caller-selected sample. It includes every admitted complete proposal, including
exact denials, and every successfully applied review under the current exact
policy. A later review retains its own observed values, not the proposal's earlier
passing evidence. Old-policy rows remain in controller history but are not mixed
into a campaign for a different policy question. Pre-admission errors and unfinished
rounds have no fabricated evaluation rows. This corpus is not claimed to cover
those missing observations or the deployment's entire traffic population.

Each report identifies newly blocked, newly reviewable, unchanged and
shadow-required cases. A policy passes the replay prerequisite only when all its
reads are supported by the retained evidence. The governor must explicitly accept
any observed newly-reviewable cases. No effect permission follows: fresh proposals,
current evidence, independent review, and the ordinary permit path are still
required under the new epoch.

The governor approves, rejects or revokes immutable campaign evidence. A different
request number cannot reroll an identical candidate at the same history cut after
rejection, revocation or a lost approval key. Approved is historical issuance
status, not a currentness certificate. A governor is a process-local separated
role, not an authenticated principal or an independently labelled outcome.

## Candidate evaluation

`action::consequence::policy_campaign` also provides
`PolicyReplayReport::from_archives` for offline consumers. It verifies each archive
against its independently retained anchor first. Offline reports have no live
promotion capability and cannot substitute for the owning broker's corpus.

The source action and original policy evaluation are checked again. Candidate
reads require an explicit observed key or a closed range containing the key.
An empty-range predicate needs complete retained coverage unless an observed
positive member already refutes it. Adjacent closed domains can compose; gaps
cannot. The maximum u64 key remains directly observable without an overflowing
successor. Required reads in unselected Boolean branches are checked too.

The original policy evaluator runs only after this coverage check. The sparse
reconstruction is never exposed as a complete provider database. New unsupported
reads produce `RequiresShadow` and identify the candidate node indices. Missing
evidence is not an empty-value substitute. Candidate resource-limit failure
refuses the comparison rather than producing a report for a convenient prefix.
There is no public builder allowing an incomplete live report to be finalized.

Each row retains its action, original and candidate predicate evaluations,
observed semantic epoch, original consequence where available, and exact-policy
change class. Proposal and later review observations remain separate cases even
for one action. Counts are retained cases, not independent trials or rate estimates.

## Promotion ordering and outstanding effects

An approval binds the exact report and candidate, originating broker, control
sequence, revocation epoch, policy/congress generations, append-only proposal and
review counts, started-round count, and actor revision. Even a new exact denial
invalidates it: those denials add history without incrementing the control sequence.
Starting or applying another review, changing the actor, resetting, revoking, or
replacing congress weights also prevents stale promotion. The caller must obtain
a fresh comparison and a new explicit approval; history is not silently rebased.

`promote_policy` invokes the original policy replacement transaction. It advances
the existing control and revocation floors, cancels old undispatched attempts,
and refunds only their real reservations. Dispatched/unknown effects, stream
prefixes, pending disclosures, narrower target ceilings and suspension survive.
Existing receipt reconciliation and cancellation do not depend on the governor
or available campaign capacity. Old automatic and human keys cannot regain
validity after promotion. A successfully consumed policy approval stays Promoted;
a refused promotion cannot spend it or repair effect accounting by refunding it.

`PolicyPromotion` retains the report, the explicit relaxation acceptance, and the
actual original `PolicyChange` receipt including cancelled IDs and refunded units.
Actor reset does not erase the campaign mode, request history, approvals, records
or quotas. The current reference clock does not authenticate any of these records.

## Bounds and limits of the claim

The fixed ceilings are 5,120 cases per report and 32 MiB of retained action/witness
variable bytes, with smaller configured limits. Repeated witness occurrences and
candidate witness copies count. At most 64 campaign requests are retained, and the
32 MiB variable-byte ceiling applies cumulatively to their reports. Policies,
node metadata, allocator overhead and external caller copies have separate bounded
shapes and are not included in that byte number. These are logical bounds, not
peak-memory, CPU-time or latency measurements. Failure returns no partial campaign.

This is scoped reference progress toward plan 22.7 / FA-130, serving FI-A14 and
FI-A18, not completion of that production packet. Only the exact-policy lane is
replayed. Old congress consequences are historical annotations: no helper is
rerun, no changed-policy helper answer is fabricated, and `NewlyReviewable` is
not `Continue`. Full congress-threshold replay, independently labelled campaign
outcomes, authenticated governance, durable campaign publication, and an actual
shadow-evaluation/promotion route remain outstanding. A new unsupported observation
requirement cannot currently be promoted through this bounded profile.

Even an unchanged finite corpus does not prove that a candidate has no relaxation
on unseen actions. The governor sees the complete candidate and must assess that
separately. This module supplies neither global policy implication nor detector
quality, helper honesty, completeness of provider capture, future safety or
independent ground truth. The public archive path retains the original verifier's
external-anchor and provenance assumptions. No older wire format is silently
promoted into a durable or authenticated policy-governance archive.

## Verification and change log

This batch adds eight replay-kernel tests, twelve public oversight integration
tests and one compile-fail type test. They pair successful governed promotion and
fresh publication with original-setter bypass, hidden denials, new keys/range gaps,
late changed observations, stale history without a sequence change, newly opened
rounds, foreign/revoked approvals, actual unknown stream liabilities, preissued
human keys, actor reset, bounds and lifecycle restrictions.

They have NOT been compiled or executed in this session. Cargo, rustc and RCH
are unavailable; no historical receipt is reused and no bead is closed. The
mandatory Rust/RCH verification gate remains pending.

Change sequence: evidence-scoped candidate replay first; then own-history
extraction and a mandatory, independently held promotion capability consumed by
the existing oversight controller. No Cargo dependency or second rights ledger
was added, and no existing test assertion or execution gate was weakened.
