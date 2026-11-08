//! The contribution functions of the base indexes. Each has the shape
//! `fn(&FileView, &BuildConstants) -> Vec<(key, value)>`: it reads one file
//! and the build's constants, and produces its entries in a deterministic
//! order (never a hash map's iteration order).

use std::collections::BTreeMap;
use std::sync::Arc;

use solid_facts::ast::{ArgumentLiteralFact, ImportKind, ModuleHazardKind, ModuleLoadKind};
use solid_facts::core::Span;
use solid_facts::{FileFacts, StandardRoot};

use super::file_table::{FileView, ItemKind};

mod census_proofs;
use crate::{HashMap, HashSet};
use census_proofs::{Proofs, TRUST_SITE, TRUSTED};

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

/// The spans of a file where the value of an entity symbol is used the way a
/// returned callback is (study S5e): a call's callee, a call's argument, a
/// JSX tag's name, the object of a `preload` member, or the identifier a
/// binding is initialized from. Keyed by the symbol of the entity at exactly
/// that span ([`FileView::symbol_at`]); the returned-callback use sites read
/// the index with TypeFacts' references of the same symbol
/// ([`crate::indexes::ReturnedUses`]).
pub(crate) fn use_spans(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(Arc<str>, Span)> {
    let file = view.file;
    let mut spans = Vec::new();
    for call in &file.ast.calls {
        spans.push(call.callee);
        spans.extend(call.arguments.iter().map(|argument| argument.span));
    }
    spans.extend(
        file.ast
            .jsx_elements
            .iter()
            .map(|element| element.name.span),
    );
    spans.extend(
        file.ast
            .members
            .iter()
            .filter(|member| file.source_text(member.property) == Some("preload"))
            .map(|member| member.object),
    );
    spans.extend(
        file.ast
            .bindings
            .iter()
            .filter_map(|binding| binding.initializer_identifier.as_ref())
            .map(|initializer| initializer.span),
    );
    spans.sort_unstable();
    spans.dedup();
    let mut entries = spans
        .into_iter()
        .filter_map(|span| view.symbol_at(span).map(|symbol| (symbol.shared(), span)))
        .collect::<Vec<_>>();
    entries.sort_unstable();
    entries
}

/// One use of an entity symbol in a file, as receiver censuses read it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SymbolUse {
    /// An identifier in a reference role whose entity is the symbol.
    Reference(Span),
    /// An import binding whose local name's entity is the symbol; `imported`
    /// is false for a default or namespace-like binding with no imported name.
    ImportLocal { local: Span, imported: bool },
    /// A reference to the symbol that the binder resolves to this named
    /// import binding.
    BoundReference { bound: Span },
}

/// Uses of entity symbols, by the symbol: the per-file part of the receiver
/// closure census (`object_members`). Entries come in three runs (references,
/// import bindings, bound references), each in source-fact order.
pub(crate) fn symbol_uses(
    view: &FileView<'_>,
    _: &BuildConstants<'_>,
) -> Vec<(Arc<str>, SymbolUse)> {
    let file = view.file;
    let mut entries = Vec::new();
    for id in &file.ast.identifiers {
        if id.role == solid_facts::ast::IdentifierRole::Reference
            && let Some(symbol) = view.symbol_at(id.span)
        {
            entries.push((symbol.shared(), SymbolUse::Reference(id.span)));
        }
    }
    for import in &file.ast.imports {
        for binding in &import.bindings {
            if let Some(symbol) = view.symbol_at(binding.local.span) {
                entries.push((
                    symbol.shared(),
                    SymbolUse::ImportLocal {
                        local: binding.local.span,
                        imported: binding.imported.is_some(),
                    },
                ));
            }
        }
    }
    for (reference, bound) in &file.ast.reference_declarations {
        if let Some(symbol) = view.symbol_at(*reference)
            && file.ast.imports.iter().any(|import| {
                import
                    .bindings
                    .iter()
                    .any(|binding| binding.local.span == *bound && binding.imported.is_some())
            })
        {
            entries.push((symbol.shared(), SymbolUse::BoundReference { bound: *bound }));
        }
    }
    entries
}

/// The names of the global bindings this file's assignments write, each
/// once: the per-file part of whether some assignment writes the global
/// binding of a name (`object_members`' host check).
pub(crate) fn global_writes(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(Arc<str>, ())> {
    let mut names = written_global_identifiers(view.file).collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .map(|name| (Arc::from(name), ()))
        .collect()
}

/// The identifiers an assignment or iteration target of `file` writes that
/// no declaration binds: the whole target, or a leaf of a destructuring
/// pattern. An identifier inside a member target or a type assertion
/// (`(x as HTMLCanvasElement).width = …`) writes no binding.
pub(crate) fn written_global_identifiers(
    file: &solid_facts::FileFacts,
) -> impl Iterator<Item = &str> + '_ {
    file.ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.iteration_targets.iter().copied())
        .flat_map(move |target| {
            let target = file.ast.peel_ts_sugar_span(target);
            let pattern = file
                .source_text(target)
                .is_some_and(|text| text.starts_with(['[', '{']));
            file.ast.identifiers.iter().filter_map(move |identifier| {
                ((identifier.span == target || (pattern && target.contains(identifier.span)))
                    && file.ast.reference_declaration(identifier.span).is_none())
                .then(|| file.source_text(identifier.span))
                .flatten()
            })
        })
}

/// How a name is spelled as a property.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Spelling {
    /// A static member access whose property is spelled this way (ordinal in
    /// `members`; not a computed member).
    Member(u32),
    /// A computed member with this literal key, or an object slot with this
    /// property name.
    Key,
}

/// Property spellings, by name: every static member access, and every
/// literal computed key and destructured slot.
pub(crate) fn spellings(view: &FileView<'_>, _: &BuildConstants<'_>) -> Vec<(Arc<str>, Spelling)> {
    let file = view.file;
    let mut entries = Vec::new();
    for (index, member) in file.ast.members.iter().enumerate() {
        if file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_err()
            && let Some(text) = file.source_text(member.property)
        {
            entries.push((Arc::from(text), Spelling::Member(ordinal(index))));
        }
    }
    for literal in &file.ast.literal_computed_members {
        entries.push((Arc::from(literal.key.as_str()), Spelling::Key));
    }
    for binding in &file.ast.bindings {
        for slot in &binding.object_slots {
            entries.push((Arc::from(slot.property.as_str()), Spelling::Key));
        }
    }
    entries
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
        let inside =
            file.ast
                .members
                .iter()
                .enumerate()
                // ADR 0874: `document.getElementById("x")!.title = v` writes `title`,
                // and a member a call in the target holds is only read.
                .filter(|(_, member)| {
                    target.contains(member.span)
                        && !file.ast.calls.iter().any(|call| {
                            target.contains(call.span) && call.span.contains(member.span)
                        })
                })
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
    /// A class extending a standard container declares this member (ADR 0683;
    /// the key is a hash of `Container.member`, value the class's ordinal): the
    /// container's own method may be the class's override. A collision only
    /// withholds another proof.
    StandardOverride(u32),
    /// A write to a standard global or to a member of a standard prototype or
    /// constructor (ADR 0781; the key is a hash of `Owner.member`, the owner
    /// empty for a global): the call the types name may run the project's
    /// function. A collision only withholds another proof.
    StandardPatch(u32),
    /// A file that is no module declares this `__` name (ADR 0803 step 2, review
    /// D24; the key is a hash of the name): a classic script's top level is the
    /// global object. Any depth counts; a collision only withholds a proof.
    ClassicScriptDunder(u32),
    /// Some [`Self::StandardPatch`] in the file (value 0), so a call that
    /// meets none hashes nothing.
    AnyStandardPatch,
    /// A top-level `await` or `for await` (value 0).
    TopLevelAwait,
    /// Some export outside every `namespace` block (value 0).
    ModuleLevelExport,
    /// Some export at all (value 0).
    Export,
    /// The file obtains an array intrinsic or code from a string as a value
    /// (ADR 0801, [`crate::interproc::accessor_provenance::intrinsic_census`];
    /// value 0).
    ArrayIntrinsicObtained,
    /// Code may put a function at this member of an object no standard
    /// constructor made (ADR 0840; the key is a hash of the member, `*` for a
    /// key no fact names; the value is the site's ordinal in the file's
    /// census): a value typed as a standard container may be that impostor.
    /// A collision only withholds another proof.
    StandardImpostor(u32),
    /// ADR 0874: code may write this member (the same hash) of an object it
    /// did not make here: a host global, which every spelling reaches, may be
    /// that object.
    StandardImpostorWrite(u32),
    /// A call receives an obtained value (review a9fc #1 step 2; the value
    /// encodes the call's ordinal, the value's kind and the argument,
    /// [`crate::interproc::accessor_provenance::intrinsic_census`]): the
    /// project decides whether its callee is seen code.
    IntrinsicHandedAtCall,
    /// A span where a value may be a window (review R12; the value indexes
    /// [`solid_facts::FileFacts::window_valued_candidates`]): the project
    /// asks its type.
    WindowValuedCandidate,
}

const STANDARD_CONTAINERS: [&str; 6] = ["Array", "Map", "Set", "WeakMap", "WeakSet", "Promise"];
/// The member a class declares under a key no fact reads: any of the container's.
const ANY_MEMBER: &str = "*";

/// FNV-1a over `Container.member`: stable across builds, as a key must be.
fn member_hash(container: &str, member: &str) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for byte in [container, ".", member]
        .iter()
        .flat_map(|part| part.bytes())
    {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
    }
    hash
}

fn standard_override(container: &str, member: &str) -> Hazard {
    Hazard::StandardOverride(member_hash(container, member))
}

/// [`Hazard::ClassicScriptDunder`] of `name`.
pub(crate) fn classic_script_dunder(name: &str) -> Hazard {
    Hazard::ClassicScriptDunder(member_hash("", name))
}

fn standard_patch(owner: &str, member: &str) -> Hazard {
    Hazard::StandardPatch(member_hash(owner, member))
}

fn standard_impostor(member: &str) -> Hazard {
    Hazard::StandardImpostor(member_hash("", member))
}

fn standard_impostor_write(member: &str) -> Hazard {
    Hazard::StandardImpostorWrite(member_hash("", member))
}

fn push_patch(root: &StandardRoot, member: &str, patches: &mut Vec<Hazard>) {
    match root {
        StandardRoot::Global => patches.push(standard_patch("", member)),
        StandardRoot::Prototype(name) => patches.push(standard_patch(name, member)),
        StandardRoot::Constructor(name) => {
            patches.push(standard_patch(name, member));
            patches.push(standard_patch(&format!("{name}Constructor"), member));
            // ADR 0874: `document` is the `Document` a qualified name says.
            if name == "document" {
                patches.push(standard_patch("Document", member));
            }
        }
    }
}

