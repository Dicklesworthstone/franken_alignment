# Cooperative prompt and non-forecast generation

The actual `create-learned-generated` numerical loop now drives prompt processing
and non-forecast output through bounded original-history replay. This complements
[forecasted generation](COOPERATIVE_FORECAST_GENERATION.md) and the existing
[recovery controls](COOPERATIVE_LEARNED_OPERATOR_RECOVERY.md). The consumer is the
publication supervisor, not an alternate runtime or a replacement model.

## Behavior

`learned/generation.rs` holds only original intent/completion preparations and
moved acknowledged replay predecessors. Each call processes at most 32 historical
reducer calls and at most one finalizer. An acknowledged intent returns before
its numerical completion, even when the replay tail fits within one quantum.
The outer operator then runs its existing stop, deadline and fresh-policy checks.
The scheduler never clones a live owner, changes the sampler, invents a token,
installs a source or replenishes numerical, telemetry or effect allowances.

The first preparation starts cold without taking the live control cache. Later
preparations reuse the scheduler's own original retired machine. Fresh source
and clock events are replayed through the existing bounded catch-up mechanism;
no suffix is skipped or candidate imported. A changed numerical predecessor is
not adopted. A consumed-preparation or finalization error/unwind latches the
scheduler against retrying an earlier candidate. Original storage poisoning and
recovery determine whether a failed acknowledgment became visible.

For required raw-residual and owned-K/V profiles, this generic scheduler stops at
the complete prompt boundary. It refuses sampling even when a forecast already
exists: output must run through the actual separately held forecast stepper.
The operator drops the prompt replay custody before acquiring/driving that
forecast. The library's original admission checks remain in force too.

Non-forecast recipes retain their original meaning. Initial prompts, accepted
stops, sampled draws, retained K/V state, logical work and source-only proposals
still originate from the same model. A recovered pending intent is completed,
not silently replaced; fresh evidence and explicit resume remain required.
Publication still needs native review, independent human approval and the
original endpoint receipt. No command, dependency or journal format was added.

## Bounds and qualification

The 32-event allowance counts historical reducer calls in one invocation and
excludes the finalizer. It is not a hard time, CPU or memory bound. Individual
historical events, numerical steps, source refresh, forecast acquisition,
encoding and filesystem work can still be synchronous. Additional private
replay custody retains memory; no memory reduction or throughput gain is claimed.

Six new loaded-operator regression functions use the original numerical,
policy-source, journal, review and publication owners. They cover step-by-step
parity against synchronous generation with separate intent yields; a cold
non-forecast run proceeding through both publication keys; required raw/owned
prompt processing that cannot bypass forecasted sampling; missing policy and
expired workflow after intent acknowledgment with a successful matched control;
refusal to adopt another numerical step; and pending-prompt recovery followed
by the original output. Existing authenticated-stop tests remain registered.
Fixture weights and probabilities are deterministic controls, not calibration.

These tests are authored and source-reviewed but UNEXECUTED. The prescribed
remote-only RCH test launch stops before compilation because `rch` is absent;
`cargo`, `rustc` and `rustfmt` are absent as well. Selected-source whitespace and
Git preimage checks are not Rust validation. No compilation, formatting, Clippy,
workspace, performance, calibration or production qualification is claimed.
FA-111/FA-014 and their unmet acceptance conditions remain open.
