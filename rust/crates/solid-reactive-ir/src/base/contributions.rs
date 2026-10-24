//! The contribution functions of the base indexes. Each has the shape
//! `fn(&FileView, &BuildConstants) -> Vec<(key, value)>`: it reads one file
//! and the build's constants, and produces its entries in a deterministic
//! order (never a hash map's iteration order).

use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentLiteralFact, ImportKind, ModuleHazardKind, ModuleLoadKind};
use solid_facts::core::Span;

use super::file_table::{FileView, ItemKind};
use crate::{HashMap, HashSet};

/// The build-wide inputs a contribution may read besides its file. A change
/// of any of them is a different cache domain, so a fresh build.
#[derive(Clone, Copy)]
pub(crate) struct BuildConstants<'a> {
    /// No contribution reads the vocabulary yet; it is here so one can
    /// without a lookup.
    #[allow(dead_code)]
    pub(crate) dialect: &'a dyn solid_dialect::Dialect,
}

fn ordinal(index: usize) -> u32 {
    u32::try_from(index).expect("fewer than 2^32 items in one file")
}

/// References by symbol: each TypeFacts reference located in this file.
pub(crate) fn references(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(Arc<str>, Span)> {
    view.references()
        .iter()
        .map(|(symbol, span)| (Arc::clone(symbol), *span))
        .collect()
}

/// A call or a JSX element, by ordinal in its file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum CalleeSite {
    Call(u32),
    Jsx(u32),
}

/// Calls by the symbol their callee resolves to ([`FileView::callee_symbol`]:
/// the value an identifier names, a member's resolved declaration; runtime
/// redirects aside), and JSX elements by the symbol that is the value of their
/// tag name ([`FileView::value_symbol`]): the sites the call graph and the
/// component census resolve ([`crate::indexes::CallSitesOf`]). A site whose
/// callee names no symbol contributes nothing here; the item index lists every
/// site.
pub(crate) fn callees(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(Arc<str>, CalleeSite)> {
    let file = view.file;
    let calls = file
        .ast
        .calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| {
            view.callee_symbol(call.callee)
                .map(|symbol| (Arc::from(symbol), CalleeSite::Call(ordinal(index))))
        });
    let jsx = file
        .ast
        .jsx_elements
        .iter()
        .enumerate()
        .filter_map(|(index, element)| {
            view.value_symbol(element.name.span)
                .map(|symbol| (symbol.shared(), CalleeSite::Jsx(ordinal(index))))
        });
    calls.chain(jsx).collect()
}

/// A module specifier as written, or none (a load whose argument is not a
/// literal, an export with no `from`).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ModuleKey {
    Specifier(Arc<str>),
    Opaque,
}

impl ModuleKey {
    pub(crate) fn specifier(&self) -> Option<&str> {
        match self {
            Self::Specifier(specifier) => Some(specifier),
            Self::Opaque => None,
        }
    }
}

/// One runtime module edge of a file: what loads or re-exports a module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModuleEdge {
    /// A runtime `import` declaration.
    Import {
        span: Span,
        /// Some runtime binding is a namespace (`* as ns`).
        namespace: bool,
        /// The runtime namespace bindings code uses other than by naming
        /// static members (ADR 0347): their local spans.
        escaping: Vec<Span>,
    },
    /// A runtime export with a `from` or a namespace binding.
    Export {
        span: Span,
        /// Outside every `namespace`/`module` block.
        module_level: bool,
        /// `export * as name from …`.
        namespace: bool,
        /// The exact exported namespace binding's span.
        namespace_binding: Option<Span>,
    },
    /// A `require()` or dynamic `import()`, by ordinal in `module_loads`.
    Load {
        ordinal: u32,
        span: Span,
        require: bool,
        /// Inside some function.
        in_function: bool,
        specifier_span: Option<Span>,
    },
    /// A runtime `import x = require(…)`.
    ImportEquals {
        span: Span,
        local: Span,
        /// Outside every `namespace`/`module` block.
        module_level: bool,
        /// Exported, or used other than by naming static members (ADR 0347).
        escapes: bool,
    },
}

