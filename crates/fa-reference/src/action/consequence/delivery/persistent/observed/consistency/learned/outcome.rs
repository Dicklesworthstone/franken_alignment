//! Compare recomputed decisions and accounting; never hydrate a saved result.
use super::*;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::monitor::decoder::sampled::host::replay::error_tag;
use crate::action::consequence::oversight::OversightBroker;

pub(super) const MAX_BYTES: usize = 256;

pub(in super::super::super) fn complete_event(event: &mut Event, transition: &Transition,
    broker: &OversightBroker) -> Result<bool, Error>
{
    let capture = match event {
        Event::Consistency(ConsistencyEvent::ForecastLearned(_, _, _, capture)
            | ConsistencyEvent::ForecastLearnedRequest(_, _, _, capture)) => capture,
        _ => return Ok(false),
    };
    let Transition::LearnedConsistencyForecast(result) = transition else { return Err(Error::Binding); };
    let bytes = witness(result, broker)?;
    if capture.outcome.as_ref().is_some_and(|saved| saved.as_ref() != bytes.as_slice()) {
        return Err(Error::Binding);
    }
    capture.outcome = Some(Rc::from(bytes));
    Ok(true)
}

pub(super) fn witness(result: &Result<LearnedForecastReport, Error>, broker: &OversightBroker)
    -> Result<Vec<u8>, Error>
{
    let mut w = Writer::new(MAX_BYTES); w.raw(b"FALCOUT\x01")?;
    match result {
        Err(error) => { w.u8(0)?; w.u8(error_tag(*error))?; }
        Ok(report) => {
            w.u8(1)?;
            w.u8(match report.monitor().outcome() {
                MonitorOutcome::NoAlarm => 0, MonitorOutcome::Alarm => 1,
                MonitorOutcome::AtThreshold => 2, MonitorOutcome::BudgetExhausted => 3,
                MonitorOutcome::Unresolved => 4,
            })?;
            work(&mut w, report.work())?;
            match report.prediction() {
                Err(error) => { w.u8(0)?; w.u8(error_tag(error))?; }
                Ok(prediction) => {
                    w.u8(1)?;
                    w.u32(prediction.forecast().null_numerator())?;
                    w.u32(prediction.forecast().alternative_numerator())?;
                    for value in [prediction.domain(), prediction.generation(), prediction.policy_generation()] {
                        w.u64(value)?;
                    }
                }
            }
        }
    }
    // The same native error code can arise before admission or after partially
    // completed numerical work. Compare the authority-relevant state as well.
    w.u8(u8::from(broker.consistency_coverage_lost()?))?;
    match broker.pending_forecast()? {
        None => w.u8(0)?,
        Some(attempt) => { w.u8(1)?; w.u64(attempt)?; }
    }
    work(&mut w, broker.learned_consistency_work()?)?;
    w.u64(u64::try_from(broker.learned_consistency_retained_source_bytes()?).map_err(|_| Error::Overflow)?)?;
    w.u8(u8::from(broker.learned_consistency_has_unreported_work()?))?;
    Ok(w.finish())
}
fn work(w: &mut Writer, work: LearnedMonitorWork) -> Result<(), Error> {
    for value in [work.encoded_bytes, work.probe_coordinates] {
        w.u64(u64::try_from(value).map_err(|_| Error::Overflow)?)?;
    }
    w.u64(work.reconstruction_products)?;
    for value in [work.materialized_values, work.refinements] {
        w.u64(u64::try_from(value).map_err(|_| Error::Overflow)?)?;
    }
    Ok(())
}