/// The owner of a patch reached through a prototype no spelling names (ADR
/// 0876): any owner's. `OWN_PROTOTYPE`'s only while the program writes some
/// `prototype` key: `PROTOTYPE_KEY` records that, as this file cannot see it.
const REACHED_OWNER: &str = "*";
const OWN_PROTOTYPE: &str = "own-prototype";
const PROTOTYPE_KEY: &str = "prototype-key";

/// How the object at `span` of a write reaches a prototype (ADR 0876).
#[derive(Clone, Copy, PartialEq, Eq)]
enum PrototypeReach {
    No,
    /// Some prototype, by a chain no fact pins to the program's own.
    Reaches,
    /// `Foo.prototype`, where `Foo` is a function or class of this file that
    /// nothing rebinds: the program's own, while no `prototype` key is written.
    OwnPrototype,
}

/// The text at a callee that reads a prototype, or an unknown member of the
/// objects that do. A false hit only keeps a proof from standing.
fn reads_prototype(text: &str) -> bool {
    ["getPrototypeOf", "getOwnPropertyDescriptor", "Reflect.get"]
        .iter()
        .any(|name| text.contains(name))
        || text.starts_with("Object[")
        || text.starts_with("Reflect[")
        || text.contains("(Object as any)[")
        || text.contains("(Reflect as any)[")
}

/// ADR 0876: whether the written object at `span` reaches a prototype by its
/// own chain: a `prototype` or `__proto__` member, or a call that reads one,
/// through assertions, members and the bindings that hold it, a `let` by each
/// of its assignments. A value that leaves the expression is no such chain.
fn prototype_reach(file: &FileFacts, span: Span) -> PrototypeReach {
    prototype_reach_within(file, span, true, 16)
}

fn prototype_reach_within(
    file: &FileFacts,
    span: Span,
    immediate: bool,
    depth: u8,
) -> PrototypeReach {
    let ast = &file.ast;
    // Review R58: a chain the walk cannot finish proves no absence of a reach.
    if depth == 0 {
        return PrototypeReach::Reaches;
    }
    let value = ast.peel_ts_sugar_span(span);
    let walk =
        |next: Span, immediate: bool| prototype_reach_within(file, next, immediate, depth - 1);
    if let Some(member) = ast.member_at(value) {
        let computed = ast.computed_members.binary_search(&member.span).is_ok();
        let key = if computed {
            ast.literal_computed_members
                .iter()
                .find(|literal| literal.span == member.span)
                .map(|literal| literal.key.to_string())
        } else {
            file.source_text(member.property).map(str::to_owned)
        };
        return match key.as_deref() {
            Some("__proto__") => PrototypeReach::Reaches,
            Some("prototype") if immediate && own_function_or_class(file, member.object) => {
                PrototypeReach::OwnPrototype
            }
            Some("prototype") => PrototypeReach::Reaches,
            _ => match walk(member.object, false) {
                // A member of `Foo.prototype` may hold anything.
                PrototypeReach::OwnPrototype => PrototypeReach::Reaches,
                reach => reach,
            },
        };
    }
    if let Some(call) = ast.call_at(value) {
        let callee = ast.peel_ts_sugar_span(call.callee);
        let held = ast
            .reference_declaration(callee)
            .and_then(|declaration| ast.identifier_binding(declaration))
            .and_then(|binding| binding.initializer);
        let destructured = ast
            .reference_declaration(callee)
            .and_then(|declaration| {
                ast.bindings.iter().find(|binding| {
                    binding.names.iter().any(|name| name.span == declaration)
                        && binding.shape != solid_facts::ast::BindingShape::Identifier
                })
            })
            .and_then(|binding| binding.initializer)
            .and_then(|initializer| file.source_text(ast.peel_ts_sugar_span(initializer)))
            .is_some_and(|text| matches!(text.trim(), "Object" | "Reflect"));
        let reads = |span: Span| file.source_text(span).is_some_and(reads_prototype);
        return if destructured || reads(callee) || held.is_some_and(reads) {
            PrototypeReach::Reaches
        } else {
            PrototypeReach::No
        };
    }
    let Some(declaration) = ast.reference_declaration(value) else {
        return PrototypeReach::No;
    };
    let held = ast
        .identifier_binding(declaration)
        .and_then(|binding| binding.initializer);
    let assigned = ast
        .assignments
        .iter()
        .filter(|assignment| {
            !assignment.reads_target
                && ast.reference_declaration(ast.peel_ts_sugar_span(assignment.target))
                    == Some(declaration)
        })
        .map(|assignment| assignment.value_span);
    // Review R46: an assignment that reaches decides, whatever the initializer
    // held. Only a binding that holds `Foo.prototype` by its initializer
    // alone is that prototype.
    if assigned
        .into_iter()
        .any(|value| walk(value, false) != PrototypeReach::No)
    {
        return PrototypeReach::Reaches;
    }
    held.map_or(PrototypeReach::No, |held| walk(held, immediate))
}

/// Whether the root at `span` names a function or class this file declares
/// and nothing rebinds. An import or a parameter is not one.
fn own_function_or_class(file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let Some(declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(span)) else {
        return false;
    };
    !crate::indexes::binding_written(file, declaration)
        && (ast.functions.iter().any(|function| {
            function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }) || ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }))
}

/// Pushes the patch of a write to `key` of the object at `object`: of the
/// standard root it names, or (ADR 0876) of the prototype its chain reaches.
fn push_written_patch(file: &FileFacts, object: Span, key: &str, patches: &mut Vec<Hazard>) {
    if let Some(root) = file.standard_root(object) {
        push_patch(&root, key, patches);
        return;
    }
    match prototype_reach(file, object) {
        PrototypeReach::No => {}
        PrototypeReach::Reaches => patches.push(standard_patch(REACHED_OWNER, key)),
        PrototypeReach::OwnPrototype => patches.push(standard_patch(OWN_PROTOTYPE, key)),
    }
}

/// The definers that write a named key of their first argument, and those
/// that write the keys of their second.
const KEY_DEFINERS: [&str; 3] = [
    "Object.defineProperty",
    "Reflect.defineProperty",
    "Reflect.set",
];
const SHAPE_DEFINERS: [&str; 2] = ["Object.defineProperties", "Object.assign"];

