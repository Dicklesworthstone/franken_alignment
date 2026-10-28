//! Closed operator data and one conserved auxiliary/model asset byte allowance.
use crate::config::{debug, read_regular};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub(super) const MAX_ASSET_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_DATA_BYTES: usize = 2 * 1024 * 1024;

pub(super) struct Assets {
    remaining: usize,
    consumed: usize,
    forbidden: Option<Vec<PathBuf>>,
}
impl Assets {
    pub(super) fn new(limit: usize) -> Result<Self, String> {
        if limit == 0 || limit > MAX_ASSET_BYTES { return Err("invalid aggregate learned asset byte limit".into()); }
        Ok(Self { remaining: limit, consumed: 0, forbidden: None })
    }
    /// One fleet allowance covers the recipe and its original numerical assets.
    /// The recipe's own independent ceiling is conjoined after decoding it.
    pub(super) fn for_fleet(limit: usize, forbidden: &[PathBuf]) -> Result<Self, String> {
        if limit == 0 { return Err("fleet input allowance exhausted".into()); }
        let mut roots = Vec::new();
        for root in forbidden {
            if !root.is_absolute() || root.components().any(|part|
                !matches!(part, Component::RootDir | Component::Normal(_))) {
                return Err("fleet authority roots must be absolute normalized paths".into());
            }
            roots.push(root.clone());
            roots.push(resolve_root(root)?);
        }
        Ok(Self { remaining: limit, consumed: 0, forbidden: Some(roots) })
    }
    pub(super) fn constrain(&mut self, limit: usize) -> Result<(), String> {
        if limit == 0 || limit > MAX_ASSET_BYTES { return Err("invalid aggregate learned asset byte limit".into()); }
        self.remaining = self.remaining.min(limit);
        Ok(())
    }
    pub(super) fn admit_path(&self, path: &Path) -> Result<(), String> {
        if let Some(roots) = &self.forbidden {
            if roots.iter().any(|root| path.starts_with(root)) {
                return Err("fleet learned assets must be outside every authority store".into());
            }
            let canonical = std::fs::canonicalize(path).map_err(debug)?;
            if roots.iter().any(|root| canonical.starts_with(root)) {
                return Err("fleet learned asset resolves inside an authority store".into());
            }
        }
        Ok(())
    }
    pub(super) fn read(&mut self, path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
        self.admit_path(path)?;
        let bytes = read_regular(path, maximum.min(self.remaining))?;
        self.charge(bytes.len())?;
        Ok(bytes)
    }
    pub(super) fn remaining(&self) -> usize { self.remaining }
    pub(super) fn consumed(&self) -> usize { self.consumed }
    /// Charge actual streamed tensor reads, including a failed attempt. An
    /// overrun exhausts the allowance rather than leaving it reusable.
    pub(super) fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let before = self.remaining;
        self.remaining = self.remaining.saturating_sub(bytes);
        self.consumed = self.consumed.saturating_add(bytes);
        if bytes > before { Err("learned asset limit exceeded".into()) } else { Ok(()) }
    }
}

// A registered authority may be offline or absent. Resolve its existing
// ancestor without opening a Store, preserving every missing suffix component.
pub(super) fn resolve_root(root: &Path) -> Result<PathBuf, String> {
    let mut ancestor = root.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match std::fs::symlink_metadata(&ancestor) {
            Ok(_) => {
                let mut canonical = std::fs::canonicalize(&ancestor).map_err(debug)?;
                for component in suffix.into_iter().rev() { canonical.push(component); }
                return Ok(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(ancestor.file_name().ok_or("missing fleet authority ancestor")?.to_os_string());
                if !ancestor.pop() { return Err("missing fleet authority ancestor".into()); }
            }
            Err(error) => return Err(debug(error)),
        }
    }
}

pub(super) struct Fields(BTreeMap<String, Json>);
impl Fields {
    pub(super) fn parse(bytes: &[u8], maximum: usize) -> Result<Self, String> {
        Self::new(strict_json::parse(bytes, Limits { max_bytes: maximum, max_depth: 10,
            max_items: 262_144, max_string_bytes: MAX_DATA_BYTES }).map_err(debug)?)
    }
    pub(super) fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected learned recipe object".into()) }
    }
    pub(super) fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0.remove(name).ok_or_else(|| format!("missing learned recipe field {name}"))
    }
    pub(super) fn object(&mut self, name: &str) -> Result<Self, String> { Self::new(self.take(name)?) }
    pub(super) fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text"))
    }
    pub(super) fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    pub(super) fn size(&mut self, name: &str) -> Result<usize, String> {
        usize::try_from(self.number(name)?).map_err(debug)
    }
    pub(super) fn scalar(&mut self, name: &str) -> Result<f32, String> {
        scalar(self.take(name)?, name)
    }
    pub(super) fn path(&mut self, name: &str) -> Result<PathBuf, String> {
        path(&self.text(name)?)
    }
    pub(super) fn array(&mut self, name: &str, limit: usize) -> Result<Vec<Json>, String> {
        let Json::Array(items) = self.take(name)? else { return Err(format!("{name} must be an array")); };
        if items.len() > limit { return Err(format!("{name} exceeds its item limit")); }
        Ok(items)
    }
    pub(super) fn ids(&mut self, name: &str, limit: usize) -> Result<Vec<u32>, String> {
        self.array(name, limit)?.into_iter().map(|item| item.as_u64()
            .and_then(|n| u32::try_from(n).ok()).ok_or_else(|| format!("{name} requires 32-bit token IDs"))).collect()
    }
    pub(super) fn scalars(&mut self, name: &str, limit: usize) -> Result<Vec<f32>, String> {
        self.array(name, limit)?.into_iter().map(|value| scalar(value, name)).collect()
    }
    pub(super) fn entries(self) -> BTreeMap<String, Json> { self.0 }
    pub(super) fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unsupported learned recipe field".into()) }
    }
}

pub(super) fn path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.file_name().is_none()
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_))) {
        return Err("learned asset paths must be absolute normalized file paths".into());
    }
    Ok(path)
}
pub(super) fn hex(value: &str, limit: usize) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || value.len() / 2 > limit { return Err("invalid bounded hex value".into()); }
    value.as_bytes().chunks_exact(2).map(|pair| {
        fn digit(byte: u8) -> Option<u8> { match byte {
            b'0'..=b'9' => Some(byte - b'0'), b'a'..=b'f' => Some(byte - b'a' + 10), _ => None,
        } }
        Ok(digit(pair[0]).ok_or("hex must be lowercase")? * 16 + digit(pair[1]).ok_or("hex must be lowercase")?)
    }).collect()
}
fn scalar(value: Json, name: &str) -> Result<f32, String> {
    let Json::Number(number) = value else { return Err(format!("{name} must be a finite binary32 number")); };
    number.lexeme().parse::<f32>().ok().filter(|value| value.is_finite())
        .ok_or_else(|| format!("{name} must be a finite binary32 number"))
}
