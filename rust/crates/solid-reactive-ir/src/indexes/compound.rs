//! Compound components (ADR 0342): a function stored in an object and
//! reached only through JSX member tags.
//!
//! `export const Split = { Root, Panel }` and
//! `export const Tabs = Object.assign(TabsRoot, { List: TabsList })` store
//! components in an object that callers render as `<Split.Panel>`. ADR 0331
//! counts the storing reference as an escape, because an object can hand a
//! member to code that calls it plainly, so every such body was undecided.
//! The object is a census, though: when each way to obtain a member is a JSX
//! tag, `createComponent` is the only caller and the member is a component
//! exactly as a directly tagged function is.
//!
//! A holder qualifies only when all of these hold:
//! - it is a `const` identifier bound to an object literal, or to
//!   `Object.assign(target, literal, ...)` with the standard library's
//!   `Object.assign`, a function `target` (an identifier whose every
//!   reference is its declaration, module surface, this argument or a plain
//!   tag, so the members are not reachable through the function itself) and
//!   each literal closed (no spread, computed key, duplicate, setter or
//!   `__proto__`) with no getter or method and every value an identifier, an
//!   anonymous function, a data literal or a nested literal;
//! - every Type Facts reference to its binding is its declaration or module
//!   surface (as for a plain component, ADR 0331) or the leftmost name of a
//!   JSX tag (`<Split.Panel>`, `<Split.Group.Item>`, a closing tag) whose
//!   keys name a member of the literal tree, ending at a value; a plain
//!   `<Tabs>` tag counts only for an `Object.assign` target;
//! - no reflective prototype write exists in the program (as ADR 0292), no
//!   direct `eval` exists (it can replace a member with no reference), and
//!   an `Object.assign` holder needs the standard library's `Object.assign`
//!   untouched ([`SemanticLookup::object_assign_may_be_patched`]);
//! - the holder is declared at module top level and every identifier it
//!   stores, and its `Object.assign` target, is a hoisted function
//!   declaration, an import or a binding declared earlier there, so no
//!   function that runs earlier reaches an uninitialized binding and throws
//!   before anything renders;
//! - an `Object.assign` literal names no own non-writable property of a
//!   function (`name`, `length`, `prototype`, `arguments`, `caller`), which
//!   would make the call throw.
//!
//! A member value some such tag renders is then accounted for in the census
//! of the function it names ([`SemanticLookup::compound_value`]); the
//! function is proven only if all its other references are accounted too. A
//! member nothing renders stays unaccounted.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::census::{
    AssignBuiltinCalls, CallPrimitives, ObjectAssignPatched, ProgramMemberTag, PrototypeWriteVeto,
};
use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, Input, Section};
use crate::base::memo::{Ctx, Cycle, Memo, Table, TableId};
use solid_facts::ast::{
    BindingFact, FunctionKind, ObjectGetShapeFact, ObjectPropertyFact, RuntimeValueKind,
};

/// One member of a holder's literal tree.
enum Member {
    /// An identifier or an anonymous function: a value a tag may render.
    Value(Span),
    /// A non-callable literal: no tag may name it.
    Data,
    Namespace(Vec<(String, Member)>),
}

/// What a plain `<Holder>` tag renders.
enum Target {
    /// The holder is a plain object: only member tags are valid.
    Object,
    /// `Object.assign(target, ...)`: the target function's value, as the
    /// identifier or the anonymous function written there.
    Function(Span),
}

struct Holder {
    target: Target,
    members: Vec<(String, Member)>,
}

/// The literals and properties of one file, by position.
struct FileLiterals<'f> {
    shapes: HashMap<Span, &'f ObjectGetShapeFact>,
    /// Sorted by start; a literal's properties are a contiguous run.
    properties: Vec<&'f ObjectPropertyFact>,
}

