# Cooperative native helper input

The original native worker process now yields during prompt BPE, before the
first numerical token. It consumes the existing incremental tokenizer and text
admission owners, then hands the completed encoding to the original monitored
generation session. The complete worker input, numerical budgets, monitor,
strict verdict reducer and commit/reveal protocol keep their original owners.

This is a bounded helper lifecycle improvement serving plan §§7.6, 9.2, 17.7,
18.7 and 19.4: exact opaque-input dependency, independently frozen judgments,
exact presented bytes, bounded preparation and causal interruption tests. It
advances the native helper reference path without closing FA-018 or qualifying
the production runtime or helper independence.

## Entry points and compatibility

| Entry point | Prompt scheduling | First numerical token |
|---|---|---|
| NativeEvaluator::begin | Existing synchronous full admission | A later advance |
| NativeEvaluator::evaluate | Existing synchronous admission and complete generation | Within that one-shot call |
| NativeEvaluator::begin_cooperative | Freeze the complete input, then return Admitting | An advance after admission has returned Running |
| NativeHelperClient::step and drive | Existing synchronous admission for a fresh input | The first inference step, as before |
| NativeHelperClient::step_cooperative | At most 256 original tokenizer advances per call | A later call after the admission handoff |
| run_native_worker and fa-native-helper | Use step_cooperative | Charged to a separate existing process step |

Calling an existing peer step/drive on an owner already started through the
explicit cooperative entry point continues that pending admission in the same
bounded quantum. It does not restart or synchronously drain the tokenizer.
The existing in-process sidecar and learned publication review paths continue
to use synchronous admission. Their source refresh schedule, numerical poll
denominators and existing allowance ceilings are unchanged.

The process chooses cooperative admission explicitly. Existing manifest schemas
and worker wire bytes are unchanged, but prompt preparation now consumes process
steps. An operator's existing step limit can therefore stop earlier in numerical
progress. That allowance is spent, not enlarged or silently refunded.

## Exact ownership and completion

NativeEvaluator freezes the original WorkerInput once, before fallible startup.
Its prompt remains exactly actual_input().submitted_bytes(), including binary
bytes. It retains the entire input profile, ordered parts and omissions. It
does not render a sidecar into text, normalize, summarize, truncate, add a chat
template, append policy/salt bytes or select a subset of the witnessed input.

The private Admitting execution state owns the original TextGenerationAdmission.
An evaluator advance in this state calls the original cursor once. Its input,
pair-lookup and heap-pop budgets remain the original TokenizationBudget; stale
heap entries retain their original charges. Completion transfers the same
TokenizedInput through finish_incremental. That handoff admits the full prompt,
output capacity and numerical request without re-tokenization or inference.
It returns Running and yields before computing the first token.

The original generation session then computes at most one monitored numerical
token per evaluator advance. The helper still requires complete prompt review,
the independently configured non-text stop token's own review and an exact
complete answer of allow, hold, deny or abstain. Only that terminal result can
freeze the original commitment and reveal frames. The supervisor's original
reveal request remains required. A local result is not congress acceptance or
publication authority.

## Costs, deadlines and cancellation

NativeEvaluationProgress adds two separate diagnostics:

- tokenization contains the original TokenizationWork, including actual work
  retained after admission refusal, cancellation and a caught interruption.
- admission_steps counts cooperative cursor advance attempts. It includes a
  failed/interrupted attempt and remains zero for the synchronous-admission
  entry point. It is not a byte, FLOP, allocation or elapsed-time counter.

Original numerical work remains in NativeEvaluationWork. During Admitting it
is zero, requested_prompt_tokens is zero because the complete count is not yet
admitted, and no token IDs, answer prefix or permitting result escape. After
completion the original full prompt count and work become available. A
generation-admission refusal retains completed tokenizer work even though no
generation report exists.

The peer quantum is fixed at 256 original cursor advances, with an immediate
yield when admission completes. A quantum cannot compute the first native
token as a side effect. The process charges each quantum to the existing
max_steps and checks its original absolute deadline before and after that call.
The lifetime still starts before checkpoint startup; neither model loading nor
prompt preparation renews it.

Cancellation destroys the private unfinished owner and closes its original
observation source. It retains actual tokenizer/numerical work, the input and
any already completed report. A failed attempt or caught interruption remains
latched; no second input, replacement seed, partial answer or retry is returned.
If a deadline coincides with a tokenizer failure, the process reports Deadline
while retaining the original failure and spent work. Previously sent commitment
bytes remain sent even if a later step limit prevents the reveal.

Cooperation is bounded by original operations, not time preemption. Initial
bounded copies, allocation, named-control matching and any configured Unicode
segmentation still run synchronously. Final output allocation, numerical
admission, an individual numerical token and filesystem operations are also
not preempted. A late operation cannot start the next numerical step or protocol
write after the process observes expiry. Supervisor cutoffs remain independent.
The existing input, vocabulary, context and work caps are unchanged.

## Verification boundary

The standalone public target is:

    cargo test --locked -p fa-reference --test native_cooperative_input

Its fixture uses the original file-backed Llama/SafeTensors bootstrap and a
403-byte binary input containing 80 real BPE merge chains. Independent framing
retains three typed input parts and a closed-absence marker. The expected 83
prompt tokens and 320 merges follow from the declared byte vocabulary and merge
chains; original tokenizer output additionally supplies exact spans and work.
The two last-byte variants produce actual native Allow and Deny.

The tests cover synchronous/cooperative parity, exact tokenizer allowances and
each one-short refusal, full context admission, peer cancellation after real
merges, the unchanged synchronous first-token behavior, and original fragmented
commit/reveal. Process tests exercise mid-tokenization step limits, the
zero-inference handoff, the next permitted numerical step, exact completion
allowance and a one-short allowance that retains the sent commitment. The
actual fa-native-helper binary executes the same positive/negative path through
the unchanged manifest schema.

New focused unit modules under native/incremental, native/peer and native/process
exercise caught interruptions, original observation-source closure, held
terminal and invalid complete-output refusal, deterministic expiry during BPE
and immediately after admission, and coincident tokenizer failure/deadline.
Existing native and learned sidecar regressions remain unchanged.

Central execution with qualified `nightly-2026-09-08` passes all nine new public
tests, all 110 native-helper unit tests (including the five new deadline and
interruption cases), and all 15 executable worker checkpoint/LoRA/static-RoPE
regressions. No existing assertion or allowance was weakened. Passing
these synthetic cases establishes the bounded composition and its causal
refusal boundaries. It does not establish trained detector quality, empirical
alignment, process independence, operating-system containment, production-sized
model support, a latency SLO or complete workspace qualification.
