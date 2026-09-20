//! Versioned producer-cut metadata. No consumer infers a cut from read time.
use super::{FilePublicationCapture, FileCaptureIdentity, SourceBinding, FilePublicationInputs,
    FileOversight, JournalError, Reader, Writer, PublicationInputCut, check_identity};
use crate::action::FrozenAction;
use crate::action::consequence::oversight::action_frame;
use crate::action::consequence::delivery::publication_gate::changes::PublicationInputCutStatus;
use crate::Error;

impl FilePublicationCapture {
    /// Produce FAPCAP02, asserting the complete change prefix included in THIS
    /// immutable image. Increment the producer generation when the cut changes.
    /// Binding this original packet makes cut metadata mandatory for later reads.
    pub fn new_at_cut(attempt: u64, identity: FileCaptureIdentity, action: &FrozenAction,
        inputs: FilePublicationInputs, input_cut: PublicationInputCut) -> Result<Self, Error>
    {
        check_identity(identity)?;
        input_cut.check()?;
        let capture = Self { attempt, identity, frame: action_frame(action), inputs, input_cut: Some(input_cut) };
        capture.to_bytes()?;
        Ok(capture)
    }
    pub fn input_cut(&self) -> Option<PublicationInputCut> { self.input_cut }
}

impl SourceBinding {
    pub(in super::super::super) fn input_cut(&self) -> Option<PublicationInputCut> { self.input_cut }
    pub(in super::super::super) fn read_at_cut(r: &mut Reader<'_>) -> Result<Self, Error> {
        let cut = read_input_cut(r)?;
        let mut binding = Self::read(r)?;
        binding.input_cut = Some(cut);
        Ok(binding)
    }
}

impl FileOversight {
    /// Last installed producer cut and retained invalidation floor, not a permit.
    pub fn publication_input_cut(&self, attempt: u64) -> Result<Option<PublicationInputCutStatus>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_input_cut(attempt)?)
    }
}

pub(super) fn write_input_cut(w: &mut Writer, cut: PublicationInputCut) -> Result<(), Error> {
    cut.check()?;
    w.u64(cut.source)?; w.u64(cut.through)
}
pub(super) fn read_input_cut(r: &mut Reader<'_>) -> Result<PublicationInputCut, Error> {
    let cut = PublicationInputCut { source: r.u64()?, through: r.u64()? };
    cut.check()?;
    Ok(cut)
}
