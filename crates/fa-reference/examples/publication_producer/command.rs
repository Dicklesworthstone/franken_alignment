//! A synchronous consumer of the original producer owner, not a new store.
#[path = "input.rs"]
mod input;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
use input::{Observation, Profile, debug, read_regular};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, MAX_PUBLICATION_PACKET_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{FilePublicationProducer, ProducerPublication, ProducerPublicationKind};
use std::io::Write;
use std::path::Path;

const USAGE: &str = "usage: publication_producer observation EXPECTED_GENERATION OBSERVED_AT_UNIX_MS INPUT_PACKET\n       publication_producer create PROFILE OBSERVATION_JSON\n       publication_producer publish PROFILE OBSERVATION_JSON\n       publication_producer inspect PROFILE";

pub(super) fn run(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let mode = args.first().map(String::as_str).ok_or(USAGE)?;
    let count = match mode { "observation" => 4, "create" | "publish" => 3, "inspect" => 2, _ => return Err(USAGE.into()) };
    if args.len() != count { return Err(USAGE.into()); }
    if mode == "observation" {
        let expected_generation = args[1].parse::<u64>().map_err(debug)?;
        expected_generation.checked_add(1).ok_or("producer generation overflow")?;
        let observed_at = ElapsedTick(args[2].parse::<u64>().map_err(debug)?);
        let bytes = read_regular(Path::new(&args[3]), MAX_PUBLICATION_PACKET_BYTES)?;
        let inputs = FilePublicationInputs::from_bytes(&bytes).map_err(debug)?;
        let observation = Observation { expected_generation, observed_at, inputs };
        out.write_all(&observation.encode()?).map_err(debug)?;
        return out.flush().map_err(debug);
    }
    let profile = Profile::read(Path::new(&args[1]))?;
    if mode == "inspect" {
        let image = FilePublicationProducer::read_image(&profile.directory, profile.identity, profile.minimum_generation).map_err(debug)?;
        // Historical data only. Inspect does not acquire the writer lock, clean
        // staging files, advance time or turn a read into a producer observation.
        writeln!(out, "{{\"status\":\"historical\",\"generation\":\"{}\",\"input_generation\":\"{}\",\"through\":\"{}\",\"retained_after\":\"{}\",\"observed_at_unix_ms\":\"{}\",\"records\":{},\"structured_available\":{},\"opaque_available\":{}}}",
            image.generation(), image.input_generation(), image.batch().heartbeat().through,
            image.batch().after(), image.batch().heartbeat().produced_at.0, image.batch().records().len(),
            image.inputs().structured().is_some(), image.inputs().opaque().is_some()).map_err(debug)?;
        return out.flush().map_err(debug);
    }
    // Decode the whole immutable observation BEFORE opening/cleaning a producer
    // store. It binds original time to exact native bytes, not a mutable file path.
    let observation = Observation::read(Path::new(&args[2]))?;
    let successor = observation.expected_generation.checked_add(1).ok_or("producer generation overflow")?;
    if profile.minimum_generation > successor { return Err("observation precedes the independent generation floor".into()); }
    let report = if mode == "create" {
        if observation.expected_generation != 0 || profile.minimum_generation != 1 {
            return Err("creation requires expected generation zero and minimum generation one".into());
        }
        let (_owner, report) = FilePublicationProducer::create(&profile.directory, profile.identity,
            observation.inputs, observation.observed_at).map_err(debug)?;
        report
    } else {
        // No create-on-missing. An exact retry, including a lost creation receipt,
        // verifies current bytes without another notice or a renewed timestamp.
        let mut owner = FilePublicationProducer::open(&profile.directory, profile.identity, profile.minimum_generation).map_err(debug)?;
        owner.publish(observation.expected_generation, observation.inputs, observation.observed_at).map_err(debug)?
    };
    // Storage has already committed. Output failure must not undo or retry it.
    report_to(report, out)?;
    out.flush().map_err(debug)
}
fn report_to(report: ProducerPublication, out: &mut impl Write) -> Result<(), String> {
    let status = match report.kind { ProducerPublicationKind::Created => "created",
        ProducerPublicationKind::Replaced => "replaced", ProducerPublicationKind::AlreadyCurrent => "already_current" };
    writeln!(out, "{{\"status\":\"{}\",\"generation\":\"{}\",\"input_generation\":\"{}\",\"through\":\"{}\",\"retained_after\":\"{}\",\"encoded_bytes\":{}}}",
        status, report.generation, report.input_generation, report.through, report.retained_after, report.encoded_bytes).map_err(debug)
}
