//! ADR 0209, 0314: the member names assignments write and the prototypes they
//! write through, as L2 memos (study S5b).
//!
//! - [`PrototypeOwnerAt`], per `R.prototype` write site: whose instances the
//!   write reaches, when that is provable. The host proof reads the class's
//!   references, the base `spellings` index at the method's name, and each
//!   caller's argument, never every file.
//! - [`AssignedMembersAnswer`]: the census over the member index and those
//!   proofs, replayed in file order.

use std::collections::BTreeSet;
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast::{BindingShape, ClassElementKind};
use solid_facts::core::Span;

use super::memo::{Slot, facts};
use crate::base::memo::build::{
    ASSIGNED_MEMBERS, BuildSchema, BuildTables, Input, PROTOTYPE_OWNER_AT, Section,
};
use crate::base::memo::{Canonical, Ctx, Cycle, Memo, Table, TableId};
use crate::base::{FileId, Hazard, Spelling};

/// Whose instances a write through one `prototype` reaches (ADR 0314).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnerProof {
    /// Nothing proves it.
    Unproven,
    /// The project constructor declared at this span of the writing file.
    Constructor(Arc<str>, Span),
    /// These host constructors, through this static method's parameter.
    Hosts(String, Vec<(String, bool)>),
}

impl Canonical for OwnerProof {}

/// The proof of one `R.prototype` write: the member at `ordinal` of the
/// `rank`-th slot of the file is `R.prototype`.
pub(crate) struct PrototypeOwnerAt;

type SiteKey = (FileId, u32, u32);

impl Memo<BuildSchema> for PrototypeOwnerAt {
    type Key = SiteKey;
    type Value = OwnerProof;
    const NAME: &'static str = "prototype_owner";
    const ID: TableId = PROTOTYPE_OWNER_AT;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.prototype_owner_at()
    }

    fn compute(
        cx: &Ctx<'_, '_, BuildSchema>,
        (id, rank, member): &SiteKey,
    ) -> Result<OwnerProof, Cycle> {
        let slot = Slot {
            id: *id,
            rank: *rank,
        };
        let Some(file) = facts(cx, slot) else {
            return Ok(OwnerProof::Unproven);
        };
        let Some(member) = file.ast.members.get(*member as usize) else {
            return Ok(OwnerProof::Unproven);
        };
        Ok(prototype_owner(cx, slot, file, member.object).unwrap_or(OwnerProof::Unproven))
    }

    fn on_cycle(_: &SiteKey) -> OwnerProof {
        OwnerProof::Unproven
    }

    fn implied((id, _, _): &SiteKey) -> Vec<Input> {
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Entities),
        ]
    }
}

/// ADR 0314: whose instances a write through `receiver.prototype` can reach,
/// when that is provable; `None` for any other receiver.
///
/// - A class, a function declaration, or a `const` bound to a function or
///   class expression, declared in this file and never rebound: that
///   constructor's instances.
/// - A parameter of a static class method nothing rebinds, when every
///   access to that method anywhere is a direct call through the class's
///   own name and passes, at that parameter, a global constructor declared
///   only in TypeScript's default libraries: those host prototypes.
fn prototype_owner(
    cx: &Ctx<'_, '_, BuildSchema>,
    slot: Slot,
    file: &FileFacts,
    receiver: Span,
) -> Option<OwnerProof> {
    let receiver = file.ast.peel_ts_sugar_span(receiver);
    if file
        .ast
        .identifiers
        .binary_search_by_key(&receiver, |identifier| identifier.span)
        .is_err()
    {
        return None;
    }
    let declaration = file.ast.reference_declaration(receiver)?;
    if let Some((function, index)) = file.ast.functions.iter().find_map(|function| {
        function
            .parameters
            .iter()
            .position(|parameter| parameter.names.iter().any(|name| name.span == declaration))
            .map(|index| (function, index))
    }) {
        return host_prototype_owner(cx, slot, file, function, index);
    }
    let declared_constructor = file.ast.classes.iter().any(|class| {
        class
            .name
            .as_ref()
            .is_some_and(|name| name.span == declaration)
    }) || file.ast.functions.iter().any(|function| {
        function.kind == solid_facts::ast::FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    });
    if declared_constructor {
        return (!crate::indexes::binding_written(file, declaration))
            .then(|| OwnerProof::Constructor(file.path.shared(), declaration));
    }
    let binding = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
    let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
    (binding.immutable
        && binding.shape == BindingShape::Identifier
        && (file.ast.functions.iter().any(|function| {
            function.span == initializer && function.kind != solid_facts::ast::FunctionKind::Arrow
        }) || file
            .ast
            .classes
            .iter()
            .any(|class| class.span == initializer)))
    .then(|| OwnerProof::Constructor(file.path.shared(), declaration))
}

