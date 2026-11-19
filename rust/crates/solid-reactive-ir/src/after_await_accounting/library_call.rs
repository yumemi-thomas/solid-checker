//! ADR 0976: the typed fact that ends a call into code outside the project (the standard library, a setter): it runs no project code.
//!
//! A call after an async function's own suspension that is no project function used to leave the accounting pass through one skipping arm, on the argument that a library "runs
//! project code only through what it is handed or a protocol the project's objects implement". The reviews of #233 found each way that holds: a callback the library runs on this
//! stack (`[1].forEach(() => source())`, a setter's updater), a listener registered earlier that a synchronous dispatch delivers to (`el.click()`), a receiver that is the project's
//! own object under a library type (a stored generator, an iterator literal, a `Map` subclass), and a standard call a veto withholds that the symbol fallback let through. So the
//! arm is gone: such a call is a site, and it ends only when every one of these is shut by a fact of the program, never by a name, a type or a package's name. A call into a package
//! is a site that nothing ends (the owner ruled FULL over a narrower scope: the package-source proof, the proof batch's 1b, would win those rows back).
//!
//! - **the program leaves every built-in alone**: no assignment writes a standard global or a member of a standard prototype or constructor, no reflective definer is spelled, no
//!   call is a reflective mutator, nothing writes through a `prototype`, no code is evaluated from a string. A standard call reaches other built-ins (`Promise.all("ab")` calls
//!   `Promise.resolve`, `new Set("ab")` calls `add`, `"abc".match("a")` looks up `Symbol.match`, `JSON.stringify({ a: 1 })` looks up `toJSON` on `Object.prototype`, `Object.assign`
//!   sets through an inherited setter), so one patched built-in anywhere takes the proof from every standard call (Research B's programs `aa-promise-all-resolve`,
//!   `aa-set-add`, `aa-string-iterator`, `aa-objproto-tojson`, `aa-objassign-setter`);
//! - **the callee resolves exactly**: the compiler's selected declaration is the standard library's and no census withholds it ([`SemanticLookup::resolved_callee_call`] answers
//!   `None` for a subclass, a patch, an impostor or an unproven assertion: ADRs 0683, 0781, 0840, 0876), so a call with no resolved-call fact, a package's declaration or a symbol
//!   found only by its declarations never ends here. `eval` and the `Function` constructor run code from a string, whatever their arguments;
//! - **it delivers to no listener on this stack**: [`crate::runtime_semantics::delivers_synchronously`], by the declaration's qualified name;
//! - **its receiver is the library's own value**: none (a global function), a primitive proven from its own expression, or an ECMAScript global the program does not shadow (the
//!   declarations of its symbol lie in `lib.es*.d.ts`: `Math`, `JSON`, `Object`, `Promise`, `Reflect`). A value that only has a library type (`const it: Iterator<number>`, `m:
//!   Map<K, V>`) is not proven: its methods may be the project's;
//! - **no argument can carry project code onto this stack**: each is a nullish literal, a primitive proven from its own expression, an exact data literal (an object literal of
//!   plain data properties all of whose values are, recursively), or a *function* handed to a slot the audited table says runs later (`setTimeout`'s callback, a registration),
//!   where the run is another stack's and the dispatch that reaches it is itself a site. A string at such a slot is code the host evaluates (`setTimeout("…")`), and a value no
//!   fact proves may be one: neither is inert. A spread, an identifier whose value no fact proves, a function that runs here and an object whose getters, `toString` or iterator
//!   may be the project's are not.
//!
//! A setter has no standard declaration: the same argument rule is its proof, with one more fact, that the signal it sets has no function-valued `equals` option (`createSignal(0, {
//! equals: (a, b) => … })` runs that function on every set: Research B's `aa-setter-equals`), read from the exact `createSignal` call the setter was destructured from. An action
//! is never inert: its call runs the function it was made of. An updater function handed to a setter is the other way project code gets in (Chrome rc.21 reports the read inside
//! `setCount((p) => p + source())`).
//!
//! Nothing here asks a type. A `string` known only from an annotation is not an inert argument, and an `Iterator` known only from one is not a built-in receiver.

use solid_facts::FileFacts;
use solid_facts::ast::{
    ArgumentFact, ArgumentLiteralFact, ArgumentValueKind, BindingShape, CallFact, RuntimeValueKind,
};

use crate::base::Hazard;
use crate::pipeline::AnalysisContext;
use crate::runtime_semantics::{self, RuntimeArgumentBehavior};

