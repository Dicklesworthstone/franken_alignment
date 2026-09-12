//! Export only a completely evaluated roster into the existing monitor schema.
//! Configuration is data, not authenticated qualification or live policy approval.

pub mod files;

use super::decoder::DecoderCampaign;
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use crate::Error;
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerMonitorSettings {
    pub levels: Vec<u8>,
    pub budget: RefinementBudget,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorExportSettings {
    pub generation: u64,
    pub budget: RefinementBudget,
    pub layers: BTreeMap<u64, LayerMonitorSettings>,
}

impl DecoderCampaign {
    /// Every required layer must have passed untouched evaluation. Thresholds,
    /// coefficients and probe identities come from those exact retained results,
    /// never from export arguments. JSON decimals round-trip the emitted f32s.
    pub fn monitor_json(&self, settings: &MonitorExportSettings, max_bytes: usize) -> Result<Vec<u8>, Error> {
        if settings.generation == 0 { return Err(Error::InvalidInput); }
        if max_bytes > MAX_MONITOR_CONFIG_BYTES { return Err(Error::Limit); }
        if !settings.layers.keys().eq(self.layers().keys()) { return Err(Error::Binding); }
        let probes = self.probes()?;
        for (layer, probe) in &probes {
            let selected = &settings.layers[layer];
            RefinementMonitor::new(vec![probe.clone()], selected.levels.clone(), selected.budget)?;
        }
        let identity = self.profile().identity();
        let mut json = LimitedJson { bytes: String::new(), limit: max_bytes };
        write!(json, "{{\"schema\":\"fa.decoder-monitor/1\",\"generation\":{},\"identity\":{{\"tenant\":{},\"model\":{},\"model_generation\":{},\"tokenizer_generation\":{},\"profile_generation\":{}}},\"budget\":{{\"encoded_bytes\":{},\"probe_coordinates\":{}}},\"layers\":[",
            settings.generation, identity.tenant, identity.model, identity.model_generation,
            identity.tokenizer_generation, identity.profile_generation, settings.budget.encoded_bytes,
            settings.budget.probe_coordinates).map_err(|_| Error::Limit)?;
        for (index, (layer, result)) in self.layers().iter().enumerate() {
            let selected = &settings.layers[layer];
            let fitted = result.calibration().fitted();
            let threshold = result.calibration().selected_threshold().ok_or(Error::WrongState)?;
            if index > 0 { json.write_str(",").map_err(|_| Error::Limit)?; }
            write!(json, "{{\"layer\":{layer},\"levels\":[").map_err(|_| Error::Limit)?;
            for (index, level) in selected.levels.iter().enumerate() {
                if index > 0 { json.write_str(",").map_err(|_| Error::Limit)?; }
                write!(json, "{level}").map_err(|_| Error::Limit)?;
            }
            write!(json, "],\"budget\":{{\"encoded_bytes\":{},\"probe_coordinates\":{}}},\"probes\":[{{\"id\":{},\"generation\":{},\"weights\":[",
                selected.budget.encoded_bytes, selected.budget.probe_coordinates,
                fitted.policy().id(), fitted.policy().generation()).map_err(|_| Error::Limit)?;
            for (index, weight) in fitted.weights().iter().enumerate() {
                if index > 0 { json.write_str(",").map_err(|_| Error::Limit)?; }
                write!(json, "{weight}").map_err(|_| Error::Limit)?;
            }
            write!(json, "],\"bias\":{},\"threshold\":{threshold}}}]}}", fitted.bias()).map_err(|_| Error::Limit)?;
        }
        json.write_str("]}").map_err(|_| Error::Limit)?;
        let bytes = json.bytes.into_bytes();
        // Match the existing consumer's parser bounds as well as its byte cap.
        crate::strict_json::parse(&bytes, crate::strict_json::Limits {
            max_bytes: MAX_MONITOR_CONFIG_BYTES, max_depth: 8,
            max_items: 262_144, max_string_bytes: 256,
        }).map_err(|_| Error::Limit)?;
        Ok(bytes)
    }

    /// Validate and encode BEFORE exclusive file creation. Existing files are
    /// never replaced. Write/sync failures may leave evidence at the new path;
    /// file sync alone is not atomic namespace publication or authentication.
    pub fn save_monitor_new(&self, settings: &MonitorExportSettings, max_bytes: usize,
        path: impl AsRef<Path>) -> Result<usize, MonitorExportError>
    {
        let bytes = self.monitor_json(settings, max_bytes).map_err(MonitorExportError::Refused)?;
        let mut options = OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|error| export_io(ExportStage::Create, error))?;
        file.write_all(&bytes).map_err(|error| export_io(ExportStage::Write, error))?;
        file.sync_all().map_err(|error| export_io(ExportStage::Sync, error))?;
        Ok(bytes.len())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportStage { Create, Write, Sync }
#[derive(Debug, PartialEq, Eq)]
pub enum MonitorExportError {
    Refused(Error),
    Io { stage: ExportStage, kind: io::ErrorKind },
}
impl fmt::Display for MonitorExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for MonitorExportError {}
fn export_io(stage: ExportStage, error: io::Error) -> MonitorExportError {
    MonitorExportError::Io { stage, kind: error.kind() }
}

struct LimitedJson { bytes: String, limit: usize }
impl fmt::Write for LimitedJson {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let length = self.bytes.len().checked_add(text.len()).ok_or(fmt::Error)?;
        if length > self.limit { return Err(fmt::Error); }
        self.bytes.try_reserve_exact(text.len()).map_err(|_| fmt::Error)?;
        self.bytes.push_str(text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emitted_decimal_numbers_preserve_binary32_boundaries() {
        for bits in [0_u32, 0x8000_0000, 1, 0x8000_0001, 0x007f_ffff, 0x0080_0000,
            0x3f80_0001, 0x7f7f_ffff, 0xff7f_ffff] {
            let value = f32::from_bits(bits);
            let mut json = LimitedJson { bytes: String::new(), limit: 128 };
            write!(json, "{value}").unwrap();
            let parsed = crate::strict_json::parse(json.bytes.as_bytes(), crate::strict_json::Limits::default()).unwrap();
            let crate::strict_json::Json::Number(number) = parsed else { panic!("not a number") };
            assert_eq!(number.lexeme().parse::<f32>().unwrap().to_bits(), bits);
        }
    }
}
