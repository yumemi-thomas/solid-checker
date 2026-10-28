//! Shared fine-grained rules decomposed out of upstream's `reactivity`.
//!
//! Upstream ships one rule reporting eight unrelated defects behind eight
//! message ids, so a project that disagrees with one of them has to silence
//! the other seven too. Here each is its own rule; the mapping is documented
//! in `docs/rules/README.md`.
//!
//! Two of the eight — the untracked read and the read after `await` — are the
//! engine's own analysis and are reported from the reactive IR as SC1001 and
//! SC1002, not from here. This module owns the rest.
//!
//! Unlike the sibling modules, which port a 1.x-era ESLint surface, the
//! defects here are defects in both language versions — an accessor used as
//! a value, a proxy written through, a listener bound to a call's result —
//! so both dialects' catalogs carry them (1.x as `v1/<rule>`, 2.0 under the
//! checker's plain names, same SC codes so suppressions survive a
//! migration).
//!
//! # Proven sources, not named ones
//!
//! Upstream decides what is reactive from syntax and convention: a variable
//! destructured from a call spelled `createSignal`, a parameter whose name
//! looks like `props`, a function whose name matches `/^(?:use|create)[A-Z]/`.
//! That is why it reports a memo passed to a helper but not the structurally
//! identical signal (upstream issue #182), and why a signal reached through a
//! wrapper like `makePersisted(createSignal(...))` defeats it entirely (#190).
//!
//! These rules ask the engine instead: [`UpstreamCompatContext::accessors`]
//! and `setters` are the sources it *proved*, through TypeScript symbol
//! resolution, dependency source and cross-file propagation. A local named
//! `createSignal` that is not Solid's is not a source here, and a signal that
//! arrives through three wrappers still is.

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, IdentifierRole};
use solid_facts::core::Span;
use typefacts::{ArrayShape, Callability, ResolvedCallValidity};

use super::{
    UpstreamCompatContext, expression_array_shape, expression_runtime_value_domain,
    is_lowercase_led, jsx_name_is_type_checked, member_root, source_symbol_at,
    static_string_expression, text,
};
use crate::pipeline::parallel_file_results;
use crate::runtime_semantics::{RuntimeArgumentBehavior, argument_behavior};
use crate::{
    DirectMutationTarget, ReactiveSourceKind, StaticDefect, StaticDefectKind, known_primitive,
    location,
};
use crate::{HashMap, HashSet};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    uncalled_accessor(file, context, defects);
    no_direct_mutation(file, context, defects);
    no_store_mutator_call(file, context, defects);
    no_store_delete(file, context, defects);
    expected_function_got_expression(file, context, defects);
    reactive_source_uncaptured(file, context, defects);
}

/// `v1/reactive-source-uncaptured` — the uncertifiable half of upstream's
/// `reactivity`.
///
/// A proven reactive source handed, uncalled, to a callee nothing in the
/// analysis can describe: not one of the dialect's primitives, no body in the
/// project, no dependency-source summary. The callee may call it in a tracking
/// scope, read it once and sever the reactivity, or store it for later — and
/// which of those it does decides whether the surrounding code is correct.
///
/// This reports the *gap*, not a defect, which is why it is uncertifiable
/// rather than a violation. It is the call-site companion to
/// `v1/package-contract-incomplete`: that rule fires once at an
/// undescribed import, this one fires where the missing description actually
/// costs certification.
///
/// Passing a source on is idiomatic — it is how accessors travel — so the
/// unknown callee is the entire premise. A helper defined in the project is
/// read directly and never reported here.
///
/// The premise is a callee whose behavior *could* be described: a package
/// export, or project code with no body here. A standard-library declaration
/// is neither -- no `solid-reactivity.json` entry can describe `setTimeout` --
/// so it is excluded by that compiler fact rather than by the bare-specifier
/// import test it replaces. Among the callees that remain, an exact
/// compiler-selected `ValueOnly` parameter certifies that the source is not
/// invoked; every other valid unresolved boundary remains an explicit
/// obligation, and invalid/recovery calls stay TypeScript-owned.
///
/// The argument must be a bare identifier. `entities.at` answers a call span
/// with the *callee's* symbol, so without that gate `console.log(count())`
/// reads as handing `count` itself to the callee.
fn reactive_source_uncaptured(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    // Nothing here can report unless some call hands a bare, proven, non-store
    // source to a callee. That is two map lookups per identifier argument, and
    // answering it first skips resolving the file's primitives -- the dominant
    // cost -- on every file that has no such call at all.
    let hands_over_a_source = |call: &solid_facts::ast::CallFact| {
        call.arguments.iter().any(|argument| {
            argument.value == ArgumentValueKind::Identifier
                && context
                    .entities
                    .at(file.path.as_str(), argument.span)
                    .is_some_and(|symbol| {
                        context.accessors.contains_key(symbol)
                            && context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                    })
        }) || (!context.forwarded_uncaptured.is_empty()
            && apply_array(file, call)
                .is_some_and(|array| sources_within(file, context, array.span).next().is_some()))
    };
    if !file.ast.calls.iter().any(hands_over_a_source) {
        return;
    }
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        if !hands_over_a_source(call) {
            continue;
        }
        // A dialect primitive is described by definition.
        if known_primitive(&primitives.calls[index]).is_some() {
            continue;
        }
        // A call the engine resolved into the project has a body it already
        // walked; that body is reported here only where it may hand the
        // parameter on to an undescribed callee (ADR 0370).
        if let Some(target) = caller_target(file, context, call) {
            forwarded_source_uncaptured(file, context, call, target, defects);
            continue;
        }
        let Some(resolved_call) = undescribed_callee(file, context, call) else {
            continue;
        };
        let callee_text = text(file, call.callee);
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            let Some(name) = handed_source(file, context, argument) else {
                continue;
            };
            if !may_invoke_argument(resolved_call, argument_index) {
                continue;
            }
            defects.push(StaticDefect {
                kind: StaticDefectKind::ReactiveSourceUncaptured {
                    source: name.to_owned(),
                    callee: callee_text.to_owned(),
                },
                location: location(file.path.shared(), argument.span),
                analysis_context: crate::PACKAGE_CALLEE.to_owned(),
                fixes: vec![],
                uncertain: false,
            });
        }
    }
}

/// The display name of the proven, non-store reactive source `argument`
/// names by itself.
///
/// Only a bare reference to the source itself. A call, a member read, or an
/// expression built from one passes a *value*, whose reactivity was already
/// resolved before the callee saw it. `entities.at` answers the callee's
/// symbol for a call span, so the argument's own kind is what enforces this
/// -- without it `console.log(count())` reads as handing `count` over.
fn handed_source<'c>(
    file: &FileFacts,
    context: &'c UpstreamCompatContext<'_>,
    argument: &solid_facts::ast::ArgumentFact,
) -> Option<&'c str> {
    if argument.value != ArgumentValueKind::Identifier {
        return None;
    }
    let symbol = context.entities.at(file.path.as_str(), argument.span)?;
    let (name, _) = context.accessors.get(symbol)?;
    if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
        return None;
    }
    Some(name.as_str())
}

/// The resolved call of `call` when its callee is one nothing in the
/// analysis describes: not a project body (the caller checks), with an exact
/// entity, a valid compiler-selected signature (invalid/recovery calls stay
/// TypeScript-owned), not a standard-library declaration, and not a callback
/// parameter of the enclosing function.
fn undescribed_callee<'c>(
    file: &FileFacts,
    context: &UpstreamCompatContext<'c>,
    call: &solid_facts::ast::CallFact,
) -> Option<&'c typefacts::ResolvedCall> {
    let callee_symbol = context.entities.at(file.path.as_str(), call.callee)?;
    let resolved_call = context.lookup.resolved_callee_call(file, call.callee)?;
    if resolved_call.validity != ResolvedCallValidity::Valid {
        return None;
    }
    // A standard-library callee is ambient: it belongs to no package, so
    // no `solid-reactivity.json` entry could ever describe it and the
    // finding would demand a fix nobody can apply. This is the exact
    // compiler fact that replaces the old bare-specifier import test --
    // which also excluded them, by accident of never reaching them.
    if resolved_call
        .declaration
        .as_ref()
        .is_some_and(|declaration| declaration.standard_library)
    {
        return None;
    }
    // Calling a callback parameter with an accessor does not read or lose
    // that source inside this helper. The interprocedural summary
    // records the accessor at the callback argument position, so the
    // consumer can classify the callback parameter exactly.
    let callee_is_parameter = file
        .ast
        .functions_body_containing(call.span)
        .min_by_key(|function| function.body.end - function.body.start)
        .is_some_and(|function| {
            function.parameters.iter().any(|parameter| {
                parameter.names.iter().any(|name| {
                    context
                        .entities
                        .at(file.path.as_str(), name.span)
                        .is_some_and(|symbol| symbol == callee_symbol)
                })
            })
        });
    (!callee_is_parameter).then_some(resolved_call)
}

/// Whether the undescribed callee may invoke a callable at `argument_index`:
/// an exact compiler-selected `ValueOnly` parameter certifies it does not.
fn may_invoke_argument(resolved_call: &typefacts::ResolvedCall, argument_index: usize) -> bool {
    argument_behavior(resolved_call, Some(Callability::Callable), argument_index)
        != Some(RuntimeArgumentBehavior::ValueOnly)
}

/// The position a rest parameter's tail fills: no argument index names it.
const REST: usize = usize::MAX;

/// An analyzed function a call enters, and how its arguments reach it.
#[derive(Clone, Copy)]
struct Entry<'a> {
    file: &'a FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
    /// How the call's arguments map onto the function's parameters.
    transport: Transport,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transport {
    /// `f(a, b)`: argument `i` is parameter `i`.
    Direct,
    /// `f.call(thisArg, a, b)`: argument `i + 1` is parameter `i`.
    Call,
    /// `f.apply(thisArg, array)`: the array's elements are the arguments,
    /// and which element fills which parameter is not followed.
    Apply,
}

/// `f.call` / `f.apply` on a function the analysis has a body for, with the
/// standard library's `Function.prototype` method selected for the member.
fn bound_method_entry<'a>(
    file: &FileFacts,
    context: &UpstreamCompatContext<'a>,
    call: &solid_facts::ast::CallFact,
) -> Option<Entry<'a>> {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.computed_members.binary_search(&callee).is_ok() {
        return None;
    }
    let member = file.ast.member_at(callee)?;
    let transport = match file.source_text(member.property)? {
        "call" => Transport::Call,
        "apply" => Transport::Apply,
        _ => return None,
    };
    let resolved = context.lookup.resolved_callee_call(file, call.callee)?;
    if !resolved
        .declaration
        .as_ref()
        .is_some_and(|declaration| declaration.standard_library)
    {
        return None;
    }
    let (target_file, function) = context.lookup.function_called_at(
        file.path.as_str(),
        file.ast.peel_ts_sugar_span(member.object),
    )?;
    Some(Entry {
        file: target_file,
        function,
        transport,
    })
}