/// How far a binding is followed when a value is proven primitive: a `const` of a `const`.
const PROOF_DEPTH: usize = 4;

/// The census hazards that say the program touches a built-in or evaluates code from a string, so a standard call may reach a function the project put there: a write to a standard
/// global or a member of a standard prototype or constructor, a reflective definer spelled, a reflective mutator called, a write through a `prototype` no owner proof narrows or
/// through the prototype of a standard root, a direct `eval` or a `Function` constructor, and an array intrinsic or code obtained as a value.
const TOUCHED_BUILTINS: [Hazard; 8] = [
    Hazard::AnyStandardPatch,
    Hazard::ReflectiveDefinitionSpelling,
    Hazard::MutationBuiltinCall,
    Hazard::UnownedPrototypeWrite,
    Hazard::PrototypeOwnerSite,
    Hazard::Eval,
    Hazard::CodeEvaluatingCall,
    Hazard::ArrayIntrinsicObtained,
];

/// Whether the call at a site of `via` runs no project code: see the module docs. `via` is `setter`, `library` or `construct`; any other site has no such proof (an action runs
/// its function, a package's call has no exact fact).
pub(super) fn runs_nothing(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    via: &str,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    if !matches!(via, "setter" | "library" | "construct" | "inexact") || !builtins_untouched(ctx) {
        return false;
    }
    if via == "inexact" {
        return action_body_runs_nothing(ctx, file, call, object_prototype_touched);
    }
    if via == "setter" {
        setter_runs_nothing(ctx, file, call, object_prototype_touched)
    } else {
        standard_call_runs_nothing(ctx, file, call, object_prototype_touched)
    }
}

/// ADR 1065 part 2 (census only): the first conjunct of [`runs_nothing`] that fails, in its order. Asked after it answered false; never read by an answer.
pub(super) fn runs_nothing_failure(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    via: &str,
    object_prototype_touched: &dyn Fn() -> bool,
) -> String {
    if let Some(hazard) = TOUCHED_BUILTINS
        .into_iter()
        .find(|hazard| ctx.semantic_lookup.hazard_present(*hazard))
    {
        return format!("builtins-touched:{hazard:?}");
    }
    if via == "setter" {
        return "setter-unproven".to_owned();
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, callee) else {
        return "callee-unresolved".to_owned();
    };
    let Some(name) = runtime_semantics::standard_call_name(resolved) else {
        return "not-standard".to_owned();
    };
    if name == "eval"
        || name.starts_with("FunctionConstructor")
        || runtime_semantics::delivers_synchronously(name)
    {
        return format!("runs-code:{name}");
    }
    if package_writes_global(ctx, file, callee) {
        return format!("package-writes-global:{name}");
    }
    if !receiver_is_the_librarys(ctx, file, call) {
        return format!("receiver-not-the-librarys:{name}");
    }
    call.arguments
        .iter()
        .enumerate()
        .find(|(index, argument)| {
            let behavior = runtime_semantics::argument_behavior(resolved, None, *index);
            !argument_is_inert(ctx, file, argument, behavior, object_prototype_touched)
        })
        .map_or_else(
            || format!("unnamed:{name}"),
            |(index, _)| format!("argument-not-inert:{name}:{index}"),
        )
}

/// Whether no analyzed file touches a built-in or evaluates code from a string ([`TOUCHED_BUILTINS`]).
pub(super) fn builtins_untouched(ctx: &AnalysisContext<'_>) -> bool {
    TOUCHED_BUILTINS
        .into_iter()
        .all(|hazard| !ctx.semantic_lookup.hazard_present(hazard))
}

/// Whether the exactly resolved standard call or construct runs no project code (the module docs).
fn standard_call_runs_nothing(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    // The selected declaration is the standard library's, and no census withholds it (a vetoed call has no resolved fact here).
    let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, callee) else {
        return false;
    };
    let Some(name) = runtime_semantics::standard_call_name(resolved) else {
        return false;
    };
    // Code from a string, a call that delivers to listeners registered earlier.
    if name == "eval"
        || name.starts_with("FunctionConstructor")
        || runtime_semantics::delivers_synchronously(name)
    {
        return false;
    }
    // ADR 0999: the declaration is the library's, but what runs is whatever holds the global at run time. A global an installed package the program loads writes
    // (`window.structuredClone = …`, by that name or under a key nothing decides) is that package's function, which may call what the program handed it earlier.
    if package_writes_global(ctx, file, callee) {
        return false;
    }
    receiver_is_the_librarys(ctx, file, call)
        && call.arguments.iter().enumerate().all(|(index, argument)| {
            let behavior = runtime_semantics::argument_behavior(resolved, None, index);
            argument_is_inert(ctx, file, argument, behavior, object_prototype_touched)
        })
}

