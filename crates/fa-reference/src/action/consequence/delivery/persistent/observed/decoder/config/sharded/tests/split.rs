// Split the existing SYNTHETIC F32 fixture without changing any tensor bytes.
// Shared by durable and executable tests; no production archive is rewritten.
use super::strict_json;
use std::collections::BTreeMap;

pub(super) fn split(bytes: &[u8]) -> (Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let root = strict_json::parse(&bytes[8..8 + length], strict_json::Limits {
        max_bytes: 100_000, max_depth: 4, max_items: 4096, max_string_bytes: 4096,
    }).unwrap();
    let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut payloads: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    let mut assignments = Vec::new();
    let mut total = 0;
    for (name, descriptor) in root.as_object().unwrap() {
        assert_eq!(descriptor.get("dtype").unwrap().as_str(), Some("F32"));
        let shape: Vec<_> = descriptor.get("shape").unwrap().as_array().unwrap().iter()
            .map(|n| n.as_u64().unwrap()).collect();
        let offsets = descriptor.get("data_offsets").unwrap().as_array().unwrap();
        let start = offsets[0].as_u64().unwrap() as usize;
        let end = offsets[1].as_u64().unwrap() as usize;
        // A stored head and its embedding source occupy DIFFERENT files.
        let label = if name == "lm_head.weight" || name.contains("mlp.") {
            "right.safetensors"
        } else { "left.safetensors" };
        let data = payloads.entry(label).or_default();
        let offset = data.len();
        data.extend_from_slice(&bytes[8 + length + start..8 + length + end]);
        total += end - start;
        groups.entry(label).or_default().push(format!(
            r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{offset},{}]}}"#, data.len()));
        assignments.push(format!(r#""{name}":"{label}""#));
    }
    let sources = groups.into_iter().map(|(label, fields)| {
        let mut header = format!("{{{}}}", fields.join(","));
        while header.len() % 8 != 0 { header.push(' '); }
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(header.as_bytes());
        bytes.extend(payloads.remove(label).unwrap());
        (label.to_owned(), bytes)
    }).collect();
    (format!(r#"{{"metadata":{{"total_size":{total}}},"weight_map":{{{}}}}}"#,
        assignments.join(",")).into_bytes(), sources)
}
