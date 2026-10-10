//! Immutable static RoPE interpretation for the original half-split kernel.
//! Frequency changes are numerical identity, never a live cache mutation.

use crate::Error;

/// Validated static scaling. None retains the original arithmetic and archives.
/// Dynamic frequency updates, partial rotation and attention rescaling are not
/// admitted by these profiles. Parameters are private and compared by f64 bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RotaryScaling {
    #[default]
    None,
    Linear(LinearRotaryScaling),
    Llama3(Llama3RotaryScaling),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinearRotaryScaling { factor_bits: u64 }
impl LinearRotaryScaling {
    pub fn factor(self) -> f64 { f64::from_bits(self.factor_bits) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Llama3RotaryScaling {
    factor_bits: u64,
    low_freq_factor_bits: u64,
    high_freq_factor_bits: u64,
    original_max_position_embeddings: usize,
}
impl Llama3RotaryScaling {
    pub fn factor(self) -> f64 { f64::from_bits(self.factor_bits) }
    pub fn low_freq_factor(self) -> f64 { f64::from_bits(self.low_freq_factor_bits) }
    pub fn high_freq_factor(self) -> f64 { f64::from_bits(self.high_freq_factor_bits) }
    pub fn original_max_position_embeddings(self) -> usize { self.original_max_position_embeddings }
}

impl RotaryScaling {
    pub fn linear(factor: f64) -> Result<Self, Error> {
        if !factor.is_finite() || factor < 1.0 { return Err(Error::InvalidInput); }
        Ok(Self::Linear(LinearRotaryScaling { factor_bits: factor.to_bits() }))
    }

    pub fn llama3(factor: f64, low_freq_factor: f64, high_freq_factor: f64,
        original_max_position_embeddings: usize) -> Result<Self, Error>
    {
        if !factor.is_finite() || factor < 1.0 || !low_freq_factor.is_finite()
            || low_freq_factor <= 0.0 || !high_freq_factor.is_finite()
            || high_freq_factor <= low_freq_factor || original_max_position_embeddings == 0
        { return Err(Error::InvalidInput); }
        Ok(Self::Llama3(Llama3RotaryScaling { factor_bits: factor.to_bits(),
            low_freq_factor_bits: low_freq_factor.to_bits(),
            high_freq_factor_bits: high_freq_factor.to_bits(), original_max_position_embeddings }))
    }

    pub(super) fn frequency(self, unscaled: f64) -> Result<f64, Error> {
        let scaled = match self {
            Self::None => unscaled,
            Self::Linear(parameters) => unscaled / parameters.factor(),
            Self::Llama3(parameters) => {
                let wavelength = std::f64::consts::TAU / unscaled;
                let original = parameters.original_max_position_embeddings as f64;
                let low_wavelength = original / parameters.low_freq_factor();
                let high_wavelength = original / parameters.high_freq_factor();
                if wavelength < high_wavelength {
                    unscaled
                } else if wavelength > low_wavelength {
                    unscaled / parameters.factor()
                } else {
                    let smooth = (original / wavelength - parameters.low_freq_factor())
                        / (parameters.high_freq_factor() - parameters.low_freq_factor());
                    (1.0 - smooth) * unscaled / parameters.factor() + smooth * unscaled
                }
            }
        };
        // An extreme but finite configuration must not collapse a positive
        // frequency to zero or defer a numerical refusal to live inference.
        if !scaled.is_finite() || scaled <= 0.0 { return Err(Error::InvalidInput); }
        Ok(scaled)
    }

    /// Exact comparison material shared by the existing owners. This is not a
    /// portable live-state constructor. Only scaled modes have an extension;
    /// unscaled profiles retain their original bytes in every containing format.
    pub(crate) fn binding_bytes(self) -> Option<[u8; 40]> {
        let words = match self {
            Self::None => return None,
            Self::Linear(p) => [1, p.factor_bits, 0, 0, 0],
            Self::Llama3(p) => [2, p.factor_bits, p.low_freq_factor_bits,
                p.high_freq_factor_bits, p.original_max_position_embeddings as u64],
        };
        let mut bytes = [0; 40];
        for (index, word) in words.into_iter().enumerate() {
            bytes[index * 8..index * 8 + 8].copy_from_slice(&word.to_be_bytes());
        }
        Some(bytes)
    }

    pub(crate) fn from_binding_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != 40 { return Err(Error::Incomplete); }
        let mut words = [0_u64; 5];
        for (word, source) in words.iter_mut().zip(bytes.chunks_exact(8)) {
            *word = u64::from_be_bytes(source.try_into().map_err(|_| Error::Incomplete)?);
        }
        match words[0] {
            1 if words[2..] == [0, 0, 0] => Self::linear(f64::from_bits(words[1])),
            2 => Self::llama3(f64::from_bits(words[1]), f64::from_bits(words[2]),
                f64::from_bits(words[3]), usize::try_from(words[4]).map_err(|_| Error::Limit)?),
            // No alternate representation for None or unused parameter words.
            _ => Err(Error::InvalidInput),
        }
    }
}
