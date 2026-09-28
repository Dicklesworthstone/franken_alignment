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

Without a stop policy, a learned hold closes numerical eligibility and the
supervisor uses the existing stop path. With the explicitly installed policy
below, the same original authority stop is serviced automatically. Neither
mode creates another effect ledger, currentness proof or authority reset.
Durable learned-source recovery and native CLI wiring remain separate.

The broker's [paired learned checkpoint/reset path](HOSTED_LEARNED_RESET.md)
now joins the original numerical restorer to the existing containment authority.
It preserves spent work and rights, retains terminal stops, and requires fresh
resumed evidence before another effect. Generic reset remains unavailable in
owned mode; registered grade and pre-incident selection are separate obligations.
The [supervised learned lifecycle](HOSTED_LEARNED_SUPERVISION.md) exposes the same
reset while connected or offline and services learned trips through the existing
endpoint fence/drain path.

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

## Fixed automatic containment

`enable_learned_host_stop` reuses the original `HostedStopPolicy` and requires
installation before the owned generator's first token. Its selection cannot
be replaced or disabled after results appear. Every actual nonquiet learned
review, admitted failure, or interrupted actor synchronization triggers the
original `request_stop`, before more inference can run. Stale calls, normal EOS
and ordinary completion do not fabricate incidents. The first cause retains
its numerical/telemetry inspection and original stream/evaluation lineage,
without withheld token IDs, cache words or RNG state. A monitoring-budget hold
is distinguished from an alarm; failed preparation is an operational failure.

The original stop suspends admission, advances the revocation floor and cancels
only undispatched reservations. Already-dispatched effects remain charged and
unknown until original endpoint reconciliation supplies an actual outcome.
`progress_stop` performs endpoint fencing and settlement separately: a local
stop receipt is not a nonexecution receipt. Real executed outcomes retain their
charges; endpoint failure neither refunds them nor resumes inference. Repeated
supervision returns the same stop receipt rather than another transition. Any
stop error is retained for retry while the numerical owner remains closed.
A prior completed manual stop is not relabelled as a new learned incident.

A caught unwind closes shared evidence immediately through the synchronization
guard; the next scheduled hosted call or explicit `enforce_learned_host_stop`
services containment before new computation. This is not a crash watchdog or
an asynchronous OS/process kill. Human-key, helper and endpoint protocols are
not replaced, and a stored human approval is not falsely called consumed just
because its original authority basis has been stopped.

Eight additional runtime tests cover quiet original computation, genuine alarms
at authorized/dispatched/executed cuts, missing versus budget-held audits,
two-key invalidation, frozen bootstrap, synchronization errors and unwinds,
endpoint fencing failure/retry, and prior manual suspension. Assertions use
the actual `Inspection::charged` field; two stale references to nonexistent
`spent` in the earlier learned-gate tests are corrected without changing their
expected values. Eighteen new runtime test functions and two compile-fail
examples are authored, not executed. Both fresh targeted and full RCH attempts
still stop before compilation; no source-only result qualifies these features.
