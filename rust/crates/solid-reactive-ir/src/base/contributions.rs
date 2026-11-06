//! The contribution functions of the base indexes. Each has the shape
//! `fn(&FileView, &BuildConstants) -> Vec<(key, value)>`: it reads one file
//! and the build's constants, and produces its entries in a deterministic
//! order (never a hash map's iteration order).

use std::sync::Arc;

use solid_facts::ast::{ArgumentLiteralFact, ImportKind, ModuleHazardKind, ModuleLoadKind};
use solid_facts::core::Span;
use solid_facts::{FileFacts, StandardRoot};

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
    /// Some [`Self::StandardPatch`] in the file (value 0), so a call that
    /// meets none hashes nothing.
    AnyStandardPatch,
    /// A top-level `await` or `for await` (value 0).
    TopLevelAwait,
    /// Some export outside every `namespace` block (value 0).
    ModuleLevelExport,
    /// Some export at all (value 0).
    Export,
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

fn standard_patch(owner: &str, member: &str) -> Hazard {
    Hazard::StandardPatch(member_hash(owner, member))
}

fn push_patch(root: &StandardRoot, member: &str, patches: &mut Vec<Hazard>) {
    match root {
        StandardRoot::Global => patches.push(standard_patch("", member)),
        StandardRoot::Prototype(name) => patches.push(standard_patch(name, member)),
        StandardRoot::Constructor(name) => {
            patches.push(standard_patch(name, member));
            patches.push(standard_patch(&format!("{name}Constructor"), member));
        }
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

/// Every write to a standard global, prototype or constructor member in the
/// file: an assignment, or a definer's call on one.
fn standard_patches(view: &FileView<'_>) -> Vec<Hazard> {
    let file = view.file;
    let ast = &file.ast;
    let mut patches = Vec::new();
    for write in assignment_writes(file) {
        match write {
            AssignmentWrite::Name(member, key) => {
                let object = ast.members[member as usize].object;
                if let Some(root) = file.standard_root(object) {
                    push_patch(&root, &key, &mut patches);
                }
            }
            AssignmentWrite::PrototypeOwner(member) => {
                let prototype = ast.members[member as usize].span;
                if let Some(root) = file.standard_root(prototype) {
                    push_patch(&root, ANY_MEMBER, &mut patches);
                }
            }
            AssignmentWrite::UnownedPrototype => {}
        }
    }
    // `globalThis[key] = f`: a key no fact reads.
    for assignment in &ast.assignments {
        let target = ast.peel_ts_sugar_span(assignment.target);
        if ast.computed_members.binary_search(&target).is_ok()
            && !ast
                .literal_computed_members
                .iter()
                .any(|literal| literal.span == target)
            && let Some(member) = ast.member_at(target)
            && crate::indexes::constant_string_key(file, member.property).is_none()
            && !symbol_key(file, member.property)
            && let Some(root) = file.standard_root(member.object)
        {
            // A key the types pin: an imported `const`.
            let key = view
                .entity_at(member.property)
                .and_then(|entity| entity.constant_value.as_ref())
                .and_then(constant_key);
            push_patch(&root, key.unwrap_or(ANY_MEMBER), &mut patches);
        }
    }
    for call in &ast.calls {
        let Some(callee) = file.source_text(ast.peel_ts_sugar_span(call.callee)) else {
            continue;
        };
        let by_key = KEY_DEFINERS.contains(&callee);
        if !by_key && !SHAPE_DEFINERS.contains(&callee) {
            continue;
        }
        let Some(root) = call
            .arguments
            .first()
            .and_then(|target| file.standard_root(target.span))
        else {
            continue;
        };
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
            push_patch(
                &root,
                literal.as_deref().unwrap_or(ANY_MEMBER),
                &mut patches,
            );
        } else if argument.exact_object_literal {
            for key in &argument.property_names {
                let key = file.source_text(*key).unwrap_or(ANY_MEMBER);
                push_patch(&root, key.trim_matches(['\'', '"', '`']), &mut patches);
            }
        } else {
            push_patch(&root, ANY_MEMBER, &mut patches);
        }
    }
    patches
}

/// Whether a computed key is a symbol the global `Symbol` makes on the spot,
/// which replaces no named member.
fn symbol_key(file: &FileFacts, key: Span) -> bool {
    let key = file.ast.peel_ts_sugar_span(key);
    file.source_text(key)
        .is_some_and(|text| text.starts_with("Symbol(") || text.starts_with("Symbol."))
        && file
            .ast
            .reference_declaration(Span::new(key.start, key.start + 6))
            .is_none()
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
pub(crate) fn patch_veto(qualified_name: &str) -> [Hazard; 5] {
    let (owner, member) = qualified_name
        .rsplit_once('.')
        .unwrap_or(("", qualified_name));
    let container = owner.strip_prefix("Readonly").unwrap_or(owner);
    let container = if container == "PromiseLike" {
        "Promise"
    } else {
        container
    };
    [
        standard_patch(container, member),
        standard_patch(container, ANY_MEMBER),
        standard_patch("", member),
        standard_patch(
            "",
            container.strip_suffix("Constructor").unwrap_or(container),
        ),
        if owner.is_empty() || owner.ends_with("Constructor") {
            standard_patch("", ANY_MEMBER)
        } else {
            standard_patch(container, ANY_MEMBER)
        },
    ]
}

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
    let patches = standard_patches(view);
    if !patches.is_empty() {
        entries.push((Hazard::AnyStandardPatch, 0));
    }
    for (index, patch) in patches.into_iter().enumerate() {
        entries.push((patch, ordinal(index)));
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
