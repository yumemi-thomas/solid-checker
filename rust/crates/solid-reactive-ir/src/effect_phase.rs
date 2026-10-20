//! Where code provably runs synchronously inside a user effect's apply
//! callback (the dialect's *effect phase*,
//! [`solid_dialect::Dialect::callback_runs_in_effect_phase`]).
//!
//! The question is the reverse of an execution role: not "what role does this
//! span run in" but "does every step between this span and the effect's apply
//! run on the apply's own stack". The walk climbs from the span outward, one
//! function layer at a time, and answers only where each layer is proven:
//!
//! - the layer is the function written as the effect's apply argument: the
//!   walk is done;
//! - the layer is written as the callback of a primitive the dialect states
//!   runs it during the call ([`solid_dialect::Dialect::synchronous_callback_slot`]:
//!   `untrack`, `createRoot`, `runWithOwner`, `flush(fn)`), or of a
//!   standard-library call the audited rows model as invoking it inline
//!   (`forEach`, `map`, ...): the walk continues from that call;
//! - the layer is an immediately invoked function: the walk continues from the
//!   invoking call;
//! - the layer is a project function bound to a name nothing reassigns: the
//!   walk continues from any one resolved call of it, so a helper reached from
//!   the effect is proven by that call alone (the other calls of the helper
//!   are no evidence either way).
//!
//! Each layer's site must also have a normal path from its function's entry
//! ([`solid_facts::ast::site_reachable`]: abrupt completion by `return`,
//! `throw`, `break`, an infinite loop, a `try` that only leaves abruptly, a
//! `switch` whose arms all return, and calls that never return: a `never`
//! signature, an exactly resolved project function that only throws, an inline
//! callback over a literal non-empty array that only throws). A condition the
//! analysis cannot decide is taken either way (the engine's feasible-path
//! premise); a function it cannot analyze is not claimed. An effect whose
//! compute is a function written inline, or an exactly resolved project
//! function, must be able to return a value ([`solid_facts::ast::function_may_return`]),
//! or the apply never runs; any other compute (a signal accessor, a member, a
//! package function) is assumed able to, like every unknown callee. A callback
//! of an array literal written empty is never invoked.
//!
//! The queue the nested `flush` finds running must be the apply's: the file
//! holding the `flush` call and the file holding the `createEffect` call must
//! load the same installed `solid-js` at runtime
//! ([`crate::PackageInstanceEvidence`], module resolution with symlinks
//! resolved), and both calls' declarations must be in one package directory.
//!
//! Anything else answers `None`: an unresolved callee, a callback handed to a
//! timer, listener or promise, a generator (calling one runs none of its body),
//! a method, a parameter initializer, an instance field initializer, a call
//! through a binding that may have been replaced. An async layer is on the
//! stack only up to its first suspension.

use crate::{HashMap, HashSet};

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, FunctionFact};
use solid_facts::core::Span;

use crate::effect_execution::{self, ApplyExecution};
use crate::execution_role::{inside_instance_field, project_callee_at_parameter};
use crate::indexes::SemanticLookup;
use crate::owners::{containing_ast_function, function_binding_name, written_directly_in};

/// Call layers one proof may cross. A chain this long is not one a reader can
/// follow, and the bound keeps a cycle-free but wide helper graph cheap.
const MAX_LAYERS: usize = 12;

/// How many function declarations one question may expand before it gives up:
/// a project with thousands of call sites of one helper stays bounded.
const BUDGET: usize = 400;

/// The `createEffect` call whose apply callback a span provably runs inside,
/// and what stood between them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EffectPhaseOrigin {
    /// Path of the file holding the effect call.
    pub(crate) path: String,
    /// Project functions the walk entered through a call, outermost last.
    pub(crate) helper_calls: usize,
}

/// The effect call whose apply callback provably runs the code at `span`
/// synchronously, or `None` where that is not proven.
pub(crate) fn effect_phase_origin(
    file: &FileFacts,
    span: Span,
    package_root: &str,
    instances: &crate::PackageInstanceEvidence,
    lookup: &SemanticLookup<'_>,
) -> Option<EffectPhaseOrigin> {
    // The instance the flush call's file loads; unresolved is no proof.
    let flush_instance = instances.solid_js_by_file.get(file.path.as_str())?;
    let mut walk = Walk {
        lookup,
        package_root,
        flush_instance,
        instances,
        returns: HashMap::default(),
        visiting: HashSet::default(),
        budget: BUDGET,
    };
    walk.layer(file, span, 0)
}