/// ADR 0904: the names a classic script (no import or export) declares at its
/// top level: a `var` or function becomes the global object's property, a
/// `let`, `const` or class the global lexical binding every module resolves
/// through, so each may replace a standard global. A declaration file and a
/// `declare global` block run nothing. A name a block or a class expression
/// scopes is counted too: a patch more, never one less.
fn classic_script_globals(file: &FileFacts) -> Vec<&str> {
    let path = file.path.as_str();
    if crate::execution_role::file_is_module(file)
        || [".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|suffix| path.ends_with(suffix))
    {
        return Vec::new();
    }
    let ast = &file.ast;
    let top = |span: Span| {
        // A function's own name is in its span, and outside its scope.
        ast.functions_containing(span)
            .all(|function| function.name.as_ref().is_some_and(|name| name.span == span))
            && !ast.module_blocks.iter().any(|block| block.contains(span))
    };
    let mut names = ast
        .function_declarations
        .iter()
        .filter(|declaration| declaration.has_body)
        .map(|declaration| declaration.name.span)
        .chain(
            ast.classes
                .iter()
                .filter_map(|class| class.name.as_ref().map(|name| name.span)),
        )
        .chain(
            ast.bindings
                .iter()
                .flat_map(|binding| binding.names.iter().map(|name| name.span)),
        )
        .chain(ast.namespace_declarations.iter().map(|name| name.span))
        .chain(declared_by_keyword(file))
        .filter(|span| top(*span))
        .filter_map(|span| file.source_text(span).map(str::trim))
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

/// ADR 0906 (review of ADR 0904): the names a TypeScript `enum` or an
/// `import X = A.B` declares, which no fact records: each is a runtime `var`.
/// Read from the text, `enum` or `import` then a name (then `=`), so a match in
/// a comment or a string is a patch more, never one less.
fn declared_by_keyword(file: &FileFacts) -> Vec<Span> {
    let source: &str = &file.source;
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut names = Vec::new();
    for keyword in ["enum", "import"] {
        for (at, _) in source.match_indices(keyword) {
            if source[..at].chars().next_back().is_some_and(word) {
                continue;
            }
            let rest = &source[at + keyword.len()..];
            let name_at = rest.len() - rest.trim_start().len();
            if name_at == 0 {
                continue;
            }
            let name = &rest[name_at..];
            let length = name.find(|c: char| !word(c)).unwrap_or(name.len());
            if length == 0 {
                continue;
            }
            let after = name[length..].trim_start();
            if keyword == "import" && !(after.starts_with('=') && !after.starts_with("==")) {
                continue;
            }
            let start = at + keyword.len() + name_at;
            if let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(start + length)) {
                names.push(Span::new(start, end));
            }
        }
    }
    names
}

/// Every write to a standard global, prototype or constructor member in the
/// file: an assignment, or a definer's call on one.
fn standard_patches(view: &FileView<'_>, writes: &[AssignmentWrite]) -> Vec<Hazard> {
    let file = view.file;
    let ast = &file.ast;
    let mut patches = Vec::new();
    for write in writes {
        match write {
            AssignmentWrite::Name(member, key) => {
                let object = ast.members[*member as usize].object;
                if key == "prototype" {
                    patches.push(standard_patch(PROTOTYPE_KEY, key));
                }
                push_written_patch(file, object, key, &mut patches);
            }
            AssignmentWrite::PrototypeOwner(member) => {
                let prototype = ast.members[*member as usize].span;
                if let Some(root) = file.standard_root(prototype) {
                    push_patch(&root, ANY_MEMBER, &mut patches);
                }
            }
            AssignmentWrite::UnownedPrototype => {}
        }
    }
    // `scrollTo = f`: the global's own binding.
    patches.extend(written_global_identifiers(file).map(|name| standard_patch("", name)));
    // ADR 0904: `var Math`, `function parseInt`, `let Map` at a classic
    // script's top level: the global's own binding, for every script and
    // module.
    patches.extend(
        classic_script_globals(file)
            .into_iter()
            .map(|name| standard_patch("", name)),
    );
    // `globalThis[key] = f`: a key no fact reads.
    for assignment in &ast.assignments {
        let target = ast.peel_ts_sugar_span(assignment.target);
        // The root first: most computed writes are `xs[i] = v`, and the tests
        // after it scan the file's tables.
        if ast.computed_members.binary_search(&target).is_ok()
            && let Some(member) = ast.member_at(target)
            && !ast
                .literal_computed_members
                .iter()
                .any(|literal| literal.span == target)
            && crate::indexes::constant_string_key(file, member.property).is_none()
            && !symbol_key(file, member.property)
        {
            // A key the types pin: an imported `const`.
            let key = view
                .entity_at(member.property)
                .and_then(|entity| entity.constant_value.as_ref())
                .and_then(constant_key);
            let key = key.unwrap_or(ANY_MEMBER);
            if key == ANY_MEMBER || key == "prototype" {
                patches.push(standard_patch(PROTOTYPE_KEY, "prototype"));
            }
            push_written_patch(file, member.object, key, &mut patches);
        }
    }
    // ADR 0876: `delete` uncovers what the prototype chain holds next.
    for target in &ast.deleted_targets {
        if let Some(member) = ast.member_at(ast.peel_ts_sugar_span(*target)) {
            let key = if ast.computed_members.binary_search(&member.span).is_ok() {
                ANY_MEMBER
            } else {
                file.source_text(member.property).unwrap_or(ANY_MEMBER)
            };
            push_written_patch(file, member.object, key, &mut patches);
        }
    }
    for call in &ast.calls {
        let Some(callee) = file.source_text(ast.peel_ts_sugar_span(call.callee)) else {
            continue;
        };
        let callee = callee.trim();
        let spelled = |names: &[&str]| names.iter().any(|name| spelled_callee(callee, name));
        let Some(target) = call.arguments.first().map(|target| target.span) else {
            continue;
        };
        // ADR 0876: a new prototype replaces every member at once.
        if spelled(&["Object.setPrototypeOf", "Reflect.setPrototypeOf"]) {
            push_written_patch(file, target, ANY_MEMBER, &mut patches);
            continue;
        }
        let by_key = spelled(&KEY_DEFINERS);
        if !by_key && !spelled(&SHAPE_DEFINERS) {
            continue;
        }
        let Some(argument) = call.arguments.get(1) else {
            continue;
        };
        if by_key {
            let quoted = file.source_text(argument.span).and_then(|text| {
                let inner = text.strip_prefix(['\'', '"'])?.strip_suffix(['\'', '"'])?;
                Some(inner.to_owned())
            });
            let literal =
                quoted.or_else(|| crate::indexes::constant_string_key(file, argument.span));
            let key = literal.as_deref().unwrap_or(ANY_MEMBER);
            if key == ANY_MEMBER || key == "prototype" {
                patches.push(standard_patch(PROTOTYPE_KEY, "prototype"));
            }
            push_written_patch(file, target, key, &mut patches);
        } else if argument.exact_object_literal {
            for key in &argument.property_names {
                let key = file.source_text(*key).unwrap_or(ANY_MEMBER);
                // Review R45: `Object.assign(Foo, { prototype: … })`.
                if key.trim_matches(['\'', '"', '`']) == "prototype" {
                    patches.push(standard_patch(PROTOTYPE_KEY, "prototype"));
                }
                // ADR 0874: what `replace` dispatches through (`symbol_key`).
                let key = if key.contains("Symbol.replace") {
                    ANY_MEMBER
                } else {
                    key
                };
                push_written_patch(
                    file,
                    target,
                    key.trim_matches(['\'', '"', '`']),
                    &mut patches,
                );
            }
        } else {
            patches.push(standard_patch(PROTOTYPE_KEY, "prototype"));
            push_written_patch(file, target, ANY_MEMBER, &mut patches);
        }
    }
    patches
}

/// Whether a computed key is a symbol that names no member: one the global
/// `Symbol` makes on the spot. A program that replaces `Symbol` reopens each
/// such site ([`SYMBOL_SITE`]): this file cannot see that.
fn symbol_key(file: &FileFacts, key: Span) -> bool {
    symbol_made_at(file, file.ast.peel_ts_sugar_span(key)) || const_symbol_key(file, key)
}

/// ADR 0877: the key at `key` names a `const` of this file holding what
/// [`symbol_made_at`] accepts at its initializer. `Symbol` replaced is
/// `symbol_open`'s, as for the same call written in place.
fn const_symbol_key(file: &FileFacts, key: Span) -> bool {
    let ast = &file.ast;
    ast.reference_declaration(ast.peel_ts_sugar_span(key))
        .and_then(|declaration| ast.identifier_binding(declaration))
        .filter(|binding| binding.immutable)
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| symbol_made_at(file, ast.peel_ts_sugar_span(initializer)))
}

/// The well-known symbols no census member reads. Not `replace`, through which
/// `replace` and `replaceAll` dispatch (ADR 0874), and not `species`, through
/// which `map`, `filter` and `then` build the value a later member is called on.
const MEMBERLESS_SYMBOLS: [&str; 13] = [
    "iterator",
    "asyncIterator",
    "hasInstance",
    "isConcatSpreadable",
    "match",
    "matchAll",
    "search",
    "split",
    "toPrimitive",
    "toStringTag",
    "unscopables",
    "dispose",
    "asyncDispose",
];

/// Exactly `Symbol(…)` or `Symbol.for(…)` of the global `Symbol`, or a listed
/// well-known symbol of it, at `at`: `Symbol("m").description` is a string.
fn symbol_made_at(file: &FileFacts, at: Span) -> bool {
    let ast = &file.ast;
    let global_symbol = |span: Span| {
        let span = ast.peel_ts_sugar_span(span);
        file.source_text(span).map(str::trim) == Some("Symbol")
            && ast.reference_declaration(span).is_none()
    };
    if let Some(call) = ast.call_at(at) {
        let callee = ast.peel_ts_sugar_span(call.callee);
        return !call.construct
            && (global_symbol(callee)
                || ast.member_at(callee).is_some_and(|member| {
                    global_symbol(member.object)
                        && ast.computed_members.binary_search(&member.span).is_err()
                        && file.source_text(member.property) == Some("for")
                }));
    }
    ast.member_at(at).is_some_and(|member| {
        global_symbol(member.object)
            && ast.computed_members.binary_search(&member.span).is_err()
            && file
                .source_text(member.property)
                .is_some_and(|name| MEMBERLESS_SYMBOLS.contains(&name))
    })
}

fn constant_key(value: &typefacts::ConstantValue) -> Option<&str> {
    (value.kind == typefacts::ConstantValueKind::String).then_some(value.string.as_ref())
}

/// The patches that replace the standard function `qualified_name` names: its
/// member, any member of its owner, the global of its name, and the owner's
/// global (`globalThis.Promise = P`). A global written under a key no fact
/// reads may be any function or constructor, and reaches a method only through
/// a value that constructor made: that is the last entry's limit, and
/// docs/open-work.md's.
pub(crate) fn patch_veto(qualified_name: &str) -> Vec<Hazard> {
    let (owner, member) = qualified_name
        .rsplit_once('.')
        .unwrap_or(("", qualified_name));
    let container = owner.strip_prefix("Readonly").unwrap_or(owner);
    // A declaration's interface is not the global it patches: `then` of
    // `PromiseLike` is `Promise`'s, `call` of `CallableFunction` `Function`'s.
    let container = match container {
        "PromiseLike" => "Promise",
        "CallableFunction" | "NewableFunction" => "Function",
        container => container,
    };
    let mut hazards = vec![
        standard_patch(container, member),
        standard_patch(container, ANY_MEMBER),
        standard_patch("", member),
        standard_patch(
            "",
            container.strip_suffix("Constructor").unwrap_or(container),
        ),
        // The window's own members live on the global: `window[k] = f` may be
        // any of them (review D21).
        if owner.is_empty()
            || owner.ends_with("Constructor")
            || matches!(
                container,
                "Window" | "WindowOrWorkerGlobalScope" | "AnimationFrameProvider"
            )
        {
            standard_patch("", ANY_MEMBER)
        } else {
            standard_patch(container, ANY_MEMBER)
        },
    ];
    // ADR 0874: a host object's member is its prototype chain's.
    if EVENT_TARGETS.contains(&container) || container.ends_with("Element") {
        hazards.extend(EVENT_TARGETS.map(|target| standard_patch(target, member)));
    }
    // ADR 0876: a write a prototype chain reaches may be any owner's. A bare
    // global (`setTimeout`) is no prototype's member; a `*Constructor` is
    // reachable (`gp.constructor.resolve = f`).
    if !owner.is_empty() {
        hazards.push(standard_patch(REACHED_OWNER, member));
        hazards.push(standard_patch(REACHED_OWNER, ANY_MEMBER));
    }
    hazards
}

/// ADR 0876: whether a write a prototype chain reaches may have replaced the
/// member of `qualified_name`, with no patch naming that owner.
pub(crate) fn reached_patch(qualified_name: &str, hazard: &impl Fn(Hazard) -> bool) -> bool {
    let Some((owner, member)) = qualified_name.rsplit_once('.') else {
        return false;
    };
    !owner.is_empty()
        && hazard(Hazard::AnyStandardPatch)
        && (hazard(standard_patch(REACHED_OWNER, member))
            || hazard(standard_patch(REACHED_OWNER, ANY_MEMBER)))
}

/// ADR 0781: whether a patch replaces the standard `qualified_name`
/// ([`patch_veto`]), and ADR 0876: a write to a project's own `prototype`,
/// once some `prototype` key is written and that may be anyone's.
pub(crate) fn patch_vetoed(qualified_name: &str, hazard: &impl Fn(Hazard) -> bool) -> bool {
    if !hazard(Hazard::AnyStandardPatch) {
        return false;
    }
    let member = qualified_name
        .rsplit_once('.')
        .map_or(qualified_name, |(_, member)| member);
    patch_veto(qualified_name).into_iter().any(hazard)
        || (qualified_name.contains('.')
            && hazard(standard_patch(PROTOTYPE_KEY, "prototype"))
            && (hazard(standard_patch(OWN_PROTOTYPE, member))
                || hazard(standard_patch(OWN_PROTOTYPE, ANY_MEMBER))))
}

/// The interfaces a window, a document or an element takes a member from.
const EVENT_TARGETS: [&str; 6] = [
    "EventTarget",
    "Node",
    "Element",
    "HTMLElement",
    "Document",
    "Window",
];

