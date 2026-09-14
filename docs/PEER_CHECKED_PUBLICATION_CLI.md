# Peer-checked publication and reviewer-only configuration

## Executable integration

The existing `supervise_publication` command now supports Linux peer verification on both sides of the independent review connection:

```text
supervise_publication create CONFIG_JSON SUBMIT_JSON --reviewer-profile REVIEWER_JSON
supervise_publication review-peer REVIEWER_JSON REQUEST_ID
```

The first command still uses the original durable actor/source/helper/human/publication pipeline. The second needs ONLY the reviewer profile, not CONFIG_JSON. In particular, the reviewer does not need access to helper executable paths, environment credentials, the source file or the protected journal. The original `review CONFIG_JSON REQUEST_ID` and unflagged `create` remain explicit legacy namespace-isolated paths; they report that peer credentials are unchecked. An invalid requested peer profile NEVER selects those compatibility paths.

The peer profile is independently provisioned operator data. It is not downloaded from the peer, decoded from the offer or chosen by an actor. Its logical reviewer ID, complete scope and clock domain must match the supervisor's configuration before creating a store, reading an observation or launching a helper. `review-peer` parses no supervisor configuration. Both clients still require an interactive terminal and an exact, explicit original nonce-bound decision; no unattended approval flag exists.

`crates/fa-reference/fixtures/reviewer_peer_profile.json` shows the complete schema. Its numeric accounts and directory are illustrative operator choices, not detected account identities. All fields are mandatory. Missing PID is rejected; explicit `null` selects UID/GID-only matching, while a positive PID narrows it. The original PeerPolicy validates UID/GID/PID values. Duplicate/unknown fields, zero audience identifiers, invalid timing, unsupported clocks, invalid paths and excessive candidate budgets refuse. Peer-profile files have a 16 KiB bound, with the existing strict JSON parser enforcing depth/item/string bounds. Non-Linux platforms reject requested peer mode rather than silently ignoring it.

## Separate-account socket access

Provision a dedicated existing socket directory owned by the supervisor UID and its declared GID, with no group or other write permission. Mode 0710 or 0750 can allow a reviewer in the selected group to traverse it while preventing that group from replacing names. The command creates only its own `review-REQUEST_ID.sock` and sets that socket to 0660. It does not alter the ledger directory's 0700 permissions, make the supervisor configuration group-readable, create accounts, change group membership or install ACLs.

The supervisor and reviewer may have different UIDs. The expected reviewer GID is its connection-time effective GID; it need not be its supplementary group that grants filesystem traversal. Configure those deliberately. The socket itself must have the supervisor UID and declared GID. The directory and all relevant ancestors must remain under trusted control. Existing names are never unlinked to force a bind; cleanup checks the device/inode of the socket this invocation created.

A shared group grants filesystem access to attempt a connection, not reviewer authority. The Linux kernel credentials must independently match the configured reviewer rule. The reviewer verifies the supervisor rule on its connected socket BEFORE reading the offered packet. Only after a candidate passes does the supervisor reread evidence and create the original human request. Rejected candidates receive no evidence or receipt and cannot consume that request.

## Refusal, deadlines and conserved authority

The host uses the original action/workflow deadline while trying candidates. Its bounded admission gate accepts one peer and counts failed candidates. A denied process does not displace a later matching reviewer. Reaching the quota fails the workflow through its existing stop/drain/child-cleanup path, not a less restrictive peer policy. This trades availability under a connection flood for refusal rather than consent.

After authentication, the same exact packet, explicit decision, durable approval and independent second key apply. The peer check cannot refresh a review, widen an action deadline, bypass source changes, release charged effects or regenerate an approval key. A committed approval whose receipt is lost retains the original outcome/unknown-delivery distinction. Query-only `resume` is unchanged and accepts no peer-profile flag because it does not contact a reviewer or create new authority.

The opaque `VerifiedReviewerSocket` from FILE_REVIEWER_PEERS.md is handed directly to the existing reviewer client or connection. It is not unwrapped and reconnected. There is no second voting protocol, cryptographic scheme, authority ledger or inference engine.

## Limits of the identity claim

This authenticates Linux connection-time UID/GID and optional PID, not a human, executable image, device, cryptographic identity or per-message sender. A same-account actor matches a UID/GID-only rule; use separate credentials or an independently pinned PID to distinguish it. Descriptor transfer, PID reuse, compromised permitted processes, same-UID namespace writers and hostile privileged hosts remain outside that claim. PID values must be supplied independently, not trusted because a connecting process announced one.

The operator profiles are not signed or persisted as new authority configuration by this increment. Their authenticity and distribution remain operator responsibilities. The already-existing durable reviewer ID/scope and all human approval records remain bound by the original journal. No peer metadata is serialized as an effect permit. No production qualification, cryptographic authenticity, wall-clock bound or full OS sandbox follows.

## Source tests and verification

Five additional example test functions plus one inert subprocess fixture cover checked publication after rejecting a same-account wrong PID, explicit checked rejection, candidate exhaustion with no human request created, preservation of private journal permissions, audience/directory preflight before the clock or any store, strict parsing and unsupported flags, and fake-supervisor rejection by the reviewer-only configuration. They reuse the original executable helper and escaped terminal-decision fixtures. Existing example test bodies and their assertions are preserved.

Together with the first library increment, this batch adds thirteen Rust test functions, including two subprocess-entry fixtures, and one compile-fail example. The choices in the fixtures are synthetic and do not establish human attention or judgment quality. Separate-account permission support is implemented; no different-UID deployment was executed during editing.

The combined invocation was attempted:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_reviewer_peer --example supervise_publication
```

It failed before compilation with `rch: command not found`, exit 127. The new code, Rust tests, subprocess tests, formatting, Clippy and full revision-bound gate remain uncompiled/unexecuted here. No Beads item was closed. No dependency, existing semantic reducer or prior verification artifact was changed.