impl<'f> FileLiterals<'f> {
    fn of(file: &'f FileFacts) -> Self {
        let mut properties = file.ast.object_properties.iter().collect::<Vec<_>>();
        properties
            .sort_by_key(|property| (property.span.start, std::cmp::Reverse(property.span.end)));
        Self {
            shapes: file
                .ast
                .object_get_shapes
                .iter()
                .map(|shape| (shape.span, shape))
                .collect(),
            properties,
        }
    }

    /// The properties written directly in `literal`: those not inside
    /// another property's value.
    fn own(&self, literal: Span) -> Vec<&'f ObjectPropertyFact> {
        let first = self
            .properties
            .partition_point(|property| property.span.start <= literal.start);
        let mut own = Vec::new();
        let mut resume = 0;
        for property in &self.properties[first..] {
            if property.span.start >= literal.end {
                break;
            }
            if property.span.start < resume || !literal.contains(property.span) {
                continue;
            }
            resume = property.span.end;
            own.push(*property);
        }
        own
    }
}

impl SemanticLookup<'_> {
    /// Whether `span` in `path` is a function value (an identifier naming it,
    /// or the anonymous function itself) that a qualifying compound holder
    /// stores and some JSX tag renders ([`CompoundValuesIn`]).
    pub(crate) fn compound_value(&self, path: &str, span: Span) -> bool {
        self.memos.table().id_of(path).is_some_and(|id| {
            self.memo_world()
                .answer::<CompoundValuesIn>(&id)
                .binary_search(&span)
                .is_ok()
        })
    }

    /// Whether `property` holding `function` is a member a JSX member tag
    /// renders (ADR 0342) and `function` is a component whose only entries
    /// those tags are: the property is a render of the function, not a
    /// retention of a callable.
    pub(crate) fn is_compound_render_witness(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        property: &ObjectPropertyFact,
    ) -> bool {
        self.compound_value(
            file.path.as_str(),
            file.ast.peel_ts_sugar_span(property.value),
        ) && self.function_is_component(file, function)
            && !self
                .component_statuses_of(file.path.as_str())
                .1
                .contains(&(file.path.as_str(), function.span))
    }

    /// Whether some analyzed file holds a JSX member tag (`<Split.Panel>`):
    /// the only way a holder renders anything.
    pub(super) fn program_has_member_tag(&self) -> bool {
        self.files().iter().any(|file| {
            file.ast
                .jsx_elements
                .iter()
                .any(|element| element.member_object.is_some())
        })
    }

    /// The `Object.assign` calls of `file` the standard library's own
    /// `Object.assign` answers ([`crate::interproc::accessor_provenance::mutation_builtin`]),
    /// by span, sorted.
    pub(super) fn assign_builtin_calls(&self, file: &FileFacts) -> Vec<Span> {
        let mut calls = file
            .ast
            .calls
            .iter()
            .filter(|call| {
                file.source_text(call.callee) == Some("Object.assign")
                    && crate::interproc::accessor_provenance::mutation_builtin(file, call, self)
                        == Some("assign")
            })
            .map(|call| call.span)
            .collect::<Vec<_>>();
        calls.sort_unstable();
        calls.dedup();
        calls
    }
}

/// The function values one file's compound holders store that some JSX tag
/// renders, sorted (ADR 0342; see the module docs for what qualifies).
///
/// Its reads: its own file, the program-wide gates (a member tag, the `eval`
/// hazard probe, the `Object.assign` and prototype-write vetoes), the
/// module census's cycle and unknown-load probes, its `Object.assign` calls and primitives, and for
/// each holder the references of its binding (and of an `Object.assign`
/// target) with the tags of the files holding them. A file with no holder
/// reads the gates only.
pub(crate) struct CompoundValuesIn;

