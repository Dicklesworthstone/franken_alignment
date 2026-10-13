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

## Runnable operator custody

The existing `shutdown_fleet create|recover|resume FLEET_PROFILE` command accepts
closed independent options with either original roster schema:

```sh
cargo run --locked -p fa-reference --example shutdown_fleet -- create /operator/fleet.json \
  --retain-coordinator-anchor /operator/custody/fleet-1.anchor
cargo run --locked -p fa-reference --example shutdown_fleet -- resume /operator/fleet.json \
  --coordinator-anchor /operator/custody/fleet-1.anchor \
  --retain-coordinator-anchor /operator/custody/fleet-2.anchor
```

Input custody is forbidden on create. Missing, duplicate, unknown and malformed
options refuse before manifest/authority intake. Selecting an input always invokes
native anchored open, with no revision-only fallback if it fails. Omitting both
options retains the previous contract. This is independent caller selection;
these flags are not inferred from or enforced by the untrusted coordinator file.

Input uses the original private regular-file and device/inode checks and charges
its actual bytes to the SAME aggregate input allowance as the manifest, member
configurations and native plan/per-member anchors. It must lie outside every
authority/coordinator directory. A failed import or native continuity check occurs
before any member command; native mismatch also precedes staging cleanup.

Retention admits only a new output file with a canonical existing parent outside
all authority stores. It repeats admission and exclusive creation after the pass,
writes with mode 0600, and syncs the file and its parent. An existing archive is
never overwritten. When a previous anchor was selected, checked advancement is
used again rather than exporting an arbitrary new owner. Output has the original
bounded anchor size, independent of the input-byte allowance. Operator-controlled
parent-directory identity remains an assumption, not an openat-style sandbox.

Native coordinator acknowledgment, archive retention, and stdout reporting are
three distinct boundaries. The command attempts retention before stdout. Output
failure cannot erase the retained copy; export failure cannot undo settled
domains or suppress the original per-member JSON. Simultaneous export, output and
incomplete-pass errors remain combined diagnostics. A caught output failure must
not trigger publication retries. Export failure can leave a partial/unconfirmed
file; preserve the old archive and choose a new destination for an explicit retry.
An unavailable coordinator after an ambiguous write exports no candidate anchor.

The original cumulative elapsed budget also checks custody acquisition and
pre-export work. Expiry after domain settlement reports that original pass plus
retention failure; it does not claim the earlier stop was undone. Storage calls
remain synchronous. Receipt-only resume still revalidates captured original
history and never acquires live helper or witness evidence.

Seven additional real-file/operator tests cover both roster formats and exact
resume, a valid same-counter fork, private/closed custody options, exact aggregate
input-budget neighbors, lost stdout after export, late output collision and
elapsed expiry after native acknowledgment. Prior fleet/operator assertions are
unchanged. The combined feature adds fifteen authored Rust test functions; none
has executed here. Targeted library/example tests and full xtask RCH dispatches
stop at missing rch, before Cargo. Exact-revision central verification is still
required; neither archive export nor a successful commit qualifies production.