/// The analyzed function a caller's call enters, as the `package-callee` check decides
/// it: the identifier callee's function, or `.call`/`.apply` of one. A
/// member callee stays the rule's own question.
fn caller_target<'a>(
    file: &FileFacts,
    context: &UpstreamCompatContext<'a>,
    call: &solid_facts::ast::CallFact,
) -> Option<Entry<'a>> {
    if let Some((target_file, function)) = context
        .lookup
        .function_called_at(file.path.as_str(), call.callee)
    {
        return Some(Entry {
            file: target_file,
            function,
            transport: Transport::Direct,
        });
    }
    bound_method_entry(file, context, call)
}

/// The analyzed function a call inside a summarized body enters: as
/// [`caller_target`], plus an exact object member (ADR 0299).
fn body_target<'a>(
    file: &FileFacts,
    context: &UpstreamCompatContext<'a>,
    call: &solid_facts::ast::CallFact,
) -> Option<Entry<'a>> {
    caller_target(file, context, call).or_else(|| {
        crate::execution_role::project_callee_at_parameter(file, call, 0, context.lookup)
            .filter(|(_, _, exact)| *exact)
            .map(|(target_file, function, _)| Entry {
                file: target_file,
                function,
                transport: Transport::Direct,
            })
    })
}

/// The parameter positions of `entry`'s function that argument `index` of
/// `call` may fill. A spread before it (or the argument's own spread) adds
/// zero or more elements, so the position is at least the count of plain
/// arguments before it and otherwise unknown; a position past the declared
/// parameters fills the rest parameter.
fn reached_positions(
    call: &solid_facts::ast::CallFact,
    entry: &Entry<'_>,
    index: usize,
) -> Vec<usize> {
    let declared = entry.function.parameters.len();
    let rest = entry.function.rest_parameter;
    let first = match entry.transport {
        Transport::Direct => 0,
        Transport::Call => 1,
        Transport::Apply => {
            if index != 1 {
                return Vec::new();
            }
            let mut all = (0..declared).collect::<Vec<_>>();
            if rest {
                all.push(REST);
            }
            return all;
        }
    };
    if index < first {
        return Vec::new();
    }
    let before = &call.arguments[first..index];
    let plain_before = before.iter().filter(|argument| !argument.spread).count();
    let spread = call.arguments[index].spread || before.iter().any(|argument| argument.spread);
    let mut positions = if spread {
        (plain_before..declared).collect::<Vec<_>>()
    } else if plain_before < declared {
        vec![plain_before]
    } else {
        Vec::new()
    };
    if rest && (spread || plain_before >= declared) {
        positions.push(REST);
    }
    positions
}

/// The array literal `f.apply(thisArg, [ ... ])` spreads into `f`.
fn apply_array<'f>(
    file: &'f FileFacts,
    call: &'f solid_facts::ast::CallFact,
) -> Option<&'f solid_facts::ast::ArgumentFact> {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let member = file.ast.member_at(callee)?;
    (file.source_text(member.property) == Some("apply"))
        .then(|| call.arguments.get(1))
        .flatten()
}

/// The proven, non-store sources referenced inside `span`.
fn sources_within<'c>(
    file: &'c FileFacts,
    context: &'c UpstreamCompatContext<'_>,
    span: Span,
) -> impl Iterator<Item = &'c str> + 'c {
    file.ast
        .identifiers
        .iter()
        .filter(move |identifier| {
            identifier.role == IdentifierRole::Reference && span.contains(identifier.span)
        })
        .filter_map(move |identifier| {
            let symbol = source_symbol_at(context, file, identifier.span)?;
            let (name, _) = context.accessors.get(symbol)?;
            (context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store))
                .then_some(name.as_str())
        })
}

/// ADR 0370: a proven source passed to an analyzed function (project or
/// selected package source) at a position whose parameter may reach the
/// argument of an undescribed callee. Nothing proves the function reaches
/// that call, with this value, whenever it runs, so the result is an
/// obligation (SC9012), never a violation or a silent clean.
fn forwarded_source_uncaptured(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    call: &solid_facts::ast::CallFact,
    entry: Entry<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    // A call TypeScript rejects is TypeScript's to report.
    if context.forwarded_uncaptured.is_empty()
        || context
            .lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| resolved.validity == ResolvedCallValidity::Recovery)
    {
        return;
    }
    let callee_text = match entry.transport {
        Transport::Direct => text(file, call.callee),
        Transport::Call | Transport::Apply => file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .map_or_else(
                || text(file, call.callee),
                |member| text(file, member.object),
            ),
    };
    for (index, argument) in call.arguments.iter().enumerate() {
        let handed = if entry.transport == Transport::Apply {
            (index == 1)
                .then(|| sources_within(file, context, argument.span).next())
                .flatten()
        } else if argument.spread {
            None
        } else {
            handed_source(file, context, argument)
        };
        if handed.is_none() {
            continue;
        }
        let Some(inner) = reached_positions(call, &entry, index)
            .into_iter()
            .find_map(|position| {
                context.forwarded_uncaptured.reached(
                    entry.file.path.as_str(),
                    entry.function.span,
                    position,
                )
            })
        else {
            continue;
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                callee: callee_text.to_owned(),
                member: None,
            },
            location: location(file.path.shared(), argument.span),
            analysis_context: format!("{}{inner}", crate::FORWARDED_TO_UNDESCRIBED_CALLEE),
            fixes: vec![],
            uncertain: true,
        });
    }
}

/// ADR 0370: the parameter positions of analyzed functions (project files
/// and selected package source alike) that may reach an undescribed callee.
///
/// A position is reached when a value it binds may be an argument the rule
/// above would report if it named a source: a bare reference to the
/// parameter or to a binding initialized with it, an element or spread of a
/// rest parameter or of `arguments`, anywhere in the function (a closure, a
/// default, a branch: none is proven to run or not), or an argument at a
/// position of a further analyzed function that is reached (through
/// `.call`/`.apply`, and from a spread onwards at every later position).
/// Writes to the parameter are not followed: the value may still be the
/// caller's. Being reached proves nothing runs; it only withholds
/// certification.
#[derive(Default)]
pub(crate) struct ForwardedUncaptured {
    /// `(path, function span, position)` to the undescribed callee's text,
    /// the one at the smallest `(path, start)` site.
    reached: HashMap<(String, Span, usize), (String, u32, String)>,
    functions: HashSet<(String, Span)>,
}

type ParameterKey = (String, Span, usize);

impl ForwardedUncaptured {
    pub(super) fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    fn reached(&self, path: &str, function: Span, position: usize) -> Option<&str> {
        let key_path = path.to_owned();
        if !self.functions.contains(&(key_path.clone(), function)) {
            return None;
        }
        self.reached
            .get(&(key_path, function, position))
            .map(|(_, _, callee)| callee.as_str())
    }

    pub(super) fn build(files: &[FileFacts], context: &UpstreamCompatContext<'_>) -> Self {
        let mut seeds: Vec<(ParameterKey, (String, u32, String))> = Vec::new();
        // target position -> the positions forwarded into it
        let mut forwards: HashMap<ParameterKey, Vec<ParameterKey>> = HashMap::default();
        for (file_seeds, file_forwards) in
            parallel_file_results(files, |file| file_parameter_flows(file, context))
        {
            seeds.extend(file_seeds);
            for (target, source) in file_forwards {
                forwards.entry(target).or_default().push(source);
            }
        }
        let mut reached: HashMap<ParameterKey, (String, u32, String)> = HashMap::default();
        let mut work = Vec::new();
        for (key, site) in seeds {
            let better = reached
                .get(&key)
                .is_none_or(|current| (&site.0, site.1) < (&current.0, current.1));
            if better {
                reached.insert(key.clone(), site);
                work.push(key);
            }
        }
        // Monotone: a site only ever moves to a smaller `(path, start)`, so
        // the fixpoint and its chosen sites are independent of order.
        while let Some(key) = work.pop() {
            let site = reached[&key].clone();
            for source in forwards.get(&key).into_iter().flatten() {
                let better = reached
                    .get(source)
                    .is_none_or(|current| (&site.0, site.1) < (&current.0, current.1));
                if better {
                    reached.insert(source.clone(), site.clone());
                    work.push(source.clone());
                }
            }
        }
        let functions = reached
            .keys()
            .map(|(path, span, _)| (path.clone(), *span))
            .collect();
        Self { reached, functions }
    }
}

type FileFlows = (
    Vec<(ParameterKey, (String, u32, String))>,
    Vec<(ParameterKey, ParameterKey)>,
);