impl Memo<BuildSchema> for CompoundValuesIn {
    type Key = FileId;
    type Value = Arc<Vec<Span>>;
    const NAME: &'static str = "compound_values";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Arc<Vec<Span>>, Cycle> {
        // A member tag somewhere is the only way a holder renders anything.
        // A direct `eval` can replace a member of any holder in scope with no
        // reference to it.
        if !ctx.memo::<ProgramMemberTag>(&())? || ctx.hazard(crate::base::Hazard::Eval) {
            return Ok(Arc::default());
        }
        let Some(read) = ctx.file(*id, 0) else {
            return Ok(Arc::default());
        };
        let file = read.facts();
        let mut accounted = Vec::new();
        let mut literals = None;
        let mut tags = BTreeMap::new();
        for binding in &file.ast.bindings {
            if !candidate(file, binding) {
                continue;
            }
            // Another module can render the holder's tags while this module
            // is still initializing, through an import cycle: the holder is
            // then uninitialized and the tag throws before any member runs.
            // An unresolved load counts as a cycle (module census probes).
            if ctx.cyclic(*id) || ctx.unknown_load() {
                continue;
            }
            let literals = literals.get_or_insert_with(|| FileLiterals::of(file));
            let Some(holder) = holder(ctx, *id, file, literals, binding)? else {
                continue;
            };
            if matches!(holder.target, Target::Function(_))
                && ctx.memo::<ObjectAssignPatched>(&())?
            {
                continue;
            }
            // The veto is program-wide: with it, no holder anywhere renders.
            if ctx.memo::<PrototypeWriteVeto>(&())? {
                return Ok(Arc::default());
            }
            census_holder(ctx, &read, binding, &holder, &mut tags, &mut accounted)?;
        }
        accounted.sort_unstable();
        accounted.dedup();
        Ok(Arc::new(accounted))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<Span>> {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![Input::File(*id, Section::Source)]
    }
}

/// The holder `binding` declares, when its initializer has the shape.
fn holder(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    file: &FileFacts,
    literals: &FileLiterals<'_>,
    binding: &BindingFact,
) -> Result<Option<Holder>, Cycle> {
    let ast = &file.ast;
    let Some(initializer) = binding.initializer.map(|span| ast.peel_ts_sugar_span(span)) else {
        return Ok(None);
    };
    // A function that runs before the holder's statement reaches its values
    // uninitialized (`const Sub` below it) and throws, and a function-local
    // holder is built by such a call.
    if !module_top_level(file, binding.declaration) {
        return Ok(None);
    }
    // Module code that runs before the holder's statement may render a tag
    // that names it, which throws before the member runs.
    if runs_code_before(ctx, id, file, binding.declaration.start)? {
        return Ok(None);
    }
    if binding.initializer_value_kind == RuntimeValueKind::Object {
        let Some(members) = literal_tree(file, literals, initializer, 0) else {
            return Ok(None);
        };
        return Ok(
            members_initialized(file, &members, initializer.start).then_some(Holder {
                target: Target::Object,
                members,
            }),
        );
    }
    let Some(call) = ast.call_at(initializer) else {
        return Ok(None);
    };
    if file.source_text(call.callee) != Some("Object.assign")
        || call.arguments.len() < 2
        || call.arguments.iter().any(|argument| argument.spread)
        || ctx
            .memo::<AssignBuiltinCalls>(&id)?
            .binary_search(&call.span)
            .is_err()
    {
        return Ok(None);
    }
    let first = ast.peel_ts_sugar_span(call.arguments[0].span);
    let is_function = |function: &solid_facts::ast::FunctionFact| {
        function.span == first
            && function.name.is_none()
            && function.method_name.is_none()
            && matches!(
                function.kind,
                FunctionKind::Arrow | FunctionKind::Expression
            )
    };
    // `Object.assign` writes with [[Set]]: a class's static accessor, an
    // object's setter or a Proxy trap would receive each member and may call
    // it, and a getter may answer another value. Only a function that
    // exactly resolves, in this file, to a function declaration or a `const`
    // bound to an arrow or function expression takes the members as plain
    // data properties.
    let function_target = ast.functions.iter().any(is_function)
        || ast.reference_declaration(first).is_some_and(|declaration| {
            ast.functions.iter().any(|function| {
                function.kind == FunctionKind::Declaration
                    && function
                        .name
                        .as_ref()
                        .is_some_and(|name| name.span == declaration)
            }) || ast.bindings.iter().any(|candidate| {
                candidate.immutable
                    && candidate.shape == solid_facts::ast::BindingShape::Identifier
                    && candidate.initializer_function
                    && candidate.names.len() == 1
                    && candidate.names[0].span == declaration
            })
        });
    if !function_target || !initialized_before(file, first, initializer.start) {
        return Ok(None);
    }
    let mut members: Vec<(String, Member)> = Vec::new();
    for argument in &call.arguments[1..] {
        let Some(tree) = literal_tree(file, literals, ast.peel_ts_sugar_span(argument.span), 0)
        else {
            return Ok(None);
        };
        if !members_initialized(file, &tree, initializer.start) {
            return Ok(None);
        }
        for (key, member) in tree {
            // A function's own `name`, `length` and (for a constructor)
            // `prototype` are non-writable, `arguments` and `caller` are
            // poisoned on some functions: `Object.assign` throws and nothing
            // renders.
            if matches!(
                key.as_str(),
                "name" | "length" | "prototype" | "arguments" | "caller"
            ) {
                return Ok(None);
            }
            // The later literal wins at runtime; a repeated key is not
            // followed.
            if members.iter().any(|(existing, _)| *existing == key) {
                return Ok(None);
            }
            members.push((key, member));
        }
    }
    Ok(Some(Holder {
        target: Target::Function(first),
        members,
    }))
}

