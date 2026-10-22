//! The declaration-shape cutoff (Go: `declarationShape`, `declarationShapeFor`,
//! `preserveExportIdentities`, `exportedDurableSymbols` in `tsgo/project.go`).
//!
//! A diagnostic-free forced declaration emit of one module proves its exported
//! TypeScript contract. If the contract (emit text, external export slots and
//! their canonical target identities, resolved imports) is unchanged by an
//! edit, importers need not be re-analyzed. The fingerprint is self-consistent
//! within one producer, so a compiler bump changes only when the cutoff fires.

use crate::durable::{DurableRef, durable_ref_for};
use crate::gocompat::clean;
use crate::unsafe_boundary::Generation;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tsrs_ast::{
    Node, SourceFile, SymbolFlags, is_external_module, is_external_module_augmentation,
    is_global_scope_augmentation,
};
use tsrs_compiler::{Context, EmitOnly, EmitOptions, Program, WriteFileData};
use tsrs_core::P;

/// One external export slot of a module.
#[derive(Clone, Debug)]
pub struct DeclarationExport {
    pub name: String,
    /// Go: `ref.exportedID()`.
    pub id: String,
    pub reference: DurableRef,
    /// The canonical target symbol's address. Valid only for a shape computed
    /// by the checker that is still alive; a cached shape's keys are never
    /// read (only `name` and `id` of an old shape are), which is why a
    /// retained shape does not pin checker memory.
    pub symbol_key: usize,
}

#[derive(Clone, Debug)]
pub struct DeclarationShape {
    pub signature: [u8; 32],
    pub exports: Vec<DeclarationExport>,
    pub imports: Vec<String>,
}

/// Go: `hasGlobalOrModuleAugmentation`.
#[must_use]
pub fn has_global_or_module_augmentation(file: P<SourceFile>) -> bool {
    fn visit(node: P<Node>, found: &mut bool) -> bool {
        if is_external_module_augmentation(node) || is_global_scope_augmentation(node) {
            *found = true;
            return true;
        }
        node.for_each_child(&mut |child| visit(child, found));
        *found
    }
    let mut found = false;
    file.as_node()
        .for_each_child(&mut |child| visit(child, &mut found));
    found
}

/// Go: `exportedDurableSymbols`. `None` when any export has no durable
/// identity or two share a name.
pub fn exported_durable_symbols(
    generation: &Generation,
    file: P<SourceFile>,
) -> Option<Vec<DeclarationExport>> {
    let module_symbol = file.symbol()?;
    let mut exports = generation.with_checker(|checker| {
        let mut exports = Vec::new();
        for module_export in checker.get_exports_of_module_exported(module_symbol) {
            let name = module_export.name().to_owned();
            let mut symbol = module_export;
            if symbol.flags().intersects(SymbolFlags::Alias) {
                symbol = checker.get_aliased_symbol(symbol);
            }
            let reference = durable_ref_for(symbol)?;
            exports.push(DeclarationExport {
                name,
                id: reference.exported_id(),
                reference,
                symbol_key: symbol.addr(),
            });
        }
        Some(exports)
    })?;
    exports.sort_by(|left, right| left.name.cmp(&right.name));
    if exports.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return None;
    }
    Some(exports)
}

/// Go: `resolvedImportPaths`. `None` when any import is unresolved.
#[must_use]
pub fn resolved_import_paths(program: &Program, file: P<SourceFile>) -> Option<Vec<String>> {
    let mut imports = Vec::with_capacity(file.imports().len());
    for specifier in file.imports() {
        let resolved = program.get_resolved_module_from_module_specifier(file, *specifier)?;
        imports.push(clean(resolved.resolved_file_name));
    }
    imports.sort();
    Some(imports)
}

/// Go: `declarationShapeFor`. The generation's program must contain `path`.
/// `None` is "no proof": not a module, an augmentation, a declaration
/// diagnostic, a second output, an export without durable identity, or an
/// unresolved import.
pub fn declaration_shape_for(generation: &Generation, path: &str) -> Option<DeclarationShape> {
    let program = generation.program();
    let file = program.get_source_file(path)?;
    if !is_external_module(file) || has_global_or_module_augmentation(file) {
        return None;
    }

    let declaration_text = Mutex::new(String::new());
    let write_diagnostics = AtomicBool::new(false);
    let write = |_name: &str, text: &str, data: &mut WriteFileData| -> Result<(), String> {
        if !data.diagnostics.is_empty() || data.skipped_dts_write {
            write_diagnostics.store(true, Ordering::SeqCst);
        }
        let mut slot = declaration_text
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_empty() {
            // A source file should produce one declaration output; a second is
            // not part of this proof.
            text.clone_into(&mut slot);
        } else {
            write_diagnostics.store(true, Ordering::SeqCst);
        }
        Ok(())
    };
    // Go's `EmitOnlyForcedDts` is `(EmitOnlyDts, force_emit)` here.
    let result = program.emit(
        &Context::default(),
        &EmitOptions {
            target_source_files: Some(vec![file]),
            emit_only: EmitOnly::EmitOnlyDts,
            force_emit: true,
            write_file: Some(&write),
        },
    );
    let declaration_text = declaration_text
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if result.emit_skipped
        || !result.diagnostics.is_empty()
        || write_diagnostics.load(Ordering::SeqCst)
        || declaration_text.is_empty()
    {
        return None;
    }

    let exports = exported_durable_symbols(generation, file)?;
    let imports = resolved_import_paths(program, file)?;
    Some(DeclarationShape {
        signature: Sha256::digest(declaration_text.as_bytes()).into(),
        exports,
        imports,
    })
}

