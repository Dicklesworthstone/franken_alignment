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
