# Model identity and anchor liveness reference

## Implemented consumer and qualification

`OversightBroker::enable_identity_checks` freezes a model passport and bounded
liveness policy before proposals. It returns a separately held IdentityObserver.
Once configured, positive congress application, effect authorization, human-key
requests, and both ordinary and two-key dispatch require a current matching
identity basis. There is no disable, observer getter, passport replacement or
caller-authored Passed input. Unconfigured brokers preserve their prior profile.

This is scoped reference progress toward plan 7.10 / FA-118, grounded in FI-A01
and FI-A10. No new Cargo dependency, runtime, crypto primitive or rights ledger
was added. The source kernel is `activation::identity`; its owning consumer is
`oversight::identity`. Identity mismatch uses the original authority's existing
fencing transaction, not a separately maintained budget or an advisory score.

Rust compilation, formatting, Clippy, tests and the required RCH gate have NOT
run in this session. The active environment has no Cargo/rustc/rustfmt/RCH. No
bead is closed, no historical execution receipt is reused, and the original
FA-118 acceptance criteria are not reduced to the tests in this reference model.

## Passport and actual anchor comparisons

A ModelManifest binds supplied commitments for weights, adapters, tokenizer,
architecture and numerical profile, with tenant/model/host/tokenizer identities.
Even an empty adapter set has an explicit registered commitment. ModelPassport
freezes that manifest and the complete mandatory anchor set. These are structural
identities, NOT signatures or hashes computed from actual model files by this
implementation. Bootstrap checks the tenant and existing actor's declared model,
host and tokenizer generations; it does not measure the host's actual files.

Each IdentityAnchor freezes reference stimulus token IDs and inclusive finite
binary32 bounds for every observed coordinate. Comparison reads actual immutable
SourceFrame bits and checks model, generation, tap, layout, stream, dimensions and
the last stimulus position. Same-dimensional data with a different capture
contract is not accepted as interchangeable.

Every coordinate is compared. Reports count outliers and retain the first one's
exact bits and registered bounds. No average error can hide a single outlier;
no norm, subtraction or rounded tolerance calculation widens the acceptance box.
Signed zero is numerically equal under the interval profile. Subnormals and
extreme finite endpoints are compared directly. Registration rejects nonfinite,
empty or reversed bounds. SourceFrame already rejects nonfinite source values.

At most sixteen anchors, 65,536 total coordinates and 8,192 total stimulus tokens
are registered. Anchor IDs are unique. Reports retain measurements and identities,
not another copy of all raw activations. An anchor set defines an empirically
chosen fingerprint; passing it is not proof of identity against all substitutions.
Actual false-accept/false-reject rates and benign-drift tolerances remain an H19
qualification obligation, not a property established by registering intervals.

## Challenge, observation and installation

A challenge fixes passport, actor revision, control sequence, revocation epoch,
identity-basis generation, start time, collection deadline and lease expiry.
Only one challenge may collect at once. The observer must submit the manifest
and an accepted capture for EVERY registered anchor. Omitting one cannot produce
Matched. Accepted source sequences increase per anchor across challenges; reset
and capture loss never lower those floors. Duplicate submissions cannot replace
accepted measurements. Foreign observers and same-number foreign challenges
refuse through private process-local binding, not merely numerical IDs.

Beginning an admitted check removes the old live identity basis and advances the
input revision of retained proposals, clearing their empirical approvals. A
completed congress review or human key cannot silently cover a different identity
check. Context-valid capacity failure leaves the old basis removed, rather than
keeping a reassuring older result usable. Bad predecessor, duplicate identity or
foreign context refuses before that admission step. Inspect identity_basis and
input_revision after a capacity refusal; effect accounting itself is unchanged.

The observer computes and retains the result in the shared gate before returning
its report. A manifest mismatch, incompatible capture contract or out-of-bounds
anchor latches Mismatch immediately. Dropping or editing the returned report
cannot erase the latch. A matching result alone is not live: the broker must
apply the original challenge under its expected current sequence and epoch, with
unchanged actor revision and identity basis. No public API accepts an edited
IdentityReport or IdentityInstallation to authorize anything.

The broker independently observes time. An observer response from its future
cannot advance the broker clock. Collection closes at now >= deadline; matching
permission expires at now >= valid_until. Expiry is measured from challenge
START, so delaying a response cannot extend validity. Monotone clock rollback
refuses. A historical Matched report remains inspectable after becoming stale;
identity_status distinguishes it from a currently Matching basis.

