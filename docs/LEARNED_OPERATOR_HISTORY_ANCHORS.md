# Independently retained history for learned operator recovery

The learned publication command can require an independently retained exact
history prefix and export a new checked prefix from its acknowledged owner.
This connects the existing [original history-anchor contract](EXACT_HISTORY_RECOVERY.md)
to actual operator recovery, including required raw-residual and owned-K/V
forecast recipes. Recipe versions 1–5 and journal encodings retain their meaning.

## Operator inputs and outputs

Append these options to an existing `create-learned-generated` command:

| Option | Meaning |
| --- | --- |
| `--history-anchor INPUT` | Require the independently retained original prefix before recovery. Valid with `--continue-generation`, `--after-message`, `--finish-after-message`, and message or finish receipt `--resume`; refused on initial creation. |
| `--retain-history-anchor NEW_OUTPUT` | At final workflow or receipt settlement, export the acknowledged original owner to a new private operator file. When an input is supplied, the owner must also be a genuine successor of that prefix. |

Each option takes one absolute normalized file path and can appear only once.
The options are separate from the immutable numerical recipe. Omitting both
retains the earlier command behavior without anchor filesystem access.

For example, retain an initial completed publication:

```sh
supervise_publication create-learned-generated /operator/host.json /operator/learned.json /operator/reviewer.json --retain-history-anchor /custodian/publication-1.anchor
```

Continue a version-4 or version-5 stream after the original confirmed message:

```sh
supervise_publication create-learned-generated /operator/host.json /operator/next.json /operator/reviewer.json --after-message 1 --history-anchor /custodian/publication-1.anchor --retain-history-anchor /custodian/publication-2.anchor
```

Recover the original message receipt and retain its recovery fence:

```sh
supervise_publication create-learned-generated /operator/host.json /operator/next.json --resume --history-anchor /custodian/publication-2.anchor --retain-history-anchor /custodian/receipt-recovered.anchor
```

The custodian must independently retain and select the input. The command
rejects paths whose canonical destination lies inside the authority store,
including parent-directory aliases. A different pathname or directory does
not establish independence of ownership, backups or failure domains.
The selected custody namespace must remain operator-controlled and stable
through admission, export and synchronization. Exclusive file creation prevents
overwriting the final entry; it does not isolate ancestor pathname resolution
from a hostile namespace change.

## Admission and exact recovery

The input uses the original `FAHANC\0\x01` standalone transport, bounded by
`MAX_HISTORY_ANCHOR_BYTES` (16 MiB plus its 24-byte header). The existing regular
file reader and `FileHistoryAnchor::read_trusted` reject missing, nonregular,
selected final-component symlink, oversized, malformed, truncated or trailing
input. No selected failure falls back to numeric floors or reconstructs an
expected anchor from the journal being recovered.

Every original learned recovery mode selects its corresponding anchored
guarded constructor. The independently required prefix is compared with the
same exclusively locked canonical image subsequently replayed. Genuine
append-only successors remain recoverable. Truncated prefixes and divergent
valid histories refuse even if their revision, control sequence and authority
epoch satisfy every numeric floor.

Complete original generation and predictor identity, guard inventory, current
policy, numeric floors and the exact terminal recovery reserve remain required.
The anchor does not replace any of them. Success still appends the original
single recovery fence before issuing roles, withdraws old eligibility and keys,
preserves spent numerical work, and retains the original lost-forecast-coverage
rules. Receipt recovery does not regenerate output, repeat review or resend an
effect.

## Retention and failure semantics

Retention uses `history_anchor()` on a healthy acknowledged owner, or
`history_anchor_after(previous)` when the operator supplied a prefix. It never
substitutes current disk bytes for a faulted owner's acknowledged history.
Neither the expected archive nor the journal is modified by exporting it.

The output must be absent during admission. At actual export the command uses
exclusive new-file creation with owner-only permissions on Unix, writes the
original bounded transport, synchronizes the file, then synchronizes its parent
directory. A destination appearing after admission is refused without overwrite.
The old archive must remain retained until the new export is confirmed. A failed
write or sync can leave a partial or unconfirmed file; its existence is not a
successful retention receipt. Select a new output path for a later retry.

The archive contains the full canonical retained prefix, including private
payloads and review inputs. It is not an actor-facing artifact and is never
written to stdout. Stdout remains the original restricted actor response.
An anchor-output failure is appended to the command failure, preserves that
response and the original effect state, and still performs normal cleanup. It
does not start another forecast, review, publication or effect retry.

Retention happens at the final workflow or receipt path before driver cleanup.
It is not a transaction spanning publication and independent custodian storage.
A process crash or an earlier failure after the recovery fence can leave no new
archive. A nonzero command result therefore does not imply that publication or
the recovery fence did not occur. The supplied old anchor still covers its
original prefix; use the original receipt recovery path to inspect or settle the
existing request.

The supplied anchor is trusted operator data, not a signature or independent
authentication. It does not authenticate the unanchored suffix or the original
observations, prove that the latest cut was retained, or protect against rolling
back both the journal and the selected anchor. This is the scoped implementation
of plan §§16.4–16.5 and 16.9 under FA-014, serving FI-A02. It does not close the
full crash/remote-effect campaign or the FA-096 deletion and repair contract.

## Verification boundary

New operator regressions exercise the original loaded numerical owner, all three
recovery representations, valid equal-counter forks, rollback, unchanged
staging on refusal, guard/reserve/floor preservation, real native review and
publication followed by receipt-only anchored recovery, closed file admission,
and a post-publication output race preserving the original effect and response.

Eleven new test functions are authored and independently source-reviewed. Seven
changed Rust files passed syntax parsing with an older, unqualified rustfmt;
that did not rewrite source or type-check it. The targeted anchor-test dispatch
and the complete `xtask check` dispatch both stopped before compilation with
`env: rch: No such file or directory` (exit 127). No Rust compilation, test,
Clippy or complete workspace qualification is claimed. Scoped work items remain
in progress; the full FA-014 packet retains its original prerequisites and open
acceptance.
