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
    if !matches!(via, "setter" | "library" | "construct") || !builtins_untouched(ctx) {
        return false;
    }
    if via == "setter" {
        setter_runs_nothing(ctx, file, call, object_prototype_touched)
    } else {
        standard_call_runs_nothing(ctx, file, call, object_prototype_touched)
    }
}

/// Whether no analyzed file touches a built-in or evaluates code from a string ([`TOUCHED_BUILTINS`]).
fn builtins_untouched(ctx: &AnalysisContext<'_>) -> bool {
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
    receiver_is_the_librarys(ctx, file, call)
        && call.arguments.iter().enumerate().all(|(index, argument)| {
            let behavior = runtime_semantics::argument_behavior(resolved, None, index);
            argument_is_inert(ctx, file, argument, behavior, object_prototype_touched)
        })
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
    if !call
        .arguments
        .iter()
        .all(|argument| argument_is_inert(ctx, file, argument, None, object_prototype_touched))
    {
        return false;
    }
    let lookup = ctx.semantic_lookup;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !crate::owners::is_identifier_span(file, callee) {
        return false;
    }
    let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), callee)
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
    let Some(created) = binding
        .initializer
        .map(|initializer| file.ast.peel_ts_sugar_span(initializer))
        .and_then(|initializer| file.ast.call_at(initializer))
    else {
        return false;
    };
    // The call is the dialect's signal creation, and its options argument is the dialect's.
    let Some(options) = lookup
        .primitive_at_call(file, created.span)
        .and_then(|primitive| ctx.dialect.signal_creation_options(primitive))
    else {
        return false;
    };
    !created.construct
        && matches!(
            crate::call_option_literal(file, created, options, "equals"),
            OptionLiteral::Absent | OptionLiteral::False
        )
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
fn is_ecmascript_global(
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
        || ctx
            .semantic_lookup
            .proven_primitive(file, argument.span, PROOF_DEPTH)
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