/// Whether a call's written type rests on an assertion nothing proves
/// (ADR 0781): the value under it is no literal or `new` of the standard owner,
/// and its own type is not that owner alone, or for a method of no container
/// any one library type (`stops[0] as HTMLElement`, a downcast whose object is
/// the library's). `(t as unknown as number[]).forEach(cb)` runs whatever `t`
/// holds.
pub(crate) fn unproven_assertion<'e>(
    file: &solid_facts::FileFacts,
    callee: Span,
    qualified_name: &str,
    entity_at: impl Fn(Span) -> Option<&'e typefacts::EntityFact>,
) -> bool {
    let Some(value) = file.asserted_call_value(callee) else {
        return false;
    };
    let ast = &file.ast;
    let owner = qualified_name
        .rsplit_once('.')
        .map_or("", |(owner, _)| owner);
    let owner = owner.strip_prefix("Readonly").unwrap_or(owner);
    let value = ast.peel_ts_sugar_span(value);
    let member = qualified_name.rsplit('.').next().unwrap_or(qualified_name);
    if (owner.is_empty()
        && file.source_text(value) == Some(member)
        && ast.reference_declaration(value).is_none())
        || (owner == "Array"
            && file
                .source_text(value)
                .is_some_and(|text| text.starts_with('[')))
        || ast.call_at(value).is_some_and(|call| {
            let constructor = ast.peel_ts_sugar_span(call.callee);
            call.construct
                && file.source_text(constructor) == Some(owner)
                && ast.reference_declaration(constructor).is_none()
        })
    {
        return false;
    }
    let Some(entity) = entity_at(value) else {
        return true;
    };
    let container = STANDARD_CONTAINERS.contains(&owner) || owner == "PromiseLike";
    !entity.library_types.as_deref().is_some_and(|types| {
        !types
            .iter()
            .any(|name| name.as_ref() == typefacts::LIBRARY_TYPE_OTHER)
            && (!container && !owner.is_empty() && !types.is_empty()
                || types
                    .iter()
                    .any(|name| name.strip_prefix("Readonly").unwrap_or(name) == owner))
    })
}

/// The container an `extends` names: the global, a `globalThis.` member of it,
/// or a `const` alias of one. An import or any other binding names no container.
fn subclassed_container(file: &solid_facts::FileFacts, heritage: Span) -> Option<&'static str> {
    let peeled = file.ast.peel_ts_sugar_span(heritage);
    let text = file.source_text(peeled)?.trim();
    let root = text.split('<').next()?.trim();
    let root = ["globalThis.", "window.", "self."]
        .iter()
        .find_map(|prefix| root.strip_prefix(prefix))
        .unwrap_or(root);
    let named = |name: &str| STANDARD_CONTAINERS.into_iter().find(|known| *known == name);
    let Some(declaration) = file.ast.reference_declaration(peeled) else {
        return named(root);
    };
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable && binding.names.iter().any(|name| name.span == declaration)
    })?;
    named(file.source_text(binding.initializer?)?.trim())
}

/// The hazards a member of a class extending a standard container raises: its
/// own name, or any name where its key is computed.
fn class_overrides(
    file: &solid_facts::FileFacts,
    class: &solid_facts::ast::ClassFact,
) -> Vec<Hazard> {
    let Some(container) = class
        .heritage
        .and_then(|heritage| subclassed_container(file, heritage))
    else {
        return Vec::new();
    };
    use solid_facts::ast::ClassElementKind;
    class
        .elements
        .iter()
        .filter(|element| {
            !element.r#static
                && matches!(
                    element.kind,
                    ClassElementKind::Method
                        | ClassElementKind::Getter
                        | ClassElementKind::Setter
                        | ClassElementKind::Field
                )
        })
        .filter_map(|element| {
            if element.computed {
                return Some(standard_override(container, ANY_MEMBER));
            }
            let key = file.source_text(element.key?)?;
            Some(standard_override(container, key))
        })
        .collect()
}

/// Whether the receiver of `callee` is, by a fact of this file, no instance of
/// a subclass of `container`: an array literal, or a `new` of the global
/// `container` itself, on the spot or held by a binding nothing rebinds.
fn plain_container_receiver(file: &solid_facts::FileFacts, callee: Span, container: &str) -> bool {
    let ast = &file.ast;
    let Some(member) = ast.member_at(ast.peel_ts_sugar_span(callee)) else {
        return false;
    };
    let receiver = ast.peel_ts_sugar_span(member.object);
    let held = ast.reference_declaration(receiver).and_then(|declaration| {
        ast.bindings
            .iter()
            .find(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.iter().any(|name| name.span == declaration)
                    && (binding.immutable
                        || (binding.runtime_let
                            && !crate::indexes::binding_written(file, declaration)))
            })
            .and_then(|binding| binding.initializer)
            .map(|initializer| ast.peel_ts_sugar_span(initializer))
    });
    let value = held.unwrap_or(receiver);
    file.source_text(value)
        .is_some_and(|text| text.starts_with('['))
        || ast.call_at(value).is_some_and(|call| {
            let constructor = ast.peel_ts_sugar_span(call.callee);
            call.construct
                && file.source_text(constructor) == Some(container)
                && ast.reference_declaration(constructor).is_none()
        })
}

/// The hazards that withhold `qualified_name` at `callee`. None where the
/// receiver is plain ([`plain_container_receiver`]).
pub(crate) fn subclass_veto(
    file: &solid_facts::FileFacts,
    callee: Span,
    qualified_name: &str,
) -> Option<[Hazard; 2]> {
    let (owner, member) = qualified_name.split_once('.')?;
    let container = owner.strip_prefix("Readonly").unwrap_or(owner);
    let container = if container == "PromiseLike" {
        "Promise"
    } else {
        container
    };
    if !STANDARD_CONTAINERS.contains(&container) {
        return None;
    }
    (!plain_container_receiver(file, callee, container)).then(|| {
        [
            standard_override(container, member),
            standard_override(container, ANY_MEMBER),
        ]
    })
}

/// ADR 0840: the standard members whose identity a proof reads for when a
/// callback runs (the [`crate::runtime_semantics`] timing table), and
/// `slice`. The standard method is not what runs on a value no fact proves a
/// real instance while the program may put a function at that member of an
/// object of its own. The other members are docs/open-work.md's.
pub(crate) const IMPOSTOR_MEMBERS: [&str; 22] = [
    "addEventListener",
    "addListener",
    "catch",
    "every",
    "filter",
    "finally",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "flatMap",
    "forEach",
    "map",
    "reduce",
    "reduceRight",
    "replace",
    "replaceAll",
    "setActionHandler",
    "slice",
    "some",
    "sort",
    "then",
];

/// What a census site's key names.
#[derive(Clone)]
enum CensusKey {
    Named(String),
    /// A symbol, which names no census member.
    Symbol,
    /// A key no fact names: any member.
    Unknown,
    /// ADR 0903: a key proved to name no census member, `prototype`,
    /// `__proto__` or host key, while the standard functions its proof
    /// trusts are unpatched.
    NoMember(Vec<&'static str>),
}

/// The key a property key at `key` names: written, quoted, a literal under
/// brackets, a `const` bound to a string literal, or a symbol.
fn census_key(file: &FileFacts, key: Span, computed: bool) -> CensusKey {
    let peeled = file.ast.peel_ts_sugar_span(key);
    let Some(text) = file.source_text(peeled).map(str::trim) else {
        return CensusKey::Unknown;
    };
    let quoted = text
        .strip_prefix(['\'', '"'])
        .and_then(|inner| inner.strip_suffix(['\'', '"']))
        .or_else(|| {
            text.strip_prefix('`')
                .and_then(|inner| inner.strip_suffix('`'))
                .filter(|inner| !inner.contains("${"))
        });
    if let Some(inner) = quoted {
        return if inner.contains('\\') {
            CensusKey::Unknown
        } else {
            CensusKey::Named(inner.to_owned())
        };
    }
    if !computed || text.starts_with(|c: char| c.is_ascii_digit()) {
        return CensusKey::Named(text.to_owned());
    }
    if symbol_key(file, peeled) {
        return CensusKey::Symbol;
    }
    crate::indexes::constant_string_key(file, peeled).map_or(CensusKey::Unknown, CensusKey::Named)
}

/// [`census_key`], and for a key it leaves open ADR 0903's proof that it names
/// no member.
fn proved_key(proofs: &mut Proofs<'_>, file: &FileFacts, key: Span, computed: bool) -> CensusKey {
    match census_key(file, key, computed) {
        CensusKey::Unknown if computed => proofs
            .key(key)
            .map_or(CensusKey::Unknown, CensusKey::NoMember),
        key => key,
    }
}

/// A trust-writer key excludes the trusted names as well as census members.
fn proved_trust_key(
    proofs: &mut Proofs<'_>,
    file: &FileFacts,
    key: Span,
    computed: bool,
) -> CensusKey {
    match census_key(file, key, computed) {
        CensusKey::Unknown if computed => proofs
            .trust_key(key)
            .map_or(CensusKey::Unknown, CensusKey::NoMember),
        key => key,
    }
}

/// Whether the value at `span` is no function, by its syntax: a primitive,
/// object or array literal, an operator whose result is primitive, or the
/// global `undefined`.
fn noncallable_value(file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let value = ast.peel_ts_sugar_span(span);
    if ast.array_literals.binary_search(&value).is_ok()
        || ast.primitive_results.binary_search(&value).is_ok()
        || ast
            .template_literals
            .iter()
            .any(|template| template.span == value)
    {
        return true;
    }
    let Some(text) = file.source_text(value).map(str::trim) else {
        return false;
    };
    text.starts_with(['\'', '"', '{'])
        || text.starts_with(|c: char| c.is_ascii_digit())
        || matches!(text, "true" | "false" | "null")
        || (text == "undefined" && ast.reference_declaration(value).is_none())
}

/// Whether `callee`, past its TypeScript wrappers, is spelled `name` or a
/// member ending in it (`globalThis.Reflect.set`).
fn spelled_callee(text: &str, name: &str) -> bool {
    text == name
        || text
            .strip_suffix(name)
            .is_some_and(|rest| rest.ends_with('.'))
}

/// ADR 0840: the census members the file's code may put a function at, on an
/// object it makes or one it writes: an object literal's member that is not a
/// data literal, a class element, an export (a module namespace), a member
/// write, `Reflect.set`, a `defineProperty`, a descriptor map, and the makers
/// of keys no fact names (`Object.fromEntries`, a `Proxy` with a `get` trap,
/// `Reflect` held as a value). Copying keys (`Object.assign`, a spread) puts
/// no function where none was. A definer under another spelling is the
/// reflective-definer census's ([`definers`]), read beside this one.
///
/// ADR 0874: beside it, the writers: a site that writes a member of an object
/// it did not make ([`fresh_object`]), which is the only way a host global
/// gains one. `Object.assign` copies, so it is the impostor census's no site,
/// but it writes its target.
fn impostor_census(file: &FileFacts) -> Vec<Hazard> {
    let (members, writers) = impostor_sites(file);
    let mut members = members
        .into_iter()
        .map(|(member, _)| member)
        .collect::<Vec<_>>();
    let mut writers = writers;
    members.sort_unstable();
    members.dedup();
    writers.sort_unstable();
    writers.dedup();
    members
        .into_iter()
        .map(standard_impostor)
        .chain(writers.into_iter().map(standard_impostor_write))
        .collect()
}

/// ADR 0875: the program's site that may hold `member` for each census key:
/// the first by path and offset, as `(key, "path:line")`. A row names it.
pub(crate) fn impostor_holders(files: &[FileFacts]) -> BTreeMap<&'static str, String> {
    let mut first = BTreeMap::<&'static str, (&str, Span, &FileFacts)>::new();
    for file in files {
        for (member, span) in impostor_sites(file).0 {
            let path = file.path.as_str();
            let earlier = first
                .get(member)
                .is_some_and(|(held, at, _)| (*held, at.start) <= (path, span.start));
            if !earlier {
                first.insert(member, (path, span, file));
            }
        }
    }
    first
        .into_iter()
        .map(|(member, (path, span, file))| {
            let start = usize::try_from(span.start).unwrap_or(0);
            let line = file.source.as_bytes()[..start.min(file.source.len())]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count();
            (member, format!("{path}:{}", line + 1))
        })
        .collect()
}

