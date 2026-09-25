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

## Runnable service

```sh
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  WITNESS_PROFILE --native-text RECIPE > native-submit.json
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE \
  WITNESS_PROFILE --native-text RECIPE > recovered-submit.json
```

The native recipe and actor/reviewer commands are unchanged. The selected
witness profile is the existing `fa.supervised-witnesses/1` through `/5` or
`fa.supervised-whole-input/1` or `/2` contract. Capture, producer, finite waiting
and exact-snapshot policies retain their original semantics. A producer's scope
must match the native owner. Joint held-out profiles are explicitly refused:
this composition cannot discard their additional qualification requirements.
There is no checked-to-unchecked fallback and no implicit witness profile.

Before inference, creation installs the selected publication contract in the
same image as the native model/tokenizer. After the authenticated actor submits
its source reference, the existing driver captures original witness evidence
before launching helpers and obtains the separate human decision. It reacquires
current witness/producer/feed evidence at the original authorization, dispatch
and first-publication boundaries. Late phantom insertion can block publication
even after dispatch; a disjoint change still undergoes exact revalidation. A
quiet model and human approval are not substitutes for valid current evidence.

Checked recovery pins the same witness/bootstrap policy before the original
fence. A pending generation follows the unchanged explicit native resume loop;
its first effect still needs fresh producer evidence and both keys. A recorded
source-linked request instead follows the unchanged receipt-only path: no
producer/capture/evidence read, new inference, helper, review listener or resend.
Its original target, epoch and expired deadline remain in the reply document.

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

The service integration adds five regression functions using an original
`FilePublicationProducer`, actual monitored numerical execution, authenticated
actor sockets and synthetic process helpers with independent human decisions.
They cover new creation, one-token and completed-generation recovery, final-cut
phantom versus disjoint changes under both feed-history policies, receipt-only
recovery after producer/evidence removal, missing original producers, and strict
argument/scope preflight. The earlier native tests, inference loop, source/time
refresh, socket intake, authority and receipt reducers are unchanged. These five
service tests are also UNEXECUTED; a fresh targeted RCH test invocation failed
before compilation because `rch` is absent (exit 127).
