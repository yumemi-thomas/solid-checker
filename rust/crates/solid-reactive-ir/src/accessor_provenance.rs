//! Exact project-owned callable transport. Shapes and declared Accessor types
//! are deliberately not evidence here. This join retains an implementation
//! or source identity; the interprocedural consumer decides execution.

use std::collections::{HashMap, HashSet};

use solid_dialect::ContextRole;
use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use super::{SummaryRead, source_has_runtime_witness};
use crate::{SymbolId, indexes::SemanticLookup, location, owners::containing_ast_function};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Callable {
    Accessor(SymbolId),
    Function {
        path: String,
        span: Span,
    },
    /// Exact synchronous element-member invocations at a built-in map/forEach.
    ArrayMembers(Vec<(String, Span)>),
}

enum Value {
    Callable(Callable),
    /// A supported tuple closure whose iteration identity was withdrawn.
    VetoedTuple,
    Object {
        path: String,
        span: Span,
        context: bool,
    },
    Array {
        path: String,
        elements: Vec<Span>,
    },
    /// ADR 0282: the slots of the fresh array literal that a factory's one
    /// unconditional return builds. Only an exact `const` array pattern
    /// bound to that very call consumes it. Every other route (an alias, a
    /// parameter handoff, a context provider) refuses it, because there the
    /// array is no longer fresh: a holder could replace a slot.
    Tuple {
        path: String,
        factory: Span,
        elements: Vec<Span>,
    },
}

pub(super) struct Resolver<'a, 'facts> {
    pub facts: &'a ProjectFacts,
    pub lookup: &'a SemanticLookup<'facts>,
    pub accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
    pub names: &'a HashMap<SymbolId, SymbolId>,
    functions: HashSet<(String, Span)>,
    binding_locations: HashMap<SymbolId, Location>,
    parameter_inputs: HashMap<Location, (String, Span)>,
    declarations: HashMap<SymbolId, (SymbolId, Location)>,
    uses: HashMap<Location, ClosedUses>,
    providers: HashMap<Location, Vec<Option<(String, Span)>>>,
    source_origins: HashSet<SymbolId>,
    runtime_source_origins: HashSet<SymbolId>,
    blocked_contexts: HashSet<Location>,
    unkeyed_provider: bool,
    member_stability: MemberStability,
    receiver_channels_opaque: bool,
}

/// ADR 0273: absence of a visible veto is an assumption, not a closed census.
#[derive(Default)]
pub(crate) struct MemberStability {
    written: HashMap<String, Location>,
    all: Option<Location>,
    /// Every wildcard site, retained so ADR 0300 can discharge one only by
    /// proving its target is a different fresh allocation.
    wildcard_sites: Vec<Location>,
    /// Dynamic-key syntax must pass the same complete exact-target proof as
    /// reflective wildcards. A failed may-derivation is never a discharge.
    unknown_receivers: Vec<Location>,
    /// ADR 0284: ordinary callable transport scopes wildcard vetoes written
    /// inside a package analyzed as source by its root. Dependency internals are outside ADR 0273's
    /// premise, as when only the package's declarations are seen, so a write
    /// that could name any member vetoes only members of objects that package
    /// creates. A write naming its member vetoes it everywhere (review
    /// round 1): the package may write it on an object the project handed it.
    dependencies: HashMap<String, MemberStability>,
    /// ADR 0282: a visible mutation that may change the array iteration
    /// protocol. Unknown receivers/keys refuse; spelling is not identity.
    tuple_iteration: bool,
}

pub(crate) enum Stability<'a> {
    Assumed,
    Vetoed(&'a Location),
}

impl MemberStability {
    fn wildcard(&mut self, site: Location) {
        self.all.get_or_insert_with(|| site.clone());
        if !self.wildcard_sites.contains(&site) {
            self.wildcard_sites.push(site);
        }
    }
    fn state(&self, name: &str) -> Stability<'_> {
        match self.all.as_ref().or_else(|| self.written.get(name)) {
            Some(site) => Stability::Vetoed(site),
            None => Stability::Assumed,
        }
    }

    /// [`Self::state`] for a member of an object created in `owner`: the
    /// project's vetoes, and those of the package analyzed as source that
    /// holds `owner`, if one does.
    pub(crate) fn state_of(&self, facts: &ProjectFacts, owner: &str, name: &str) -> Stability<'_> {
        match self.state(name) {
            Stability::Assumed => facts
                .dependency_source_root(owner)
                .and_then(|root| self.dependencies.get(root))
                .map_or(Stability::Assumed, |package| package.state(name)),
            vetoed => vetoed,
        }
    }

    pub(crate) fn index(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut project = Self::default();
        for file in &facts.files {
            let index = match facts.dependency_source_root(file.path.as_str()) {
                Some(root) => project.dependencies.entry(root.to_owned()).or_default(),
                None => &mut project,
            };
            index.syntax(file);
            for call in &file.ast.calls {
                let Some(method) = mutation_builtin(file, call, lookup) else {
                    continue;
                };
                let site = location(file.path.shared(), call.span);
                // A spread argument shifts every later position (a tuple can
                // supply target, key and value at once), so no key position is
                // known: it could write any member.
                if call.arguments.iter().any(|argument| argument.spread) {
                    index.wildcard(site);
                    continue;
                }
                match method {
                    // A key the literal fact cannot name (a template, an
                    // identifier, a spread) could be any member, so it vetoes
                    // every transported member: over-vetoing only withholds.
                    "defineProperty" | "set" | "deleteProperty" => {
                        let name = call
                            .arguments
                            .get(1)
                            .filter(|argument| !argument.spread)
                            .and_then(|argument| match &argument.literal_value {
                                ast::ArgumentLiteralFact::String(name) => Some(name.to_string()),
                                ast::ArgumentLiteralFact::Integer(index) => Some(index.to_string()),
                                _ => None,
                            });
                        match name {
                            Some(name) => {
                                index.written.entry(name).or_insert(site);
                            }
                            None => {
                                index.wildcard(site);
                            }
                        }
                    }
                    "defineProperties" => {
                        // A literal descriptor map names its literal keys; a
                        // spread in it, or a map that is not a literal, could
                        // name any member. An unnamed computed descriptor
                        // key is also a wildcard for the integrity proof.
                        let shape = call
                            .arguments
                            .get(1)
                            .filter(|argument| !argument.spread)
                            .and_then(|argument| {
                                file.ast.object_get_shapes.iter().find(|shape| {
                                    shape.span == file.ast.peel_ts_sugar_span(argument.span)
                                })
                            })
                            .filter(|shape| {
                                !file
                                    .ast
                                    .spreads
                                    .iter()
                                    .any(|spread| shape.span.contains(spread.span))
                                    // An unnamed computed descriptor key is
                                    // a wildcard, even inside a literal map.
                                    && shape.properties.len() == file.ast.object_properties.iter().filter(|property| {
                                        shape.span.contains(property.span)
                                            && !file.ast.object_properties.iter().any(|parent| {
                                                parent.span != property.span
                                                    && shape.span.contains(parent.span)
                                                    && parent.span.contains(property.span)
                                            })
                                    }).count()
                            });
                        match shape {
                            Some(shape) => {
                                for property in &shape.properties {
                                    index
                                        .written
                                        .entry(property.key.to_string())
                                        .or_insert_with(|| site.clone());
                                }
                            }
                            None => {
                                index.wildcard(site);
                            }
                        }
                    }
                    "assign" => {
                        for (position, argument) in call.arguments.iter().enumerate() {
                            // A first-slot spread can supply both target and sources.
                            if argument.spread {
                                index.wildcard(site.clone());
                                continue;
                            }
                            if position == 0 {
                                continue;
                            }
                            let shape = file.ast.object_get_shapes.iter().find(|shape| {
                                shape.span == file.ast.peel_ts_sugar_span(argument.span)
                                    && shape.closed
                            });
                            if shape.is_none() {
                                index.wildcard(site.clone());
                            } else if let Some(shape) = shape {
                                for property in &shape.properties {
                                    index
                                        .written
                                        .entry(property.key.to_string())
                                        .or_insert_with(|| site.clone());
                                }
                            }
                        }
                    }
                    _ => unreachable!("mutation builtin catalog"),
                }
            }
        }
        for package in project.dependencies.values_mut() {
            for (name, site) in package.written.drain() {
                project.written.entry(name).or_insert(site);
            }
        }
        project.tuple_iteration = lookup.tuple_iteration_may_be_mutated();
        project
    }

    fn syntax(&mut self, file: &FileFacts) {
        // Normalized leaves exclude receiver Gets, destructuring keys/defaults
        // and reads inside computed keys. Updates and loop targets are included.
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
        for member in &file.ast.members {
            if !targets.contains(&member.span) {
                continue;
            }
            let name = if file.ast.computed_members.contains(&member.span) {
                literal_keys.get(&member.span).copied()
            } else {
                file.source_text(member.property)
            };
            if let Some(name) = name {
                self.written
                    .entry(name.to_owned())
                    .or_insert_with(|| location(file.path.shared(), member.span));
            } else {
                self.unknown_receivers
                    .push(location(file.path.shared(), member.object));
            }
        }
    }
}

/// Whether a reflective write's target may be spelled through a `prototype`.
/// A spread can supply any target, a prototype included.
fn targets_prototype(file: &FileFacts, call: &ast::CallFact) -> bool {
    call.arguments.first().is_some_and(|target| {
        target.spread
            || file
                .source_text(target.span)
                .is_some_and(|text| text.contains("prototype"))
    })
}

/// The engine's dense returned literal owns its indices and length, and the
/// iterator result owns value/done. Iteration still Gets @@iterator, next and
/// (on early close) return through mutable prototype chains. A __proto__ write
/// or setPrototypeOf can change any of those lookups. Unknown keys may name
/// these properties; a known different string cannot.
fn iteration_key(key: Option<&str>) -> bool {
    key.is_none_or(|key| matches!(key, "next" | "return" | "__proto__"))
}

fn member_key<'a>(file: &'a FileFacts, member: &ast::MemberFact) -> Option<&'a str> {
    if file.ast.computed_members.contains(&member.span) {
        file.ast
            .literal_computed_members
            .iter()
            .find(|key| key.span == member.span)
            .map(|key| key.key.as_str())
    } else {
        file.source_text(member.property)
    }
}

/// A negative proof only: a separate source allocation cannot be one of the
/// intrinsic prototypes, nor the fresh literal consumed directly at its return.
/// Follow exact immutable local aliases. Parameters, calls (getPrototypeOf
/// included), imports, reassigned bindings and cycles remain unknown.
fn separate_iteration_target(file: &FileFacts, span: Span, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if file.ast.array_literals.contains(&span)
        || file
            .ast
            .functions
            .iter()
            .any(|function| function.span == span)
        || file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == span)
    {
        return true;
    }
    if let Some(member) = file.ast.members.iter().find(|member| member.span == span)
        && member_key(file, member) == Some("prototype")
        && let Some(declaration) = file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
        && file.ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        })
        && !iteration_binding_written(file, declaration)
    {
        return true;
    }
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.immutable
                && binding.shape == ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declaration
                && !iteration_binding_written(file, declaration)
        })
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| separate_iteration_target(file, initializer, depth - 1))
}

fn iteration_binding_written(file: &FileFacts, declaration: Span) -> bool {
    file.ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .any(|target| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(*target))
                == Some(declaration)
        })
}

fn iteration_map(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    let Some(shape) = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)
    else {
        return true;
    };
    // closed excludes spreads and unnamed computed keys. Do not accidentally
    // treat a symbol descriptor (which has no string key) as an empty map.
    shape
        .properties
        .iter()
        .any(|property| iteration_key(Some(property.key.as_str())))
}

fn tuple_mutation_method<'a>(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    // A library signature on an alias is insufficient to certify the runtime
    // callee, but is sufficient to retain a possible mutation as a veto.
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let (method, library) = match declaration.qualified_name.as_ref() {
        "ObjectConstructor.assign" => ("assign", "lib.es2015.core.d.ts"),
        "ObjectConstructor.defineProperty" => ("defineProperty", "lib.es5.d.ts"),
        "ObjectConstructor.defineProperties" => ("defineProperties", "lib.es5.d.ts"),
        "ObjectConstructor.setPrototypeOf" => ("setPrototypeOf", "lib.es2015.core.d.ts"),
        "Reflect.set" => ("set", "lib.es2015.reflect.d.ts"),
        "Reflect.defineProperty" => ("defineProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.deleteProperty" => ("deleteProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.setPrototypeOf" => ("setPrototypeOf", "lib.es2015.reflect.d.ts"),
        "Function.call"
        | "Function.apply"
        | "Function.bind"
        | "CallableFunction.call"
        | "CallableFunction.apply"
        | "CallableFunction.bind"
        | "NewableFunction.call"
        | "NewableFunction.apply"
        | "NewableFunction.bind" => ("invoke", "lib.es5.d.ts"),
        "Reflect.apply" => ("reflectInvoke", "lib.es2015.reflect.d.ts"),
        "FunctionConstructor" | "FunctionConstructor.new" => ("stringCode", "lib.es5.d.ts"),
        "eval" => ("stringCode", "lib.es5.d.ts"),
        _ => return None,
    };
    (std::path::Path::new(declaration.source_file.as_ref())
        .file_name()?
        .to_str()?
        == library)
        .then_some(method)
}

pub(crate) fn tuple_iteration_write(lookup: &SemanticLookup<'_>) -> bool {
    if lookup.visible_string_code() {
        return true;
    }
    lookup.files().iter().any(|file| {
        let targets = file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .map(|span| file.ast.peel_ts_sugar_span(*span))
            .collect::<HashSet<_>>();
        file.ast.members.iter().any(|member| {
            targets.contains(&member.span)
                && iteration_key(member_key(file, member))
                && !separate_iteration_target(file, member.object, 24)
        }) || file.ast.calls.iter().any(|call| {
            let Some(method) = tuple_mutation_method(file, call, lookup) else {
                // An erased reflective signature cannot use its receiver or
                // first argument as the mutator's target. These spellings are
                // possible invocation only, never positive callee authority.
                if lookup
                    .standard_library_declaration(file, call, typefacts::CallKind::Call)
                    .is_none()
                    && file.ast.members.iter().any(|member| {
                        member.span == file.ast.peel_ts_sugar_span(call.callee)
                            && matches!(
                                member_key(file, member),
                                None | Some("call" | "apply" | "bind")
                            )
                    })
                    && !iteration_source_function(file, call.callee, lookup)
                {
                    return true;
                }
                // Missing callee evidence must not turn an opaque mutation
                // helper into a negative census. A supplied receiver that may
                // be an intrinsic retains the veto. Source implementations
                // are covered by the same project-wide write census; exact
                // non-mutating library signatures are outside this catalog.
                return lookup.primitive_at_call(file, call.span).is_none()
                    && !intrinsic_disjoint_store_setter(file, call, lookup)
                    && !intrinsic_disjoint_proxy_creation(file, call, lookup)
                    && !source_bound_invocation(file, call.callee, lookup)
                    && lookup
                        .standard_library_declaration(file, call, typefacts::CallKind::Call)
                        .is_none()
                    // A factory's declaration does not describe the callable
                    // it returns. Its object handoff needs its own census.
                    && (file.ast.call_at(file.ast.peel_ts_sugar_span(call.callee)).is_some()
                        || lookup
                            .function_called_at(
                                file.path.as_str(),
                                file.ast.peel_ts_sugar_span(call.callee),
                            )
                            .is_none())
                    && call.arguments.first().is_some_and(|target| {
                        target.spread
                            || (!matches!(
                                target.runtime_value_kind,
                                ast::RuntimeValueKind::Primitive
                                    | ast::RuntimeValueKind::Nullish
                                    | ast::RuntimeValueKind::Function
                            ) && !separate_iteration_target(file, target.span, 24))
                    });
            };
            if method == "stringCode" {
                return true;
            }
            if matches!(method, "invoke" | "reflectInvoke") {
                if source_bound_invocation(file, call.callee, lookup) {
                    return false;
                }
                // Receiver/argument slots are different for reflective calls.
                // Only a source-resolved implementation discharges the target;
                // its writes are covered by this same whole-program census.
                let target = if method == "reflectInvoke" {
                    call.arguments
                        .first()
                        .filter(|argument| !argument.spread)
                        .map(|argument| argument.span)
                } else {
                    file.ast
                        .members
                        .iter()
                        .find(|member| member.span == file.ast.peel_ts_sugar_span(call.callee))
                        .map(|member| member.object)
                };
                return !invocation_operator_is_exact(file, call, method, lookup)
                    || target
                        .is_none_or(|target| !iteration_source_function(file, target, lookup));
            }
            if call.arguments.iter().any(|argument| argument.spread) {
                return true;
            }
            let Some(target) = call.arguments.first() else {
                return true;
            };
            if separate_iteration_target(file, target.span, 24) {
                return false;
            }
            match method {
                "setPrototypeOf" => true,
                "assign" => call
                    .arguments
                    .iter()
                    .skip(1)
                    .any(|argument| iteration_map(file, argument.span)),
                "defineProperties" => call
                    .arguments
                    .get(1)
                    .is_none_or(|argument| iteration_map(file, argument.span)),
                _ => iteration_key(call.arguments.get(1).and_then(|argument| {
                    match &argument.literal_value {
                        ast::ArgumentLiteralFact::String(key) => Some(key.as_ref()),
                        ast::ArgumentLiteralFact::Integer(_) => Some("0"),
                        _ => None,
                    }
                })),
            }
        })
    })
}

/// This census withholds authority: it never proves that a gateway ran or
/// mutated a particular property. Erased signatures cannot hide visible code
/// construction, including source slots and returned gateway values.
pub(crate) fn visible_string_code(lookup: &SemanticLookup<'_>) -> bool {
    lookup.files().iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == ast::ModuleHazardKind::Eval)
            || file.ast.calls.iter().any(|call| {
                string_code_reference(file, call.callee, lookup, 24, false)
                    || tuple_mutation_method(file, call, lookup) == Some("stringCode")
                    || (call.construct
                        && lookup
                            .standard_library_declaration(
                                file,
                                call,
                                typefacts::CallKind::Construct,
                            )
                            .is_some_and(|declaration| {
                                declaration
                                    .qualified_name
                                    .starts_with("FunctionConstructor")
                            }))
            })
    })
}