/// Imports, re-exports, namespace and dynamic loads, by module specifier.
/// Entries come in four runs (imports, exports, loads, `import =`), each in
/// source-fact order, which is the order every census reads them in.
pub(crate) fn modules(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(ModuleKey, ModuleEdge)> {
    let file = view.file;
    let key = |module: &str| ModuleKey::Specifier(Arc::from(module));
    let mut entries = Vec::new();
    for import in file.ast.imports.iter().filter(|import| !import.type_only) {
        let namespaces = import
            .bindings
            .iter()
            .filter(|binding| !binding.type_only && binding.kind == ImportKind::Namespace);
        let namespace = namespaces.clone().next().is_some();
        let escaping = namespaces
            .filter(|binding| {
                !crate::attribution::namespace_used_by_static_members(file, binding.local.span)
            })
            .map(|binding| binding.local.span)
            .collect();
        entries.push((
            key(&import.module),
            ModuleEdge::Import {
                span: import.span,
                namespace,
                escaping,
            },
        ));
    }
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        let namespace_binding = export
            .namespace_binding
            .as_ref()
            .map(|binding| binding.span);
        if export.module.is_none() && namespace_binding.is_none() {
            continue;
        }
        entries.push((
            export.module.as_deref().map_or(ModuleKey::Opaque, key),
            ModuleEdge::Export {
                span: export.span,
                module_level: !file.ast.is_inside_module_block(export.span),
                namespace: export.namespace.is_some(),
                namespace_binding,
            },
        ));
    }
    for (index, load) in file.ast.module_loads.iter().enumerate() {
        entries.push((
            load.specifier.as_deref().map_or(ModuleKey::Opaque, key),
            ModuleEdge::Load {
                ordinal: ordinal(index),
                span: load.span,
                require: load.kind == ModuleLoadKind::Require,
                in_function: file
                    .ast
                    .functions
                    .iter()
                    .any(|function| function.span.contains(load.span)),
                specifier_span: load.specifier_span,
            },
        ));
    }
    for import in file
        .ast
        .import_equals
        .iter()
        .filter(|import| !import.type_only)
    {
        entries.push((
            key(&import.module),
            ModuleEdge::ImportEquals {
                span: import.span,
                local: import.local.span,
                module_level: !file
                    .ast
                    .module_blocks
                    .iter()
                    .any(|block| block.contains(import.span)),
                escapes: import.exported
                    || !crate::attribution::namespace_used_by_static_members(
                        file,
                        import.local.span,
                    ),
            },
        ));
    }
    entries
}

/// A member's property name, or none (a computed key no literal or constant
/// names).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum MemberName {
    Named(Arc<str>),
    Unnamed,
}

/// Why a member is listed under its name.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum MemberRole {
    /// An assignment or loop head writes the member under this name (ADR
    /// 0209): `x.name = …`, `x["name"] = …`, `[x.name] = …`.
    Assigned,
    /// A write or `delete` target ([`MemberStability`]'s syntax census).
    ///
    /// [`MemberStability`]: crate::interproc::accessor_provenance::MemberStability
    StabilityWrite,
    /// A static member read in a runtime value position: not a computed
    /// member, not a call's callee, not inside a type query (ADR 0347).
    ValueRead,
}

/// One member of a file, by ordinal in `members`, and its role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MemberUse {
    pub(crate) ordinal: u32,
    pub(crate) role: MemberRole,
}

