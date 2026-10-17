//! External pipeline fixtures; patterns come from the upstream grammars, not
//! from the implementation's pattern accessor or native archive serializer.
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::PreTokenization;

pub fn select(raw_json: &str, mode: PreTokenization) -> String {
    let byte_level = r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#;
    let from = format!("\"pre_tokenizer\":{byte_level}");
    assert!(raw_json.contains(&from));
    let pipeline = match mode {
        PreTokenization::RawBytes => byte_level.to_owned(),
        PreTokenization::Gpt2Unicode17 => byte_level.replace("\"use_regex\":false", "\"use_regex\":true"),
        PreTokenization::Llama3Unicode17 => {
            let pattern = r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+";
            let quoted = pattern.replace('\\', "\\\\");
            format!(r#"{{"type":"Sequence","pretokenizers":[{{"type":"Split","pattern":{{"Regex":"{quoted}"}},"behavior":"Isolated","invert":false}},{byte_level}]}}"#)
        }
    };
    raw_json.replacen(&from, &format!("\"pre_tokenizer\":{pipeline}"), 1)
}


/// The actual Llama converter's ByteLevel/TemplateProcessing serialization.
/// The caller names an already registered control; the importer must validate it.
#[allow(dead_code)] // This shared fixture is included by several independent test targets.
pub fn with_llama3_bos(raw_json: &str, name: &str, token: u32) -> String {
    let selected = select(raw_json, PreTokenization::Llama3Unicode17);
    let name = format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""));
    let processor = concat!(
        r#"{"type":"Sequence","processors":["#,
        r#"{"type":"ByteLevel","add_prefix_space":true,"trim_offsets":false,"use_regex":true},"#,
        r#"{"type":"TemplateProcessing","single":["#,
        r#"{"SpecialToken":{"id":BOS_NAME,"type_id":0}},{"Sequence":{"id":"A","type_id":0}}],"#,
        r#""pair":[{"SpecialToken":{"id":BOS_NAME,"type_id":0}},{"Sequence":{"id":"A","type_id":0}},"#,
        r#"{"SpecialToken":{"id":BOS_NAME,"type_id":1}},{"Sequence":{"id":"B","type_id":1}}],"#,
        r#""special_tokens":{BOS_NAME:{"id":BOS_NAME,"ids":[BOS_ID],"tokens":[BOS_NAME]}}}]}"#,
    ).replace("BOS_NAME", &name).replace("BOS_ID", &token.to_string());
    assert!(selected.contains("\"post_processor\":null"));
    selected.replacen("\"post_processor\":null", &format!("\"post_processor\":{processor}"), 1)
}
