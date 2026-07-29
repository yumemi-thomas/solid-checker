use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{borrow::Borrow, fmt, path::Path, sync::Arc};
use thiserror::Error;

pub const SHA256_PREFIX: &str = "sha256:";

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    pub fn validate(self, source_len: usize) -> Result<(), FactIdentityError> {
        if self.start > self.end || usize::try_from(self.end).unwrap_or(usize::MAX) > source_len {
            return Err(FactIdentityError::InvalidSpan {
                span: self,
                source_len,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourcePath(Arc<str>);

impl SourcePath {
    pub fn new(path: impl Into<String>) -> Result<Self, FactIdentityError> {
        let path = path.into();
        if path.trim().is_empty() || path.contains('\0') {
            return Err(FactIdentityError::InvalidPath(path));
        }
        Ok(Self(normalize_path(&path).into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn shared(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }
}

impl AsRef<str> for SourcePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for SourcePath {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SourcePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceHash(String);

impl SourceHash {
    #[must_use]
    pub fn of(source: &str) -> Self {
        Self(format!(
            "{SHA256_PREFIX}{:x}",
            Sha256::digest(source.as_bytes())
        ))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, FactIdentityError> {
        let value = value.into();
        let digest = value
            .strip_prefix(SHA256_PREFIX)
            .ok_or_else(|| FactIdentityError::InvalidSourceHash(value.clone()))?;
        if digest.len() != 64
            || !digest
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(FactIdentityError::InvalidSourceHash(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Generation(u64);

impl Generation {
    pub fn new(value: u64) -> Result<Self, FactIdentityError> {
        if value == 0 {
            return Err(FactIdentityError::ZeroGeneration);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceIdentity {
    pub path: SourcePath,
    pub hash: SourceHash,
}

impl SourceIdentity {
    pub fn new(path: impl Into<String>, source: &str) -> Result<Self, FactIdentityError> {
        Ok(Self {
            path: SourcePath::new(path)?,
            hash: SourceHash::of(source),
        })
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum FactIdentityError {
    #[error("source path is empty or contains NUL: {0:?}")]
    InvalidPath(String),
    #[error("source hash is not canonical sha256: {0:?}")]
    InvalidSourceHash(String),
    #[error("generation must be non-zero")]
    ZeroGeneration,
    #[error("span {span:?} is outside source length {source_len}")]
    InvalidSpan { span: Span, source_len: usize },
}

fn normalize_path(path: &str) -> String {
    let normalized = Path::new(path).components().collect::<std::path::PathBuf>();
    normalized.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_identity_is_canonical() {
        let identity = SourceIdentity::new("src/../src/App.tsx", "const π = 1;").unwrap();
        assert_eq!(identity.path.as_str(), "src/../src/App.tsx");
        assert_eq!(identity.hash.as_str().len(), SHA256_PREFIX.len() + 64);
        assert_eq!(
            SourceHash::parse(identity.hash.to_string()).unwrap(),
            identity.hash
        );
    }

    #[test]
    fn cloned_source_paths_share_their_text_allocation() {
        let path = SourcePath::new("src/App.tsx").unwrap();
        let cloned = path.clone();
        assert!(std::ptr::eq(path.as_str(), cloned.as_str()));
    }

    #[test]
    fn rejects_invalid_ranges_and_generation() {
        assert!(Span::new(4, 2).validate(8).is_err());
        assert_eq!(Generation::new(0), Err(FactIdentityError::ZeroGeneration));
    }
}
