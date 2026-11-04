# Topology-aware publication bootstrap

## Implemented native boundary

`FileOversight::create_mediated_guarded_reserved` composes the original guarded
bootstrap with a `FileMediatedBootstrap`: the declared authority graph, an explicit
recovery reserve and optional **plain** publication-witness limits. The original
source, credential-registration, numerical, prediction and evaluation validators
remain in use. A stream and a plain witness gate cannot replace one another.

The first canonical image contains all selected bootstrap events, including the
reserve before ordinary guard work and an uncertified topology. The original
Store performs one replacement. Native validation precedes directory creation;
a failed replacement returns no owner or roles, even when its complete image may
already be visible. There is no intermediate source-free, topology-free or
unreserved owner to recover after an interrupted multi-step setup.

The separately held topology observer must still obtain an independent native
cut check. Configuration is not a certificate, human approval, or effect key.
Original guarded recovery preserves the selected reserve and source while
withdrawing old graph coverage; it does not restore old observer custody or
permit silent recertification of a withdrawn graph. Checked witness recovery
still needs its matching full native requirements; this constructor does not
invent an automatic recovery profile.

## Consumer and scope

This composition supports the publication operator's topology-source integration:
install the complete guarded owner first, bind the independently provisioned
source to its separately returned observer, then retain source custody in the
original driver. No new executor, dependency, authority algorithm, event encoding
or canonical replacement protocol is introduced. The graph remains conditional
on a complete declared inventory and functioning modeled enforcers, not proof of
host isolation, authenticated capture or absence of unmodeled egress.

`publication: Some(...)` selects only the existing plain witness gate. It cannot
stand in for a change-feed, freshness, joint-evaluation or snapshot-fallback
publication profile. Such consumers must use their complete matching bootstrap.

## Change log and verification status

2026-10-15: added reserved mediated first-image composition, retaining the original
unreserved API. Five new native regression functions cover source/witness
combinations, original cut certification, guarded recovery, every original Store
barrier, invalid preflight and preservation of an existing store. The tests use
real local-file operations and original reducers; controlled faults are not
hardware power-loss or deployment evidence.

Required launch attempted:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference reserved_bootstrap
```

It stopped before compilation with exit 127 (`rch` is absent in the editing
environment). Rust compilation, tests, formatting and Clippy are unexecuted.
FA-072/FA-073 and their prerequisites remain open. This is unqualified source,
not a promotion of the implementation or deployment gates.
