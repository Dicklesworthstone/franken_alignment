# Policy replay from the durable oversight journal

`FileOversight::replay_candidate_policy(candidate, limits)` compares the candidate with every retained complete proposal and applied review under the current policy, including exact denials. It delegates to the original controller's replay builder: it neither substitutes an evaluator nor accepts caller-selected archives. The result is a `PolicyReplayReport`, not approval, a permit or a newly executed helper judgment.

`FileOversight::read_policy_replay(directory, profile, candidate, limits)` reads one canonical journal image, validates it against the independent bootstrap profile and reconstructs that corpus in RAM. It returns the report together with the original `FileDeliverySnapshot` identifying its historical cut. It acquires no writer, appends no fence, cleans up no staging file and cannot recover effect keys. A live owner's read refuses after a storage fault; the read-only file API can separately inspect whichever canonical image is actually visible.

Learned histories use `FileLearnedRecovery::finish_policy_replay(candidate, limits)`
and the same finalizer on the five guarded recovery wrappers. Each returns the
original `FileDeliverySnapshot` and complete `PolicyReplayReport` from the
same verified reconstructed machine. The core cursor retains its existing
numerical/profile boundary; it does not independently authenticate history
custody. Each guarded wrapper still enforces its original recipe, anchor,
predictor, credentials, floors, reserve and topology/source requirements.
Wrapper readiness checks precede the report. The canonical journal is
reconfirmed under the existing exclusive lock before returning; there is no
ordinary-replay fallback, live owner, Fence, cleanup, source refresh or helper
rerun. Dropping the read cursor releases the lock without modifying the store.
Historical numerical replay verifies the learned cut; it is not a new generation.

The [learned operator entry](POLICY_REPLAY_OPERATOR.md#guarded-learned-histories)
drives this finalizer with the same strict candidate grammar and report schema.
It requires independent history custody and exact v2 checkpoints for mediated
histories. Recipe request IDs never filter the corpus. Successful comparison
does not create a policy campaign or permit live promotion.

Candidate reads absent from the retained witnesses remain `RequiresShadow`, including reads in otherwise satisfied Boolean arms. Missing/corrupt journals, mismatched bootstrap profiles and exhausted case/byte limits return errors, not a selective prefix report. Reports compare exact policy predicates; newly reviewable does not mean newly authorized or harmless. Earlier policy generations remain in the journal but are not silently combined with the current-policy corpus.

The original public integration tests cover denied and approved proposals, applied reviews, unchanged live keys/accounting and file bytes, read-only inspection beside a live owner, reopen/fencing, shadow-only new observations, resource exhaustion and corrupt/profile-mismatched journals. This is reference-profile functionality serving plan §22.7 / FA-130. The existing operator filesystem/authentication assumptions remain. Compilation, formatting, Clippy and Rust tests have not executed: RCH was attempted and exited 127 because `rch` is absent. No Beads or production qualifications are changed.

Additional native and operator regression source covers guarded learned replay,
complete current-policy corpora and the no-mutation boundary. Required RCH test
and canonical-check attempts still exit 127 because the dispatcher is absent;
these new tests have not executed. No packet, Bead or production gate is closed.