No automatic randomized scheduler is implemented. The host/operator initiates
renewal and audit challenges; the fixed expiry bounds how long it may omit a
new successful check. There is no statistical claim about that challenge cadence.

## Immediate hold, original authority fence, real effect history

On mismatch, the common positive-path check refuses immediately, including the
human second-key path. `apply_identity_check` then commits the original control
ledger's admission stop: increment control sequence and revocation floor, cancel
all undispatched work, refund only Authorized reservations, and suspend the scope.
It returns the actual cancellation IDs and refund with the identity report.
Duplicate application refuses without repeating the transaction; the retained
installation receipt is available through identity_installation.

A stale expected predecessor cannot apply a stop at an invented cut. The shared
mismatch nevertheless stays latched, and the operator can apply that restrictive
finding at the actual current cut. Actor reset, policy change, input loss and
another caller's dropped handle cannot clear the mismatch. This profile has no
implicit re-arm or governed model replacement/resumption operation.

An identity fence is NOT endpoint nonexecution evidence. An envelope returned
before the stop may still execute afterward. Dispatched, unknown and terminal
effects preserve their accounting. A pending stream slot and any actual disclosed
prefix remain intact until their own terminal receipt protocol resolves them.
Cancellation, endpoint fences, nonexecution sealing and receipt reconciliation
remain available during identity failure, observer loss and quota exhaustion.

A successful fresh check can recover from timeout or explicitly reported capture
loss, but not from a latched identity mismatch. It cannot by itself release an
old empirical hold: the changed input basis still requires a fresh congress and,
when configured, a new human key. An unchanged reserved automatic permit can be
used only after those normal checks succeed; no second reservation is created.
Actor reset and policy replacement stale a previously installed identity lease
through the original revocation epoch. Historical checks, capture sequences and
quota consumption remain out of the actor checkpoint.

## Scope, costs and negative space

At most 64 checks are retained, subject to a smaller configured quota. Each check
has at most one manifest and sixteen coordinate-comparison reports. The passport
is shared immutable data. Comparing all anchors visits at most 65,536 coordinates
per completed check; reference input invalidation scans the existing bounded
attempt set. Only a mismatch installation retains a cancellation list, bounded
by that existing attempt cap. These are logical bounds, not measured peak memory,
latency or native inference cost. Caller-held copies remain the caller's memory.

The observer is a trusted, separately provisioned measurement ingress. The code
does not execute the reference stimuli, authenticate capture, ensure that a
host measured after the challenge rather than relabelling an older buffer, or
prove that checking stimuli did not perturb the production actor's state. A real
adapter must establish those boundaries. A malicious host that fabricates both
manifest and activation captures can defeat this reference mechanism. The opaque
process-local handle is not a cryptographic nonce, attestation, signature or
cross-process protocol. Matching under a finite anchor set does not prove weight
identity, universal safety, monitoring quality or absence of a hidden adapter.

The prototype passport does not yet include authenticated qualification closures
for restart profiles, calibration domains and signature libraries. Older decision
archives do not thereby acquire a model-identity proof. Native serving integration,
signed passports, durable/anti-rollback storage, randomized audit scheduling,
model-replacement governance and held-out substitution/drift campaigns remain
mandatory production work. No identity observation supplies an exact policy
disqualifier or overrides the other optional monitoring lanes.

## Source tests and change sequence

Five kernel tests cover inclusive drift, hidden outliers, signed zero/subnormals,
changed same-dimensional capture contracts, manifest binding, invalid registration
and aggregate bounds. Thirteen public integration tests cover startup gating and
actual reference publication, omitted anchors, discarded manifest mismatches,
unknown-disclosure reconciliation, reserved siblings, old congress/human keys,
future and expired observations, timeout recovery, exhausted quota, explicit
capture gaps, replayed captures, foreign roles, actor/policy change, reset and
restrictive-review availability. Two compile-fail doctests enforce observer
nonclonability and the lack of an observation-to-Permit conversion.

These eighteen Rust test functions and two doctests are unexecuted source.
First the comparison kernel and registration bounds landed; then the shared
mismatch latch, original-ledger fence and full oversight/stream integration.
No existing assertion or verification gate was weakened.
