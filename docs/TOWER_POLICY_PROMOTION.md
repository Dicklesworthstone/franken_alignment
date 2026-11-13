# Promote one independently approved original policy campaign

The native publication Tower connects `next` and the registered `promote` verb
to one independently approved campaign in the original durable owner. This is
a trusted Rust embedding capability, including the existing authenticated Linux
`PublicationPeerSession` transport. The ordinary `supervise_publication` server
can now explicitly provision a [separate live governor](LIVE_POLICY_GOVERNOR.md)
for a new ordinary plain or checked scheduled run. Its ordinary policy-replay selection continues
to grant comparison only; governor custody and operator promotion are separate
startup selections and explicit authenticated decisions.

## Keep approval outside the operator

The embedding enables the original policy-campaign guard before the first
proposal or actor request and retains the returned `FilePolicyGovernor`
separately from the actor, helpers, effect reviewer and Tower. The existing
`request_policy_campaign` operation evaluates the complete retained corpus.
The independent governor reviews that exact campaign and explicitly approves
it, including any newly reviewable cases. Missing observations still require
shadow and cannot be waived by the acceptance flag.

Only the resulting nonclone `FilePolicyPromotionPermit` can be delegated:

```rust,ignore
let (port, custodian) = supervisor.policy_promotion_port(
    independently_approved_permit,
    32,                         // cumulative Next queries
    4096,                       // complete journal, including Promote
)?;
let mut tower = PublicationTower::new_control_operator(
    original_run_address,
    original_actor_port,
    PublicationLimits::default(),
)?;
tower.set_policy_promotion(Some(port))?;
// Retain custodian outside the Tower for this original owner's lifetime.
```

The constructor checks the original issuer and current native campaign before
delegation. It cannot manufacture a permit from a report or recover a lost key.
The custodian owns one approval and at most one entered mutation. It is not
cloneable. Port clones retain weak access to the same owner, permit and work
allowance; reconstructing a facade or reconnecting cannot replenish them.
Dropping the custodian revokes every weak port. Installing a foreign port clears
the previous installation and refuses, even when numeric scopes match.

An already running `PublicationPeerSession` can receive that original weak port
through its host-only `set_policy_promotion(Some(port))` method. Installation
uses the same Tower ownership check and does not replace the authenticated
session, channel, connected socket, pending reply, registration, access profile
or cumulative connection/exchange limits. Removing a port withdraws future
access without rewriting an already queued historical response. A revoked
session cannot be revived by installing a new port. This setter never provisions
a governor or reconstructs an approval from a response.

For a new ordinary owner, `FileOversight::create_guarded_reserved` provisions the
explicit `FileGuardSet` and original `RecoveryReserve` in the first canonical
image, returning the separate `FileOversightRoles`. It uses the existing guarded
bootstrap and sole canonical replacement. Native guard, reserve and profile
validation precedes store creation; canonical path-dependent encoding and write
failures retain the original store semantics. Existing directories are refused.
This provides no late upgrade or restoration of a previously approved key.

`FileOversight::create_guarded_publication_reserved` uses the same prepared
bootstrap with explicit plain publication witness limits. The first image
contains those limits together with the other guards and reserve, and every
post-promotion effect remains subject to the original final witness validation.

The caller also needs authenticated operator status, access to the original run
and explicit `Permit` purpose. Actor, observation-only, legacy reconciliation,
imported-snapshot and branch mounts do not acquire this authority. A governor
never crosses the Tower interface, and a promotion receipt never supplies either
of the two effect keys.

## Observe and explicitly select the original transition

Use `ReadArguments::PolicyPromotionNext { campaign, operation }` at the original
Run address. Its closed wire arguments are:

```json
{"action":"promote_policy","campaign":"91","operation":"900"}
```

All integers use the Tower's canonical unsigned decimal-string representation.
Both IDs must be nonzero. `campaign` selects only the installed approval;
`operation` identifies this grant's explicit attempt. An optional Next
idempotency key must equal the operation. Next spends one query but writes no
journal event, takes no admission snapshot and spends no promotion operation.

A known plan records the complete original cut: tenant, principal, run, branch,
authority, effect purpose, campaign, journal revision, control sequence,
authority epoch, current policy generation and recorded elapsed time. It also
names the fixed approved candidate generation. The clock is historical; this
operation samples no fresh clock or source.

When the observed preconditions permit promotion, `LinkedPolicyPromotionNext`
returns one complete `TowerRequest` from `action()`. That document uses
`Verb::Promote`, the exact registration generation, the same operation as its
idempotency key, and `ReadArguments::Promote { operation, expected }`. It is data
until explicitly submitted. The candidate policy itself cannot be replaced by
wire bytes. Costs, information gain, ranking and current publication eligibility
remain Unknown.

Known blocked plans distinguish Stop, revoked/unapproved campaigns, stale full
corpus, interrupted source, unsupported numerical replay, exhausted journal work
and an already consumed operation. Wrong-campaign and revoked-grant observations
are Withheld. Lost, replaced or unavailable original owners remain Unknown.