/// ADR 0999: whether some installed package the program loads writes the global a call goes through: the callee's own name (`fetch(…)`), or the global its receiver
/// is (`JSON.parse(…)`, `window.fetch(…)`: the root of the member chain), or the method a package writes on a built-in's prototype (ADR 1108). Every writer counts, a package whose closure names no Solid package too: it runs no action,
/// but it may run a function of the project it was handed. A name no unbound identifier spells answers by the any-name writers alone.
fn package_writes_global(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    callee: solid_facts::core::Span,
) -> bool {
    let Some(reach) = ctx.package_reach else {
        return false;
    };
    if reach.global_writers.is_empty()
        && reach.global_writers_any.is_empty()
        && reach.prototype_writers.is_empty()
        && reach.prototype_writers_any.is_empty()
    {
        return false;
    }
    let ast = &file.ast;
    let mut names = Vec::new();
    let mut at = ast.peel_ts_sugar_span(callee);
    // ADR 1108: a method a package writes on a built-in's prototype is that package's function wherever the receiver inherits it (the callee's own member name).
    if let Some(member) = ast.member_at(at)
        && (!reach.prototype_writers_any.is_empty()
            || if ast.computed_members.contains(&member.span) {
                !reach.prototype_writers.is_empty()
            } else {
                file.source_text(member.property)
                    .is_some_and(|name| reach.prototype_writers.contains_key(name))
            })
    {
        return true;
    }
    while let Some(member) = ast.member_at(at) {
        if !ast.computed_members.contains(&member.span)
            && let Some(name) = file.source_text(member.property)
        {
            names.push(name);
        }
        at = ast.peel_ts_sugar_span(member.object);
    }
    if crate::owners::is_identifier_span(file, at)
        && ast.reference_declaration(at).is_none()
        && let Some(name) = file.source_text(at)
    {
        names.push(name);
    }
    !reach.global_writers_any.is_empty()
        || names
            .iter()
            .any(|name| reach.global_writers.contains_key(*name))
}

/// Whether a setter call runs no project code: its arguments are inert and the signal has no function-valued `equals` option. The callee is an identifier bound by a destructuring
/// `const [get, set] = createSignal(init, options)` (this slot, that exact primitive call), and the options are absent, or an exact literal whose `equals` is absent or `false`: an
/// options value no fact shows may hold a function.
fn setter_runs_nothing(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    use solid_dialect::OptionLiteral;
    let lookup = ctx.semantic_lookup;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let (binding_file, binding) = if crate::owners::is_identifier_span(file, callee) {
        let Some((binding_file, binding, _)) =
            lookup.binding_at_reference(file.path.as_str(), callee)
        else {
            return false;
        };
        let Some(setter) = binding.array_slots.get(1).and_then(Option::as_ref) else {
            return false;
        };
        if binding_file.path != file.path
            || !binding.immutable
            || binding.shape != BindingShape::Array
            || file.ast.reference_declaration(callee) != Some(setter.span)
        {
            return false;
        }
        (binding_file, binding)
    } else {
        // ADR 1098: a setter a value carries (`thing.useIt(7)`, the setter a hook's returned literal holds): the callee's symbol is a
        // setter of the table (the transport the write rules use), and its declaration is the setter slot of an exact
        // `const [get, set] = createSignal(…)` or `createStore(…)`, asked below as the plain callee's is (a store's only in its own
        // file).
        let Some((_, declared, _, _)) = crate::static_rules::solid_call_symbol(ctx, file, call)
            .and_then(|symbol| ctx.setters.get(symbol))
        else {
            return false;
        };
        // The binder's own answer at the declaration (a recorded read of the declaring file), and its setter slot is that name.
        let (Ok(start), Ok(end)) = (
            u32::try_from(declared.start_byte),
            u32::try_from(declared.end_byte),
        ) else {
            return false;
        };
        let at = solid_facts::core::Span::new(start, end);
        let Some((declaring, binding, _)) = lookup.binding_at_reference(declared.path.as_ref(), at)
        else {
            return false;
        };
        if !binding
            .array_slots
            .get(1)
            .and_then(Option::as_ref)
            .is_some_and(|slot| slot.span == at)
        {
            return false;
        }
        if !binding.immutable || binding.shape != BindingShape::Array {
            return false;
        }
        (declaring, binding)
    };
    let Some(created) = binding
        .initializer
        .map(|initializer| binding_file.ast.peel_ts_sugar_span(initializer))
        .and_then(|initializer| binding_file.ast.call_at(initializer))
    else {
        return false;
    };
    if created.construct {
        return false;
    }
    let Some(primitive) = lookup.primitive_at_call(binding_file, created.span) else {
        return false;
    };
    // The call is the dialect's signal creation, and its options argument is the dialect's.
    if let Some(options) = ctx.dialect.signal_creation_options(primitive) {
        return call.arguments.iter().all(|argument| {
            argument_is_inert(ctx, file, argument, None, object_prototype_touched)
        }) && matches!(
            crate::call_option_literal(binding_file, created, options, "equals"),
            OptionLiteral::Absent | OptionLiteral::False
        );
    }
    // ADR 1089: a plain store's setter runs the one function it is given, on this stack, and nothing else of the project's
    // (`Dialect::store_setter_calls_updater`). The store is made from a data literal written there (not the derived form a
    // function makes), and the function only writes literals under static keys of the store.
    let root = binding
        .array_slots
        .first()
        .and_then(Option::as_ref)
        .map(|slot| slot.span);
    let [initial] = created.arguments.as_slice() else {
        return false;
    };
    let [updater] = call.arguments.as_slice() else {
        return false;
    };
    // ADR 1098: the store's root and initial value are read where the setter is called: same file only.
    binding_file.path == file.path
        && ctx.dialect.store_setter_calls_updater(primitive)
        && !initial.spread
        && (initial.exact_object_literal
            || file
                .ast
                .array_literal_at(file.ast.peel_ts_sugar_span(initial.span))
                .is_some())
        && !updater.spread
        && updater_writes_literals_only(ctx, file, updater.span, root)
}