/// One file's seeds (a position handed to an undescribed callee) and
/// forwards (a position handed to an analyzed function's position).
fn file_parameter_flows(file: &FileFacts, context: &UpstreamCompatContext<'_>) -> FileFlows {
    let mut seeds = Vec::new();
    let mut forwards = Vec::new();
    // Declaration span -> the (function, position) pairs its value may be.
    let mut parameters: HashMap<Span, Vec<(Span, usize)>> = HashMap::default();
    for function in &file.ast.functions {
        for (position, parameter) in function.parameters.iter().enumerate() {
            if parameter.shape != solid_facts::ast::BindingShape::Identifier {
                continue;
            }
            if let [name] = parameter.names.as_slice() {
                parameters
                    .entry(name.span)
                    .or_default()
                    .push((function.span, position));
            }
        }
        for name in &function.rest_parameter_names {
            parameters
                .entry(name.span)
                .or_default()
                .push((function.span, REST));
        }
    }
    let has_arguments_object = file
        .ast
        .identifiers
        .iter()
        .any(|identifier| file.source_text(identifier.span) == Some("arguments"));
    let has_rest = file
        .ast
        .functions
        .iter()
        .any(|function| !function.rest_parameter_names.is_empty());
    if parameters.is_empty() && !has_arguments_object {
        return (seeds, forwards);
    }
    // A binding initialized with the parameter itself (`const t = source`),
    // to any depth, may hold what the caller passed.
    loop {
        let mut added = false;
        for binding in &file.ast.bindings {
            let ([name], Some(initializer)) = (
                binding.names.as_slice(),
                binding.initializer_identifier.as_ref(),
            ) else {
                continue;
            };
            if binding.shape != solid_facts::ast::BindingShape::Identifier
                || parameters.contains_key(&name.span)
            {
                continue;
            }
            if let Some(owners) = file
                .ast
                .reference_declaration(initializer.span)
                .and_then(|declaration| parameters.get(&declaration))
                .cloned()
            {
                parameters.insert(name.span, owners);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    // `arguments` names every position of the nearest non-arrow function.
    let arguments_owner = |span: Span| {
        file.ast
            .functions
            .iter()
            .filter(|function| {
                function.kind != solid_facts::ast::FunctionKind::Arrow
                    && function.body.contains(span)
            })
            .min_by_key(|function| function.body.end - function.body.start)
            .map(|function| {
                let mut owners = (0..function.parameters.len())
                    .map(|position| (function.span, position))
                    .collect::<Vec<_>>();
                owners.push((function.span, REST));
                owners
            })
    };
    // The positions an argument's value may come from: a bare reference to a
    // parameter (or its alias), or any reference to a rest parameter or to
    // `arguments` inside it (an element, a spread).
    let owners_of = |argument: &solid_facts::ast::ArgumentFact| {
        let mut owners = Vec::new();
        if argument.value == ArgumentValueKind::Identifier
            && !argument.spread
            && let Some(found) = file
                .ast
                .reference_declaration(argument.span)
                .and_then(|declaration| parameters.get(&declaration))
        {
            owners.extend(
                found
                    .iter()
                    .copied()
                    .filter(|(_, position)| *position != REST),
            );
        }
        // Only a file with a rest parameter or `arguments` has such a transport.
        let transports = if has_rest || has_arguments_object {
            file.ast.identifiers.as_slice()
        } else {
            &[]
        };
        for identifier in transports.iter().filter(|identifier| {
            identifier.role == IdentifierRole::Reference && argument.span.contains(identifier.span)
        }) {
            if let Some(found) = file
                .ast
                .reference_declaration(identifier.span)
                .and_then(|declaration| parameters.get(&declaration))
            {
                owners.extend(
                    found
                        .iter()
                        .copied()
                        .filter(|(_, position)| *position == REST),
                );
            } else if file.source_text(identifier.span) == Some("arguments")
                && file.ast.reference_declaration(identifier.span).is_none()
                && let Some(found) = arguments_owner(identifier.span)
            {
                owners.extend(found);
            }
        }
        owners.sort_unstable();
        owners.dedup();
        owners
    };
    let path = file.path.as_str();
    let primitives = std::cell::OnceCell::new();
    for (index, call) in file.ast.calls.iter().enumerate() {
        let handed = call
            .arguments
            .iter()
            .enumerate()
            .map(|(position, argument)| (position, owners_of(argument)))
            .filter(|(_, owners)| !owners.is_empty())
            .collect::<Vec<_>>();
        if handed.is_empty() {
            continue;
        }
        if known_primitive(
            &primitives
                .get_or_init(|| context.lookup.primitives(file))
                .calls[index],
        )
        .is_some()
        {
            continue;
        }
        let key = |(function, position): (Span, usize)| (path.to_owned(), function, position);
        if let Some(entry) = body_target(file, context, call) {
            for (position, owners) in handed {
                for target in reached_positions(call, &entry, position) {
                    for &owner in &owners {
                        forwards.push((
                            (
                                entry.file.path.as_str().to_owned(),
                                entry.function.span,
                                target,
                            ),
                            key(owner),
                        ));
                    }
                }
            }
            continue;
        }
        let Some(resolved_call) = undescribed_callee(file, context, call) else {
            continue;
        };
        let callee_text = text(file, call.callee);
        for (position, owners) in handed {
            let argument = &call.arguments[position];
            // Only a bare value's position has a compiler-selected parameter
            // that can prove it is never invoked.
            if !argument.spread
                && argument.value == ArgumentValueKind::Identifier
                && !may_invoke_argument(resolved_call, position)
            {
                continue;
            }
            for owner in owners {
                seeds.push((
                    key(owner),
                    (path.to_owned(), call.span.start, callee_text.to_owned()),
                ));
            }
        }
    }
    (seeds, forwards)
}

/// `v1/reactive-handler-frozen` — upstream's
/// `expectedFunctionGotExpression`.
///
/// A native element's event-handler binding receiving an already-evaluated
/// expression: `onClick={count()}` binds whatever `count()` returned during
/// setup as the listener.
///
/// Narrower than upstream, deliberately. `onClick={makeHandler()}` is a
/// factory call and correct, and upstream cannot tell the two apart because it
/// works from syntax. The normal, declared attribute path reports only a
/// reactive source member whose callable value is frozen during setup.
/// Hyphenated attribute names form a separate boundary TypeScript does not
/// check: there the runtime value is classified as safe, invalid, or
/// unresolved against Solid's function-or-bound-pair representations.
fn expected_function_got_expression(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for element in &file.ast.jsx_elements {
        let element_name = text(file, element.name.span);
        // Components receive props, not listeners: `onFoo` on a component is
        // an ordinary prop and its value may legitimately be anything.
        if element_name.contains('.') || !is_lowercase_led(element_name) {
            continue;
        }
        for attribute in element
            .attributes
            .iter()
            .filter(|attribute| attribute.namespace.is_none())
        {
            let name = text(file, attribute.name);
            // Only a name the compiler lowers as a listener is a handler
            // position; `onclick` and `on-foo` are plain attributes.
            if !context.dialect.compiler_lowers_event_attribute(name) {
                continue;
            }
            let Some(expression) = attribute.expression else {
                continue;
            };
            if context.dialect.static_event_values_are_attributes()
                && (static_string_expression(context, file, expression).is_some()
                    || super::syntax::expression_is_static_literal(file, expression))
            {
                // Solid 1.x freezes this value into a plain attribute instead
                // of installing it as a listener. It is therefore not a
                // non-callable runtime listener, so this rule stays silent.
                continue;
            }
            // TypeScript deliberately skips every JSX attribute whose name
            // contains a hyphen. Solid's compiler does not when it lowers the
            // name as a listener (`onFoo-bar`); `on-foo` is a plain attribute
            // there. Keep the uncovered listener value in this rule's
            // tri-state domain instead of leaving it to fail when dispatched.
            if !jsx_name_is_type_checked(name) {
                let proof = unchecked_handler_value_proof(file, context, attribute, expression);
                match proof {
                    HandlerValueProof::Safe => {}
                    HandlerValueProof::Invalid | HandlerValueProof::Unresolved => {
                        defects.push(StaticDefect {
                            kind: StaticDefectKind::HandlerValueUnresolved {
                                attribute: name.to_owned(),
                                expression: text(file, expression).to_owned(),
                            },
                            location: location(file.path.shared(), expression),
                            analysis_context: "unchecked-native-handler-value".into(),
                            fixes: vec![],
                            uncertain: proof == HandlerValueProof::Unresolved,
                        });
                        continue;
                    }
                }
            }
            // A native listener receives its function value once during DOM
            // setup. Reading that function through reactive props/store state
            // here freezes the initial handler. The member root is a proven
            // source; a plain object member is left alone. Props follow the
            // caller classification: a handler prop every call site passes
            // statically is a plain property whose value never changes —
            // installing it once is exactly right and stays silent.
            let member_symbol = file
                .ast
                .member_at(expression)
                .and_then(|_| member_root(file, expression))
                .and_then(|root| Some((root, context.entities.at(file.path.as_str(), root)?)));
            if let Some((root, symbol)) = member_symbol {
                let uncertain = if context.accessors.contains_key(symbol) {
                    Some(false)
                } else if let Some((_, declaration)) = context.prop_sources.get(symbol) {
                    if context.uncertain_prop_sources.contains(symbol) {
                        Some(true)
                    } else {
                        let property = file
                            .ast
                            .members
                            .iter()
                            .find(|member| {
                                member.object == root && expression.contains(member.span)
                            })
                            .map(|member| text(file, member.property))
                            .unwrap_or_default();
                        match context.props_reactivity.prop_use(declaration, property) {
                            crate::source_discovery::PropUse::Static => None,
                            crate::source_discovery::PropUse::Reactive => Some(false),
                            crate::source_discovery::PropUse::Unknown => Some(true),
                        }
                    }
                } else {
                    None
                };
                // Review round 4: an element created inside a tracked JSX
                // expression (`{cond && <button onClick={props.f}/>}`) or in a
                // component's children (`<Show>`) is created again when that
                // scope re-runs, installing the current handler: "installed
                // once" is not proven there.
                let recreated = file
                    .ast
                    .jsx_containing(expression)
                    .find(|element| {
                        element
                            .attributes
                            .iter()
                            .any(|candidate| candidate.expression == Some(expression))
                    })
                    .is_some_and(|element| {
                        file.compiler
                            .tracked_regions
                            .iter()
                            .any(|region| region.span.contains(element.span))
                            || crate::execution_role::inside_component_element_children(
                                file,
                                element.span,
                            )
                    });
                if let Some(uncertain) = uncertain.map(|uncertain| uncertain || recreated) {
                    defects.push(StaticDefect {
                        kind: StaticDefectKind::ReactiveHandlerRead {
                            attribute: name.to_owned(),
                            expression: text(file, expression).to_owned(),
                        },
                        location: location(file.path.shared(), expression),
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain,
                    });
                    continue;
                }
                if context.accessors.contains_key(symbol)
                    || context.prop_sources.contains_key(symbol)
                {
                    // A proven-static prop member: not a defect, and not the
                    // call-result shape below either.
                    continue;
                }
            }
            // The call-result arm was removed on 2026-08-17 under AGENTS.md's
            // absolute rule. It fired for a handler expression that is itself a
            // call, on either of two triggers, and neither survives:
            //
            //   * the expression is *proven non-callable* -- which is exactly
            //     when TypeScript reports TS2322 ("Type 'number' is not
            //     assignable to type 'EventHandlerUnion<…>'") at that same
            //     attribute, so the finding was the type error restated;
            //   * the callee is a proven accessor -- which lands on the same
            //     TS2322 whenever the accessor's value is not callable
            //     (`onClick={count()}` with `count: Accessor<number>`). The one
            //     spelling TypeScript permits is an accessor holding a
            //     function, `onClick={handler()}`, and there the finding would
            //     be *wrong*: a JSX attribute expression is a tracked read, so
            //     that handler does update.
            //
            // What remains above is the reactive-handler-read arm, where the
            // value is callable, TypeScript is silent, and the claim is that a
            // native listener is installed once from a value that changes.
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandlerValueProof {
    Safe,
    Invalid,
    Unresolved,
}

fn unchecked_handler_value_proof(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    attribute: &solid_facts::ast::JsxAttributeFact,
    expression: Span,
) -> HandlerValueProof {
    let runtime = if attribute.runtime_type_escape {
        file.ast.peel_ts_sugar_span(expression)
    } else {
        expression
    };
    if context
        .lookup
        .entity_at(file.path.as_str(), runtime)
        .is_none()
    {
        return HandlerValueProof::Unresolved;
    }
    let Some(domain) = expression_runtime_value_domain(context, file, runtime) else {
        return HandlerValueProof::Unresolved;
    };
    if domain.unknown() {
        return HandlerValueProof::Unresolved;
    }
    // `never` cannot reach the listener. A callable (optionally absent) is a
    // valid handler, while absence sentinels disable the handler harmlessly.
    if (!domain.may_be_callable() && !domain.may_be_undefined() && !domain.may_be_other())
        || (domain.may_be_callable() && !domain.may_be_other())
        || (!domain.may_be_callable() && domain.may_be_undefined() && !domain.may_be_other())
    {
        return HandlerValueProof::Safe;
    }
    if !domain.may_be_callable()
        && !domain.may_be_undefined()
        && handler_value_is_absent_sentinel(context, file, runtime)
    {
        return HandlerValueProof::Safe;
    }
    if domain.may_be_callable() || domain.may_be_undefined() {
        return HandlerValueProof::Unresolved;
    }
    match expression_array_shape(context, file, runtime) {
        // A non-callable, non-array runtime value cannot be either accepted
        // handler representation: a function or `[handler, data]`.
        Some(ArrayShape::NotArray) => HandlerValueProof::Invalid,
        // An array may or may not be a valid two-slot bound-handler pair.
        Some(ArrayShape::Array | ArrayShape::Mixed | ArrayShape::Unknown) | None => {
            HandlerValueProof::Unresolved
        }
    }
}

/// Whether the value handed to the listener is one of the absent-handler
/// sentinels this rule certifies: `null` or `false`.
///
/// Proved from the literal the program actually passes -- written at the
/// attribute, or as the initializer of an immutable binding this reference
/// resolves to -- and never from a rendered type name. `type Falsy = false`
/// renders as `Falsy`, so a `TypeDescriptor.text` test reported the alias and
/// certified the literal for one runtime value. The runtime value domain
/// cannot answer this: `null` and `false` both arrive as `may_be_other`,
/// indistinguishable from a number.
fn handler_value_is_absent_sentinel(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> bool {
    fn is_sentinel_literal(file: &FileFacts, span: Span) -> bool {
        matches!(
            text(file, file.ast.peel_ts_sugar_span(span)).trim(),
            "null" | "false"
        )
    }
    if is_sentinel_literal(file, span) {
        return true;
    }
    context
        .lookup
        .binding_at_reference(file.path.as_str(), span)
        .is_some_and(|(binding_file, binding, _)| {
            binding.immutable
                && binding
                    .initializer
                    .is_some_and(|initializer| is_sentinel_literal(binding_file, initializer))
        })
}

/// `v1/uncalled-accessor` — upstream's `badSignal`.
///
/// An accessor referenced where a *value* was meant: interpolated into a
/// template literal, used as a coercive operand, used as a computed member
/// key, or assigned to a native JSX value attribute. The accessor is a
/// function, so the expression sees `function count() {...}` rather than the
/// current value, and it never updates.
///
/// The positions are the ones upstream enumerates; what differs is the
/// premise. A reference only counts when the engine proved the symbol is an
/// accessor, so a plain function of the same shape is not reported, and an
/// accessor that reached this file through a helper or a re-export is.
fn uncalled_accessor(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for identifier in &file.ast.identifiers {
        if identifier.role != IdentifierRole::Reference {
            continue;
        }
        let Some(symbol) = source_symbol_at(context, file, identifier.span) else {
            continue;
        };
        // Proven accessor, and specifically an accessor: a store is read by
        // path, not by call, so `store.items` is correct and not this rule's
        // business.
        let Some((name, _)) = context.accessors.get(symbol) else {
            continue;
        };
        let name = name.as_str();
        if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            continue;
        }
        // A call of the accessor is the correct use; so is passing it on,
        // which hands the callee something it can call later. The exemption
        // is deliberately unconditional: TypeFacts' resolved-call parameter
        // callability could in principle prove a callee's slot non-callable,
        // but a non-callable parameter receiving an accessor is already a
        // type error TypeScript reports itself, and recovery-signature
        // resolutions would make the "proof" wrong exactly where it fired.
        if is_called(file, identifier.span) || is_argument(file, identifier.span) {
            continue;
        }
        let Some(position) = value_position(file, identifier.span) else {
            continue;
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::UncalledAccessor {
                name: name.to_owned(),
                position: position.to_owned(),
            },
            location: location(file.path.shared(), identifier.span),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: false,
        });
    }
}

/// `v1/no-direct-mutation` — upstream's `noWrite`.
///
/// A reactive value written through instead of through its setter: `count =
/// 2` on a signal accessor, or `props.value = x` / `store.a.b = x` on props
/// and stores, which are readonly proxies in Solid.
fn no_direct_mutation(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for assignment in &file.ast.assignments {
        // The assignment target is either the reactive binding itself or the
        // object of a member chain rooted at one. The root is resolved with
        // the same reference-index fallback `uncalled-accessor` uses: an
        // assignment target is an ordinary operator operand, exactly the
        // expression shape entity facts may skip (see the field doc on
        // `source_reference_index`).
        let root = member_root(file, assignment.target).unwrap_or(assignment.target);
        let Some(symbol) = source_symbol_at(context, file, root) else {
            continue;
        };
        // `prop_sources` proves the props container, not the mutability of
        // values supplied through it. Store sources have their own proof.
        let (name, _) = match context.accessors.get(symbol) {
            Some(source) => source,
            None => {
                let Some(source) = context.prop_sources.get(symbol) else {
                    continue;
                };
                source
            }
        };
        let props = !context.accessors.contains_key(symbol);
        // `const { program } = props` binds the *value* of one property, not
        // the props container: `program.x = y` writes to whatever object the
        // caller passed (a plain mutable one as often as not), so it is not a
        // write to a readonly proxy. The binding shape is the fact -- a
        // whole-object alias (`const p = props`) keeps the container proof.
        if props
            && file.ast.bindings.iter().any(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Object
                    && binding.object_slots.iter().any(|slot| {
                        context
                            .entities
                            .get(&location(file.path.shared(), slot.local.span))
                            == Some(symbol)
                    })
            })
        {
            continue;
        }
        let name = name.as_str();
        let through_member = root != assignment.target;
        let kind = context.source_kinds.get(symbol).copied();
        // A member chain rooted at a *call* writes to whatever that call
        // returned, and this rule's claim -- the write is dropped because
        // Solid handed out a readonly proxy -- is a claim about the reactive
        // container, not about its value. Calling a signal accessor yields the
        // stored value, so `el()!.style.color = "red"` mutates a DOM node and
        // `obj().a = 2` mutates a plain object: both writes land, and neither
        // is this rule's. Nine such findings were measured against
        // `@kobalte/core`, every one of them wrong.
        //
        // Only the container binding itself is proven readonly here: `props`,
        // a store root, or an accessor written through without being called.
        // `peel_ts_sugar_span` peels `!` and `as` but never a call or a member,
        // so the deliberate `(state as { count: number }).count = 1` case below
        // still reaches its branch, and only the call form leaves.
        if through_member
            && file
                .ast
                .call_at(file.ast.peel_ts_sugar_span(root))
                .is_some()
        {
            continue;
        }
        // `createMutable` is the one Solid 1.x source whose proxy is designed
        // to be written through directly. It still behaves like a store for
        // reads, but treating that shared read shape as readonly recreates
        // eslint-plugin-solid issue #99.
        if through_member
            && context
                .source_primitives
                .get(symbol)
                .is_some_and(|primitive| primitive.as_str() == "createMutable")
        {
            continue;
        }
        // Rebinding the container itself (`count = 2` on a signal accessor,
        // `store = {}`, a props-like local) is not a dropped write, and the
        // spellings that would make it one are TypeScript's: on the audited
        // rc.17 typings a `const` binding is TS2588 and a `let` assigned a
        // number, a function or an update is TS2322/TS2356/TS2365 (tsc-oracle
        // `removed-because-redundant`). What type-checks is a deliberate
        // rebinding (`count = otherAccessor`, `store = { ... }` on a `let`)
        // or a cast, whose later reads are not proven to fail, so the branch
        // is gone (ADR 0425).
        if !through_member {
            continue;
        }
        // Props are shallow: `props.state.count = 1` may write through an
        // application-owned mutable proxy or a plain object. Only a write to
        // the props container's own property is proven dropped here. Keep
        // transparent wrappers, but never infer the nested value's behavior
        // from the root's readonly behavior.
        if props
            && (!file
                .ast
                .identifiers
                .iter()
                .any(|identifier| identifier.span == file.ast.peel_ts_sugar_span(root))
                || !file
                    .ast
                    .member_at(file.ast.peel_ts_sugar_span(assignment.target))
                    .is_some_and(|member| {
                        file.ast.peel_ts_sugar_span(member.object)
                            == file.ast.peel_ts_sugar_span(root)
                    }))
        {
            continue;
        }
        // A member written on an accessor function (`memo.a = 1`) is not a
        // write through a readonly proxy: the accessor is an ordinary
        // extensible function, so the property lands (probed on rc.17: the
        // value reads back, and the accessor is neither frozen nor a proxy).
        // The uncast spelling is TypeScript's (TS2339, TS7053 under `strict`)
        // and a cast one may attach metadata deliberately, so no claim
        // remains (tsc-oracle `removed-because-redundant`).
        let target = if props {
            DirectMutationTarget::Props
        } else if kind == Some(ReactiveSourceKind::Store) {
            store_target(context, symbol)
        } else {
            continue;
        };
        let store = matches!(
            target,
            DirectMutationTarget::Store | DirectMutationTarget::Projection
        );
        let standing = if store {
            store_write_standing(
                file,
                context,
                StoreWriteSite {
                    span: assignment.target,
                    member: assignment.target,
                    root,
                },
                symbol,
            )
        } else {
            WriteStanding::Dropped
        };
        if standing == WriteStanding::Exempt {
            continue;
        }
        // ADR 0465: a store write past the root (`store.a.b = 1`) is
        // TypeScript's where a link may be nullish, and unproven otherwise.
        let past_root = if store {
            let Some(past_root) = chain_links_verdict(file, context, assignment.target, root)
            else {
                continue;
            };
            past_root
        } else {
            false
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::DirectMutation {
                name: name.to_owned(),
                target,
            },
            location: location(file.path.shared(), assignment.target),
            analysis_context: String::new(),
            fixes: vec![],
            // A write through a 2.0 props object, or a `merge`/`omit` view of
            // it, is not dropped: on rc.13 the assigned value reads back
            // (runtime oracle cases `r9-d5-writeplainprops`, `-writemerge`,
            // `-writeomit`, `review-r1-alias-mutation`). Only a getter-backed
            // prop rejects it (a TypeError), and which props are getters is
            // not proven here, so the claim stays an obligation.
            uncertain: props || past_root || standing == WriteStanding::Unproven,
        });
    }
}

/// A store write the rule is about to report: the whole write (an assignment
/// target or a mutator call), the member expression whose object it writes
/// through (the assignment target itself, or the mutator's callee), and the
/// root identifier that member chain starts at.
/// Whether `span` is a bare identifier, or a member read that is exactly a
/// `const` (ADR 0481), which [`super::member_root`] returns as a root and
/// [`super::source_symbol_at`] resolves to that `const` (ADR 0482).
fn bare_root(file: &FileFacts, span: Span) -> bool {
    file.ast.local_alias_member(span).is_some()
        || file
            .ast
            .identifiers
            .iter()
            .any(|identifier| identifier.span == span)
}

#[derive(Clone, Copy)]
struct StoreWriteSite {
    span: Span,
    member: Span,
    root: Span,
}

/// The finding target for a write through the proven store `symbol`: a
/// `createProjection` result (or a view of one) has no setter to name, so it
/// reads differently from a `createStore` read proxy.
fn store_target(
    context: &UpstreamCompatContext<'_>,
    symbol: &crate::SymbolId,
) -> DirectMutationTarget {
    let primitive = context.source_primitives.get(symbol).or_else(|| {
        // A `merge`/`omit` view or an effect-apply parameter carries the
        // source's declaration, not its primitive.
        let (_, declaration) = context.accessors.get(symbol)?;
        context
            .source_primitives
            .iter()
            .find_map(|(source, primitive)| {
                (context.accessors.get(source).map(|(_, found)| found) == Some(declaration))
                    .then_some(primitive)
            })
    });
    if primitive.is_some_and(|primitive| primitive.as_str() == "createProjection") {
        DirectMutationTarget::Projection
    } else {
        DirectMutationTarget::Store
    }
}

/// Where a store write stands against the windows that write-enable the
/// store's proxy (ADR 0476).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteStanding {
    /// Correct code, or TypeScript's: the rule is silent.
    Exempt,
    /// The write runs with no window open, at least on one run: dropped.
    Dropped,
    /// Whether a window is open when it runs is not proven: uncertifiable.
    Unproven,
}

/// Where a write through the proven store `symbol` stands.
///
/// - A write to the root record's *own* property is TS2540 where the
///   dialect's store type is `Readonly` at that level (`@solidjs/signals`
///   rc.0-rc.6), so it is TypeScript's. The readonly-ness is shallow: a
///   nested record, or a mutator on a nested array, stays here. The root must
///   be a bare identifier: `(state as { count: number }).count = 1` casts the
///   readonly away, so TypeScript falls silent there while the write is still
///   dropped at runtime, and `member_root` resolves through the cast.
/// - 2.0 write-enables the original proxy while the store's own setter runs
///   its callback and while the compute function that derives the store runs
///   (`inDraft`, rc.17 `dist/dev.js:4955-4957`, `:5847-5866`, `:6675-6678`).
///   That is a dynamic extent, not a lexical one (ADR 0476): a write is exempt
///   only when it runs on the callback's own stack, and a write the callback
///   schedules (`queueMicrotask`, `setTimeout`, `.then`) runs after the
///   window closes and is dropped ([`write_window_standing`]).
fn store_write_standing(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    site: StoreWriteSite,
    symbol: &crate::SymbolId,
) -> WriteStanding {
    if context.dialect.store_root_properties_are_readonly()
        && file
            .ast
            .member_at(site.member)
            .is_some_and(|member| member.object == site.root)
        && bare_root(file, site.root)
    {
        return WriteStanding::Exempt;
    }
    if !context.dialect.store_setter_callback_enables_proxy_writes() {
        return WriteStanding::Dropped;
    }
    let stores = store_and_aliases(context, file, site.root, symbol);
    let windows = write_windows(file, context, &stores, symbol);
    if windows.is_empty() {
        return WriteStanding::Dropped;
    }
    write_window_standing(file, context, &windows, site.span, WINDOW_DEPTH)
}

/// How many named functions [`write_window_standing`] follows from a write to
/// the calls that run it.
const WINDOW_DEPTH: usize = 3;

/// A function that runs while the store's proxy is write-enabled, or that is
/// written inside such a function's argument: the store's own setter callback
/// (`direct`: the argument itself) or a function within that argument, and
/// the store's own compute function.
#[derive(Clone, Copy)]
struct WriteWindow {
    function: Span,
    /// The function is the argument the setter (or the deriving primitive)
    /// itself invokes, not one written inside it.
    direct: bool,
}

/// The windows of the store `store_symbol` in this file. The pairing is
/// proven, not guessed ([`is_own_setter`]). The compute function is
/// [`own_compute_functions`]'s.
fn write_windows(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    stores: &[crate::SymbolId],
    store_symbol: &crate::SymbolId,
) -> Windows {
    let mut windows = Windows::default();
    for call in &file.ast.calls {
        if !is_own_setter(context, file, call.callee, stores, 4) {
            continue;
        }
        for argument in &call.arguments {
            let direct = file.ast.peel_ts_sugar_span(argument.span);
            // A function named by the argument (`setStore(bump)`) is invoked
            // by the setter like an inline one ([`named_function_standing`]).
            windows.arguments.push(direct);
            // A function written in the argument, not the argument itself
            // (`setStore(flag ? a : b)`, a function inside an object), is a
            // region of the window whose run this does not prove.
            for function in file.ast.functions_within(argument.span) {
                windows.regions.push(WriteWindow {
                    function: function.span,
                    direct: function.span == direct,
                });
            }
        }
    }
    windows.regions.extend(
        own_compute_functions(file, context, store_symbol)
            .into_iter()
            .map(|function| WriteWindow {
                function,
                direct: true,
            }),
    );
    windows
}

/// The windows of one store in one file: the functions they run
/// ([`WriteWindow`], `regions`) and the argument positions of its own setter's calls.
#[derive(Default)]
struct Windows {
    regions: Vec<WriteWindow>,
    arguments: Vec<Span>,
}

impl Windows {
    fn is_empty(&self) -> bool {
        self.regions.is_empty() && self.arguments.is_empty()
    }
}

/// The symbols the store a write goes through may be bound to: the proven
/// store `symbol`, and the exact `const` aliases its root identifier `root` is
/// declared through (`const useIt = s1; useIt.v = 7` is a write through `s1`,
/// ADR 0463), at most four. The setter destructured beside any of them is the
/// write's own (ADR 0476).
fn store_and_aliases(
    context: &UpstreamCompatContext<'_>,
    file: &solid_facts::FileFacts,
    root: Span,
    symbol: &crate::SymbolId,
) -> Vec<crate::SymbolId> {
    let path = file.path.as_str();
    let mut stores = vec![symbol.clone()];
    let mut at = (path.to_owned(), root);
    // ADR 0482: a member alias root is no reference the index knows; the walk
    // starts from the binding its `const` declares, which is in this file.
    if let Some(alias) = file
        .ast
        .local_alias_member(file.ast.peel_ts_sugar_span(root))
    {
        let Some(source) = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == alias.source))
            .filter(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier && binding.immutable
            })
            .and_then(|binding| binding.initializer_identifier.as_ref())
        else {
            return close_over_local_aliases(context, file, stores);
        };
        at = (path.to_owned(), source.span);
    }
    for _ in 0..4 {
        let Some((binding_file, binding, bound)) = context.lookup.binding_at_reference(&at.0, at.1)
        else {
            break;
        };
        if !stores.contains(&bound) {
            stores.push(bound.clone());
        }
        let (solid_facts::ast::BindingShape::Identifier, true, Some(source)) = (
            binding.shape,
            binding.immutable,
            binding.initializer_identifier.as_ref(),
        ) else {
            break;
        };
        at = (binding_file.path.as_str().to_owned(), source.span);
    }
    // The last alias's source, when four aliases ran out the loop.
    if let Some((_, _, bound)) = context
        .lookup
        .binding_at_reference(&at.0, at.1)
        .filter(|(_, _, bound)| !stores.contains(bound))
    {
        stores.push(bound);
    }
    close_over_local_aliases(context, file, stores)
}

