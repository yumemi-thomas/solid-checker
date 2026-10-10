//! Runtime module resolution: which file the project's own bundler loads for
//! one module specifier occurrence, as that bundler resolved it.
//!
//! [`crate::resolution`] carries TypeScript's answer, which selects
//! declarations and knows nothing of bundler aliases, extension lists or
//! shims. This table is the runtime's answer, observed from the project's
//! installed resolver (ADR 0220). It is optional on [`crate::ProjectFacts`]
//! and absent unless the host asked for it. A present table answers every
//! occurrence it holds a row for, and a missing row is unknown: it is never
//! filled in from another occurrence or from TypeScript.

use crate::core::Span;
use compact_str::CompactString;
use std::collections::HashMap;
use std::sync::Arc;

/// What the runtime resolver loads for one occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeOutcome {
    /// A file on disk: the resolver's path and its real path.
    File {
        path: Arc<str>,
        physical_path: Arc<str>,
    },
    /// The resolver left the request to the host, outside the bundle.
    External,
    /// A module the runtime provides itself (`node:fs`).
    Builtin,
    /// Unresolved, an error, a virtual or transformed module, or a load the
    /// resolver cannot attest (a CommonJS `require` in a browser bundle).
    Unknown,
}

/// The runtime resolution of the occurrences the resolver answered for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeResolutionIndex {
    by_site: HashMap<(Arc<str>, u32, u32), (CompactString, RuntimeOutcome)>,
}

impl RuntimeResolutionIndex {
    /// Records the outcome for the specifier written at `span` in `path`.
    pub fn insert(
        &mut self,
        path: impl Into<Arc<str>>,
        span: Span,
        text: impl Into<CompactString>,
        outcome: RuntimeOutcome,
    ) {
        self.by_site
            .insert((path.into(), span.start, span.end), (text.into(), outcome));
    }

    /// The outcome for the specifier `text` written at `span` in `path`.
    /// A missing row, or one whose text differs, is [`RuntimeOutcome::Unknown`].
    #[must_use]
    pub fn outcome(&self, path: &str, span: Span, text: &str) -> &RuntimeOutcome {
        match self.by_site.get(&(Arc::from(path), span.start, span.end)) {
            Some((recorded, outcome)) if recorded == text => outcome,
            _ => &RuntimeOutcome::Unknown,
        }
    }

    /// Deterministic native cache identity; includes unknown and text-mismatched rows.
    #[must_use]
    pub fn identity_rows(&self) -> Vec<String> {
        let mut rows = self
            .by_site
            .iter()
            .map(|((path, start, end), (text, outcome))| {
                format!("{path:?}:{start}:{end}:{text:?}:{outcome:?}")
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    }

    /// The number of answered occurrences.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_site.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_site.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_mismatched_row_is_unknown() {
        let mut index = RuntimeResolutionIndex::default();
        index.insert(
            "/p/a.ts",
            Span::new(10, 20),
            "./b.js",
            RuntimeOutcome::External,
        );
        assert_eq!(
            index.outcome("/p/a.ts", Span::new(10, 20), "./b.js"),
            &RuntimeOutcome::External
        );
        assert_eq!(
            index.outcome("/p/a.ts", Span::new(10, 20), "./c.js"),
            &RuntimeOutcome::Unknown
        );
        assert_eq!(
            index.outcome("/p/a.ts", Span::new(11, 20), "./b.js"),
            &RuntimeOutcome::Unknown
        );
        assert_eq!(
            index.outcome("/p/x.ts", Span::new(10, 20), "./b.js"),
            &RuntimeOutcome::Unknown
        );
    }
}
