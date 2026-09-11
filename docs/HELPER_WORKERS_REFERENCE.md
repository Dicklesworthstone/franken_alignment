# Per-member helper workers

Consumer: the existing frozen oversight congress, not a second reducer. This is a reference implementation increment for FA-018/FA-019 and FA-107, plan sections 9.1 through 9.3, serving FI-A03, FI-A06, FI-A13, FI-A14 and FI-A16. It does not qualify the complete production packets or their dependencies.

## Member-local interfaces

`HelperRound::new` takes ownership of a fresh `ObservedSession` and provisions exactly one non-cloneable `HelperPort` for every frozen member. A port exposes only that member's original evidence-view manifest and reference commitment challenge. It cannot select another member, inspect another view or vote, change the roster, apply the completed review, or obtain an effect permit. The coordinator retains the original session; there is no mutable-session escape or caller-vote fallback after worker admission. A session with a previously accepted caller commitment is refused.

Each worker queues one reference commitment, waits for the coordinator's reveal signal, then queues one bounded verdict/salt reveal. It supplies no clock reading. The supervising host calls `advance` with trusted elapsed time, and the original session's half-open commit/reveal deadlines govern acceptance. Queuing bytes before a deadline does not backdate receipt if the coordinator first processes them after the cutoff. The complete input closure is checked before cloning views; the roster and per-worker salt buffers are bounded. Invalid phase operations do not create queued work.

The coordinator opens reveals only when every frozen member committed or the commit deadline closed. Disconnected, expired or invalid-reveal workers remain missing in the original congress denominator, never converted to abstentions or replaced with fabricated votes. `finish` returns the original `ObservedReview`; the existing broker must still validate its issuer, predecessor, current policy, actual input revision, and all configured dispatch prerequisites. A completed permitting review is evidence, not a permit. Actor interfaces receive no worker handle or private worker-health report.

## Reference commitment boundary

The worker bridge imports a raw digest into the existing session/member brand, then subjects its reveal and final transcript to the existing checks. It deliberately uses the current `round::commitment` FNV comparison oracle. This is NOT authenticated, collision-resistant or production cryptography. A real production worker profile still requires admitted audited cryptography, peer/model authentication, provider execution/capture evidence, OS isolation, resource scheduling and qualified runtime integration. No test in this increment establishes those facts.

## Verification source

The public tests pair successful two-helper congress-to-publication with phase violations, missing workers, invalid reveals, precise deadline receipt, disconnection, coordinator loss, scoped input visibility, capacity bounds, non-pristine session refusal, stale input revisions, and exact-disqualifier dominance. The effect still crosses the original one-shot permit and endpoint receipt path. Two compile-fail examples forbid cloning a worker port or converting it into a broker.

September 11, 2026: Rust source and test source added. Cargo, rustc, rustfmt, RCH and br are unavailable in this editing environment; none of these tests has been compiled or run here. Historical execution receipts do not qualify this increment. The br-managed beads are not hand-edited or closed. No dependency, runtime, cryptographic or release admission is changed.
