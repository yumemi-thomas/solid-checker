//! Compact semantic identities shared by the analysis indexes.
//!
//! Compiler symbol IDs are short and copied into several retained indexes.
//! Keeping them as `String` gives every copy its own heap allocation.  These
//! wrappers preserve string lookup ergonomics while making clones share one
//! immutable allocation across all retained analysis stages.

use std::{borrow::Borrow, collections::HashSet, fmt, ops::Deref, sync::Arc};

use solid_facts::TypeScriptTable;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct SymbolId(Arc<str>);

impl SymbolId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SymbolId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for SymbolId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for SymbolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&str> for SymbolId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for SymbolId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl AsRef<str> for SymbolId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<str> for SymbolId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SymbolId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

pub(super) type SymbolName = SymbolId;

pub(super) struct SymbolInterner {
    values: HashSet<SymbolId>,
}

impl SymbolInterner {
    pub(super) fn from_table(table: &TypeScriptTable) -> Self {
        let mut values = HashSet::new();
        for symbol in table.symbols() {
            values.insert(SymbolId::from(symbol.id()));
            if !symbol.alias_target().is_empty() {
                values.insert(SymbolId::from(symbol.alias_target()));
            }
        }
        Self { values }
    }

    pub(super) fn intern(&self, value: &str) -> SymbolId {
        self.values
            .get(value)
            .cloned()
            .unwrap_or_else(|| SymbolId::from(value))
    }
}

pub(super) fn symbol_id(value: &str) -> SymbolId {
    SymbolId::from(value)
}

pub(super) fn symbol_name(value: &str) -> SymbolName {
    SymbolId::from(value)
}
