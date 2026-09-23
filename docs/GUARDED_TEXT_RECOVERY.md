# Exact-history guarded text recovery

## Positive capability and boundary

`FileOversight::open_guarded_text_anchored` combines three existing contracts in
one exclusive recovery: the durable text tokenizer/model pin, the full base
guarded-role requirements, and an independently retained exact-history anchor.
The existing text-only opener returns only the human reviewer and does not match
the complete optional guard set. The existing guarded opener does not pin the
text tokenizer. Neither is silently substituted for this combined entry point.

The actual consumer is a supervisor restarting a guarded durable text-generation
owner. It can recover pending generation cursors and spent sampling allowances
while receiving the original human, identity-observer and policy-governor roles
for separate custody. It does not need to weaken its recovery requirements or
reconstruct a reviewer from the actor port. This serves FA-014, FA-025 and the
original nonrewind/resource/identity contracts; no packet is closed by source.

The caller supplies the complete `FileRecoveryRequirements`, the immutable
`ByteBpe`, and a trusted `FileHistoryAnchor` retained outside the rollbackable
journal. The tokenizer must bind the expected decoder profile and fit the
original 8 MiB durable-tokenizer ceiling. One Store supplies one decoded event
history. The anchor checks the exact canonical prefix, including storage identity;
the tokenizer and model/monitor/sampler event inventory is matched before token
replay. This includes named-control recognition bytes, not just generation IDs.

The original guard validator then checks all optional gates, effective policy,
credential state and counter floors. The original machine validates the entire
suffix, including all generation witnesses and spent budgets. Only afterward
may canonical cleanup and the single native recovery fence occur. Role custody
is returned only after that fence is acknowledged. No alternate reducer,
permission ledger, tokenizer, numerical cursor or journal encoding is introduced.

The fence pauses inference and withdraws saved clock/source/identity eligibility
and old effect approvals. Historical text remains readable, including incomplete
UTF-8 and exhausted requests. Pending work requires fresh time, explicit resume
and the original active guards; it cannot restart a partial request or widen its
allowance. Unknown effects are not resent or refunded. Storage failures return
no candidate owner or roles, and recovery must inspect the actual canonical cut.

This base profile rejects additional independent-evaluation, prediction and
mediation contracts. The independently stored anchor is trusted data, not a
signature; replacing both the anchor and journal with old copies defeats it.
An unanchored suffix still requires semantic replay and is not authenticated by
the prefix. This is the existing local journal sink, not remote atomicity or a
claim of pretrained-model accuracy, filesystem isolation or deployment safety.

## Authored regressions and qualification

Ten `text_anchor_` tests reuse the unchanged durable-text synthetic-weight
fixtures and original numerical implementation. They cover every pending/output
boundary, role preservation, pause/resume, valid rollback under zero floors,
a valid equal-counter fork accepted by weaker recovery, pre-replay tokenizer and
model mismatch, invalid suffixes, guard/policy/credential/floor mismatch, foreign
storage, absent guards/tokenizers, exact fence capacity, all five original Store
replacement barriers, split UTF-8 and retained exhausted sampling work. Positive
counterparts remain alongside every refusal class.

The targeted attempt failed BEFORE compilation: `rch: command not found`, exit
127. Tests, compilation, rustfmt, Clippy and the complete gate are UNEXECUTED.
Source whitespace and uploaded Git blob identities are checked separately; they
are not execution evidence and historical receipts do not qualify this source.

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference text_anchor_
```