/// The census's detail on a withheld call of `member`: the site that holds it.
pub(crate) fn impostor_detail(
    member: &str,
    holders: &BTreeMap<&'static str, String>,
) -> Option<String> {
    holders
        .get(member)
        .map(|site| format!("the program may put a function at `{member}`: {site}"))
        .or_else(|| {
            holders
                .get(ANY_MEMBER)
                .map(|site| format!("a key no fact names may be `{member}`: {site}"))
        })
        .or_else(|| {
            holders
                .get(SYMBOL_SITE)
                .map(|site| format!("a symbol's site or a definer may hold `{member}`: {site}"))
        })
        .or_else(|| {
            holders.get(TRUST_SITE).map(|site| {
                format!("a proof here trusts a standard function the program patches: {site}")
            })
        })
}

/// The census keys of the file's sites, sorted, and the keys it writes.
#[cfg(test)]
pub(crate) fn census_keys(file: &FileFacts) -> (Vec<&'static str>, Vec<&'static str>) {
    let (members, mut writers) = impostor_sites(file);
    let mut members = members
        .into_iter()
        .map(|(member, _)| member)
        .collect::<Vec<_>>();
    members.sort_unstable();
    members.dedup();
    writers.sort_unstable();
    writers.dedup();
    (members, writers)
}

/// Each census site of the file, by key, and the keys it writes on an object it
/// did not make.
fn impostor_sites(file: &FileFacts) -> (Vec<(&'static str, Span)>, Vec<&'static str>) {
    use solid_facts::ast::ClassElementKind;
    let ast = &file.ast;
    let mut members: Vec<(&'static str, Span)> = Vec::new();
    let mut writers: Vec<&'static str> = Vec::new();
    let mut proofs = Proofs::new(file);
    let census_member = |key: &CensusKey| match key {
        CensusKey::Named(name) => IMPOSTOR_MEMBERS
            .iter()
            .find(|member| **member == name)
            .copied(),
        CensusKey::Unknown => Some(ANY_MEMBER),
        CensusKey::Symbol => Some(SYMBOL_SITE),
        CensusKey::NoMember(_) => None,
    };
    // A `__proto__` write replaces every member at once; `self` and
    // `globalThis` are themselves keys a host receiver stands on.
    let writer_member = |key: CensusKey| match key {
        CensusKey::Named(name) if name == "__proto__" => Some(ANY_MEMBER),
        CensusKey::Named(name) if HOST_KEYS.contains(&name.as_str()) => {
            HOST_KEYS.iter().find(|key| **key == name).copied()
        }
        key => census_member(&key),
    };
    // ADR 0903: a proof that trusts standard functions raises them, and the
    // site reopens as `*` where the program patches one ([`impostor_veto`]).
    let trusted = |trust: &[&'static str], at: Span, members: &mut Vec<(&'static str, Span)>| {
        if !trust.is_empty() {
            members.push((TRUST_SITE, at));
            members.extend(trust.iter().filter_map(|name| {
                TRUSTED
                    .iter()
                    .find(|(trusted, _)| trusted == name)
                    .map(|(_, key)| (*key, at))
            }));
        }
    };
    let mut note = |key: CensusKey, at: Span| {
        if let CensusKey::NoMember(trust) = &key {
            trusted(trust, at, &mut members);
        }
        members.extend(census_member(&key).map(|member| (member, at)));
    };
    let mut lean = Vec::new();
    let mut noted = Vec::new();
    let mut write = |key: CensusKey, object: Span| {
        if !fresh_object(file, object) {
            writers.extend(writer_member(key));
        }
    };
    for property in &ast.object_properties {
        if property.data && property.value_kind.is_proven_noncallable() {
            continue;
        }
        note(
            proved_key(&mut proofs, file, property.key, property.computed),
            property.key,
        );
    }
    for element in ast.classes.iter().flat_map(|class| &class.elements) {
        match element.kind {
            ClassElementKind::Method | ClassElementKind::Getter | ClassElementKind::Setter => {}
            ClassElementKind::Field => {
                // A field is defined, never set: its key installs no prototype.
                match element.value.map(|value| proofs.value(value, false)) {
                    None => continue,
                    Some(Some(trust)) => {
                        trusted(&trust, element.span, &mut lean);
                        continue;
                    }
                    Some(None) => {}
                }
            }
            _ => continue,
        }
        if element.computed {
            note(CensusKey::Unknown, element.span);
        } else if let Some(key) = element.key {
            note(census_key(file, key, false), key);
        }
    }
    for export in ast.exports.iter().filter(|export| !export.type_only) {
        for specifier in export.specifiers.iter().chain(&export.declarations) {
            if !specifier.type_only {
                note(
                    CensusKey::Named(specifier.exported.to_string()),
                    export.span,
                );
            }
        }
    }
    let assigned = ast
        .assignments
        .iter()
        .map(|assignment| (ast.peel_ts_sugar_span(assignment.target), assignment))
        .collect::<HashMap<_, _>>();
    for target in &ast.write_targets {
        let target = ast.peel_ts_sugar_span(*target);
        let Some(member) = ast.member_at(target) else {
            // `self = fake`: the global's own key.
            if ast.reference_declaration(target).is_none()
                && let Some(name) = file.source_text(target).map(str::trim)
                && HOST_KEYS.contains(&name)
            {
                write(CensusKey::Named(name.to_owned()), target);
            }
            continue;
        };
        let assignment = assigned.get(&target);
        if assignment
            .is_some_and(|assignment| assignment.reads_target && assignment.logical.is_none())
        {
            continue;
        }
        let key = if ast.computed_members.binary_search(&member.span).is_err() {
            census_key(file, member.property, false)
        } else if let Some(literal) = ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == member.span)
        {
            CensusKey::Named(literal.key.to_string())
        } else {
            proved_key(&mut proofs, file, member.property, true)
        };
        // ADR 0903 S3b: a value that is no function puts none there. Under a
        // key no fact names, it may become a prototype.
        if let Some(assignment) = assignment.filter(|assignment| !assignment.reads_target)
            && let Some(mut trust) =
                proofs.value(assignment.value_span, matches!(key, CensusKey::Unknown))
        {
            // The value's proof stands on the key's: no `__proto__` there.
            if let CensusKey::NoMember(key_trust) = &key {
                trust.extend(key_trust);
            }
            trusted(&trust, target, &mut lean);
            continue;
        }
        // ADR 0903 S3c: a copy of a literal's own member under its own key.
        // It still writes the object under a key no fact names.
        if matches!(key, CensusKey::Unknown)
            && let Some(assignment) = assignment
                .filter(|assignment| !assignment.reads_target && assignment.logical.is_none())
            && let Some(trust) = proofs.copy(member.property, assignment.value_span)
        {
            write(CensusKey::Unknown, member.object);
            note(CensusKey::NoMember(trust), target);
            continue;
        }
        write(key.clone(), member.object);
        note(key, target);
    }
    for call in &ast.calls {
        let Some(callee) = file
            .source_text(ast.peel_ts_sugar_span(call.callee))
            .map(str::trim)
        else {
            continue;
        };
        let argument = |index: usize| {
            call.arguments
                .get(index)
                .filter(|argument| !argument.spread)
        };
        let argument_key = |proofs: &mut Proofs<'_>, index: usize| match argument(index) {
            Some(key) => match &key.literal_value {
                ArgumentLiteralFact::String(name) => CensusKey::Named(name.to_string()),
                // ADR 0877: a definer keeps `*`.
                _ if const_symbol_key(file, key.span) => CensusKey::Unknown,
                _ => proved_key(proofs, file, key.span, true),
            },
            None => CensusKey::Unknown,
        };
        // The first argument, written by a definer. A spread may be anything.
        let target = argument(0).map(|target| target.span);
        let mut write_target = |key: CensusKey| match target {
            Some(target) => write(key, target),
            None => noted.push(key),
        };
        if spelled_callee(callee, "Reflect.set") {
            let key = argument_key(&mut proofs, 1);
            let value = argument(2)
                .and_then(|value| proofs.value(value.span, matches!(key, CensusKey::Unknown)));
            match value {
                Some(mut trust) => {
                    if let CensusKey::NoMember(key_trust) = &key {
                        trust.extend(key_trust);
                    }
                    trusted(&trust, call.span, &mut lean);
                }
                None => {
                    write_target(key.clone());
                    note(key, call.span);
                }
            }
        } else if spelled_callee(callee, "Object.defineProperty")
            || spelled_callee(callee, "Reflect.defineProperty")
        {
            write_target(argument_key(&mut proofs, 1));
            note(argument_key(&mut proofs, 1), call.span);
        } else if spelled_callee(callee, "Object.defineProperties")
            || spelled_callee(callee, "Object.create")
        {
            // `Object.create`'s object is new.
            let writes = spelled_callee(callee, "Object.defineProperties");
            match call.arguments.get(1) {
                None => {}
                Some(map) if !map.spread && map.exact_object_literal => {
                    for key in &map.property_names {
                        if writes {
                            write_target(census_key(file, *key, false));
                        }
                        note(census_key(file, *key, false), call.span);
                    }
                }
                Some(_) => {
                    if writes {
                        write_target(CensusKey::Unknown);
                    }
                    note(CensusKey::Unknown, call.span);
                }
            }
        } else if spelled_callee(callee, "Object.setPrototypeOf")
            || spelled_callee(callee, "Reflect.setPrototypeOf")
        {
            write_target(CensusKey::Unknown);
        } else if spelled_callee(callee, "Object.assign") {
            if target.is_some_and(|target| fresh_object(file, target)) {
                continue;
            }
            for source in call.arguments.iter().skip(1) {
                if !source.spread && source.exact_object_literal {
                    for key in &source.property_names {
                        write_target(census_key(file, *key, false));
                    }
                } else {
                    write_target(CensusKey::Unknown);
                }
            }
            if call.arguments.first().is_some_and(|first| first.spread) {
                write_target(CensusKey::Unknown);
            }
        } else if spelled_callee(callee, "Object.fromEntries") {
            note(CensusKey::Unknown, call.span);
        } else if call.construct && spelled_callee(callee, "Proxy") {
            let traps_get = argument(1).is_none_or(|handler| {
                !handler.exact_object_literal
                    || handler
                        .property_names
                        .iter()
                        .any(|key| file.source_text(*key).map(str::trim) == Some("get"))
            });
            if traps_get {
                note(CensusKey::Unknown, call.span);
            }
        }
    }
    if let Some(reflect) = ast.identifiers.iter().find(|identifier| {
        file.source_text(identifier.span) == Some("Reflect")
            && ast.reference_declaration(identifier.span).is_none()
            && !ast.any_member_with_object(identifier.span)
    }) {
        note(CensusKey::Unknown, reflect.span);
    }
    members.extend(lean);
    writers.extend(noted.into_iter().filter_map(writer_member));
    // R65: `w.Math = { floor: f }` and `w.Math.floor = f` need no rooted
    // receiver, and a noncallable replacement invalidates a trust proof too.
    for target in ast.write_targets.iter().chain(&ast.deleted_targets) {
        let target = ast.peel_ts_sugar_span(*target);
        let key = if let Some(member) = ast.member_at(target) {
            if fresh_object(file, member.object) {
                continue;
            }
            proved_trust_key(
                &mut proofs,
                file,
                member.property,
                ast.computed_members.binary_search(&target).is_ok(),
            )
        } else if ast.reference_declaration(target).is_none() {
            census_key(file, target, false)
        } else {
            continue;
        };
        match key {
            CensusKey::Named(name) => writers.extend(trusted_write_name(&name)),
            CensusKey::Unknown => writers.push(TRUST_WRITE_ANY),
            CensusKey::NoMember(trust) => trusted(&trust, target, &mut members),
            CensusKey::Symbol => {}
        }
    }
    // A reflective delete can expose an inherited replacement, just as delete.
    for call in &ast.calls {
        let callee = file
            .source_text(ast.peel_ts_sugar_span(call.callee))
            .unwrap_or("");
        if !spelled_callee(callee, "Reflect.deleteProperty") {
            continue;
        }
        let target = call.arguments.first().filter(|argument| !argument.spread);
        if target.is_some_and(|target| fresh_object(file, target.span)) {
            continue;
        }
        let key = call
            .arguments
            .get(1)
            .filter(|argument| !argument.spread)
            .map_or(CensusKey::Unknown, |argument| {
                proved_trust_key(&mut proofs, file, argument.span, true)
            });
        match key {
            CensusKey::Named(name) => writers.extend(trusted_write_name(&name)),
            CensusKey::Unknown => writers.push(TRUST_WRITE_ANY),
            CensusKey::NoMember(trust) => trusted(&trust, call.span, &mut members),
            CensusKey::Symbol => {}
        }
    }

    // `Symbol.replace` read at all: it may become a key of any object.
    if let Some(read) = ast.members.iter().find(|member| {
        file.source_text(member.span).map(str::trim) == Some("Symbol.replace")
            && ast
                .reference_declaration(ast.peel_ts_sugar_span(member.object))
                .is_none()
    }) {
        members.push((SYMBOL_REPLACE, read.span));
    }
    (members, writers)
}

