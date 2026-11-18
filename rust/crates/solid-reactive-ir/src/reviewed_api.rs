//! ADR 0975: a reviewed reading of a package's source, applied at a site.
//!
//! A call into a package the build does not read as source is a site (ADR 0972). Where the source of one API was *read* (the backend pins the reading to digests
//! of the files it rests on: `solid-facts-backend/src/reviewed_packages.rs`), the call is no site for what the reading shows it calls. The reading of vitest 4.1's
//! `expect` and `vi` is in ADR 0975: a matcher that is handed a value no foreign accessor can run in calls only vitest's and chai's code, the functions the program
//! registered (`expect.addEqualityTesters`, which are handed to a package and are the hand-off's), and the standard library.
//!
//! Everything is identified by the declaration the compiler resolves (its name, and the installed declaration file it is in), never by the spelling at the call, and
//! only where the premise holds for that file's package ([`solid_facts::package_reach::PackageReachIndex::premise_holds`]). A call, a value or a chain the reading does
//! not name stays a site. A value handed to the API is judged by what the matcher does with it:
//!
//! - **a primitive** (the compiler's primitive value domain at the expression has no object bit and is not unknown; a literal of one): a generic traversal of it reads
//!   no member that can be an accessor;
//! - **a flat object literal of primitives**, written where it is used (`toEqual`, `toStrictEqual`): its own members are all data, and nothing else holds it;
//! - **a spy** `vi.fn` or `vi.spyOn` made (a binding of one initializer), for the matchers that read the spy's own call count.
//!
//! Anything else (an object, an array, an `any`, a type parameter's value, a value a package made, a class instance) keeps its site.

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentFact, CallFact, RuntimeValueKind};
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;

/// The id of the premise in `solid-facts-backend/src/reviewed_packages.rs`.
const VITEST_4_1: &str = "vitest-4.1";

/// ADR 1063: the premise of `@stylexjs/stylex` 0.19.0's runtime, and the exports it shows throw on every call (declared in `lib/es/stylex.d.ts`).
const STYLEX_0_19: &str = "stylex-0.19";
const STYLEX_THROWING: &[&str] = &[
    "create",
    "createTheme",
    "defineConsts",
    "defineVars",
    "defineMarker",
    "defaultMarker",
    "firstThatWorks",
    "keyframes",
    "positionTry",
    "viewTransitionClass",
    "unstable_conditional",
    "unstable_defineVarsNested",
    "unstable_defineConstsNested",
    "unstable_createThemeNested",
];
const STYLEX_DECLARATIONS: &[&str] = &[
    "node_modules",
    "@stylexjs",
    "stylex",
    "lib",
    "es",
    "stylex.d.ts",
];

/// ADR 1063: whether `call` is a call of a package export the reviewed reading shows throws on every call, so it returns no value and nothing read off its result
/// ever runs: one of [`STYLEX_THROWING`], named by its own identifier (`create(…)`) or as a member of a namespace or an object (`stylex.create(…)`), where every
/// declaration the compiler resolves for that name is that export in the pinned `@stylexjs/stylex` declarations and the premise holds for the package. A `new`, a
/// computed member, a name declared anywhere else, and a version nobody read are not.
pub(crate) fn call_always_throws(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
) -> bool {
    if call.construct || !any_premise_holds(ctx) {
        return false;
    }
    let ast = &file.ast;
    let callee = ast.peel_ts_sugar_span(call.callee);
    let named = if let Some(member) = ast.member_at(callee) {
        if ast.computed_members.binary_search(&callee).is_ok() {
            return false;
        }
        member.property
    } else if crate::owners::is_identifier_span(file, callee) {
        callee
    } else {
        return false;
    };
    let Some(index) = ctx.package_reach else {
        return false;
    };
    let Some(declarations) = ctx.semantic_lookup.alias_root_declarations(file, named) else {
        return false;
    };
    !declarations.is_empty()
        && declarations.iter().all(|declaration| {
            let path = declaration.location.path.as_ref();
            STYLEX_THROWING.contains(&declaration.name.as_ref())
                && !path.starts_with("bundled:")
                && solid_facts::package_reach::installed_spelling(Some(index), path)
                    .split(['/', '\\'])
                    .collect::<Vec<_>>()
                    .windows(STYLEX_DECLARATIONS.len())
                    .any(|window| window == STYLEX_DECLARATIONS)
                && index.premise_holds(path, STYLEX_0_19)
        })
}

