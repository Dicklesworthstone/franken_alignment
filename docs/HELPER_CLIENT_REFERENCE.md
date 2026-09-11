# Worker-side independent helper client

## Actual consumer and authority boundary

`oversight::helper_client::HelperClient` is the worker-side counterpart of the existing `helper_workers::io::HelperConnection`. It uses the SAME request parser and commitment/reveal encoders. This is reference integration progress for FA-018/FA-019/FA-107, serving plan 9.2, 9.3 and 17.1. It does not replace the congress, create a second protocol or grant the worker any controller handle.

A complete request must pass the existing bounded parser and match the worker's explicitly supplied `InputProfileBinding` before `input()` exposes it for inference. Profile ID, original profile bytes, model epoch, tokenizer epoch and policy epoch all compare exactly. The host is responsible for making that expected profile describe its actual loaded model. This check catches mismatched declared profiles; it does not inspect weights, authenticate a model, prove inference or establish helper independence.

Transport framing is not model input. The worker uses the original `WorkerInput::actual_input` bytes, ordered partitions and omission metadata. There is no default vote, canned inference callback, model fallback or automatic response on missing input. The client stops at NeedsInference until the worker supplies a verdict and salt.

`respond` constructs both original wire frames before changing state. It freezes one response, which cannot be replaced even before the first byte is sent. Only the commitment is sent first. The reveal is retained privately until the original supervisor sends its one-byte reveal request. Bad salt lengths leave the input uncommitted; an I/O failure after selection cannot trigger a fresh answer or resend an already written prefix. Debug formatting excludes input, profile bytes, verdict and salt.

## Bounded I/O and outcome meaning

Each step performs at most one read, or one write plus at most one flush. Reads are capped at 4,096 bytes; the existing 96 KiB total request-frame bound is checked from the nine-byte header before reserving body storage. Reads stop at the exact current frame boundary. Partial writes and transient Interrupted/WouldBlock outcomes preserve offsets; a blocked flush does not resend the preceding commitment. Terminal I/O/protocol errors latch and future steps do no I/O. ReplySent means only that the response was written/flushed locally, NOT that the coordinator accepted the reveal, completed a review or authorized an effect.

Generic Read/Write streams require the host's own bounded I/O contract. `from_unix` sets an explicitly supplied UnixStream nonblocking, and AsFd plus ClientInterest support an external native scheduler. There is no listener, worker process launcher, task runtime, accept loop, authentication service or infinite retry loop. The host bounds worker count, schedules steps and inference fairly, advances the ORIGINAL collector clock, retains the original authority, and authenticates each paired connection. A new connection is not a deadline extension or a license to reroll an outstanding job.

The existing FNV-1a-64 commitment is still explicitly non-cryptographic. This client adds no signatures, collision resistance, durable job/session identity, cross-process recovery or model-isolation guarantee. Publication remains gated by the original policy, whole-input congress, permits, optional human key and endpoint fences. A helper client contains none of those authorities.

## Verification and change record

The first increment adds six injected-I/O tests and a compile-fail authority-access example. Tests cover one-byte fragmentation, exact model-input delivery, no default verdict, no early reveal, immutable answer selection, blocked flush and interrupted reads without repeated commitment bytes, all five profile-binding components, malformed lengths, truncated requests, unexpected reveal signals and terminal partial-write failures. Error injection is not represented as a live-model or host-crash test.

All six tests and the doctest are UNEXECUTED source. Cargo/rustc and the required RCH runner are unavailable. Formatting, compilation, Clippy and the full revision-bound gate remain pending; no bead is closed or historical execution receipt reused. The concurrent helper worker/codec/server implementation is reused unchanged and is not counted as this increment's work. No dependency, unsafe code or secondary executor was added.