/// Whether the object at `span` is one this file makes on the spot: exactly
/// an array or object literal, or a `const` or never-written `let` bound to
/// one. A write through it reaches that object, whatever holds it.
fn fresh_object(file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let literal = |span: Span| {
        ast.array_literals.binary_search(&span).is_ok()
            || (ast.member_at(span).is_none()
                && ast.call_at(span).is_none()
                && file
                    .source_text(span)
                    .map(str::trim)
                    .is_some_and(|text| text.starts_with('{') && text.ends_with('}')))
    };
    let value = ast.peel_ts_sugar_span(span);
    literal(value)
        || ast.reference_declaration(value).is_some_and(|declaration| {
            ast.identifier_binding(declaration).is_some_and(|binding| {
                (binding.immutable
                    || (binding.runtime_let && !crate::indexes::binding_written(file, declaration)))
                    && binding
                        .initializer
                        .is_some_and(|initializer| literal(ast.peel_ts_sugar_span(initializer)))
            })
        })
}

/// The global's own keys a host receiver stands on: a program may replace
/// `self` and `globalThis`, unlike `window` and `document`. `Symbol` is the one
/// [`symbol_key`] stands on.
const HOST_KEYS: [&str; 3] = ["self", "globalThis", "Symbol"];
/// The census key of a site keyed by a symbol ([`symbol_key`]): no member, while
/// the program leaves `Symbol` alone.
const SYMBOL_SITE: &str = "@@symbol";
/// The census key of a read of `Symbol.replace` ([`primitive_dispatch_standard`]).
const SYMBOL_REPLACE: &str = "@@replace";

/// The array methods whose result is a new array of the receiver's species,
/// and the promise methods whose result is a new promise.
const ARRAY_FRESH: [&str; 10] = [
    "concat",
    "filter",
    "flat",
    "flatMap",
    "map",
    "slice",
    "toReversed",
    "toSorted",
    "toSpliced",
    "with",
];
const PROMISE_FRESH: [&str; 3] = ["catch", "finally", "then"];

/// Whether the value at `value` is one the standard library made for
/// `owner`, by a fact of this file: an array literal, a `new` of the global
/// container, `Array.from`, `Array.of`, `Object.entries`, `keys` or `values`,
/// a promise from the global `Promise`'s own factories (a resolve of nothing
/// or of a value no promise can be, which adopts no instance), a `fetch`, the
/// result of a standard method that makes a new one on such a value
/// (`[a].map(f)`, `fetch(u).then(f)`), a call of a function of this file
/// nothing rebinds whose every return is one ([`returned_made`]; an `async`
/// one's is a promise), or a string literal for a string's own. Each root is a global no declaration
/// of this file shadows, and each function a patch may not have replaced
/// (`patched`, ADR 0781).
fn standard_made(
    file: &FileFacts,
    value: Span,
    owner: &str,
    patched: &dyn Fn(&str) -> bool,
    depth: u8,
) -> bool {
    let ast = &file.ast;
    if depth > 8 {
        return false;
    }
    let global = |span: Span, name: &str| {
        file.source_text(span).map(str::trim) == Some(name)
            && ast.reference_declaration(span).is_none()
    };
    let root_global = |span: Span| {
        let mut root = ast.peel_ts_sugar_span(span);
        while let Some(member) = ast.member_at(root) {
            root = ast.peel_ts_sugar_span(member.object);
        }
        ast.reference_declaration(root).is_none()
    };
    let owner = owner.strip_prefix("Readonly").unwrap_or(owner);
    let owner = if owner == "PromiseLike" {
        "Promise"
    } else {
        owner
    };
    let text = file.source_text(value).map(str::trim).unwrap_or_default();
    // `R.m(…)` for a method that makes a new value of `R`'s kind.
    let chained = ast.call_at(value).is_some_and(|call| {
        let callee = ast.peel_ts_sugar_span(call.callee);
        let Some(link) = ast.member_at(callee) else {
            return false;
        };
        let fresh: &[&str] = match owner {
            "Array" => &ARRAY_FRESH,
            "Promise" => &PROMISE_FRESH,
            _ => &[],
        };
        !call.construct
            && ast.computed_members.binary_search(&link.span).is_err()
            && file.source_text(link.property).is_some_and(|member| {
                // ADR 0874: the new value's kind is the receiver's species.
                fresh.contains(&member)
                    && !patched(&format!("{owner}.{member}"))
                    && !patched(&format!("{owner}.constructor"))
            })
            && standard_made(
                file,
                ast.peel_ts_sugar_span(link.object),
                owner,
                patched,
                depth + 1,
            )
    });
    if chained
        || returned_made(file, value, owner, patched, depth)
        || awaited_made(file, value, owner, patched, depth)
    {
        return true;
    }
    // A call of the global function `name`, which no patch replaced.
    let factory = |name: &str, qualified: &str| {
        ast.call_at(value).is_some_and(|call| {
            let callee = ast.peel_ts_sugar_span(call.callee);
            root_global(callee)
                && file.source_text(callee).map(str::trim) == Some(name)
                && !patched(qualified)
        })
    };
    let constructed = |name: &str| {
        ast.call_at(value).is_some_and(|call| {
            call.construct
                && global(ast.peel_ts_sugar_span(call.callee), name)
                && !patched(&format!("{name}Constructor.construct"))
        })
    };
    match owner {
        "Array" => {
            ast.array_literals.binary_search(&value).is_ok()
                || constructed("Array")
                || [
                    ("Array.from", "ArrayConstructor.from"),
                    ("Array.of", "ArrayConstructor.of"),
                    ("Object.entries", "ObjectConstructor.entries"),
                    ("Object.keys", "ObjectConstructor.keys"),
                    ("Object.values", "ObjectConstructor.values"),
                ]
                .iter()
                .any(|(name, qualified)| {
                    ast.call_at(value).is_some_and(|call| !call.construct)
                        && factory(name, qualified)
                })
        }
        "Map" | "Set" | "WeakMap" | "WeakSet" => constructed(owner),
        "Promise" => {
            constructed("Promise")
                || ast.call_at(value).is_some_and(|call| {
                    // `Promise.resolve(p)` hands back `p` itself when `p` is
                    // a promise: only a value no promise can be makes a new one.
                    !call.construct
                        && (factory("Promise.resolve", "PromiseConstructor.resolve")
                            && call.arguments.first().is_none_or(|argument| {
                                !argument.spread
                                    && (matches!(
                                        argument.literal_value,
                                        ArgumentLiteralFact::Null
                                            | ArgumentLiteralFact::Boolean(_)
                                            | ArgumentLiteralFact::Integer(_)
                                            | ArgumentLiteralFact::String(_)
                                    ) || noncallable_value(file, argument.span))
                            })
                            || ["reject", "all", "allSettled", "race", "any"]
                                .iter()
                                .any(|name| {
                                    factory(
                                        &format!("Promise.{name}"),
                                        &format!("PromiseConstructor.{name}"),
                                    )
                                })
                            || ["fetch", "window.fetch", "globalThis.fetch"]
                                .iter()
                                .any(|name| factory(name, "fetch")))
                })
        }
        "String" => {
            text.starts_with(['\'', '"'])
                || ast
                    .template_literals
                    .iter()
                    .any(|template| template.span == value)
        }
        _ => false,
    }
}

