# Read-only fleet situation operator

The publication operator now consumes the native declared-roster reader in
[Fleet publication situations](FLEET_PUBLICATION_SITUATION.md). Ordinary,
plain learned-generation, raw pre-output prediction and owned-K/V prediction
histories share one invocation, without a shutdown coordinator or recovery owner.
This is a bounded extension of FA-136 / plan 17.8, not full-system coverage or a
planner-based relevance contract.

## Invocation and independent roster

```text
supervise_publication situation --fleet /operator/fleet-situation.json
```

The complete grammar admits exactly this mode and one profile path. It cannot
be combined with ordinary situation options, credential activation, recovery,
retention, shutdown or other mutation options. The existing single-journal
`situation CONFIG` command and its learned recipe selection remain available.

Example profile (paths must refer to independently provisioned real inputs):

```json
{
  "schema": "fa.publication-situation-fleet/1",
  "runtime_ms": 60000,
  "input_bytes": 268435456,
  "journal_bytes": 134217728,
  "output_bytes": 4194304,
  "replay_events": 65536,
  "page_size": 32,
  "members": [
    {
      "id": 1,
      "config": "/operator/ordinary.json",
      "learned_recipe": null,
      "history_anchor": "/custody/ordinary.anchor"
    },
    {
      "id": 2,
      "config": "/operator/learned.json",
      "learned_recipe": "/models/learned-recipe.json",
      "history_anchor": "/custody/learned.anchor"
    }
  ]
}
```

Every field is required; absence of a recipe or anchor is explicit JSON null.
The existing strict JSON parser rejects unknown fields, duplicate keys, invalid
bounds and types. Member IDs are unique and nonzero. Configuration paths are
unique absolute normalized file paths. The readable configurations are resolved
before any model or journal intake; two members resolving to the same canonical
authority directory reject the whole query instead of double-counting coverage.
Missing configurations or unavailable directories remain declared Unknown slots.

Readable configuration files and selected anchors must resolve outside every
resolved authority store. The existing fleet asset loader applies its own same
cross-store path checks to learned recipes and assets. This is a path/custody
boundary, not OS isolation, inode authentication or a defense against a hostile
concurrent filesystem administrator. Unreadable members cannot establish their
own store identity. The roster remains caller-declared, not an authenticated
inventory of all workers or effect domains.

Learned members use the existing offline recipe/model/tokenizer/fit/monitor/
predictor loader with native-reviewer loading disabled. Original recipe bytes
select the corresponding numerical reader. A selected recipe or anchor that
fails never falls back to ordinary or unanchored replay. No live evidence read,
reviewer model, helper process, socket, credential secret or effect is required.
Learned recipes require the existing Linux operator profile; unsupported
platforms refuse that selection before member configuration intake.

## Conserved resource allowances

The profile is at most 65,536 bytes and is itself charged against `input_bytes`.
The maximum input allowance is 512 MiB, aggregate journal allowance 256 MiB,
output allowance 16 MiB, and elapsed allowance 86,400,000 milliseconds. The roster
contains 1..64 members, pages contain 1..64 attempts from one member, and the
aggregate replay event allowance is 0..1,048,576. Zero events admits only empty
valid journals or unavailable members, not a nonempty partially checked history.
Smaller explicit limits are supported; other byte/time limits must be nonzero.

Configuration and anchor reads reserve a bounded read plus an overflow sentinel
before I/O. Only successful reads refund unused bytes. Learned intake reserves
up to the existing 64 MiB asset ceiling from the same cumulative input allowance;
success refunds unused bytes, while failure conservatively spends its reservation.
A failed member cannot replenish later members' input budget. Exhaustion may make
later members Unknown; they are never omitted or treated as zero balances.

Before opening any canonical image, the command reserves each readable member's
full configured journal byte ceiling plus one overflow byte from `journal_bytes`.
This is intentionally conservative rather than a second read/stat of a mutable
journal to guess its eventual size. Exceeding this aggregate ceiling rejects the
query before model/journal reads. The native cursor then admits aggregate event
counts before applying any transition. These are work/input ceilings, not a peak
RSS promise; decoded structures and original numerical replay have their own costs.

One elapsed clock begins before profile intake and is conjoined with the runtime
limits of readable member configurations. It covers preparation, asset loading,
image binding, every original replay event, rendering and output. No member gets
a renewed deadline. Native replay advances one event per checkpoint, round-robin
across members. File loading and each numerical event remain synchronous; elapsed
checks do not preempt a blocking file operation or inference in the middle.

## JSONL and exit semantics

Each line uses `fa.publication-situation-fleet-page/1`. The `member` is the declared
numeric ID, not a path. Coverage counts repeat on every page. Known members nest
the unchanged original `fa.publication-situation/1` report under `situation`,
including that member's exact scope, journal revision, anchor prefix, recorded
clock, whole-cut accounting and attempt states. Unknown members have
`member_state: "unknown"`, a fixed failure category, and `situation: null`.
No arbitrary parser errors, paths, payloads or helper inputs enter the report.

`enumeration_complete` on the OUTER page means every declared member's pages have
been emitted. The inner original field covers only that member. The separate
`all_members_known` flag reports read coverage; an entirely read journal may
still contain unresolved or irrecoverably unknown effects. Completing enumeration
with any Unknown member returns a nonzero exit after emitting the bounded report.
A zero exit is read success only, not confirmed execution or current eligibility.

Missing reads sort first, then members with unresolved outcomes, pending work,
and other histories. Original per-member attempt ordering is retained. This is
lifecycle triage, not planner-estimated information gain. Every page derives from
one immutable fleet capture of independent historical cuts. There is explicitly
no atomic fleet snapshot, common revision, common clock or summed global ledger.

No output begins while any member is still replaying. A semantic replay failure
retires that member as Unknown rather than exposing its valid prefix. Deadline,
output-budget or write errors abort; prior lines or partial writes may remain
visible, and consumers must check both framing and process exit. No failure
replays the query, recovers a store, appends a fence or retries an effect.

## Verification status

The native feature adds eight regression functions using original canonical
histories and two-key construction. The operator adds six functions for complete
rosters and unknown members, lifecycle ordering and exact bases, closed grammar,
alias/self-anchor rejection, every pre-output deadline checkpoint, aggregate
limits and partial output. Two more loaded-operator functions exercise a mixed
ordinary/plain/raw/owned fleet, unavailable live reviewer/evidence assets,
changed-recipe failure isolation, shared intake charging and loader deadlines.
These sixteen new runtime tests are authored, not executed. Synthetic fixture
weights and ballots do not qualify detector accuracy or deployment safety.

Targeted native and operator tests, Clippy and the canonical check could not
start because `rch` is absent. Formatting could not start because `cargo` is
absent; the attempts returned exit 127. No Actions run was present for the native
feature commit when checked. Git blob hashes and narrowly scoped source changes
were reviewed separately; those checks do not establish compilation or runtime
correctness. No bead or full FA-136 acceptance condition is closed.