fn string_code_reference(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    returned_value: bool,
) -> bool {
    if depth == 0 {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(branch) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|branch| branch.span == span)
    {
        return string_code_reference(file, branch.consequent, lookup, depth - 1, returned_value)
            || string_code_reference(file, branch.alternate, lookup, depth - 1, returned_value);
    }
    if let Some(branch) = file
        .ast
        .logical_expressions
        .iter()
        .find(|branch| branch.span == span)
    {
        return string_code_reference(file, branch.left, lookup, depth - 1, returned_value)
            || string_code_reference(file, branch.right, lookup, depth - 1, returned_value);
    }
    if let Some(member) = file.ast.members.iter().find(|member| member.span == span) {
        return string_code_slot(
            file,
            member.object,
            member_key(file, member),
            lookup,
            depth - 1,
            false,
        );
    }
    if let Some(call) = file.ast.call_at(span) {
        if string_code_reference(file, call.callee, lookup, depth - 1, false) {
            return true;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if gateway_binding(file, callee, lookup).is_some_and(|(origin, binding, declaration)| {
            !binding.immutable || iteration_binding_written(origin, declaration)
        }) || file
            .ast
            .reference_declaration(callee)
            .is_some_and(|declaration| iteration_binding_written(file, declaration))
        {
            return true;
        }
        // Source implementation, not an identifier's nominal signature, owns
        // the value returned by a visible gateway factory.
        if let Some((origin, function)) =
            lookup.function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
        {
            return function
                .expression_return
                .as_ref()
                .filter(|returned| {
                    !matches!(
                        returned.runtime_value_kind,
                        ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                    )
                })
                .and_then(|returned| returned.argument)
                .into_iter()
                .chain(
                    origin
                        .ast
                        .returns
                        .iter()
                        .filter(|returned| {
                            containing_ast_function(&origin.ast, returned.span)
                                .is_some_and(|owner| owner.span == function.span)
                        })
                        .filter(|returned| {
                            !matches!(
                                returned.runtime_value_kind,
                                ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                            )
                        })
                        .filter_map(|returned| returned.argument),
                )
                .any(|returned| string_code_reference(origin, returned, lookup, depth - 1, true));
        }
        return string_code_reference(file, call.callee, lookup, depth - 1, returned_value);
    }
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == span)
    {
        // Unsupported expression transports (a sequence, for example) do
        // not prove absence. Exact fresh functions/allocations are not code
        // constructors; their bodies still participate in the call census.
        return !file
            .ast
            .functions
            .iter()
            .any(|function| function.span == span)
            && !file.ast.array_literals.contains(&span)
            && !file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == span);
    }
    let Some((file, binding, declaration)) = gateway_binding(file, span, lookup) else {
        if file
            .ast
            .reference_declaration(span)
            .is_some_and(|declaration| gateway_assignment(file, declaration, lookup, depth))
        {
            return true;
        }
        return matches!(file.source_text(span), Some("Function" | "eval"))
            && file.ast.reference_declaration(span).is_none()
            || (returned_value
                && !file.ast.functions.iter().any(|function| {
                    function
                        .name
                        .as_ref()
                        .is_some_and(|name| Some(name.span) == file.ast.reference_declaration(span))
                        && lookup.function_value_is_current(file, function)
                }));
    };
    if gateway_assignment(file, declaration, lookup, depth) {
        return true;
    }
    let Some(initializer) = binding.initializer else {
        return returned_value;
    };
    if binding.shape == ast::BindingShape::Object {
        return binding
            .object_slots
            .iter()
            .find(|slot| slot.local.span == declaration)
            .is_none_or(|slot| {
                string_code_slot(
                    file,
                    initializer,
                    Some(slot.property.as_str()),
                    lookup,
                    depth - 1,
                    false,
                )
            });
    }
    if binding.shape == ast::BindingShape::Array
        && file
            .ast
            .array_literals
            .contains(&file.ast.peel_ts_sugar_span(initializer))
    {
        // No element identity is guessed. Any possible gateway in a literal
        // withholds every selected callable slot until exact value flow exists.
        return file
            .ast
            .identifiers
            .iter()
            .filter(|id| id.role == ast::IdentifierRole::Reference && initializer.contains(id.span))
            .any(|id| string_code_reference(file, id.span, lookup, depth - 1, true))
            || file
                .ast
                .members
                .iter()
                .filter(|member| initializer.contains(member.span))
                .any(|member| string_code_reference(file, member.span, lookup, depth - 1, true));
    }
    if matches!(
        binding.initializer_value_kind,
        ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
    ) {
        return false;
    }
    string_code_reference(file, initializer, lookup, depth - 1, returned_value)
}

fn gateway_assignment(
    file: &FileFacts,
    declaration: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    iteration_binding_written(file, declaration)
        && file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .filter(|target| {
                file.ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(**target))
                    == Some(declaration)
            })
            .any(|target| {
                file.ast
                    .assignments
                    .iter()
                    .find(|assignment| assignment.target == *target && !assignment.reads_target)
                    .is_none_or(|assignment| {
                        assignment.value != ast::AssignmentValueKind::Function
                            || string_code_reference(
                                file,
                                assignment.value_span,
                                lookup,
                                depth - 1,
                                true,
                            )
                    })
            })
}

fn gateway_binding<'a>(
    file: &'a FileFacts,
    span: Span,
    lookup: &'a SemanticLookup<'_>,
) -> Option<(&'a FileFacts, &'a ast::BindingFact, Span)> {
    file.ast
        .reference_declaration(span)
        .and_then(|declaration| {
            file.ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
                .map(|binding| (file, binding, declaration))
        })
        .or_else(|| {
            let (origin, binding, symbol) =
                lookup.binding_at_reference(file.path.as_str(), span)?;
            let declaration = binding
                .names
                .iter()
                .find(|name| lookup.entity_symbol(origin, name.span) == Some(symbol.as_str()))?
                .span;
            Some((origin, binding, declaration))
        })
}

fn string_code_slot(
    file: &FileFacts,
    object: Span,
    key: Option<&str>,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    returned_receiver: bool,
) -> bool {
    let possible_gateway_key = matches!(key, None | Some("Function" | "eval" | "constructor"));
    // Only exact, current source implementations discharge these potentially
    // executable slots. A Function/Proxy type is no runtime identity proof.
    // A selected property declaration on a structural receiver is not enough.
    // The receiver below must be a closed, non-escaping source allocation.
    if depth == 0 {
        return true;
    }
    let object = file.ast.peel_ts_sugar_span(object);
    if let Some(branch) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|branch| branch.span == object)
    {
        return string_code_slot(
            file,
            branch.consequent,
            key,
            lookup,
            depth - 1,
            returned_receiver,
        ) || string_code_slot(
            file,
            branch.alternate,
            key,
            lookup,
            depth - 1,
            returned_receiver,
        );
    }
    if let Some(branch) = file
        .ast
        .logical_expressions
        .iter()
        .find(|branch| branch.span == object)
    {
        return string_code_slot(file, branch.left, key, lookup, depth - 1, returned_receiver)
            || string_code_slot(
                file,
                branch.right,
                key,
                lookup,
                depth - 1,
                returned_receiver,
            );
    }
    if let Some(call) = file.ast.call_at(object)
        && let Some((origin, function)) =
            lookup.function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
    {
        return function
            .expression_return
            .as_ref()
            .filter(|returned| {
                !matches!(
                    returned.runtime_value_kind,
                    ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                )
            })
            .and_then(|returned| returned.argument)
            .into_iter()
            .chain(
                origin
                    .ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        containing_ast_function(&origin.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    })
                    .filter(|returned| {
                        !matches!(
                            returned.runtime_value_kind,
                            ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                        )
                    })
                    .filter_map(|returned| returned.argument),
            )
            .any(|returned| string_code_slot(origin, returned, key, lookup, depth - 1, true));
    }
    if let Some((file, binding, declaration)) = gateway_binding(file, object, lookup)
        && binding.shape == ast::BindingShape::Identifier
        && binding.immutable
        && !iteration_binding_written(file, declaration)
        && let Some(initializer) = binding.initializer
    {
        let possible =
            string_code_slot(file, initializer, key, lookup, depth - 1, returned_receiver);
        if possible || !possible_gateway_key {
            return possible;
        }
        if file.ast.exports.iter().any(|export| {
            export
                .declarations
                .iter()
                .any(|value| value.local.span == declaration)
                || export.specifiers.iter().any(|value| {
                    file.ast.reference_declaration(value.local.span) == Some(declaration)
                })
        }) {
            return true;
        }
        if !file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, bound)| *bound == declaration)
            .all(|(reference, _)| {
                file.ast.members.iter().any(|member| {
                    file.ast.peel_ts_sugar_span(member.object) == *reference
                        && !file.ast.write_targets.contains(&member.span)
                        && !file.ast.deleted_targets.contains(&member.span)
                }) || file.ast.bindings.iter().any(|binding| {
                    binding.immutable
                        && binding.shape == ast::BindingShape::Object
                        && binding
                            .initializer
                            .map(|span| file.ast.peel_ts_sugar_span(span))
                            == Some(*reference)
                })
            })
        {
            return true;
        }
        return false;
    }
    if let Some(key) = key
        && let Some(shape) = file.ast.object_get_shapes.iter().find(|shape| {
            shape.span == object
                && shape.closed
                && shape
                    .properties
                    .iter()
                    .all(|property| property.getter.is_none())
        })
        && let Some(property) = file.ast.object_properties.iter().find(|property| {
            object.contains(property.span)
                && !property.computed
                && file.source_text(property.key) == Some(key)
                && !file.ast.object_get_shapes.iter().any(|nested| {
                    nested.span != object
                        && object.contains(nested.span)
                        && nested.span.contains(property.span)
                })
        })
        && shape.properties.iter().any(|property| property.key == key)
    {
        // A method body can expose/replace its slot through `this`, without a
        // reference to the object binding. Refuse possible slot replacement
        // before using an own source method as negative gateway evidence.
        if lookup.files().iter().any(|candidate| {
            candidate.ast.write_targets.iter().chain(&candidate.ast.deleted_targets).any(|target| {
                candidate.ast.members.iter().any(|member| member.span == candidate.ast.peel_ts_sugar_span(*target)
                    && member_key(candidate, member).is_none_or(|written| written == key || written == "__proto__"))
            }) || candidate.ast.calls.iter().any(|call| {
                tuple_mutation_method(candidate, call, lookup).is_some_and(|method| match method {
                    "setPrototypeOf" => true,
                    "assign" | "defineProperties" => call.arguments.iter().skip(1).any(|argument| {
                        candidate.ast.object_get_shapes.iter().find(|shape| shape.span == candidate.ast.peel_ts_sugar_span(argument.span) && shape.closed)
                            .is_none_or(|shape| shape.properties.iter().any(|property| property.key == key || property.key == "__proto__"))
                    }),
                    "set" | "defineProperty" | "deleteProperty" => call.arguments.get(1).is_none_or(|argument| {
                        !matches!(&argument.literal_value, ast::ArgumentLiteralFact::String(written) if written.as_str() != key && written.as_str() != "__proto__")
                    }),
                    _ => false,
                })
            })
        }) { return true; }
        let Some(function) = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == file.ast.peel_ts_sugar_span(property.value))
        else {
            if matches!(
                property.value_kind,
                ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
            ) {
                return false;
            }
            return string_code_reference(file, property.value, lookup, depth - 1, true);
        };
        return function
            .expression_return
            .as_ref()
            .and_then(|returned| returned.argument)
            .into_iter()
            .chain(
                file.ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    })
                    .filter(|returned| {
                        !matches!(
                            returned.runtime_value_kind,
                            ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                        )
                    })
                    .filter_map(|returned| returned.argument),
            )
            .any(|returned| string_code_reference(file, returned, lookup, depth - 1, true));
    }
    // Unknown member authority remains a possibility, including a function's
    // inherited constructor and erased/aliased realm objects. No name grants
    // positive authority and no unresolved value proves a mutation.
    possible_gateway_key || returned_receiver
}

fn iteration_source_function(file: &FileFacts, span: Span, lookup: &SemanticLookup<'_>) -> bool {
    if invocation_property_write(lookup) {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if lookup
        .function_called_at(file.path.as_str(), span)
        .is_some()
    {
        return true;
    }
    let declaration = file.ast.reference_declaration(span);
    file.ast.functions.iter().any(|function| {
        (function.span == span
            || function
                .name
                .as_ref()
                .is_some_and(|name| Some(name.span) == declaration))
            && lookup.function_value_is_current(file, function)
    })
}

fn invocation_key(key: Option<&str>) -> bool {
    key.is_none_or(|key| matches!(key, "call" | "apply" | "bind" | "__proto__" | "Reflect"))
}

fn invocation_operator_is_exact(
    file: &FileFacts,
    call: &ast::CallFact,
    method: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(member) = file
        .ast
        .members
        .iter()
        .find(|member| member.span == file.ast.peel_ts_sugar_span(call.callee))
    else {
        return false;
    };
    if method == "invoke" {
        return matches!(member_key(file, member), Some("call" | "apply" | "bind"));
    }
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if member_key(file, member) != Some("apply")
        || file.source_text(receiver) != Some("Reflect")
        || file.ast.reference_declaration(receiver).is_some()
    {
        return false;
    }
    let Some(symbol) = lookup.symbol_at(file.path.as_str(), receiver) else {
        return false;
    };
    let Some(declaration) =
        lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)
    else {
        return false;
    };
    symbol.alias_target().is_empty()
        && !symbol.declarations().is_empty()
        && symbol.declarations().iter().all(|binding| {
            binding.name.as_ref() == "Reflect" && binding.location.path == declaration.source_file
        })
}

/// Knowing f's implementation does not prove that f.call/f.apply/f.bind, or
/// Reflect.apply, still selects the intrinsic invocation operator. Retain
/// visible possible replacement, including writes onto a fresh function.
fn invocation_property_write(lookup: &SemanticLookup<'_>) -> bool {
    if lookup.visible_string_code() {
        return true;
    }
    let map = |file: &FileFacts, span: Span| {
        file.ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == file.ast.peel_ts_sugar_span(span) && shape.closed)
            .is_none_or(|shape| {
                shape
                    .properties
                    .iter()
                    .any(|property| invocation_key(Some(property.key.as_str())))
            })
    };
    lookup.files().iter().any(|file| {
        file.ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .any(|target| {
                let target = file.ast.peel_ts_sugar_span(*target);
                (file.source_text(target) == Some("Reflect")
                    && file.ast.reference_declaration(target).is_none())
                    || file.ast.members.iter().any(|member| {
                        member.span == target && invocation_key(member_key(file, member))
                    })
            })
            || file.ast.calls.iter().any(|call| {
                let Some(method) = tuple_mutation_method(file, call, lookup) else {
                    return false;
                };
                if call.arguments.iter().any(|argument| argument.spread) {
                    return true;
                }
                match method {
                    "invoke" | "reflectInvoke" | "stringCode" => false,
                    "setPrototypeOf" => true,
                    "assign" => call
                        .arguments
                        .iter()
                        .skip(1)
                        .any(|argument| map(file, argument.span)),
                    "defineProperties" => call
                        .arguments
                        .get(1)
                        .is_none_or(|argument| map(file, argument.span)),
                    _ => invocation_key(call.arguments.get(1).and_then(|argument| {
                        match &argument.literal_value {
                            ast::ArgumentLiteralFact::String(key) => Some(key.as_ref()),
                            ast::ArgumentLiteralFact::Integer(_) => Some("0"),
                            _ => None,
                        }
                    })),
                }
            })
    })
}

