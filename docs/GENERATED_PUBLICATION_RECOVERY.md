# Recovering native generated publications

## Runnable consumer

The existing Linux `supervise_publication` command accepts:

```sh
supervise_publication create-generated CONFIG GENERATION_RECIPE --resume
```

The last argument replaces the reviewer profile deliberately: receipt recovery
opens no reviewer or stop socket, launches no helpers and reads no current evidence
file. It requires the independently retained original model, monitor, sampler,
tokenizer, stream profile and exact text recipe. Those operator-owned input files
are still read and validated; this is not recovery without the original model.

The existing native-stream opener verifies its immutable bootstrap and logical
terminal reserve, replays the original numerical history, and durably fences old
sendable keys. The command additionally checks the configured mandatory source
policy and publication profile. It then checks the historical source-linked
request against the generation ID and exact text request, including prompt,
control/stop tokens and numerical/tokenization/output budgets. No ordinary
caller-text request or neighboring generation can stand in for that association.

Recovery invokes the SAME query-only reconciliation used by the existing
publication workflow and polls the SAME restricted generated actor gateway.
A fresh actor wire has no polling ticket. Its only Submit frame repeats the
prevalidated EXACT retained source reference to reacquire that process-local
ticket; the original retry branch only reads recorded status. The command verifies
that this step did not change the journal revision. It never submits a new effect,
regenerates output, reauthorizes an effect, resumes the decoder, or obtains a new
human approval. A terminal result needs neither a clock observation nor a source
lease; unresolved dispatch uses the
original settlement semantics. Request absence and identity mismatch are errors,
not invitations to create a store, generate a token or retry an effect.

The recipe lifetime does not renew an earlier publication deadline: no new
action is built from it. Rejected or cancelled work is not reauthorized, and an
unknown result can be resolved only by the original reconciliation evidence.
The existing stdout emitter returns success only for the original Executed outcome, and never prints raw generated text. Output failure cannot
change committed work or send another publication. Old human approval is not
resurrected; the writable owner remains decoder-paused after recovery.

## Scope and limitations

This closes the create-only command's missing receipt-recovery path. It is not
continuation of an interrupted generation, automatic retry of pre-dispatch work,
remote delivery proof, authenticated anti-rollback protection or deployment
qualification. A missing source-linked request, even with saved numerical work,
is not a completed publication and is not advanced by this command. The original
opener can append its recovery fence before a later request/recipe mismatch is
reported; such a refusal does not imply that the journal was untouched.

Original recovery is synchronous and may replay substantial bounded numerical
history. The command does not pretend to provide a watchdog during that replay.
The unchanged generic inspection APIs remain privileged; this command emits only
the original actor response.

## Changelog and implementation status

Added the native recovery consumer, routed through an explicit --resume form of
the existing generated command. Seven regression functions use actual durable
native generation/publication and the existing synthetic model/helper fixtures.
They cover lost stdout with source/helper absence, rejected publication, exact
recipe mismatches, missing store and wrong monitor, failed response output, and
closed command-option admission. Assertions include preserved publication count,
charged rights, native generation revision/sample count and paused recovery.
The ticket regression pairs an unsuccessful bare Poll and conflicting source with
a successful exact source retry and unchanged original journal revision.

These are authored tests, not execution evidence. The required command was
attempted in this environment:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It could not start: `rch: command not found` (exit 127). Compilation, Rust tests,
rustfmt and Clippy remain UNEXECUTED. No production gate or bead is closed. There
is no new dependency, journal format, authority type or alternative effect path.

## Fresh continuation after a confirmed native message

```sh
supervise_publication create-generated CONFIG NEXT_GENERATION_RECIPE REVIEWER_PROFILE --after PRIOR_REQUEST_ID
```

`--after` selects an existing native-only stream rather than creating another
store. The recipe must use a NEW request ID and NEW generation ID, and retain the
exact model/monitor/sampler/tokenizer/stream bootstrap. Its prompt is additional
input to the retained numerical context; it is not an inferred chat template,
reset, replacement conversation or caller-supplied output. The prior request must
be the latest receipt-confirmed native message, not merely computed, published
without reconciliation, rejected, cancelled or an older confirmed prefix.

The command uses the original pinned opener and native
`prepare_decoder_text_continuation`/`begin_decoder_text_continuation` pair. It
checks request-ID freshness, helper credibility, the full confirmed prefix,
unchanged numerical predecessor and the ENTIRE requested context/disclosure
horizon before new computation. Preparation while paused is not authority. A
fresh mandatory source observation and post-read clock precede the explicit
original decoder resume and the second native continuation-cut check.

Creation and continuation now share the same implementation for every numerical
step, independent stop checkpoint, source acquisition, helper congress, human
review, publication and reconciliation. Lifetime numerical/monitoring work, RNG
state, previous disclosure and cumulative resource charges remain in the original
owner. The new effect includes all confirmed message boundaries, receives a fresh
policy-epoch-bound helper review and requires a fresh human key. Helpers must be
independently configured for the current post-recovery policy epoch; an old helper
profile is never silently accepted. Independent stop remains active from listener
creation through computation and publication, but not as a background watchdog
inside synchronous numerical recovery.

`--after` is never a retry: an already used request or generation refuses before
new computation. `--resume` remains the separate receipt-only path, and the two
options cannot be combined. A failed continuation preserves earlier publication
charges; missing source cannot spend another token. Current source/qualification
failure never causes fallback to a different deployment profile. A preflight
refusal can still follow the original opener's recovery-fence write.

Six additional functional regressions plus an explicit subprocess helper harness
cover real second-message publication and receipt recovery, fresh-human rejection,
stale prefixes and used IDs, rejected predecessors, remaining-context exhaustion,
source loss after reopen and closed option admission. The positive test expects
two confirmed message boundaries, numerical position advancing from four to eight,
and the sum of both complete-frame resource charges. The subprocess helper pins
the expected policy epoch independently rather than copying it from an offer.
All tests remain authored but UNEXECUTED under the RCH limitation above.