## Commit and conserve the original obligations

Before entering a mutation, the native port rechecks every predecessor field,
original ownership, source-interruption latch, native campaign approval and full
corpus stamp. A new exact denial can invalidate the campaign even when the
control sequence has not changed. A clock-only journal advance instead requires
a fresh plan while retaining a still-valid approval. Rejected preconditions do
not spend the sole operation or clear the original admission snapshot.

Entered work spends the operation and invalidates the admission snapshot before
calling the existing durable `promote_policy_campaign`. The complete journal
plus proposed promotion event must fit the selected replay ceiling. This
ordinary profile refuses decoder and predictor histories instead of acquiring
numerical work implicitly. Ordinary mediated owners retain their independent
source and final-publication checks.

The existing native transition advances the control sequence and revocation
epoch, installs the exact approved policy, cancels eligible old undispatched
attempts and withdraws old human effect keys. Only actual cancelled reservations
are refunded. Dispatched and Unknown effects retain their charged liabilities
and original endpoint obligations. The target ceiling cannot widen, suspension
cannot be cleared, and promotion publishes no effect.

Subsequent work needs fresh admission at the new epoch, complete current evidence,
helper review and independent human approval. The driver method
`FileSupervisedDriver::promote_policy_campaign` performs helper maintenance
immediately. A promotion through the weak supervisor port is observed by the
driver's existing maintenance on its next step, before later clock or evidence
work. Child exit still requires actual bounded reaping.

An exact entered-request retry returns the original retained acknowledgment or
failure without another transition or work charge. It does not treat an old
cancelled-attempt list as instructions to cancel newer work. Changed requests
conflict, another operation cannot reuse the grant, and failures after entry
never refund its work allowance. Unavailable storage exposes no candidate receipt.
Recovery revokes the original process-local approval; it does not recreate it.

## Sealed response profiles and scope of evidence

### Explicit authenticated document client

An independently provisioned native endpoint can be called with the existing
operator client and an explicitly selected public connection profile:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE NEXT_DOCUMENT
supervise_publication tower-operator-exchange OPERATOR_PROFILE PROMOTE_DOCUMENT
```

The client validates the complete closed promotion schema, original registration,
campaign, operation, predecessor and returned structured action. Every selected
document must match the configured tenant, principal, run, branch and authority;
the entire input sequence is checked before connecting. A known response's full
audience is checked again before output or any later document.

Acknowledgments must preserve the original before-cut and recorded clock, advance
journal revision, control sequence and authority epoch exactly once, and name a
newer policy generation. Cancellation IDs must be canonical, ordered and bounded
by `MAX_ATTEMPTS`; an empty cancellation set cannot claim a refund. The client
checks internal consistency, not independent truth of a remote ledger. The reply
parser admits a complete bounded cancellation set within the existing response
byte ceiling. Malformed replies stop the sequence before another document is sent.

The client sends only supplied documents. It does not follow a returned action,
approve a candidate, reconnect or retry automatically. A promotion result or
refusal must retain `retry_safe: false`. Genuine native Next refusals keep their
original Boolean retry metadata. A payload relabeled as a generic refusal cannot
bypass validation. The profile supplies connection identity and finite transport
limits; it does not provision a governor or install server-side promotion custody.

For the executable ordinary service, select `--governor-profile` and the
independent operator `policy_promotion` object at new ordinary plain or checked scheduled creation.
The separate `policy-governor-exchange` client sends one explicit review or
approval document per invocation. The service requires approval to contain the
exact complete report returned for that campaign. The original native key is installed into the surviving operator session
before its approval acknowledgment. See [live governor setup and wire
contracts](LIVE_POLICY_GOVERNOR.md). Open, mediated and numerical
combinations remain outside this new ordinary service selection.

### Native evidence and limits

`PublicationResult::PolicyPromotionNext` encodes as
`fa.publication-policy-promotion-next/1`; `PolicyPromotion` encodes as
`fa.publication-policy-promotion/1`. Both remain inside the existing sixteen-field
Tower envelope. The encoder checks the original registration, request identity,
campaign, operation and complete native basis, including the exact before/after
sequence, epoch and journal changes. Results disclose neither policy literals
nor action payloads. A receipt explicitly reports historical cancellation and
refund observations, not present effect permission.

Native and public tests cover real fresh-policy publication, retained Unknown
liabilities, old-key withdrawal, exact retries during newer work, every expected
cut field, independent custody and caller roles, malformed documents, altered
response envelopes, finite work, source interruption, numerical refusal and all
five original storage barriers. Linux peer tests use actual kernel credential
checks, a rejected identity paired with the selected identity, lost replies and
reconnection under the same finite grant.

These tests are source coverage unless an exact execution result is recorded in
`IMPLEMENTATION_STATUS.md`. They do not establish production governor identity,
administrative independence beyond the configured role admissions, cryptographic approval,
anti-rollback storage, scientific policy efficacy or the full repository gate.
