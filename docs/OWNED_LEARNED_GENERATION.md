# Broker-owned learned generation

## The actor copy comes from the original generator

`OversightBroker::own_learned_generation` connects the original learned-K/V
source to the existing actor state and effect gate (plan sections 7.2, 8 and
10.15). It constructs a fresh original generator at trusted bootstrap, pins its
liveness handle, and synchronizes the initial canonical cache and sampler.
There is no adoption of an advanced/restarted generator, experimental state,
foreign model, caller-supplied quiet flag or replacement source. The complete
originally declared horizon must fit the existing actor-cache size limit before
ownership changes. Bootstrap refuses a nonempty pre-existing actor prefix.

`advance_hosted_learned` delegates each prompt/sample step to the original
`ObservedLearnedGeneration`. After a quiet accepted token it copies the actual
complete canonical KV image, exact sampler encoding and token prefix through
the original actor replacement operation. The actor revision advances only
when that synchronization succeeds. The shared evidence, actor copy and latest
review now agree without a supervisor manually inventing opaque cache bytes.
No mutable generation, raw actor replacement, generic reset or unpaired
checkpoint path is available in this owned mode. Residual-hosted and externally
observed configurations retain their existing behavior and cannot replace it.

A held token remains withheld. Its attempted numerical/telemetry costs remain
recorded, but neither its unaccepted cache row nor uncommitted random draw is
copied into actor state. Stale calls are free. A synchronization error or caught
unwind latches the host and withdraws its shared source even if the numerical
step had already been accepted. The last synchronized actor copy is retained;
it is not misrepresented as the newer numerical position. Inspection explicitly
separates the numerical status from a synchronization failure. The composition
is synchronous and callback-free; this is not a cross-process transaction.

## The original authority still decides effects

The existing learned decoder gate binds proposed effects to this exact live
source and actor revision. More inference invalidates an older proposal basis;
a new proposal uses the newly synchronized state. A quiet token does not bypass
congress, witnesses, epochs, human keys, deadlines or endpoint fencing. Terminal
EOS/token-limit generations stay terminal but can support review while their
quiet source remains live. Dropping the broker closes that source. Manual
suspension blocks further inference without refunding historical work.

This first owned profile does not automatically suspend the authority on a
learned hold; it closes numerical eligibility and lets the supervisor use the
existing stop path. No new effect ledger, currentness proof or authority reset
is created. Durable learned-source recovery and native CLI wiring are separate.

## Costs and verification

The actor copy serializes the complete accepted cache after each quiet token,
so accumulated byte movement is quadratic in a growing prefix. The host does
not claim incremental storage, zero-copy synchronization, low peak RSS or a
measured speedup. Original generation and telemetry budgets remain unchanged;
actor serialization is bounded by the existing state-size cap, not charged as
new decoder products. Retained proposal evidence has its original gate budget.

Ten runtime test functions exercise actual nonzero-attention inference,
compression and monitoring, exact actor cache/sampler equality against an
independent original generator, congress/two-key publication, stale approvals,
held/failed attempts, bootstrap isolation, whole-horizon capacity, forbidden
state replacement/reset, suspension, EOS and injected synchronization failure
or unwind. The compile-fail example rejects mutable generation extraction.
Tests are authored but unexecuted. The required RCH route is unavailable here
(exit 127 before compilation); local compilation is not an allowed fallback.
No source-only checks qualify a detector, restart profile, release or Bead.