/// ProxyCreate validates two object values and installs internal slots; it
/// invokes neither value and writes no intrinsic. A ProxyConstructor type is
/// not this runtime identity: require the unaliased global's library variable,
/// and refuse visible replacement (including unknown reflective targets).
fn intrinsic_disjoint_proxy_creation(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if lookup.visible_string_code()
        || !call.construct
        || call.arguments.iter().any(|argument| argument.spread)
    {
        return false;
    }
    let Some(declaration) =
        lookup.standard_library_declaration(file, call, typefacts::CallKind::Construct)
    else {
        return false;
    };
    if declaration.qualified_name.as_ref() != "ProxyConstructor.construct"
        || std::path::Path::new(declaration.source_file.as_ref())
            .file_name()
            .and_then(|name| name.to_str())
            != Some("lib.es2015.proxy.d.ts")
    {
        return false;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
        || file.ast.reference_declaration(callee).is_some()
    {
        return false;
    }
    let Some(symbol) = lookup.symbol_at(file.path.as_str(), callee) else {
        return false;
    };
    if !symbol.alias_target().is_empty()
        || symbol.declarations().len() != 1
        || !symbol.declarations().iter().all(|binding| {
            binding.name.as_ref() == "Proxy"
                && binding.kind.as_ref() == "variable"
                && binding.location.path == declaration.source_file
        })
    {
        return false;
    }
    !lookup.files().iter().any(|candidate| {
        candidate
            .ast
            .write_targets
            .iter()
            .chain(&candidate.ast.deleted_targets)
            .any(|target| {
                let target = candidate.ast.peel_ts_sugar_span(*target);
                (candidate.source_text(target) == Some("Proxy")
                    && candidate.ast.reference_declaration(target).is_none())
                    || candidate.ast.members.iter().any(|member| {
                        member.span == target
                            && matches!(member_key(candidate, member), None | Some("Proxy"))
                    })
            })
            || candidate.ast.calls.iter().any(|mutation| {
                tuple_mutation_method(candidate, mutation, lookup).is_some_and(|method| {
                    !matches!(method, "invoke" | "reflectInvoke" | "stringCode")
                }) && mutation.arguments.first().is_none_or(|target| {
                    target.spread || !separate_iteration_target(candidate, target.span, 24)
                })
            })
    })
}

fn source_bound_invocation(file: &FileFacts, callee: Span, lookup: &SemanticLookup<'_>) -> bool {
    let Some(bound) = file.ast.call_at(file.ast.peel_ts_sugar_span(callee)) else {
        return false;
    };
    let Some(declaration) =
        lookup.standard_library_declaration(file, bound, typefacts::CallKind::Call)
    else {
        return false;
    };
    declaration.qualified_name.ends_with(".bind")
        && tuple_mutation_method(file, bound, lookup) == Some("invoke")
        && file
            .ast
            .members
            .iter()
            .find(|member| member.span == file.ast.peel_ts_sugar_span(bound.callee))
            .is_some_and(|member| {
                member_key(file, member) == Some("bind")
                    && iteration_source_function(file, member.object, lookup)
            })
}

/// A store-setter type corroborates the binding, but does not prove runtime
/// identity alone. Require the exact unwritten slot produced by a dialect
/// primitive as well. Casts and declared-only setters cannot discharge a call.
fn intrinsic_disjoint_store_setter(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
    {
        return false;
    }
    let Some((origin, binding, symbol)) = lookup.binding_at_reference(file.path.as_str(), callee)
    else {
        return false;
    };
    let Some(slot) = binding.array_slots.get(1).and_then(Option::as_ref) else {
        return false;
    };
    let store_role = lookup
        .entity_at(file.path.as_str(), callee)
        .or_else(|| lookup.entity_at(origin.path.as_str(), slot.span))
        .and_then(|entity| entity.type_descriptor.as_ref())
        .is_some_and(|descriptor| {
            descriptor.alias_declarations.iter().any(|declaration| {
                lookup
                    .dialect
                    .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                    == Some(solid_dialect::TypeRole::StoreSetter)
            })
        });
    simple_array_binding(binding)
        && lookup.entity_symbol(origin, slot.span) == Some(symbol.as_str())
        && !crate::indexes::binding_written(origin, slot.span)
        && binding
            .initializer
            .and_then(|span| origin.ast.call_at(origin.ast.peel_ts_sugar_span(span)))
            .and_then(|created| lookup.primitive_at_call(origin, created.span))
            .is_some_and(|primitive| {
                lookup.dialect.returns_reactive_tuple(primitive)
                    && (store_role || lookup.dialect.returns_store(primitive))
            })
}

/// ADR 0292: whether any analyzed file holds a reflective write
/// (`Object.assign`, `defineProperty`, `Reflect.set`, ...) whose target is
/// spelled through a `prototype`, the one write [`MemberStability`] lets reach
/// a fresh literal nothing else holds (ADR 0282, narrowed by ADR 0288's
/// review). The same visible veto, for a fresh module object.
pub(crate) fn reflective_prototype_write(lookup: &SemanticLookup<'_>) -> bool {
    lookup.files().iter().any(|file| {
        file.ast.calls.iter().any(|call| {
            targets_prototype(file, call) && mutation_builtin(file, call, lookup).is_some()
        })
    })
}

/// Signature and runtime receiver identities are independent, as in ADR 0272.
/// A local ObjectConstructor-typed value or a homonymous helper is not Object.
fn mutation_builtin<'a>(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let (global, property, library) = match declaration.qualified_name.as_ref() {
        "ObjectConstructor.defineProperty" => ("Object", "defineProperty", "lib.es5.d.ts"),
        "ObjectConstructor.defineProperties" => ("Object", "defineProperties", "lib.es5.d.ts"),
        "ObjectConstructor.assign" => ("Object", "assign", "lib.es2015.core.d.ts"),
        "Reflect.set" => ("Reflect", "set", "lib.es2015.reflect.d.ts"),
        "Reflect.defineProperty" => ("Reflect", "defineProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.deleteProperty" => ("Reflect", "deleteProperty", "lib.es2015.reflect.d.ts"),
        _ => return None,
    };
    let member = file.ast.members.iter().find(|member| {
        member.span == file.ast.peel_ts_sugar_span(call.callee)
            && !file.ast.computed_members.contains(&member.span)
            && file.source_text(member.property) == Some(property)
    })?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if file.ast.reference_declaration(receiver).is_some() {
        return None;
    }
    let entity = lookup.entity_at(file.path.as_str(), receiver)?;
    if entity.symbol_unresolved || entity.symbol.is_empty() {
        return None;
    }
    let symbol = lookup.symbol_at(file.path.as_str(), receiver)?;
    if !symbol.alias_target().is_empty()
        || symbol.declarations().iter().any(|binding| {
            lookup
                .file_by_path(binding.location.path.as_ref())
                .is_some()
        })
    {
        return None;
    }
    let mut bindings = symbol.declarations().iter().filter(|binding| {
        binding.name.as_ref() == global
            && (global == "Reflect" || binding.kind.as_ref() == "variable")
    });
    let binding = bindings.next()?;
    // Object's runtime variable is in es5 even when assign's signature is in es2015.core.
    let expected = std::path::Path::new(declaration.source_file.as_ref())
        .parent()?
        .join(if global == "Object" {
            "lib.es5.d.ts"
        } else {
            library
        });
    (bindings.next().is_none()
        && std::path::Path::new(binding.location.path.as_ref()) == expected
        && declaration.location.path == declaration.source_file
        && std::path::Path::new(declaration.source_file.as_ref())
            .file_name()?
            .to_str()?
            == library)
        .then_some(property)
}

#[derive(Clone, Copy, Default)]
struct ClosedUses {
    object_escaped: bool,
    array_escaped: bool,
    callable_escaped: bool,
    context_escaped: bool,
    context_value_escaped: bool,
    destructured_escaped: bool,
}

impl ClosedUses {
    /// An exported binding's shared object can be replaced by any importer.
    fn export(&mut self) {
        self.object_escaped = true;
        self.array_escaped = true;
        self.context_value_escaped = true;
        self.destructured_escaped = true;
    }
}

