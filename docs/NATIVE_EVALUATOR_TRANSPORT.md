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

## Audience-bound requests and native result receipts

Both peers now exchange versioned envelopes. Upgrade the client and server
together: the earlier bare-assessment input and bare-result output are rejected,
not retried through an unbound fallback. Local `native_andon assess` file input
keeps its existing schema and behavior. No journal encoding changes.

The client wraps the original, locally validated assessment bytes as lowercase
`assessment_hex` in `fa.native-evaluator-request/1`, alongside `audience` containing
tenant, principal, run, branch, authority, deployment and evaluator IDs from its
independent peer profile. The server checks that complete audience before any
native recovery, fence or label operation. It then decodes the EXACT original
assessment through the existing mutation decoder. Request frames are bounded to
20 KiB; underlying assessments retain their original 8 KiB ceiling.

`fa.native-evaluator-receipt/1` contains the exact request, `native_completed`, and
`native_result`: the original native mutation report, or null when no complete
report is available. The client checks the request, closed schemas, operation,
status, acknowledgment revisions, canonical counters, window identities and
anchor-retention flag before printing response bytes. Raw text, a legacy report,
a different valid assessment, an unknown field or a contradictory success fails.
The receipt ceiling remains 64 KiB; oversized or partial output never succeeds.

Only completed `applied` and `already_applied` outcomes return client success.
A refusal stays a refusal. Missing result data stays unconfirmed. A recorded
assessment whose invocation did not complete (for example, anchor retention
failed) stays recorded-but-incomplete, not refunded or renamed as refused. Valid
negative/incomplete receipts are rendered for diagnosis with a nonzero result;
malformed or mismatched replies are not printed. A lost response may follow a
committed label: inspect original history before any explicit retry.

These are correlated reports from the kernel-verified local supervisor, NOT
cryptographic journal proofs or signed evaluator attestations. The client does
not independently verify the server's storage or anchor export. Its stdout can
include its own encoded assessment and must retain evaluator-custodian access.
The supervisor's separate summary contains only completion/delivery flags and a
candidate count; it does not disclose assessment contents or private paths.

Seven protocol regressions exercise permitted applied/retry receipts, every
audience dimension, exact assessment mismatch, refused/unknown/incomplete states,
malformed/extra/duplicate fields, counter/operation drift and byte ceilings.
Three integration regressions use actual source capture, applied congress review,
original file recovery and real kernel-verified sockets: benign assessment with
exact retry and conflicting-label refusal, wrong-domain admission before fencing,
and lost reply with native charge/anchor retention intact. They are authored,
not executed. Remote-only targeted tests and unchanged xtask still cannot start
because rch is absent. No qualification or production-containment claim follows.
