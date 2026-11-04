//! Public original-token controls for explicit whole-piece-first BPE. The same
//! fixed vocabulary distinguishes this policy from ranked merging causally.
#[path = "whole_piece_bpe/fixture.rs"]
mod fixture;
#[path = "whole_piece_bpe/import.rs"]
mod import;
#[path = "whole_piece_bpe/wire.rs"]
mod wire;

use fixture::{CONTROL, merges, names, profile, tokenizer, vocabulary};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, MergePolicy, PreTokenization, TokenizationBudget, TokenizationProgress,
    TokenizationWork,
};
use fa_reference::Error;
use std::collections::BTreeMap;

#[test]
fn whole_piece_lookup_changes_original_ids_without_changing_rank_or_vocabulary() {
    for mode in [PreTokenization::RawBytes, PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let ranked = tokenizer(mode, MergePolicy::RankedOnly);
        let whole = tokenizer(mode, MergePolicy::WholePieceFirst);
        let old = ranked.encode(b"abc", TokenizationBudget::default()).unwrap();
        let exact = whole.encode(b"abc", TokenizationBudget::default()).unwrap();
        assert_eq!(old.tokens(), &[256, 116]);
        assert_eq!(old.spans(), &[0..2, 2..3]);
        assert_eq!(exact.tokens(), &[258]);
        assert_eq!(exact.spans(), &[0..3]);
        assert_eq!(exact.source(), b"abc");
        assert_eq!(whole.decode(exact.tokens(), 3).unwrap(), b"abc");
        assert_eq!(exact.work(), TokenizationWork { input_bytes: 3, ..TokenizationWork::default() });
        assert!(old.work().merges > 0);
    }
}

#[test]
fn an_unknown_whole_piece_uses_ranked_bpe_without_greedy_vocabulary_substrings() {
    for mode in [PreTokenization::RawBytes, PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let ranked = tokenizer(mode, MergePolicy::RankedOnly);
        let whole = tokenizer(mode, MergePolicy::WholePieceFirst);
        // The prefix abc has a vocabulary entry, but abcd has none. Selecting
        // abc+d would be a different algorithm and must not happen here.
        let old = ranked.encode(b"abcd", TokenizationBudget::default()).unwrap();
        let result = whole.encode(b"abcd", TokenizationBudget::default()).unwrap();
        assert_eq!(result.tokens(), &[256, 116, 117]);
        assert_eq!(result.tokens(), old.tokens());
        assert_eq!(result.spans(), old.spans());
        assert_eq!(result.work(), old.work());
    }
}

#[test]
fn whole_piece_only_content_is_reachable_only_under_the_explicit_policy() {
    let construct = |policy| ByteBpe::new_with_merge_policy(profile(), vocabulary(true),
        merges(true), names(), PreTokenization::Llama3Unicode17, policy);
    assert_eq!(construct(MergePolicy::RankedOnly).unwrap_err(), Error::Incomplete);
    let whole = construct(MergePolicy::WholePieceFirst).unwrap();
    assert_eq!(whole.encode(b"xyz", TokenizationBudget::default()).unwrap().tokens(), &[258]);
    assert_eq!(whole.encode(b"xyzq", TokenizationBudget::default()).unwrap().tokens(), &[137, 138, 139, 130]);
    // Earlier-defined operands remain mandatory even when exact hits exist.
    let mut unordered = merges(false);
    unordered.swap(1, 2);
    assert_eq!(ByteBpe::new_with_merge_policy(profile(), vocabulary(false), unordered,
        names(), PreTokenization::RawBytes, MergePolicy::WholePieceFirst).unwrap_err(), Error::Binding);
}

#[test]
fn direct_matches_obey_unicode_and_named_control_boundaries() {
    let raw = tokenizer(PreTokenization::RawBytes, MergePolicy::WholePieceFirst);
    assert_eq!(raw.encode(b"a1", TokenizationBudget::default()).unwrap().tokens(), &[259]);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let whole = tokenizer(mode, MergePolicy::WholePieceFirst);
        let result = whole.encode(b"a1", TokenizationBudget::default()).unwrap();
        assert_eq!(result.tokens(), &[114, 66]);
        assert_eq!(result.spans(), &[0..1, 1..2]);
        assert_eq!(result.work().pair_lookups, 0);
    }
    for mode in [PreTokenization::RawBytes, PreTokenization::Llama3Unicode17] {
        let whole = tokenizer(mode, MergePolicy::WholePieceFirst);
        let result = whole.encode(b"abc<q>abc", TokenizationBudget::default()).unwrap();
        assert_eq!(result.tokens(), &[258, CONTROL, 258]);
        assert_eq!(result.spans(), &[0..3, 3..6, 6..9]);
        assert_eq!(result.source(), b"abc<q>abc");
    }
    // An explicitly named control takes priority even over a whole content hit.
    let named = ByteBpe::new_with_merge_policy(profile(), vocabulary(false), merges(false),
        BTreeMap::from([(CONTROL, b"abc".to_vec())]), PreTokenization::RawBytes,
        MergePolicy::WholePieceFirst).unwrap();
    assert_eq!(named.encode(b"abc", TokenizationBudget::default()).unwrap().tokens(), &[CONTROL]);
}