/// ADR 0480: the exact aliases of `local_alias_bindings` (an object or array
/// literal's slot, an object-literal arrow's slot) hold the same store, in
/// either direction, to a fixpoint within the site's file.
fn close_over_local_aliases(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    mut stores: Vec<crate::SymbolId>,
) -> Vec<crate::SymbolId> {
    let path = file.path.as_str();
    {
        let pairs = file
            .ast
            .local_alias_bindings
            .iter()
            .filter_map(|alias| {
                Some((
                    context.entities.at(path, alias.local)?.clone(),
                    context.entities.at(path, alias.source)?.clone(),
                ))
            })
            .collect::<Vec<_>>();
        loop {
            let mut grew = false;
            for (local, source) in &pairs {
                let (has_local, has_source) = (stores.contains(local), stores.contains(source));
                if has_local != has_source {
                    stores.push(if has_local {
                        source.clone()
                    } else {
                        local.clone()
                    });
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
    }
    stores
}

/// Whether the callee at `callee` (in `path`) is the setter destructured beside
/// one of `stores` (`const [store, setStore] = createStore(...)`),
/// directly or through exact `const` aliases of it (`const set = setStore`, at
/// most `depth` of them). Another store's setter or a same-spelled local is not.
fn is_own_setter(
    context: &UpstreamCompatContext<'_>,
    file: &solid_facts::FileFacts,
    callee: Span,
    stores: &[crate::SymbolId],
    depth: usize,
) -> bool {
    // ADR 0482: a member read that is exactly a `const` (ADR 0481) calls that
    // `const`: the setter slot of one of `stores`' patterns, or an alias of it.
    if let Some(alias) = file
        .ast
        .local_alias_member(file.ast.peel_ts_sugar_span(callee))
    {
        return setter_declaration_is_own(context, file, alias.source, stores, depth);
    }
    let Some((binding_file, binding, callee_symbol)) = context
        .lookup
        .binding_at_reference(file.path.as_str(), callee)
    else {
        return false;
    };
    match binding.shape {
        solid_facts::ast::BindingShape::Array => {
            let slot_symbol = |slot: usize| {
                binding
                    .array_slots
                    .get(slot)
                    .and_then(Option::as_ref)
                    .and_then(|name| context.entities.at(binding_file.path.as_str(), name.span))
            };
            slot_symbol(1) == Some(&callee_symbol)
                && slot_symbol(0).is_some_and(|store| stores.contains(store))
        }
        solid_facts::ast::BindingShape::Identifier if depth > 0 && binding.immutable => {
            binding
                .initializer_identifier
                .as_ref()
                .is_some_and(|source| {
                    is_own_setter(context, binding_file, source.span, stores, depth - 1)
                })
                || literal_alias_is_own_setter(context, binding_file, &callee_symbol, stores, depth)
        }
        _ if depth > 0 => {
            literal_alias_is_own_setter(context, binding_file, &callee_symbol, stores, depth)
        }
        _ => false,
    }
}

/// ADR 0480: whether the binding `symbol` is an exact local alias
/// (`local_alias_bindings`: a literal slot) of the setter destructured beside
/// one of `stores`, directly or through further such aliases.
fn literal_alias_is_own_setter(
    context: &UpstreamCompatContext<'_>,
    file: &solid_facts::FileFacts,
    symbol: &crate::SymbolId,
    stores: &[crate::SymbolId],
    depth: usize,
) -> bool {
    let path = file.path.as_str();
    let Some(alias) = file
        .ast
        .local_alias_bindings
        .iter()
        .find(|alias| context.entities.at(path, alias.local) == Some(symbol))
    else {
        return false;
    };
    setter_declaration_is_own(context, file, alias.source, stores, depth)
}

/// Whether the binding declared at `declaration` is the setter slot of one of
/// `stores`' `createStore` patterns, or an exact local alias of one.
fn setter_declaration_is_own(
    context: &UpstreamCompatContext<'_>,
    file: &solid_facts::FileFacts,
    declaration: Span,
    stores: &[crate::SymbolId],
    depth: usize,
) -> bool {
    let path = file.path.as_str();
    if let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding.shape == solid_facts::ast::BindingShape::Array
            && binding
                .array_slots
                .get(1)
                .and_then(Option::as_ref)
                .is_some_and(|name| name.span == declaration)
    }) {
        return binding
            .array_slots
            .first()
            .and_then(Option::as_ref)
            .and_then(|name| context.entities.at(path, name.span))
            .is_some_and(|store| stores.contains(store));
    }
    let Some(source) = context.entities.at(path, declaration) else {
        return false;
    };
    depth > 0 && literal_alias_is_own_setter(context, file, source, stores, depth - 1)
}

/// Where the site at `span` (a store write, or the call of a function that
/// writes) stands against `windows` (ADR 0476).
///
/// The site is climbed out through the layers that run in place
/// ([`crate::execution_role::layer_run`]: an IIFE, an inline standard-library
/// callback, a helper's callback it calls in its own body). Where the climb
/// stops:
///
/// - in a direct window, with no await before the site on the way and not in
///   a parameter list: the site runs on the window's own stack, so the write
///   is exempt; after an await on every path it runs once the window has
///   closed (dropped), after a possible one it is unproven;
/// - in a function written inside a window that a reviewed scheduler runs on
///   a fresh stack (`setTimeout(fn)`, `queueMicrotask(fn)`, `.then(fn)`,
///   [`crate::execution_role::LayerRun::FreshStack`]): the window has closed,
///   so the write is dropped;
/// - in a named function: the windows of the calls that run it
///   ([`named_function_standing`]);
/// - anywhere else in a window: unproven; outside every window: dropped.
fn write_window_standing(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    windows: &Windows,
    span: Span,
    depth: usize,
) -> WriteStanding {
    // Climb out through the layers that run in place ([`crate::execution_role::layer_run`]),
    // up to a window. Unlike `climb_in_place_layers`, a layer called inside a
    // JSX expression is crossed: inside a window function it runs on that
    // function's stack, whatever JSX the function itself is written in.
    let mut at = span;
    let mut suspended: Option<bool> = None;
    let mut in_list = false;
    let mut owner = None;
    for _ in 0..16 {
        let Some(function) = crate::owners::owning_function(&file.ast, at) else {
            break;
        };
        owner = Some(function);
        if function.generator
            || windows
                .regions
                .iter()
                .any(|window| window.direct && window.function == function.span)
        {
            break;
        }
        let inside = windows
            .regions
            .iter()
            .any(|window| window.function.contains(function.span));
        match crate::execution_role::layer_run(file, function, context.lookup) {
            // A layer called in JSX written inside the window runs where the
            // compiler places that expression (a render effect, after the
            // setter returned); only JSX around the whole window is crossed.
            crate::execution_role::LayerRun::InPlace(call)
                if inside
                    && file.ast.jsx_containing(call).any(|element| {
                        !windows.regions.iter().any(|window| {
                            window.direct
                                && window.function.contains(function.span)
                                && element.span.contains(window.function)
                        })
                    }) =>
            {
                return WriteStanding::Unproven;
            }
            crate::execution_role::LayerRun::InPlace(call) => {
                in_list |= !function.body.contains(at);
                if function.r#async {
                    if crate::execution_role::follows_unconditional_await(file, at) {
                        suspended = Some(true);
                    } else if crate::execution_role::may_follow_own_await(file, at) {
                        suspended.get_or_insert(false);
                    }
                }
                at = call;
            }
            crate::execution_role::LayerRun::InPlaceAmongOthers(_) if inside => {
                return WriteStanding::Unproven;
            }
            crate::execution_role::LayerRun::FreshStack if inside => return WriteStanding::Dropped,
            _ => break,
        }
    }
    let Some(owner) = owner else {
        return WriteStanding::Dropped;
    };
    if windows
        .regions
        .iter()
        .any(|window| window.direct && window.function == owner.span)
    {
        // An async window (a compute function `async () => { await …; }`)
        // closes at its first await.
        if owner.r#async {
            if crate::execution_role::follows_unconditional_await(file, at) {
                suspended = Some(true);
            } else if crate::execution_role::may_follow_own_await(file, at) {
                suspended.get_or_insert(false);
            }
        }
        return if in_list || !owner.body.contains(at) {
            WriteStanding::Unproven
        } else {
            match suspended {
                None => WriteStanding::Exempt,
                Some(true) => WriteStanding::Dropped,
                Some(false) => WriteStanding::Unproven,
            }
        };
    }
    let in_window = windows
        .regions
        .iter()
        .any(|window| window.function.contains(owner.span));
    if in_window
        && matches!(
            crate::execution_role::layer_run(file, owner, context.lookup),
            crate::execution_role::LayerRun::FreshStack
        )
    {
        return WriteStanding::Dropped;
    }
    match named_function_standing(file, context, windows, owner, depth) {
        Some(standing) => standing,
        None if in_window || called_in_window(file, context, windows, owner) => {
            WriteStanding::Unproven
        }
        None => WriteStanding::Dropped,
    }
}

