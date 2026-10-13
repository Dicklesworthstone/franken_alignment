# Guarded native evaluation/Andon restart

Source implementation, 2026-10-13. Continues JOURNALED_NATIVE_ANDON.md, plan §9.9
and §16, FI-A08/FI-A13/FI-A18 and the still-open credibility-ledger-dc0.4 packet.

`create_andon_guarded` prepares every original guard, the native evaluation
protocol and native Andon activation before creating storage. The same first
canonical replacement publishes all of them. There is no intermediate live
owner missing accounting, and an uncertain write returns no partial role bundle.

`open_andon_guarded` requires the independently retained FileOversightProfile,
evaluation protocol, FileAndonRecoveryRequirements and original FileHistoryAnchor.
Requirements include the complete current window inventory (not just bootstrap
windows), unchanged initial configuration, the original guard/effective-policy/
authority floors and minimum native evaluation revision. Exact anchored prefix
comparison and configuration preflight precede native replay; complete original
evaluated-guard checks and native source floors precede cleanup and fencing.
An equally long valid history with different label order cannot replace the
anchored history merely because its counts, costs and clock ticks match.

The original recovery fence invalidates old approvals, evaluator/ticket brands
and clock eligibility. Only after its canonical acknowledgment are the separate
fresh human/evaluator roles returned. Replayed source includes pending cases,
censored and final labels, full native receipts and incidence order. Pending
Benign labels require fresh time before first acceptance; already accepted exact
retries append nothing and require no time. No helper is rerun during replay.

`read_andon_guarded` applies the same exact requirements beside a locked or
faulted writer, without cleanup, fencing, fresh clock or role issuance. Its
historical snapshot keeps pending/censored counts explicit and receipts ordered
by native acceptance revision. A complete accounting snapshot does not assert
that unknown labels have resolved or that the evaluator is authentic.

These explicit APIs add the ordinary evaluated-guarded composition. Existing
raw, joint, learned and other recovery APIs are unchanged and do not implicitly
acquire these additional independently pinned Andon expectations. This is not
signed evaluator intake, arbitrary-seed import, a new executor, a new storage
format or command-line activation. The caller must retain anchors independently;
replacing both source and anchor with old copies defeats rollback protection.

Seven new regressions exercise first-image and recovery-fence failures across
all five original Store barriers, actual post-restart assessment with fresh roles,
old-ticket rejection, no-clock refusal and corrected retry, every independent
contract/floor mismatch, a valid equal-counter fork, read-only pending state,
truncated/missing source and genuine successors. One compile-fail boundary makes
the data-only snapshot incompatible with an effect permit. Existing tests remain
unchanged. RCH is absent: target tests and the unchanged xtask gate each exit 127
before Cargo. Compilation, runtime tests, rustfmt and Clippy remain unexecuted.
