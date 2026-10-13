# Independently retained fleet coordinator history

FA-014, plan 16.4/16.5: a numeric coordinator revision and the original fleet
plan cannot distinguish all valid later-history substitutions. A coordinator
image contains later visit identities/outcomes and newer whole-domain heads.
An independent copy of those facts can now be required before recovery.

`FileShutdownCoordinator::history_anchor` captures only a healthy acknowledged
owner. An entered live operation or ambiguous write cannot export an older RAM
image as a newly acknowledged anchor. `history_anchor_after(previous)` checks
continuity before advancing custody. Both return data, perform no file writes,
and acquire no domain owner, helper, reviewer, clock or effect rights.

`open_anchored(directory, plan, max_bytes, minimum_revision, anchor)` uses the
original exclusive coordinator lock and ordinary decoding path. The SAME
independently selected plan, path, byte ceiling and minimum revision must pass.
The supplied anchor is checked before staged-file cleanup or returning the owner.
Reopened successful visits remain historical; current session observations start
unavailable exactly as in ordinary open. No domain is opened during admission.

Coordinator images are not append-only byte strings: pending visits acquire
results in place, and each domain's retained head grows. The continuity check
therefore requires the complete previous visit sequence with unchanged domain
and kind (including its clock), preserves every finalized result exactly, permits
Entered results to gain a completion, and requires every new domain head to
extend its previously retained exact original journal prefix. A same-revision
image must be byte-identical. A higher counter or extra visits cannot excuse an
earlier finalized rewrite, dropped visit, domain rollback or equal-counter fork.
The original codec still checks visit/revision accounting and all head bounds.
This is continuity of retained facts, not reconstruction of every intermediate
coordinator write or proof that the unanchored suffix is authentic.

The private `FASCHAN1` transport is bounded by the original 16 MiB coordinator
ceiling plus 20 framing bytes. `from_trusted_bytes` validates closed framing;
complete original configuration and image validation occurs when used. No native
coordinator or domain format changes. Clone shares bytes and Debug omits private
history. The archive contains payloads, helper evidence and refusal diagnostics:
its custodian must have access to the complete original history.

Keep the previous archive until separate durable retention of the successor is
acknowledged. Ordinary open intentionally remains available with its weaker
revision-only contract; selection of the stronger API must be independent of the
untrusted image. A jointly rolled-back coordinator and all retained archives
still defeats this protection. Authentication, filesystem isolation, actual fleet
completeness and external custody remain operator assumptions. Successful open
is not a current stop observation or evidence of nonexecution.

All decoding/prefix checks are bounded by the existing input, domain, visit and
journal limits, but synchronous and additional to later semantic replay. This
is not a constant-memory or latency claim. No new native authority interaction,
event, effect retry or background process is introduced.

Eight regression functions use actual original stores and two-key effects:
genuine successors and exact visit-ceiling receipt refresh for reserved, sent,
executed and expired obligations; pending resolution versus changed finalized
intent; valid equal-counter domain forks; rewritten refusals hidden by later
visits; independent plan/path/capacity/revision pins; every coordinator write
barrier; closed archive framing and redacted Debug; and attempted custody advance
from an older healthy owner. Existing tests are unchanged. Rust compilation,
tests, formatting and Clippy remain UNEXECUTED until the exact-revision RCH gate.
The editing environment lacks rch/cargo/rustc/rustfmt; missing tools are not a
pass. No production activation or FA-014 closure is claimed.