impl Resolver<'_, '_> {
    pub fn new<'a, 'facts>(
        facts: &'a ProjectFacts,
        lookup: &'a SemanticLookup<'facts>,
        accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
        source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
        names: &'a HashMap<SymbolId, SymbolId>,
        functions: HashSet<(String, Span)>,
    ) -> Resolver<'a, 'facts> {
        let mut resolver = Resolver {
            facts,
            lookup,
            accessors,
            source_kinds,
            names,
            functions,
            binding_locations: HashMap::new(),
            parameter_inputs: HashMap::new(),
            declarations: HashMap::new(),
            uses: HashMap::new(),
            providers: HashMap::new(),
            source_origins: HashSet::new(),
            runtime_source_origins: HashSet::new(),
            blocked_contexts: HashSet::new(),
            unkeyed_provider: false,
            member_stability: MemberStability::index(facts, lookup),
            receiver_channels_opaque: facts.files.iter().any(|file| {
                file.ast
                    .module_hazards
                    .iter()
                    .any(|hazard| hazard.kind == ast::ModuleHazardKind::Eval)
                    || file.ast.calls.iter().any(|call| {
                        lookup
                            .resolved_callee_call(file, call.callee)
                            .and_then(|resolved| resolved.declaration.as_ref())
                            .is_some_and(|declaration| {
                                declaration.standard_library
                                    && (declaration
                                        .qualified_name
                                        .starts_with("FunctionConstructor")
                                        || declaration.qualified_name.as_ref() == "eval")
                            })
                    })
            }),
        };
        for file in &facts.files {
            for binding in &file.ast.bindings {
                for name in &binding.names {
                    if let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) {
                        resolver.binding_locations.insert(
                            symbol.clone(),
                            location(file.path.shared(), binding.pattern),
                        );
                        if let Some(display) = file.source_text(name.span) {
                            resolver.declarations.insert(
                                symbol.clone(),
                                (
                                    SymbolId::from(display),
                                    location(file.path.shared(), name.span),
                                ),
                            );
                        }
                    }
                }
            }
        }
        resolver.index_parameter_inputs();
        let forwarded_arguments = resolver
            .parameter_inputs
            .values()
            .map(|(path, span)| location(path.as_str(), *span))
            .collect::<HashSet<_>>();
        // Census each exact reference once. A receiver proves closure only
        // when its selected member is immediately called; a Get that escapes
        // an object/member cannot certify a later invocation.
        for file in &facts.files {
            let array_receivers = file
                .ast
                .calls
                .iter()
                .filter_map(|call| array_iteration_receiver(file, call, lookup))
                .collect::<HashSet<_>>();
            let callees = file
                .ast
                .calls
                .iter()
                .map(|c| file.ast.peel_ts_sugar_span(c.callee))
                .collect::<HashSet<_>>();
            let receivers = file
                .ast
                .members
                .iter()
                .filter(|m| {
                    !file.ast.computed_members.contains(&m.span) && callees.contains(&m.span)
                })
                .map(|m| file.ast.peel_ts_sugar_span(m.object))
                .collect::<HashSet<_>>();
            let returned = file
                .ast
                .returns
                .iter()
                .filter_map(|r| r.argument)
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .collect::<HashSet<_>>();
            // An exact context wrapper may forward its immutable intermediate
            // binding once. This is a handoff to the eventual receiver census,
            // not permission for an arbitrary object return or other escape.
            let context_returns = file
                .ast
                .functions
                .iter()
                .filter(|f| !f.r#async && !f.generator && lookup.function_value_is_current(file, f))
                .filter_map(|f| single_return(file, f))
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .filter(|s| resolver.context_origin(file, *s, 24).is_some())
                .collect::<HashSet<_>>();
            let properties = file
                .ast
                .object_properties
                .iter()
                .filter(|p| p.data)
                .map(|p| file.ast.peel_ts_sugar_span(p.value))
                .collect::<HashSet<_>>();
            let destructured_initializers = file
                .ast
                .bindings
                .iter()
                .filter(|binding| simple_object_binding(binding))
                .filter_map(|binding| binding.initializer)
                .map(|span| file.ast.peel_ts_sugar_span(span))
                .collect::<HashSet<_>>();
            let guard_operands = file
                .ast
                .conditional_tests
                .iter()
                .copied()
                .chain(file.ast.nullish_test_operands.iter().copied())
                .chain(file.ast.coercive_operands.iter().filter_map(|operand| {
                    (operand.kind == ast::CoerciveOperandKind::LogicalNot).then_some(operand.span)
                }))
                .map(|span| file.ast.peel_ts_sugar_span(span))
                .collect::<HashSet<_>>();
            let mut provider_values = HashSet::new();
            let mut context_tags = HashSet::new();
            for element in &file.ast.jsx_elements {
                let target = if let Some(member) = lookup.dialect.context_provider_member() {
                    (element.member_property.and_then(|p| file.source_text(p)) == Some(member))
                        .then_some(element.member_object)
                        .flatten()
                } else {
                    Some(element.name.span)
                };
                let Some(target) = target else { continue };
                if !lookup.is_context_reference(file.path.as_str(), target) {
                    continue;
                }
                // A provider whose tag is not one exact identifier (a
                // namespace member, for one) cannot be attributed to one
                // context binding, so no context's provider census is closed.
                let key = exact_identifier(file, target)
                    .then(|| resolver.binding_key(file, target))
                    .flatten();
                if key.is_none() {
                    resolver.unkeyed_provider = true;
                }
                if let Some(key) = key {
                    let mut values = element
                        .attributes
                        .iter()
                        .filter(|a| file.source_text(a.local_name) == Some("value"));
                    let value = values.next().and_then(|a| a.expression);
                    let valid = values.next().is_none() && element.spreads.is_empty();
                    resolver
                        .providers
                        .entry(key)
                        .or_default()
                        .push(value.filter(|_| valid).map(|v| (file.path.to_string(), v)));
                    if let Some(value) = value.filter(|_| valid) {
                        provider_values.insert(file.ast.peel_ts_sugar_span(value));
                    }
                    context_tags.insert(target);
                    context_tags.insert(element.name.span);
                    context_tags.extend(element.closing_name);
                }
            }
            let mut context_reads = HashSet::new();
            for call in &file.ast.calls {
                if resolver.role(file, call) == Some(ContextRole::Read)
                    && call.arguments.len() == 1
                    && !call.arguments[0].spread
                {
                    context_reads.insert(file.ast.peel_ts_sugar_span(call.arguments[0].span));
                    continue;
                }
                for (position, argument) in call.arguments.iter().enumerate() {
                    let span = file.ast.peel_ts_sugar_span(argument.span);
                    let context = exact_identifier(file, span)
                        && resolver
                            .lookup
                            .binding_at_reference(file.path.as_str(), span)
                            .is_some_and(|(context_file, binding, _)| {
                                binding
                                    .call_initializer
                                    .and_then(|created| context_file.ast.call_at(created))
                                    .is_some_and(|created| {
                                        resolver.role(context_file, created)
                                            == Some(ContextRole::Create)
                                    })
                            });
                    if context && resolver.context_read_only_argument(file, call, position, 8) {
                        context_reads.insert(span);
                    }
                }
            }
            // In a closed program, an exact export or re-export publishes the
            // binding: every visible importer is itself a censused reference.
            // Binding resolution below still has to identify its exact root.
            // It is therefore
            // censused as an export rather than as an escaping reference.
            // An open program keeps the specifier an escape.
            let export_specifiers = file
                .ast
                .exports
                .iter()
                .filter(|e| lookup.program_closed && !e.type_only)
                .flat_map(|e| e.specifiers.iter().filter(|s| !s.type_only))
                .map(|s| s.local.span)
                .collect::<HashSet<_>>();
            for id in file
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                let Some(key) = resolver.binding_key(file, id.span) else {
                    continue;
                };
                if export_specifiers.contains(&id.span) {
                    resolver.uses.entry(key).or_default().export();
                    continue;
                }
                let written = file
                    .ast
                    .assignments
                    .iter()
                    .any(|a| a.target.contains(id.span))
                    || file.ast.deleted_targets.iter().any(|s| s.contains(id.span))
                    || file
                        .ast
                        .iteration_targets
                        .iter()
                        .any(|s| s.contains(id.span));
                let forwarded =
                    forwarded_arguments.contains(&location(file.path.shared(), id.span));
                let context_guard = guard_operands.contains(&id.span)
                    && resolver.context_origin(file, id.span, 24).is_some();
                let uses = resolver.uses.entry(key).or_default();
                uses.object_escaped |= written
                    || !(receivers.contains(&id.span)
                        || provider_values.contains(&id.span)
                        || context_returns.contains(&id.span)
                        || context_guard
                        || destructured_initializers.contains(&id.span)
                        || forwarded);
                uses.array_escaped |= written || !(array_receivers.contains(&id.span) || forwarded);
                uses.callable_escaped |= written
                    || !(callees.contains(&id.span)
                        || returned.contains(&id.span)
                        || properties.contains(&id.span)
                        || provider_values.contains(&id.span));
                uses.context_escaped |= written
                    || !(context_reads.contains(&id.span) || context_tags.contains(&id.span));
                uses.context_value_escaped |= written
                    || !(receivers.contains(&id.span)
                        || callees.contains(&id.span)
                        || provider_values.contains(&id.span)
                        || context_returns.contains(&id.span)
                        || context_guard
                        || destructured_initializers.contains(&id.span)
                        || forwarded);
                uses.destructured_escaped |= written || !callees.contains(&id.span);
            }
            for export in file.ast.exports.iter() {
                for binding in file.ast.exported_bindings(export) {
                    resolver
                        .uses
                        .entry(location(file.path.shared(), binding.pattern))
                        .or_default()
                        .export();
                }
            }
        }
        // A handoff permits no new escape. Join the exact parameter's use
        // census back into its argument binding, including chained handoffs.
        // Index the handoff edges once; the fixed point queries only edges,
        // never rescans the project or repeats binding resolution.
        let mut handoffs = resolver
            .parameter_inputs
            .iter()
            .filter_map(|(parameter, (path, span))| {
                // Only an argument that is exactly a binding hands that binding
                // over; `f(model.read())` passes the read's result, not `model`.
                let file = lookup.file_by_path(path)?;
                if !exact_identifier(file, *span) {
                    return None;
                }
                let argument = resolver.binding_key(file, *span)?;
                Some((argument, parameter.clone(), false))
            })
            .collect::<Vec<_>>();
        for file in &facts.files {
            for binding in file
                .ast
                .bindings
                .iter()
                .filter(|binding| simple_object_binding(binding))
            {
                if let Some(argument) = binding
                    .initializer
                    .map(|span| file.ast.peel_ts_sugar_span(span))
                    .filter(|span| exact_identifier(file, *span))
                    .and_then(|span| resolver.binding_key(file, span))
                {
                    handoffs.push((
                        argument,
                        location(file.path.shared(), binding.pattern),
                        true,
                    ));
                }
            }
        }
        loop {
            let mut changed = false;
            for (argument, receiver, destructured) in &handoffs {
                let mut input = resolver.uses.get(receiver).copied().unwrap_or_default();
                if *destructured {
                    input = ClosedUses {
                        object_escaped: input.destructured_escaped,
                        context_value_escaped: input.destructured_escaped,
                        ..Default::default()
                    };
                }
                let uses = resolver.uses.entry(argument.clone()).or_default();
                for (target, source) in [
                    (&mut uses.object_escaped, input.object_escaped),
                    (&mut uses.array_escaped, input.array_escaped),
                    (&mut uses.callable_escaped, input.callable_escaped),
                    (&mut uses.context_value_escaped, input.context_value_escaped),
                ] {
                    changed |= source && !*target;
                    *target |= source;
                }
            }
            if !changed {
                break;
            }
        }
        for file in &facts.files {
            for binding in &file.ast.bindings {
                if !binding.immutable {
                    continue;
                }
                for name in &binding.names {
                    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
                        continue;
                    };
                    if source_has_runtime_witness(
                        symbol,
                        accessors,
                        lookup.entities(),
                        names,
                        lookup,
                    ) {
                        resolver.runtime_source_origins.insert(symbol.clone());
                    } else if !crate::indexes::binding_written(file, name.span)
                        && resolver.source_call(file, name.span).as_ref() == Some(symbol)
                    {
                        resolver.source_origins.insert(symbol.clone());
                    }
                }
            }
        }
        // Join every exact context-derived receiver, including wrapper-return
        // bindings and aliases, before resolving any transported call. One
        // escaped/mutated receiver can replace the shared provider's members.
        let mut blocked_contexts = HashSet::new();
        let object_contexts = resolver
            .providers
            .iter()
            .filter_map(|(context, providers)| {
                let [Some((path, span))] = providers.as_slice() else {
                    return None;
                };
                let file = resolver.lookup.file_by_path(path)?;
                matches!(
                    resolver.value(file, *span, 24, &mut false),
                    Some(Value::Object { .. })
                )
                .then(|| context.clone())
            })
            .collect::<HashSet<_>>();
        for file in facts
            .files
            .iter()
            .filter(|_| !resolver.providers.is_empty())
        {
            for binding in file
                .ast
                .bindings
                .iter()
                .chain(file.ast.functions.iter().flat_map(|f| f.parameters.iter()))
            {
                let Some(context) = binding
                    .initializer
                    .or_else(|| binding.names.first().map(|name| name.span))
                    .and_then(|span| resolver.context_origin(file, span, 24))
                else {
                    continue;
                };
                let key = location(file.path.shared(), binding.pattern);
                let parameter = resolver.parameter_inputs.contains_key(&key);
                let destructured = simple_object_binding(binding);
                if (!binding.immutable && !parameter)
                    || !(binding.shape == ast::BindingShape::Identifier && binding.names.len() == 1
                        || destructured)
                    || resolver.uses.get(&key).is_some_and(|uses| {
                        if destructured {
                            uses.destructured_escaped
                        } else {
                            uses.context_value_escaped
                                || (object_contexts.contains(&context) && uses.object_escaped)
                        }
                    })
                    || binding
                        .names
                        .iter()
                        .any(|name| crate::indexes::binding_written(file, name.span))
                {
                    blocked_contexts.insert(context);
                }
            }
            let bound_values = file
                .ast
                .bindings
                .iter()
                .filter_map(|b| b.initializer)
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .collect::<HashSet<_>>();
            let wrapper_returns = file
                .ast
                .functions
                .iter()
                .filter(|f| {
                    !f.r#async && !f.generator && resolver.lookup.function_value_is_current(file, f)
                })
                .filter_map(|f| single_return(file, f))
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .collect::<HashSet<_>>();
            let callees = file
                .ast
                .calls
                .iter()
                .map(|c| file.ast.peel_ts_sugar_span(c.callee))
                .collect::<HashSet<_>>();
            let receivers = file
                .ast
                .members
                .iter()
                .filter(|m| {
                    !file.ast.computed_members.contains(&m.span) && callees.contains(&m.span)
                })
                .map(|m| file.ast.peel_ts_sugar_span(m.object))
                .collect::<HashSet<_>>();
            // An unbound result can mutate/escape the same shared object too.
            // Exact wrapper returns and initializer handoffs are checked at
            // their eventual bindings; direct invocations remain closed.
            for call in &file.ast.calls {
                if bound_values.contains(&call.span)
                    || wrapper_returns.contains(&call.span)
                    || callees.contains(&call.span)
                    || receivers.contains(&call.span)
                    || forwarded_arguments.contains(&location(file.path.shared(), call.span))
                {
                    continue;
                }
                if let Some(context) = resolver.context_origin(file, call.span, 24) {
                    blocked_contexts.insert(context);
                }
            }
        }
        resolver.blocked_contexts = blocked_contexts;
        resolver
    }

    fn binding_key(&self, file: &FileFacts, span: Span) -> Option<Location> {
        if let Some((declared, binding, _)) =
            self.lookup.binding_at_reference(file.path.as_str(), span)
        {
            return Some(location(declared.path.shared(), binding.pattern));
        }
        if let Some(key) = self
            .lookup
            .entities()
            .at(file.path.as_str(), span)
            .and_then(|s| self.binding_locations.get(s))
        {
            return Some(key.clone());
        }
        let declaration = file.ast.reference_declaration(span)?;
        let binding = file
            .ast
            .bindings
            .iter()
            .chain(file.ast.functions.iter().flat_map(|f| f.parameters.iter()))
            .find(|b| b.names.iter().any(|n| n.span == declaration))?;
        Some(location(file.path.shared(), binding.pattern))
    }

    /// One source-local function value, one exact identifier call, and no
    /// unaccounted reference. Do not use call-site count alone: a stored or
    /// aliased value can have an invocation missing from that count.
    fn index_parameter_inputs(&mut self) {
        let mut local_functions = HashMap::new();
        for file in &self.facts.files {
            for function in &file.ast.functions {
                if !function.parameters.is_empty()
                    && let Some(name) = crate::owners::function_binding_name(file, function)
                {
                    local_functions.insert(
                        location(file.path.shared(), name.span),
                        (file.path.to_string(), function.span),
                    );
                }
            }
        }
        if local_functions.is_empty() {
            return;
        }
        let mut references = HashMap::<(String, Span), HashSet<Location>>::new();
        let mut argument_observers = HashSet::new();
        for file in &self.facts.files {
            for id in file
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                if matches!(file.source_text(id.span), Some("arguments" | "eval")) {
                    for function in file.ast.functions_body_containing(id.span) {
                        argument_observers.insert((file.path.to_string(), function.span));
                    }
                }
                let target = file.ast.reference_declaration(id.span).and_then(|span| {
                    local_functions
                        .get(&location(file.path.shared(), span))
                        .cloned()
                });
                if let Some(target) = target {
                    references
                        .entry(target)
                        .or_default()
                        .insert(location(file.path.shared(), id.span));
                }
            }
        }
        for file in &self.facts.files {
            for function in &file.ast.functions {
                if function.parameters.is_empty()
                    || function.generator
                    || function.rest_parameter
                    || !self.lookup.function_value_is_current(file, function)
                    || function.parameters.iter().any(|p| {
                        p.initializer.is_some()
                            || p.shape != ast::BindingShape::Identifier
                            || p.names.len() != 1
                            || p.pattern != p.names[0].span
                    })
                    || argument_observers.contains(&(file.path.to_string(), function.span))
                {
                    continue;
                }
                let Some(name) = crate::owners::function_binding_name(file, function) else {
                    continue;
                };
                let exported =
                    self.lookup
                        .typescript_file(file.path.as_str())
                        .is_some_and(|typescript| {
                            typescript.functions.iter().any(|f| {
                                f.exported
                                    && f.body.start_byte == u64::from(function.body.start)
                                    && f.body.end_byte == u64::from(function.body.end)
                            })
                        })
                        || file.ast.exports.iter().any(|export| {
                            (export.span.contains(function.span)
                                && !file.ast.functions.iter().any(|outer| {
                                    outer.span != function.span
                                        && export.span.contains(outer.span)
                                        && outer.span.contains(function.span)
                                }))
                                || export.specifiers.iter().chain(&export.declarations).any(
                                    |specifier| {
                                        !specifier.type_only
                                            && (specifier.local.span == name.span
                                                || file
                                                    .ast
                                                    .reference_declaration(specifier.local.span)
                                                    == Some(name.span))
                                    },
                                )
                        });
                // Reuse ADR 0193's open-program boundary, and keep this first
                // parameter channel narrower: even app exports are refused.
                if exported && !self.lookup.program_closed {
                    continue;
                }
                if exported {
                    continue;
                }
                let target = (file.path.to_string(), function.span);
                let Some(uses) = references.get(&target) else {
                    continue;
                };
                if uses.len() != 1 {
                    continue;
                }
                let site = uses.iter().next().expect("one reference");
                let sites = self
                    .lookup
                    .function_call_sites(file.path.as_str(), function.span);
                let [(caller, callee)] = sites.as_slice() else {
                    continue;
                };
                let callee = caller.ast.peel_ts_sugar_span(*callee);
                if *site != location(caller.path.shared(), callee)
                    || caller.path != file.path
                    || caller.ast.reference_declaration(callee) != Some(name.span)
                {
                    continue;
                }
                let Some(call) = caller
                    .ast
                    .calls
                    .iter()
                    .find(|c| caller.ast.peel_ts_sugar_span(c.callee) == callee)
                else {
                    continue;
                };
                if call.construct
                    || call.arguments.iter().any(|argument| argument.spread)
                    || !self
                        .lookup
                        .resolved_callee_call(caller, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Valid
                        })
                {
                    continue;
                }
                // Include the producer reference census too: a reference in
                // excluded/missing source is never evidence of closure.
                if self
                    .lookup
                    .symbol_at(file.path.as_str(), name.span)
                    .is_none_or(|symbol| {
                        symbol.references().any(|reference| {
                            reference.path.as_ref() != caller.path.as_str()
                                || reference.start_byte != u64::from(callee.start)
                                || reference.end_byte != u64::from(callee.end)
                        })
                    })
                {
                    continue;
                }
                for (position, parameter) in function.parameters.iter().enumerate() {
                    let name = &parameter.names[0];
                    if crate::indexes::binding_written(file, name.span) {
                        continue;
                    }
                    let Some(argument) = call.arguments.get(position) else {
                        continue;
                    };
                    let key = location(file.path.shared(), parameter.pattern);
                    self.parameter_inputs.insert(
                        key.clone(),
                        (
                            caller.path.to_string(),
                            caller.ast.peel_ts_sugar_span(argument.span),
                        ),
                    );
                    if let Some(symbol) = self.lookup.entities().at(file.path.as_str(), name.span) {
                        self.binding_locations.insert(symbol.clone(), key);
                    }
                }
            }
        }
    }

    pub fn has_source_origin(&self, symbol: &SymbolId) -> bool {
        self.source_origins.contains(symbol)
    }
    pub fn has_runtime_source_origin(&self, symbol: &SymbolId) -> bool {
        self.runtime_source_origins.contains(symbol)
    }
    pub fn proven_source_origins(&self) -> impl Iterator<Item = &SymbolId> {
        self.runtime_source_origins
            .iter()
            .chain(self.source_origins.iter())
    }
    pub fn has_extra_source_origins(&self) -> bool {
        !self.source_origins.is_empty()
    }
    pub fn source_call(&self, file: &FileFacts, callee: Span) -> Option<SymbolId> {
        match self.value(file, callee, 24, &mut false)? {
            Value::Callable(Callable::Accessor(symbol)) => Some(symbol),
            _ => None,
        }
    }
    pub fn calls(&self) -> (HashMap<Location, Callable>, HashSet<Location>) {
        let mut calls = HashMap::new();
        let mut unavailable = HashSet::new();
        for call_file in &self.facts.files {
            for call in &call_file.ast.calls {
                if let Some(targets) = self.array_members(call_file, call) {
                    let callee = location(call_file.path.shared(), call.callee);
                    if targets.iter().all(|target| self.functions.contains(target)) {
                        calls.insert(callee, Callable::ArrayMembers(targets));
                    } else if targets.iter().any(|(path, span)| {
                        let file = self
                            .lookup
                            .file_by_path(path)
                            .expect("resolved array member file");
                        let function = file
                            .ast
                            .functions
                            .iter()
                            .find(|f| f.span == *span)
                            .expect("resolved array member");
                        file.ast
                            .calls
                            .iter()
                            .any(|c| function.body.contains(c.span))
                            || file
                                .ast
                                .members
                                .iter()
                                .any(|m| function.body.contains(m.span))
                    }) {
                        unavailable.insert(callee);
                    }
                    continue;
                }
                let mut transported = false;
                let value = self.value(call_file, call.callee, 24, &mut transported);
                // Callable identity is not constructability. In particular,
                // a type assertion cannot turn a returned method/arrow into
                // a constructor whose body runs. Keep this transport surface
                // explicit rather than projecting its ordinary-call summary.
                if call.construct && transported {
                    unavailable.insert(location(call_file.path.shared(), call.callee));
                    continue;
                }
                if transported {
                    match value {
                        Some(Value::Callable(Callable::Function { path, span }))
                            if !self.functions.contains(&(path.clone(), span)) =>
                        {
                            let file = self
                                .lookup
                                .file_by_path(&path)
                                .expect("resolved function file");
                            let function = file
                                .ast
                                .functions
                                .iter()
                                .find(|f| f.span == span)
                                .expect("resolved function span");
                            // Preserve the missing implementation obligation without
                            // admitting a synthetic node or inferring its summary.
                            if file
                                .ast
                                .calls
                                .iter()
                                .any(|c| function.body.contains(c.span))
                                || file
                                    .ast
                                    .members
                                    .iter()
                                    .any(|m| function.body.contains(m.span))
                            {
                                unavailable.insert(location(call_file.path.shared(), call.callee));
                            }
                        }
                        Some(Value::Callable(target)) => {
                            calls.insert(location(call_file.path.shared(), call.callee), target);
                        }
                        Some(Value::VetoedTuple) => {
                            unavailable.insert(location(call_file.path.shared(), call.callee));
                        }
                        _ => {}
                    }
                }
            }
        }
        (calls, unavailable)
    }

    fn array_members(&self, file: &FileFacts, call: &ast::CallFact) -> Option<Vec<(String, Span)>> {
        let receiver = array_iteration_receiver(file, call, self.lookup)?;
        // A stored or named callback's witnessed use cannot establish its
        // other invocations. Leave those definitions and obligations alone;
        // only direct call-site execution and ordinary helper entry compose.
        if super::runs_in_object_callable(file, call.span)
            || crate::execution_role::named_callback_execution_role(file, call.callee, self.lookup)
                .is_some()
        {
            return None;
        }
        if call.arguments.len() != 1 || call.arguments[0].spread {
            return None;
        }
        let callback_span = file.ast.peel_ts_sugar_span(call.arguments[0].span);
        let callback = file
            .ast
            .functions
            .iter()
            .find(|f| f.span == callback_span)?;
        if callback.kind != ast::FunctionKind::Arrow
            || callback.r#async
            || callback.generator
            || !callback.expression_body
            || callback.parameters.len() != 1
            || callback.rest_parameter
        {
            return None;
        }
        let parameter = &callback.parameters[0];
        let [name] = parameter.names.as_slice() else {
            return None;
        };
        if parameter.initializer.is_some() || parameter.pattern != name.span {
            return None;
        }
        // Exactly one invocation; no deferred nested body, alias or element escape.
        let mut invocations = file
            .ast
            .calls
            .iter()
            .filter(|c| callback.body.contains(c.span));
        let invocation = invocations.next()?;
        if invocations.next().is_some()
            || invocation.construct
            || invocation.arguments.iter().any(|a| a.spread)
            || file.ast.peel_ts_sugar_span(callback.body) != invocation.span
            || !super::body_site_runs_during_call(file, callback, invocation.span)
        {
            return None;
        }
        let member = file.ast.members.iter().find(|m| {
            m.span == file.ast.peel_ts_sugar_span(invocation.callee)
                && !file.ast.computed_members.contains(&m.span)
        })?;
        let object = file.ast.peel_ts_sugar_span(member.object);
        if file.ast.reference_declaration(object) != Some(name.span)
            || file.ast.identifiers.iter().any(|id| {
                id.role == ast::IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span) == Some(name.span)
                    && id.span != object
            })
        {
            return None;
        }
        let name = file.source_text(member.property)?;
        let Value::Array { path, elements } = self.value(file, receiver, 24, &mut false)? else {
            return None;
        };
        // The array and its element objects are created in `path`.
        if ["map", "forEach", name].into_iter().any(|member| {
            matches!(
                self.member_stability.state_of(self.facts, &path, member),
                Stability::Vetoed(_)
            )
        }) {
            return None;
        }
        let array_file = self.lookup.file_by_path(&path)?;
        let mut targets = Vec::new();
        for element in elements {
            let properties = closed_properties(array_file, element)?;
            let property = properties.into_iter().find(|p| {
                array_file
                    .source_text(p.key)
                    .map(|s| s.trim_matches(['\'', '"']))
                    == Some(name)
            })?;
            let Value::Callable(Callable::Function { path, span }) = self.value(
                array_file,
                property.shorthand_binding.unwrap_or(property.value),
                24,
                &mut false,
            )?
            else {
                return None;
            };
            let selected = self
                .lookup
                .file_by_path(&path)?
                .ast
                .functions
                .iter()
                .find(|f| f.span == span)?;
            if selected.kind != ast::FunctionKind::Arrow || selected.r#async || selected.generator {
                return None;
            }
            if !targets.contains(&(path.clone(), span)) {
                targets.push((path, span));
            }
        }
        (!targets.is_empty()).then_some(targets)
    }

    pub fn read(&self, symbol: &SymbolId, file: &FileFacts, span: Span) -> Option<SummaryRead> {
        let (display, declaration) = self
            .accessors
            .get(symbol)
            .or_else(|| self.declarations.get(symbol))
            .cloned()?;
        Some(SummaryRead {
            transported: true,
            contract_read_context: None,
            symbol: symbol.clone(),
            display,
            kind: Some("accessor".into()),
            declaration,
            origin: location(file.path.shared(), span),
            origin_context: crate::owners::enclosing_function_label(file, span),
            owner: None,
            local: false,
            conditional: false,
            undecided: false,
        })
    }

    fn role(&self, file: &FileFacts, call: &ast::CallFact) -> Option<ContextRole> {
        let primitive = crate::call_primitive_name(
            file,
            call,
            self.lookup.entities(),
            self.names,
            self.lookup.dialect,
        )?
        .primitive()?;
        self.lookup
            .dialect
            .context_role(self.lookup.dialect.name_of(primitive)?)
    }

    /// Every use of this exact parameter only reads a context, directly or
    /// through another current source function with the same complete census.
    /// A provider, mutation, alias, export, return or opaque handoff refuses.
    fn context_read_only_argument(
        &self,
        file: &FileFacts,
        call: &ast::CallFact,
        position: usize,
        depth: usize,
    ) -> bool {
        if depth == 0
            || call.construct
            || call.arguments.iter().take(position + 1).any(|a| a.spread)
        {
            return false;
        }
        if self.role(file, call) == Some(ContextRole::Read) {
            return position == 0 && call.arguments.len() == 1;
        }
        let Some((target_file, function)) = self
            .lookup
            .function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
        else {
            return false;
        };
        let Some(parameter) = function.parameters.get(position) else {
            return false;
        };
        if function.rest_parameter
            || function.r#async
            || function.generator
            || !self.lookup.function_value_is_current(target_file, function)
            || parameter.initializer.is_some()
            || parameter.shape != ast::BindingShape::Identifier
            || parameter.names.len() != 1
            || crate::indexes::binding_written(target_file, parameter.pattern)
            || target_file.ast.identifiers.iter().any(|id| {
                function.body.contains(id.span)
                    && id.role == ast::IdentifierRole::Reference
                    && matches!(target_file.source_text(id.span), Some("arguments" | "eval"))
            })
        {
            return false;
        }
        let mut references = target_file
            .ast
            .identifiers
            .iter()
            .filter(|id| {
                id.role == ast::IdentifierRole::Reference
                    && target_file.ast.reference_declaration(id.span)
                        == Some(parameter.names[0].span)
            })
            .peekable();
        references.peek().is_some()
            && references.all(|id| {
                target_file.ast.calls.iter().any(|inner| {
                    inner
                        .arguments
                        .iter()
                        .enumerate()
                        .any(|(position, argument)| {
                            target_file.ast.peel_ts_sugar_span(argument.span) == id.span
                                && self.context_read_only_argument(
                                    target_file,
                                    inner,
                                    position,
                                    depth - 1,
                                )
                        })
                })
            })
    }

    fn value(
        &self,
        file: &FileFacts,
        span: Span,
        depth: usize,
        transported: &mut bool,
    ) -> Option<Value> {
        if depth == 0 {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        let declaration = file.ast.reference_declaration(span).unwrap_or(span);
        if let Some((path, argument)) = self
            .parameter_inputs
            .get(&location(file.path.shared(), declaration))
        {
            let argument_file = self.lookup.file_by_path(path)?;
            let mut origin_transport = false;
            let value = self.value(argument_file, *argument, depth - 1, &mut origin_transport)?;
            if !origin_transport {
                return None;
            }
            let uses = self.uses.get(&location(file.path.shared(), declaration));
            let closed = match &value {
                Value::Object { .. } => uses.is_none_or(|uses| !uses.object_escaped),
                Value::Array { .. } => uses.is_none_or(|uses| !uses.array_escaped),
                // ADR 0282: a handed-off tuple is a held array, not a fresh one.
                Value::Callable(_) | Value::Tuple { .. } | Value::VetoedTuple => false,
            };
            if !closed {
                return None;
            }
            *transported = true;
            return Some(value);
        }
        if let Some(function) = file.ast.functions.iter().find(|f| f.span == span) {
            if function.generator || function.r#async {
                return None;
            }
            return Some(Value::Callable(Callable::Function {
                path: file.path.to_string(),
                span,
            }));
        }
        let declared_function = file
            .ast
            .identifiers
            .iter()
            .any(|id| id.span == span)
            .then(|| {
                self.lookup
                    .callee_symbol(file, span)
                    .and_then(|symbol| self.lookup.function_for_symbol(symbol))
            })
            .flatten()
            .or_else(|| self.lookup.namespace_member_function(file, span));
        if let Some((target_file, function)) = declared_function
            && function.kind == ast::FunctionKind::Declaration
            && self.lookup.function_value_is_current(target_file, function)
        {
            return Some(Value::Callable(Callable::Function {
                path: target_file.path.to_string(),
                span: function.span,
            }));
        }
        if let Some(member) = file.ast.members.iter().find(|m| m.span == span) {
            if file.ast.computed_members.contains(&span) {
                return None;
            }
            let Value::Object {
                path, span: object, ..
            } = self.value(file, member.object, depth - 1, transported)?
            else {
                return None;
            };
            let object_file = self.lookup.file_by_path(&path)?;
            let name = file.source_text(member.property)?;
            return self.member_value(object_file, object, name, depth - 1, transported);
        }
        if let Some(call) = file.ast.calls.iter().find(|c| c.span == span) {
            if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                *transported = true;
                let (provider_file, value) = self.provider(file, call)?;
                let mut value = self.value(provider_file, value, depth - 1, transported)?;
                // ADR 0282: every consumer of the context shares the provider's
                // one array, and any of them can write a slot. (An array
                // pattern of a context value already blocks that context in
                // the receiver census; this keeps the refusal local.)
                if matches!(value, Value::Tuple { .. }) {
                    return None;
                }
                if let Value::Object { context, .. } = &mut value {
                    *context = true;
                }
                return Some(value);
            }
            let Value::Callable(Callable::Function { path, span }) =
                self.value(file, call.callee, depth - 1, transported)?
            else {
                return None;
            };
            let target_file = self.lookup.file_by_path(&path)?;
            let function = target_file.ast.functions.iter().find(|f| f.span == span)?;
            if function.r#async
                || function.generator
                || !self.lookup.function_value_is_current(target_file, function)
            {
                return None;
            }
            let returned = single_return(target_file, function)?;
            *transported = true;
            // ADR 0282: a returned array literal whose slots are not all closed
            // objects (ADR 0273's map/forEach receivers keep their own path) is
            // a tuple. It is described here, at the call, and nowhere else, so
            // only a literal this one return builds is ever a tuple: `const t =
            // [..]; return t` is not one, nor is a literal the factory merely
            // holds.
            let literal = target_file.ast.peel_ts_sugar_span(returned);
            if target_file.ast.array_literals.contains(&literal)
                && returned_array_elements(target_file, literal).is_none()
            {
                return Some(Value::Tuple {
                    path: target_file.path.to_string(),
                    factory: function.span,
                    elements: returned_tuple_elements(target_file, literal)?,
                });
            }
            let value = self.value(target_file, returned, depth - 1, transported)?;
            // ADR 0282: `return make()` forwards another factory's tuple. It is
            // still fresh, but v1 admits only the literal's own factory.
            if matches!(value, Value::Tuple { .. }) {
                return None;
            }
            if matches!(value, Value::Array { .. })
                && !target_file
                    .ast
                    .array_literals
                    .contains(&target_file.ast.peel_ts_sugar_span(returned))
            {
                return None;
            }
            if let Value::Object {
                path,
                span,
                context: false,
            } = &value
                && (path != target_file.path.as_str()
                    || *span != target_file.ast.peel_ts_sugar_span(returned))
            {
                return None;
            }
            return Some(value);
        }
        if let Some(properties) = closed_properties(file, span) {
            // Admitting a method must not let another field leak the holder
            // through its receiver. Close every callable channel, including
            // function-valued aliases; unknown values withhold transport.
            if properties.iter().any(|property| property.method)
                && (!module_scoped_source(file)
                    || self.receiver_channels_opaque
                    || !properties.iter().all(|property| {
                        if property.value_kind == ast::RuntimeValueKind::Primitive {
                            return true;
                        }
                        let value = file.ast.peel_ts_sugar_span(property.value);
                        if let Some(function) = file
                            .ast
                            .functions
                            .iter()
                            .find(|function| function.span == value)
                        {
                            // Receiver closure is independent of when an async
                            // body runs. Its invocation remains outside ordinary
                            // callable transport; an unused sibling need not veto
                            // this object's other exact methods.
                            return function.kind == ast::FunctionKind::Arrow
                                || crate::object_members::receiver_inert(file, function);
                        }
                        match self.value(
                            file,
                            property.shorthand_binding.unwrap_or(property.value),
                            depth - 1,
                            &mut false,
                        ) {
                            Some(Value::Callable(Callable::Function { path, span })) => self
                                .lookup
                                .file_by_path(&path)
                                .and_then(|owner| {
                                    owner
                                        .ast
                                        .functions
                                        .iter()
                                        .find(|function| function.span == span)
                                        .map(|function| (owner, function))
                                })
                                .is_some_and(|(owner, function)| {
                                    module_scoped_source(owner)
                                        && (function.kind == ast::FunctionKind::Arrow
                                            || crate::object_members::receiver_inert(
                                                owner, function,
                                            ))
                                }),
                            Some(Value::Callable(Callable::Accessor(symbol))) => {
                                self.accessors.contains_key(&symbol)
                            }
                            Some(
                                Value::Object { .. }
                                | Value::Array { .. }
                                | Value::Tuple { .. }
                                | Value::VetoedTuple,
                            ) => true,
                            _ => false,
                        }
                    }))
            {
                return None;
            }
            return Some(Value::Object {
                path: file.path.to_string(),
                span,
                context: false,
            });
        }
        if file.ast.array_literals.contains(&span) {
            // ReturnStructure owns the exact slots, including holes/spreads.
            // Do not infer them from contained object spans.
            let elements = returned_array_elements(file, span)?;
            return Some(Value::Array {
                path: file.path.to_string(),
                elements,
            });
        }
        // A demanded entity at a composite span carries its leading token's
        // symbol, not the expression's value: `ctx.a() === "x"` names `ctx`.
        // Only an exact identifier denotes its binding.
        if !exact_identifier(file, span) {
            return None;
        }
        let (bound_file, binding, symbol) = self
            .lookup
            .binding_at_reference(file.path.as_str(), span)
            .or_else(|| {
                // Shorthand values use the binder's declaration, where no
                // reference fact need have been demanded.
                let declaration = file.ast.reference_declaration(span).unwrap_or(span);
                let binding = file
                    .ast
                    .bindings
                    .iter()
                    .find(|b| b.names.iter().any(|n| n.span == declaration))?;
                let symbol = self
                    .lookup
                    .entities()
                    .at(file.path.as_str(), declaration)?
                    .clone();
                Some((file, binding, symbol))
            })?;
        if !binding.immutable
            || crate::value_identity::binding_has_write(bound_file, self.lookup.entities(), &symbol)
            || binding
                .names
                .iter()
                .any(|name| crate::indexes::binding_written(bound_file, name.span))
        {
            return None;
        }
        if binding.shape == ast::BindingShape::Object {
            if !simple_object_binding(binding)
                || self
                    .uses
                    .get(&location(bound_file.path.shared(), binding.pattern))
                    .is_some_and(|uses| uses.destructured_escaped)
            {
                return None;
            }
            let slot = binding
                .object_slots
                .iter()
                .find(|slot| slot.local.span == declaration)?;
            let Value::Object {
                path, span: object, ..
            } = self.value(bound_file, binding.initializer?, depth - 1, transported)?
            else {
                return None;
            };
            if !*transported {
                return None;
            }
            return self.member_value(
                self.lookup.file_by_path(&path)?,
                object,
                slot.property.as_str(),
                depth - 1,
                transported,
            );
        }
        if self.source_kinds.get(&symbol) != Some(&crate::ReactiveSourceKind::Store)
            && (source_has_runtime_witness(
                &symbol,
                self.accessors,
                self.lookup.entities(),
                self.names,
                self.lookup,
            ) || (binding.shape == ast::BindingShape::Identifier
                && binding
                    .call_initializer
                    .is_some_and(|span| self.created_accessor(bound_file, span, 8))))
        {
            if !self.accessors.contains_key(&symbol) {
                *transported = true;
            }
            return Some(Value::Callable(Callable::Accessor(symbol)));
        }
        if binding.shape == ast::BindingShape::Array {
            return self.tuple_slot(file, bound_file, binding, declaration, depth, transported);
        }
        if binding.shape != ast::BindingShape::Identifier || binding.names.len() != 1 {
            return None;
        }
        let value = self.value(bound_file, binding.initializer?, depth - 1, transported)?;
        // ADR 0282: `const t = make()` holds the tuple. `t[1] = other` or a
        // call that receives `t` can replace a slot, so a held tuple is no
        // tuple; `t[1]()` keeps its computed-callee obligation.
        if matches!(value, Value::Tuple { .. }) {
            return None;
        }
        if matches!(value, Value::Array { .. }) {
            let key = location(bound_file.path.shared(), binding.pattern);
            return self
                .uses
                .get(&key)
                .is_none_or(|uses| !uses.array_escaped)
                .then_some(value);
        }
        if !self.binding_is_closed(bound_file, binding, matches!(value, Value::Object { .. })) {
            return None;
        }
        Some(value)
    }

    fn member_value(
        &self,
        file: &FileFacts,
        object: Span,
        name: &str,
        depth: usize,
        transported: &mut bool,
    ) -> Option<Value> {
        if let Stability::Vetoed(_site) =
            self.member_stability
                .state_of(self.facts, file.path.as_str(), name)
            && !self.dependency_member_stable(file.path.as_str(), object, name)
        {
            return None;
        }
        let property = closed_properties(file, object)?
            .into_iter()
            .find(|property| {
                file.source_text(property.key)
                    .map(|s| s.trim_matches(['\'', '"']))
                    == Some(name)
            })?;
        let value = self.value(
            file,
            property.shorthand_binding.unwrap_or(property.value),
            depth,
            transported,
        )?;
        if let Value::Callable(Callable::Function { path, span }) = &value {
            let selected = self
                .lookup
                .file_by_path(path)?
                .ast
                .functions
                .iter()
                .find(|f| f.span == *span)?;
            if selected.kind != ast::FunctionKind::Arrow
                && (!module_scoped_source(self.lookup.file_by_path(path)?)
                    || self.receiver_channels_opaque
                    || !crate::object_members::receiver_inert(
                        self.lookup.file_by_path(path)?,
                        selected,
                    ))
            {
                return None;
            }
        }
        matches!(value, Value::Callable(_)).then_some(value)
    }

    /// ADR 0282: `const [running, start] = make(); start()` calls the closure
    /// in slot 1 of the array literal `make`'s one unconditional return
    /// builds. The array is fresh and this pattern consumes it in the same
    /// statement, so no other code holds it and no write can replace a slot
    /// between the return and the destructuring. The caller has already
    /// refused a `let`, a written name and a declaration this lookup cannot
    /// pair with one binding.
    fn tuple_slot(
        &self,
        file: &FileFacts,
        bound_file: &FileFacts,
        binding: &ast::BindingFact,
        declaration: Span,
        depth: usize,
        transported: &mut bool,
    ) -> Option<Value> {
        // `declaration` is a span of `file`; a binding another file holds
        // cannot be paired with it by position.
        if bound_file.path != file.path || !simple_array_binding(binding) {
            return None;
        }
        let index = binding
            .array_slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.span == declaration))?;
        // The initializer is the call itself, so the array is the call's
        // fresh result: `const [, start] = t` destructures a held tuple.
        let initializer = bound_file.ast.peel_ts_sugar_span(binding.initializer?);
        bound_file.ast.call_at(initializer)?;
        let Value::Tuple {
            path,
            factory,
            elements,
        } = self.value(bound_file, initializer, depth - 1, transported)?
        else {
            return None;
        };
        if !*transported {
            return None;
        }
        // A slot past the literal's end destructures `undefined`.
        let element = *elements.get(index)?;
        let span = slot_closure(
            self.lookup.file_by_path(&path)?,
            factory,
            element,
            self.lookup,
        )?;
        // Retain the transported-call obligation after establishing this is
        // a tuple closure. A mutation withdraws its identity, never certifies
        // silence or substitutes another violating implementation.
        if self.member_stability.tuple_iteration {
            return Some(Value::VetoedTuple);
        }
        Some(Value::Callable(Callable::Function { path, span }))
    }

    /// One exact default-less dialect context, one syntactic provider, no
    /// hidden/spread value, and no escape of the context itself. Several
    /// providers are ambiguous even if their values happen to look alike.
    fn provider<'a>(
        &'a self,
        file: &FileFacts,
        call: &ast::CallFact,
    ) -> Option<(&'a FileFacts, Span)> {
        // useContext is a channel any code beneath the provider can use. In an
        // open program, unseen importers can reach the value through an
        // exported context, wrapper or returned closure and replace a member
        // where ADR 0273's name index cannot see the write, so no context's
        // provider census is closed.
        if !self.lookup.program_closed
            || self.unkeyed_provider
            || call.arguments.len() != 1
            || call.arguments[0].spread
        {
            return None;
        }
        let argument = file.ast.peel_ts_sugar_span(call.arguments[0].span);
        if !exact_identifier(file, argument) {
            return None;
        }
        let (context_file, binding, _) = self
            .lookup
            .binding_at_reference(file.path.as_str(), argument)?;
        // ADR 0300 excludes unseen providers of selected-package contexts.
        // Visible providers, escapes and writes still participate below.
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
            || crate::indexes::binding_written(context_file, binding.names[0].span)
        {
            return None;
        }
        let created = context_file.ast.call_at(binding.call_initializer?)?;
        if self.role(context_file, created) != Some(ContextRole::Create)
            || !created.arguments.is_empty()
        {
            return None;
        }
        let key = location(context_file.path.shared(), binding.pattern);
        if self.blocked_contexts.contains(&key)
            || self.uses.get(&key).is_some_and(|uses| uses.context_escaped)
        {
            return None;
        }
        let providers = self.providers.get(&key)?;
        if providers.len() != 1 {
            return None;
        }
        let (path, value) = providers[0].as_ref()?;
        let provider_file = self.lookup.file_by_path(path)?;
        if let Some((bound_file, binding, _)) = self.lookup.binding_at_reference(
            provider_file.path.as_str(),
            provider_file.ast.peel_ts_sugar_span(*value),
        ) {
            let key = location(bound_file.path.shared(), binding.pattern);
            if self
                .uses
                .get(&key)
                .is_some_and(|uses| uses.context_value_escaped)
            {
                return None;
            }
        }
        Some((provider_file, *value))
    }

    /// Derivation only: this deliberately ignores receiver closure/provider
    /// acceptance so a bad receiver cannot disappear from the shared census.
    fn context_origin(&self, file: &FileFacts, span: Span, depth: usize) -> Option<Location> {
        if depth == 0 {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        let declaration = file.ast.reference_declaration(span).unwrap_or(span);
        if let Some((path, argument)) = self
            .parameter_inputs
            .get(&location(file.path.shared(), declaration))
        {
            return self.context_origin(self.lookup.file_by_path(path)?, *argument, depth - 1);
        }
        if let Some(call) = file.ast.call_at(span) {
            if call.construct {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                let argument = call
                    .arguments
                    .first()
                    .map(|a| file.ast.peel_ts_sugar_span(a.span))?;
                return (call.arguments.len() == 1
                    && !call.arguments[0].spread
                    && exact_identifier(file, argument))
                .then(|| self.binding_key(file, argument))
                .flatten();
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let target = self
                .lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| self.lookup.namespace_member_function(file, callee));
            let (target_file, function) = target?;
            if function.r#async
                || function.generator
                || !self.lookup.function_value_is_current(target_file, function)
            {
                return None;
            }
            return self.context_origin(
                target_file,
                single_return(target_file, function)?,
                depth - 1,
            );
        }
        if !exact_identifier(file, span) {
            return None;
        }
        let (bound_file, binding, _) =
            self.lookup.binding_at_reference(file.path.as_str(), span)?;
        self.context_origin(bound_file, binding.initializer?, depth - 1)
    }

    /// ADR 0300: every visible value must carry the same reactive member
    /// claim. Unlike `dependency_getters`, this walk refuses open literals,
    /// unknown providers, mutable bindings and visible member/descriptor
    /// writes. The package integrity premise replaces the receiver escape
    /// census only for objects created by a selected package.
    pub fn proven_dependency_getters(
        &self,
        file: &FileFacts,
        span: Span,
        name: &str,
    ) -> Option<Vec<Location>> {
        let objects = self.closed_dependency_objects(file, span, 32)?;
        let mut getters = Vec::new();
        for (path, object) in objects {
            self.facts.dependency_source_root(&path)?;
            if !self.dependency_member_stable(&path, object, name) {
                return None;
            }
            let file = self.lookup.file_by_path(&path)?;
            let shape = file
                .ast
                .object_get_shapes
                .iter()
                .find(|s| s.span == object && s.closed)?;
            let property = shape.properties.iter().find(|p| p.key.as_str() == name)?;
            let getter = property.getter?;
            // The normalized entry call executes before any branch or user
            // operation, not merely somewhere in the getter's body.
            let call = file.ast.call_at(property.entry_call?)?;
            let source = self.source_call(file, call.callee)?;
            if !self.has_runtime_source_origin(&source) {
                return None;
            }
            let site = location(file.path.shared(), getter);
            if !getters.contains(&site) {
                getters.push(site);
            }
        }
        (!getters.is_empty()).then_some(getters)
    }

    fn closed_dependency_objects(
        &self,
        file: &FileFacts,
        span: Span,
        depth: usize,
    ) -> Option<Vec<(String, Span)>> {
        if depth == 0 {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        if let Some(choice) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|c| c.span == span)
        {
            // Both runtime alternatives must close. A missing alternative is
            // not permission to keep only the known package object.
            let mut values = self.closed_dependency_objects(file, choice.consequent, depth - 1)?;
            for value in self.closed_dependency_objects(file, choice.alternate, depth - 1)? {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            return (!values.is_empty()).then_some(values);
        }
        if let Some(shape) = file.ast.object_get_shapes.iter().find(|s| s.span == span) {
            return shape.closed.then(|| vec![(file.path.to_string(), span)]);
        }
        if let Some(member) = file.ast.members.iter().find(|m| m.span == span) {
            if file.ast.computed_members.contains(&span) {
                return None;
            }
            let name = file.source_text(member.property)?;
            let mut values = Vec::new();
            for (path, object) in self.closed_dependency_objects(file, member.object, depth - 1)? {
                if !self.dependency_member_stable(&path, object, name) {
                    return None;
                }
                let object_file = self.lookup.file_by_path(&path)?;
                let property = direct_data_properties(object_file, object)
                    .into_iter()
                    .find(|p| {
                        object_file
                            .source_text(p.key)
                            .map(|s| s.trim_matches(['\'', '"']))
                            == Some(name)
                    })?;
                values.extend(self.closed_dependency_objects(
                    object_file,
                    property.shorthand_binding.unwrap_or(property.value),
                    depth - 1,
                )?);
            }
            return (!values.is_empty()).then_some(values);
        }
        if let Some(call) = file.ast.call_at(span) {
            // This proves the value if this expression is evaluated, not
            // that the containing function always invokes it. A preceding
            // guard/throw cannot supply a different successful return value.
            if call.construct {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                if !self.lookup.program_closed
                    || self.unkeyed_provider
                    || call.arguments.len() != 1
                    || call.arguments[0].spread
                {
                    return None;
                }
                let argument = file.ast.peel_ts_sugar_span(call.arguments[0].span);
                if !exact_identifier(file, argument) {
                    return None;
                }
                let (context_file, binding, _) = self
                    .lookup
                    .binding_at_reference(file.path.as_str(), argument)?;
                self.facts
                    .dependency_source_root(context_file.path.as_str())?;
                if !binding.immutable
                    || binding.shape != ast::BindingShape::Identifier
                    || binding.names.len() != 1
                    || crate::indexes::binding_written(context_file, binding.names[0].span)
                {
                    return None;
                }
                let created = context_file.ast.call_at(binding.call_initializer?)?;
                if self.role(context_file, created) != Some(ContextRole::Create)
                    || !created.arguments.is_empty()
                {
                    return None;
                }
                let key = location(context_file.path.shared(), binding.pattern);
                if self.uses.get(&key).is_some_and(|uses| uses.context_escaped) {
                    return None;
                }
                let providers = self.providers.get(&key)?;
                let mut values = Vec::new();
                for provider in providers {
                    let (path, value) = provider.as_ref()?;
                    values.extend(self.closed_dependency_objects(
                        self.lookup.file_by_path(path)?,
                        *value,
                        depth - 1,
                    )?);
                }
                return (!values.is_empty()).then_some(values);
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let (target_file, function) = self
                .lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| self.lookup.namespace_member_function(file, callee))?;
            if function.r#async
                || function.generator
                || !self.lookup.function_value_is_current(target_file, function)
            {
                return None;
            }
            let returned = single_return(target_file, function)?;
            let returned = target_file.ast.peel_ts_sugar_span(returned);
            if let Some(position) =
                target_file
                    .ast
                    .reference_declaration(returned)
                    .and_then(|declaration| {
                        function.parameters.iter().position(|p| {
                            p.pattern == declaration && p.shape == ast::BindingShape::Identifier
                        })
                    })
            {
                let parameter = &function.parameters[position];
                if parameter.initializer.is_some()
                    || function.rest_parameter
                    || crate::indexes::binding_written(target_file, parameter.pattern)
                    || target_file.ast.identifiers.iter().any(|id| {
                        function.body.contains(id.span)
                            && id.role == ast::IdentifierRole::Reference
                            && matches!(
                                target_file.source_text(id.span),
                                Some("arguments" | "eval")
                            )
                    })
                {
                    return None;
                }
                let argument = call.arguments.get(position).filter(|a| !a.spread)?;
                if call.arguments.iter().take(position).any(|a| a.spread) {
                    return None;
                }
                return self.closed_dependency_objects(file, argument.span, depth - 1);
            }
            return self.closed_dependency_objects(target_file, returned, depth - 1);
        }
        if !exact_identifier(file, span) {
            return None;
        }
        let (bound_file, binding, symbol) =
            self.lookup.binding_at_reference(file.path.as_str(), span)?;
        if !binding.immutable
            || binding.names.len() != 1
            || crate::indexes::binding_written(bound_file, binding.names[0].span)
            || crate::value_identity::binding_has_write(bound_file, self.lookup.entities(), &symbol)
        {
            return None;
        }
        if binding.shape == ast::BindingShape::Object {
            let [slot] = binding.object_slots.as_slice() else {
                return None;
            };
            if !slot.plain_identifier_key || slot.local.span != binding.names[0].span {
                return None;
            }
            let mut values = Vec::new();
            for (path, object) in
                self.closed_dependency_objects(bound_file, binding.initializer?, depth - 1)?
            {
                // A package-owned data descriptor is stable under the same
                // integrity premise as an ordinary member projection. No
                // project receiver escape census is waived here.
                self.facts.dependency_source_root(&path)?;
                if !self.dependency_member_stable(&path, object, slot.property.as_str()) {
                    return None;
                }
                let owner = self.lookup.file_by_path(&path)?;
                let property = direct_data_properties(owner, object)
                    .into_iter()
                    .find(|p| {
                        owner
                            .source_text(p.key)
                            .map(|key| key.trim_matches(['\'', '"']))
                            == Some(slot.property.as_str())
                    })?;
                values.extend(self.closed_dependency_objects(
                    owner,
                    property.shorthand_binding.unwrap_or(property.value),
                    depth - 1,
                )?);
            }
            return (!values.is_empty()).then_some(values);
        }
        if binding.shape != ast::BindingShape::Identifier {
            return None;
        }
        let values = self.closed_dependency_objects(bound_file, binding.initializer?, depth - 1)?;
        // Project-owned values retain ADR 0273's ordinary receiver census.
        if values
            .iter()
            .any(|(path, _)| self.facts.dependency_source_root(path).is_none())
            && !self.binding_is_closed(bound_file, binding, true)
        {
            return None;
        }
        Some(values)
    }

    fn dependency_member_stable(&self, path: &str, object: Span, name: &str) -> bool {
        // Named writes anywhere retain their veto. Every visible unknown
        // target needs an exact disjoint identity before the integrity proof.
        if self.member_stability.written.contains_key(name) {
            return false;
        }
        let object = location(path, object);
        // Visible syntax cannot disappear when receiver derivation is
        // incomplete. In particular a package's unknown receiver can be an
        // object from another selected package, or one handed it by the app.
        if !self
            .member_stability
            .unknown_receivers
            .iter()
            .chain(
                self.member_stability
                    .dependencies
                    .values()
                    .flat_map(|p| &p.unknown_receivers),
            )
            .all(|receiver| {
                let Some(file) = self.lookup.file_by_path(receiver.path.as_ref()) else {
                    return false;
                };
                let (Ok(start), Ok(end)) = (
                    u32::try_from(receiver.start_byte),
                    u32::try_from(receiver.end_byte),
                ) else {
                    return false;
                };
                mutation_target_is_distinct(
                    file,
                    Span::new(start, end),
                    &object,
                    self.lookup,
                    32,
                    &mut HashSet::new(),
                )
            })
        {
            return false;
        }
        self.member_stability
            .wildcard_sites
            .iter()
            .chain(
                self.member_stability
                    .dependencies
                    .values()
                    .flat_map(|package| &package.wildcard_sites),
            )
            .all(|site| {
                let Some(file) = self.lookup.file_by_path(site.path.as_ref()) else {
                    return false;
                };
                let Some(call) = file.ast.calls.iter().find(|call| {
                    u64::from(call.span.start) == site.start_byte
                        && u64::from(call.span.end) == site.end_byte
                }) else {
                    return false;
                };
                // A target spread can supply the getter object itself.
                let Some(target) = call.arguments.first().filter(|a| !a.spread) else {
                    return false;
                };
                mutation_target_is_distinct(
                    file,
                    target.span,
                    &object,
                    self.lookup,
                    32,
                    &mut HashSet::new(),
                )
            })
    }

    /// ADR 0289: derivation only, like [`Self::context_origin`]. The getters
    /// a Get of `name` on the value at `span` may run, when a package analyzed
    /// as source (ADR 0284) defines them and they read an exact accessor.
    ///
    /// The walk follows exact bindings, a resolved function's every return
    /// (a returned parameter continues at this call's argument), the data
    /// members of object literals and every provider of an exact context. It
    /// ignores every closure, census and member-stability requirement, so it
    /// is never a proof: its one consumer is an uncertifiable obligation.
    pub fn dependency_getters(&self, file: &FileFacts, span: Span, name: &str) -> Vec<Location> {
        let mut objects = Vec::new();
        let mut visited = HashSet::new();
        self.derived_objects(file, span, 24, &mut visited, &mut objects);
        let mut getters = Vec::new();
        for (path, object) in objects {
            if self.facts.dependency_source_root(&path).is_none() {
                continue;
            }
            let Some(object_file) = self.lookup.file_by_path(&path) else {
                continue;
            };
            let Some(shape) = object_file
                .ast
                .object_get_shapes
                .iter()
                .find(|shape| shape.span == object)
            else {
                continue;
            };
            for getter in shape
                .properties
                .iter()
                .filter(|property| property.key.as_str() == name)
                .filter_map(|property| property.getter)
            {
                let site = location(object_file.path.shared(), getter);
                if !getters.contains(&site) && self.getter_reads_accessor(object_file, getter) {
                    getters.push(site);
                }
            }
        }
        getters
    }

    /// Whether the getter function at `getter` calls an exact accessor
    /// directly in its body: the read a Get of it may perform.
    fn getter_reads_accessor(&self, file: &FileFacts, getter: Span) -> bool {
        let Some(function) = file.ast.functions.iter().find(|f| f.span == getter) else {
            return false;
        };
        file.ast.calls.iter().any(|call| {
            function.body.contains(call.span)
                && crate::owners::written_directly_in(&file.ast, function, call.span)
                && self.source_call(file, call.callee).is_some()
        })
    }

    /// The object literals the value at `span` may be, for
    /// [`Self::dependency_getters`]. `visited` breaks recursion through
    /// mutually recursive bindings and functions.
    fn derived_objects(
        &self,
        file: &FileFacts,
        span: Span,
        depth: usize,
        visited: &mut HashSet<Location>,
        objects: &mut Vec<(String, Span)>,
    ) {
        let span = file.ast.peel_ts_sugar_span(span);
        if depth == 0 || !visited.insert(location(file.path.shared(), span)) {
            return;
        }
        if let Some(choice) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|c| c.span == span)
        {
            // May-provenance retains each known alternative even when its
            // sibling cannot supply the complete proof above.
            self.derived_objects(file, choice.consequent, depth - 1, visited, objects);
            self.derived_objects(file, choice.alternate, depth - 1, visited, objects);
            return;
        }
        let declaration = file.ast.reference_declaration(span).unwrap_or(span);
        if let Some((path, argument)) = self
            .parameter_inputs
            .get(&location(file.path.shared(), declaration))
        {
            if let Some(argument_file) = self.lookup.file_by_path(path) {
                self.derived_objects(argument_file, *argument, depth - 1, visited, objects);
            }
            return;
        }
        if file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == span)
        {
            objects.push((file.path.to_string(), span));
            return;
        }
        if let Some(member) = file.ast.members.iter().find(|m| m.span == span) {
            if file.ast.computed_members.contains(&span) {
                return;
            }
            let Some(name) = file.source_text(member.property) else {
                return;
            };
            let mut receivers = Vec::new();
            self.derived_objects(file, member.object, depth - 1, visited, &mut receivers);
            for (path, object) in receivers {
                let Some(object_file) = self.lookup.file_by_path(&path) else {
                    continue;
                };
                for property in direct_data_properties(object_file, object) {
                    if object_file
                        .source_text(property.key)
                        .map(|key| key.trim_matches(['\'', '"']))
                        == Some(name)
                    {
                        self.derived_objects(
                            object_file,
                            property.shorthand_binding.unwrap_or(property.value),
                            depth - 1,
                            visited,
                            objects,
                        );
                    }
                }
            }
            return;
        }
        if let Some(call) = file.ast.call_at(span) {
            if call.construct {
                return;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                let Some(argument) = call
                    .arguments
                    .first()
                    .filter(|argument| call.arguments.len() == 1 && !argument.spread)
                    .map(|argument| file.ast.peel_ts_sugar_span(argument.span))
                    .filter(|argument| exact_identifier(file, *argument))
                else {
                    return;
                };
                let Some(key) = self.binding_key(file, argument) else {
                    return;
                };
                for (path, value) in self.providers.get(&key).into_iter().flatten().flatten() {
                    if let Some(provider_file) = self.lookup.file_by_path(path) {
                        self.derived_objects(provider_file, *value, depth - 1, visited, objects);
                    }
                }
                return;
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let Some((target_file, function)) = self
                .lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| self.lookup.namespace_member_function(file, callee))
            else {
                return;
            };
            if function.r#async || function.generator {
                return;
            }
            let returns = function
                .expression_return
                .iter()
                .chain(target_file.ast.returns.iter().filter(|r| {
                    containing_ast_function(&target_file.ast, r.span)
                        .is_some_and(|f| f.span == function.span)
                }))
                .filter_map(|r| r.argument)
                .collect::<Vec<_>>();
            for returned in returns {
                let returned = target_file.ast.peel_ts_sugar_span(returned);
                // A returned parameter is this call's argument.
                let position =
                    target_file
                        .ast
                        .reference_declaration(returned)
                        .and_then(|declaration| {
                            function.parameters.iter().position(|parameter| {
                                parameter.pattern == declaration
                                    && parameter.shape == ast::BindingShape::Identifier
                            })
                        });
                match position {
                    Some(position) => {
                        if let Some(argument) = call
                            .arguments
                            .get(position)
                            .filter(|argument| !argument.spread)
                        {
                            self.derived_objects(file, argument.span, depth - 1, visited, objects);
                        }
                    }
                    None => {
                        self.derived_objects(target_file, returned, depth - 1, visited, objects);
                    }
                }
            }
            return;
        }
        if !exact_identifier(file, span) {
            return;
        }
        let Some((bound_file, binding, _)) =
            self.lookup.binding_at_reference(file.path.as_str(), span)
        else {
            return;
        };
        if binding.shape == ast::BindingShape::Object
            && binding.names.len() == 1
            && let [slot] = binding.object_slots.as_slice()
            && slot.plain_identifier_key
            && slot.local.span == binding.names[0].span
            && let Some(initializer) = binding.initializer
        {
            let mut receivers = Vec::new();
            self.derived_objects(bound_file, initializer, depth - 1, visited, &mut receivers);
            for (path, object) in receivers {
                let Some(owner) = self.lookup.file_by_path(&path) else {
                    continue;
                };
                for property in direct_data_properties(owner, object) {
                    if owner
                        .source_text(property.key)
                        .map(|key| key.trim_matches(['\'', '"']))
                        == Some(slot.property.as_str())
                    {
                        self.derived_objects(
                            owner,
                            property.shorthand_binding.unwrap_or(property.value),
                            depth - 1,
                            visited,
                            objects,
                        );
                    }
                }
            }
            return;
        }
        if binding.shape == ast::BindingShape::Identifier
            && binding.names.len() == 1
            && let Some(initializer) = binding.initializer
        {
            self.derived_objects(bound_file, initializer, depth - 1, visited, objects);
        }
    }

    fn created_accessor(&self, file: &FileFacts, span: Span, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(call) = file.ast.call_at(file.ast.peel_ts_sugar_span(span)) else {
            return false;
        };
        if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
            return false;
        }
        if let Some(primitive) = crate::call_primitive_name(
            file,
            call,
            self.lookup.entities(),
            self.names,
            self.lookup.dialect,
        )
        .and_then(|p| p.primitive())
        {
            return self.lookup.dialect.creates_reactive_source(primitive)
                && !self.lookup.dialect.returns_reactive_tuple(primitive)
                && !self.lookup.dialect.returns_store(primitive);
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let target = if file.ast.identifiers.iter().any(|id| id.span == callee) {
            self.lookup.function_called_at(file.path.as_str(), callee)
        } else {
            self.lookup.namespace_member_function(file, callee)
        };
        let Some((target_file, function)) = target else {
            return false;
        };
        !function.r#async
            && !function.generator
            && self.lookup.function_value_is_current(target_file, function)
            && single_return(target_file, function)
                .is_some_and(|returned| self.created_accessor(target_file, returned, depth - 1))
    }

    fn binding_is_closed(
        &self,
        file: &FileFacts,
        binding: &ast::BindingFact,
        object: bool,
    ) -> bool {
        let key = location(file.path.shared(), binding.pattern);
        self.uses.get(&key).is_none_or(|uses| {
            if object {
                !uses.object_escaped
            } else {
                !uses.callable_escaped
            }
        })
    }
}

