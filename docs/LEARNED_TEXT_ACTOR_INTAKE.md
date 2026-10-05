# Source-only learned-text actor intake

Status: unqualified source integration. The consumer is the existing durable
actor request book and its native/probe review-to-publication driver (plan
sections 8, 9, 10.15 and 17). This adds no journal tag, dependency or authority.

`into_learned_text_actor_gateway` retains the original weak actor gateway and
separate supervisor. `LearnedTextProposal` contains destination, policy epoch,
deadline and units, but no payload, source, observation or review. First intake
derives the entire completed original learned message under the same owner
borrow used by the original durable `submit_request`. Partial, paused, failed,
interrupted and pending-reset sources cannot supply a new message.

A request key names the FIRST message admitted under it, not a fresh source read
on retry. The original request spec supplies historical bytes for exact retries,
including recorded refusals. All caller-selected fields must still match. No
source resume, snapshot consumption, numerical work or new admission occurs on
retry. Fenced recovery returns only the original ticket projection; it restores
no automatic/human key. Changed fields conflict even when the source is gone.
The request book, limits, actor outcomes, cancellation and publication are the
original implementations. The new raw-text port rejects stream and nontext
profiles. Public byte-based actor APIs retain their existing source checks.

Nine regression functions exercise real ByteBpe/inference/learned-codec output,
exact retries, policy refusal, conflicts, incomplete generation, cancellation,
fenced reopening, live interruption and all five journal replacement barriers.
Two compile-fail examples prohibit caller payload and host access. Synthetic
weights are test controls, not learned-detector effectiveness evidence.

Targeted and full mandatory RCH commands were attempted and failed to launch:
`rch` is unavailable (exit 127). Compilation, tests, rustfmt and Clippy remain
unexecuted. Source/preimage hashes and whitespace checks do not qualify runtime
behavior. No existing test or gate is weakened and no bead is marked complete.