/// Member reads, writes and assignments by name. Three runs, one per role,
/// each in the order its census reads it.
pub(crate) fn members(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(MemberName, MemberUse)> {
    let file = view.file;
    let named = |name: &str| MemberName::Named(Arc::from(name));
    let mut entries = Vec::new();
    for write in assignment_writes(file) {
        if let AssignmentWrite::Name(member, name) = write {
            entries.push((
                MemberName::Named(Arc::from(name)),
                MemberUse {
                    ordinal: member,
                    role: MemberRole::Assigned,
                },
            ));
        }
    }
    entries.extend(stability_writes(file));
    let callees = file
        .ast
        .calls
        .iter()
        .map(|call| file.ast.peel_ts_sugar_span(call.callee))
        .collect::<HashSet<_>>();
    for (index, member) in file.ast.members.iter().enumerate() {
        if file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
            || callees.contains(&member.span)
            || file
                .ast
                .type_queries
                .iter()
                .any(|query| query.contains(member.span))
        {
            continue;
        }
        entries.push((
            file.source_text(member.property)
                .map_or(MemberName::Unnamed, named),
            MemberUse {
                ordinal: ordinal(index),
                role: MemberRole::ValueRead,
            },
        ));
    }
    entries
}

/// The write and `delete` targets that are members, in member order, under
/// their names ([`MemberRole::StabilityWrite`]): `MemberStability`'s syntax
/// census. Normalized leaves exclude receiver Gets, destructuring
/// keys/defaults and reads inside computed keys; updates and loop targets are
/// included.
pub(crate) fn stability_writes(file: &FileFacts) -> Vec<(MemberName, MemberUse)> {
    let targets = file
        .ast
        .write_targets
        .iter()
        .copied()
        .chain(file.ast.deleted_targets.iter().copied())
        .map(|span| file.ast.peel_ts_sugar_span(span))
        .collect::<HashSet<_>>();
    let literal_keys = file
        .ast
        .literal_computed_members
        .iter()
        .map(|key| (key.span, key.key.as_str()))
        .collect::<HashMap<_, _>>();
    let mut entries = Vec::new();
    for (index, member) in file.ast.members.iter().enumerate() {
        if !targets.contains(&member.span) {
            continue;
        }
        let name = if file.ast.computed_members.contains(&member.span) {
            literal_keys.get(&member.span).copied()
        } else {
            file.source_text(member.property)
        };
        entries.push((
            name.map_or(MemberName::Unnamed, |name| {
                MemberName::Named(Arc::from(name))
            }),
            MemberUse {
                ordinal: ordinal(index),
                role: MemberRole::StabilityWrite,
            },
        ));
    }
    entries
}

/// What one assignment or loop-head target writes (ADR 0209, 0314).
pub(crate) enum AssignmentWrite {
    /// The member at this ordinal is written under this name.
    Name(u32, String),
    /// A write through some `prototype` whose owner nothing here proves.
    UnownedPrototype,
    /// A write through `R.prototype`, where `R` is the object of the member
    /// at this ordinal: whose instances it reaches is the owner proof's
    /// question ([`crate::indexes::SemanticLookup::assigned_members`]).
    PrototypeOwner(u32),
}

/// Every member write of the file's assignments and loop heads, in target
/// order: the per-file part of ADR 0209's assigned-member census.
pub(crate) fn assignment_writes(file: &FileFacts) -> Vec<AssignmentWrite> {
    let mut writes = Vec::new();
    let targets = file
        .ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.iteration_targets.iter().copied());
    for target in targets {
        let target = file.ast.peel_ts_sugar_span(target);
        let text = file.source_text(target).unwrap_or_default();
        // Every member written by the target: the target itself, or a leaf
        // of a destructuring pattern (`[c.run] = …`) or loop head (`for
        // (c.run of …)`). A member that is another written member's object
        // (`C.prototype` in `C.prototype.m = f`) is read, not written.
        let inside = file
            .ast
            .members
            .iter()
            .enumerate()
            .filter(|(_, member)| target.contains(member.span))
            .collect::<Vec<_>>();
        let mut written = inside
            .iter()
            .copied()
            .filter(|(_, member)| {
                !inside
                    .iter()
                    .any(|(_, outer)| file.ast.peel_ts_sugar_span(outer.object) == member.span)
            })
            .peekable();
        if written.peek().is_none() && text.contains("prototype") {
            writes.push(AssignmentWrite::UnownedPrototype);
        }
        for (index, member) in written {
            let index = ordinal(index);
            if file
                .ast
                .computed_members
                .binary_search(&member.span)
                .is_ok()
            {
                // A computed write names its key only when the key is a
                // literal, or a `const` bound to a string literal. Any other
                // key, on a prototype, may be any member.
                if let Some(literal) = file
                    .ast
                    .literal_computed_members
                    .iter()
                    .find(|literal| literal.span == member.span)
                {
                    writes.push(AssignmentWrite::Name(index, literal.key.to_string()));
                    if literal.key.as_str() == "prototype" {
                        writes.push(AssignmentWrite::UnownedPrototype);
                    }
                } else if let Some(key) = crate::indexes::constant_string_key(file, member.property)
                {
                    if key == "prototype" {
                        writes.push(AssignmentWrite::UnownedPrototype);
                    }
                    writes.push(AssignmentWrite::Name(index, key));
                } else if file
                    .source_text(member.object)
                    .unwrap_or_default()
                    .contains("prototype")
                {
                    // `R.prototype[key] = …`: narrowed to `R`'s instances
                    // when `R` is proven (ADR 0314).
                    let object = file.ast.peel_ts_sugar_span(member.object);
                    match file.ast.members.iter().position(|prototype_member| {
                        prototype_member.span == object
                            && file
                                .ast
                                .computed_members
                                .binary_search(&prototype_member.span)
                                .is_err()
                            && file.source_text(prototype_member.property) == Some("prototype")
                    }) {
                        Some(prototype_member) => {
                            writes.push(AssignmentWrite::PrototypeOwner(ordinal(prototype_member)));
                        }
                        None => writes.push(AssignmentWrite::UnownedPrototype),
                    }
                }
            } else if let Some(property) = file.source_text(member.property) {
                writes.push(AssignmentWrite::Name(index, property.to_string()));
                if property == "prototype" {
                    // `R.prototype = …` (ADR 0314).
                    writes.push(AssignmentWrite::PrototypeOwner(index));
                }
            }
        }
        // A dynamic key not spelled through a prototype (`c[key] = f` with
        // `key: keyof C`, or through a prototype alias) is not modeled. Like
        // `Object.defineProperty`, it is a reflective write this trust
        // boundary leaves out (ADR 0215): vetoing every dynamic write would
        // refuse every `xs[i] = v`.
    }
    writes
}

