# Native evaluations and Andon accounting in one original journal

Source implementation, 2026-10-13. Plan §9.9 and §16; FI-A08/FI-A13/FI-A18;
scoped advancement of credibility-ledger-dc0.4 and FA-105, not their closure.

The existing FileOversight already journals native evaluation protocols, applied
reviews, independent labels, pending cases and promotions. Native Andon had not
been activated by that journal. This extension connects it to the same owner;
it does not create another evaluation engine, compact-seed importer or database.

`FileAndonConfig` binds a new accounting identity and nonempty initial windows.
`FileOversight::enable_andon` requires the original native evaluator to be enabled
before any request/proposal/review, then commits activation through the original
Store before returning. `register_andon_window` persists subsequent immutable
windows. Exact retries append nothing; changed allowances, prices or old windows
cannot reset accounting. Recover an existing domain through its original journal,
not by activating an empty replacement ledger.

The original `FileIndependentEvaluator::assess` transaction now reconstructs the
original native label and its Andon transition together. No stored count, receipt
or incident is imported as truth. The original source events recompute all of them,
including first accepted revision, full helper contracts, occurrence/confirmation
time, censored-label resolution and incident-opening order. The existing staged
replacement publishes both or leaves both unacknowledged. A complete uncertain
canonical replacement can contain both; a failed call never implies neither.

A NEW Benign assessment in this opt-in file lane requires `clock_ready`, even
when the case ultimately proves ineligible for a charge. Historical elapsed time
after a recovery fence cannot date a new confirmation. Exact public retries,
Censored and Violation labels retain their clock-free behavior. A missing window
or invalid native transition fails before canonical publication, so a corrected
window registration permits the same label retry. No effect right, reviewer
weight, policy threshold, issuer credential or clock is inferred from accounting.

The compact standalone Andon file is now an optional derived accounting sink,
not a substitute for the native source journal. Read-only native binding, receipts,
ledger and complete source are exposed only on an available acknowledged owner.
The source's lifetime prevents mutation while borrowed. Generic legacy opens
replay the configuration but do not independently pin an expected Andon contract;
operators must retain source identity, exact windows and recovery floors.

The original credibility subtags 0–6 keep their exact encoding. Subtags 7 and 8
carry activation and window registration. Counts and field widths are bounded;
noncanonical/duplicate window IDs and all truncations refuse. Older decoders
refuse the new tags rather than silently dropping the lane.

Seven new regressions exercise original proposal, commit/reveal, application,
independent evaluator, canonical Store and replay. They cover order, wide costs,
delayed/censored labels, missing-window corrected retry, all five original Store
barriers, exact retries, pending/violation labels, and wire bounds/truncation.
Existing assertions and encodings are unchanged. Required remote-only RCH tests
and xtask are attempted separately; their actual result must accompany each
commit. Source inspection does not establish compilation or production safety.
