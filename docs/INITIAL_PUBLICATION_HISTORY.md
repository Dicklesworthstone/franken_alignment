# First-publication history custody

The ordinary Unix publication example can now retain the first independent
history anchor directly from creation:

```text
supervise_publication create CONFIG SUBMIT_JSON --retain-history-anchor OUTPUT
supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE --retain-history-anchor OUTPUT
```

Both accept the existing optional `--reviewer-profile REVIEWER_JSON`, in either
order after the positional inputs. With this explicit output flag the option
set is closed: recovery profiles, input anchors, continuation modes and credibility
activation are not initial-creation instructions. Unflagged commands and learned
creation keep their existing behavior. This is unqualified reference source.

## Start the same original owner

The output path is admitted before store creation, source acquisition or worker
launch. It must be absolute, new, and outside the authority directory. Existing
files, directories, final symlinks (including dangling links) and unavailable
parents are refused. The store's existing parent is resolved without creating
it, so an invalid output cannot leave behind an empty authority store. Canonical
parent aliases are checked. The original create-only API still rejects an
existing store: this command is never a recovery fallback or budget reset.

The implementation uses the original checked or ordinary native constructor,
then installs the same terminal reserve and source policy in the original order.
There is no recovery fence before the first request; its original epoch is not
rewritten. The shared continuation body obtains fresh source evidence, runs the
original helper congress, obtains a fresh human key and publishes through the
original endpoint. Checked creation still requires its original witness recipe,
original capture and current evidence. An inserted key invalidates an earlier
absence witness rather than being accepted because history export was requested.

One start Instant and original logical deadline cover output admission, creation
and the shared continuation. Synchronous input, filesystem and reducer work can
exceed an elapsed budget; no hard interruption or real-time bound is claimed.
Configuration and document file parsing precede that workflow budget.

## Retain only the acknowledged cut

On ordinary returned completion or error paths, the current healthy owner
supplies `history_anchor()` before helper cleanup and actor-response rendering.
No later disk read or reopened owner supplies the archive. The same bounded
create-new mode-0600 writer used by guarded recovery writes the anchor and
synchronizes the file and its parent directory. A late path collision cannot
replace another custodian's archive. A poisoned owner cannot export its old
memory as an acknowledged cut after ambiguous storage failure.

The retained cut need not be a successful publication. A healthy bootstrap or
source-withdrawal prefix can be exported after a failed first evidence read;
a denied or stopped request can also be retained. The original actor result is
not replaced with success. Retention failure is combined with workflow and
write/flush failures, and the original actor response is still attempted when
one exists. Source failures before actor forwarding leave the request unadmitted.
No failure initiates an effect resend or silently retries creation.

Capture/export is separate from authority persistence. A crash or unwind before
finalization may leave no archive; a failed export may leave a partial file that
is not acknowledged custody. Do not infer export success from pathname existence
or from an executed actor JSON response alone. Do not delete or overwrite an
existing output merely to make creation retry succeed. Recover the original
store rather than creating a replacement; independent recovery prerequisites
still apply. Output is private operator history, not actor data or a permit.

## Use the first anchor

After successful retention, OUTPUT is suitable as the independently selected
`history_anchor` in the existing guarded submit, receipt-recovery or stop profile.
Those profiles still require their own explicit numeric floors and exact original
CONFIG; the creation command does not guess missing external requirements. Each
successor export requires a different new pathname, preserving the old archive.
See [guarded continuation](GUARDED_NEW_PUBLICATION.md),
[receipt recovery](GUARDED_RECEIPT_RECOVERY.md), and
[guarded stopping](GUARDED_STOP_OPERATOR.md).

The current ordinary guarded operator profiles do not support checked witness
histories. A checked initial archive preserves those gates and must use its
matching recovery contract; export never downgrades it into the ordinary profile.
Trusted storage, cooperating writer locks, protected configuration, independently
retained archives, helper provenance and reviewer authentication remain host
assumptions. Joint rollback of both journal and archive is not prevented here.

## Regression sources and qualification

Eleven new regression functions use the existing helper-process and reviewer-
socket fixtures, actual actor requests, original journal and anchor codecs, and
native witness capture. They cover creation followed by fresh guarded publication
and offline receipt retry; source failure plus corrected work; custody refusal;
malformed input and expired deadlines; late collisions and create-only retries;
simultaneous output/export failure; closed command grammar; unmodified stale
actor epochs; checked witness success versus a phantom insertion; poisoned-owner
refusal paired with healthy export; and actual create/resume command dispatch.
Existing continuation and qualification tests are unchanged.

These tests are authored but unexecuted. The required targeted Cargo tests and
unchanged xtask gate are attempted via `RCH_REQUIRE_REMOTE=1 rch exec`; the absent
launcher stops execution before Cargo. No compilation, rustfmt, Clippy, runtime,
release, full FA-014 or production-activation qualification is claimed. This
feature supplies the previously missing first archive for actual operator
continuation; it changes no dependency, runtime, journal format or actor verb.
