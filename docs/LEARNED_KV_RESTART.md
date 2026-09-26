# Direct exact-KV restart with fresh learned audits

## Implemented source boundary

`decoder::monitoring::restart` connects the existing exact all-layer decoder
restorer to the original learned publication guard. This is the numerical
restart work called for by plan sections 10.1, 11.1 and 11.7, not a new authority
path, containment command, service recovery format or qualified restart grade.
The reference API consumes its own typed, in-memory original state. It is not
an importer for arbitrary KV images, learned latents or serialized verdicts.

`LearnedDecoderSession::checkpoint_kv` captures a complete original accepted
prefix, including its actual model object, all K/V layers, token history and
next-token logits. A held or failed guard cannot checkpoint its shorter prefix
as an active run. Immutable checkpoints may be shared; the source owner remains
unchanged, including a hold reached after an earlier checkpoint was taken.

`MonitoredKvCheckpoint::begin_restart` first admits the destination through the
original constructor and its training-split checks. Zero or source-equal stream
IDs are refused. A new stream identifies derived buffers, not new independent
source evidence. The original evaluation origin and original source descriptor
remain attached to the restart audit. This API does not provide a global stream
ID registry or deduplicate repeated experiments.

## Fresh evidence before a new executable owner

Every nonempty prefix is freshly compressed with its frozen fitted codec,
checked against the retained ORIGINAL cache by `CheckedLearnedKv`, and audited
by its complete original `LearnedModelMonitor`. No saved quiet result is used.
`None`, `All` and structural `Heads` residual retention are preserved, including
the same head selection at every absolute position. No candidate policy, omitted
layer, different model object, or enlarged fixed policy can be supplied.

A preparation retains the new audit for inspection. `finish` releases a new
monitored session only after that audit is completely quiet and the ORIGINAL
transactional all-layer restore and recapture succeed. An incomplete/budget-held
audit cannot finish. Empty-prefix readiness means only admission: there is no
fabricated quiet audit for zero observations. The returned session has no old
last-event receipt and every subsequent token executes the original inference,
source checking and learned publication guard.

Restoration copies exact original KV and logits; it does not substitute lossy
codec reconstructions. It executes no historical decoder matrix/attention work.
This is not the stronger claim that restart is constant-time or free: complete
prefix compression, source checking, monitoring, exact writes, recapture and
metadata copying have real cost. The receipt retains those original audit and
restore work reports. The original recomputation/archive verifier is unchanged.

## Sampled-generation continuation without budget resets

`sampling::monitored::restart` extends this same path to the complete
`LearnedGeneration`, not just a forced-token decoder. Its `checkpoint_kv`
retains the exact original sampler state and sample records, frozen prompt and
stop IDs, maximum continuation length, generation status, numerical estimate,
original numerical and aggregate telemetry ceilings, and all historical spend.
Only active or finished original generations can construct this checkpoint;
held and failed attempts cannot export their shorter accepted prefix as active.
The captured token history, prompt prefix, draw counts, phase/stop state and
numerical accounting are checked together before the typed state is returned.

`GenerationKvCheckpoint::begin_restart` delegates to the freshly audited KV
preparation above. No generation, random state or pending output escapes when
that audit is incomplete. A successful `finish` returns a `LearnedGeneration`
using the ORIGINAL generation advance method, the restored original guard and
`Sampler::from_snapshot`, never a freshly seeded sampler. All remaining sample
capacity is reserved before release, preserving the original pre-reserved
sample append. No prior `last_event` is republished as a new observation.

The cache stream changes to identify derived buffers; the original sampler
stream, random state and evaluation origin do not change. Each subsequent
sample therefore uses the next original random draw and the same frozen
sampling policy. Prefilling stays at its original prompt position. An EOS or
token-limit checkpoint stays terminal; it does not acquire another continuation.
Repeated typed restarts retain accumulated numerical and telemetry spend.
A source already held after an earlier checkpoint remains held, and the
restarted branch must still pass the same monitor on every new token.

`GenerationKvRestartReceipt` keeps historical generation work distinct from
fresh full-prefix auditing and KV writes/recapture. Neither historical spend
nor fresh restart work is erased. Fresh restart auditing has its own bounded
allowance; it does not enlarge the original generation's remaining source-check,
refinement, decoder-product or vocabulary-score ceilings. A generation with no
remaining telemetry can be restored after a separately paid fresh audit but
still fails its next token under the original conserved allowance. Failed
attempts cannot commit a random draw or disclose a sampled candidate.

These are sealed in-memory checkpoints, not trusted deserializations. There is
no conversion from a bare cache, saved audit verdict or unverified archive, and
no mutable sampler, guard or budget accessor. The existing archive/replay
verifier still recomputes and compares the original full expected state; this
API does not give imported saved words a shortcut around that verification.

## Resource and scope limitations

Snapshot and restore check complete retained cache-value limits. Fresh audit
allowances intersect the original policy ceilings. The complete prefix must fit
the original monitor's registered row capacity and aggregate budget. In this
profile a long prefix can therefore fail even when every incremental one-token
audit previously passed. It does not widen a monitor to force admission, silently
check only the last token, or fall back to unaudited restore.

These limits describe numerical work and retained values, not wall time, peak
RSS, storage leases or a global experiment-budget escrow. A failed compression
or source check can have bounded work without a successful receipt. Repeated
preparations each incur fresh work and do not become independent observations.
No external witness freshness, human authorization, effect permit, revocation
floor or incident ledger is restored. A service must still use its own admitted
current-source and authority path; this API does not implement reset containment.

## Verification status

Fifteen new integration test functions use the ORIGINAL numerical/compression/monitor paths
with nonzero attention, including an explicit control showing that history
actually affects logits. They cover exact continuation against uninterrupted and
independent recomputation, source/destination lineage, incomplete re-audits, exact
and one-less cache/source-check limits, full-prefix row coverage, subsequent
alarms and persistent source holds, empty/stale cases, and all retention modes.
Seven of these tests exercise sampled generation: every empty/prompt/sample/
terminal checkpoint cut, exact random words and probability bits, repeated
restarts, exact lifetime numerical allowances, exhausted aggregate telemetry,
EOS, later held sources, blocked fresh audits and stale calls. The eight
forced-token tests and their assertions remain unchanged. Four compile-fail
examples cover byte import and mutable preparation escape at both levels.

Tests are authored, not executed. The required RCH build/test gate cannot start
in this environment because `rch` is absent (exit 127); `cargo` and `rustc` are
also absent. No compilation, rustfmt, Clippy, measured speedup, functional restart
grade or production qualification is claimed. Static source/hash checks cannot
supply those missing results. No roadmap packet or Bead is closed by this work.
