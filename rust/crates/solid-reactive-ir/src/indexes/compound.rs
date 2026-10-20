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

use super::*;
use solid_facts::ast::{
    BindingFact, FunctionKind, ObjectGetShapeFact, ObjectPropertyFact, RuntimeValueKind,
};

type Accounted<'a> = HashSet<(&'a str, Span)>;

pub(super) struct State<'a> {
    values: OnceLock<Accounted<'a>>,
}

impl Default for State<'_> {
    fn default() -> Self {
        Self {
            values: OnceLock::new(),
        }
    }
}

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

impl<'a> SemanticLookup<'a> {
    /// Whether `span` in `path` is a function value (an identifier naming it,
    /// or the anonymous function itself) that a qualifying compound holder
    /// stores and some JSX tag renders.
    pub(crate) fn compound_value(&self, path: &str, span: Span) -> bool {
        self.compound
            .values
            .get_or_init(|| self.compound_values())
            .contains(&(path, span))
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
                .escaped_component_functions()
                .contains(&(file.path.as_str(), function.span))
    }

    fn compound_values(&self) -> Accounted<'a> {
        let mut accounted = Accounted::default();
        // A member tag somewhere is the only way a holder renders anything.
        if !self.facts.files.iter().any(|file| {
            file.ast
                .jsx_elements
                .iter()
                .any(|e| e.member_object.is_some())
        }) {
            return accounted;
        }
        // A direct `eval` can replace a member of any holder in scope with no
        // reference to it.
        if self.facts.files.iter().any(|file| {
            file.ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        }) {
            return accounted;
        }
        let mut tags = HashMap::<&'a str, HashMap<Span, Vec<String>>>::default();
        let mut vetoed = None;
        let mut patched = None;
        for file in &self.facts.files {
            let mut literals = None;
            for binding in &file.ast.bindings {
                if !candidate(file, binding) {
                    continue;
                }
                let literals = literals.get_or_insert_with(|| FileLiterals::of(file));
                let Some(holder) = self.holder(file, literals, binding) else {
                    continue;
                };
                if matches!(holder.target, Target::Function(_))
                    && *patched.get_or_insert_with(|| self.object_assign_may_be_patched())
                {
                    continue;
                }
                if *vetoed.get_or_insert_with(|| {
                    crate::interproc::accessor_provenance::reflective_prototype_write(self)
                }) {
                    return Accounted::default();
                }
                self.census_holder(file, binding, &holder, &mut tags, &mut accounted);
            }
        }
        accounted
    }

    /// The holder `binding` declares, when its initializer has the shape.
    fn holder(
        &self,
        file: &FileFacts,
        literals: &FileLiterals<'_>,
        binding: &BindingFact,
    ) -> Option<Holder> {
        let ast = &file.ast;
        let initializer = ast.peel_ts_sugar_span(binding.initializer?);
        // A function that runs before the holder's statement reaches its
        // values uninitialized (`const Sub` below it) and throws, and a
        // function-local holder is built by such a call.
        if !module_top_level(file, binding.declaration) {
            return None;
        }
        if binding.initializer_value_kind == RuntimeValueKind::Object {
            let members = self.literal_tree(file, literals, initializer, 0)?;
            return members_initialized(file, &members, initializer.start).then_some(Holder {
                target: Target::Object,
                members,
            });
        }
        let call = ast.call_at(initializer)?;
        if file.source_text(call.callee) != Some("Object.assign")
            || call.arguments.len() < 2
            || call.arguments.iter().any(|argument| argument.spread)
            || crate::interproc::accessor_provenance::mutation_builtin(file, call, self)
                != Some("assign")
        {
            return None;
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
        // object's setter or a Proxy trap would receive each member and may
        // call it, and a getter may answer another value. Only a function
        // that exactly resolves, in this file, to a function declaration or
        // a `const` bound to an arrow or function expression takes the
        // members as plain data properties.
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
            return None;
        }
        let mut members: Vec<(String, Member)> = Vec::new();
        for argument in &call.arguments[1..] {
            let tree =
                self.literal_tree(file, literals, ast.peel_ts_sugar_span(argument.span), 0)?;
            if !members_initialized(file, &tree, initializer.start) {
                return None;
            }
            for (key, member) in tree {
                // A function's own `name`, `length` and (for a constructor)
                // `prototype` are non-writable, `arguments` and `caller` are
                // poisoned on some functions: `Object.assign` throws and
                // nothing renders.
                if matches!(
                    key.as_str(),
                    "name" | "length" | "prototype" | "arguments" | "caller"
                ) {
                    return None;
                }
                // The later literal wins at runtime; a repeated key is not
                // followed.
                if members.iter().any(|(existing, _)| *existing == key) {
                    return None;
                }
                members.push((key, member));
            }
        }
        Some(Holder {
            target: Target::Function(first),
            members,
        })
    }

    /// The member tree of the closed, getter-free literal at `literal`.
    fn literal_tree(
        &self,
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
                Member::Namespace(self.literal_tree(file, literals, value, depth + 1)?)
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

    /// Accounts for the member values of `holder` that some tag renders,
    /// when every reference to its binding qualifies.
    fn census_holder(
        &self,
        file: &'a FileFacts,
        binding: &BindingFact,
        holder: &Holder,
        tags: &mut HashMap<&'a str, HashMap<Span, Vec<String>>>,
        accounted: &mut Accounted<'a>,
    ) {
        let Some(name) = binding.names.first() else {
            return;
        };
        let Some(references) = self.references_at(file, name.span) else {
            return;
        };
        let mut rendered = HashSet::<Span>::default();
        let mut plain_rendered = false;
        for (user, span) in references {
            if reference_is_declaration_or_module_surface(user, span) {
                continue;
            }
            let Some(keys) = tags
                .entry(user.path.as_str())
                .or_insert_with(|| tag_names(user))
                .get(&span)
            else {
                return;
            };
            if keys.is_empty() {
                if !matches!(holder.target, Target::Function(_)) {
                    return;
                }
                plain_rendered = true;
                continue;
            }
            let mut members = &holder.members;
            for (index, key) in keys.iter().enumerate() {
                let Some((_, member)) = members.iter().find(|(candidate, _)| candidate == key)
                else {
                    return;
                };
                match (member, index + 1 == keys.len()) {
                    (Member::Namespace(inner), false) => members = inner,
                    (Member::Value(value), true) => {
                        rendered.insert(*value);
                    }
                    _ => return,
                }
            }
        }
        if let Target::Function(target) = holder.target {
            // The target function is the holder's own value: a reference to
            // it that is no tag, this argument or its surface would reach
            // every member without naming the holder. An anonymous function
            // has no other reference.
            let sealed = file.ast.function_at(target).is_some()
                || self.references_at(file, target).is_some_and(|uses| {
                    uses.into_iter().all(|(user, span)| {
                        span == target
                            || reference_is_declaration_or_module_surface(user, span)
                            || tags
                                .entry(user.path.as_str())
                                .or_insert_with(|| tag_names(user))
                                .get(&span)
                                .is_some_and(Vec::is_empty)
                    })
                });
            if !sealed {
                return;
            }
            if plain_rendered {
                accounted.insert((file.path.as_str(), target));
            }
        }
        for value in rendered {
            accounted.insert((file.path.as_str(), value));
        }
    }

    /// The Type Facts references of the symbol at `span` as analyzed files
    /// and spans, `None` when any evidence is missing.
    fn references_at(&self, file: &FileFacts, span: Span) -> Option<Vec<(&'a FileFacts, Span)>> {
        let symbol = self.entities.at(file.path.as_str(), span)?;
        self.symbol_references_if_present(symbol.as_str())?
            .iter()
            .map(|reference| {
                Some((
                    self.file_by_path(reference.path.as_ref())?,
                    Span::new(
                        u32::try_from(reference.start_byte).ok()?,
                        u32::try_from(reference.end_byte).ok()?,
                    ),
                ))
            })
            .collect()
    }
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
    fn object_assign_may_be_patched(&self) -> bool {
        let assigned = self.assigned_members();
        if assigned.global
            || assigned.hosts.iter().any(|(_, dom)| !dom)
            || assigned.names.contains("assign")
            || super::global_identifier_written(self.facts, "Object")
        {
            return true;
        }
        self.facts.files.iter().any(|file| {
            file.ast.calls.iter().any(|call| {
                let Some(target) = call.arguments.first() else {
                    return false;
                };
                let text = file
                    .source_text(file.ast.peel_ts_sugar_span(target.span))
                    .unwrap_or_default();
                crate::interproc::accessor_provenance::mutation_builtin(file, call, self).is_some()
                    && (target.spread
                        || matches!(text, "Object" | "globalThis" | "window" | "self" | "global")
                        || text.ends_with(".Object"))
            })
        })
    }
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
