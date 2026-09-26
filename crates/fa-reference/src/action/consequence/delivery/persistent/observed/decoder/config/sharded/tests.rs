//! Original numerical execution, physical shard bytes and durable replay.
//! Synthetic weights are enforcement fixtures, not trained-model evidence.
use super::*;
use super::super::FileDecoderConfig;
use crate::action::consequence::delivery::persistent::observed::storage;
use crate::strict_json;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile, JournalError};
use crate::action::consequence::delivery::persistent::observed::decoder::{DecoderEvent, codec, text::FileTextGenerationCommand};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
}
mod weights {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/tests/weights.rs"));
}
mod split;
use fixtures::{Directory, bytes, command, request, tokenizer};

fn single(head: OutputHead, stored: bool, threshold: f32) -> FileDecoderConfig {
    let base = fixtures::config(threshold, 65);
    FileDecoderConfig::new_with_output_head(base.profile.clone(), weights::tied_weights(stored, false),
        base.monitor.to_vec(), base.sampling.to_vec(), base.stream, base.limits, head).unwrap()
}
fn inputs(single: &FileDecoderConfig) -> FileDecoderShardInputs {
    let (index, sources) = split::split(&single.weights);
    FileDecoderShardInputs { index, sources }
}
fn configured(single: &FileDecoderConfig, inputs: FileDecoderShardInputs) -> Result<FileDecoderConfig, Error> {
    FileDecoderConfig::new_sharded(single.profile.clone(), inputs, single.monitor.to_vec(),
        single.sampling.to_vec(), single.stream, single.limits, single.output_head)
}
fn encode(config: &FileDecoderConfig) -> Vec<u8> {
    let mut w = Writer::new(100_000);
    codec::write(&mut w, &DecoderEvent::Enable(Rc::new(config.clone()))).unwrap();
    w.finish()
}
fn decode(bytes: &[u8]) -> Result<FileDecoderConfig, Error> {
    let mut r = Reader::new(bytes);
    let DecoderEvent::Enable(config) = codec::read(&mut r)? else { return Err(Error::Binding); };
    r.end()?; Ok(config.as_ref().clone())
}

#[test]
fn durable_sharded_inputs_preserve_physical_identity_and_explicit_head_semantics() {
    for (head, stored) in [(OutputHead::Independent, true), (OutputHead::TiedEmbeddings, false),
        (OutputHead::TiedEmbeddings, true)] {
        let single = single(head, stored, 3.0); let supplied = inputs(&single);
        let expected = supplied.index.len() + supplied.sources.values().map(Vec::len).sum::<usize>()
            + single.monitor.len() + single.sampling.len();
        FileDecoderShardInputs::check_labels(single.profile(), &supplied.index,
            supplied.sources.keys().map(String::as_str), head).unwrap();
        let sharded = configured(&single, supplied).unwrap();
        assert!(sharded.is_sharded()); assert!(!single.is_sharded());
        assert_ne!(sharded, single); assert_eq!(sharded.input_bytes(), expected);
        assert_eq!(sharded.output_head(), head);
        assert_eq!(decode(&encode(&sharded)).unwrap(), sharded);
        assert_eq!(encode(&sharded)[0], if head == OutputHead::Independent { 13 } else { 14 });
        assert_eq!(encode(&single)[0], if head == OutputHead::Independent { 0 } else { 12 });
        assert!(sharded.build().is_ok());
    }
}

#[test]
fn durable_sharded_missing_extra_misassigned_and_conflicting_sources_refuse() {
    let base = single(OutputHead::TiedEmbeddings, true, 3.0);
    assert!(configured(&base, inputs(&base)).is_ok());
    for fault in 0..5 {
        let mut supplied = inputs(&base);
        match fault {
            0 => { supplied.sources.remove("left.safetensors"); }
            1 => { supplied.sources.insert("extra.safetensors".into(), vec![0]); }
            2 => {
                let a = supplied.sources.remove("left.safetensors").unwrap();
                let b = supplied.sources.remove("right.safetensors").unwrap();
                supplied.sources.insert("left.safetensors".into(), b);
                supplied.sources.insert("right.safetensors".into(), a);
            }
            3 => { supplied.index = String::from_utf8(supplied.index).unwrap()
                .replace("left.safetensors", "../left.safetensors").into_bytes(); }
            _ => { supplied.sources.get_mut("left.safetensors").unwrap().pop(); }
        }
        assert!(configured(&base, supplied).is_err(), "fault {fault}");
    }
    // Independent heads may differ; tying refuses the same valid finite bytes.
    let raw = weights::tied_weights(true, true);
    let (index, sources) = split::split(&raw);
    let conflict = FileDecoderShardInputs { index, sources };
    assert!(configured(&single(OutputHead::Independent, true, 3.0), conflict.clone()).is_ok());
    assert_eq!(configured(&base, conflict).unwrap_err(), Error::InvalidInput);
    let omitted = inputs(&single(OutputHead::TiedEmbeddings, false, 3.0));
    assert!(configured(&single(OutputHead::Independent, true, 3.0), omitted).is_err());
}