/// The member tree of the closed, getter-free literal at `literal`.
fn literal_tree(
    file: &FileFacts,
    literals: &FileLiterals<'_>,
    literal: Span,
    depth: u32,
) -> Option<Vec<(String, Member)>> {
    let ast = &file.ast;
    let shape = literals.shapes.get(&literal).filter(|shape| shape.closed)?;
    if depth > 8
        || shape
            .properties
            .iter()
            .any(|property| property.getter.is_some())
    {
        return None;
    }
    let own = literals.own(literal);
    if own.len() != shape.properties.len() {
        return None;
    }
    let mut members = Vec::with_capacity(own.len());
    for property in own {
        if !property.data || property.method || property.computed {
            return None;
        }
        let key = file.source_text(property.key)?;
        if !plain_identifier(key) {
            return None;
        }
        let value = ast.peel_ts_sugar_span(property.value);
        let member = if ast
            .identifiers
            .binary_search_by_key(&value, |id| id.span)
            .is_ok()
        {
            Member::Value(value)
        } else if let Some(function) = ast.function_at(value) {
            if function.name.is_some()
                || function.method_name.is_some()
                || !matches!(
                    function.kind,
                    FunctionKind::Arrow | FunctionKind::Expression
                )
            {
                return None;
            }
            Member::Value(value)
        } else if literals.shapes.contains_key(&value) {
            Member::Namespace(literal_tree(file, literals, value, depth + 1)?)
        } else if property.value_kind.is_proven_noncallable()
            && property.value_kind != RuntimeValueKind::Object
        {
            Member::Data
        } else {
            return None;
        };
        members.push((key.to_owned(), member));
    }
    Some(members)
}

/// Each file's tag names ([`tag_names`]), read once per census.
type FileTags = BTreeMap<FileId, HashMap<Span, Vec<String>>>;

/// The tag names of `id`, read through `ctx`.
fn tags_of<'t>(
    ctx: &Ctx<'_, '_, BuildSchema>,
    tags: &'t mut FileTags,
    id: FileId,
) -> &'t HashMap<Span, Vec<String>> {
    tags.entry(id).or_insert_with(|| {
        ctx.file(id, 0)
            .map(|read| tag_names(read.facts()))
            .unwrap_or_default()
    })
}

