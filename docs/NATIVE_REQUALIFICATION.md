# Native generation requalification

## Capability and invariant

A recovered native decoder is paused, while the original recovery fence makes
previous held-out qualification stale. The decoder admission boundary now admits
exactly the existing `ActivateHeldOut` and `WithdrawHeldOut` governance events
while paused or holding an unfinished generation. Without this composition,
reactivation was rejected before the original credibility reducer could run;
accepted evidence loss could also poison an otherwise healthy pending owner.

The same original activation still validates scope, actor/committee identities,
policy, exact control/epoch predecessor, evidence age, influence caps and any
configured held-out joint policy. Both transitions use the existing rights and
endpoint fences. They neither advance inference, import a cursor, reset a draw,
replenish a generation budget, change its prompt nor clear the decoder pause.
A new clock and explicit resume remain separate requirements. Historical
activation and withdrawal retries remain receipt-only and cannot reinstate or
withdraw a later qualification.

This is not blanket admission of credibility events: enabling protocols,
assessment and weight promotion retain their existing decoder admission rules.
No wire tag, journal encoding, model constructor, numerical reducer or authority
algorithm changes. This serves the existing credibility, replay and effect
boundary contracts (plan sections 9.9 and 11.2), not a new permission path.

## Native service consumer

The existing `--credibility-activation` option now reaches native `serve-open`
and `serve-open-checked`. It selects the SAME bounded canonical activation file
reader and governance transition already used by ordinary actor intake:

```sh
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  WITNESS_PROFILE --native-text RECIPE \
  --credibility-activation ACTIVATION_FILE > recovered-submit.json
```

For an unsubmitted pending or completed native generation, independent stop is
available before the file read and checked again afterward. Activation must be
new and current, not an idempotent historical receipt. Its control-sequence and
epoch predecessor are never rewritten by the executable. The original resume
path then refreshes source evidence/time and continues the saved intent. Helper
review, the independent human key and current publication witnesses remain
mandatory for the first effect. Failure follows the original stop/drain path;
no alternate qualification or unqualified retry occurs.

For a source-linked request already recorded in the journal, the same command
instead follows the ORIGINAL reconciliation/observation path. It does not open
an activation file even when the option is supplied. The old source document,
target, epoch and deadline remain exact; no model resume, new helper review,
human key, producer read or duplicate publication is introduced. A missing
qualification file therefore cannot prevent retrieval of an existing outcome.

The global option parser is unchanged: actor/reviewer commands and create modes
do not accept a credibility activation. New joint native streams are created in
the original baseline state with a frozen guard on later promotion; that guard
is NOT itself an empirical qualification. The service never fabricates campaign
history, advances control sequences to admit future evidence, rewrites an
activation, or turns elapsed milliseconds into evaluation sequence positions.

Both checked native create/open modes now accept the existing
`fa.supervised-joint-publication/1` envelope. A private complete selection routes
to the original atomic joint constructors. The old expressly non-joint
`generated_profile` still rejects joint policies. Its fields cannot be used as a
fallback when the joint opener rejects an absent or changed policy. See
[NATIVE_JOINT_PUBLICATION.md](NATIVE_JOINT_PUBLICATION.md).

## Tests and source status — 2026-09-26

Four library regression functions use the original numerical fixture, original
completed reviews to establish control history, independently labeled synthetic
campaigns, and real canonical files. They cover recovered activation versus
uninterrupted numerics; live pending activation/withdrawal and historical
retries; stale, foreign and repeated-generation rejection plus exclusion of
unrelated inference; and staging/directory-acknowledgment failure during loss.

Five executable regression functions (plus a synthetic child-helper entrypoint)
cover partial/completed continuation with both human decisions; missing,
historical and individually sufficient but joint-insufficient evidence; exact
expired receipts after removing qualification/producer/evidence files and helper
programs; original joint-policy substitution refusal; and public option routing
without expanding create or actor roles. The original physical model/source
fixture is reused. Seed reviews are real original reducer transitions, not
edited sequence numbers. The fixture does not establish detector quality,
independent evaluator authenticity, hostile-process isolation or joint-failure
rates outside its own manifest.

The required fresh RCH gate and targeted `native_joint_` test attempts failed
BEFORE compilation because `rch` is not installed (exit 127). The preparation
environment has no complete checkout. All nine new regression functions, Rust
compilation, rustfmt and Clippy are UNEXECUTED. No prior receipt qualifies these
changes, no Beads item is closed, and no production qualification is claimed.