#[test]
fn durable_sharded_encoding_rejects_bad_order_truncation_and_oversized_claims() {
    let base = single(OutputHead::Independent, true, 3.0);
    let config = configured(&base, inputs(&base)).unwrap(); let image = encode(&config);
    for end in [0, 1, 8, 64, 128, image.len() - 1] { assert!(decode(&image[..end]).is_err()); }
    let mut extra = image.clone(); extra.push(0); assert!(decode(&extra).is_err());
    let mut legacy = image; legacy[0] = 0;
    assert!(match decode(&legacy) { Err(_) => true, Ok(decoded) => decoded.build().is_err() });
    let supplied = inputs(&base);
    for duplicate in [false, true] {
        let mut w = Writer::new(100_000);
        w.blob(&supplied.index).unwrap(); w.count(2).unwrap();
        for label in if duplicate { ["left.safetensors", "left.safetensors"] }
            else { ["right.safetensors", "left.safetensors"] } {
            w.blob(label.as_bytes()).unwrap(); w.blob(&supplied.sources[label]).unwrap();
        }
        assert!(ShardSet::read(&mut Reader::new(&w.finish()), base.profile(), base.output_head()).is_err());
    }
    for oversized_count in [false, true] {
        let mut w = Writer::new(100_000); w.blob(&supplied.index).unwrap();
        w.count(if oversized_count { MAX_WEIGHT_SHARDS + 1 } else { 1 }).unwrap();
        if !oversized_count {
            w.blob(b"left.safetensors").unwrap();
            w.count(MAX_WEIGHT_SET_BYTES + 1).unwrap(); // no giant allocation
        }
        assert!(matches!(ShardSet::read(&mut Reader::new(&w.finish()), base.profile(), base.output_head()), Err(Error::Limit)));
    }
    assert_eq!(sum_bytes([7, 9].into_iter(), 16), Ok(16));
    assert_eq!(sum_bytes([7, 10].into_iter(), 16), Err(Error::Limit));
    assert_eq!(sum_bytes([usize::MAX, 1].into_iter(), usize::MAX), Err(Error::Limit));
}

#[test]
fn durable_sharded_recovery_binds_index_bytes_and_rejects_monolithic_substitution() {
    let root = Directory::new(); let base = single(OutputHead::TiedEmbeddings, false, 3.0);
    let supplied = inputs(&base); let original = configured(&base, supplied.clone()).unwrap();
    let mut spaced = supplied; spaced.index.push(b' ');
    let other = configured(&base, spaced).unwrap(); // same actual tensor values
    assert_ne!(original, other);
    let mut host = fixtures::owner(&root, &original);
    let intent = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), intent).unwrap();
    host.advance_decoder_text(host.revision(), 7, 0).unwrap();
    let before = bytes(&host); drop(host);
    for expected in [&other, &base] {
        assert!(matches!(FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
            expected, &tokenizer(false)), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    }
    // Exact original configuration still opens after the failed substitutions.
    let (host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
        &original, &tokenizer(false)).unwrap();
    assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(), 1);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn durable_sharded_partial_recovery_matches_single_file_work_and_output() {
    for (head, stored) in [(OutputHead::Independent, true), (OutputHead::TiedEmbeddings, false)] {
        let root = Directory::new(); let control_root = Directory::new();
        let single = single(head, stored, 3.0); let shards = configured(&single, inputs(&single)).unwrap();
        let mut host = fixtures::owner(&root, &shards); let mut control = fixtures::owner(&control_root, &single);
        for h in [&mut host, &mut control] {
            let intent = command(h, 7, request(b"ab", 2));
            h.begin_decoder_text(h.revision(), intent).unwrap();
            h.advance_decoder_text(h.revision(), 7, 0).unwrap();
        }
        drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
            &shards, &tokenizer(false)).unwrap();
        let before = bytes(&host);
        assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err()); assert_eq!(bytes(&host), before);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let n = host.decoder_inspection().unwrap().numerical;
        host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
        for h in [&mut host, &mut control] {
            for revision in 1..4 { h.advance_decoder_text(h.revision(), 7, revision).unwrap(); }
        }
        let actual = host.decoder_text_generation(7).unwrap(); let expected = control.decoder_text_generation(7).unwrap();
        let a = actual.result().unwrap(); let b = expected.result().unwrap();
        assert_eq!(a.bytes().unwrap(), b"A"); assert_eq!(a.bytes().unwrap(), b.bytes().unwrap());
        assert_eq!(a.generation().tokens(), b.generation().tokens());
        assert_eq!(a.generation().work(), b.generation().work());
        assert_eq!(a.generation().finish(), GenerationFinish::StopToken);
        assert_eq!(a.generation().work().attempted_samples, 2);
        assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(), 4);
        let before = bytes(&host); host.advance_decoder_text(0, 7, 0).unwrap(); assert_eq!(bytes(&host), before);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn durable_sharded_held_generation_cannot_be_rerolled_after_recovery() {
    let root = Directory::new(); let base = single(OutputHead::TiedEmbeddings, false, -3.0);
    let config = configured(&base, inputs(&base)).unwrap(); let mut host = fixtures::owner(&root, &config);
    let intent = command(&host, 7, request(b"ab", 2)); host.generate_decoder_text(host.revision(), intent).unwrap();
    let result = host.decoder_text_generation(7).unwrap();
    assert_eq!(result.result().unwrap().generation().finish(), GenerationFinish::Held);
    assert!(result.result().unwrap().bytes().unwrap().is_empty()); drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
        &config, &tokenizer(false)).unwrap(); host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical; let before = bytes(&host);
    assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(bytes(&host), before); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().generation().work(),
        result.result().unwrap().generation().work());
}