/// Accounts for the member values of `holder` that some tag renders, when
/// every reference to its binding qualifies.
fn census_holder(
    ctx: &Ctx<'_, '_, BuildSchema>,
    read: &crate::base::memo::build::FileRead<'_, '_, '_>,
    binding: &BindingFact,
    holder: &Holder,
    tags: &mut FileTags,
    accounted: &mut Vec<Span>,
) -> Result<(), Cycle> {
    let Some(name) = binding.names.first() else {
        return Ok(());
    };
    let Some(references) = references_at(ctx, read, name.span)? else {
        return Ok(());
    };
    let mut rendered = BTreeSet::<Span>::new();
    let mut plain_rendered = false;
    for (user, span) in references {
        let Some(user_read) = ctx.file(user, 0) else {
            return Ok(());
        };
        if reference_is_declaration_or_module_surface(user_read.facts(), span) {
            continue;
        }
        let Some(keys) = tags_of(ctx, tags, user).get(&span) else {
            return Ok(());
        };
        if keys.is_empty() {
            if !matches!(holder.target, Target::Function(_)) {
                return Ok(());
            }
            plain_rendered = true;
            continue;
        }
        let mut members = &holder.members;
        for (index, key) in keys.iter().enumerate() {
            let Some((_, member)) = members.iter().find(|(candidate, _)| candidate == key) else {
                return Ok(());
            };
            match (member, index + 1 == keys.len()) {
                (Member::Namespace(inner), false) => members = inner,
                (Member::Value(value), true) => {
                    rendered.insert(*value);
                }
                _ => return Ok(()),
            }
        }
    }
    if let Target::Function(target) = holder.target {
        // The target function is the holder's own value: a reference to it
        // that is no tag, this argument or its surface would reach every
        // member without naming the holder. An anonymous function has no
        // other reference.
        let file = read.facts();
        let sealed = if file.ast.function_at(target).is_some() {
            true
        } else {
            match references_at(ctx, read, target)? {
                None => false,
                Some(uses) => {
                    let mut sealed = true;
                    for (user, span) in uses {
                        let Some(user_read) = ctx.file(user, 0) else {
                            sealed = false;
                            break;
                        };
                        if span == target
                            || reference_is_declaration_or_module_surface(user_read.facts(), span)
                            || tags_of(ctx, tags, user)
                                .get(&span)
                                .is_some_and(Vec::is_empty)
                        {
                            continue;
                        }
                        sealed = false;
                        break;
                    }
                    sealed
                }
            }
        };
        if !sealed {
            return Ok(());
        }
        if plain_rendered {
            accounted.push(target);
        }
    }
    accounted.extend(rendered);
    Ok(())
}

/// The Type Facts references of the symbol at `span` as analyzed files and
/// spans, `None` when any evidence is missing.
fn references_at(
    ctx: &Ctx<'_, '_, BuildSchema>,
    read: &crate::base::memo::build::FileRead<'_, '_, '_>,
    span: Span,
) -> Result<Option<Vec<(FileId, Span)>>, Cycle> {
    let Some(symbol) = read.symbol_at(span) else {
        return Ok(None);
    };
    Ok(super::census::symbol_references(ctx, &symbol.shared())?
        .and_then(|references| references.into_iter().collect::<Option<Vec<_>>>()))
}

/// Whether `span` is written at module top level: inside no function, class
/// body (a static block) or namespace.
fn module_top_level(file: &FileFacts, span: Span) -> bool {
    crate::owners::containing_ast_function(&file.ast, span).is_none()
        && !file
            .ast
            .classes
            .iter()
            .any(|class| class.span.contains(span) && class.span.start < span.start)
        && !file.ast.is_inside_module_block(span)
}