/// Whether `span` is exactly one identifier, as opposed to an expression an
/// identifier merely begins.
/// ADR 0300: a wildcard target whose complete local identity comes from
/// other fresh allocation sites cannot replace this literal's descriptor.
/// Unknown inputs never count as distinct. Cyclic local assignments are
/// checked as a graph: every non-cyclic input must be a distinct allocation
/// (or an uninitialized local's undefined); no source edge is skipped.
fn mutation_target_is_distinct(
    file: &FileFacts,
    span: Span,
    object: &Location,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    visited: &mut HashSet<(String, Span)>,
) -> bool {
    mutation_target_projected(file, span, &[], object, lookup, depth, visited)
}

/// A complete identity join. Projections are copied by destructuring, or
/// read through a holder whose exact uses cannot replace the projected slot.
#[allow(clippy::too_many_arguments)]
fn mutation_target_projected(
    file: &FileFacts,
    span: Span,
    projection: &[String],
    object: &Location,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    visited: &mut HashSet<(String, Span)>,
) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    // Evaluating a function expression allocates a new callable object. A
    // write to its own fields cannot rewrite the package's getter literal.
    if projection.is_empty()
        && file.ast.functions.iter().any(|function| {
            function.span == span
                && matches!(
                    function.kind,
                    ast::FunctionKind::Expression | ast::FunctionKind::Arrow
                )
        })
    {
        return location(file.path.shared(), span) != *object;
    }
    if file
        .ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == span)
    {
        if projection.is_empty() {
            return location(file.path.shared(), span) != *object;
        }
        return file
            .ast
            .object_get_shapes
            .iter()
            .any(|s| s.span == span && s.closed)
            && Some(direct_data_properties(file, span)).is_some_and(|properties| {
                properties
                    .iter()
                    .find(|p| {
                        file.source_text(p.key).map(|k| k.trim_matches(['\'', '"']))
                            == Some(projection[0].as_str())
                    })
                    .is_some_and(|p| {
                        mutation_target_projected(
                            file,
                            p.shorthand_binding.unwrap_or(p.value),
                            &projection[1..],
                            object,
                            lookup,
                            depth - 1,
                            visited,
                        )
                    })
            });
    }
    if file.ast.array_literals.contains(&span) {
        if projection.is_empty() {
            return location(file.path.shared(), span) != *object;
        }
        return projection[0]
            .parse::<usize>()
            .ok()
            .filter(|index| index.to_string() == projection[0])
            .and_then(|index| {
                ast::literal_array_element(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    span,
                    index,
                )
            })
            .is_some_and(|element| {
                mutation_target_projected(
                    file,
                    element,
                    &projection[1..],
                    object,
                    lookup,
                    depth - 1,
                    visited,
                )
            });
    }
    if let Some(logical) = file.ast.logical_expressions.iter().find(|e| e.span == span) {
        return mutation_target_projected(
            file,
            logical.left,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        ) && mutation_target_projected(
            file,
            logical.right,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if let Some(choice) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|e| e.span == span)
    {
        return mutation_target_projected(
            file,
            choice.consequent,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        ) && mutation_target_projected(
            file,
            choice.alternate,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if let Some(member) = file.ast.members.iter().find(|m| m.span == span) {
        let key = if file.ast.computed_members.contains(&span) {
            file.ast
                .literal_computed_members
                .iter()
                .find(|m| m.span == span)
                .map(|m| m.key.to_string())
        } else {
            file.source_text(member.property).map(str::to_owned)
        };
        let Some(key) = key else {
            return false;
        };
        let mut keys = vec![key];
        keys.extend_from_slice(projection);
        return mutation_target_projected(
            file,
            member.object,
            &keys,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if file.source_text(span) == Some("this") {
        let Some(function) = file
            .ast
            .functions_body_containing(span)
            .filter(|f| f.kind != ast::FunctionKind::Arrow)
            .min_by_key(|f| f.span.end - f.span.start)
        else {
            return false;
        };
        return mutation_parameter_inputs(
            file,
            function,
            None,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if let Some(call) = file.ast.call_at(span) {
        // Object.assign returns the exact target. No other call is guessed.
        return mutation_builtin(file, call, lookup) == Some("assign")
            && call.arguments.first().is_some_and(|target| {
                !target.spread
                    && mutation_target_projected(
                        file,
                        target.span,
                        projection,
                        object,
                        lookup,
                        depth - 1,
                        visited,
                    )
            });
    }
    if !exact_identifier(file, span) {
        return false;
    }
    let declaration = file.ast.reference_declaration(span).unwrap_or(span);
    if let Some((function, position, binding)) = file.ast.functions.iter().find_map(|f| {
        f.parameters
            .iter()
            .enumerate()
            .find(|(_, p)| p.names.iter().any(|n| n.span == declaration))
            .map(|(i, p)| (f, i, p))
    }) {
        let Some(mut keys) = mutation_binding_projection(binding, declaration) else {
            return false;
        };
        keys.extend_from_slice(projection);
        return !crate::indexes::binding_written(file, declaration)
            && mutation_parameter_inputs(
                file,
                function,
                Some(position),
                &keys,
                object,
                lookup,
                depth - 1,
                visited,
            );
    }
    let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|b| b.names.iter().any(|n| n.span == declaration))
    else {
        return false;
    };
    let Some(mut keys) = mutation_binding_projection(binding, declaration) else {
        return false;
    };
    keys.extend_from_slice(projection);
    if file.ast.identifiers.iter().any(|id| {
        id.role == ast::IdentifierRole::Reference
            && matches!(file.source_text(id.span), Some("eval" | "arguments"))
    }) || file.ast.exports.iter().any(|export| {
        file.ast
            .exported_bindings(export)
            .any(|exported| exported.pattern == binding.pattern)
    }) {
        return false;
    }
    if !projection.is_empty()
        && !mutation_holder_closed(file, declaration, projection[0].as_str(), false)
    {
        return false;
    }
    let key = (file.path.to_string(), declaration);
    if !visited.insert(key.clone()) {
        return true;
    }
    let refers = |target: Span| {
        let target = file.ast.peel_ts_sugar_span(target);
        target == declaration || file.ast.reference_declaration(target) == Some(declaration)
    };
    let initial = binding.initializer.map_or(binding.runtime_let, |initial| {
        mutation_target_projected(file, initial, &keys, object, lookup, depth - 1, visited)
    });
    let writes = file
        .ast
        .write_targets
        .iter()
        .chain(&file.ast.iteration_targets)
        .chain(&file.ast.deleted_targets)
        .filter(|target| refers(**target))
        .all(|target| {
            file.ast
                .assignments
                .iter()
                .find(|a| a.target == *target && !a.reads_target)
                .is_some_and(|assignment| {
                    keys == projection
                        && mutation_target_projected(
                            file,
                            assignment.value_span,
                            projection,
                            object,
                            lookup,
                            depth - 1,
                            visited,
                        )
                })
        });
    let assignments = file
        .ast
        .assignments
        .iter()
        .filter(|a| refers(a.target))
        .all(|a| {
            !a.reads_target
                && keys == projection
                && mutation_target_projected(
                    file,
                    a.value_span,
                    projection,
                    object,
                    lookup,
                    depth - 1,
                    visited,
                )
        });
    visited.remove(&key);
    initial && writes && assignments
}

fn mutation_binding_projection(
    binding: &ast::BindingFact,
    declaration: Span,
) -> Option<Vec<String>> {
    match binding.shape {
        ast::BindingShape::Identifier
            if binding.names.len() == 1 && binding.pattern == declaration =>
        {
            Some(vec![])
        }
        ast::BindingShape::Object
            if binding.object_slots.len() == binding.names.len()
                && binding.object_slots.iter().all(|s| s.plain_identifier_key) =>
        {
            binding
                .object_slots
                .iter()
                .find(|s| s.local.span == declaration)
                .map(|s| vec![s.property.to_string()])
        }
        ast::BindingShape::Array
            if binding.array_slots.iter().flatten().count() == binding.names.len() =>
        {
            binding
                .array_slots
                .iter()
                .position(|s| s.as_ref().is_some_and(|s| s.span == declaration))
                .map(|i| vec![i.to_string()])
        }
        _ => None,
    }
}

/// A holder projection needs more than its initializer: all exact references
/// remain static member receivers, and no visible write can replace the slot.
fn mutation_holder_closed(
    file: &FileFacts,
    declaration: Span,
    key: &str,
    method_identity: bool,
) -> bool {
    let targets = file
        .ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .collect::<HashSet<_>>();
    let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|b| b.names.iter().any(|n| n.span == declaration))
    else {
        return false;
    };
    if let Some(literal) = binding.initializer.map(|s| file.ast.peel_ts_sugar_span(s))
        && file.ast.object_get_shapes.iter().any(|s| s.span == literal)
    {
        let Some(this_uses) =
            ast::this_expression_spans(std::path::Path::new(file.path.as_str()), &file.source)
        else {
            return false;
        };
        if !this_uses
            .into_iter()
            .filter(|s| literal.contains(*s))
            .all(|s| {
                let owner = file
                    .ast
                    .functions_body_containing(s)
                    .filter(|f| f.kind != ast::FunctionKind::Arrow)
                    .min_by_key(|f| f.span.end - f.span.start);
                // A nested ordinary function has a different this channel. Its
                // own mutation still faces the global exact-target gate.
                if owner.is_some_and(|f| {
                    !file
                        .ast
                        .object_properties
                        .iter()
                        .any(|p| literal.contains(p.span) && p.value == f.span)
                }) {
                    return true;
                }
                file.ast
                    .members
                    .iter()
                    .find(|m| file.ast.peel_ts_sugar_span(m.object) == s)
                    .is_some_and(|m| {
                        (method_identity && targets.contains(&m.span)
                            || !file.ast.computed_members.contains(&m.span))
                            && !(targets.contains(&m.span)
                                && file.source_text(m.property) == Some(key))
                            && !file.ast.calls.iter().any(|c| c.callee == m.span)
                            && (!method_identity
                                || file.source_text(m.property) != Some(key)
                                || targets.contains(&m.span))
                    })
            })
        {
            return false;
        }
    }
    file.ast
        .reference_declarations
        .iter()
        .filter(|(_, d)| *d == declaration)
        .all(|(reference, _)| {
            file.ast
                .members
                .iter()
                .find(|m| file.ast.peel_ts_sugar_span(m.object) == *reference)
                .is_some_and(|m| {
                    !file.ast.computed_members.contains(&m.span)
                        && !(targets.contains(&m.span) && file.source_text(m.property) == Some(key))
                })
        })
        && !file.ast.calls.iter().any(|c| {
            c.arguments.iter().any(|a| {
                file.ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(a.span))
                    == Some(declaration)
            })
        })
}

/// Join every invocation of a private, current source-local helper. An alias,
/// export, unknown call, spread or default is an unknown input, never an empty
/// set of receivers. call/apply require the actual library signature, and
/// methods require one unchanged literal holder with a closed reference census.
#[allow(clippy::too_many_arguments)]
fn mutation_parameter_inputs(
    file: &FileFacts,
    function: &ast::FunctionFact,
    position: Option<usize>,
    projection: &[String],
    object: &Location,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    visited: &mut HashSet<(String, Span)>,
) -> bool {
    if depth == 0
        || function.rest_parameter
        || !lookup.function_value_is_current(file, function)
        || function.parameters.iter().any(|p| p.initializer.is_some())
        || file.ast.identifiers.iter().any(|id| {
            id.role == ast::IdentifierRole::Reference
                && matches!(file.source_text(id.span), Some("eval" | "arguments"))
        })
    {
        return false;
    }
    let name = crate::owners::function_binding_name(file, function);
    let method = file
        .ast
        .object_properties
        .iter()
        .find(|p| !p.computed && !p.data && file.ast.peel_ts_sugar_span(p.value) == function.span)
        .and_then(|property| {
            let holder = file
                .ast
                .object_get_shapes
                .iter()
                .filter(|s| s.span.contains(property.span))
                .min_by_key(|s| s.span.end - s.span.start)?;
            let binding = file.ast.bindings.iter().find(|b| {
                b.immutable
                    && b.shape == ast::BindingShape::Identifier
                    && b.initializer == Some(holder.span)
            })?;
            let name = binding.names.first()?;
            let key = file.source_text(property.key)?;
            mutation_holder_closed(file, name.span, key, true).then_some((name.span, key))
        });
    let Some(declaration) = method
        .map(|(d, _)| d)
        .or_else(|| name.as_ref().map(|n| n.span))
    else {
        return false;
    };
    if file.ast.exports.iter().any(|export| {
        export.span.contains(function.span)
            || file
                .ast
                .exported_bindings(export)
                .any(|b| b.names.iter().any(|n| n.span == declaration))
            || export
                .specifiers
                .iter()
                .chain(&export.declarations)
                .any(|s| {
                    s.local.span == declaration
                        || file.ast.reference_declaration(s.local.span) == Some(declaration)
                })
    }) {
        return false;
    }
    let mut seen = false;
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, d)| *d == declaration)
    {
        let direct = file
            .ast
            .calls
            .iter()
            .find(|c| file.ast.peel_ts_sugar_span(c.callee) == *reference);
        let member = file.ast.members.iter().find(|m| {
            file.ast.peel_ts_sugar_span(m.object) == *reference
                && !file.ast.computed_members.contains(&m.span)
        });
        let call = direct.or_else(|| {
            member.and_then(|m| {
                file.ast
                    .calls
                    .iter()
                    .find(|c| file.ast.peel_ts_sugar_span(c.callee) == m.span)
            })
        });
        let Some(call) = call else {
            // A method holder may also have ordinary data member reads. They
            // are checked when that projected receiver is proved separately.
            if method.is_some()
                && member
                    .is_some_and(|m| file.source_text(m.property) != method.map(|(_, key)| key))
            {
                continue;
            }
            return false;
        };
        if call.construct || call.arguments.iter().any(|a| a.spread) {
            return false;
        }
        let mut offset = 0;
        let mut apply = false;
        let receiver = if let Some(member) = member {
            let property = file.source_text(member.property);
            if let Some((_, key)) = method {
                if property != Some(key) {
                    continue;
                }
                Some(member.object)
            } else {
                let Some(signature) =
                    lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)
                else {
                    return false;
                };
                if !matches!(
                    signature.qualified_name.as_ref(),
                    "Function.call"
                        | "Function.apply"
                        | "CallableFunction.call"
                        | "CallableFunction.apply"
                ) || std::path::Path::new(signature.source_file.as_ref())
                    .file_name()
                    .and_then(|p| p.to_str())
                    != Some("lib.es5.d.ts")
                    || !matches!(property, Some("call" | "apply"))
                {
                    return false;
                }
                offset = 1;
                apply = property == Some("apply");
                call.arguments.first().map(|a| a.span)
            }
        } else {
            None
        };
        seen = true;
        let input = match position {
            None => receiver,
            Some(index) if apply => call.arguments.get(1).and_then(|a| {
                ast::literal_array_element(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    file.ast.peel_ts_sugar_span(a.span),
                    index,
                )
            }),
            Some(index) => call.arguments.get(index + offset).map(|a| a.span),
        };
        if !input.is_some_and(|input| {
            mutation_target_projected(file, input, projection, object, lookup, depth - 1, visited)
        }) {
            return false;
        }
    }
    seen
}