/// ADR 1089: whether `call`, with no argument, calls a `const` action (`crate::owners::named_action_generator`: no alias withheld, no write) whose
/// generator body runs no project code: the action's wrapper runs the body's first step on this stack, and this body has no `yield`, `await`, nested
/// function, class, JSX, load, member, assignment, `delete` or returned value, and every call in it is a setter [`setter_runs_nothing`] ends, called by
/// its name. Every name the body references is one of those callees.
fn action_body_runs_nothing(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    let ast = &file.ast;
    if call.construct || !call.arguments.is_empty() {
        return false;
    }
    let Some(generator) =
        crate::owners::named_action_generator(file, call.callee, ctx.semantic_lookup)
    else {
        return false;
    };
    let Some(function) = ast
        .functions
        .iter()
        .find(|function| function.span == generator)
    else {
        return false;
    };
    let body = function.body;
    let inside = |inner: solid_facts::core::Span| body.contains(inner);
    if function.r#async
        || ast.yields.iter().any(|yielded| inside(yielded.span))
        || ast.awaits.iter().any(|awaited| inside(*awaited))
        || ast
            .functions
            .iter()
            .any(|inner| inner.span != function.span && inside(inner.span))
        || ast.classes.iter().any(|class| inside(class.span))
        || ast.jsx_elements.iter().any(|element| inside(element.span))
        || ast.module_loads.iter().any(|load| inside(load.span))
        || ast.members.iter().any(|member| inside(member.span))
        || ast
            .assignments
            .iter()
            .any(|assignment| inside(assignment.target))
        || ast.deleted_targets.iter().any(|target| inside(*target))
        || ast
            .returns
            .iter()
            .any(|returned| inside(returned.span) && returned.argument.is_some())
    {
        return false;
    }
    let calls = ast
        .calls
        .iter()
        .filter(|inner| inside(inner.span))
        .collect::<Vec<_>>();
    let callees = calls
        .iter()
        .map(|inner| ast.peel_ts_sugar_span(inner.callee))
        .collect::<Vec<_>>();
    calls.iter().all(|inner| {
        !inner.construct && setter_runs_nothing(ctx, file, inner, object_prototype_touched)
    }) && ast
        .identifiers
        .iter()
        .filter(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && inside(identifier.span)
        })
        .all(|identifier| callees.contains(&identifier.span))
}