/// Whether a call written in a window may run the project function `function`,
/// so that one of its runs is in the window and a write in it is not proven
/// dropped, whatever its references show:
///
/// - the call resolves, exactly, to it
///   ([`crate::execution_role::project_callee_at_parameter`], the resolution
///   the write rules take);
/// - `function` is a method, which has no binding whose references list its
///   calls, and the call is through a member that resolves to no project
///   function (`api.run()` where the receiver's resolution is withheld, ADR
///   0299): it may be this method.
fn called_in_window(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    windows: &Windows,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    let method = function.method_name.is_some();
    file.ast.calls.iter().any(|call| {
        if !windows
            .regions
            .iter()
            .any(|window| window.function.contains(call.span))
        {
            return false;
        }
        match crate::execution_role::project_callee_at_parameter(file, call, 0, context.lookup) {
            Some((callee_file, callee, _)) => {
                callee_file.path == file.path && callee.span == function.span
            }
            None => {
                method
                    && file
                        .ast
                        .member_at(file.ast.peel_ts_sugar_span(call.callee))
                        .is_some()
            }
        }
    })
}

/// The standing of a write in the named function `function`, from the calls
/// that run it: `None` when every call of it is outside every window, or it
/// has none (nothing to say; the caller keeps its own answer). Callers deeper
/// than [`WINDOW_DEPTH`] are unknown.
///
/// Every reference to its binding must be its declaration, a type query or
/// the callee of a call (by name, or as a method), or the store setter's own
/// argument; a reference that passes the function on, an export, or one in
/// another file may run it anywhere, so with a call in a window the write is
/// unproven. Each call is placed by [`write_window_standing`]: one
/// call with no window open drops the write on that run, every call in a
/// window exempts it, anything else leaves it unproven. An async function's
/// body may run past an await, so it proves no exemption; nor does a
/// generator, whose body runs when it is iterated.
fn named_function_standing(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    windows: &Windows,
    function: &solid_facts::ast::FunctionFact,
    depth: usize,
) -> Option<WriteStanding> {
    if function.generator {
        return None;
    }
    // The chain of callers is longer than this follows: unknown, not dropped.
    if depth == 0 {
        return Some(WriteStanding::Unproven);
    }
    let name = crate::owners::function_binding_name(file, function)?;
    let path = file.path.as_str();
    let symbol = context.lookup.entities().at(path, name.span)?;
    // A binding that may hold another function by the time a call runs (an
    // object's method, a reassignable `let`) proves no exemption.
    let current = context.lookup.function_value_is_current(file, function);
    let references = context
        .lookup
        .symbol_references_if_present(symbol.as_str())?;
    let mut handed = context.lookup.hands_out_by_name(path, function.span)
        || crate::execution_role::module_exports(file, function, context.lookup);
    let mut standings = Vec::new();
    for reference in &references {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            handed = true;
            continue;
        };
        let at = Span::new(start, end);
        if reference.path.as_ref() != path {
            handed = true;
            continue;
        }
        if at == name.span || file.ast.type_queries.iter().any(|query| query.contains(at)) {
            continue;
        }
        // The function named as the setter's own argument (`setStore(bump)`):
        // the setter invokes it while its window is open.
        if windows.arguments.contains(&at) {
            standings.push(WriteStanding::Exempt);
            continue;
        }
        // A call of it: by its name, or as a method (`api.run()`, whose
        // reference is the member's property).
        match file.ast.calls.iter().find(|call| {
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            callee == at
                || file
                    .ast
                    .member_at(callee)
                    .is_some_and(|member| member.property == at)
        }) {
            Some(call) => standings.push(write_window_standing(
                file,
                context,
                windows,
                call.span,
                depth - 1,
            )),
            None => handed = true,
        }
    }
    // No run of it is in a window, or unknown: nothing to say here.
    if standings
        .iter()
        .all(|standing| *standing == WriteStanding::Dropped)
    {
        return None;
    }
    Some(if handed || function.r#async || !current {
        WriteStanding::Unproven
    } else if standings.contains(&WriteStanding::Dropped) {
        WriteStanding::Dropped
    } else if standings
        .iter()
        .all(|standing| *standing == WriteStanding::Exempt)
    {
        WriteStanding::Exempt
    } else {
        WriteStanding::Unproven
    })
}

