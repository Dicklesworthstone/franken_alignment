# Kernel-authenticated durable actor ingress

The Linux `PeerSession` and `UnixPeerListener` are now generic over the existing
sealed `ActorRequestPort` interface. Their default remains the in-memory
`ActorPort`, so existing callers retain the same API. Durable `FileActorPort`
instances can now use the identical SO_PEERCRED admission boundary instead of
placing an unauthenticated Unix connection directly in front of the file-backed
request journal.

Authentication occurs before the actor wire is moved into a connection and
before any frame is read. The frozen `PeerPolicy` requires exact UID and effective
GID, with an optional PID. A rejected candidate does not consume a request ID,
one-use supervisor snapshot, ticket slot, connection count, or actor response.
There is no wildcard/root exception and no in-band identity field.

Reconnects authenticate again while retaining the same `ActorWire` ticket and
idempotency session. An exact durable submit retry can therefore recover ticket
visibility after a lost response without installing another admission snapshot
or creating another journal request. Disconnect and ingress revocation remain
transport operations only: neither proves nonexecution, refunds an effect,
revokes a permit, nor changes the control ledger.

The underlying `UnixActorConnection` was already generic, so this increment does
not add another codec, transport, executor, buffer, or effect path. It also does
not change actor-visible responses. Durable file I/O can still occur synchronously
during a valid exchange; socket `DriveBudget` limits socket work rather than disk
latency.

Regression source covers a kernel-authenticated durable submit followed by the
original review/authorization/publication path, wrong-PID refusal before snapshot
consumption, and authenticated reconnection with exact retry and no second durable
admission. These scenarios use Unix socket pairs and real SO_PEERCRED observations.

SO_PEERCRED identifies the peer recorded by the Linux kernel for the connected
socket. It does not authenticate executable bytes, a human, a container workload,
or the current holder after a file descriptor transfer. UID/GID-only policies
admit every connecting process with those credentials. The source-aware cold
intake hook still needs its own integration through this authenticated transport;
this commit establishes the generic credential gate it can reuse.

The required RCH runner is unavailable in this environment. Rust compilation,
formatting, Clippy, tests and doctests remain unexecuted, and no Beads or
production-gate status is changed.
