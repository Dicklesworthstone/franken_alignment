# Recover every retained learned message outcome

The learned publication operator's `--resume` now selects the original message
from the durable request book, rather than requiring a successfully admitted
`FrozenAction`. A policy-refused message is still a retained request: losing its
stdout receipt must not force a new request, renewed deadline or new inference.
This is scoped FA-014 recovery work, serving plan sections 8.4 and 16.4 and the
founding effect-boundary/evidence principles. Full packet acceptance stays open.

`FileOversight::learned_text_stream_message_request` is a historical data accessor.
It requires an existing key and uses the original source-only retry validator:
version, scope, complete stream profile, release kind, witnesses, full-frame
charge and retained deadline are checked. No output is rebuilt from today's
model state or audience prefix. A finish is not accepted as a message. Faulted
owners and absent keys refuse. The returned proposal is not an endpoint receipt,
permit, new request or proof that anything was published.

The existing guarded model/predictor replay, reserve, external floors and anchors
still precede live-owner recovery. The original source-only gateway recovers the
exact ticket; the existing receipt handler preserves refusal, cancellation,
confirmed execution and unknown liabilities. Terminal recovery reads no live
policy/helper assets and consults no logical clock. A denial remains a denial,
including the executable's nonzero exit status; it is not relabeled success.

Two regression functions cover legacy, required raw-residual and required owned-K/V
profiles. Paired scenarios recover a policy-refused request with no FrozenAction
and a real numerically generated, native-reviewed, independently human-approved
confirmed publication. Further cases reject absent/zero keys without writes and
retain the exact original deadline, numerical state, request disposition and
forecast sample count after live assets have been removed.

These Rust tests are authored but unexecuted: the required RCH launcher and Rust
toolchain are unavailable in this session. Source-preimage and whitespace checks
are not compilation or runtime validation. No production, calibration, performance
or full-workspace qualification is claimed. The initial increment did not yet move receipt selection ahead of the
recovery fence. The integration below completes that step without changing the
original effect or recovery semantics.

## Pre-fence request selection

The completed integration selects the original message or exact finish/predecessor
on the same exclusively locked, fully replayed cursor, **before** its sole recovery
fence. A missing request, a message/finish mismatch, or a wrong finish predecessor
returns no owner, performs no staging cleanup and consumes no journal event or
recovery reserve. This supersedes the first-increment limitation above; no second
replay, independently opened snapshot, or caller-supplied candidate is substituted.

The base `FileLearnedRecovery` now offers the same read-only message and finish
accessors. Incomplete, failed or interrupted replay returns no descriptor. The
ordinary, raw-forecast and owned-K/V guarded wrappers each recheck their complete
independent guard, source/model/predictor inventory, policy and external floors
before forwarding. They do not expose their inner replay, mutable machine or
role bundle. The original live source-only intake and sealed cursor share one
retained-request validator and the same accepted predecessor-receipt checks.

The actual `--resume` and `--resume --finish-after-message` paths use this selection
within the original one-event replay driver. Its single monotonic budget still
checks pre-open, every original event and Ready. The successful finalizer still
rechecks the exact canonical bytes before its original cleanup/fence, then the
source-only actor gateway and existing receipt handler perform their ordinary
exact retry. Correct recovery still spends that one fence; this is not a read-only
terminal receipt service and repeated valid recovery is not capacity-free.

Four additional regression functions cover all three operator profiles with
anchored missing/wrong-kind requests, correct denied receipts, replay expiry,
unmet external floors, descriptor equality across the single fence, withheld
pre-Ready descriptors, and canonical-file replacement after selection. The
finish cases use an original confirmed message followed by either a policy-refused
finish or a fresh native-reviewed and independently human-approved confirmed
finish. Wrong selection retains the entire store and old anchor; corrected
selection recovers the original terminal result with no logical clock or live
assets. Existing elapsed-replay tests retain their unchanged assertions through
the same shared timer/finalizer driver. No older negative test was weakened.

These four new tests, like the first two, remain **unexecuted**. Targeted and full
remote-only RCH commands were attempted and could not start compilation because
the launcher is absent. There is no Rust toolchain here. Git preimage and whitespace
checks do not qualify Rust syntax, compilation, tests or performance. One event,
frame validation and filesystem work remain synchronous; this adds no hard wall-time,
physical-resource, adversarial-filesystem or production guarantee. FA-014 stays open.

## Retained nonadmission and policy denial (2026-10-24)

The original controller distinguishes two terminal refusals. A complete known
policy violation creates an admitted `Denied` action. A failed native admission
can retain `NotAdmitted(error)` without a retained `FrozenAction`. Both must
remain recoverable; neither is successful publication. Earlier descriptions and
fixtures above conflated these states when using a known-false policy value.

An already-expired source-only stream envelope is refused before source I/O or
request retention. That gate is unchanged, and expiry cannot be assumed to
produce a retained receipt in every learned profile. The retained nonadmission
fixture instead submits the actual completed model's frame with one mismatched
semantic epoch in the supplied snapshot. The native broker compares it with the
actual captured source and records `NotAdmitted(Binding)`, with no action. No
caller-supplied outcome or replacement request record is installed. Historical
source-only lookup uses the original retained frame and unchanged deadline,
including when no action exists. This is a trusted-owner API negative control;
it does not claim that an expired live actor intake created a request.

The corrective fixture work keeps the original nonadmission obligations and
adds admitted-denial cases beside the original native/human-confirmed positive
in the same message, finish, selection and recovery tests. This clarifies the
existing native contract rather than changing admission or deadline rules.
Current execution status is recorded in `IMPLEMENTATION_STATUS.md`; authored
cases and pending remote attempts are not qualification evidence.
