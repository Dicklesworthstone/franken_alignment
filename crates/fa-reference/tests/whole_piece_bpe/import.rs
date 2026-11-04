use super::fixture::{CONTROL, document, profile};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, MergePolicy, PreTokenization,
};
use fa_reference::Error;
use std::io::{self, Cursor, Read};

#[test]
fn json_merge_flag_must_match_the_independent_caller_selection_in_both_directions() {
    for mode in [PreTokenization::RawBytes, PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        for whole in [false, true] {
            let json = document(mode, whole, false, false);
            let policy = if whole { MergePolicy::WholePieceFirst } else { MergePolicy::RankedOnly };
            let wrong = if whole { MergePolicy::RankedOnly } else { MergePolicy::WholePieceFirst };
            let imported = ByteBpe::from_huggingface_json_with_merge_policy(&profile(), json.as_bytes(),
                json.len(), mode, policy).unwrap();
            let ids: &[u32] = if whole { &[258] } else { &[256, 116] };
            assert_eq!(imported.encode(b"abc", Default::default()).unwrap().tokens(), ids);
            assert_eq!(ByteBpe::from_huggingface_json_with_merge_policy(&profile(), json.as_bytes(),
                json.len(), mode, wrong).unwrap_err(), Error::Binding);
            assert_eq!(ByteBpe::from_huggingface_json_with_pre_tokenization(&profile(), json.as_bytes(),
                json.len(), mode).is_ok(), !whole);
        }
    }
}

#[test]
fn explicit_true_import_admits_whole_only_content_and_keeps_other_model_flags_strict() {
    let mode = PreTokenization::Llama3Unicode17;
    let json = document(mode, true, true, false);
    let imported = ByteBpe::from_huggingface_json_with_merge_policy(&profile(), json.as_bytes(),
        json.len(), mode, MergePolicy::WholePieceFirst).unwrap();
    assert_eq!(imported.encode(b"xyz", Default::default()).unwrap().tokens(), &[258]);
    let restored = ByteBpe::from_bytes(&profile(), &imported.to_bytes().unwrap()).unwrap();
    assert_eq!(restored.encode(b"xyz", Default::default()).unwrap().tokens(), &[258]);
    for (from, to) in [("\"ignore_merges\":true", "\"ignore_merges\":null"),
        ("\"ignore_merges\":true,", ""),
        ("\"fuse_unk\":false", "\"fuse_unk\":true"),
        ("\"byte_fallback\":false", "\"byte_fallback\":true"),
        ("\"dropout\":null", "\"dropout\":0.1")]
    {
        assert!(json.contains(from));
        let bad = json.replacen(from, to, 1);
        assert!(ByteBpe::from_huggingface_json_with_merge_policy(&profile(), bad.as_bytes(),
            bad.len(), mode, MergePolicy::WholePieceFirst).is_err());
    }
    let false_json = json.replace("\"ignore_merges\":true", "\"ignore_merges\":false");
    assert_eq!(ByteBpe::from_huggingface_json_with_pre_tokenization(&profile(), false_json.as_bytes(),
        false_json.len(), mode).unwrap_err(), Error::Incomplete);
}

#[test]
fn whole_piece_bos_import_requires_both_its_policy_and_original_fixed_template() {
    let json = document(PreTokenization::Llama3Unicode17, true, true, true);
    let imported = ByteBpe::from_huggingface_llama3_bos_json_with_merge_policy(&profile(), json.as_bytes(),
        json.len(), MergePolicy::WholePieceFirst).unwrap();
    assert_eq!(imported.encode(b"xyz<q>", Default::default()).unwrap().tokens(), &[CONTROL, 258, CONTROL]);
    assert_eq!(ByteBpe::from_huggingface_llama3_bos_json(&profile(), json.as_bytes(),
        json.len()).unwrap_err(), Error::Binding);
    assert_eq!(ByteBpe::from_huggingface_json_with_merge_policy(&profile(), json.as_bytes(),
        json.len(), PreTokenization::Llama3Unicode17, MergePolicy::WholePieceFirst).unwrap_err(), Error::Binding);
    let bad = json.replace("\"ids\":[260]", "\"ids\":[259]");
    assert_eq!(ByteBpe::from_huggingface_llama3_bos_json_with_merge_policy(&profile(), bad.as_bytes(),
        bad.len(), MergePolicy::WholePieceFirst).unwrap_err(), Error::Binding);
}

struct Fragmented { reader: Cursor<Vec<u8>>, chunk: usize }
impl Read for Fragmented {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let maximum = output.len().min(self.chunk);
        self.reader.read(&mut output[..maximum])
    }
}

#[test]
fn whole_piece_reader_requires_complete_eof_and_preserves_selected_semantics() {
    let json = document(PreTokenization::Llama3Unicode17, true, true, false);
    for chunk in [1, 7, 8192] {
        let mut source = Fragmented { reader: Cursor::new(json.as_bytes().to_vec()), chunk };
        let tokenizer = ByteBpe::read_huggingface_json_with_merge_policy(&profile(), &mut source,
            json.len(), PreTokenization::Llama3Unicode17, MergePolicy::WholePieceFirst).unwrap();
        assert_eq!(tokenizer.encode(b"xyz", Default::default()).unwrap().tokens(), &[258]);
        assert_eq!(source.reader.position(), json.len() as u64);
    }
    let mut tail = json.as_bytes().to_vec(); tail.push(b'!');
    let error = ByteBpe::read_huggingface_json_with_merge_policy(&profile(), &mut Cursor::new(tail),
        json.len(), PreTokenization::Llama3Unicode17, MergePolicy::WholePieceFirst).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    let error = ByteBpe::read_huggingface_json_with_merge_policy(&profile(), &mut Cursor::new(json.as_bytes()),
        json.len(), PreTokenization::Llama3Unicode17, MergePolicy::RankedOnly).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