fn host_prototype_owner(
    cx: &Ctx<'_, '_, BuildSchema>,
    slot: Slot,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
    index: usize,
) -> Option<OwnerProof> {
    let parameter = function.parameters.get(index)?;
    let [name] = parameter.names.as_slice() else {
        return None;
    };
    if parameter.shape != BindingShape::Identifier
        || parameter.initializer.is_some()
        || (function.rest_parameter && index + 1 == function.parameters.len())
        || crate::indexes::binding_written(file, name.span)
        || file.ast.identifiers.iter().any(|identifier| {
            function.span.contains(identifier.span)
                && file.source_text(identifier.span) == Some("arguments")
        })
    {
        return None;
    }
    let (class, element) = file.ast.classes.iter().find_map(|class| {
        class
            .elements
            .iter()
            .find(|element| {
                element.value == Some(function.span)
                    && element.r#static
                    && element.kind == ClassElementKind::Method
                    && !element.computed
            })
            .map(|element| (class, element))
    })?;
    let method = file.source_text(element.key?)?;
    let class_name = class.name.as_ref()?;
    let class_symbol = cx
        .file(slot.id, slot.rank as usize)?
        .symbol_at(class_name.span)?;
    // The class never escapes: past its declaration and imports, every
    // reference is the object of a member access naming its key.
    for (path, start, end) in cx.symbol_references(class_symbol.as_str())? {
        let (id, rank) = cx.path_file(&path)?;
        let reference_file = facts(cx, Slot { id, rank })?;
        let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else {
            return None;
        };
        let span = Span::new(start, end);
        if (id == slot.id && span == class_name.span)
            || reference_file
                .ast
                .imports
                .iter()
                .any(|import| import.span.contains(span))
        {
            continue;
        }
        if !reference_file.ast.members.iter().any(|member| {
            reference_file.ast.peel_ts_sugar_span(member.object) == span
                && reference_file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
        }) {
            return None;
        }
    }
    let mut hosts = Vec::new();
    let spellings = cx.spellings(method);
    // A computed key or an object slot spelled like the method may reach it
    // without a member access.
    if spellings
        .iter()
        .any(|(_, _, spelling)| *spelling == Spelling::Key)
    {
        return None;
    }
    for (id, rank, spelling) in spellings {
        let Spelling::Member(ordinal) = spelling else {
            continue;
        };
        let other_slot = Slot { id, rank };
        let other = facts(cx, other_slot)?;
        let member = other.ast.members.get(ordinal as usize)?;
        let receiver = other.ast.peel_ts_sugar_span(member.object);
        let is_this_class = cx
            .file(id, rank as usize)?
            .symbol_at(receiver)
            .and_then(|symbol| cx.class_for_symbol(symbol.as_str()))
            .is_some_and(|(found, _, found_class)| found == slot.id && found_class == class.span);
        if !is_this_class {
            return None;
        }
        let call = other.ast.calls.iter().find(|call| {
            !call.construct && other.ast.peel_ts_sugar_span(call.callee) == member.span
        })?;
        if call
            .arguments
            .iter()
            .take(index + 1)
            .any(|argument| argument.spread)
        {
            return None;
        }
        hosts.push(global_constructor(
            cx,
            other_slot,
            other,
            call.arguments.get(index)?.span,
        )?);
    }
    Some(OwnerProof::Hosts(method.to_owned(), hosts))
}

