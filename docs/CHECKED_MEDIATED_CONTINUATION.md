# Resume witness-checked publication under the original topology contract

## Capability and command

The topology-guarded `submit` command can now continue an existing publication
journal whose first image also enabled plain publication-witness validation:

```text
supervise_publication submit CONFIG SUBMIT_JSON \
  --guarded RECOVERY_PROFILE \
  --topology-profile TOPOLOGY_PROFILE \
  --publication-profile WITNESS_PROFILE
```

`--reviewer-profile REVIEWER_JSON` and the existing independent
`--credibility-activation EVIDENCE_FILE` option remain available. The new witness
option is a value-bearing pair; duplicate, missing, mixed or unknown options
are refused before configuration ingestion. No actor wire command changes.

Creation and continuation remain separate intents. The earlier checked
live-topology creation feature pins both guards in the first native canonical
image. This increment joins that domain to the existing **anchored mediated
submission** path; it does not reinterpret `serve-open` as a creation command or
synthesize missing guards on recovery.

## Independent bootstrap admission before replay

The public `FileMediatedRecovery::require_publication_profile` forwards the full
`FilePublicationRecoveryRequirements` to the original frozen guarded cursor.
This is the same decoder/checker already used by ordinary guarded recovery:
validation limits, feed/change/freshness contracts, snapshot fallback, joint
qualification, duplicate configurations and unsupported subtree routing are not
reimplemented or inferred from an actor request.

The method runs before the first replay attempt. Each `None` field requires
absence, not a journal-selected default. Matching settings do not bypass the
mediated opener's independent topology, policy, source, prediction/evaluation,
anchor, reserve or external-floor checks. A late or mismatching call drops the
cursor without a fence, staging cleanup, new role or logical-clock observation.
The same private canonical image remains bound through every finish route.

The operator currently selects only a **plain** publication-witness profile:
`PublicationProfile::simple_limits()` must succeed. Feed-backed, diagnosed-wait,
exact-current-snapshot and joint profiles are rejected instead of being reduced
to their numerical limits. Omission of `--publication-profile` explicitly
requires absence of all publication bootstrap fields. Consequently, a wrong or
missing selected profile refuses before replay and recovery mutation, rather
than spending a fence and only then discovering the mismatch. The command's
initial invocation clock still precedes historical admission; matching profile
configuration is not a fresh evidence observation.

Neither parsing this option nor checking the frozen bootstrap opens original or
current witness captures. The selected profile is retained for the eventual new
request, not used to reinterpret old decisions or receipts.

## Original recovery, source accounting and two-key publication

Recovery continues through the original fixed `Fence -> Time -> Sweep`
transaction. Outstanding unresolved effects still block new work and remain
charged. A successful recovery invalidates old approvals and topology coverage;
it does not certify that the topology files are current.

For a new request, the existing selection must independently provide a newer
graph and inventory generation with the same source/checker limits and retained
source expenditure. The original source custodian is rebound before fresh native
certification. Its lifetime checks, reserved read bytes, operation floor and
healthy checkpoint retention are not reset by selecting publication witnesses.
The original one-event replay checkpoints and Ready deadline remain in place.

The **same** `Continuation` and publication driver then consume the independently
selected `PublicationProfile`. There is no second execution or approval engine.
Initial topology certification, fresh policy evidence, actual helper review,
independent human approval, the final native topology refresh and final witness
validation all remain necessary. The original actor epoch, target, payload,
units and deadline are not rewritten to make a stale proposal acceptable.

Passing a topology check does not excuse a changed required witness or missing
capture. Passing witness validation does not excuse lost topology inventory.
An unrelated source change can remain permissible under the original structured
witness semantics. The native endpoint, stop/drain protocol and ledger determine
which attempts executed, did not execute or remain unknown; this orchestration
never manufactures a cancellation refund for an unknown effect.

## Historical retries and failure distinctions

A recorded request stays on the existing receipt/reconciliation route. It does
not read the next topology files, original/current witness captures, fresh policy
evidence or a credibility capsule, launch a helper, obtain another human key or
resubmit an effect. It still requires independently selected historical contracts
and an exact original request. Changed request bytes conflict even when all live
assets are absent.

Bootstrap mismatches and replay cancellation leave canonical and staged files
unchanged. Later errors can follow acknowledged recovery or rebind events; the
existing retention and error wrappers preserve that acknowledgment. This command
is not zero-write receipt inspection, and output/retention failure cannot undo
an acknowledged publication. No fallback to an unguarded owner is introduced.

## Change and validation record — 2026-10-17

This increment adds one library composition method, one explicit optional
publication selection to mediated submission, and eight regression functions.
Tests use original journal owners, native topology checking and witness
validation, actual helper subprocess plumbing and independent human sockets.
The helper ballots and capture contents are controlled fixtures, not evidence of
model accuracy, judgment independence or operating-system containment.

Coverage includes exact and late publication-profile selection; a second real
publication with mutations introduced only after human-approved dispatch;
unrelated versus required-witness changes; missing witness/topology/both inputs;
original receipt recovery and changed-byte conflicts without live assets;
wrong/missing bootstrap profiles without fencing or staging cleanup; rejection
of richer profiles and malformed flags; replay and Ready expiry; human rejection;
and the real top-level checked continuation command.

These tests are **authored and source-reviewed, not executed**. The editing
runtime has no RCH launcher or Rust toolchain. Dispatcher attempts stopped before
compilation with exit 127. A sparse patch-review tree is not a full build checkout;
file identity and patch application checks do not establish Rust syntax, type
correctness, formatting, Clippy, runtime behavior or release qualification.

The original source baseline for both modified files was checked by Git blob
identity against `454385b441bf0858a929a8aeef8b8b7f40009805` and remained unchanged
at `5e0dba55cd7d179f7a4441f619c3cf2a8d03795f`. The concurrent checked-creation and
process-backed learned-review changes are preserved. This work composes the
existing FA-014 recovery and FA-107 actor boundaries; it does not close either
broader packet. The unavailable `br` tool was not replaced by manual Beads edits.
No existing test assertion, dependency, journal tag or qualification gate changes.

Required qualification in a complete authorized checkout:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication workflow::continuation::guarded::mediated -- --test-threads=1
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib action::consequence::delivery::persistent::observed::guarded::recovery
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
