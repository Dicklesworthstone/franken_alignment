# Checked native-text publication

## Atomic native and witness contracts

`FileOversight::create_generated_text_stream_checked` composes the existing
native-text-only stream with the existing final-publication witness gate. The
first canonical image contains the stream contract, optional logical recovery
reserve, exact validation limits, optional change-feed/freshness/snapshot-fallback
policies, and exact model/monitor/sampler and tokenizer configuration. No token
runs and no successfully initialized image omits a selected guard.

The public `GeneratedPublicationProfile` and `GeneratedPublicationFeed` types
live in `observed::stream::generated::checked`. They contain operator-selected
bootstrap contracts, not source captures, approvals or live authority. Structured
exact/absent/range witnesses and opaque whole-input evidence continue to use the
original publication APIs; this module contains no new validation engine.

`open_generated_text_stream_checked` reads one locked canonical image and pins
all selected contracts before numerical replay, storage confirmation, staging
cleanup or the original recovery fence. An ordinary stream, missing witness
profile, changed limits, changed feed identity/cut/lookup budget, changed producer
lease, removed fallback, extra routing mode or duplicate bootstrap event refuses.
Presence and absence of a reserve/feed are distinct. Exact model and tokenizer
bytes are checked by the existing text replay validator against the same image.
Recovery cannot upgrade an unchecked stream or silently weaken a checked stream.

A successful open returns the original fenced, paused numerical owner. It does
not restore a fresh clock, evidence lease, human approval, sendable key or output
permission. Acknowledged inference resumes only through the existing fresh-source,
fresh-clock, explicit-resume APIs. First publication still requires native source
admission, the complete helper input/review, the separate human key, and fresh
validation at the original publication boundary. Receipts remain historical.

This closes a composition gap in the bounded reference implementation of plan
sections 8.7, 16.4 and FA-062 final-publication validation; it does not close the
full roadmap packet or assert a statistical alignment result. Additional joint
or anchored governance roles require their own composed interfaces. The local
journal-as-publication sink is not remote delivery, independent latest-head
attestation, anti-rollback protection or hostile-process isolation.

## Change and execution record — 2026-09-25

Added the two checked constructors and six regression functions. Tests cover all
three supported witness profiles, exact bootstrap substitutions with unchanged
canonical bytes on refusal, paused recovery and continuation against an
uninterrupted numerical control, rejection of valid ordinary/unchecked native
streams, invalid configuration before store creation, duplicate bootstrap events
and unselected routing. Fixtures use real canonical files and original numerical
execution with synthetic weights; they do not certify trained monitor quality.
The original generated-message reducer is unchanged except its new module export.

**UNEXECUTED:** a fresh `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p
xtask -- check` attempt failed before compilation (`rch` not found, exit 127).
Rust compilation, these tests, rustfmt, Clippy and the full repository gate have
not run. Local preimage/blob-hash and whitespace checks are not runtime evidence.
No Beads closure, dependency, wire encoding or journal encoding change is made.
