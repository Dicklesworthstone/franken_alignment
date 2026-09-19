# Native byte-BPE input contract

`generation::tokenizer::ByteBpe` supplies a bounded, exact byte-pair encoder for
native monitored inference. Its scope is **FA-BBPE/1**, not the general
Hugging Face tokenizer format. Parameter loading alone does not establish a
checkpoint's tokenization: the operator must supply and authenticate a vocabulary
and ordered merge table whose trained semantics match this profile.

## Supported meaning

The entire input is one byte sequence. There is no normalization, whitespace
rewriting, regex pretokenization, chat templating, dropout, automatic BOS/EOS,
unknown-token substitution or recognition of control spellings in user text.
All 256 byte values have a unique singleton token; vector positions are the actual
model token IDs, which need not equal byte values. Each content token has unique
nonempty bytes. Each ordered rule concatenates already defined content tokens.
Every multibyte token must be reachable. A control token has no guessed spelling
and cannot be produced by encoding text or silently stripped while decoding.

The next merge is the lowest-ranked currently adjacent pair, with the leftmost
occurrence winning ties. The encoder uses a heap and linked input intervals; only
the two neighboring candidates change after a merge. Stale candidates are
rechecked and counted. The resulting immutable `TokenizedInput` retains original
bytes, original IDs, and a complete partition of byte spans. Unicode and invalid
UTF-8 are both retained verbatim. Existing numerical replay continues to consume
original IDs, not a text round trip.

The complete `DecoderProfile` binds model/tenant/tokenizer generations, vocabulary,
dimensions, context and numerical configuration. This is an exact data binding,
not authentication of an operator-supplied model or tokenizer. Cloning these
objects copies observations; none can supply effect credentials, a permit, or a
monitoring verdict. The optional front end does not change the numerical decoder's
original-token API.

## Budgets and interchange

Encoding admits at most 65,536 input bytes. Pair examinations and heap pops have
independent caller budgets capped at three times that maximum. Actual stale pops
count, and a budget failure returns spent logical work with no partial encoding.
Vocabulary, token lengths, control count, merge count and decoded bytes are also
bounded. These counts do not claim allocator, runtime-latency or deployment SLOs.

`to_bytes` and `from_bytes` use the strict `FABBPE01` binary domain. The 120-byte
header contains the exact model profile in big-endian fields. Vocabulary entries
are either tag 0 (control) or tag 1 plus a u32 byte length and content. A u32 merge
count precedes ordered u32 `(left, right, result)` triples. No trailing bytes or
alternate tags are admitted. The expected profile is supplied independently and
checked before vocabulary allocation. Imported data passes the same complete
constructor validation; no file bytes authorize code execution or set authority.

## Verification boundary

Twelve authored Rust tests cover ordinary-byte controls, arbitrary original IDs,
rank and overlap semantics, exhaustive binary inputs, exact budgets, malformed
inventories and merges, decoding controls, strict wire round trips/truncations,
and profile changes. They have not been compiled or executed in this environment:
the required RCH runner is absent. A separate Python heap-versus-rule-scan check
exercises 2,047 inputs and exact partitions, but is not execution evidence for the
Rust implementation. No production qualification or roadmap/bead closure follows.