/// Whether `span` is exactly one identifier, as opposed to an expression an
/// identifier merely begins.
fn exact_identifier(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .identifiers
        .binary_search_by_key(&span, |id| id.span)
        .is_ok()
        || file.ast.reference_declaration(span).is_some()
}

fn simple_object_binding(binding: &ast::BindingFact) -> bool {
    binding.immutable
        && binding.shape == ast::BindingShape::Object
        && !binding.names.is_empty()
        && binding.object_slots.len() == binding.names.len()
        && binding
            .object_slots
            .iter()
            .all(|slot| slot.plain_identifier_key)
        && binding.names.iter().all(|name| {
            binding
                .object_slots
                .iter()
                .any(|slot| slot.local.span == name.span)
        })
}

/// ADR 0282: a `const` array pattern of plain identifier slots. A default
/// (`[a = f]`), a nested pattern (`[[a]]`, `[{ a }]`) or a rest element binds a
/// name that has no identifier slot, so the counts differ and the pattern is
/// refused. A hole (`[, a]`) skips a position and keeps every other exact.
fn simple_array_binding(binding: &ast::BindingFact) -> bool {
    binding.immutable
        && binding.shape == ast::BindingShape::Array
        && !binding.names.is_empty()
        && binding.array_slots.iter().flatten().count() == binding.names.len()
        && binding.names.iter().all(|name| {
            binding
                .array_slots
                .iter()
                .flatten()
                .any(|slot| slot.span == name.span)
        })
}

