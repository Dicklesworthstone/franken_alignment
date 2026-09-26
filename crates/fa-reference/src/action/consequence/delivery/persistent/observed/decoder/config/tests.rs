//! Original tensor import, journal encoding and numerical recovery, not saved
//! outputs or caller-supplied monitor decisions. Synthetic model only.
use super::*;
use super::super::{DecoderEvent, codec};
use super::super::super::{FileOversight, FileOversightProfile, JournalError, storage};
use super::super::text::FileTextGenerationCommand;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
}
mod weights;
use fixtures::{Directory, bytes, command, request, tokenizer};
use weights::tied_weights;

fn configured(output_head: OutputHead, stored_head: bool, conflict: bool, threshold: f32)
    -> Result<FileDecoderConfig, Error>
{
    let baseline = fixtures::config(threshold, 65);
    FileDecoderConfig::new_with_output_head(baseline.profile.clone(), tied_weights(stored_head, conflict),
        baseline.monitor.to_vec(), baseline.sampling.to_vec(), baseline.stream, baseline.limits, output_head)
}
fn encode(config: &FileDecoderConfig) -> Vec<u8> {
    let mut writer = Writer::new(100_000);
    codec::write(&mut writer, &DecoderEvent::Enable(Rc::new(config.clone()))).unwrap();
    writer.finish()
}
fn decode(bytes: &[u8]) -> Result<FileDecoderConfig, Error> {
    let mut reader = Reader::new(bytes);
    let DecoderEvent::Enable(config) = codec::read(&mut reader)? else { return Err(Error::Binding); };
    reader.end()?;
    Ok(config.as_ref().clone())
}

#[test]
fn durable_tied_head_is_explicit_and_a_conflicting_physical_head_is_not_ignored() {
    for stored_head in [false, true] {
        let tied = configured(OutputHead::TiedEmbeddings, stored_head, false, 3.0).unwrap();
        assert_eq!(tied.output_head(), OutputHead::TiedEmbeddings);
        let legacy = FileDecoderConfig::new(tied.profile.clone(), tied.weights.to_vec(),
            tied.monitor.to_vec(), tied.sampling.to_vec(), tied.stream, tied.limits);
        assert_eq!(legacy.is_ok(), stored_head);
        if let Ok(legacy) = legacy {
            assert_eq!(legacy.output_head(), OutputHead::Independent);
            assert_ne!(legacy, tied); // identical bytes/numbers, different contract
            assert_eq!(legacy.input_bytes(), tied.input_bytes());
        }
    }
    // Both stored matrices are valid finite weights; only the explicit equality
    // requirement rejects the signed-zero conflict in tied mode.
    assert!(configured(OutputHead::Independent, true, true, 3.0).is_ok());
    assert_eq!(configured(OutputHead::TiedEmbeddings, true, true, 3.0).unwrap_err(), Error::InvalidInput);
    assert_eq!(configured(OutputHead::Independent, false, false, 3.0).unwrap_err(), Error::InvalidInput);
}

#[test]
fn durable_tied_head_preserves_legacy_encoding_and_has_a_distinct_roundtrip_tag() {
    let independent = configured(OutputHead::Independent, true, false, 3.0).unwrap();
    let tied = configured(OutputHead::TiedEmbeddings, true, false, 3.0).unwrap();
    // Independently spelled legacy field order. Do not generate this expected
    // layout through the configuration writer under test.
    let mut legacy = Writer::new(100_000);
    legacy.u8(0).unwrap();
    let p = independent.profile(); let id = p.identity(); let s = p.shape();
    for value in [id.tenant, id.model, id.model_generation, id.tokenizer_generation,
        id.profile_generation, p.epsilon().to_bits(), p.theta().to_bits(), independent.stream()] {
        legacy.u64(value).unwrap();
    }
    for value in [s.vocabulary, s.hidden, s.intermediate, s.layers, s.query_heads,
        s.cache_heads, s.context, independent.limits.token_ids, independent.limits.score_words] {
        legacy.count(value).unwrap();
    }
    legacy.blob(&independent.weights).unwrap(); legacy.blob(&independent.monitor).unwrap();
    legacy.blob(&independent.sampling).unwrap();
    let old_bytes = legacy.finish();
    assert_eq!(encode(&independent), old_bytes);
    let tied_bytes = encode(&tied);
    assert_eq!(tied_bytes[0], 12); assert_eq!(old_bytes[0], 0);
    assert_eq!(&tied_bytes[1..], &old_bytes[1..]);
    assert_eq!(decode(&old_bytes).unwrap(), independent);
    assert_eq!(decode(&tied_bytes).unwrap(), tied);
    for c in [independent, tied, configured(OutputHead::TiedEmbeddings, false, false, 3.0).unwrap()] {
        let decoded = decode(&encode(&c)).unwrap();
        assert_eq!(decoded, c); assert!(decoded.build().is_ok());
    }
}

