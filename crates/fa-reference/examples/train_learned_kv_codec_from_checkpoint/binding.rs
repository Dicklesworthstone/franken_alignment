//! Emit the exact existing independently retained fit-binding schema.
use super::invalid;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::replay::archive::LearnedKvFitBinding;
use std::io::{self, Write};

pub(super) fn encode(binding: &LearnedKvFitBinding, limit: usize) -> io::Result<Vec<u8>> {
    if limit == 0 || limit > super::fields::MAX_DATA_BYTES { return Err(invalid("invalid original binding byte limit")); }
    let mut output = Bounded { bytes: Vec::new(), limit };
    let p = binding.policy; let b = binding.budget;
    write!(output, "{{\"schema\":\"fa.learned-fit-binding/1\",\"policy\":{{\"id\":{},\"generation\":{},\"rank\":{},\"sweeps\":{}}},\"budget\":{{\"source_values\":{},\"parameter_values\":{},\"scratch_values\":{},\"work_units\":{}}},\"sources\":[",
        p.id(), p.generation(), p.rank(), p.sweeps(), b.source_values, b.parameter_values, b.scratch_values, b.work_units)?;
    for (index, (origin, descriptor)) in binding.sources.iter().enumerate() {
        if index != 0 { output.write_all(b",")?; }
        write!(output, "{{\"origin\":{origin},\"descriptor_hex\":\"")?;
        for byte in descriptor.encode().map_err(super::contract)? { write!(output, "{byte:02x}")?; }
        output.write_all(b"\"}")?;
    }
    output.write_all(b"]}\n")?;
    Ok(output.bytes)
}
struct Bounded { bytes: Vec<u8>, limit: usize }
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) { return Err(invalid("fit binding exceeds its output byte allowance")); }
        self.bytes.try_reserve(bytes.len()).map_err(|error| invalid(error.to_string()))?;
        self.bytes.extend_from_slice(bytes); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
