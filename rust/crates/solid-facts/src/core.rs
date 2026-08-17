use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{borrow::Borrow, collections::HashSet, fmt, path::Path, sync::Arc};
use thiserror::Error;

pub const SHA256_PREFIX: &str = "sha256:";
const MODULE_EXTENSIONS: &[&str] = &[".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"];

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

    /// This span as a [`typefacts::Location`] in `path` — the one conversion
    /// between the fact tables' u32 spans and typefacts' u64 byte offsets.
    #[must_use]
    pub fn location(self, path: impl Into<Arc<str>>) -> typefacts::Location {
        typefacts::Location {
            path: path.into(),
            start_byte: u64::from(self.start),
            end_byte: u64::from(self.end),
        }
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

/// Resolve a plain relative module specifier against an analyzed project file
/// set.
///
/// This is deliberately a lexical resolver. The checker must not consult the
/// host filesystem while proving a cross-file fact: an on-disk sibling may be
/// outside the analyzed project, and extension/index precedence is not a
/// semantic fact available to this layer. If more than one analyzed path
/// matches, resolution fails closed instead of using iteration order.
#[must_use]
pub fn resolve_relative_module_path<'a, I>(
    source: &str,
    module: &str,
    project_paths: I,
) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    if !module.starts_with("./") && !module.starts_with("../") {
        return None;
    }
    let base = source
        .rsplit_once('/')
        .map_or("", |(directory, _)| directory);
    let joined = if base.is_empty() {
        module.to_owned()
    } else {
        format!("{base}/{module}")
    };
    let normalized = normalize_relative_path(&joined)?;
    let mut candidates = HashSet::from([normalized.clone()]);
    if let Some(stem) = strip_known_extension(&normalized) {
        for extension in MODULE_EXTENSIONS {
            candidates.insert(format!("{stem}{extension}"));
        }
    } else {
        for extension in MODULE_EXTENSIONS {
            candidates.insert(format!("{normalized}{extension}"));
        }
    }
    for extension in MODULE_EXTENSIONS {
        candidates.insert(format!("{normalized}/index{extension}"));
    }
    let mut resolved = None;
    for path in project_paths {
        if !candidates
            .iter()
            .any(|candidate| equivalent_project_path(path, candidate))
        {
            continue;
        }
        if resolved.is_some() {
            return None;
        }
        resolved = Some(path);
    }
    resolved
}

fn equivalent_project_path(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix("/private")
            .is_some_and(|left| left == right)
        || right
            .strip_prefix("/private")
            .is_some_and(|right| right == left)
}

fn strip_known_extension(path: &str) -> Option<String> {
    let (stem, extension) = path.rsplit_once('.')?;
    MODULE_EXTENSIONS
        .iter()
        .any(|candidate| *candidate == format!(".{extension}"))
        .then(|| stem.to_owned())
}

fn normalize_relative_path(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let absolute = path.starts_with('/');
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    if absolute {
                        return None;
                    }
                    segments.push("..".to_owned());
                }
            }
            segment => segments.push(segment.to_owned()),
        }
    }
    let joined = segments.join("/");
    if absolute {
        Some(format!("/{joined}"))
    } else {
        Some(joined)
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

    #[test]
    fn relative_module_resolution_is_lexical_and_fails_on_ambiguity() {
        let paths = ["src/App.tsx", "src/values.ts", "src/values/index.ts"];
        assert_eq!(
            resolve_relative_module_path(
                "src/App.tsx",
                "./values",
                ["src/App.tsx", "src/values.ts",]
            ),
            Some("src/values.ts")
        );
        assert_eq!(
            resolve_relative_module_path("src/App.tsx", "./values.js", ["src/values.ts"]),
            Some("src/values.ts")
        );
        assert_eq!(
            resolve_relative_module_path("App.tsx", "./values", ["values.ts"]),
            Some("values.ts")
        );
        assert_eq!(
            resolve_relative_module_path(
                "/private/tmp/project/App.tsx",
                "./values",
                ["/tmp/project/values.ts"]
            ),
            Some("/tmp/project/values.ts")
        );
        assert_eq!(
            resolve_relative_module_path("src/App.tsx", "./values", paths),
            None
        );
        assert_eq!(
            resolve_relative_module_path("src/App.tsx", "../values", ["values.ts"]),
            Some("values.ts")
        );
        assert_eq!(
            resolve_relative_module_path("src/App.tsx", "solid-js", ["solid-js.ts"]),
            None
        );
    }
}
