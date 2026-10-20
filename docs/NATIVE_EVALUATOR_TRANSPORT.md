# Native evaluator peer transport

Source implementation; execution qualification is pending. This continues
NATIVE_ANDON_OPERATOR.md and plan sections 9.9, 16-18. The actual consumers are
`native_andon serve-assess` and `send-assess`; no new evaluator or journal exists.

## One invocation deadline and bounded peer work

After original kernel credential verification, each socket becomes nonblocking.
Every read/write syscall is surrounded by checks of the same invocation deadline,
including successful progress and the final EOF/write. A stalled peer, trickled
input or blocked reply cannot renew the budget by restarting a syscall timeout.
Reads retain at most the declared byte ceiling and examine one extra byte only to
reject oversized input. EOF still delimits exactly one document; partial input is
never passed to the original assessment decoder or native journal.

The server captures the original mutation report in a bounded buffer. Native
journal acknowledgment and independent anchor retention finish before peer output
begins. A reply failure cannot hold the native owner, undo its acknowledged state,
refund a charge, or trigger another assessment. The supervisor report distinguishes
`native_completed` from `reply_written`; a failed invocation is not automatically
a refused label. A socket write is not proof the peer received or accepted it.

Client timing begins before profile/assessment ingestion. Configuration reads,
standard-library connect, native replay/synchronization, and stdout remain
synchronous. Checkpoints are cooperative, not preemptive wall-time guarantees.
No new runtime or dependency is added. Namespace control and process identity at
connection creation remain original assumptions; transferred descriptors do not
become authenticated executable or institutional identities.

Socket cleanup checks the saved filesystem device/inode and socket type, leaving
a replacement at the original path untouched. This relies on the protected parent
namespace, not an atomic unlink defense against a privileged concurrent attacker.

## Regression coverage and evidence

Five transport tests cover EOF/size boundaries, stalls and trickles, expiry after
progress/EOF, write backpressure with a permitted control, and pre-I/O expiry plus
bounded native report capture. A sixth regression checks replacement-path cleanup.
Existing peer credential checks remain, with their fixture paths corrected and
the document-read test moved onto the same bounded I/O interface.

Required remote-only example tests and the unchanged xtask gate could not start:
`rch` is absent (exit 127 before Cargo). These tests are authored, not executed.
Source hashes, indexed patches and whitespace checks do not prove compilation,
Rust behavior, population calibration or deployment safety. No bead is closed.
