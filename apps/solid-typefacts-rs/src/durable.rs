//! Durable symbol identity (Go: `durableSymbolRef` in `tsgo/project.go`).
//!
//! The identity of a symbol is the name span of its first declaration plus the
//! symbol's name. It survives program rebuilds while that declaration is
//! unchanged, so retained facts stay resolvable after an update. The hash
//! inputs are byte-identical to Go's, because durable ids persist across
//! sessions and the rendering may never drift.

use crate::gocompat::clean;
use sha2::{Digest, Sha256};
use tsrs_ast::{Node, Symbol, get_source_file_of_node};
use tsrs_core::P;
use tsrs_scanner::skip_trivia;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DurableRef {
    pub path: String,
    pub start_byte: u32,
    pub end_byte: u32,
    pub name: String,
}

/// Go: `hashedSymbolID`: `symbol:h:` and 24 lowercase hex digits (the first 12
/// bytes of the digest).
fn hashed_symbol_id(digest: &[u8]) -> String {
    let mut id = String::with_capacity(9 + 24);
    id.push_str("symbol:h:");
    for byte in &digest[..12] {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

impl DurableRef {
    /// Go: `durableSymbolRef.id`: `sha256(path 0 start 0 end 0 name)`.
    #[must_use]
    pub fn id(&self) -> String {
        let mut input = Vec::with_capacity(self.path.len() + self.name.len() + 32);
        input.extend_from_slice(self.path.as_bytes());
        input.push(0);
        input.extend_from_slice(self.start_byte.to_string().as_bytes());
        input.push(0);
        input.extend_from_slice(self.end_byte.to_string().as_bytes());
        input.push(0);
        input.extend_from_slice(self.name.as_bytes());
        hashed_symbol_id(&Sha256::digest(&input))
    }

    /// Go: `durableSymbolRef.exportedID`: `sha256("export" 0 path 0 name)`,
    /// span-insensitive, so a module-visible symbol keeps it across edits that
    /// only move its declaration.
    #[must_use]
    pub fn exported_id(&self) -> String {
        let mut input = Vec::with_capacity(self.path.len() + self.name.len() + 16);
        input.extend_from_slice(b"export\0");
        input.extend_from_slice(self.path.as_bytes());
        input.push(0);
        input.extend_from_slice(self.name.as_bytes());
        hashed_symbol_id(&Sha256::digest(&input))
    }
}

/// Go: `durableRefFor`: the symbol's first declaration.
#[must_use]
pub fn durable_ref_for(symbol: P<Symbol>) -> Option<DurableRef> {
    let declaration = *symbol.declarations().first()?;
    durable_ref_for_declaration(symbol, declaration)
}

/// Go: `durableRefForDeclaration`.
#[must_use]
pub fn durable_ref_for_declaration(symbol: P<Symbol>, node: P<Node>) -> Option<DurableRef> {
    let source_file = get_source_file_of_node(node)?;
    let name_node = node.name().unwrap_or(node);
    Some(DurableRef {
        path: clean(source_file.file_name()),
        start_byte: skip_trivia(source_file.text(), name_node.pos().source_offset()?),
        end_byte: name_node.end().source_offset()?,
        name: symbol.name().to_owned(),
    })
}