/// What a reviewed matcher does with the value it asserts about and with its arguments.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Class {
    /// Compares or tests the value: the value and the arguments are read by the generic equality and message helpers, so neither may hold an accessor the program did
    /// not write. A primitive; `toEqual` and `toStrictEqual` also take a flat object literal of primitives.
    Value,
    /// Reads a spy's own call count and name: the value is a spy `vi.fn` or `vi.spyOn` made, the arguments primitives.
    SpyCount,
}

/// The matchers the reading covers, by the name the compiler declares them under in `@vitest/expect`'s declarations (`JestAssertion`). `.not` chains, `.resolves`,
/// `.rejects`, `.soft` and every other matcher are not in it: a chain written another way is no site this clears.
const MATCHERS: &[(&str, Class)] = &[
    ("toBe", Class::Value),
    ("toEqual", Class::Value),
    ("toStrictEqual", Class::Value),
    ("toBeTruthy", Class::Value),
    ("toBeFalsy", Class::Value),
    ("toBeNull", Class::Value),
    ("toBeUndefined", Class::Value),
    ("toBeDefined", Class::Value),
    ("toBeNaN", Class::Value),
    ("toBeTypeOf", Class::Value),
    ("toBeGreaterThan", Class::Value),
    ("toBeGreaterThanOrEqual", Class::Value),
    ("toBeLessThan", Class::Value),
    ("toBeLessThanOrEqual", Class::Value),
    ("toMatch", Class::Value),
    ("toHaveBeenCalled", Class::SpyCount),
    ("toHaveBeenCalledTimes", Class::SpyCount),
    ("toHaveBeenCalledOnce", Class::SpyCount),
];

/// The members of `vi` whose registration the reading shows stores its argument and calls nothing of it: `vi.fn(impl)` keeps `impl` in the mock's configuration, and
/// `vi.stubGlobal(name, value)` defines `value` on the global object. A later call through the mock is a site of its own. (`vi.spyOn` reads and redefines a property of
/// the object it is given, which may be an accessor, so it is not here.) Whether the arguments of the registration may be anything.
const REGISTRATIONS: &[(&str, RegistrationArguments)] = &[
    ("fn", RegistrationArguments::NoneOrFreshFunction),
    ("stubGlobal", RegistrationArguments::Any),
];

#[derive(Clone, Copy, Eq, PartialEq)]
enum RegistrationArguments {
    /// No argument, or one function written in place: a fresh function has no accessor of its own that anything else defined.
    NoneOrFreshFunction,
    /// Stored and never read.
    Any,
}

/// The members of `vi` that make a spy.
const SPY_MAKERS: &[&str] = &["fn", "spyOn"];

/// Whether the reviewed reading says `call` runs no code a Solid action can be reached from: a registration (`vi.fn()`), the `expect(value)` of a chain, or the matcher
/// call of one, each judged by what it is handed.
pub(crate) fn clears_call(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    if call.construct || !any_premise_holds(ctx) {
        return false;
    }
    clears_registration(ctx, file, call) || clears_assertion(ctx, file, call)
}

/// Whether the call that spans exactly `span` is an assertion the reading covers, so a value of a package handed to it (a constant typed as a primitive) is no hand-off
/// site: an assertion's value and arguments are primitives, flat literals or spies, never a function that is called later. A registration that only stores what it is
/// given (`vi.stubGlobal(name, value)`) clears the call but not the value: the stored function runs wherever the global is called, so a package function handed to it is
/// a site as before.
pub(crate) fn clears_value_at(ctx: &AnalysisContext<'_>, file: &FileFacts, span: Span) -> bool {
    if !any_premise_holds(ctx) {
        return false;
    }
    file.ast
        .call_at(span)
        .is_some_and(|call| !call.construct && clears_assertion(ctx, file, call))
}

/// Whether a reviewed premise holds for any installed package of the program: where none does (no installed package, or none the reading covers, or no filesystem), no
/// call is cleared and no declaration needs to be asked.
fn any_premise_holds(ctx: &AnalysisContext<'_>) -> bool {
    let Some(index) = ctx.package_reach else {
        return false;
    };
    !index.reviewed.is_empty()
}

