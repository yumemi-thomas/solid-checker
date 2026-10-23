//! ADR 0372: the own keys an installed package's exported function can
//! return, read by the host from the package's selected runtime source.
//!
//! Like [`crate::package_boundary`], a normalized project-analysis input the
//! backend acquires and this index only answers. A row says: from `importer`,
//! the export `export` of the module `specifier` loads is a function every
//! value of which is an object holding at most `keys` as own properties
//! (`AstFacts::returned_object_keys`). **Absence is not evidence**: an import
//! with no row is one nothing is known about, and keeps every obligation.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};

/// See the module documentation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageReturnKeys {
    rows: BTreeMap<(String, String, String), BTreeSet<String>>,
}

impl PackageReturnKeys {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Records what `export` of `specifier`, imported from `importer`, returns.
    pub fn record(
        &mut self,
        importer: impl Into<String>,
        specifier: impl Into<String>,
        export: impl Into<String>,
        keys: impl IntoIterator<Item = String>,
    ) {
        self.rows.insert(
            (importer.into(), specifier.into(), export.into()),
            keys.into_iter().collect(),
        );
    }

    /// The keys `export` of `specifier`, imported from `importer`, can return.
    #[must_use]
    pub fn keys(&self, importer: &str, specifier: &str, export: &str) -> Option<&BTreeSet<String>> {
        self.rows
            .get(&(importer.to_owned(), specifier.to_owned(), export.to_owned()))
    }

    /// The cache identity of the rows.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        let text = |hash: &mut Sha256, value: &str| {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        };
        hash.update(b"package-return-keys-v1");
        hash.update((self.rows.len() as u64).to_be_bytes());
        for ((importer, specifier, export), keys) in &self.rows {
            text(&mut hash, importer);
            text(&mut hash, specifier);
            text(&mut hash, export);
            hash.update((keys.len() as u64).to_be_bytes());
            for key in keys {
                text(&mut hash, key);
            }
        }
        hash.finalize().into()
    }
}
