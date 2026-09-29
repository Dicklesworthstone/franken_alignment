# Identity measurements from the owned learned model

This source addition connects the existing identity-stimulus engine to the
original broker-owned learned generator (FA-113; plan model-identity contracts).
It does not replace the identity matcher, numerical engine, or authority gate.

`OversightBroker::hosted_learned_identity_probe` selects immutable parameters
from that broker's actual original learned generation. The caller supplies the
registered passport, actor predecessor, measurement sequence and product ceiling,
not a substitute model, cached frame or asserted match. The existing
`DecoderIdentityProbe` validates every anchor stimulus, residual contract and the
aggregate product allowance before performing numerical work.

Each probe advance executes the original decoder in its private anchor session.
The live generator's cache, sampler, continuation budget and telemetry are not
mutated or refilled. Probe costs are reported by the existing `IdentityProbeWork`
and are separate from live generation costs. This is a bounded per-probe API,
not a lifetime measurement escrow or a preemptive scheduler.

A diagnostic probe may also inspect a held, failed or paused generator's immutable
parameters. It cannot clear the hold, resume generation, install eligibility or
issue either publication key. The detached probe binds the model selected at
construction; a consumer must separately bind the original identity challenge
and reject stale actor/control/source observations before applying measurements.

Manifest digest fields remain independently supplied registration/observation
data, not hashes calculated by this API. Exact equality to reference anchor
intervals is not authentication and does not establish discrimination against
all possible model substitutions. Returned anchor frames are sensitive operator
evidence, not an actor-facing model-input channel.

## Durable challenge integration

`FileIdentityObserver::learned_decoder_probe` constructs the original bounded
`FileDecoderIdentityProbe` from a begun `FileIdentityChallenge` and the current
journal owner's original learned model. The challenge supplies the registered
passport. No argument can select a substitute model, probe, anchor subset or
cached activation. Construction writes no journal and executes no token.

The observed manifest remains an independent input; expected metadata is not
silently reported as an observation. The original runner records that manifest
first and closes on its original mismatch/containment result before inference.
It then executes one original anchor token per numerical step, re-observes the
trusted clock after computation, records actual residuals, and separately applies
the original identity gate only when every mandatory anchor matches.

Every original actor/control/epoch/basis check, whole-roster budget, current
challenge check, deadline, durability barrier and interruption latch is reused.
Changed actors and withdrawn identity bases close partial runs before more work.
Foreign roles cannot take over. An expired or interrupted measurement cannot
be replayed through this runner as a fresh successful observation.

Recovery provisions fresh roles and challenges rather than resuming old runs.
A paused learned owner may be measured for requalification, but a matching
identity cannot itself resume it or recover its old effect keys. Original resume,
source-bound sidecar preparation, congress, human approval and checked publication
remain distinct operations. A numerical matcher success is not a publication.

Legacy trusted external-probe/manual observation interfaces are unchanged; this
is an explicitly selected owned-model path, not a claim that all host ingress is
authenticated or that every existing caller is forced onto this constructor.
Historical journal replay uses the existing recorded identity observations, not
a new numerical identity witness format or a new proof of remote provenance.

## Verification status

Six integration regressions and one compile-fail boundary are authored. They
compare actual anchor execution with the original standalone engine, check
subsequent live sampler/cache behavior against an untouched control, distinguish
two different parameter sets with identical labels and passport, exercise exact
versus one-less aggregate budgets, reject stale/absent/invalid source admission,
and retain a real monitor hold despite successful diagnostic measurements.
A detached probe remains historical if the live actor advances.

The fixture uses real tiny-model inference with synthetic coefficients and
registered intervals. It is not trained-detector or passport qualification.
Ten further real-journal regression functions exercise actual identity through
sidecar/congress/two-key publication, a same-label changed-parameter mismatch,
independently observed manifest mismatch, exact setup budgets, source/basis loss,
foreign custody, receipt-time expiry with a timely control, fresh measurement
while paused after recovery, failed persistence after anchor inference, and a
caught post-inference clock unwind. There are sixteen authored regression
functions and two compile-fail examples across both increments. Original identity
runner bodies, matching rules and journal formats are unchanged.

Required targeted and full RCH checks cannot launch in this environment because
`rch` is absent (exit 127); Cargo, rustc and rustfmt are also absent. Rust
compilation, tests, rustfmt and Clippy are UNEXECUTED. Selected-source hash and
patch checks are not runtime evidence. No Bead or qualification gate is closed.