/// Whether every declaration of the symbol at `span` is `name` in a declaration file with the path components `run`, of a package the premise holds for.
fn declared_by_reviewed(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    span: Span,
    name: &str,
    run: &[&str],
) -> bool {
    let Some(index) = ctx.package_reach else {
        return false;
    };
    let Some(declarations) = ctx.semantic_lookup.alias_root_declarations(file, span) else {
        return false;
    };
    !declarations.is_empty()
        && declarations.iter().all(|declaration| {
            let path = declaration.location.path.as_ref();
            declaration.name.as_ref() == name
                && !path.starts_with("bundled:")
                && solid_facts::package_reach::installed_spelling(Some(index), path)
                    .split(['/', '\\'])
                    .collect::<Vec<_>>()
                    .windows(run.len())
                    .any(|window| window == run)
                && index.premise_holds(path, VITEST_4_1)
        })
}

/// The member of `vi` a callee names: `Some(property span)` when the callee is `vi.<member>` for the `vi` the reviewed `vitest` declares and `member` is a declared
/// property of `VitestUtils` in the reviewed declarations.
fn vi_member(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    callee: Span,
    members: &[&str],
) -> Option<usize> {
    let ast = &file.ast;
    let callee = ast.peel_ts_sugar_span(callee);
    if ast.computed_members.binary_search(&callee).is_ok() {
        return None;
    }
    let member = ast.member_at(callee)?;
    let object = ast.peel_ts_sugar_span(member.object);
    if !declared_by_reviewed(
        ctx,
        file,
        object,
        "vi",
        &["node_modules", "vitest", "dist", "index.d.ts"],
    ) {
        return None;
    }
    members.iter().position(|name| {
        declared_by_reviewed(
            ctx,
            file,
            member.property,
            name,
            &["node_modules", "vitest", "dist", "index.d.ts"],
        )
    })
}

/// `vi.fn(…)` and `vi.stubGlobal(…)`, where what they are handed is what the reading shows they only store.
fn clears_registration(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let names = REGISTRATIONS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    let Some(which) = vi_member(ctx, file, call.callee, &names) else {
        return false;
    };
    match REGISTRATIONS[which].1 {
        RegistrationArguments::Any => !call.arguments.iter().any(|argument| argument.spread),
        RegistrationArguments::NoneOrFreshFunction => match call.arguments.as_slice() {
            [] => true,
            [argument] => {
                !argument.spread
                    && matches!(
                        argument.value,
                        solid_facts::ast::ArgumentValueKind::Function
                            | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    )
            }
            _ => false,
        },
    }
}

/// The two calls of `expect(value).matcher(arguments)`: this call is one of them, and the pair is one the reading covers with a value and arguments it is safe for.
fn clears_assertion(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let ast = &file.ast;
    let callee = ast.peel_ts_sugar_span(call.callee);
    // This call is the matcher's (its callee is a member of the `expect(…)` call), or the `expect(…)` call whose result a matcher is called on.
    let (inner, matcher_call) = if let Some(member) = ast.member_at(callee)
        && let Some(inner) = ast.call_at(ast.peel_ts_sugar_span(member.object))
    {
        (inner, call)
    } else {
        let Some(member) = ast.members_with_peeled_object(call.span).next() else {
            return false;
        };
        let Some(matcher_call) = ast.calls_with_peeled_callee(member.span).next() else {
            return false;
        };
        (call, matcher_call)
    };
    if inner.construct || matcher_call.construct {
        return false;
    }
    // The `expect(…)` call: the identifier the reviewed `vitest` declares as its global expect, handed exactly one argument.
    let expect = ast.peel_ts_sugar_span(inner.callee);
    if ast.member_at(expect).is_some()
        || !declared_by_reviewed(
            ctx,
            file,
            expect,
            "globalExpect",
            &["node_modules", "vitest", "dist", "index.d.ts"],
        )
    {
        return false;
    }
    let [actual] = inner.arguments.as_slice() else {
        return false;
    };
    // The matcher: a member of the `expect(…)` call itself (no `.not`, `.resolves`, `.soft`), by a static name, that the reviewed `JestAssertion` declares.
    let matcher_callee = ast.peel_ts_sugar_span(matcher_call.callee);
    let Some(member) = ast.member_at(matcher_callee) else {
        return false;
    };
    if ast.computed_members.binary_search(&matcher_callee).is_ok()
        || ast.peel_ts_sugar_span(member.object) != inner.span
    {
        return false;
    }
    let Some(&(_, class)) = MATCHERS.iter().find(|(name, _)| {
        declared_by_reviewed(
            ctx,
            file,
            member.property,
            name,
            &["node_modules", "@vitest", "expect", "dist", "index.d.ts"],
        )
    }) else {
        return false;
    };
    if matcher_call
        .arguments
        .iter()
        .any(|argument| argument.spread)
    {
        return false;
    }
    let literal_allowed = matcher_allows_literal(ctx, file, member.property);
    let arguments_safe = matcher_call.arguments.iter().all(|argument| {
        primitive_value(ctx, file, argument) || (literal_allowed && flat_literal(file, argument))
    });
    match class {
        Class::Value => {
            arguments_safe
                && (primitive_value(ctx, file, actual)
                    || (literal_allowed && flat_literal(file, actual)))
        }
        Class::SpyCount => {
            matcher_call
                .arguments
                .iter()
                .all(|argument| primitive_value(ctx, file, argument))
                && made_by_spy_maker(ctx, file, actual)
        }
    }
}

