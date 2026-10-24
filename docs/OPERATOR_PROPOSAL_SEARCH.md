# Search repairs to an original durable proposal

The Unix reference operator now connects the ORIGINAL cooperative proposal
repair search to an independently anchored request investigation:

```sh
supervise_publication explain-proposal CONFIG REQUEST_ID RECOVERY_PROFILE --search MENU_JSON
```

This extends [original proposal diagnosis](OPERATOR_PROPOSAL_DIAGNOSIS.md).
The existing baseline and `--payload` invocation retain their meanings. The
options are mutually exclusive; missing files, malformed menus and unknown
options fail rather than silently requesting another mode.

## Original history, never recovered authority

The command uses the same exact source-publication guard set, independently
retained `fa.guarded-stop/1` input anchor and historical floors as diagnosis.
`retain_history_anchor` remains forbidden: a read acknowledges no new durable
cut. The first original admission is selected by external request key, not an
assumed internal attempt number. The whole captured journal suffix and ending
requirements qualify before any branch data is exposed. No second history
replay, recovered owner, writer lock, current source read, helper invocation,
human approval, authority clock or effect command is introduced.

The search starts from `FileProposalInvestigation`, then calls the original
`FileProposalRepairSearch` and `ProposalRepairCursor`. The operator contains no
alternative policy evaluator or minimality solver. Each branch starts at the
same original admission, not at the preceding branch or the ending journal
state. A later cancellation, execution or source withdrawal does not rewrite
that admission. A concurrent writer can advance after image capture; the output
still names the captured revision, not necessarily the latest state.

## Closed, bounded registration

An example menu registers payload and unit interventions explicitly:

```json
{
  "schema_version": "fa.proposal-search/1",
  "scope": {
    "id": 81,
    "payload": true,
    "target": false,
    "units": true,
    "keys": []
  },
  "budget": { "cases": 4, "retained_edit_bytes": 4 },
  "candidates": [
    { "kind": "payload", "bytes_hex": "6f6b" },
    { "kind": "units", "value": 1 }
  ]
}
```

Every scope field and both native budget dimensions are required. The scope ID
is nonzero. Keys are unique unsigned integers; they must have original retained
evidence, not merely appear in an edited candidate. This registration limits
hypothetical data changes; it confers no capabilities.

The four original intervention forms are:

* `payload`: required `bytes_hex`.
* `units`: required unsigned `value`.
* `target`: required nonzero unsigned `adapter`, `object`, `contract_version`,
  `expected_version` and `generation`.
* `key`: required unsigned `key`, and required `expected_hex` and
  `replacement_hex`, each a lowercase hex string or explicit JSON null.

For keys, null means exactly known absence, not a wildcard; empty hex means a
present empty byte string. A stale expected value remains a refused native
branch. No key edit creates observations or closes an unobserved range. All
candidate byte occurrences count, including repeated preimages/replacements.
Conflicting alternatives retain their positions; subsets containing two edits
to the same field remain explicit refused cases rather than being deduplicated.

The encoding is limited to 256 KiB, six levels, 512 JSON items and bounded
strings. Duplicate and unknown fields, unsupported kinds, undeclared edit
fields, invalid hex and more than the original eight candidates refuse. The
native original action and aggregate 65,536-byte intervention limits apply
before branch execution. No native maximum has been raised.

For N candidates the ORIGINAL search admits all 2^N masks, including mask zero.
The native case allowance must cover that entire menu. Its retained-edit-byte
allowance must cover the sum of input edit bytes multiplied by 2^(N-1) for a
nonempty menu, or zero for an empty menu. The native fixed maxima remain 256
cases and 8 MiB of planned retained edit bytes. Underfunded menus refuse before
search execution; allowances are not renewed per branch. These byte counts do
not measure process memory, evidence/trace allocation or output size.

## Every outcome and every missing case stays explicit

Output is one `fa.proposal-search-result/1` JSON object. It contains historical
request/attempt, registration, admission and captured ending state, original
policy/semantic generations, witness count, execution/accounting counters,
ordered candidate edits, budget, work, cases, sufficient repairs and unreported
masks. Candidate bytes appear once in hexadecimal; cases refer to candidate
indexes. Identity and durable u64 counters are decimal strings; bounded counts
and mask/index values are JSON numbers.

