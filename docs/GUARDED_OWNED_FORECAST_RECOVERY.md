# Guarded recovery of owned learned-code forecasts

This connects the original owned-K/V predictor to the existing complete guarded
recovery contract (FA-111 and the original recovery/authority boundaries). The
learned numerical engine, predictor, source checker, event codecs, likelihood
process, guard validators, storage replacement and role issuers are unchanged.
This is source integration, not calibration or deployment qualification.

## Independent inputs and one original replay

`FileOwnedPredictiveRequirements` contains the original `FileRecoveryRequirements`,
the complete `FileLearnedConsistencyConfig`, and an exact optional evaluation
protocol. The generation recipe is supplied independently as `FileLearnedConfig`.
The predictor must select owned generation; both optional pre-action and required
pre-output modes retain their exact versioned bytes. A separately pinned raw
residual predictor in the generation recipe is incompatible, not silently removed.

`begin_open_predictive_guarded_with_owned_learned_consistency` starts the original
exclusive `FileLearnedRecovery`. A private, typed, pre-replay binding step matches
both recipes and rejects supplied-capture records. It cannot rebind expectations
after a reducer has run. The original inventory preflight rejects unlisted raw
prediction, sampled-decoder, stop, topology and evaluator configurations before
numerical replay. No saved tensors or archived fidelity bounds become input.

`FileOwnedPredictiveRecovery::advance` uses the original cursor and event budget.
Every historical numerical operation and forecast outcome is recomputed and its
witness compared. On completion, the original guard validator checks exact guard
presence, effective policy, credential generation/revocation and externally kept
journal/control/authority floors. The full learned configuration is also checked,
not merely its probability table. A guard mismatch is sticky Failed, never Ready.

The anchored begin variant additionally verifies `FileHistoryAnchor` against the
same locked canonical image before replay. It accepts true successors of the
retained prefix, not equal-counter forks. Counter floors alone do not detect
forks; neither counters nor anchors authenticate an untrusted operator source or
make an outdated externally retained requirement current.

## Acknowledgment and returned custody

`finish` repeats the original guard checks and delegates to the original final
canonical-byte comparison, cleanup and one recovery-fence acknowledgment. No
second numerical replay is performed at this boundary. Only then are the original
human, identity, governance, evaluation and consistency roles provisioned. The
public wrapper has no inner-recovery accessor, candidate, role getter or clone.
Dropping it, finishing early, or encountering a mismatch leaves storage untouched.
A failed or ambiguous fence write returns no owner or role.

The synchronous `open_predictive_guarded_with_owned_learned_consistency` consumes
that same cooperative recovery. Old role brands and approvals stay invalid;
generation stays paused; new custody alone cannot resume or approve it. A fresh
clock and any original identity/source requalification remain required. A pending
forecast keeps its request identity, spent work and lost coverage. A pending
numerical intent does not acquire permission to sample. Unknown effects remain
charged and query-only; recovery cannot reroll them or manufacture a refund.

## Cost and validation limits

A quantum bounds original event count, not wall-clock time. A numerical event,
configuration comparison, final encoding, cleanup and storage write are still
synchronous. Replay repeats physical inference and monitoring. Restored logical
budgets are not exactly-once physical-computation accounting.

Seven regression functions use the original generator, learned-code predictor,
actor gateway and real Store implementation: usable observer recovery through
actual generated intake; abandoned recovery; exact recipe and floor rejection;
pending forecasts and intents; exact-prefix fork rejection; all five fence-write
fault barriers; and numerical/forecast-witness corruption paired with valid
history. No existing test assertions or production algorithms are replaced.

The required targeted RCH test and full gate were attempted but could not launch
because `rch` is absent (exit 127). Compilation, tests, rustfmt and Clippy remain
unexecuted. Selected-source hash/patch checks are not a full-checkout build. No
bead or qualification gate is closed by this implementation.