/// Whether every identifier `members` stores is initialized when the holder
/// whose initializer starts at `holder` runs.
fn members_initialized(file: &FileFacts, members: &[(String, Member)], holder: u32) -> bool {
    members.iter().all(|(_, member)| match member {
        Member::Value(value) => initialized_before(file, *value, holder),
        Member::Data => true,
        Member::Namespace(inner) => members_initialized(file, inner, holder),
    })
}

/// Whether the value at `value` is initialized when the module-level
/// statement whose initializer starts at `holder` runs: a function literal, an
/// import, a hoisted function declaration, or a top-level binding or class
/// declared earlier. Anything else (a later `const`, a global, a name that does
/// not resolve) is not.
fn initialized_before(file: &FileFacts, value: Span, holder: u32) -> bool {
    let ast = &file.ast;
    if ast.function_at(value).is_some() {
        return true;
    }
    let Some(declaration) = ast.reference_declaration(value) else {
        // No declaration in this file's scope tree: an import binds the name.
        let name = file.source_text(value);
        return name.is_some()
            && ast.imports.iter().any(|import| {
                import
                    .bindings
                    .iter()
                    .any(|binding| file.source_text(binding.local.span) == name)
            });
    };
    if !module_top_level(file, declaration) {
        return false;
    }
    if ast.functions.iter().any(|function| {
        function.kind == FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) {
        return true;
    }
    declaration.end <= holder
        && (ast
            .bindings
            .iter()
            .any(|binding| binding.names.iter().any(|name| name.span == declaration))
            || ast.classes.iter().any(|class| {
                class
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
            }))
}

impl SemanticLookup<'_> {
    /// Whether the project may replace the standard library's
    /// `Object.assign`, which an `Object.assign` holder needs to store its
    /// members as plain data properties. The analogue of ADR 0338's
    /// `function_bind_may_be_patched`, over what the project visibly writes: a
    /// write to a member named `assign` (`Object.assign = …`), a prototype
    /// write whose owner is not proven, a write to the global `Object`, or a
    /// reflective write (`Object.defineProperty(Object, "assign", …)`,
    /// `Reflect.set(globalThis.Object, …)`) whose target is the global object
    /// or `Object`. A local `Object` that shadows the global is refused where
    /// the call resolves (`mutation_builtin`).
    pub(super) fn object_assign_may_be_patched(&self) -> bool {
        let assigned = self.assigned_members();
        if assigned.global
            || assigned.hosts.iter().any(|(_, dom)| !dom)
            || assigned.names.contains("assign")
            || super::global_identifier_written(self.facts, "Object")
        {
            return true;
        }
        self.facts.files.iter().any(|file| {
            self.computed_write_may_reach_object(file)
                || file.ast.calls.iter().any(|call| {
                    let Some(target) = call.arguments.first() else {
                        return false;
                    };
                    let reflective = self.reflective_write_name(file, call);
                    if call.construct || reflective.is_none() {
                        return false;
                    }
                    // `Object.assign(target, …)` is everywhere; only a target
                    // that is `Object` itself (or its alias) patches it. The
                    // reflective writes that take a descriptor or a key
                    // refuse any target that may be `Object`, unresolved ones
                    // included.
                    let strict = reflective == Some(ReflectiveWrite::Assign);
                    target.spread || self.may_be_object(file, target.span, !strict, 8)
                })
        })
    }

    /// Whether a computed member write whose key is not a literal (or a
    /// `const` string) can write the global `Object` function: its object may
    /// be `Object`, an alias of it, the global object or an unresolved value.
    /// A write with a known key is a named write ([`Self::assigned_members`]).
    fn computed_write_may_reach_object(&self, file: &FileFacts) -> bool {
        let ast = &file.ast;
        let targets = ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(ast.iteration_targets.iter().copied());
        for target in targets {
            let target = ast.peel_ts_sugar_span(target);
            let inside = ast
                .members
                .iter()
                .filter(|member| target.contains(member.span))
                .collect::<Vec<_>>();
            for member in inside.iter().copied().filter(|member| {
                // A member that is another written member's object is read.
                !inside
                    .iter()
                    .any(|outer| ast.peel_ts_sugar_span(outer.object) == member.span)
                    && ast.computed_members.binary_search(&member.span).is_ok()
            }) {
                let literal = ast
                    .literal_computed_members
                    .iter()
                    .any(|literal| literal.span == member.span)
                    || super::constant_string_key(file, member.property).is_some();
                if !literal && self.may_be_object(file, member.object, true, 8) {
                    return true;
                }
            }
        }
        false
    }

    /// Which builtin a call is, when it writes properties of its first
    /// argument reflectively: by the exact standard-library declaration, or by
    /// its spelling when the compiler resolved none.
    fn reflective_write_name(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<ReflectiveWrite> {
        let by_name = |name: &str| match name {
            "ObjectConstructor.assign" | "Object.assign" => Some(ReflectiveWrite::Assign),
            "ObjectConstructor.defineProperty"
            | "ObjectConstructor.defineProperties"
            | "Object.defineProperty"
            | "Object.defineProperties"
            | "Reflect.set"
            | "Reflect.defineProperty"
            | "Reflect.deleteProperty" => Some(ReflectiveWrite::Descriptor),
            _ => None,
        };
        if let Some(declaration) =
            self.standard_library_declaration(file, call, typefacts::CallKind::Call)
            && let Some(found) = by_name(declaration.qualified_name.as_ref())
        {
            return Some(found);
        }
        by_name(file.source_text(file.ast.peel_ts_sugar_span(call.callee))?)
    }

    /// Whether the value at `span` may be the global `Object` function: the
    /// identifier `Object`, a binding initialized by such a value, or a
    /// member of the global object. With `unresolved`, any value the file
    /// does not prove to be something else counts too (a parameter, an
    /// import, a call's result, a member of an unknown object).
    fn may_be_object(&self, file: &FileFacts, span: Span, unresolved: bool, depth: u32) -> bool {
        let ast = &file.ast;
        let span = ast.peel_ts_sugar_span(span);
        if depth == 0 {
            return true;
        }
        let text = file.source_text(span).unwrap_or_default();
        if matches!(text, "globalThis" | "window" | "self" | "global")
            && ast.reference_declaration(span).is_none()
            || text.ends_with(".Object")
            || text.ends_with("[\"Object\"]")
        {
            return true;
        }
        let is_identifier = ast
            .identifiers
            .binary_search_by_key(&span, |identifier| identifier.span)
            .is_ok();
        if !is_identifier {
            // A literal, a function or a class expression is not Object.
            let fresh = ast.function_at(span).is_some()
                || ast.array_literals.binary_search(&span).is_ok()
                || ast.classes.iter().any(|class| class.span == span)
                || ast.object_get_shapes.iter().any(|shape| shape.span == span);
            return !fresh && unresolved;
        }
        let Some(declaration) = ast.reference_declaration(span) else {
            // A global: `Object` itself, or some other global.
            return text == "Object" || unresolved;
        };
        if ast.functions.iter().any(|function| {
            function.kind == FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
        }) || ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }) {
            return false;
        }
        let Some(binding) = ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))
        else {
            return unresolved;
        };
        let Some(initializer) = binding.initializer else {
            return unresolved;
        };
        if binding.shape != solid_facts::ast::BindingShape::Identifier
            || !binding.immutable
            || crate::indexes::binding_written(file, declaration)
        {
            return unresolved;
        }
        if binding.initializer_function
            || matches!(
                binding.initializer_value_kind,
                RuntimeValueKind::Object | RuntimeValueKind::Array | RuntimeValueKind::Primitive
            )
        {
            return false;
        }
        let initializer = ast.peel_ts_sugar_span(initializer);
        // `const Tabs = Object.assign(Root, {...})` is its first argument;
        // that call is judged on its own.
        if let Some(call) = ast.call_at(initializer)
            && self.reflective_write_name(file, call) == Some(ReflectiveWrite::Assign)
            && let Some(first) = call.arguments.first()
        {
            return self.may_be_object(file, first.span, unresolved, depth - 1);
        }
        self.may_be_object(file, initializer, unresolved, depth - 1)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ReflectiveWrite {
    /// `Object.assign(target, …)`.
    Assign,
    /// `Object.defineProperty`, `defineProperties`, `Reflect.set`, …
    Descriptor,
}