/// Go: `preserveExportIdentities`, reduced to its decision. It proves two
/// generations of a module expose the same declaration contract and pairs
/// their canonical target symbols by external export name; any non-bijective
/// pairing fails closed. (Go also returns the pairing so the project can
/// re-key its symbol maps; that map belongs to the symbol store of S4, which
/// recomputes it from the new shape.)
#[must_use]
pub fn preserve_export_identities(previous: &DeclarationShape, next: &DeclarationShape) -> bool {
    if previous.signature != next.signature
        || previous.exports.len() != next.exports.len()
        || previous.imports != next.imports
    {
        return false;
    }
    let mut preserved: HashMap<usize, &str> = HashMap::with_capacity(next.exports.len());
    let mut symbol_by_id: HashMap<&str, usize> = HashMap::with_capacity(next.exports.len());
    for (old_export, new_export) in previous.exports.iter().zip(&next.exports) {
        if old_export.name != new_export.name
            || old_export.id != new_export.id
            || new_export.symbol_key == 0
        {
            return false;
        }
        if let Some(existing) = preserved.get(&new_export.symbol_key)
            && *existing != new_export.id
        {
            return false;
        }
        if let Some(existing) = symbol_by_id.get(new_export.id.as_str())
            && *existing != new_export.symbol_key
        {
            return false;
        }
        preserved.insert(new_export.symbol_key, &new_export.id);
        symbol_by_id.insert(&new_export.id, new_export.symbol_key);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export(name: &str, key: usize) -> DeclarationExport {
        let reference = DurableRef {
            path: "/p/m.ts".into(),
            start_byte: 0,
            end_byte: 1,
            name: name.into(),
        };
        DeclarationExport {
            name: name.into(),
            id: reference.exported_id(),
            reference,
            symbol_key: key,
        }
    }

    fn shape(exports: Vec<DeclarationExport>) -> DeclarationShape {
        DeclarationShape {
            signature: [1; 32],
            exports,
            imports: vec!["/p/i.ts".into()],
        }
    }

    #[test]
    fn identical_contracts_are_preserved() {
        let old = shape(vec![export("a", 10), export("b", 20)]);
        let new = shape(vec![export("a", 11), export("b", 21)]);
        assert!(preserve_export_identities(&old, &new));
    }

    #[test]
    fn signature_import_or_slot_changes_are_not() {
        let old = shape(vec![export("a", 10)]);
        let mut changed = shape(vec![export("a", 10)]);
        changed.signature = [2; 32];
        assert!(!preserve_export_identities(&old, &changed));
        let mut imports = shape(vec![export("a", 10)]);
        imports.imports.push("/p/j.ts".into());
        assert!(!preserve_export_identities(&old, &imports));
        assert!(!preserve_export_identities(
            &old,
            &shape(vec![export("c", 10)])
        ));
        assert!(!preserve_export_identities(
            &old,
            &shape(vec![export("a", 10), export("b", 20)])
        ));
    }

    #[test]
    fn a_missing_symbol_and_a_non_bijective_pairing_fail_closed() {
        let old = shape(vec![export("a", 10), export("b", 20)]);
        assert!(!preserve_export_identities(
            &old,
            &shape(vec![export("a", 0), export("b", 21)])
        ));
        // Two export names reaching one target symbol under different ids.
        assert!(!preserve_export_identities(
            &old,
            &shape(vec![export("a", 30), export("b", 30)])
        ));
    }

    #[test]
    fn exported_ids_are_span_insensitive_and_match_go() {
        let first = export("value", 1);
        let mut moved = first.reference.clone();
        moved.start_byte = 900;
        assert_eq!(first.reference.exported_id(), moved.exported_id());
        assert_ne!(first.reference.id(), moved.id());
        assert!(first.id.starts_with("symbol:h:") && first.id.len() == 9 + 24);
    }
}