/// ADR 1089: whether `span` is a function written there whose whole body writes primitive literals under static keys of its
/// one parameter or of the store root `root`, and does nothing else: no call or construct, no nested function, class, JSX
/// or load, no `return`, every member expression one of those assignment targets (a plain `=`), and every name it
/// references the object of one. Not async, not a generator, its parameter a plain name whose list runs no code.
fn updater_writes_literals_only(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    span: solid_facts::core::Span,
    root: Option<solid_facts::core::Span>,
) -> bool {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    let Some(function) = ast.functions.iter().find(|function| function.span == span) else {
        return false;
    };
    if function.kind == solid_facts::ast::FunctionKind::Declaration
        || function.r#async
        || function.generator
        || function.expression_body
        || function.rest_parameter
        || !function.parameters_inert
        || function.parameters.len() > 1
        || function
            .parameters
            .iter()
            .any(|parameter| parameter.shape != BindingShape::Identifier)
    {
        return false;
    }
    let parameter = function
        .parameters
        .first()
        .map(|parameter| parameter.declaration);
    let body = function.body;
    let inside = |inner: solid_facts::core::Span| body.contains(inner);
    if ast.calls.iter().any(|call| inside(call.span))
        || ast
            .functions
            .iter()
            .any(|inner| inner.span != function.span && inside(inner.span))
        || ast.classes.iter().any(|class| inside(class.span))
        || ast.jsx_elements.iter().any(|element| inside(element.span))
        || ast.module_loads.iter().any(|load| inside(load.span))
        || ast.returns.iter().any(|returned| inside(returned.span))
        || ast.deleted_targets.iter().any(|target| inside(*target))
    {
        return false;
    }
    // The store the write lands in: the parameter (the setter passes the store's proxy) or the root the creation bound.
    let store_name = |object: solid_facts::core::Span| {
        let object = ast.peel_ts_sugar_span(object);
        crate::owners::is_identifier_span(file, object)
            && ast
                .reference_declaration(object)
                .is_some_and(|declaration| {
                    Some(declaration) == parameter || Some(declaration) == root
                })
    };
    let mut targets = Vec::new();
    for assignment in ast
        .assignments
        .iter()
        .filter(|assignment| inside(assignment.target))
    {
        let target = ast.peel_ts_sugar_span(assignment.target);
        let Some(member) = ast.member_at(target) else {
            return false;
        };
        // A key with no text is no static key: the updater is not proven.
        let Some(key) = file.source_text(member.property) else {
            return false;
        };
        if assignment.reads_target
            || ast.computed_members.binary_search(&member.span).is_ok()
            || matches!(key, "__proto__" | "constructor" | "prototype")
            || !store_name(member.object)
            || !ctx
                .semantic_lookup
                .proven_primitive(file, assignment.value_span, PROOF_DEPTH)
        {
            return false;
        }
        targets.push(member);
    }
    let objects = targets
        .iter()
        .map(|member| ast.peel_ts_sugar_span(member.object))
        .collect::<Vec<_>>();
    ast.members
        .iter()
        .filter(|member| inside(member.span))
        .all(|member| targets.iter().any(|target| target.span == member.span))
        && ast
            .identifiers
            .iter()
            .filter(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && inside(identifier.span)
            })
            .all(|identifier| objects.contains(&identifier.span))
}

/// Whether the receiver of `call` is the library's own value: no receiver (an identifier callee), a primitive proven from its own expression, or an unshadowed ECMAScript
/// global named directly (`Math.max`, not `Intl.DateTimeFormat.supportedLocalesOf`).
fn receiver_is_the_librarys(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let Some(member) = file.ast.member_at(callee) else {
        // `f(…)` of an identifier; a callee that is a call's result or a composite names a value no fact proves.
        return crate::owners::is_identifier_span(file, callee);
    };
    if file.ast.computed_members.contains(&member.span) {
        return false;
    }
    let object = file.ast.peel_ts_sugar_span(member.object);
    ctx.semantic_lookup
        .proven_primitive(file, object, PROOF_DEPTH)
        || is_ecmascript_global(ctx, file, object)
}

/// Whether `object` is an identifier no declaration of the program binds whose symbol the compiler declares only in the default library's ECMAScript files (`lib.es5.d.ts`,
/// `lib.es2015.promise.d.ts`): a global the realm provides. A global the DOM library declares (`window`, `document`, `console`) is a host object with listeners and hooks of its
/// own, and a global a project declares is the project's.
pub(super) fn is_ecmascript_global(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    object: solid_facts::core::Span,
) -> bool {
    if !crate::owners::is_identifier_span(file, object)
        || file.ast.reference_declaration(object).is_some()
    {
        return false;
    }
    let Some(symbol) = ctx.entities.at(file.path.as_str(), object) else {
        return false;
    };
    ctx.semantic_lookup
        .symbol_declarations(symbol.as_str())
        .is_some_and(|declarations| {
            !declarations.is_empty()
                && declarations.iter().all(|declaration| {
                    std::path::Path::new(declaration.location.path.as_ref())
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("lib.es") && name.ends_with(".d.ts"))
                })
        })
}

