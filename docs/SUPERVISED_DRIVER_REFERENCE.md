# Supervised actor-to-effect driver

Consumer: a trusted host integrating the existing actor gateway and connected helper workers. `oversight::supervised::SupervisedDriver` owns the original ActorSupervisor and PublicationEndpoint. The actor receives only its existing ActorPort. This is the reference integration of plan section 17.1, the congress in sections 9.1-9.3, and the effect gate in section 8.3, serving FA-018/FA-019/FA-107 and FI-A03/FI-A06/FI-A13/FI-A16. It adds no runtime, alternative ledger, inference implementation or dependency.

## Request-to-effect behavior

The host accepts the original FIFO request against a supplied current Snapshot, supplies the complete captured CommitteeInput and an exact roster-to-UnixStream map, and starts a bounded HelperPool. The driver never invents an input view, selects a verdict or removes a failed helper. Wrong rosters refuse before recording inputs. Later setup errors can leave the original round ID consumed; they do not create a hidden retry.

Only one review is active in this authority domain because the original congress binds a global control predecessor. This is not a claim of parallel multi-action scheduling. All actor requests, input captures, attempts and completed rounds remain subject to their existing lifetime bounds. The driver retains one additional exact CommitteeInput clone and at most one original noncloneable Permit; no unbounded job history is introduced.

One step performs one bounded helper-pool pass OR one publication attempt. Applying a Continue result does not execute in that same step. The next step checks the retained review's control sequence and input revision, requires the caller's current whole input, and goes through original authorization and dispatch. Missing current input cannot be replaced with the driver's retained bytes. A restrictive review ends the active job without automatically convening another round. A completed review refused by current governance/evidence is reported once as ReviewRejected, not re-applied later.

Two-key mode returns AwaitingHuman without reserving resources. The host requests an original human approval for the exact approved attempt and submits the separately minted HumanPermit. There is no one-key fallback or actor-provided key. Expiry and revocation remain enforced by the original key and endpoint machinery. A pre-dispatch failure retains the original automatic permit and its one reservation for explicit repair or cancellation, rather than authorizing another attempt.

Actor cancellation is synchronized before progress and through the existing final dispatch path. A cancelled or otherwise terminal attempt drops the driver job without executing. Once a send has occurred, an endpoint error yields DeliveryUnknown and drops the sending job. The original authority ledger and actor request retain the outstanding obligation; subsequent driver steps do not send again. PublicationResolved contains the actual endpoint receipt, which can indicate executed OR confirmed nonexecution. Its name is not an execution claim.

## Clock and integration boundaries

`step(now, ...)` is an atomic logical-tick batch for controlled-time hosts. `step_with_clock` uses the host's controller-domain elapsed clock around each helper I/O step, observes a fresh post-I/O time before applying the completed review, and samples again after resource reservation before final dispatch. It does not accept helper timestamps. Monotone clock observations and partial worker progress are not rolled back after an error. This is not authenticated time or a measurement of the filesystem's physical publication/fsync instant.

Bootstrap and governance accessors remain explicitly supervisor-only. Direct broker changes still require synchronization of actor projections. The driver does not authenticate peers, start a trained helper, sandbox a hostile process, install a listener, or implement production Asupersync scheduling. It reuses the existing explicitly non-cryptographic commitment oracle. A host must still establish evidence authenticity and actual effect mediation.

## Status and change history

September 11, 2026: added the driver and eight public integration tests plus one compile-fail example. The socket fixtures check complete helper-client input equality before choosing deterministic fixture verdicts. They exercise Continue followed by one publication; a genuine Hold with no reroll; cancellation during review and after approval; missing/changed current evidence; repeated pre-dispatch fence refusal retaining exactly one reservation; repair versus explicit cancellation; human approval and exact expiry; wrong-roster preflight; and a disconnected member beside a healthy peer. These are test sources, not observed executions or trained-model evidence.

Cargo, rustc, rustfmt, RCH and br are unavailable in this editing environment. The additions have not been compiled, executed, formatted with rustfmt, or qualified by the project gate. Existing execution receipts remain historical. No br-managed task or production gate is closed, and no native foundation admission is implied.