/// Whether `value` is a host global (`window`, `document`, `globalThis`,
/// `self`) or a node the document holds (`document.body`), no declaration of
/// this file shadowing it. Every spelling of the program reaches it, so
/// nothing but the absence of a writer keeps it the host's own (ADR 0874).
fn host_global(file: &FileFacts, value: Span) -> bool {
    let ast = &file.ast;
    let global = |span: Span, name: &str| {
        file.source_text(span).map(str::trim) == Some(name)
            && ast.reference_declaration(span).is_none()
    };
    ["window", "document", "globalThis", "self"]
        .iter()
        .any(|name| global(value, name))
        || ast.member_at(value).is_some_and(|member| {
            global(ast.peel_ts_sugar_span(member.object), "document")
                && ast.computed_members.binary_search(&member.span).is_err()
                && matches!(
                    file.source_text(member.property),
                    Some("body" | "head" | "documentElement")
                )
        })
}

/// Whether `value` is a call of a function this file declares and nothing
/// rebinds (a declaration, or a `const` bound to a function), whose every
/// `return` hands back a value [`standard_made`] proves for `owner`: a native
/// promise for an `async` one.
fn returned_made(
    file: &FileFacts,
    value: Span,
    owner: &str,
    patched: &dyn Fn(&str) -> bool,
    depth: u8,
) -> bool {
    let ast = &file.ast;
    let Some(call) = ast.call_at(value).filter(|call| !call.construct) else {
        return false;
    };
    let function = local_function(file, ast.peel_ts_sugar_span(call.callee));
    let Some(function) = function.filter(|function| !function.generator) else {
        return false;
    };
    if function.r#async {
        return matches!(owner, "Promise" | "PromiseLike");
    }
    let returns = function_returns(file, function);
    !returns.is_empty()
        && returns.into_iter().all(|returned| {
            returned.is_some_and(|returned| {
                standard_made(
                    file,
                    ast.peel_ts_sugar_span(returned),
                    owner,
                    patched,
                    depth + 1,
                )
            })
        })
}

/// Whether the value at `value` is a primitive by a fact of this file: a
/// literal, a template, an operator whose result is primitive, or a call of
/// the global `String`, `Number`, `Boolean` or `BigInt` no patch replaced. A
/// member of a primitive is its wrapper prototype's, which only a patch
/// replaces (ADR 0781): no object of the program's is it.
fn primitive_value(file: &FileFacts, value: Span, patched: &dyn Fn(&str) -> bool) -> bool {
    let ast = &file.ast;
    if ast.primitive_results.binary_search(&value).is_ok()
        || ast
            .template_literals
            .iter()
            .any(|template| template.span == value)
    {
        return true;
    }
    let text = file.source_text(value).map(str::trim).unwrap_or_default();
    text.starts_with(['\'', '"'])
        || text.starts_with(|c: char| c.is_ascii_digit())
        || matches!(text, "true" | "false")
        || ast.call_at(value).is_some_and(|call| {
            let callee = ast.peel_ts_sugar_span(call.callee);
            !call.construct
                && ast.reference_declaration(callee).is_none()
                && ["String", "Number", "Boolean", "BigInt"]
                    .iter()
                    .any(|name| {
                        file.source_text(callee).map(str::trim) == Some(name)
                            && !patched(&format!("{name}Constructor.call"))
                    })
        })
}