/// What the links a store write's member chain reads past its root
/// ([`solid_facts::ast::AstFacts::member_chain_links`] of the written member
/// `member`) leave of its proof (ADR 0465, shared with `delete`, ADR 0461). `None`: the
/// rule is silent. `Some(true)`: uncertifiable. `Some(false)`: the write's
/// proof stands.
///
/// - No link past the root: the proof stands. `createStore` wraps its initial
///   value whatever it is (rc.17 `dist/dev.js:5904-5915`, `wrap` never asks
///   `isWrappable`), so the root is always the read proxy.
/// - The program applies `strictNullChecks` (Type Facts' table answer), and a
///   link's primitive value domain may hold `null` or `undefined`, in a chain
///   with no optional access, in a file `tsc` checks: TS18047/TS18048, so the
///   write is TypeScript's and the rule is silent.
/// - Any other link leaves the write uncertifiable (ADR 0476). The proxy serves
///   a child through `isWrappable` (`dist/dev.js:2272-2283`, `:5569`, `:5605`),
///   which refuses a frozen object, a value whose tag is not `Object` (a `Map`,
///   a `Date`; a class instance tagged `Object` is wrapped) and a DOM node, and
///   serves it raw: a write below lands. Whether
///   a link's value is frozen or plain is a property of the value, not of its
///   type (`Object.freeze` can reach any reference, `Readonly<T>` is assignable
///   to `T`), so no fact proves the link is wrapped. An undefined link throws
///   `TypeError` before any write, and a frozen array's mutator throws too.
///   Except (ADR 0491) where [`links_proven_wrappable`] holds.
fn chain_links_verdict(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    member: Span,
    root: Span,
) -> Option<bool> {
    let ast = &file.ast;
    let member = ast.peel_ts_sugar_span(member);
    let mut links = ast.member_chain_links(member);
    // ADR 0482: a member read that is exactly a `const` (ADR 0481) is the root
    // (`super::member_root`), so it and the links inside it are not past it.
    if let Some(root) = links
        .iter()
        .map(|&link| ast.peel_ts_sugar_span(link))
        .find(|&link| ast.local_alias_member(link).is_some())
    {
        links.retain(|&link| !root.contains(ast.peel_ts_sugar_span(link)));
    }
    if links.is_empty() {
        return Some(false);
    }
    if context.strict_null_checks != Some(true) || !tsc_reports_in(file) {
        return Some(true);
    }
    let path = file.path.as_str();
    // An optional access anywhere in the chain (`store.list?.push(1)`,
    // `store.deep?.list.push(1)`) short-circuits it, so tsc reports no
    // nullish link of it.
    let optional = std::iter::once(member)
        .chain(links.iter().map(|&link| ast.peel_ts_sugar_span(link)))
        .any(|access| ast.optional_members.binary_search(&access).is_ok());
    let nullish = links.iter().any(|&link| {
        context
            .lookup
            .entity_at(path, link)
            .and_then(|entity| entity.primitive_value_domain.present())
            .is_some_and(|domain| {
                !domain.unknown() && (domain.may_be_null() || domain.may_be_undefined())
            })
    });
    if nullish && !optional {
        return None;
    }
    Some(
        !(optional_free(ast, member, &links)
            && links_proven_wrappable(file, context, member, root)),
    )
}

