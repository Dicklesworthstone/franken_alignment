# Explicit named controls in byte BPE

## Source status

2026-09-22: authored implementation and ten Rust regressions; compilation, tests,
rustfmt and Clippy are UNEXECUTED. The targeted remote-only command refused before
execution because `rch` is unavailable (exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference tokenizer::special
```

No prior execution receipt qualifies these changes. No production bead is closed.
This extends the FA-025 original-token inference path, not effect authority.

## Native behavior

`ByteBpe::new_with_special_tokens` takes the original profile, vocabulary and merge
rules plus a map from existing Control IDs to literal UTF-8 spellings. Construction
runs the original complete vocabulary/merge validation. Named controls never
participate in merges; unnamed controls remain explicit-ID-only. Registration is
immutable, so it cannot change a live text owner's tokenization halfway through
its cache history. `ByteBpe::new` keeps its old behavior, including treating strings
that resemble unregistered controls as ordinary content.

At the earliest matching byte offset, the longest registered name wins. Names can
overlap or be prefixes of one another; token IDs and insertion order do not select
the winner. There is no regex, normalization, word-boundary rule, whitespace
stripping, guessed BOS/EOS or template. Original source bytes and complete spans
are retained, including a special's entire spelling and unmodified invalid UTF-8
elsewhere in the native byte-input extension.

The sparse recognizer uses reversed patterns and a reverse input scan with failure
links, retaining only the longest match starting at each byte offset. A forward
pass selects nonoverlapping controls before the ORIGINAL heap-based BPE pass.
Ordinary merges cannot cross a control. There is no repeated whole-prompt scan per
name, output of all nested matches, or dense states-by-byte transition matrix.

Registration allows at most 256 names, at most 4096 bytes per name, and at most
65536 aggregate spelling bytes. Duplicate spellings, invalid UTF-8, empty names,
non-Control IDs and unknown IDs refuse. The trie has at most aggregate bytes plus
one states. Input remains capped at 65536 bytes; its longest-match vector has one
slot per byte. Existing pair-lookup and heap-pop budgets/counters cover the BPE
pass, not recognizer construction or matching. No latency, allocation, RSS or
production-cost claim is made. General allocator aborts are not Result-level
recovery guarantees.

Controls remain non-text: `decode` refuses to emit them. Text generation and native
helper evaluation must still observe the original configured stop, consume its
sampling draw, and complete its mandatory monitoring before treating it as a
terminal. A control in the forced prompt is input, not an early generation stop.

## Interchange

Unregistered tokenizers retain byte-identical `FABBPE01` archives. A tokenizer with
at least one registered name uses `FABBPE02`: the same 120-byte model-bound header,
original vocabulary and merge table, followed by a big-endian u32 name count and
strictly increasing records `(u32 id, u32 byte_length, literal bytes)`.

Empty version-two name tables, unordered/repeated IDs, malformed names, missing
bytes, unknown versions, wrong profiles and trailing data refuse. Sizes are
checked before allocating names; the same native constructor validates imports.
Older readers reject version two rather than silently losing recognition. This is
a tokenizer DATA format extension, not a change to the authority/action journal.

## Authored regression coverage

Tests pair named and unnamed controls, longest/overlapping matches, Unicode and
binary input, boundaries around actual BPE merges, exact/one-over registration and
input budgets, and long common-prefix input. An independent literal scanner plus
whole-sequence greedy BPE compares 9331 finite inputs without sharing the trie,
failure links or candidate heap. Manual version-one/two archives test exact bytes,
all truncations, trailing data, altered versions/profiles, ordering and limits.
These are authored tests, not executed evidence or upstream-library parity claims.