#[test]
fn durable_tied_head_bad_framing_and_retagged_legacy_data_cannot_become_a_working_model() {
    let tied = configured(OutputHead::TiedEmbeddings, false, false, 3.0).unwrap();
    let complete = encode(&tied);
    for end in [0, 1, 8, 64, 128, complete.len() - 1] {
        assert!(decode(&complete[..end]).is_err());
    }
    let mut trailing = complete.clone(); trailing.push(0);
    assert!(decode(&trailing).is_err());
    let mut unknown = complete.clone(); unknown[0] = 255;
    assert_eq!(decode(&unknown).unwrap_err(), Error::InvalidInput);
    let mut retagged = complete; retagged[0] = 0;
    let old_contract = decode(&retagged).unwrap();
    assert_eq!(old_contract.output_head(), OutputHead::Independent);
    assert!(matches!(old_contract.build(), Err(Error::InvalidInput)));
    // Retagging a genuine independent archive cannot discard its conflicting
    // stored head; semantic build is still mandatory after bounded decoding.
    let independent = fixtures::config(3.0, 65);
    let mut retagged = encode(&independent); retagged[0] = 12;
    assert!(matches!(decode(&retagged).unwrap().build(), Err(Error::InvalidInput)));
}

#[test]
fn durable_tied_head_recovery_checks_mode_even_when_model_bytes_and_numbers_match() {
    for original_mode in [OutputHead::Independent, OutputHead::TiedEmbeddings] {
        let root = Directory::new();
        let config = configured(original_mode, true, false, 3.0).unwrap();
        let other_mode = if original_mode == OutputHead::Independent {
            OutputHead::TiedEmbeddings
        } else { OutputHead::Independent };
        let other = configured(other_mode, true, false, 3.0).unwrap();
        let mut host = fixtures::owner(&root, &config);
        let intent = command(&host, 7, request(b"ab", 2));
        host.generate_decoder_text(host.revision(), intent).unwrap();
        assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"A");
        let before = bytes(&host); drop(host);
        assert!(matches!(FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
            &other, &tokenizer(false)), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
        let (host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
            &config, &tokenizer(false)).unwrap();
        assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"A");
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn durable_tied_head_partial_recovery_matches_independent_numerics_and_conserved_work() {
    let root = Directory::new(); let control_root = Directory::new();
    let tied = configured(OutputHead::TiedEmbeddings, false, false, 3.0).unwrap();
    let independent = configured(OutputHead::Independent, true, false, 3.0).unwrap();
    let mut host = fixtures::owner(&root, &tied);
    let mut control = fixtures::owner(&control_root, &independent);
    for owner in [&mut host, &mut control] {
        let intent = command(owner, 7, request(b"ab", 2));
        owner.begin_decoder_text(owner.revision(), intent).unwrap();
        owner.advance_decoder_text(owner.revision(), 7, 0).unwrap();
    }
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
        &tied, &tokenizer(false)).unwrap();
    assert!(host.decoder_inspection().unwrap().paused);
    let frozen = bytes(&host);
    assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
    assert_eq!(bytes(&host), frozen);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    for owner in [&mut host, &mut control] {
        for revision in 1..4 { owner.advance_decoder_text(owner.revision(), 7, revision).unwrap(); }
    }
    let actual = host.decoder_text_generation(7).unwrap();
    let expected = control.decoder_text_generation(7).unwrap();
    let a = actual.result().unwrap(); let b = expected.result().unwrap();
    assert_eq!(a.bytes().unwrap(), b"A"); assert_eq!(a.bytes().unwrap(), b.bytes().unwrap());
    assert_eq!(a.generation().finish(), GenerationFinish::StopToken);
    assert_eq!(a.generation().tokens(), b.generation().tokens());
    assert_eq!(a.generation().work(), b.generation().work());
    assert_eq!(a.generation().work().attempted_samples, 2);
    assert_eq!(a.generation().end_position(), 4);
    assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(), 4);
    let frozen = bytes(&host);
    assert_eq!(host.advance_decoder_text(0, 7, 0).unwrap().bytes().unwrap(), b"A");
    assert_eq!(bytes(&host), frozen); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn durable_tied_head_does_not_bypass_monitor_holds_or_restart_them_on_recovery() {
    let root = Directory::new(); let tied = configured(OutputHead::TiedEmbeddings, false, false, -3.0).unwrap();
    let mut host = fixtures::owner(&root, &tied);
    let intent = command(&host, 7, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), intent).unwrap();
    let before = host.decoder_text_generation(7).unwrap();
    assert_eq!(before.result().unwrap().generation().finish(), GenerationFinish::Held);
    assert!(before.result().unwrap().bytes().unwrap().is_empty());
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
        &tied, &tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let frozen = bytes(&host);
    assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(bytes(&host), frozen);
    assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().generation().work(),
        before.result().unwrap().generation().work());
    assert_eq!(host.inspect().executions, 0);
}
