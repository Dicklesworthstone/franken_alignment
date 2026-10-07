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
It never constructs a Submit frame, regenerates output, reauthorizes an effect,
resumes the decoder, or obtains a new human approval. A terminal result needs
neither a clock observation nor a source lease; unresolved dispatch uses the
original settlement semantics. Request absence and identity mismatch are errors,
not invitations to create a store, generate a token or retry an effect.

The recipe lifetime does not renew an earlier publication deadline: no new
action is built from it. A rejected/cancelled/pending/unknown result stays such a
result. The existing stdout emitter returns success only for the original
Executed outcome, and never prints raw generated text. Output failure cannot
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
the existing generated command. Six regression functions use actual durable
native generation/publication and the existing synthetic model/helper fixtures.
They cover lost stdout with source/helper absence, rejected publication, exact
recipe mismatches, missing store and wrong monitor, failed response output, and
closed command-option admission. Assertions include preserved publication count,
charged rights, native generation revision/sample count and paused recovery.

These are authored tests, not execution evidence. The required command was
attempted in this environment:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It could not start: `rch: command not found` (exit 127). Compilation, Rust tests,
rustfmt and Clippy remain UNEXECUTED. No production gate or bead is closed. There
is no new dependency, journal format, authority type or alternative effect path.