/// No link of the chain, nor the written member, is an optional access.
fn optional_free(ast: &solid_facts::ast::AstFacts, member: Span, links: &[Span]) -> bool {
    std::iter::once(member)
        .chain(links.iter().map(|&link| ast.peel_ts_sugar_span(link)))
        .all(|access| ast.optional_members.binary_search(&access).is_err())
}

/// The in-place array methods [`links_proven_wrappable`] lets a store path
/// call: [`ARRAY_MUTATORS`] less `sort`, whose comparator receives elements.
const CALLBACK_FREE_ARRAY_MUTATORS: [&str; 8] = [
    "push",
    "pop",
    "shift",
    "unshift",
    "splice",
    "reverse",
    "fill",
    "copyWithin",
];

/// ADR 0491: every link the write at `member` reads past the store root holds
/// the object or array literal the store's own initializer wrote there, so the
/// proxy serves it wrapped (`isWrappable`, rc.17 `dist/dev.js:2272-2283`: a
/// fresh literal is neither frozen nor of a foreign prototype) and a write
/// below it is dropped like a write to the root. Proven only where nothing can
/// replace or reach that raw value:
/// - the root resolves, through exact `const` aliases, to slot 0 of a `const`
///   array pattern in a function body whose initializer is `createStore` with
///   one argument, a closed object literal (no spread, computed, duplicate,
///   accessor or `__proto__` key), so the store is deep and its value inline;
/// - the setter slot is absent or never referenced, so no write lands and no
///   value replaces a link (a write outside the setter is dropped);
/// - every reference to the store or an alias is an alias's initializer or the
///   root of a static, non-optional member path that is written, deleted,
///   called as a callback-free array mutator on an array literal, or read
///   where the literal holds a primitive: no proxy of the store or below it
///   leaves as a value, so nothing reaches the raw value (`$TARGET`), freezes
///   or marks it.
///
/// Each link's key is a data property of the literal the previous key named;
/// an array literal ends the walk (its elements have no static key).
fn links_proven_wrappable(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    member: Span,
    root: Span,
) -> bool {
    let ast = &file.ast;
    let Some(root_declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(root)) else {
        return false;
    };
    // The store pattern and its exact `const` aliases, by declared name span.
    let mut names = vec![root_declaration];
    let mut store = None;
    for _ in 0..4 {
        let declaration = *names.last().expect("one name");
        let Some(binding) = ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))
        else {
            return false;
        };
        if !binding.immutable {
            return false;
        }
        match binding.shape {
            solid_facts::ast::BindingShape::Identifier => {
                let Some(source) = binding
                    .initializer_identifier
                    .as_ref()
                    .and_then(|source| ast.reference_declaration(source.span))
                else {
                    return false;
                };
                names.push(source);
            }
            solid_facts::ast::BindingShape::Array => {
                store = Some(binding);
                break;
            }
            _ => return false,
        }
    }
    let Some(store) = store else {
        return false;
    };
    let slot = |index: usize| store.array_slots.get(index).and_then(Option::as_ref);
    if slot(0).map(|name| name.span) != names.last().copied()
        || store.array_slots.len() > 2
        || file
            .source_text(store.pattern)
            .is_none_or(|pattern| pattern.contains("..."))
        || !ast.any_function_body_containing(store.declaration)
    {
        return false;
    }
    let primitive = context
        .entities
        .at(file.path.as_str(), slot(0).expect("slot 0").span)
        .and_then(|symbol| context.source_primitives.get(symbol));
    let Some(call) = store.call_initializer.and_then(|call| ast.call_at(call)) else {
        return false;
    };
    let [argument] = call.arguments.as_slice() else {
        return false;
    };
    let initial = ast.peel_ts_sugar_span(argument.span);
    if primitive.is_none_or(|primitive| primitive.as_str() != "createStore")
        || ast.peel_ts_sugar_span(store.initializer.unwrap_or(call.span)) != call.span
        || closed_literal(ast, initial).is_none()
    {
        return false;
    }
    let referenced = |declaration: Span| {
        ast.identifiers.iter().any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && ast.reference_declaration(identifier.span) == Some(declaration)
        })
    };
    if slot(1).is_some_and(|setter| referenced(setter.span)) {
        return false;
    }
    // Alias initializers are the references the alias walk already took.
    let alias_sources: Vec<Span> = ast
        .bindings
        .iter()
        .filter(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.iter().any(|name| names.contains(&name.span))
        })
        .filter_map(|binding| {
            binding
                .initializer_identifier
                .as_ref()
                .map(|source| source.span)
        })
        .collect();
    for identifier in &ast.identifiers {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference
            || !ast
                .reference_declaration(identifier.span)
                .is_some_and(|declaration| names.contains(&declaration))
            || alias_sources.contains(&identifier.span)
        {
            continue;
        }
        if !contained_store_use(file, identifier.span, initial) {
            return false;
        }
    }
    // The written path's links resolve to literals (the use check above
    // already walked it; this pins the links themselves).
    let Some(path) = static_path(
        file,
        ast.peel_ts_sugar_span(member),
        ast.peel_ts_sugar_span(root),
    ) else {
        return false;
    };
    path.len() >= 2 && resolve_literal_path(file, initial, &path[..path.len() - 1]).is_some()
}

/// What a literal value resolved from a store's initializer is.
#[derive(Clone, Copy, Eq, PartialEq)]
enum LiteralValue {
    Object(Span),
    Array,
    Primitive,
}

/// The closed object literal at exactly `span`.
fn closed_literal(ast: &solid_facts::ast::AstFacts, span: Span) -> Option<Span> {
    ast.object_get_shapes
        .iter()
        .find(|shape| shape.span == span)
        .filter(|shape| {
            shape.closed
                && shape
                    .properties
                    .iter()
                    .all(|property| property.getter.is_none())
        })
        .map(|shape| shape.span)
}

/// The value the closed literal `literal` holds at `path`, one static key per
/// step; an array literal or a primitive ends it.
fn resolve_literal_path(file: &FileFacts, literal: Span, path: &[Span]) -> Option<LiteralValue> {
    let ast = &file.ast;
    let mut value = LiteralValue::Object(literal);
    for &key in path {
        let LiteralValue::Object(object) = value else {
            return None;
        };
        let name = file.source_text(key)?;
        let property = ast.object_properties.iter().find(|property| {
            object.contains(property.span)
                && !property.computed
                && property.data
                && file.source_text(property.key) == Some(name)
                && !ast.object_get_shapes.iter().any(|shape| {
                    shape.span != object
                        && object.contains(shape.span)
                        && shape.span.contains(property.span)
                })
        })?;
        let inner = ast.peel_ts_sugar_span(property.value);
        value = match property.value_kind {
            solid_facts::ast::RuntimeValueKind::Object => {
                LiteralValue::Object(closed_literal(ast, inner)?)
            }
            solid_facts::ast::RuntimeValueKind::Array
                if ast.array_literals.binary_search(&inner).is_ok() =>
            {
                LiteralValue::Array
            }
            solid_facts::ast::RuntimeValueKind::Primitive => LiteralValue::Primitive,
            _ => return None,
        };
    }
    Some(value)
}

/// The static, non-optional member path rooted at `root` that ends at
/// `member`: each step's property span, outermost last.
fn static_path(file: &FileFacts, member: Span, root: Span) -> Option<Vec<Span>> {
    let ast = &file.ast;
    let mut path = Vec::new();
    let mut current = member;
    while current != root {
        let access = ast.member_at(current)?;
        if ast.computed_members.binary_search(&current).is_ok()
            || ast.optional_members.binary_search(&current).is_ok()
        {
            return None;
        }
        path.push(access.property);
        current = ast.peel_ts_sugar_span(access.object);
    }
    path.reverse();
    Some(path)
}

/// The reference `reference` to a store whose initializer is the closed
/// literal `initial` is the root of a static member path that is written,
/// deleted, called as a callback-free array mutator on an array literal, or
/// read where the literal holds a primitive.
fn contained_store_use(file: &FileFacts, reference: Span, initial: Span) -> bool {
    let ast = &file.ast;
    let mut top = reference;
    while let Some(parent) = ast.members.iter().find(|parent| {
        ast.peel_ts_sugar_span(parent.object) == top
            && ast.computed_members.binary_search(&parent.span).is_err()
            && ast.optional_members.binary_search(&parent.span).is_err()
    }) {
        top = parent.span;
    }
    if top == reference {
        return false;
    }
    let Some(path) = static_path(file, top, reference) else {
        return false;
    };
    let (last, prefix) = path.split_last().expect("one step");
    let written = ast.write_targets.binary_search(&top).is_ok()
        || ast
            .deleted_targets
            .iter()
            .any(|&target| file.peel_parentheses(target) == Some(top));
    if written {
        return resolve_literal_path(file, initial, prefix).is_some();
    }
    let called = ast
        .calls
        .iter()
        .any(|call| !call.construct && ast.peel_ts_sugar_span(call.callee) == top);
    if called {
        return file
            .source_text(*last)
            .is_some_and(|name| CALLBACK_FREE_ARRAY_MUTATORS.contains(&name))
            && resolve_literal_path(file, initial, prefix) == Some(LiteralValue::Array);
    }
    resolve_literal_path(file, initial, &path) == Some(LiteralValue::Primitive)
}

/// Whether `tsc` reports a type error written in `file` at all: not a
/// JavaScript file (whether `checkJs` applies is not in the facts) and no
/// comment reading as `@ts-nocheck`, `@ts-ignore` or `@ts-expect-error`
/// (`AstFacts::typescript_directive_comment`, from the parser's comments; a
/// conservative veto: a directive anywhere withholds the premise for the
/// whole file).
fn tsc_reports_in(file: &FileFacts) -> bool {
    let path = file.path.as_str();
    let javascript = [".js", ".jsx", ".mjs", ".cjs"]
        .iter()
        .any(|extension| path.ends_with(extension));
    !javascript && !file.ast.typescript_directive_comment
}

