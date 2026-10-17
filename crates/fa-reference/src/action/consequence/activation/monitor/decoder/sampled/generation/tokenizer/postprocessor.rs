//! One explicit prefix operation, not an evaluator for arbitrary templates.

use super::{ByteBpe, DecoderProfile, Error, Merge, PreTokenization, TokenBytes};
use std::collections::BTreeMap;
use std::rc::Rc;

impl ByteBpe {
    /// Construct the exact Llama-3/Unicode-17 input profile with one automatic
    /// beginning-of-sequence control. `bos` must name an existing registered
    /// control ID; it is never inferred from vocabulary order or token spelling.
    ///
    /// Every completed encoding starts with this ID and a synthetic 0..0 span,
    /// including empty input. Literal occurrences retain their own original spans
    /// and IDs after the prefix, without deduplication. All source, BPE work and
    /// output bounds still apply. No pair-input or general template API is added.
    pub fn new_with_bos_token(profile: DecoderProfile, vocabulary: Vec<TokenBytes>,
        merges: Vec<Merge>, names: BTreeMap<u32, Vec<u8>>, bos: u32)
        -> Result<Self, Error>
    {
        if !matches!(vocabulary.get(bos as usize), Some(TokenBytes::Control))
            || !names.contains_key(&bos)
        { return Err(Error::Binding); }
        let mut tokenizer = Self::new_with_pre_tokenization(profile, vocabulary,
            merges, names, PreTokenization::Llama3Unicode17)?;
        Rc::get_mut(&mut tokenizer.0).expect("fresh unshared tokenizer").bos_token = Some(bos);
        Ok(tokenizer)
    }
}
