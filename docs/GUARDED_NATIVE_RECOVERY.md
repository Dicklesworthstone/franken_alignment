# Guarded recovery of native generated publication

The original source-bound native generated-text owner now has an independently
guarded recovery entry point:

```rust,ignore
FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
    directory, profile, &requirements, &decoder, &tokenizer, selected, &anchor,
)
```

It returns `FileGuardedGeneratedSourceRecovery`, a sealed wrapper around the
existing `FileGeneratedSourceRecovery`. This is the recovery composition for
plan sections 8.3, 8.5, 11.2, 16.4, 16.9 and 22.7, serving FA-026 and FA-130.
It adds no journal event, inference implementation, dependency, policy reducer
or alternative storage owner.

## Independently selected requirements

The caller supplies the original decoder, tokenizer, source and publication
selection plus a complete `FileRecoveryRequirements` and retained
`FileHistoryAnchor`. The source and stream in the guard set must agree with the
native selection; the selected decoder must be exactly the guarded decoder.
The anchor is checked against the same exclusively locked canonical image
before the first numerical replay event.

The complete guard inventory is checked against the original reconstructed
machine. A missing optional guard requires its absence; it is not permission to
accept whatever the journal contains. The effective policy is compared as a
complete policy, including its predicates, rather than by generation alone.
Credential generation and revocation state, when selected, remain exact.
Independently retained journal-revision, control-sequence and authority-epoch
floors reject an older history. The existing native checks also pin the
tokenizer, original terminal reserve, publication witnesses and selected
held-out joint policy.

The original guarded profile refuses unexpected topology, predictor and
evaluation contracts. This entry point does not manufacture those more specific
recovery selections. It never installs an omitted guard during recovery.

An anchor demonstrates continuity with an independently retained exact prefix.
It neither authenticates the latest suffix nor protects an operator who replaces
both the journal and its independent anchor with an old matching pair. Recovery
does not provide current source evidence or a new policy approval.

## One replay and one original fence

`advance(expected_events, max_events)` executes the original reducer on the same
private reconstruction. The predecessor and positive bounded quantum retain
their original checks. `Ready` additionally requires every independent guard,
policy, credential and floor expectation to pass. An expectation failure is
latched and cannot become a successful Ready result on a later poll.

Neither Ready nor a progress query returns a live owner or privileged role.
Cancelling or dropping recovery leaves the canonical and pending files
unchanged. Event-call bounds are not CPU or elapsed-time bounds for an
individual numerical event.

`finish()` rechecks the complete requirements and delegates to the original
finalizer. That finalizer verifies canonical bytes, admits the original storage
operation and durably records its single recovery Fence before any live role
escapes. The wrapper then returns the original `FileOversightRoles` bundle.
A governor is present only when the original campaign guard requires one.

The fence withdraws old source/time eligibility, human approvals and campaign
approval keys. The original numerical work, confirmed stream prefix, retained
campaign usage and charged or unresolved effects survive. New publication
requires the normal fresh source, helper, human and final-publication checks;
recovery itself supplies no effect permit.

## Authenticated Stop remains a terminal path

The guarded cursor exposes the original Stop selection and terminal finalizers
only after the same complete requirements pass. Its authenticated recovery Stop
consumer retains the original terminal reserve and nonce-bound peer protocol.
It does not first issue a live owner or append an ordinary recovery Fence.
Unknown effects remain charged, and acknowledging Stop does not assert that
all in-flight effects have drained.

## Verification boundary

The implementation and regression sources received independent source review.
Initial compiler attempts were killed and installation exhausted the shared
toolchain filesystem. After workspace recovery and a separate disk-backed
installation, the pinned `nightly-2026-09-08` compiled the library and all seven
integration regressions passed. The scoped run used one compiler job, the
toolchain's Cranelift backend, no debug information and serial tests; it is not
the canonical qualification gate. No packet, Bead or deployment gate is closed
by this scoped result.

The focused target is:

```sh
cargo +nightly-2026-09-08 test --locked -p fa-reference --test governed_generated_recovery
```

The canonical repository gate remains
`cargo run --locked -p xtask -- check`; this change does not weaken it.