struct Walk<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    /// The package instance the starting call's declaration is in: the effect
    /// call must resolve to the same one, or its queue is not the one running.
    package_root: &'l str,
    /// The `solid-js` directory the flush call's file loads at runtime, and
    /// what every analyzed file loads.
    flush_instance: &'l str,
    instances: &'l crate::PackageInstanceEvidence,
    /// Whether a call of a project function can return, by function.
    returns: HashMap<(String, Span), Option<bool>>,
    /// Functions the walk is inside through a call, so a call cycle ends.
    visiting: HashSet<(String, Span)>,
    budget: usize,
}

impl Walk<'_, '_> {
    fn layer(
        &mut self,
        file: &FileFacts,
        span: Span,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        if helper_calls > MAX_LAYERS || inside_instance_field(file, span) {
            return None;
        }
        // Module scope: nothing around the span runs it.
        let literal = containing_ast_function(&file.ast, span)?;
        // A parameter initializer lies outside every body, so the body-based
        // query above placed it in the outer function; it runs when its own
        // function is called, which no layer here proves.
        if !written_directly_in(&file.ast, literal, span) {
            return None;
        }
        // Calling a generator runs none of its body; a method or accessor is
        // reached through a receiver this walk does not resolve.
        if literal.generator || literal.method_name.is_some() {
            return None;
        }
        if literal.r#async && !runs_before_first_suspension(file, literal, span) {
            return None;
        }
        // A site no normal path reaches never runs. A function the analysis
        // cannot parse or find is no proof either way.
        let diverging = self.diverging_calls(file, literal, 0);
        if solid_facts::ast::site_reachable(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            literal.span,
            span,
            &diverging,
        ) != Some(true)
        {
            return None;
        }
        if let Some((call, index)) = argument_position(file, literal) {
            return self.argument_layer(file, call, index, helper_calls);
        }
        if let Some(call) = iife_call(file, literal) {
            return self.layer(file, call.span, helper_calls);
        }
        self.named_layer(file, literal, helper_calls)
    }

    /// `literal` is written as an argument of `call`.
    fn argument_layer(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        let dialect = self.lookup.dialect;
        if call.construct
            || call
                .arguments
                .iter()
                .take(index + 1)
                .any(|argument| argument.spread)
        {
            return None;
        }
        if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
            if dialect.callback_runs_in_effect_phase(primitive, index) {
                // An apply the runtime may never run proves nothing, nor does
                // one whose compute may always throw, nor one of another
                // package instance's queue.
                return (effect_execution::at_call(file, call, primitive, index, self.lookup)
                    == ApplyExecution::Proven
                    && self.compute_may_return(file, call)
                    && self.same_package_instance(file, call))
                .then(|| EffectPhaseOrigin {
                    path: file.path.to_string(),
                    helper_calls,
                });
            }
            return if dialect.synchronous_callback_slot(primitive, index, call.arguments.len()) {
                self.layer(file, call.span, helper_calls)
            } else {
                None
            };
        }
        // A standard-library call the audited rows model as invoking the
        // argument before it returns.
        let resolved = self
            .lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library)
            })?;
        // A callback of an array literal written empty is never invoked.
        if file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .and_then(|member| file.source_text(file.ast.peel_ts_sugar_span(member.object)))
            .is_some_and(|object| object.trim() == "[]")
        {
            return None;
        }
        let callability = self
            .lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, index),
            Some(crate::runtime_semantics::RuntimeArgumentBehavior::InlineCallback)
        )
        .then(|| self.layer(file, call.span, helper_calls))
        .flatten()
    }

    /// Whether the call's resolved declaration is in the package instance the
    /// walk started from (the nearest `node_modules` package directory).
    fn same_package_instance(&self, file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
        self.instances
            .solid_js_by_file
            .get(file.path.as_str())
            .map(String::as_str)
            == Some(self.flush_instance)
            && self
                .lookup
                .resolved_callee_call(file, call.callee)
                .and_then(|resolved| resolved.declaration.as_ref())
                .and_then(|declaration| package_root(declaration.location.path.as_ref()))
                .is_some_and(|root| root == self.package_root)
    }

    /// Whether the effect's compute can return a value, so that its apply
    /// runs: a function written inline, or an exactly resolved project
    /// function, must show a normal return; anything else is assumed to.
    fn compute_may_return(&mut self, file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
        let Some(compute) = call.arguments.first() else {
            return false;
        };
        let span = file.ast.peel_ts_sugar_span(compute.span);
        let function = if let Some(function) = file.ast.function_at(span) {
            Some((file, function))
        } else if compute.value == ArgumentValueKind::Identifier {
            self.lookup
                .function_called_at(file.path.as_str(), span)
                .filter(|(function_file, function)| {
                    self.lookup
                        .function_value_is_current(function_file, function)
                })
        } else {
            None
        };
        match function {
            Some((function_file, function)) => {
                self.may_return(function_file, function, 0) == Some(true)
            }
            None => true,
        }
    }

    /// Whether a call of `function` can return, by the completion analysis;
    /// `None` when it cannot be analyzed or the budget is spent.
    fn may_return(
        &mut self,
        file: &FileFacts,
        function: &FunctionFact,
        depth: usize,
    ) -> Option<bool> {
        let key = (file.path.to_string(), function.span);
        if let Some(known) = self.returns.get(&key) {
            return *known;
        }
        if depth > 3 || self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        // A call cycle answers "unknown" to its own inner calls.
        self.returns.insert(key.clone(), None);
        let diverging = self.diverging_calls(file, function, depth + 1);
        let answer = solid_facts::ast::function_may_return(
            std::path::Path::new(file.path.as_str()),
            &file.source,
            function.span,
            &diverging,
        );
        self.returns.insert(key, answer);
        answer
    }

    /// The calls written directly in `function` that provably never return
    /// normally.
    fn diverging_calls(
        &mut self,
        file: &FileFacts,
        function: &FunctionFact,
        depth: usize,
    ) -> Vec<Span> {
        let mut found = Vec::new();
        for call in &file.ast.calls {
            if call.construct
                || !function.body.contains(call.span)
                || !written_directly_in(&file.ast, function, call.span)
            {
                continue;
            }
            if self.call_diverges(file, call, depth) {
                found.push(call.span);
            }
        }
        found
    }

    fn call_diverges(
        &mut self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> bool {
        // A `never` signature (not `asserts`, which may return).
        if self
            .lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| {
                resolved.never_returns
                    && resolved.validity == typefacts::ResolvedCallValidity::Valid
            })
        {
            return true;
        }
        // A project function that only throws, reached by an exact call.
        if let Some((callee_file, callee, true)) =
            project_callee_at_parameter(file, call, 0, self.lookup)
            && !callee.generator
            && !callee.r#async
            && self.may_return(callee_file, callee, depth) == Some(false)
        {
            return true;
        }
        // An inline array method over a literal with a first element, whose
        // callback only throws: the first element reaches it.
        const PER_ELEMENT: [&str; 10] = [
            "forEach",
            "map",
            "filter",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "some",
            "every",
            "flatMap",
        ];
        let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
            return false;
        };
        if !file
            .source_text(member.property)
            .is_some_and(|name| PER_ELEMENT.contains(&name))
            || solid_facts::ast::literal_array_element(
                std::path::Path::new(file.path.as_str()),
                &file.source,
                file.ast.peel_ts_sugar_span(member.object),
                0,
            )
            .is_none()
        {
            return false;
        }
        let Some(resolved) =
            self.lookup
                .resolved_callee_call(file, call.callee)
                .filter(|resolved| {
                    resolved
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.standard_library)
                })
        else {
            return false;
        };
        let Some(argument) = call.arguments.first() else {
            return false;
        };
        let callability = self
            .lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if !matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, 0),
            Some(crate::runtime_semantics::RuntimeArgumentBehavior::InlineCallback)
        ) || argument.spread
        {
            return false;
        }
        file.ast
            .function_at(file.ast.peel_ts_sugar_span(argument.span))
            .filter(|callback| !callback.r#async && !callback.generator)
            .is_some_and(|callback| self.may_return(file, callback, depth) == Some(false))
    }

    /// `literal` is a function bound to a name: it runs during the effect
    /// phase when one resolved call of it does.
    fn named_layer(
        &mut self,
        file: &FileFacts,
        literal: &FunctionFact,
        helper_calls: usize,
    ) -> Option<EffectPhaseOrigin> {
        function_binding_name(file, literal)?;
        if !self.lookup.function_value_is_current(file, literal) {
            return None;
        }
        let key = (file.path.to_string(), literal.span);
        if !self.visiting.insert(key.clone()) {
            return None;
        }
        let mut found = None;
        for (caller, callee) in self
            .lookup
            .function_call_sites(file.path.as_str(), literal.span)
        {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            // A call expression of the function, not a JSX tag that renders it
            // or a `new`.
            let Some(call) = caller.ast.calls.iter().find(|call| {
                !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
            }) else {
                continue;
            };
            // The call must *exactly* run this function: a member callee only
            // when its receiver is proven to hold the declared method, a
            // binding only while nothing replaces it, no spread shifting the
            // arguments ([`project_callee_at_parameter`]'s `exact`).
            let exact = project_callee_at_parameter(caller, call, 0, self.lookup).is_some_and(
                |(callee_file, function, exact)| {
                    exact
                        && callee_file.path.as_str() == file.path.as_str()
                        && function.span == literal.span
                },
            );
            if !exact {
                continue;
            }
            if let Some(origin) = self.layer(caller, call.span, helper_calls + 1) {
                found = Some(origin);
                break;
            }
        }
        self.visiting.remove(&key);
        found
    }
}