/// Whether `object` is an unshadowed ECMAScript global in a program that leaves every built-in alone: a plain value (S4's member reads of `Math.PI`, `Number.MAX_SAFE_INTEGER`).
pub(super) fn is_unpatched_ecmascript_global(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    object: solid_facts::core::Span,
) -> bool {
    builtins_untouched(ctx) && is_ecmascript_global(ctx, file, object)
}

/// Whether `argument` cannot carry project code onto this stack (module docs). `behavior` is the audited table's answer for the argument's slot, when the call is exactly resolved.
fn argument_is_inert(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    argument: &ArgumentFact,
    behavior: Option<RuntimeArgumentBehavior>,
    object_prototype_touched: &dyn Fn() -> bool,
) -> bool {
    if argument.spread {
        return false;
    }
    let nullish = argument.runtime_value_kind == RuntimeValueKind::Nullish;
    // A slot the table says runs the value later, on another stack or with a registration: a timer's callback, a listener. The dispatch that reaches a listener is its own site. Only a
    // function counts there: a string is code the host evaluates, and a value no fact proves may be one.
    if matches!(
        behavior,
        Some(
            RuntimeArgumentBehavior::DeferredCallback | RuntimeArgumentBehavior::FreshStackCallback
        )
    ) {
        return nullish || is_function_value(ctx, file, argument);
    }
    nullish
        || (literal_is_inert(&argument.literal_value)
            // An object literal inherits from `Object.prototype`: where a file of the program can name it (give it an accessor, or hand it to code that does: #232's "touched"
            // fact, an unread package included), a library that reads a member of the literal (`JSON.stringify` looks up `toJSON`) runs project code.
            && (!matches!(argument.literal_value, ArgumentLiteralFact::Object(_))
                || !object_prototype_touched()))
        // ADR 1097 (the reviews of its widening): `proven_primitive` proves a value's kind, not that computing it runs no code (`"k" in px` reaches a
        // Proxy's `has` trap, `x instanceof K` `Symbol.hasInstance`, `${host}` a `toString`). An argument is inert by it only when it is a name
        // the program declares: reading one runs nothing.
        || (crate::owners::is_identifier_span(file, file.ast.peel_ts_sugar_span(argument.span))
            && file
                .ast
                .reference_declaration(file.ast.peel_ts_sugar_span(argument.span))
                .is_some()
            && ctx
                .semantic_lookup
                .proven_primitive(file, argument.span, PROOF_DEPTH))
        // ADR 0984: a `let` timer handle every reference of which is a proven write or a clear, in a browser.
        || super::timer_handle::is_timer_handle(ctx, file, argument.span)
}

/// Whether `argument` is a function by an exact fact: a function literal, or an identifier that names a project function.
fn is_function_value(ctx: &AnalysisContext<'_>, file: &FileFacts, argument: &ArgumentFact) -> bool {
    matches!(
        argument.value,
        ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
    ) || argument.runtime_value_kind == RuntimeValueKind::Function
        || ctx
            .semantic_lookup
            .function_called_at(file.path.as_str(), argument.span)
            .is_some()
}

/// Whether the exact runtime literal written at an argument is data and nothing else: a null, a boolean, a number, a string, or an object literal of plain data properties (the
/// fact is `Unknown` for a spread, an accessor, a computed key or a `__proto__`) whose values are, recursively. An array literal's elements are not recorded, and a function
/// is code.
fn literal_is_inert(literal: &ArgumentLiteralFact) -> bool {
    match literal {
        ArgumentLiteralFact::Null
        | ArgumentLiteralFact::Boolean(_)
        | ArgumentLiteralFact::Integer(_)
        | ArgumentLiteralFact::String(_) => true,
        ArgumentLiteralFact::Object(properties) => properties
            .iter()
            .all(|property| literal_is_inert(&property.value)),
        ArgumentLiteralFact::Unknown
        | ArgumentLiteralFact::Function
        | ArgumentLiteralFact::ArrayLength(_) => false,
    }
}