/// Whether `value` is `await x` where `x` is a value [`standard_made`] proves
/// for `owner` (not a promise, so `await` hands it back), or a call of a
/// function this file declares, `async` and nothing rebinds, whose every
/// `return` is one: its promise fulfils with that value, which no patch made
/// thenable (`owner.then`, ADR 0781).
fn awaited_made(
    file: &FileFacts,
    value: Span,
    owner: &str,
    patched: &dyn Fn(&str) -> bool,
    depth: u8,
) -> bool {
    let ast = &file.ast;
    let owner = owner.strip_prefix("Readonly").unwrap_or(owner);
    // ADR 0874: `await` looks `then` up the whole prototype chain.
    if matches!(owner, "Promise" | "PromiseLike")
        || patched(&format!("{owner}.then"))
        || patched("Object.then")
    {
        return false;
    }
    let Some(text) = file.source_text(value) else {
        return false;
    };
    let Some(rest) = text.strip_prefix("await") else {
        return false;
    };
    if !rest.starts_with(|c: char| c.is_whitespace() || c == '(') {
        return false;
    }
    let skipped = text.len() - rest.trim_start().len();
    let Ok(skipped) = u32::try_from(skipped) else {
        return false;
    };
    let operand = ast.peel_ts_sugar_span(Span::new(value.start + skipped, value.end));
    if standard_made(file, operand, owner, patched, depth + 1) {
        return true;
    }
    let Some(function) = ast
        .call_at(operand)
        .filter(|call| !call.construct)
        .and_then(|call| local_function(file, ast.peel_ts_sugar_span(call.callee)))
        .filter(|function| function.r#async && !function.generator)
    else {
        return false;
    };
    let returns = function_returns(file, function);
    !returns.is_empty()
        && returns.into_iter().all(|returned| {
            returned.is_some_and(|returned| {
                standard_made(
                    file,
                    ast.peel_ts_sugar_span(returned),
                    owner,
                    patched,
                    depth + 1,
                )
            })
        })
}

/// The function a callee names when this file declares it and nothing
/// rebinds it: a declaration, or a `const` bound to a function.
fn local_function(file: &FileFacts, callee: Span) -> Option<&solid_facts::ast::FunctionFact> {
    let ast = &file.ast;
    let declaration = ast.reference_declaration(callee)?;
    ast.functions
        .iter()
        .find(|function| {
            function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
                && !crate::indexes::binding_written(file, declaration)
        })
        .or_else(|| {
            let binding = ast.identifier_binding(declaration)?;
            let initializer = ast.peel_ts_sugar_span(binding.initializer?);
            binding
                .immutable
                .then(|| {
                    ast.functions
                        .iter()
                        .find(|function| function.span == initializer)
                })
                .flatten()
        })
}

/// What each `return` of `function` hands back (its expression body's value
/// for an arrow), `None` for a bare `return`.
fn function_returns(
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Vec<Option<Span>> {
    let ast = &file.ast;
    if function.expression_body {
        function
            .expression_return
            .iter()
            .map(|returned| returned.argument)
            .collect()
    } else {
        ast.returns_within(function.body)
            .filter(|returned| {
                ast.innermost_function_owning(returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            })
            .map(|returned| returned.argument)
            .collect()
    }
}

/// The members of a standard container that run no code of the program on
/// their receiver: no callback, and no `this` but the library's (ADR 0874).
fn hands_nothing_on(owner: &str, member: &str) -> bool {
    match owner {
        "Array" => matches!(
            member,
            "slice"
                | "concat"
                | "join"
                | "indexOf"
                | "lastIndexOf"
                | "includes"
                | "at"
                | "push"
                | "pop"
                | "shift"
                | "unshift"
                | "reverse"
                | "fill"
                | "keys"
                | "values"
                | "entries"
                | "flat"
                | "toReversed"
                | "toSpliced"
                | "with"
                | "splice"
                | "copyWithin"
                // A comparator gets two elements.
                | "sort"
                | "toSorted"
        ),
        "Map" | "Set" | "WeakMap" | "WeakSet" => matches!(
            member,
            "get" | "set" | "has" | "add" | "delete" | "clear" | "keys" | "values" | "entries"
        ),
        // A promise's callbacks get its value, never the promise.
        "Promise" => matches!(member, "then" | "catch" | "finally"),
        _ => false,
    }
}

/// Whether a [`hands_nothing_on`] member's result is its receiver.
fn answers_with_receiver(member: &str) -> bool {
    matches!(
        member,
        "reverse" | "sort" | "fill" | "copyWithin" | "set" | "add"
    )
}

/// The parameter position at which a callback-taking member of a standard
/// container hands the callback its receiver, 0-based.
fn receiver_slot(owner: &str, member: &str) -> Option<usize> {
    match (owner, member) {
        (
            "Array",
            "forEach" | "map" | "filter" | "find" | "findIndex" | "findLast" | "findLastIndex"
            | "some" | "every" | "flatMap",
        )
        | ("Map" | "Set", "forEach") => Some(2),
        ("Array", "reduce" | "reduceRight") => Some(3),
        _ => None,
    }
}

/// Whether the use of a binding at `used`, the receiver of a call, leaves the
/// program without its value (ADR 0874): the member hands nothing on, or it
/// takes a function literal that stops short of the slot that holds the
/// receiver and can reach it no other way.
///
/// The member must be the library's: a patch may run with the receiver as
/// `this` (`Array.prototype.slice = function () { … }`). No impostor can be on
/// the receiver itself: the first would need a use that hands it on.
fn receiver_stays(
    file: &FileFacts,
    used: &solid_facts::ast::MemberFact,
    owner: &str,
    patched: &dyn Fn(&str) -> bool,
) -> bool {
    let ast = &file.ast;
    if ast.computed_members.binary_search(&used.span).is_ok() {
        return false;
    }
    let Some(member) = file.source_text(used.property).map(str::trim) else {
        return false;
    };
    if patched(&format!("{owner}.{member}")) {
        return false;
    }
    if hands_nothing_on(owner, member) {
        // A member that answers with its receiver hands it to the program
        // unless the answer is discarded.
        return !answers_with_receiver(member)
            || ast
                .calls_with_peeled_callee(used.span)
                .all(|call| call.result_discarded);
    }
    let Some(slot) = receiver_slot(owner, member) else {
        return false;
    };
    let Some(call) = ast.calls_with_peeled_callee(used.span).next() else {
        return false;
    };
    call.arguments.first().is_some_and(|argument| {
        let value = ast.peel_ts_sugar_span(argument.span);
        !argument.spread
            && ast
                .functions
                .iter()
                .find(|function| function.span == value)
                .is_some_and(|function| {
                    function.parameters.len() <= slot
                        && !function.rest_parameter
                        && file.source_text(function.span).is_some_and(|text| {
                            !text.contains("arguments")
                                && !text.contains("eval")
                                && (function.kind == solid_facts::ast::FunctionKind::Arrow
                                    || !crate::execution_role::mentions_receiver(text))
                        })
                })
    })
}

/// Whether a primitive receiver's `member` is the standard one's whatever the
/// program holds: all but `replace` and `replaceAll`, which dispatch through
/// their first argument's `Symbol.replace`, so that argument must be one with
/// none of the program's: a primitive or a regular expression literal, on a
/// prototype no patch reached (ADR 0874).
fn primitive_dispatch_standard(
    file: &FileFacts,
    callee: Span,
    member: &str,
    patched: &dyn Fn(&str) -> bool,
    replace_read: bool,
) -> bool {
    if !matches!(member, "replace" | "replaceAll") {
        return true;
    }
    if replace_read {
        return false;
    }
    let ast = &file.ast;
    ast.calls_with_peeled_callee(ast.peel_ts_sugar_span(callee))
        .next()
        .is_some_and(|call| {
            call.arguments.first().is_none_or(|argument| {
                let value = ast.peel_ts_sugar_span(argument.span);
                !argument.spread
                    && !patched("String.replace")
                    && !patched("Object.replace")
                    && (primitive_value(file, value, patched)
                        || (file
                            .source_text(value)
                            .is_some_and(|text| text.trim_start().starts_with('/'))
                            && !patched("RegExp.replace")
                            && !patched("RegExp.exec")))
            })
        })
}

/// Whether the receiver of `callee` is, by a fact of this file, a value the
/// standard library made ([`standard_made`]), a primitive
/// ([`primitive_value`]) or a host global no writer reached ([`host_global`],
/// `written` answers for a member): on the spot, or held by a binding nothing rebinds (and,
/// for a made value, nothing exports, every use of which leaves the program
/// without it, [`receiver_stays`]).
fn standard_receiver(
    file: &FileFacts,
    callee: Span,
    qualified: (&str, &str),
    patched: &dyn Fn(&str) -> bool,
    written: &dyn Fn(&str) -> bool,
    named_written: &dyn Fn(&str) -> bool,
    replace_read: bool,
) -> bool {
    let ast = &file.ast;
    let (owner, member_name) = qualified;
    let Some(member) = ast.member_at(ast.peel_ts_sugar_span(callee)) else {
        return false;
    };
    let container = matches!(
        owner.strip_prefix("Readonly").unwrap_or(owner),
        "Array" | "Map" | "Set" | "WeakMap" | "WeakSet" | "Promise" | "PromiseLike" | "String"
    );
    let host = |value: Span| {
        !container
            && !written(member_name)
            && host_global(file, value)
            && !HOST_KEYS
                .iter()
                .any(|key| written(key) && file.source_text(value).map(str::trim) == Some(*key))
    };
    let primitive = |value: Span| {
        primitive_value(file, value, patched)
            && primitive_dispatch_standard(file, callee, member_name, patched, replace_read)
    };
    let receiver = ast.peel_ts_sugar_span(member.object);
    // ADR 0876: a named write may have reached this value, by any route.
    // ADR 0874: a string is made too, and `replace` dispatches all the same.
    let dispatches =
        || primitive_dispatch_standard(file, callee, member_name, patched, replace_read);
    let made = |value: Span| {
        !named_written(member_name) && dispatches() && standard_made(file, value, owner, patched, 0)
    };
    if primitive(receiver) || host(receiver) || made(receiver) {
        return true;
    }
    let Some(declaration) = ast.reference_declaration(receiver) else {
        return false;
    };
    let Some(binding) = ast.identifier_binding(declaration) else {
        return false;
    };
    let exported = ast.exports.iter().any(|export| {
        export
            .specifiers
            .iter()
            .chain(&export.declarations)
            .any(|specifier| specifier.local.span == declaration)
    });
    let unbound = binding.immutable
        || (binding.runtime_let && !crate::indexes::binding_written(file, declaration));
    let initializer = binding
        .initializer
        .map(|initializer| ast.peel_ts_sugar_span(initializer));
    // A primitive held is one, wherever the binding goes; a host global too.
    if unbound && initializer.is_some_and(|initializer| primitive(initializer) || host(initializer))
    {
        return true;
    }
    let owner = owner.strip_prefix("Readonly").unwrap_or(owner);
    let owner = if owner == "PromiseLike" {
        "Promise"
    } else {
        owner
    };
    unbound
        && !exported
        && initializer.is_some_and(made)
        && ast
            .references_to(declaration)
            .iter()
            .filter(|reference| **reference != declaration)
            .all(|reference| {
                ast.member_with_peeled_object(*reference)
                    .is_some_and(|used| {
                        ast.any_call_with_peeled_callee(used.span)
                            && !ast.any_write_target_containing(used.span)
                            && receiver_stays(file, used, owner, patched)
                    })
            })
}

/// R65: a trusted standard function may be replaced through an unrooted
/// alias too. The write census is independent of whether the value is callable.
pub(crate) fn trust_open(
    hazard: &impl Fn(Hazard) -> bool,
    defined_reflectively: &impl Fn(&str) -> bool,
) -> bool {
    hazard(standard_impostor(TRUST_SITE))
        && TRUSTED.iter().any(|(name, key)| {
            let (global, member) = name.split_once('.').unwrap_or((name, name));
            hazard(standard_impostor(key))
                && (patch_vetoed(name, hazard)
                    || [global, member, ANY_MEMBER, TRUST_WRITE_ANY]
                        .into_iter()
                        .any(|key| hazard(standard_impostor_write(key)))
                    || defined_reflectively(global)
                    || defined_reflectively(member))
        })
}

/// Open writes that may replace a trusted function with a noncallable value
/// are separate from callable-member impostors (ADR 0903, R65).
const TRUST_WRITE_ANY: &str = "@@trust-write:*";

fn trusted_write_name(name: &str) -> Option<&'static str> {
    TRUSTED
        .iter()
        .flat_map(|(name, _)| name.split('.'))
        .find(|part| *part == name)
}

/// ADR 0840: whether the standard `qualified_name` at `callee` is withheld:
/// it is a census member, the program may hold an impostor of it
/// ([`Hazard::StandardImpostor`] for it or for any key, or a reflective
/// definer of it), and no fact proves the receiver a real instance.
pub(crate) fn impostor_veto(
    file: &FileFacts,
    callee: Span,
    qualified_name: &str,
    hazard: impl Fn(Hazard) -> bool,
    defined_reflectively: impl Fn(&str) -> bool,
) -> bool {
    let Some((owner, member)) = qualified_name.rsplit_once('.') else {
        return false;
    };
    if owner.is_empty() || !IMPOSTOR_MEMBERS.contains(&member) {
        return false;
    }
    // ADR 0875: `Symbol` replaced, a key a site took for a symbol may be a name.
    let symbol_open = hazard(standard_impostor(SYMBOL_SITE))
        && (hazard(standard_impostor_write("Symbol"))
            || (hazard(Hazard::AnyStandardPatch)
                && [
                    standard_patch("", "Symbol"),
                    standard_patch("", ANY_MEMBER),
                    standard_patch("Symbol", "for"),
                    standard_patch("Symbol", ANY_MEMBER),
                ]
                .into_iter()
                .any(&hazard)));
    let patched = |name: &str| symbol_open || patch_vetoed(name, &hazard);
    // ADR 0903: a site proved to put no function at a member, trusting a
    // standard function the program patches, may be any key.
    let trust_open = trust_open(&hazard, &defined_reflectively);
    (symbol_open
        || trust_open
        || hazard(standard_impostor(member))
        || hazard(standard_impostor(ANY_MEMBER))
        || defined_reflectively(member)
        || (matches!(member, "replace" | "replaceAll")
            && hazard(standard_impostor(SYMBOL_REPLACE))))
        && !standard_receiver(
            file,
            callee,
            (owner, member),
            &patched,
            &|name| {
                symbol_open
                    || trust_open
                    || hazard(standard_impostor_write(name))
                    || hazard(standard_impostor_write(ANY_MEMBER))
                    || defined_reflectively(name)
            },
            &|name| symbol_open || hazard(standard_impostor_write(name)),
            hazard(standard_impostor(SYMBOL_REPLACE)),
        )
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
    for (index, class) in file.ast.classes.iter().enumerate() {
        for hazard in class_overrides(file, class) {
            entries.push((hazard, ordinal(index)));
        }
    }
    for (index, call) in file.ast.calls.iter().enumerate() {
        let resolved = view.resolved_callee_call(call.callee);
        if resolved
            .and_then(crate::runtime_semantics::standard_call_name)
            .is_some_and(crate::runtime_semantics::delivers_synchronously)
            || (resolved
                .and_then(crate::runtime_semantics::standard_call_name)
                .is_none()
                && crate::indexes::spelled_dispatch(file, call))
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
    let writes = assignment_writes(file);
    let patches = standard_patches(view, &writes);
    if !patches.is_empty() {
        entries.push((Hazard::AnyStandardPatch, 0));
    }
    for (index, patch) in patches.into_iter().enumerate() {
        entries.push((patch, ordinal(index)));
    }
    for (index, impostor) in impostor_census(file).into_iter().enumerate() {
        entries.push((impostor, ordinal(index)));
    }
    for write in writes {
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
    if !crate::execution_role::file_is_module(file) {
        let declared = file
            .ast
            .functions
            .iter()
            .filter_map(|function| function.name.as_ref())
            .chain(
                file.ast
                    .bindings
                    .iter()
                    .flat_map(|binding| binding.names.iter()),
            );
        for name in declared {
            if let Some(text) = file
                .source_text(name.span)
                .filter(|text| text.starts_with("__"))
            {
                entries.push((classic_script_dunder(text), 0));
            }
        }
    }
    let (obtained, handed_calls) = crate::interproc::accessor_provenance::intrinsic_census(file);
    if obtained {
        entries.push((Hazard::ArrayIntrinsicObtained, 0));
    }
    for call in handed_calls {
        entries.push((Hazard::IntrinsicHandedAtCall, call));
    }
    for index in 0..file.window_valued_candidates().len() {
        entries.push((
            Hazard::WindowValuedCandidate,
            u32::try_from(index).unwrap_or(u32::MAX),
        ));
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