Native masks run in increasing binary order, not best-first or cardinality
order. Each case preserves either the original evaluator's baseline,
counterfactual truth, full node-indexed truth trace, changed nodes and next
requirement, or the original refusal code. Successful outcomes are not selected
at the expense of failed alternatives or the untouched control.

The native per-repair minimality values retain their exact meaning:

* `established_within_menu`: every proper subset has an established exact-policy
  violation, and the complete search has finished.
* `not_minimal`: a proper subset already satisfies the exact policy.
* `undetermined`: incomplete execution or unknown/refused smaller cases prevent
  the stronger claim.

This is subset minimality, **not globally fewest edits**. Repairs of different
cardinalities can both be minimal in this sense. The untouched sufficient
control is explicitly labeled `untouched: true`; it is not a newly found action.
Every result has `purpose: experiment` and `permission: not_issued`. Sufficient
repairs still require fresh independent review; they never inherit helper votes
or change the original denial. `complete` means enumeration completed, even
when every branch fails. It does not mean an effect was approved.

## Deliberate cuts, deadlines and output failures

Optional `stop_after_cases` requests a native cancellation after at most that
many cases. It can be zero and defaults to 256. The whole menu must still pass
original resource admission; this is not an allowance bypass. Before the full
menu finishes, the command returns a cancelled report and a nonzero exit.
Planned, entered and completed cases, planned and retained edit bytes, existing
outcomes and every unreported mask remain distinct. Partial reports never claim
established minimality. Already proved nonminimality can remain visible.

One monotonic runtime allowance starts before CONFIG reading and covers menu
admission, independent custody, original journal event checkpoints, each search
case, final report construction and pre-output. A deadline or caught-unwind
interruption publishes no partial JSON. A deliberate case cut is different: it
returns the native cancelled report. No cursor is resumed after an unwind.

Output is built under a separate 4-MiB encoding bound before writing stdout.
A write or flush error remains an error and may leave partial stdout; it cannot
change a request, mint a key or retry an effect. File I/O, one original event or
case, rendering and output writes remain synchronous. Neither checkpointing nor
these logical byte limits constitute a hard latency or allocator-memory claim.
Safe shell redirection and private operator access remain the caller's duties.

## Scope and verification record

This is an effect-free reference operator increment serving plan sections 12
and 17, especially scoped counterfactual interventions and typed diagnostic
repair suggestions. It is not the full authorization-filtered production agent
API, production containment, inference quality, or roadmap-packet completion.
It preserves the existing founding separation between experiments, observations,
judgments and authority rather than adding a new founding claim.

The new tests use original source observations and durable journals with a held
writer lock. They cover native report parity, two independent blockers,
conflicting alternatives, nonmonotonicity, all explicit case cuts and deadline
checkpoints, exact/one-short native admission, malformed registrations, absent
live assets, stale versus unseen key baselines, precise binary/null encoding,
output failures, concurrent writer progress, and partial range evidence. A
separate control proves subset-minimal solutions can have different sizes.
Existing diagnosis tests and the native solver are unchanged.

Status: authored implementation and regression sources, **not compile/test
qualified**. Required targeted and full remote gate attempts and the exact
selected source manifest are recorded in
[the attempt receipt](../artifacts/execution/2026-10-13-proposal-search-attempt.json).
The launcher is unavailable in this editing environment. No Rust test, formatter,
Clippy, production gate or bead is reported complete by this source increment.


## 2026-10-24 selected execution

A qualified standard-backend RCH run passed all 21 selected `diagnosis::` tests
(18 behavioral cases and three renderer cases), with zero failed/ignored and
706 unrelated example functions filtered. Source: `b688a37bc7baea0efce2d53fc1b0bb769e40af86`
plus the three owned diagnosis fixture/test paths, overlay fingerprint
`69126149d57c429e2ae1d0227638da806ab08be68a914f04c3ab80717c972627`.
The fixture repair funds payload bytes through the original adapter, preserves
original policy limits and denial/search assertions, and retains occupied
staging bytes before the separate positive writer phase. No production command,
policy, budget, storage behavior or test registration changed.

Earlier failed attempts remain retained. In particular, the complete original
727-function example run recorded failures and aborted on a learned-test stack
overflow; it has no complete suite result. The frozen full driver still refuses
the initial package-admission boundary. This selected result does not qualify
the complete example, roadmap packet, authorization-filtered agent API or release.
Raw logs are outside the frozen root at
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-implementation-0761ea9b/`.
