//! Exact whole-pretoken vocabulary matching before the original ranked BPE pass.

use super::{ByteBpe, DecoderProfile, Error, Merge, Node, PreTokenization, TokenBytes,
    content, special::SpecialTokens};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Immutable BPE interpretation selected independently of imported JSON.
/// The default keeps the original strict merge-reachability contract. The opt-in
/// policy implements `ignore_merges=true`: an exact whole ordinary piece uses
/// its original vocabulary ID, while every miss uses the original ranked BPE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergePolicy {
    RankedOnly,
    WholePieceFirst,
}

impl ByteBpe {
    /// Freeze the selected merge policy with the existing grammar and controls.
    /// WholePieceFirst also admits content reachable only by an exact whole-piece
    /// match. It does not admit unknown bytes, greedy substring matching, new
    /// special tokens, relaxed merge ordering or a mutable tokenizer policy.
    pub fn new_with_merge_policy(profile: DecoderProfile, vocabulary: Vec<TokenBytes>,
        merges: Vec<Merge>, names: BTreeMap<u32, Vec<u8>>, pre_tokenization: PreTokenization,
        merge_policy: MergePolicy) -> Result<Self, Error>
    {
        let special = SpecialTokens::new(&vocabulary, names)?;
        let mut tokenizer = Self::new_inner(profile, vocabulary, merges, merge_policy)?;
        let data = Rc::get_mut(&mut tokenizer.0).expect("fresh unshared tokenizer");
        data.special = special;
        data.pre_tokenization = pre_tokenization;
        Ok(tokenizer)
    }

    pub(super) fn bind_whole_pieces(&self, source: &[u8], nodes: &mut [Node])
        -> Result<(), Error>
    {
        if self.merge_policy() == MergePolicy::RankedOnly { return Ok(()); }
        let mut start = 0;
        while start < nodes.len() {
            // Literal controls were selected first. Neither direct lookup nor
            // the subsequent merge pass can consume through those controls.
            if self.is_control(nodes[start].token)? {
                start = nodes[start].end;
                continue;
            }
            let mut end = nodes[start].end;
            while end < nodes.len() && !nodes[end].boundary_before
                && !self.is_control(nodes[end].token)?
            {
                end = nodes[end].end;
            }
            if end - start <= self.max_content_bytes() {
                let piece = source.get(start..end).ok_or(Error::Binding)?;
                if let Ok(index) = self.0.whole_piece_tokens.binary_search_by(|id|
                    content(&self.0.vocabulary, *id).expect("content index").cmp(piece))
                {
                    nodes[start].token = self.0.whole_piece_tokens[index];
                    nodes[start].end = end;
                    nodes[start].next = (end < nodes.len()).then_some(end);
                    for node in &mut nodes[start + 1..end] { node.live = false; }
                    if end < nodes.len() { nodes[end].previous = Some(start); }
                }
            }
            start = end;
        }
        Ok(())
    }
}
