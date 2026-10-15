# Alignment Observatory

A read-only visual projection of the original publication journal, for local
operators. The native snapshot adapter uses `FileOversight::read_publication`:
it never opens an authority owner, fences a run, cleans staging, launches helpers,
acquires fresh policy evidence, approves a request, or performs an effect.

The `fa.observatory/1` envelope distinguishes `known` canonical replay from
`unknown` read failure. Known means a validated **historical observation** of the
selected canonical image; it does not attest an owner's acknowledgment, current
source freshness, reviewer liveness, or the safety of an effect. Payloads, helper
inputs, salts, programs, environment variables and credential contents are not
included. Full-width identities and counters are decimal strings so the browser
cannot round them. Configured helper weights are bootstrap metadata, not a live
qualification or credibility claim.

A machine-readable snapshot can be emitted with:

```sh
cargo run --locked -p fa-reference --example alignment_observatory -- \
  --snapshot /absolute/path/to/supervised-publication.json
```

Use the original supervisor's independently retained CONFIG; its bootstrap must
match the journal. Missing, corrupted, incompatible or unsupported loaded-model
histories do not become an empty healthy run, a demonstration, or a recreated
store. Snapshot reads are bounded by the original journal limits but replay is
synchronous and may be expensive. This is not a constant-time production metrics
exporter, an authenticated remote service, or the full FA-134–FA-140 agent surface.

This serves the existing plan §17 operator projection and §6.7 separation of
observation from authority. No journal, policy or authority semantics change.
Three native regression tests cover inspection alongside a locked owner,
unchanged bytes/no fencing, redaction, loss/corruption, and exact JSON counters.
They are authored but remain unexecuted until the required remote RCH gate runs.
