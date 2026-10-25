# Rotate and revoke learned publication credentials

The learned operator exposes the original durable credential lifecycle through
an offline command. Rotation advances the native credential generation by one;
revocation permanently closes that guard to live publication. Both use the
original learned replay and lifecycle reducers. They preserve already retained
publication receipts and unresolved effect liabilities.

## Commands and independent custody

Select the same complete credential-wrapped learned recipe and operator config
used by the original owner. An independently retained input history anchor and
a new output archive outside the authority store are mandatory:

```sh
supervise_publication create-learned-generated CONFIG CREDENTIAL_RECIPE \
  --rotate-credential 501 1 2 \
  --history-anchor /operator/history-before.anchor \
  --retain-history-anchor /operator/history-rotated.anchor

supervise_publication create-learned-generated CONFIG CREDENTIAL_RECIPE \
  --revoke-credential 502 2 \
  --history-anchor /operator/history-rotated.anchor \
  --retain-history-anchor /operator/history-revoked.anchor
```

The positional values are an operation ID, its original expected credential
generation, and, for rotation, the exact next generation. They must be nonzero
unsigned decimal integers. Rotation cannot skip a generation or overflow it.
Operation identity belongs to the original bounded native lifecycle ledger;
reusing an ID for changed arguments or the other operation refuses.

The authority store must have no live owner. The command uses the original
exclusive owner lock and never asks an active actor service to rotate itself.
Do not combine it with actor, reviewer, continuation, receipt, activation or
Stop options. Such mixed commands refuse before reading configuration or
replaying history.

## Current selection and historical request are different

The credential profile's `epoch` independently selects the **current** expected
generation and revoked state. The immutable policy must still match the original
recipe and complete guarded history. The command validates that selection and
the lifecycle operation before the ordinary recovery Fence or staging cleanup.

After the first example acknowledges rotation, provision the replacement broker
and provider materials independently and update the selected profile's epoch to
`{"generation":2,"revoked":false}` before starting another live session. The
next message still needs its own original reviews and approval. This command
changes the native guard epoch; it does not provision a remote provider, create
new secret bytes or rewrite the credential profile.

After revocation, the current epoch retains its generation and has
`"revoked":true`. Select that current state for historical receipt, audit and
terminal operations. It cannot authorize another live publication.

An exact retry returns the original retained receipt without applying the
lifecycle event again. For example, after operations `501: 1→2` and `503: 2→3`,
retrying `501 1 2` requires the independently selected current profile at
**generation 3**. Its result contains receipt generation **2** and current
generation **3** separately. A new recovery Fence is still acknowledged and
retained. Never replace today's profile epoch with an old receipt's generation.

## Complete historical guards remain required

The command admits the original plain, raw-forecast and owned-forecast learned
recovery paths, including selected computed identity, joint qualification and
checked-publication requirements. Replay keeps the original bounded cooperative
checkpoints, floors, effective policy, counters and credential policy/epoch
checks. It does not resume numerical generation, acquire effect permissions,
contact a helper, read live publication evidence or invoke terminal Time/Sweep.

Historical loading still requires the original independently selected numerical
recipe assets needed to reconstruct and check its immutable generation. It skips
the live perimeter inventory, broker/provider secret files and native helper
roster. Those live files may be absent during revocation or rotation. Their
absence does not become permission for a later live command.

Topology profiles are explicitly refused before loading or mutation. The current
paired source checkpoint pins an exact journal extent; acknowledging a lifecycle
Fence would require a corresponding source-custody successor. This command does
not relax that contract or strand a topology owner behind a stale checkpoint.

## Acknowledgment, deadlines and output

The result schema is `fa.learned-credential-lifecycle-result/1`. Its bounded
`knowledge` envelope reports the original nonsecret lifecycle receipt, current
credential status, acknowledged journal and recovery-Fence revisions, and retained
anchor revision. `status` is `applied`, `already_applied` or `refused`.
`authority` is `none`: the report is not a credential or publication permit.
The stated basis covers the original native lifecycle and independently selected
history; it does not claim authenticated remote-provider origin.

The configured runtime budget covers admission, replay, transition and reporting.
Once a Fence or lifecycle change is acknowledged, a subsequent deadline or stdout
failure cannot undo it. The operator attempts to retain the latest acknowledged
anchor before rendering the result, and returns an error when the command cannot
complete within its budget. A returned error after acknowledgment is not evidence
that the transition did not occur.

Output archives are created privately as new files, synchronized with their
parent directory and never overwritten. Keep the previous independent archive
until the new export succeeds. If export fails or leaves a partial file, keep the
old archive and select another new output path. A known result can report an
acknowledged transition even when `history_retained` is false; the command still
returns an error.

If a native journal write becomes ambiguous, the owner remains unavailable under
its original storage-failure contract. The command can retain only the previously
captured, acknowledged Fence prefix. It explicitly reports the transition and
current status as unconfirmed and produces no known-current result. That prefix
is not evidence that the later mutation failed. No effect or lifecycle write is
automatically resent to make an uncertain result look successful.

See [credential selection](LEARNED_CREDENTIAL_PUBLICATION.md),
[explicit route names](ROUTE_CREDENTIAL_SELECTION.md) and the original
[file credential lifecycle](FILE_CREDENTIAL_LIFECYCLE.md).