/// ADR 0282: a slot names its closure only as an inline arrow, or as an
/// identifier for the exact unwritten `const` arrow binding the factory itself
/// declares. Arrows only, as in `member_value`: a `function` value's `this` and
/// a declaration's hoisted, writable binding are outside the summary. An
/// accessor slot (`running`) is source discovery's, never a second claim here;
/// a parameter, an import, a module binding, a call result and every other
/// expression refuse.
fn slot_closure(
    file: &FileFacts,
    factory: Span,
    element: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let (arrow, declaration) = slot_arrow(file, factory, element)?;
    // The same write pair the identifier route checks: `binding_written`
    // (redeclarations) ran in `slot_arrow`; assignment targets join through
    // the binding's symbol, which only the lookup holds.
    if let Some(declaration) = declaration {
        let symbol = lookup.entities().at(file.path.as_str(), declaration)?;
        if crate::value_identity::binding_has_write(file, lookup.entities(), symbol) {
            return None;
        }
    }
    Some(arrow)
}

/// The syntactic half of [`slot_closure`]: the arrow a slot names, and the
/// declaration of the binding that named it, if one did.
fn slot_arrow(file: &FileFacts, factory: Span, element: Span) -> Option<(Span, Option<Span>)> {
    let element = file.ast.peel_ts_sugar_span(element);
    let (arrow, declaration) = if file.ast.functions.iter().any(|f| f.span == element) {
        (element, None)
    } else {
        if !exact_identifier(file, element) {
            return None;
        }
        let declaration = file.ast.reference_declaration(element)?;
        let binding = file
            .ast
            .bindings
            .iter()
            .find(|b| b.names.iter().any(|n| n.span == declaration))?;
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
            || containing_ast_function(&file.ast, declaration).map(|f| f.span) != Some(factory)
            || crate::indexes::binding_written(file, declaration)
        {
            return None;
        }
        (
            file.ast.peel_ts_sugar_span(binding.initializer?),
            Some(declaration),
        )
    };
    let function = file.ast.functions.iter().find(|f| f.span == arrow)?;
    (function.kind == ast::FunctionKind::Arrow && !function.r#async && !function.generator)
        .then_some((arrow, declaration))
}

/// ADR 0282: the exact slots of a returned array literal. A hole or a spread
/// moves every later position, so either refuses the whole literal.
fn returned_tuple_elements(file: &FileFacts, span: Span) -> Option<Vec<Span>> {
    let span = file.ast.peel_ts_sugar_span(span);
    if !file.ast.array_literals.contains(&span) {
        return None;
    }
    let returned = file
        .ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|f| f.expression_return.as_ref()),
        )
        .find(|r| {
            r.argument
                .is_some_and(|s| file.ast.peel_ts_sugar_span(s) == span)
        })?;
    let structure = returned.structure.as_ref()?;
    if !structure.complete_literal || structure.elements.is_empty() {
        return None;
    }
    structure
        .elements
        .iter()
        .map(|s| s.map(|s| file.ast.peel_ts_sugar_span(s)))
        .collect()
}

