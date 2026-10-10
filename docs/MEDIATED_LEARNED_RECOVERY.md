# Learned generation with durable topology-observer custody

The original learned generator and the original mediation gate now share a
complete guarded startup and recovery profile. This connects real numerical
continuation to the existing effect boundary: a generated source still needs
current topology coverage, independent review, and both publication keys.
No second graph checker, replay engine, reset algorithm, or journal is added.

## Atomic startup

`FileOversight::create_mediated_guarded_with_learned_generation` accepts the
independently selected learned recipe, complete `FileGuardSet`, original graph,
credential registration when required, and optional marginal
`EvaluationProtocol`. Every original bootstrap transition and the complete
resulting guard inventory are checked before storage creation. One first
canonical replacement installs them all; only acknowledgment releases the
owner and `FileMediatedRoles` for separate custody.

The topology starts available but uncertified. No numerical token, identity
match, forecast, cut certificate, or effect approval is invented. When the
recipe owns a policy-only file source, its policy must exactly match the
separate source guard. The original learned Enable installs it once.

## Recovery and fresh qualification

`begin_open_mediated_guarded_with_learned_generation` returns the sealed
`guarded::mediated::learned::FileMediatedLearnedRecovery`. Its anchored variant,
`begin_open_mediated_guarded_anchored_with_learned_generation`, additionally
compares an independently retained `FileHistoryAnchor` against the same locked
canonical image before numerical replay. The synchronous
`open_mediated_guarded_with_learned_generation` consumes the same cursor.

`FileMediatedRequirements` remains an independent recovery input. The exact
recipe, original and last registered graphs, optional evaluator, and excluded
numerical modes are checked before replay. Original replay verifies every
numerical witness, graph update, cut check, label, and control transition.
Readiness additionally requires exact topology availability, every guard,
effective policy, credential epoch, and all externally retained recovery floors.
The requirements are copied into the cursor; changing caller data cannot relax
them. A failed final check remains failed and never exposes Ready.

`advance` executes a bounded number of original events. The original exclusive
lock stays held; unfinished recovery and contract refusals do not clean staging
or alter canonical bytes. An event count is not a wall-time bound, and final
encoding and filesystem I/O remain synchronous.

`finish` checks the requirements again and acknowledges the single original
recovery fence before releasing topology, human, identity, governance, and
optional evaluator roles. The fence withdraws the current cut and topology
availability, pauses learned generation, and invalidates old role handles,
effect keys, and identity eligibility. Recovery does not recertify the old graph.

Fresh permitting requires a newer graph **and** inventory generation, the
original cut verification, fresh clock and admissible numerical continuation,
and every applicable source/identity/review requirement. A graph with a bypass
cannot satisfy the cut even when the numerical source is quiet. Historical
dispatch cuts remain attached to their original effects.

## Completing an acknowledged pending reset

`finish_pending_reset` requires the exact independently retained
`FileLearnedResetIntent`. It checks the full mediated contract before and after
the original private reset, then publishes completion and the original fence
together. No intermediate owner or topology observer escapes. A changed command,
interrupted predecessor, missing intent, or failed requirement is not rebased.

An exact already-completed retry adds only the original fence. It cannot repeat
the numerical audit, increment an incident again, refund unknown effects, or
restore old source coverage. Requirements describe the selected image before
that fence; after an earlier successful recovery its topology is unavailable.
The original incident, authority, numerical-work, and effect ledgers remain
authoritative. An unresolved dispatch stays charged until its actual original
reconciliation or nonexecution receipt resolves it.

## Scope and verification

This profile admits ordinary learned generation and its optional marginal
evaluator. Predictor-pinned raw/owned recipes, standalone predictors, joint
evaluation, and held-out joint evaluation are refused explicitly. The existing
profiles retain their original inventory checks. No old guard is removed to
make recovery succeed, and no prediction-observer role is silently omitted.

Eight public `mediated_learned_recovery` tests cover all optional role masks;
actual original generation after recovery through fresh topology, identity,
congress, both keys and publication; anchored pending reset and sampled-token
reproduction; exact retries and unknown-effect accounting; no-write contract
refusals; equal-counter forks and truncation; bypass rejection with a permitted
control; unsupported prediction pins; and a real policy-file source refresh.
Two compile-fail examples cover premature role access and recovery unwrapping.
Execution results belong in `IMPLEMENTATION_STATUS.md`; authored tests alone
are not execution evidence.

This implements the composition in plan §§11.10, 15.2, 15.6 and 16.4. The graph
remains an operator-declared model, not a claim of operating-system containment.
An external anchor rejects incompatible history relative to its retained bytes;
it does not authenticate itself or guarantee that its custodian retained the
latest cut. No dependency admission or deployment qualification is implied.
