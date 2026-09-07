# Integration contracts

These contracts are prospective. No production host or effect adapter ships in this draft.

## 1. Capability negotiation before work

A host identifies its adapter revision, effect families, mediation mode, identity/fencing mechanism, bounds, observation channels, supported model spaces, replay grades, and failure semantics. The consumer asks for a required contract; the adapter either supplies it or returns a typed unsupported result. Negotiation never silently upgrades cooperative interception into containment.

The draft [capability schema](../schemas/capability_manifest.schema.json) and [example](../examples/capability_manifest.json) illustrate the shape. They are not a report of capabilities currently implemented by this repository.

## 2. Black-box tool integration

Start with an existing tool dispatcher. Inventory all real side-effect routes, including direct network, files, subprocesses, messages, downloads, shared queues, and user-visible streams. Put effective credentials behind a broker and remove bypass routes from the actor's environment. Register the exact scope of each tool; do not derive it from a natural-language description supplied by an untrusted server.

Freeze the tool's final arguments, payload/artifact bytes, target binding, principal, purpose, and policy epoch. The gate evaluates the prepared effect. The broker consumes the one-use permit and invokes the registered adapter. A result is confirmed, confirmed-not-executed, or unknown according to the adapter's actual external semantics. An SDK return value alone is not a remote transaction proof.

**Acceptance tests:** direct actor bypass attempts; altered payload after review; redirected destination; symlink/descriptor races; stale epoch; expiry; duplicate permit; cancellation before and after dispatch; remote success followed by local failure; status/idempotency expiry; and attempted resource-limit overrun.

A deployment that cannot remove direct routes remains `COOPERATIVE_GATE` or `OBSERVE_ONLY` for those routes. It may still be useful, but its prevention claims are narrower.

## 3. White-box serving integration

The serving adapter names architecture, weights/adapters, tokenizer/template, numeric and kernel profile, tap semantics, cache layout, and output ownership. It retrieves **real** tensors through a tested interface. It does not fabricate missing activations, use random placeholders, or infer GQA cache shapes from query-head counts.

Capture separates prefill from decode and records actual positions, masks, batch membership, dropped frames, device transfers, and generation changes. Bounded rings and zero-copy views have explicit lifetimes; a deferred worker must not read a recycled serving buffer. Compression occurs on owned or properly leased data.

**Acceptance tests:** compare capture to a direct host oracle; multiple batch sizes and sequence lengths; packed requests; GQA/MQA; cancellation; cache growth; device/precision changes; reused buffers; missing taps; model hot-swap; backpressure; and hostile tensor metadata.

Activation capture and restart are negotiated independently. A host can support useful audit traces while returning `unavailable` for restart.

## 4. Checkpoint and replay integration

A profile lists complete mutable state, external environment basis, and the advertised replay grade. Begin with original-token recomputation and lossless state capture. Test actual resumed generation, not only tensor decode. Record RNG/sampler state and nondeterministic backend limits.

Experimental branches have no production credentials. Tool simulations are explicit adapters, not ordinary tools with a “simulation” prompt. Promoting a branch creates a new action proposal under current authority. Physical state, messages already delivered, and prior network effects do not rewind.

**Acceptance tests:** omitted RNG, wrong tokenizer, wrong model adapter, cache-head mismatch, positional mismatch, lossy state presented as exact, missing evidence object, expired key, partially captured environment, stale checkpoint, external side effects during replay, and branch budget duplication.

## 5. Franken storage and runtime integration

Use reviewed, feature-pinned public interfaces. Respect task-owned non-`Send` database connections; do not invent cross-thread safety through a wrapper. Translate logical control commits into actual storage barriers and test crashes at each one. A bulk trace queue must not starve the exact control ledger or credential revocation.

Search and graph projections expose their committed frontier and generation. Their results reference original evidence and are authorization-filtered. They may guide investigation; they do not decide exact quota balances. Rendering uses authorized host-supplied assets without untrusted network/file inclusion.

## 6. Profile manifest minimum

| Area | Required fields |
|---|---|
| Identity | Adapter ID/revision, supported host/model identities, dependency/feature closure |
| Scope | Effect families, exact target/payload binding, ownership and purpose boundaries |
| Authority | Mediation mode, broker identity, credential/egress inventory, revocation semantics |
| Lifecycle | Prepare/authorize/dispatch/outcome transitions, idempotency, unknown behavior |
| Resources | Byte/operation/time/memory bounds, enforceable maximum effect, queue admission |
| Observation | Channels, coverage, ordering, transformations, gaps and freshness |
| Replay | Grade, required state, environmental closure, deterministic profile |
| Evidence | Test artifacts, negative cases, revision, unsupported combinations, expiry |

## 7. Migration without false confidence

Deploy observation first to inventory behavior, but evaluate attacks before interpreting benign traces as an allowlist. Then broker one effect family and verify containment. Add helpers in shadow, compare against exact/text baselines, and only activate a review requirement after its outage and attack paths are tested. Add white-box capture and learned codecs independently.

Rollback restores an earlier tested profile under a new explicit activation event. It does not restore old authority epochs or make consumed permits reusable. The operating status must say which features are active and which are merely installed.
