//! A liability disposition cannot resume or discard real pending computation.
use super::*;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"
    ));
    pub(super) fn stopped() -> FileDecoderConfig {
        configured(3.0, 65, Some(256))
    }
}

#[test]
fn pending_generation_survives_disposition_with_its_original_pause_cursor_and_work() {
    for paused in [false, true] {
        let root = Directory::new();
        let p = fixtures::host_profile();
        let decoder = fixtures::stopped();
        let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
        host.enable_decoder(host.revision(), decoder.clone())
            .unwrap();
        host.enable_decoder_tokenizer(host.revision(), fixtures::tokenizer(false))
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let first = fixtures::command(&host, 6, fixtures::request(b"ab", 2));
        host.generate_decoder_text(host.revision(), first).unwrap();
        let action = proposed(&mut host, 1);
        reviewed_dispatch(&mut host, &reviewer, 1, &action);
        host.reconcile(host.revision(), 1).unwrap();
        let next = fixtures::command(&host, 7, fixtures::request(b"ab", 2));
        host.begin_decoder_text(host.revision(), next).unwrap();
        host.advance_decoder_text(host.revision(), 7, 0).unwrap();
        if paused {
            drop(host);
            host = FileOversight::open_with_text_decoder(
                root.store(),
                p,
                &decoder,
                &fixtures::tokenizer(false),
            )
            .unwrap()
            .0;
            assert!(!host.clock_ready());
        }
        let numerical = host.decoder_inspection().unwrap();
        assert_eq!(numerical.paused, paused);
        let pending = host.pending_decoder_text().unwrap().unwrap();
        let progress = host.decoder_text_progress(7).unwrap();
        let charge = host.inspect().control.ledger.charged;
        let instruction = request(&host, 71, 1);
        host.abandon_unknown(host.revision(), instruction).unwrap();
        let after = host.decoder_inspection().unwrap();
        assert_eq!(after.paused, numerical.paused);
        assert_eq!(after.numerical, numerical.numerical);
        assert_eq!(host.pending_decoder_text().unwrap(), Some(pending));
        let after = host.decoder_text_progress(7).unwrap();
        assert_eq!(after.command(), progress.command());
        assert_eq!(after.generation_revision(), progress.generation_revision());
        assert_eq!(after.bytes(), progress.bytes());
        let original = progress.numerical().partial().unwrap();
        let retained = after.numerical().partial().unwrap();
        assert_eq!(retained.position(), original.position());
        assert_eq!(
            retained.reviewed_prompt_tokens(),
            original.reviewed_prompt_tokens()
        );
        assert_eq!(retained.work(), original.work());
        assert_eq!(retained.finish(), original.finish());
        assert_eq!(host.inspect().control.ledger.charged, charge);
        assert_eq!(
            host.inspect().control.ledger.stages[&1],
            ActionState::IrrecoverablyUnknown
        );
        assert_eq!(host.inspect().executions, 0);
        if paused {
            let frozen = bytes(&host);
            assert!(
                host.advance_decoder_text(host.revision(), 7, progress.generation_revision())
                    .is_err()
            );
            assert_eq!(bytes(&host), frozen);
        } else {
            // The matching original numerical continuation is still usable;
            // disposition did not substitute a blanket refusal for progress.
            host.advance_decoder_text(host.revision(), 7, progress.generation_revision())
                .unwrap();
            assert_eq!(
                host.decoder_text_progress(7).unwrap().generation_revision(),
                progress.generation_revision() + 1
            );
        }
    }
}