/// The call and argument position at which `literal` is written as the
/// argument itself (behind TypeScript sugar at most). `wrap(() => …)` places a
/// literal in `wrap`'s argument; `wrap(function () {})()` does not place the
/// inner function anywhere this walk understands.
fn argument_position<'f>(
    file: &'f FileFacts,
    literal: &FunctionFact,
) -> Option<(&'f solid_facts::ast::CallFact, usize)> {
    file.ast
        .arguments_containing(literal.span)
        .filter(|(call, index)| {
            let argument = &call.arguments[*index];
            matches!(
                argument.value,
                ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
            ) && file.ast.peel_ts_sugar_span(argument.span) == literal.span
        })
        .min_by_key(|(call, _)| call.span.end - call.span.start)
}

/// The call that invokes `literal` where it is written: the literal is the
/// call's callee, behind TypeScript sugar at most.
fn iife_call<'f>(
    file: &'f FileFacts,
    literal: &FunctionFact,
) -> Option<&'f solid_facts::ast::CallFact> {
    file.ast
        .calls
        .iter()
        .filter(|call| !call.construct && file.ast.peel_ts_sugar_span(call.callee) == literal.span)
        .min_by_key(|call| call.span.end - call.span.start)
}

/// Whether the code at `span` in the async function `function` runs before
/// the function first suspends, so on the stack of whoever called it.
///
/// Every `await` of the body that starts before the span ends must contain it
/// (the awaited operand is evaluated first), no implicit suspension (`for
/// await`, an async `yield*`, `await using`) may start before it, and no loop
/// around it may contain a suspension that a later iteration would pass first.
pub(crate) fn runs_before_first_suspension(
    file: &FileFacts,
    function: &FunctionFact,
    span: Span,
) -> bool {
    let own = |candidate: Span| {
        containing_ast_function(&file.ast, candidate)
            .is_some_and(|owner| owner.span == function.span)
    };
    let awaits = file
        .ast
        .awaits
        .iter()
        .copied()
        .filter(|suspension| function.body.contains(*suspension) && own(*suspension))
        .collect::<Vec<_>>();
    let implicit = file
        .ast
        .implicit_suspensions
        .iter()
        .copied()
        .filter(|suspension| function.body.contains(*suspension) && own(*suspension))
        .collect::<Vec<_>>();
    let before = |suspension: &Span| suspension.start < span.end;
    if awaits
        .iter()
        .any(|suspension| before(suspension) && !suspension.contains(span))
        || implicit.iter().any(before)
    {
        return false;
    }
    !file.ast.loop_statements.iter().any(|looped| {
        looped.contains(span)
            && function.body.contains(*looped)
            && awaits
                .iter()
                .chain(&implicit)
                .any(|suspension| looped.contains(*suspension))
    })
}

