# Helper byte transport

Consumer: the per-member workers of `HELPER_WORKERS_REFERENCE.md`, feeding the existing authority-bound congress. This extends the reference implementation of FA-018/FA-019/FA-107 (FI-A03/FI-A06/FI-A13/FI-A14/FI-A16); it does not activate a production helper profile.

## Exact input delivery

A request contains `FAHW1`, a big-endian u32 body length, the frozen reference round/root/member, the configured salt bound, and the original ActualHelperInput: profile identity and exact profile bytes, model/tokenizer/policy epochs, exact submitted bytes, every ordered part and its role/provenance, and closed-absence omission metadata. The decoder reconstructs that object through its existing checked constructor. No submitted bytes are reconstructed from metadata, hex-encoded into a model prompt, truncated, or silently replaced. Transport framing is not model input. A real worker must use the declared profile when interpreting the supplied ActualHelperInput, and its actual model identity and faithful use of that input remain host/provider obligations.

The one-member frame deliberately omits other members, weights, deliberations, action-private read witnesses and controller authority. The supervisor retains the complete EvidenceViewManifest and its authorization/transformation provenance; the transmitted object is the exact actual input, not a purported portable manifest or authenticated evidence certificate. The request header is checked before body allocation; the hard frame cap is 96 KiB, with the narrower original byte/profile/part/omission bounds checked independently on decode. Integers are fixed-width and big-endian. Unknown tags, extra fields/bytes, invalid spans, unsupported omissions, truncation and oversized declarations refuse.

A helper returns exactly `C` plus its eight-byte reference digest. Only after the supervising channel emits `R` may its reveal be accepted: `R`, a one-byte verdict tag, a two-byte salt length, and the salt. Tags are Allow=1, Hold=2, Deny=3 and Abstain=4. `WorkerInput` provides commitment/reveal encoders over the existing reference oracle. There is no actor-selectable member or round in a reply: the host has already assigned the channel. This remains the explicitly non-cryptographic FNV profile; no authentication or cryptographic binding claim follows.

## Bounded I/O and failure semantics

`HelperConnection<S>` owns one HelperPort and one Read/Write transport. Each `step` performs at most one bounded read, or one write of at most 16 KiB followed by a possible flush. There is no hidden read-to-EOF, write-all, thread, subprocess launcher or second executor. Partial input/output and an outstanding flush remain retained across Interrupted/WouldBlock. Until the entire request has been written and flushed, no commitment is admitted. Reveals are accepted only after the coordinator opens the original reveal phase and the channel writes/flushes its reveal request.

The generic adapter assumes its host supplies appropriately bounded or nonblocking I/O. Its step count alone is not a wall-clock timeout for an arbitrary blocking implementation. Supervisor observation time, not a worker timestamp or claimed send time, controls acceptance. The protocol establishes the order of accepted events, not a proof of when a remote worker first sent bytes or performed inference.

Malformed replies and fatal I/O latch a per-connection failure and leave that worker missing in the original reducer. A subsequent step cannot retry, reconnect, replace the worker, change its vote or manufacture an abstention. The transport reads only the declared bounded response; an advertised oversized salt is refused before its body is read or allocated. A previously accepted valid reveal is terminal and cannot be rewritten by later disconnection. Operator-facing failures contain typed protocol/I/O reasons rather than copied untrusted strings. There is no effect authority in the transport.

## Verification source

Source tests compare reconstructed inputs with the originals, use an independent manual all-part-kind/closed-absence frame, preserve full-width epochs, reject every truncated prefix, unsupported headers, oversized lengths, trailing fields and malformed member encoding. Foreign member/round commitments are processed before the deadline to isolate their binding failure from timeout behavior.

Scripted I/O tests exercise bytewise delivery, the complete-roster barrier, blocked flushes without request replay, malformed commitments beside a valid peer, truncated reveals, oversized salts without body consumption, interrupted writes and permanent broken pipes. These are deterministic fault fixtures, not claims of real provider isolation or empirical model safety.

September 11, 2026: code and tests added, with no new crate or dependency. Rust/Cargo/rustfmt/RCH/br remain unavailable in this editing environment. The tests have not been compiled or executed here and do not close their production beads. Historical execution evidence does not qualify these changes. Authenticated worker identities, production cryptography, process sandboxing, model execution/capture attestation and qualified runtime scheduling remain separate work.