/// The nine in-place `Array.prototype` methods upstream names
/// (`MUTATING_METHODS`).
const ARRAY_MUTATORS: [&str; 9] = [
    "push",
    "pop",
    "shift",
    "unshift",
    "splice",
    "sort",
    "reverse",
    "fill",
    "copyWithin",
];

/// A call of an in-place array method on a path of a proven store: the call
/// resolves, through Type Facts, to the standard library's `Array` method, so
/// a project method of the same name, a `ReadonlyArray`, a typed array and an
/// unresolved receiver are all silent. TypeScript accepts the call (`Store<T>
/// = T` from `@solidjs/signals` rc.7, and a nested array is mutable on every
/// release), and the runtime drops it: every element write and the `length`
/// write reach the proxy's `set` trap, which returns `true` without storing
/// (rc.17 `dist/dev.js:5691-5700`, ADR 0425).
fn no_store_mutator_call(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for call in &file.ast.calls {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let Some(member) = file.ast.member_at(callee) else {
            continue;
        };
        if call.construct
            || file.ast.computed_members.binary_search(&callee).is_ok()
            || !ARRAY_MUTATORS.contains(&text(file, member.property))
        {
            continue;
        }
        let Some(declaration) =
            context
                .lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
        else {
            continue;
        };
        if declaration.qualified_name.as_ref() != format!("Array.{}", text(file, member.property)) {
            continue;
        }
        let Some(root) = member_root(file, callee) else {
            continue;
        };
        let Some(symbol) = source_symbol_at(context, file, root) else {
            continue;
        };
        if context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store) {
            continue;
        }
        let Some((name, _)) = context.accessors.get(symbol) else {
            continue;
        };
        // A member chain rooted at a call writes to what the call returned
        // (`store.items.slice().push(1)` is a plain copy), and a root that is
        // not a bare identifier is not the proxy.
        if file
            .ast
            .call_at(file.ast.peel_ts_sugar_span(root))
            .is_some()
            || !bare_root(file, root)
        {
            continue;
        }
        let standing = store_write_standing(
            file,
            context,
            StoreWriteSite {
                span: call.span,
                member: callee,
                root,
            },
            symbol,
        );
        if standing == WriteStanding::Exempt {
            continue;
        }
        // ADR 0465: the receiver of `store.items.push(1)` is a link past the
        // root; only a store that is itself the array (`list.push(1)`) has none.
        let Some(past_root) = chain_links_verdict(file, context, callee, root) else {
            continue;
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::DirectMutation {
                name: name.as_str().to_owned(),
                target: store_target(context, symbol),
            },
            location: location(file.path.shared(), call.span),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: past_root || standing == WriteStanding::Unproven,
        });
    }
}

/// `delete` of a static member path of a proven store (ADR 0461): the proxy's
/// `deleteProperty` trap drops it outside a setter like the `set` trap drops an
/// assignment (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:5761-5767`), with
/// the same exemptions ([`store_write_standing`]).
///
/// The rule never reports what TypeScript reports on the same delete:
/// - the operand must be a bare or parenthesized member: `delete store.v!` and
///   `delete (store.v as T)` are TS2703, so a type assertion is silent;
/// - its property must carry Type Facts' `writable_optional_property`
///   (optional, not readonly): a required one is TS2790 under
///   strictNullChecks, a readonly one TS2704;
/// - the links the chain reads past the root (`store.a` in `delete
///   store.a.b`) get the verdict the assignment and mutator forms get
///   ([`chain_links_verdict`], ADR 0465): silent on a link TypeScript reports
///   as possibly nullish, a violation under strictNullChecks when every link
///   excludes `null` and `undefined`, uncertifiable otherwise.
///
/// A proven store delete with no entity at the operand is uncertifiable: the
/// write is proven, the TypeScript question is open. A computed key, an optional
/// chain, a root that is not a bare identifier and a props object stay silent.
fn no_store_delete(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for &target in &file.ast.deleted_targets {
        let Some(member) = file.peel_parentheses(target) else {
            continue;
        };
        if file.ast.member_at(member).is_none()
            || file.ast.computed_members.binary_search(&member).is_ok()
        {
            continue;
        }
        let Some(root) = member_root(file, member) else {
            continue;
        };
        // Every link from the operand down to the root is a plain member read:
        // no `?.` (which may skip the delete) and no wrapper between links.
        // The walk ends at the root `member_root` found: a member alias root
        // (ADR 0482) is itself a member, and its own links are not the chain's.
        let mut link = member;
        let mut plain = true;
        while link != root
            && let Some(current) = file.ast.member_at(link)
        {
            if file.ast.optional_members.binary_search(&link).is_ok() {
                plain = false;
                break;
            }
            link = current.object;
        }
        if !plain || link != root || !bare_root(file, root) {
            continue;
        }
        let Some(symbol) = source_symbol_at(context, file, root) else {
            continue;
        };
        if context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store) {
            continue;
        }
        let Some((name, _)) = context.accessors.get(symbol) else {
            continue;
        };
        let standing = store_write_standing(
            file,
            context,
            StoreWriteSite {
                span: member,
                member,
                root,
            },
            symbol,
        );
        if standing == WriteStanding::Exempt {
            continue;
        }
        let path = file.path.as_str();
        let uncertain = match context.lookup.entity_at(path, member) {
            None => true,
            Some(entity) if !entity.writable_optional_property => continue,
            Some(_) => match chain_links_verdict(file, context, member, root) {
                None => continue,
                Some(past_root) => past_root,
            },
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::DirectMutation {
                name: name.as_str().to_owned(),
                target: store_target(context, symbol),
            },
            location: location(file.path.shared(), member),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: uncertain || standing == WriteStanding::Unproven,
        });
    }
}

/// The compute function that derives the store `store_symbol`: the first
/// argument of the `createProjection(fn)` or function-form `createStore(fn,
/// seed)` call its binding initializes.
///
/// The runtime runs that function inside `storeSetter(draft, …)`, which
/// write-enables the store's family for the run (`@solidjs/signals@2.0.0-rc.17`
/// `dist/dev.js:6675-6678`, `:5847-5866`); a write through the result symbol
/// from the re-run commits like the draft's (probed). A write it schedules
/// runs after the run and is dropped ([`write_window_standing`], ADR 0476).
/// The binding must be in this file: a write from another module is never
/// inside the call.
fn own_compute_functions(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    store_symbol: &crate::SymbolId,
) -> Vec<Span> {
    let Some((_, declaration)) = context.accessors.get(store_symbol) else {
        return Vec::new();
    };
    if declaration.path.as_ref() != file.path.as_str() {
        return Vec::new();
    }
    let declared = Span::new(
        u32::try_from(declaration.start_byte).unwrap_or(u32::MAX),
        u32::try_from(declaration.end_byte).unwrap_or(u32::MAX),
    );
    file.ast
        .bindings
        .iter()
        .filter(|binding| binding.names.iter().any(|name| name.span == declared))
        .filter_map(|binding| binding.call_initializer)
        .filter_map(|call| file.ast.call_at(call))
        .filter_map(|call| call.arguments.first())
        .filter_map(|argument| {
            let argument = file.ast.peel_ts_sugar_span(argument.span);
            file.ast
                .functions
                .iter()
                .find(|function| function.span == argument)
                .map(|function| function.span)
        })
        .collect()
}

/// Whether this identifier is the callee of a call — the correct use.
fn is_called(file: &FileFacts, span: Span) -> bool {
    file.ast.calls.iter().any(|call| call.callee == span)
}

/// Whether this identifier is being passed to a call, which hands the
/// receiver something it can call later and is how accessors travel.
fn is_argument(file: &FileFacts, span: Span) -> bool {
    file.ast
        .calls
        .iter()
        .any(|call| call.arguments.iter().any(|argument| argument.span == span))
}

/// The value position this reference sits in, phrased for the message, or
/// `None` when the position does not demand a value.
///
/// Deliberately narrow. Every arm is a position where a function object is
/// provably not what the author meant: string interpolation renders
/// `function () {...}`, a computed key stringifies it, and the rest is
/// upstream's own enumeration. Anywhere else — a bare reference, a return, a
/// property value — passing the accessor on is idiomatic, and reporting it
/// would be the false positive upstream's issue #193 describes.
fn value_position(file: &FileFacts, span: Span) -> Option<&'static str> {
    if let Some(operand) = file
        .ast
        .coercive_operands
        .iter()
        .find(|operand| operand.span == span)
    {
        return Some(match operand.kind {
            solid_facts::ast::CoerciveOperandKind::StringConcatenation => "a string concatenation",
            solid_facts::ast::CoerciveOperandKind::LogicalNot => "a logical-not operator",
            solid_facts::ast::CoerciveOperandKind::NumericCoercion => "a numeric coercion",
        });
    }

    // Narrowed 2026-08-17 under AGENTS.md's absolute rule. Three of this
    // rule's six positions are ones the type system closes, in *both*
    // dialects, so they are gone:
    //
    //   a native JSX attribute       `<div title={label} />` is TS2322 --
    //                                `Accessor<string>` is not assignable to
    //                                `string` (1.x) or `string |
    //                                RemoveAttribute` (2.0);
    //   a class object value         `<div class={{ active: count }} />` is
    //                                TS2322 against `Record<string, boolean>`
    //                                in 2.0, the only dialect where the
    //                                position was enabled;
    //   a computed property access   `table[label]` is TS2538, "cannot be used
    //                                as an index type".
    //
    // What remains are the positions where an accessor is *legal* to
    // TypeScript and silently wrong at runtime -- see the returns below. This
    // also removed the last consumers of the dialect's
    // `class_object_values_are_truthiness_coerced` and
    // `native_children_attribute_invokes_functions` predicates, which went with
    // them.
    //
    // The native-attribute position has one surviving shape, and it is the
    // boundary of the argument above: TypeScript does not check a JSX attribute
    // name containing a hyphen at all (a deliberate exemption for HTML's own
    // hyphenated attributes), so `<div data-count={count} />` is accepted and
    // the accessor is stringified into the attribute with nothing to say so.
    if file.ast.jsx_elements.iter().any(|element| {
        is_lowercase_led(text(file, element.name.span))
            && element.attributes.iter().any(|attribute| {
                attribute.namespace.is_none()
                    && attribute.expression == Some(span)
                    && !super::jsx_name_is_type_checked(text(file, attribute.name))
            })
    }) {
        return Some("a native JSX attribute");
    }
    // An untagged template literal stringifies each interpolation, so an
    // accessor there renders its own source text. A *tagged* one hands the
    // interpolations to the tag as values, which may legitimately call
    // them. The fact table records only untagged literals — a tag's quasi
    // is filtered out at collection — so `css`color: ${theme}`` stays out
    // of this rule while an untagged template nested inside a tagged
    // interpolation still reports.
    if file
        .ast
        .template_literals
        .iter()
        .any(|template| template.expressions.contains(&span))
    {
        return Some("a template literal");
    }
    // The third position the narrowing above removed: a computed key is
    // TS2538 ("Type 'Accessor<string>' cannot be used as an index type") in
    // both dialects, so it never reaches this function's callers any more.
    None
}
