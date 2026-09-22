# Joint qualification in checked supervisor startup

FA-105/FA-062 implementation increment; plan 9.4, 9.7, 9.9 and 8.8;
FI-A07/FI-A08. The consumer is the existing checked-publication supervisor.

The concurrent upstream native JointPublicationProfile implementation is retained.
It already composes joint, witness, feed, freshness and snapshot-fallback selection
in one initial canonical image and pins them before recovery. This increment
adds check_joint_publication_profile: the existing owner can verify the SAME
complete selection before a frontend reads or binds a source. None still requires
absence; a different feed or lookup budget is not equivalent merely because the
witness limits match. No source observation or current qualification is implied
by configuration equality. Invalid selection does not poison or mutate the owner.

Three newly authored native tests pair rejected joint promotion with actual
qualified two-key publication, then change the final opaque model binding after
dispatch to ensure qualification cannot replace publication evidence. Recovery
keeps unknown charges until an endpoint nonexecution receipt, rejects historical
reactivation and old keys, and permits new qualification with new approvals.
The live-selection test checks unchanged authority and bytes on mismatches.
Existing upstream startup and storage-barrier tests are preserved, not counted
as new. Synthetic labels are not detector qualification.

Rust compilation, formatting, Clippy and tests have not executed. The required
RCH command cannot start because rch is unavailable. No qualification gate or
work packet is closed by these source changes.

## Runnable supervisor selection

The existing `WITNESS_PROFILE` argument now accepts an explicit wrapper schema:

```json
{
  "schema": "fa.supervised-joint-publication/1",
  "joint": {
    "id": 71,
    "generation": 1,
    "minimum_safe_roots": 100,
    "minimum_violation_roots": 100,
    "maximum_escape_ppm": 0,
    "maximum_false_stop_ppm": 0,
    "max_cases": 1024,
    "max_member_outcomes": 65536
  },
  "publication": {
    "schema": "fa.supervised-witnesses/1",
    "source": 91,
    "original": "/operator/original.bin",
    "current": "/operator/current.bin",
    "limits": {"bindings": 8, "steps": 10000, "value_bytes": 1048576},
    "requests": [{"kind": "absent_key", "key": 1}]
  }
}
```

The numbers illustrate the format; they are not calibrated requirements or
recommended reliability thresholds. Select the registered policy for the actual
evaluation population. The inner `publication` may be any existing supported
witness, producer, whole-input, waiting or snapshot-fallback profile. Nested
wrappers and unknown fields are refused. Whole-input modes still require opaque
input; a joint policy cannot turn an empty structured recipe into valid evidence.
The complete wrapper retains the 65,536-byte, 2,048-item and 4,096-byte string
limits; one outer object adds one level of parsing depth. Unwrapped profiles keep
the original five-level parser limit and all existing field semantics.

Use the wrapper with existing commands; no new actor verb or credential is added:

```text
supervise_publication create-checked CONFIG SUBMIT_JSON JOINT_PROFILE
supervise_publication submit-checked CONFIG SUBMIT_JSON JOINT_PROFILE --credibility-activation CAPSULE
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE JOINT_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE JOINT_PROFILE --credibility-activation CAPSULE
```

The existing reviewer-profile option remains available on the one-shot commands.
Without it, that reference transport still relies on namespace isolation; the
joint wrapper does not authenticate an evaluator, helper or human. Live service
commands retain their original connected-process admission and independent stop.
The current CLI publication schemas select PrefixV1 routing; the wrapper pins
that exact choice rather than silently opening an independently selected SubtreeV2
journal. This wrapper does not add a new routing-selection mode.

The supervisor's existing central create/open functions now select the composed
native owner. Neither the actor document nor the source/producer observation is
rewritten. Preparation on an existing owner checks the full selection before
reading or binding the original capture. Qualification files remain separate,
operator-controlled inputs to the original activation operation.

Selecting this bootstrap guard is NOT activation: baseline policy still governs
initial work until independently prepared held-out evidence is explicitly
activated. The existing create commands do not fabricate a campaign sequence
history. After an activated owner is reopened, new work still needs a fresh
qualifying activation in that same owner. Old actor keys remain exact-retry and
receipt-only; even missing capsule, producer and source files cannot cause their
re-execution. A legacy wrapper-free opener cannot disable an already persisted
native joint gate, but only the explicit wrapper independently pins its policy.

The supervisor increment adds four authored behavioral tests and a synthetic
helper-child fixture. Actual producer files, child-helper channels and reviewer
sockets exercise two baseline publications, qualified continuation, a changed
whole-input model binding after dispatch, and a source-free execution retry.
The paired complementary-helper case refuses joint regression before another
actor attempt or activation is recorded. Other cases cover strict parsing,
legacy separation, first-image completeness and configuration mismatches before
staging cleanup or source reads. The fixture is not a detector benchmark. These
Rust tests remain unexecuted pending the required operator RCH verification.