/// The signature names the built-in operation; the admitted fresh literal
/// and closed receiver census establish the runtime array identity. A typed
/// custom receiver cannot become an array solely from this signature.
fn array_iteration_receiver(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    if call.construct {
        return None;
    }
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let property = match declaration.qualified_name.as_ref() {
        "Array.map" | "ReadonlyArray.map" => "map",
        "Array.forEach" | "ReadonlyArray.forEach" => "forEach",
        _ => return None,
    };
    if declaration.location.path != declaration.source_file
        || std::path::Path::new(declaration.source_file.as_ref())
            .file_name()?
            .to_str()?
            != "lib.es5.d.ts"
    {
        return None;
    }
    let member = file.ast.members.iter().find(|m| {
        m.span == file.ast.peel_ts_sugar_span(call.callee)
            && !file.ast.computed_members.contains(&m.span)
            && file.source_text(m.property) == Some(property)
    })?;
    Some(file.ast.peel_ts_sugar_span(member.object))
}

pub(crate) fn single_return(file: &FileFacts, function: &ast::FunctionFact) -> Option<Span> {
    if let Some(returned) = &function.expression_return {
        return returned.argument;
    }
    let mut returns = file.ast.returns.iter().filter(|r| {
        containing_ast_function(&file.ast, r.span).is_some_and(|f| f.span == function.span)
    });
    let returned = returns.next()?;
    if returns.next().is_some() || returned.conditional || !returned.control_tests.is_empty() {
        return None;
    }
    returned.argument
}

fn returned_array_elements(file: &FileFacts, span: Span) -> Option<Vec<Span>> {
    let span = file.ast.peel_ts_sugar_span(span);
    if !file.ast.array_literals.contains(&span) {
        return None;
    }
    let returned = file
        .ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|f| f.expression_return.as_ref()),
        )
        .find(|r| {
            r.argument
                .is_some_and(|s| file.ast.peel_ts_sugar_span(s) == span)
        })?;
    let structure = returned.structure.as_ref()?;
    if !structure.complete_literal || structure.elements.is_empty() {
        return None;
    }
    structure
        .elements
        .iter()
        .map(|s| {
            let s = file.ast.peel_ts_sugar_span((*s)?);
            closed_properties(file, s).map(|_| s)
        })
        .collect()
}

/// ADR 0289: the own data properties written directly in the object literal
/// at `span`, whatever else the literal holds. Derivation only.
fn direct_data_properties(file: &FileFacts, span: Span) -> Vec<&ast::ObjectPropertyFact> {
    file.ast
        .object_properties
        .iter()
        .filter(|p| {
            p.data
                && !p.computed
                && span.contains(p.span)
                && !file.ast.object_properties.iter().any(|parent| {
                    parent.span != p.span
                        && span.contains(parent.span)
                        && parent.span.contains(p.span)
                })
        })
        .collect()
}

/// Literal syntax supplies member identity only when its entire own property
/// set is closed. Nested properties/callback objects are never siblings.
fn module_scoped_source(file: &FileFacts) -> bool {
    if [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| file.path.as_str().ends_with(suffix))
    {
        return false;
    }
    !file.ast.imports.is_empty()
        || file.ast.exports.iter().any(|export| {
            !file
                .ast
                .module_blocks
                .iter()
                .any(|block| block.contains(export.span))
        })
}

pub(crate) fn closed_properties(
    file: &FileFacts,
    span: Span,
) -> Option<Vec<&ast::ObjectPropertyFact>> {
    let shape = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)?;
    let properties = file
        .ast
        .object_properties
        .iter()
        .filter(|p| {
            span.contains(p.span)
                && !file.ast.object_properties.iter().any(|parent| {
                    parent.span != p.span
                        && span.contains(parent.span)
                        && parent.span.contains(p.span)
                })
        })
        .collect::<Vec<_>>();
    let mut names = HashSet::new();
    for property in &properties {
        let name = file.source_text(property.key)?.trim_matches(['\'', '"']);
        let inert_method = property.method
            && file
                .ast
                .functions
                .iter()
                .find(|function| function.span == property.value)
                .is_some_and(|function| crate::object_members::receiver_inert(file, function));
        if property.computed
            || !(property.data || inert_method)
            || name == "__proto__"
            || !names.insert(name)
        {
            return None;
        }
    }
    (properties.len() == shape.properties.len() && !properties.is_empty()).then_some(properties)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::{
        compiler::ExecutionMap,
        core::{Generation, SourceHash, SourcePath},
    };
    use std::sync::Arc;

    fn file(source: &str) -> FileFacts {
        let ast = ast::extract("provenance.ts", source).unwrap();
        FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("provenance.ts").unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(ast),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        }
    }

    #[test]
    fn visible_member_writes_use_normalized_targets_and_cooked_keys() {
        for source in [
            "x.rows = f;",
            "x.rows += 1;",
            "x.rows ||= f;",
            "++x.rows;",
            "x.rows--;",
            "[x.rows] = values;",
            "({ key: x.rows } = value);",
            "for (x.rows of values) {}",
            "for (x.rows in values) {}",
            "delete x.rows;",
            "x['rows'] = f;",
            "delete x['r\\u006fws'];",
            "x['rows' as const] = f;",
            "x['rows' satisfies string] = f;",
            "delete x['rows'!];",
            "x[(<string>'rows')] = f;",
            "x[`r\\u006fws`] = f;",
            "(x.rows as any) = f;",
        ] {
            let file = file(source);
            let mut index = MemberStability::default();
            index.syntax(&file);
            assert!(
                matches!(index.state("rows"), Stability::Vetoed(_)),
                "{source}"
            );
            assert!(matches!(index.state("other"), Stability::Assumed));
        }
        for source in [
            "x.rows.other = f;",
            "x[x.rows] = f;",
            "const rows = x.rows;",
            "({ rows: x.other = x.rows } = value);",
            "delete x[key];",
            "x[key as string] = f;",
            "x[`r${key}ws`] = f;",
        ] {
            let file = file(source);
            let mut index = MemberStability::default();
            index.syntax(&file);
            assert!(
                matches!(index.state("rows"), Stability::Assumed),
                "{source}"
            );
        }
    }

    #[test]
    fn tuple_iteration_negative_targets_require_exact_separate_allocations() {
        for (source, separate) in [
            ("class Foo {} Foo.prototype.next = f;", true),
            ("class Foo {} const FP = Foo.prototype; FP.next = f;", true),
            (
                "class Foo {} const FP = Foo.prototype; (FP as any).next = f;",
                true,
            ),
            ("const other = () => {}; other.next = f;", true),
            (
                "const other = {}; const alias = other; alias.next = f;",
                true,
            ),
            ("const other = []; other.next = f;", true),
            ("const AP = Array.prototype; AP.next = f;", false),
            (
                "const IP = Object.getPrototypeOf([][Symbol.iterator]()); IP.next = f;",
                false,
            ),
            ("function patch(target) { target.next = f; }", false),
            ("class Foo {} Foo = Array; Foo.prototype.next = f;", false),
            (
                "let other = {}; other = Array.prototype; other.next = f;",
                false,
            ),
            ("const a = b; const b = a; a.next = f;", false),
        ] {
            let file = file(source);
            let member = file
                .ast
                .members
                .iter()
                .find(|member| {
                    file.ast.write_targets.contains(&member.span)
                        && member_key(&file, member) == Some("next")
                })
                .unwrap();
            assert_eq!(
                separate_iteration_target(&file, member.object, 24),
                separate,
                "{source}"
            );
        }
        for key in [None, Some("next"), Some("return"), Some("__proto__")] {
            assert!(iteration_key(key));
        }
        for key in ["bar", "filter", "length", "0", "prototype", "iterator"] {
            assert!(!iteration_key(Some(key)), "{key}");
        }
    }

    #[test]
    fn wildcard_target_requires_every_local_identity_input_to_be_distinct() {
        for (source, distinct) in [
            (
                "const location = {}; const other = {}; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const other = {}; const alias = other; Object.assign(alias, input);",
                true,
            ),
            (
                "const location = {}; const other = {}; Object.assign((other as object), input);",
                true,
            ),
            (
                "const location = {}; let target; target = {}; target = target; Object.assign(target, input);",
                true,
            ),
            (
                "const location = {}; Object.assign(location, input);",
                false,
            ),
            (
                "const location = {}; const other = location; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; const other = input; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; let other = {}; other = input; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; let other = {}; ({ x: other } = input); Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; let other = {}; for (other of input) {} Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; export const other = {}; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; const other = {}; eval(input); Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; function patch(other) { Object.assign(other, input); }",
                false,
            ),
            (
                "const location = {}; const other = {}; Object.assign(other || {}, input);",
                true,
            ),
            (
                "const location = {}; Object.assign(location || {}, input);",
                false,
            ),
            ("const location = {}; Object.assign(make(), input);", false),
            (
                "const location = {}; const other = function () {}; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const other = () => {}; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; let other = () => {}; other = location; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; declare const other: object; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; Object.assign([{}, {}][\"01\"], input);",
                false,
            ),
            (
                "const location = {}; const holder = { patch() { Object.assign(this, input); } }; holder.patch();",
                false,
            ),
            (
                "const location = {}; const holder = { other: {}, patch() { Object.assign(this.other, input); } }; holder.patch();",
                true,
            ),
            (
                "const location = {}; const holder = { other: location, patch() { Object.assign(this.other, input); } }; holder.patch();",
                false,
            ),
            (
                "const location = {}; const holder = { other: {}, change() { this[key] = location; }, patch() { Object.assign(this.other, input); } }; holder.change(); holder.patch();",
                false,
            ),
            (
                "const location = {}; const holder = { patch() { Object.assign(this, input); } }; const borrowed = holder.patch; holder.patch();",
                false,
            ),
            (
                "const location = {}; const other = Math.random() ? {} : {}; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const other = Math.random() ? {} : location; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; const other = {} && {}; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const other = {} && location; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; const other = {} ?? {}; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const other = {} ?? location; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; const [other] = [{}]; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const [other] = [location]; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; const { other } = { other: {} }; Object.assign(other, input);",
                true,
            ),
            (
                "const location = {}; const { other } = { other: location }; Object.assign(other, input);",
                false,
            ),
            (
                "const location = {}; function patch({ other }) { Object.assign(other, input); } patch({ other: {} }); patch({ other: {} });",
                true,
            ),
            (
                "const location = {}; function patch({ other }) { Object.assign(other, input); } patch({ other: {} }); patch({ other: location });",
                false,
            ),
            (
                "const location = {}; function patch([other]) { Object.assign(other, input); } patch([{}]);",
                true,
            ),
            (
                "const location = {}; function patch([other]) { Object.assign(other, input); } patch([location]);",
                false,
            ),
            (
                "const location = {}; function patch(other) { Object.assign(other, input); } const escaped = patch; patch({});",
                false,
            ),
            (
                "const location = {}; const holder = { other: {} }; holder.other = location; Object.assign(holder.other, input);",
                false,
            ),
            (
                "const location = {}; const holder = { other: {} }; handoff(holder); Object.assign(holder.other, input);",
                false,
            ),
        ] {
            let file = file(source);
            let object = location(
                file.path.shared(),
                file.ast.bindings[0].initializer.unwrap(),
            );
            let target = file
                .ast
                .calls
                .iter()
                .find(|call| file.source_text(call.callee) == Some("Object.assign"))
                .unwrap()
                .arguments[0]
                .span;
            let facts = ProjectFacts {
                generation: file.generation,
                project_id: "fixture".into(),
                files: vec![file.clone()],
                typescript: solid_facts::TypeScriptTable::from_parts(
                    3,
                    1,
                    "fixture",
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                ),
                typescript_changes: None,
                resolved_imports: None,
                runtime_resolutions: None,
                runtime_symbol_redirects: HashMap::new(),
                dependency_source_roots: Vec::new(),
            };
            let entities = crate::EntitySymbols {
                by_path: HashMap::new(),
            };
            let ast_indexes = HashMap::new();
            let names = HashMap::new();
            let dialect = solid_dialect::Solid2;
            let lookup =
                SemanticLookup::new(&facts, &ast_indexes, &entities, &names, &dialect, true);
            // A declaration at the same byte offsets in a different file is
            // not a back edge and must not short-circuit an unknown input.
            let mut visited = file
                .ast
                .bindings
                .iter()
                .flat_map(|binding| {
                    binding
                        .names
                        .iter()
                        .map(|name| ("different-module.ts".to_owned(), name.span))
                })
                .collect();
            assert_eq!(
                mutation_target_is_distinct(&file, target, &object, &lookup, 32, &mut visited),
                distinct,
                "{source}"
            );
        }
    }

    #[test]
    fn returned_members_require_the_entire_literal_to_be_closed() {
        for (source, admitted) in [
            ("const result = { rows, selected: () => rows() };", true),
            (
                "const result = { rows, selected: () => ({rows: other}) };",
                true,
            ),
            ("const result = { rows, ...other };", false),
            ("const result = { [key]: rows };", false),
            ("const result = { rows, rows: other };", false),
            ("const result = { get rows() { return read(); } };", false),
            ("const result = { rows() { return read(); } };", true),
            ("const result = { rows() { return this; } };", false),
            ("const result = { rows() { return arguments; } };", false),
            ("const result = { __proto__: other, rows };", false),
        ] {
            let file = file(source);
            let span = file.ast.bindings[0].initializer.unwrap();
            assert_eq!(
                closed_properties(&file, span).is_some(),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn returned_array_census_requires_every_exact_slot_to_be_a_closed_object() {
        for (source, admitted) in [
            (
                "function make() { return [{ query: read }, { query: other }]; }",
                true,
            ),
            ("const make = () => ([{ query: read }] as const);", true),
            (
                "function make() { return [{ query: read }, ...other]; }",
                false,
            ),
            ("function make() { return [{ query: read }, ,]; }", false),
            ("function make() { return [{ query: read }, 0]; }", false),
            ("function make() { return [[{ query: read }]]; }", false),
            (
                "function make() { return [{ query: read, ...extra }]; }",
                false,
            ),
            ("function make() { return [{ [key]: read }]; }", false),
            (
                "function make() { return [{ get query() { return read; } }]; }",
                false,
            ),
            (
                "function make() { return [{ query() { return read(); } }]; }",
                true,
            ),
            (
                "function make() { return [{ query() { return this; } }]; }",
                false,
            ),
            ("function make() { return []; }", false),
        ] {
            let file = file(source);
            let returned = single_return(&file, &file.ast.functions[0]).unwrap();
            assert_eq!(
                returned_array_elements(&file, returned).is_some(),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn returned_tuple_census_requires_every_slot_literal() {
        for (source, slots) in [
            (
                "function make() { const start = () => 1; return [read, start]; }",
                Some(2),
            ),
            (
                "function make() { return [read, () => 1] as const; }",
                Some(2),
            ),
            ("const make = () => [read, () => 1] as const;", Some(2)),
            ("function make() { return [read, , start]; }", None),
            ("function make() { return [read, ...rest]; }", None),
            ("function make() { return [...rest, start]; }", None),
            ("function make() { return []; }", None),
        ] {
            let file = file(source);
            let returned = single_return(&file, &file.ast.functions[0]).unwrap();
            assert_eq!(
                returned_tuple_elements(&file, returned).map(|elements| elements.len()),
                slots,
                "{source}"
            );
        }
    }

    #[test]
    fn simple_array_binding_refuses_defaults_rest_nested_and_mutable() {
        for (source, admitted) in [
            ("const [running, start] = make();", true),
            ("const [, start] = make();", true),
            ("const [running, start] = make() as const;", true),
            ("let [running, start] = make();", false),
            ("var [running, start] = make();", false),
            ("const [running, start = other] = make();", false),
            ("const [running, ...rest] = make();", false),
            ("const [running, [start]] = make();", false),
            ("const [running, { start }] = make();", false),
        ] {
            let file = file(source);
            assert_eq!(
                simple_array_binding(&file.ast.bindings[0]),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn tuple_slot_is_the_exact_arrow_the_factory_declares() {
        let source = "const outer = () => 0;
function make(parameter: () => number) {
  const start = () => 1;
  const typed = (() => 2) as () => number;
  let replaceable = () => 3;
  const expression = function () { return 4; };
  function declared() { return 5; }
  const wrapped = wrap(() => 6);
  return [start, typed, () => 9, (() => 10) as () => number, replaceable,
    expression, declared, wrapped, outer, parameter, async () => 11];
}";
        let file = file(source);
        let factory = file
            .ast
            .functions
            .iter()
            .find(|f| f.kind == ast::FunctionKind::Declaration && f.parameters.len() == 1)
            .unwrap();
        let returned = single_return(&file, factory).unwrap();
        let elements = returned_tuple_elements(&file, returned).unwrap();
        let admitted = elements
            .iter()
            .map(|element| slot_arrow(&file, factory.span, *element).is_some())
            .collect::<Vec<_>>();
        assert_eq!(
            admitted,
            [
                true, true, true, true, false, false, false, false, false, false, false
            ]
        );
        // A slot names the arrow itself, never the binding or the wrapper.
        let (arrow, declaration) = slot_arrow(&file, factory.span, elements[1]).unwrap();
        assert_eq!(file.source_text(arrow), Some("() => 2"));
        assert_eq!(declaration.and_then(|d| file.source_text(d)), Some("typed"));
        let (arrow, declaration) = slot_arrow(&file, factory.span, elements[3]).unwrap();
        assert_eq!(file.source_text(arrow), Some("() => 10"));
        assert_eq!(declaration, None);
    }

    #[test]
    fn factory_return_census_excludes_alternatives_and_nested_returns() {
        for (source, admitted) in [
            ("function make() { return { rows }; }", true),
            (
                "function make() { const read = () => { return 1; }; return { rows }; }",
                true,
            ),
            (
                "function make() { if (flag) return { rows }; return other; }",
                false,
            ),
            ("function make() { if (flag) return { rows }; }", false),
            ("function make() { return; }", false),
        ] {
            let file = file(source);
            let function = file
                .ast
                .functions
                .iter()
                .find(|f| f.kind == ast::FunctionKind::Declaration)
                .unwrap();
            assert_eq!(
                single_return(&file, function).is_some(),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn object_callable_definitions_never_supply_their_invocation_role() {
        for (source, stored) in [
            ("const decor = { header: () => read() };", true),
            ("const notes = () => read(); const decor = { notes };", true),
            (
                "const notes = () => read(); const decor = { notes: (notes as () => number) };",
                true,
            ),
            ("consume({ header: () => inline(() => read()) });", true),
            (
                "const notes = () => read(); function other(notes) { return { notes }; }",
                false,
            ),
            (
                "function Consumer() { const value = read(); return value; }",
                false,
            ),
        ] {
            let file = file(source);
            let read = file
                .ast
                .calls
                .iter()
                .find(|call| call.static_callee(&file.source) == Some("read"))
                .unwrap();
            assert_eq!(
                super::super::runs_in_object_callable(&file, read.span),
                stored,
                "{source}"
            );
        }
    }
}