/// The directory of the package a declaration file belongs to: the path up to
/// and including the package name after its last `node_modules/`, scope
/// included. `None` for a file outside any `node_modules`.
pub(crate) fn package_root(path: &str) -> Option<String> {
    let marker = "node_modules/";
    let at = path.rfind(marker)? + marker.len();
    let rest = &path[at..];
    let mut segments = rest.split('/');
    let first = segments.next()?;
    let name_len = if first.starts_with('@') {
        first.len() + 1 + segments.next()?.len()
    } else {
        first.len()
    };
    Some(path[..at + name_len].to_owned())
}

#[cfg(test)]
mod tests {
    use super::package_root;

    #[test]
    fn a_package_root_is_the_nearest_node_modules_package_directory() {
        assert_eq!(
            package_root("/p/node_modules/@solidjs/signals/dist/types/signals.d.ts").as_deref(),
            Some("/p/node_modules/@solidjs/signals")
        );
        // A copy nested under `solid-js` is another instance.
        assert_eq!(
            package_root(
                "/p/node_modules/solid-js/node_modules/@solidjs/signals/dist/types/signals.d.ts"
            )
            .as_deref(),
            Some("/p/node_modules/solid-js/node_modules/@solidjs/signals")
        );
        assert_eq!(
            package_root("/p/node_modules/solid-js/types/index.d.ts").as_deref(),
            Some("/p/node_modules/solid-js")
        );
        assert_eq!(package_root("/p/src/App.ts"), None);
    }
}