#[test]
fn whole_piece_bos_preserves_empty_and_literal_control_provenance() {
    let whole = ByteBpe::new_with_bos_token_and_merge_policy(profile(), vocabulary(false),
        merges(false), names(), CONTROL, MergePolicy::WholePieceFirst).unwrap();
    let result = whole.encode(b"abc<q>abc", TokenizationBudget::default()).unwrap();
    assert_eq!(result.tokens(), &[CONTROL, 258, CONTROL, 258]);
    assert_eq!(result.spans(), &[0..0, 0..3, 3..6, 6..9]);
    let empty = whole.encode(b"", TokenizationBudget::default()).unwrap();
    assert_eq!(empty.tokens(), &[CONTROL]);
    assert_eq!(empty.spans(), &[0..0]);
    assert_eq!(empty.work(), TokenizationWork::default());
}

#[test]
fn direct_lookup_retains_input_charge_without_inventing_bpe_work() {
    let whole = tokenizer(PreTokenization::Llama3Unicode17, MergePolicy::WholePieceFirst);
    let exact = TokenizationBudget { input_bytes: 3, pair_lookups: 0, heap_pops: 0 };
    let result = whole.encode(b"abc", exact).unwrap();
    assert_eq!(result.tokens(), &[258]);
    assert_eq!(result.work(), TokenizationWork { input_bytes: 3, ..TokenizationWork::default() });
    let failure = whole.begin_encode(b"abc", TokenizationBudget { input_bytes: 2, ..exact }).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work, TokenizationWork::default());
    let invalid = whole.begin_encode(b"ab\xff", exact).unwrap_err();
    assert_eq!(invalid.error, Error::InvalidInput);
    assert_eq!(invalid.work.input_bytes, 3);
    let ranked = tokenizer(PreTokenization::Llama3Unicode17, MergePolicy::RankedOnly);
    assert_eq!(ranked.encode(b"abc", exact).unwrap_err().error, Error::Limit);
}

#[test]
fn cooperative_cursor_keeps_exact_ids_work_cancellation_and_sticky_failure() {
    let whole = tokenizer(PreTokenization::Llama3Unicode17, MergePolicy::WholePieceFirst);
    for source in [b"".as_slice(), b"abc", b"abcd", b"abc<q>abcd", b"a1"] {
        let mut cursor = whole.begin_encode(source, TokenizationBudget::default()).unwrap();
        while cursor.advance().unwrap() != TokenizationProgress::Complete {}
        let spent = cursor.work();
        assert_eq!(cursor.advance().unwrap(), TokenizationProgress::Complete);
        assert_eq!(cursor.work(), spent);
        let result = cursor.finish().unwrap();
        let synchronous = whole.encode(source, TokenizationBudget::default()).unwrap();
        assert_eq!(result.tokens(), synchronous.tokens());
        assert_eq!(result.spans(), synchronous.spans());
        assert_eq!(result.work(), synchronous.work());
    }
    let budget = TokenizationBudget { input_bytes: 4, pair_lookups: 0, heap_pops: 0 };
    let mut cursor = whole.begin_encode(b"abcd", budget).unwrap();
    let failure = cursor.advance().unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work, TokenizationWork { input_bytes: 4, ..TokenizationWork::default() });
    assert_eq!(cursor.advance(), Err(failure));
    assert_eq!(cursor.finish().unwrap_err(), failure);
    assert_eq!(whole.encode(b"abcd", budget).unwrap_err(), failure);
    let cursor = whole.begin_encode(b"abc", TokenizationBudget::default()).unwrap();
    let spent = cursor.work();
    assert_eq!(cursor.cancel(), spent);
    assert_eq!(whole.begin_encode(b"abc", TokenizationBudget::default()).unwrap()
        .finish().unwrap_err().error, Error::Incomplete);
}