/// Whether `binding` can declare a holder: a `const` identifier initialized
/// by an object literal or an `Object.assign` call.
fn candidate(file: &FileFacts, binding: &BindingFact) -> bool {
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Identifier
        && binding.names.len() == 1
        && binding.initializer.is_some()
        && (binding.initializer_value_kind == RuntimeValueKind::Object
            || binding.call_initializer.is_some_and(|call| {
                file.source_text(call)
                    .is_some_and(|text| text.starts_with("Object.assign"))
            }))
}

/// Every JSX tag name of `file` by the span of its leftmost identifier, with
/// the keys that follow it (empty for a plain tag): opening and closing names.
fn tag_names(file: &FileFacts) -> HashMap<Span, Vec<String>> {
    let mut names = HashMap::<Span, Vec<String>>::default();
    for element in &file.ast.jsx_elements {
        for name in std::iter::once(element.name.span).chain(element.closing_name) {
            let Some(text) = file.source_text(name) else {
                continue;
            };
            let mut segments = text.split('.');
            let Some(first) = segments.next().filter(|segment| plain_identifier(segment)) else {
                continue;
            };
            let keys = segments.map(str::to_owned).collect::<Vec<_>>();
            if !keys.iter().all(|key| plain_identifier(key)) {
                continue;
            }
            let Ok(length) = u32::try_from(first.len()) else {
                continue;
            };
            names.insert(Span::new(name.start, name.start + length), keys);
        }
    }
    names
}