/// Whether the matcher compares with deep equality, so a flat object literal of primitives is a value it may be handed: `toEqual` and `toStrictEqual`, whose
/// declared names are the only two of the table that take one.
fn matcher_allows_literal(ctx: &AnalysisContext<'_>, file: &FileFacts, property: Span) -> bool {
    ["toEqual", "toStrictEqual"].iter().any(|name| {
        declared_by_reviewed(
            ctx,
            file,
            property,
            name,
            &["node_modules", "@vitest", "expect", "dist", "index.d.ts"],
        )
    })
}

/// Whether the argument is a primitive by the compiler's facts: a literal one, or an expression whose primitive value domain has no object bit and is not unknown.
/// The domain of a type parameter is its constraint's (the producer closes it, and an unconstrained one is unknown), so a value typed `T extends string` is one; the
/// printed type may not be read for a decision (ADR 0367), so the fact cannot say it was a type parameter. An `any` and an `unknown` have the unknown domain; an absent
/// fact is no proof.
fn primitive_value(ctx: &AnalysisContext<'_>, file: &FileFacts, argument: &ArgumentFact) -> bool {
    if argument.spread {
        return false;
    }
    if matches!(
        argument.runtime_value_kind,
        RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
    ) {
        return true;
    }
    let span = argument.value_span.unwrap_or(argument.span);
    ctx.semantic_lookup
        .entity_at(file.path.as_str(), span)
        .and_then(|entity| entity.primitive_value_domain.present())
        .is_some_and(|domain| !domain.unknown() && !domain.may_be_object())
}

/// An object literal written as the argument whose members are all data with static keys (no spread, accessor or computed key: `exact_object_literal`) and each of
/// whose values is a primitive literal: nothing in it can run code when it is traversed, and nothing else holds it.
fn flat_literal(file: &FileFacts, argument: &ArgumentFact) -> bool {
    if !argument.exact_object_literal || argument.runtime_value_kind != RuntimeValueKind::Object {
        return false;
    }
    let literal = argument.value_span.unwrap_or(argument.span);
    file.ast
        .object_properties
        .iter()
        .filter(|property| literal.contains(property.span))
        // A property of a literal nested in this one is another literal's.
        .filter(|property| {
            !file.ast.object_properties.iter().any(|outer| {
                outer.span != property.span
                    && literal.contains(outer.span)
                    && outer.span.contains(property.span)
            })
        })
        .all(|property| {
            property.data
                && !property.computed
                && !property.method
                && matches!(
                    property.value_kind,
                    RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
                )
        })
}

/// Whether the argument is a spy a reviewed maker made: a binding of one initializer that is a `vi.fn(…)` or `vi.spyOn(…)` call, or such a call written in place.
fn made_by_spy_maker(ctx: &AnalysisContext<'_>, file: &FileFacts, argument: &ArgumentFact) -> bool {
    if argument.spread {
        return false;
    }
    let span = file
        .ast
        .peel_ts_sugar_span(argument.value_span.unwrap_or(argument.span));
    if let Some(call) = file.ast.call_at(span) {
        return !call.construct && vi_member(ctx, file, call.callee, SPY_MAKERS).is_some();
    }
    let Some((initializer_file, initializer)) = ctx
        .semantic_lookup
        .exact_initializer_at_reference(file, span)
    else {
        return false;
    };
    initializer_file
        .ast
        .call_at(initializer_file.ast.peel_ts_sugar_span(initializer))
        .is_some_and(|call| {
            !call.construct && vi_member(ctx, initializer_file, call.callee, SPY_MAKERS).is_some()
        })
}
