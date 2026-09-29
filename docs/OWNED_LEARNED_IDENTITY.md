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
Required targeted and full RCH checks cannot launch in this environment because
`rch` is absent (exit 127); Cargo, rustc and rustfmt are also absent. Rust
compilation, tests, rustfmt and Clippy are UNEXECUTED. Selected-source hash and
patch checks are not runtime evidence. No Bead or qualification gate is closed.