/// Module hazards and module-level flags. A boolean over the project is
/// "some file has an entry"; a read of the boolean does not move when only
/// the count does.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Hazard {
    /// A direct `eval` (value: ordinal in `module_hazards`).
    Eval,
    /// A load whose specifier is not a literal (ordinal in `module_hazards`).
    NonliteralDynamicLoad,
    /// A call TypeFacts resolves to `EventTarget.dispatchEvent` or
    /// `HTMLElement.click` (ADR 0356; ordinal in `calls`).
    SynchronousDispatch,
    /// A call TypeFacts resolves to a standard-library reflective mutator
    /// ([`crate::interproc::accessor_provenance::mutation_builtin_spelling`];
    /// ordinal in `calls`): a candidate the mutation proof then checks.
    MutationBuiltinCall,
    /// A call TypeFacts resolves to a standard-library `Function`
    /// constructor or `eval` (ordinal in `calls`), whatever its validity.
    CodeEvaluatingCall,
    /// A member, destructured property or callee spelled like a reflective
    /// definer (review round 1 of ADR 0293; value 0).
    ReflectiveDefinitionSpelling,
    /// An assignment writes through a `prototype` no owner proof narrows
    /// (ADR 0314; value 0).
    UnownedPrototypeWrite,
    /// An assignment writes through `R.prototype`, `R` the object of the
    /// member at this ordinal (ADR 0314).
    PrototypeOwnerSite,
    /// An eager `import.meta.glob` (value 0).
    InPlaceImportGlob,
    /// A `require` the facts do not model as a load: a bare call they did
    /// not record, or any other mention (value 0).
    UnmodeledRequire,
    /// A top-level `await` or `for await` (value 0).
    TopLevelAwait,
    /// Some export outside every `namespace` block (value 0).
    ModuleLevelExport,
    /// Some export at all (value 0).
    Export,
}