fn plain_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_' || rest == '$')
}

/// Whether module top-level code written before `before` can run user code
/// or render: a call, `new`, a JSX element or a tagged template outside every
/// function (a function body runs only when it is called). A call to a
/// primitive of the dialect that is given nothing but data literals
/// (`createSignal(0)`) runs no user code and does not count.
fn runs_code_before(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    file: &FileFacts,
    before: u32,
) -> Result<bool, Cycle> {
    let ast = &file.ast;
    let top_level = |span: Span| {
        span.start < before
            && !ast
                .functions
                .iter()
                .any(|function| function.span.contains(span) && function.span != span)
    };
    if ast
        .jsx_elements
        .iter()
        .any(|element| top_level(element.span))
        || ast
            .tagged_templates
            .iter()
            .any(|template| top_level(template.span))
    {
        return Ok(true);
    }
    let mut primitives = None;
    for call in ast.calls.iter().filter(|call| top_level(call.span)) {
        let primitives = match &primitives {
            Some(primitives) => primitives,
            None => primitives.insert(ctx.memo::<CallPrimitives>(&id)?),
        };
        if !call_runs_no_user_code(ctx, primitives, call) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn call_runs_no_user_code(
    ctx: &Ctx<'_, '_, BuildSchema>,
    primitives: &BTreeMap<Span, solid_dialect::Primitive>,
    call: &solid_facts::ast::CallFact,
) -> bool {
    if call.construct
        || call.arguments.iter().any(|argument| {
            argument.spread
                || !matches!(
                    argument.runtime_value_kind,
                    RuntimeValueKind::Primitive
                        | RuntimeValueKind::Nullish
                        | RuntimeValueKind::Object
                        | RuntimeValueKind::Array
                )
        })
    {
        return false;
    }
    primitives
        .get(&call.span)
        .is_some_and(|primitive| !ctx.dialect().mounts_component_tree(*primitive))
}
