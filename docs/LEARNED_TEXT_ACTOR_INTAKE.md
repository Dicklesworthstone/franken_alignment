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

## Original cumulative streams, not actor-authored frames

`into_learned_text_stream_actor_gateway` exposes a separate stream-only actor
port. Its descriptor is just request identity, `Message` or `Finish`, and a
deadline. The original `learned_stream_spec` builder derives all output, previous
message boundaries, destination version, epoch and cumulative-frame charge.
There is no actor byte, chunk, target, prior-history or cheaper-cost override.

The existing original stream law chooses which release is available: the whole
completed generated message first, and a separately reviewed finish only after
its original receipt confirms the append. Dispatch and visible-but-unreconciled
publication cannot unlock finish. Each release keeps its own original request,
review, automatic key, independent human key, and confirmation. Historical keys
match the original frame kind and deadline, not today's source or audience; an
old message retry cannot become a finish or append the output twice.

Raw and stream ports share private same-borrow intake, preserving one-shot
snapshots and original refusal/limit behavior. Retained request access uses the
existing `RequestBook::original_spec`, including refused admissions, rather
than introducing another identity table or duplicate getter. This is an
in-process actor surface, not a new wire protocol, registry verb or transport.

Eight additional regression functions exercise frame derivation and full cost,
kind/deadline conflicts, cancellation and paused recovery, distinct bootstrap
profiles, all five stream-intake storage barriers, original native helper
allow/deny controls, and complete generated message then finish publication.
The native composition runs the existing tokenizer/model/probe evaluators;
there is no supplied verdict. It checks the original actor ticket, a separate
human key for finish, immutable historical retries, confirmed versus published
audience state and query-only recovery of an unknown append without source
resumption or refund. These source tests do not qualify runtime behavior.

The final targeted/full RCH attempts still cannot launch because `rch` is
absent (exit 127). Source checks do not replace compilation or execution.
No source recipe, frame encoding, policy reducer or existing test is weakened.