pub(crate) fn hazards(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(Hazard, u32)> {
    let file = view.file;
    let mut entries = Vec::new();
    for (index, hazard) in file.ast.module_hazards.iter().enumerate() {
        match hazard.kind {
            ModuleHazardKind::Eval => entries.push((Hazard::Eval, ordinal(index))),
            ModuleHazardKind::NonliteralDynamicLoading => {
                entries.push((Hazard::NonliteralDynamicLoad, ordinal(index)));
            }
            _ => {}
        }
    }
    for (index, call) in file.ast.calls.iter().enumerate() {
        let resolved = view.resolved_callee_call(call.callee);
        if resolved
            .and_then(crate::runtime_semantics::standard_call_name)
            .is_some_and(|name| matches!(name, "EventTarget.dispatchEvent" | "HTMLElement.click"))
        {
            entries.push((Hazard::SynchronousDispatch, ordinal(index)));
        }
        if crate::indexes::standard_library_declaration_of(resolved, typefacts::CallKind::Call)
            .is_some_and(|declaration| {
                crate::interproc::accessor_provenance::mutation_builtin_spelling(
                    declaration.qualified_name.as_ref(),
                )
                .is_some()
            })
        {
            entries.push((Hazard::MutationBuiltinCall, ordinal(index)));
        }
        if resolved
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                declaration.standard_library
                    && (declaration
                        .qualified_name
                        .starts_with("FunctionConstructor")
                        || declaration.qualified_name.as_ref() == "eval")
            })
        {
            entries.push((Hazard::CodeEvaluatingCall, ordinal(index)));
        }
    }
    if reflective_definition_spelled(file) {
        entries.push((Hazard::ReflectiveDefinitionSpelling, 0));
    }
    for write in assignment_writes(file) {
        match write {
            AssignmentWrite::Name(..) => {}
            AssignmentWrite::UnownedPrototype => entries.push((Hazard::UnownedPrototypeWrite, 0)),
            AssignmentWrite::PrototypeOwner(member) => {
                entries.push((Hazard::PrototypeOwnerSite, member));
            }
        }
    }
    if !file.ast.in_place_import_globs.is_empty() {
        entries.push((Hazard::InPlaceImportGlob, 0));
    }
    // A call of a bare `require` the facts did not record as a load (a
    // `declare function require` shim shadows the global in the binder,
    // though the host `require` is what runs), or any other mention of
    // `require` (`(0, require)(x)`, an alias, an argument).
    if file.ast.calls.iter().any(|call| {
        !call.construct
            && file.source_text(file.ast.peel_ts_sugar_span(call.callee)) == Some("require")
            && !file
                .ast
                .module_loads
                .iter()
                .any(|load| load.span == call.span)
    }) || !file.ast.unmodeled_require_references.is_empty()
    {
        entries.push((Hazard::UnmodeledRequire, 0));
    }
    let top_level = |span: Span| {
        !file
            .ast
            .functions
            .iter()
            .any(|function| function.span.contains(span))
    };
    if file
        .ast
        .awaits
        .iter()
        .chain(&file.ast.implicit_suspensions)
        .any(|span| top_level(*span))
    {
        entries.push((Hazard::TopLevelAwait, 0));
    }
    if file.ast.module_level_exports().next().is_some() {
        entries.push((Hazard::ModuleLevelExport, 0));
    }
    if !file.ast.exports.is_empty() {
        entries.push((Hazard::Export, 0));
    }
    entries
}

const DEFINERS: [&str; 3] = ["defineProperty", "defineProperties", "__defineGetter__"];