/// The name of the global constructor `value` names, and whether every
/// `var` declaration of it is in `lib.dom.d.ts`, when it is an identifier
/// no project binding declares, every declaration of it is ambient, and
/// its value is declared only by TypeScript's default libraries.
fn global_constructor(
    cx: &Ctx<'_, '_, BuildSchema>,
    slot: Slot,
    file: &FileFacts,
    value: Span,
) -> Option<(String, bool)> {
    let value = file.ast.peel_ts_sugar_span(value);
    if file
        .ast
        .identifiers
        .binary_search_by_key(&value, |identifier| identifier.span)
        .is_err()
        || file.ast.reference_declaration(value).is_some()
    {
        return None;
    }
    let symbol = cx.symbol_at(slot.id, value)?;
    if symbol.alias || symbol.declarations.is_empty() {
        return None;
    }
    // Every declaration is ambient, in a declaration file no analyzed
    // source is: none creates a runtime binding (`@webgpu/types` merges an
    // interface into `HTMLCanvasElement`; `babylonjs` re-declares
    // `var OffscreenCanvas`). The value is the host's, which a default
    // library declares, under the standard host premise (ADR 0295, 0300).
    let mut dom = true;
    let mut library = false;
    for declaration in &symbol.declarations {
        let path = declaration.path.as_ref();
        let name = std::path::Path::new(path).file_name()?.to_str()?;
        if cx.path_file(path).is_some() || !name.ends_with(".d.ts") {
            return None;
        }
        if declaration.kind.as_ref() == "variable" && name.starts_with("lib.") {
            library = true;
            dom &= name == "lib.dom.d.ts";
        }
    }
    library
        .then(|| file.source_text(value).map(str::to_owned))
        .flatten()
        .map(|name| (name, dom))
}

/// What assignments write (ADR 0209, 0314), canonical.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AssignedAnswer {
    /// The member names some assignment writes, sorted.
    pub(crate) names: Vec<Arc<str>>,
    /// A write through a prototype whose owner is not proven: every object's
    /// members may be anything.
    pub(crate) global: bool,
    /// Project constructors, by path and declaration, whose prototype is
    /// written.
    pub(crate) constructors: BTreeSet<(Arc<str>, Span)>,
    /// Host constructors whose prototype is written, each with whether every
    /// declaration of it is in `lib.dom.d.ts`, sorted.
    pub(crate) hosts: Vec<(String, bool)>,
}

impl Canonical for AssignedAnswer {}

/// The project's assigned-member census.
pub(crate) struct AssignedMembersAnswer;

impl Memo<BuildSchema> for AssignedMembersAnswer {
    type Key = ();
    type Value = AssignedAnswer;
    const NAME: &'static str = "assigned_members";
    const ID: TableId = ASSIGNED_MEMBERS;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.assigned_members()
    }

    fn compute(cx: &Ctx<'_, '_, BuildSchema>, (): &()) -> Result<AssignedAnswer, Cycle> {
        let names = cx.assigned_names();
        let mut prototype = cx.hazard(Hazard::UnownedPrototypeWrite);
        let mut constructors = BTreeSet::new();
        let mut pending_hosts = Vec::new();
        for site in cx.prototype_sites() {
            match cx.memo::<PrototypeOwnerAt>(&site)? {
                OwnerProof::Constructor(path, declaration) => {
                    constructors.insert((path, declaration));
                }
                OwnerProof::Hosts(method, hosts) => pending_hosts.push((method, hosts)),
                OwnerProof::Unproven => prototype = true,
            }
        }
        // A host proof holds only while neither the method nor the global
        // constructors it names are rewritten.
        let mut hosts = Vec::new();
        let written = |name: &str| {
            names
                .binary_search_by(|candidate| candidate.as_ref().cmp(name))
                .is_ok()
        };
        for (method, found) in pending_hosts {
            if written(&method)
                || found
                    .iter()
                    .any(|(host, _)| written(host) || cx.global_written(host))
            {
                prototype = true;
            }
            hosts.extend(found);
        }
        hosts.sort_unstable();
        Ok(AssignedAnswer {
            names,
            global: prototype,
            constructors,
            hosts,
        })
    }

    fn on_cycle((): &()) -> AssignedAnswer {
        // Fails closed: every object's members may be anything.
        AssignedAnswer {
            global: true,
            ..AssignedAnswer::default()
        }
    }
}
