# Pre-action forecasts and sequential consistency evidence

## Numerical implementation

`action::consequence::activation::consistency` supplies a frozen binary forecast
from exact activation-probe bands and a bounded exact likelihood-ratio process.
The existing source-checked binary32 decoder and exact linear probe compute the
negative, zero or positive band. A registered table maps each band to null and
alternative probabilities for a registered payload-prefix category. No margin
is mislabelled as a probability; registration/calibration remain trusted inputs.
This is an exact-reconstruction baseline, not learned predictor training.

Probabilities have denominator 65,536 and strictly positive support for both
categories. Each observed category contributes q/p or (1-q)/(1-p). Under the
explicit null that p is conditionally calibrated given the entire prior history,
the conditional mean factor is one. A sum of negative log probabilities is not
used. No independence between frames is asserted; conditional calibration is
stronger than matching marginal frequencies and is not proved by this code.

Products use 129 fixed u64 words each for numerator and denominator, with checked
integer cross-products against the frozen lifetime alpha. Up to 512 factors
below 2^16 plus a 64-bit alpha cross-product fit in 8,256 bits. There is no
floating-point underflow or rounded threshold test. First crossing is latched,
even when later evidence decreases. Historical evidence is not a Permit.

## Verification and roadmap scope

Nine unit tests cover finite support, likelihood construction, exact equality,
latched crossings, the maximum sample count, extreme ratios, independent u128
short-path comparisons, all 256 outcomes of an eight-step declared Bernoulli
null, and real activation-band selection. **These Rust tests have not run.**
Cargo/rustc/RCH are not available in this session; compilation, formatting,
Clippy and repository qualification remain pending. No bead is closed.

This is reference progress toward FA-111 and plan 12.7/13.2, grounded in FI-A12.
It does not establish detector quality, calibration, future-token isolation in a
real host, authenticated capture, durable statistical history, live permission,
or the full production packet. Broker enforcement is a subsequent integration
step; this numerical module imports no authority or runtime.