/// Review round 1 of ADR 0293: any member access, destructured property or
/// callee spelled `defineProperty`, `defineProperties` or `__defineGetter__`,
/// whatever its receiver.
fn reflective_definition_spelled(file: &FileFacts) -> bool {
    file.ast.members.iter().any(|member| {
        file.source_text(member.property)
            .is_some_and(|property| DEFINERS.contains(&property.trim_matches(['\'', '"', '`'])))
    }) || file.ast.bindings.iter().any(|binding| {
        binding
            .object_slots
            .iter()
            .any(|slot| DEFINERS.contains(&slot.property.as_str()))
    }) || file.ast.calls.iter().any(|call| {
        file.source_text(call.callee)
            .is_some_and(|callee| DEFINERS.iter().any(|name| callee.ends_with(name)))
    })
}

/// What a reflective definer spelling says about the key it may define
/// (ADR 0314): any key, or only this one.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DefinerKey {
    Any,
    Sole(Arc<str>),
}

/// The per-file part of [`crate::indexes::SemanticLookup::property_may_be_defined_reflectively_as`].
///
/// A definer spelled as a member is safe for `name` when some call of it
/// passes a string-literal key other than `name`; it therefore blocks `name`
/// only when every such call's key is `name` (or none passes a literal key),
/// so the members that block a single name are listed by that name and the
/// rest block every name. Every other spelling blocks every name. Sole keys
/// come sorted, `Any` at most once and first.
pub(crate) fn definers(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(DefinerKey, ())> {
    let file = view.file;
    let spelled = |text: &str| DEFINERS.contains(&text.trim_matches(['\'', '"', '`']));
    let mut any = file.ast.bindings.iter().any(|binding| {
        binding
            .object_slots
            .iter()
            .any(|slot| DEFINERS.contains(&slot.property.as_str()))
    });
    // For each definer-spelled member: whether it is `defineProperties`
    // (which never names one key) and the keys its calls pass.
    let mut definers: std::collections::BTreeMap<Span, (bool, HashSet<&str>)> =
        std::collections::BTreeMap::default();
    for member in &file.ast.members {
        if let Some(text) = file.source_text(member.property)
            && spelled(text)
        {
            definers.entry(member.span).or_insert_with(|| {
                (
                    text.trim_matches(['\'', '"', '`']) != "defineProperties",
                    HashSet::default(),
                )
            });
        }
    }
    let member_spans = file
        .ast
        .members
        .iter()
        .map(|member| member.span)
        .collect::<HashSet<_>>();
    for call in &file.ast.calls {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if let Some((names_one_key, keys)) = definers.get_mut(&callee)
            && *names_one_key
            && let Some((ArgumentLiteralFact::String(key), false)) = call
                .arguments
                .get(1)
                .map(|key| (&key.literal_value, key.spread))
            && !call.arguments.first().is_some_and(|target| target.spread)
        {
            keys.insert(key.as_str());
        }
        if !member_spans.contains(&callee)
            && file
                .source_text(call.callee)
                .is_some_and(|callee| DEFINERS.iter().any(|definer| callee.ends_with(definer)))
        {
            any = true;
        }
    }
    let mut sole = Vec::new();
    for (_, keys) in definers.into_values() {
        match keys.len() {
            0 => any = true,
            1 => sole.extend(keys.into_iter().map(Arc::<str>::from)),
            _ => {}
        }
    }
    sole.sort();
    any.then_some((DefinerKey::Any, ()))
        .into_iter()
        .chain(sole.into_iter().map(|key| (DefinerKey::Sole(key), ())))
        .collect()
}

/// Item key spaces: every function, call, JSX element and binding of the
/// file, by kind, in ordinal order.
pub(crate) fn items(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(ItemKind, Span)> {
    let table = super::file_table::FileItems::of(view.file);
    [
        ItemKind::Function,
        ItemKind::Call,
        ItemKind::Jsx,
        ItemKind::Binding,
    ]
    .into_iter()
    .flat_map(|kind| table.spans(kind).iter().map(move |span| (kind, *span)))
    .collect()
}
