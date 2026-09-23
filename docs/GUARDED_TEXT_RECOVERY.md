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

## Evaluated, predictive and mediated recovery

The three additional entry points keep the same mandatory tokenizer/history
pin while returning the ORIGINAL role bundle for the selected guard profile:

- `open_evaluated_guarded_text_anchored` takes the exact evaluation protocol and
  returns `FileEvaluatedOversightRoles`, including its independent evaluator.
- `open_predictive_guarded_text_anchored` takes `FilePredictiveRequirements` and
  returns its consistency observer and exactly the optional evaluator declared.
- `open_mediated_guarded_text_anchored` takes `FileMediatedRequirements` and
  returns the topology observer, all base roles and the exact optional prediction
  and evaluation roles. It checks both initial/current graphs and pre-fence
  availability, not merely the original topology generation.

These paths do not approximate the higher profiles with a base guard. The three
existing validators are shared after an internal event preflight; original
readers and openers retain the same conditions. The anchored text path decodes
once and invokes the appropriate original validator for ONE numerical replay,
then commits ONE original fence. Role provisioning remains confined to the
private guarded implementation and occurs after acknowledged fencing; there is
no public event-import callback, owner-role getter or partially returned bundle.

The original recovery effects are intentionally not erased. An unanswered
forecast remains pending and is marked coverage-lost, retaining its lifetime
likelihood evidence. Old consistency observers cannot clear or replace that
record. A topology certificate and graph availability are withdrawn; a fresh
observer can supply a newer graph and obtain the original cut, but that does
not resume the paused decoder or authenticate the declared graph. The caller's
availability requirement describes the actual canonical PRE-fence image. After
an ambiguous visible fence it must not assert the earlier availability instead.

The evaluated entry point is the existing marginal-evaluation profile, not the
separately registered joint/held-out evaluation profile. Unexpected additional
contracts refuse through the original validator; they are not silently dropped.
Original unanchored APIs remain explicitly unanchored and retain their existing
scope. No existing requirement struct or journal event encoding was changed.

Eight additional `text_anchor_` regression functions cover completed native text
with evaluator custody, pending forecast preservation and old/fresh observer
behavior, all four optional-role combinations under topology control, original
cut withdrawal and reacquisition, refusal to downgrade to a weaker opener,
exact graph/evaluation/prediction matching before invalid-suffix execution, all
five replacement barriers in every new variant, and rollback with zero numeric
floors. Evaluation reports are retained as native reports, not fabricated labels.
The pending-forecast test uses the original declared external-capture profile,
not a claim of measured hosted residuals or calibrated predictor correctness.

Source review also corrected a causal fixture error in the first batch: native
decoder installation itself enables the publication guard, so that fixture could
not represent a valid unguarded decoder. The test now checks absent decoder and
absent tokenizer installations, paired with successful native installation; no
production admission assertion was weakened or bypassed.

All EIGHTEEN authored tests, Rust compilation, rustfmt, Clippy and the complete
gate remain UNEXECUTED. A second attempt of the same `text_anchor_` command also
failed before compilation because `rch` is unavailable (exit 127). Whitespace,
upstream/postimage Git hashes and exact extraction of the existing validator
bodies were checked separately. Those checks are not runtime qualification.
