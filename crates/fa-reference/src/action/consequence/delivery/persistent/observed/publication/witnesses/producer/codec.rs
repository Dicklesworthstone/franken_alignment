//! One image contains both original input bytes and derived change-window bytes.
use super::*;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};

const DOMAIN: &[u8; 8] = b"FAPPRD01";

pub(super) fn encode(image: &PublicationProducerImage) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new(MAX_PRODUCER_BYTES);
    w.raw(DOMAIN)?;
    for value in [image.profile.source, image.profile.feed, image.profile.clock_domain,
        image.profile.after, image.input_generation] { w.u64(value)?; }
    let scope = image.profile.scope;
    for value in [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority] { w.u64(value)?; }
    match image.floor {
        None => w.u8(0)?,
        Some((revision, cut, epoch)) => {
            w.u8(1)?; w.u64(revision)?; w.u64(cut)?; w.u64(epoch)?;
        }
    }
    w.blob(&image.inputs.to_bytes()?)?;
    w.blob(&image.batch.to_bytes()?)?;
    Ok(w.finish())
}

pub(super) fn decode(bytes: &[u8]) -> Result<PublicationProducerImage, Error> {
    if bytes.len() > MAX_PRODUCER_BYTES { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
    let source = r.u64()?; let feed = r.u64()?; let clock_domain = r.u64()?; let after = r.u64()?;
    let input_generation = r.u64()?;
    let scope = Scope { tenant: r.u64()?, principal: r.u64()?, run: r.u64()?,
        branch: r.u64()?, authority: r.u64()?, purpose: Purpose::Effect };
    let profile = PublicationProducerProfile { source, scope, feed, clock_domain, after };
    profile.check()?;
    let floor = match r.u8()? {
        0 => None,
        1 => Some((r.u64()?, r.u64()?, r.u64()?)),
        _ => return Err(Error::InvalidInput),
    };
    let inputs = FilePublicationInputs::from_bytes(r.blob(MAX_PUBLICATION_PACKET_BYTES)?)?;
    let batch = PublicationFeedBatch::from_bytes(r.blob(MAX_FEED_BYTES)?)?;
    r.end()?;
    let heartbeat = batch.heartbeat();
    if heartbeat.source != profile.feed || heartbeat.clock_domain != profile.clock_domain
        || batch.after() < profile.after { return Err(Error::Binding); }
    if input_generation == 0 || input_generation > heartbeat.generation { return Err(Error::InvalidInput); }
    if input_floor(&inputs).is_some() && input_floor(&inputs) != floor { return Err(Error::Binding); }
    if heartbeat.generation == 1 && (heartbeat.through != profile.after || input_generation != 1) {
        return Err(Error::Binding);
    }
    let image = PublicationProducerImage { profile, input_generation, floor, inputs, batch };
    if encode(&image)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(image)
}
