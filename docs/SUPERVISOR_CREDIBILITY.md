# Held-out credibility in continued supervised publication

FA-105 implementation increment; plan 9.9, 9.11 and 17; FI-A08/FI-A13/FI-A18.
This connects the existing offline promotion to the runnable reference consumer,
not a new authority, score rule, executor, production protocol or agent verb.

## One recovered owner, qualification, and original two-key execution

The existing commands now accept an explicit operator-only option:

```text
supervise_publication submit CONFIG SUBMIT_JSON --credibility-activation EVIDENCE_FILE
supervise_publication submit-checked CONFIG SUBMIT_JSON WITNESS_PROFILE --credibility-activation EVIDENCE_FILE
```

The existing `--reviewer-profile REVIEWER_JSON` can precede or follow this option.
Without that reviewer option the original namespace-isolated reference transport
remains unchecked; supplying credibility evidence does not authenticate a reviewer.
The checked publication profile still must match its journal-pinned configuration.
There is no checked-to-legacy, old-weight, missing-file or source fallback.

A new request opens only an existing journal, performs its original recovery fence,
validates the mandatory deployment profile, and then reads the bounded evidence
file. It activates through `FileOversight::activate_credibility` in that SAME owner,
requires current qualification, and only then captures source evidence and enters
the original actor/helper/human/dispatch/publication/reconciliation workflow.
Neither actor bytes nor an expected predecessor is silently rewritten.

Opening an offline-qualified full-input owner deliberately stales the old
qualification. Therefore a standalone command that activates and exits would
not make the next owner eligible. Continued submission instead consumes an
explicit activation after reopening and before proposing new work. A historical
activation receipt cannot clear recovery invalidation or admit a new actor key.
A fresh promotion must pass all original label, scope, stratum, profile,
freshness, generation, cap and promotion-lane checks.

## Evidence file and predecessor discipline

The file uses the EXISTING `FACRED` canonical reference encoding already embedded
in both durable journals. `CredibilityActivation::encode_reference` exports it;
`decode_reference` reconstructs the original ledger and sealed snapshot. The
public `MAX_ENCODED_BYTES` limit is the original codec limit. No weights, scores,
rights balances, derived receipts or permits are imported. Encoding/decoding
alone does not authenticate a label or validate a governance request.

An independent evaluator/integrator constructs the original `CredibilityActivation`
with held-out evidence. Store its `encode_reference()` result in a bounded regular
file controlled by the operator, never in the actor's proposal or helper input.
Its expected control sequence must match the persisted original control sequence;
its expected authority epoch must include the next reopening fence. The original
actor proposal must name the further post-activation epoch (two advances from the
persisted epoch) and current endpoint target/version. Concurrent changes can make
both inputs stale. Scope, clock-domain mapping and evaluator authenticity remain
explicit operator assumptions, not facts inferred from numeric IDs.

This option is not accepted by create, resume, actor-submit or reviewer commands.
A new empty authority has not established this evaluation sequence domain. The
implementation does not invent completed control events to bootstrap eligibility.
Use an existing deployment with a valid independently prepared campaign.

## Read-only qualification-aware proposal planning

The original proposal builder now also accepts the same explicit option:

```text
supervise_publication proposal-next CONFIG REQUEST_ID PAYLOAD_FILE TTL_MS --credibility-activation EVIDENCE_FILE
```

This reads the existing canonical journal without opening a writable owner or
acquiring its lock. It checks that the decoded capsule binds this scope and the
next recovered sequence/epoch, then uses the original actor encoder with the
current target/version and the additional activation epoch. It executes neither
recovery nor activation, reserves nothing, and changes no journal bytes. The
unchanged builder handles invocations without the option. Invalid request IDs,
TTLs, payload bounds and arithmetic overflow retain their original refusals.

Supply the resulting document unchanged to `submit` or `submit-checked` with the
same capsule. The native owner still performs all qualification and authority
checks: successful planning does not certify sufficient evidence, allocate a
request ID, promise a future epoch, or bypass a concurrently changed predecessor.
Evidence and evaluator identities do not enter the actor document or helper view.

## Retries and failure boundaries

A recorded actor key is always query/reconciliation-only, including an expired
original document. The qualification path is NOT read on such a retry; missing
current evidence or a deleted capsule cannot hide a retained execution receipt.
The original gateway still distinguishes exact retries from conflicting bytes.
No new qualification, helper launch or human approval occurs for either case.

For NEW keys a missing, malformed, oversized, stale or unqualified capsule refuses.
The opening recovery fence may already be durable, but no new actor proposal is
admitted and no fallback workflow runs. Activation can itself commit before a
later source or submission refusal; it is an explicit governance transition, not
an atomic transaction with a future model execution. Its historical receipt is
not permission to resend. Original unknown effects remain charged until the
original endpoint supplies terminal evidence.

## Validation and scope

Nine Rust tests are authored in this increment: three option-parsing tests and
six public-codec/continued-workflow tests. The workflow fixtures use actual child
helper and reviewer sockets and temporary-file journals with synthetic labels.
They cover positive two-key publication, source-free exact/conflicting retries,
malformed/foreign/incomplete/stale inputs, no downgrade without the option, and
historical activation versus a fresh permitted activation. Existing tests and
assertions are preserved.

Required RCH example tests and xtask were attempted; both exited 127 because rch
is absent. cargo, rustc and rustfmt are absent too. Rust compilation, formatting,
Clippy and tests remain UNEXECUTED. Scoped diffs, baseline/upload blob hashes and
lexical checks are not substitutes for execution. No FA-105 closure, calibrated
reliability or production qualification follows. Live actor-service qualification
and a production authenticated evaluator remain outside this increment.

The planning/composition increment adds six Rust tests (fifteen total with the
submission increment). It pairs generated original documents with actual helper,
reviewer and publication consumers; checks read-only planning while an owner
holds the store lock; rejects stale/foreign predecessors and arithmetic failures;
and distinguishes a well-formed document from a qualifying campaign. Checked-mode
integration pairs an unrelated current-key change with an inserted forbidden key:
qualification cannot bypass the original negative witness or drop the stored
witness profile. These tests remain authored, not executed; the repeated RCH
example/xtask attempts again exited 127. No existing test assertion is weakened.
