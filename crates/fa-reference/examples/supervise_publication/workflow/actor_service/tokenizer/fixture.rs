//! Test-only external JSON data; every token ID remains the checkpoint's ID.
pub(in crate::workflow::actor_service) fn raw_byte_level_json(merged: bool) -> String {
    // Independent piecewise ByteLevel spelling, not the importer's inverse table.
    let glyph = |byte: u8| char::from_u32(match byte {
        0..=32 => u32::from(byte) + 256,
        33..=126 | 161..=172 | 174..=255 => u32::from(byte),
        127..=160 => u32::from(byte) + 162,
        173 => 323,
    }).unwrap();
    let quote = |value: &str| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""));
    let mut vocabulary: Vec<_> = (0..=255_u8).rev().map(|byte|
        format!("{}:{byte}", quote(&glyph(byte).to_string()))).collect();
    vocabulary.push("\"<eos>\":256".into());
    if merged { vocabulary.extend(["\"ab\":257".into(), "\"Ã©\":258".into()]); }
    let merges = if merged { "[\"a\",\"b\"],[\"Ã\",\"©\"]" } else { "" };
    format!(concat!(
        "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,\"added_tokens\":[",
        "{{\"id\":256,\"content\":\"<eos>\",\"single_word\":false,\"lstrip\":false,",
        "\"rstrip\":false,\"normalized\":false,\"special\":true}}],",
        "\"normalizer\":null,\"pre_tokenizer\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"post_processor\":null,\"decoder\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"model\":{{\"type\":\"BPE\",\"dropout\":null,\"unk_token\":null,",
        "\"continuing_subword_prefix\":null,\"end_of_word_suffix\":null,",
        "\"fuse_unk\":false,\"byte_fallback\":false,\"ignore_merges\":false,",
        "\"vocab\":{{{}}},\"merges\":[{}]}}}}"), vocabulary.join(","), merges)
}
