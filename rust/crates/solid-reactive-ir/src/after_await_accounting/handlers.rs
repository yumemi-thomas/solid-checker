//! ADR 0976, S3: the functions a promise handler call runs after a suspension.
//!
//! A `.then`, `.catch` or `.finally` callback runs when its promise settles, in a later microtask: it is code written after a suspension whether or not the function holding
//! the chain is `async`. The review of #233 found Chrome raising UNTRACKED_READ_AFTER_AWAIT at the read in `function work() { return pause().then(() => source()); }`
//! handed through a wrapper to `createMemo` (a compute that returns the chained promise), with no row: the accounting listed only the code of an `async` function after its
//! own `await`, and the walks that follow promise chains start only from a computation the producers recognize. There is no census reason (ADR 0973), so a function that is a
//! promise handler is after a suspension, and the reads of the universe and the calls written directly in it are sites like those after an `await`.
//!
//! A handler is a function literal, or an identifier that exactly names a project function, handed as an argument of a promise handler call: a call whose callee the compiler
//! resolves to `Promise.then`, `Promise.catch`, `Promise.finally` or `PromiseLike.then` (before any veto: a project subclass or a patch only makes the call less known), or, with
//! no standard resolution, a member spelled `then`, `catch` or `finally` (a thenable of the project's, or a receiver whose type no fact proves: the text only adds sites, and
//! ends nothing). A callback a thenable runs at once is not after a suspension; treating it as one gives an uncertifiable row, never a violation.
//!
//! ADR 1003: a handler written another way is followed to the functions it may be ([`Resolver`]): both operands of a conditional or a logical expression, the tail of a
//! sequence, the target of `f.bind(…)`, a member of a project object (`handlers.ok`), a `const` to its initializer, the elements of an array literal that is spread, and a
//! parameter to the argument of each call the program names. A parameter of the executor of a standard `new Promise(…)` is the library's own `resolve` or `reject`. Where the
//! walk cannot name every function (a parameter: a caller the program does not name may hand any; a call's result; a member no project object decides), the handler
//! argument itself is a site that nothing ends: what runs after the suspension is not known.

use solid_facts::FileFacts;
use solid_facts::ast::{CallFact, RuntimeValueKind};
use solid_facts::core::Span;

use crate::HashMap;
use crate::HashSet;
use crate::pipeline::AnalysisContext;

/// The handler functions of the program by file path: the spans of the function literals and project functions handed to a promise handler call.
pub(super) type Handlers<'a> = HashMap<&'a str, HashSet<Span>>;

/// What the promise handler calls of the program run after a suspension.
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Found<'a> {
    /// The functions found.
    pub(super) functions: Handlers<'a>,
    /// The handler arguments whose functions the walk could not all name (ADR 1003), by file path, in source order.
    pub(super) unresolved: Vec<(&'a str, Span)>,
}

/// Every function the program hands to a promise handler call, outside dependency source. A debug build asks again without the per-file gate and requires the same answer
/// (the gate only skips files where [`is_handler_call`] can answer nothing).
pub(super) fn handler_functions<'a>(ctx: &AnalysisContext<'a>) -> Found<'a> {
    let found = collect(ctx, true);
    if cfg!(debug_assertions) {
        assert_eq!(
            found,
            collect(ctx, false),
            "the per-file gate of the promise handler scan skipped a handler call"
        );
    }
    found
}

fn collect<'a>(ctx: &AnalysisContext<'a>, gated: bool) -> Found<'a> {
    let lookup = ctx.semantic_lookup;
    let mut resolver = Resolver::new(ctx);
    let mut unresolved = Vec::new();
    for file in lookup.files() {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        // A file where no call can be a promise handler call holds none: such a call resolves to a standard promise handler or is a member spelled like one.
        if gated
            && !lookup.path_resolves_promise_handler(file.path.as_str())
            && !spells_handler_member(file)
        {
            continue;
        }
        for call in &file.ast.calls {
            if call.construct || !is_handler_call(ctx, file, call) {
                continue;
            }
            for argument in &call.arguments {
                let value = if argument.spread {
                    // The operand of the spread, not the `...` around it.
                    file.ast
                        .iterated_operands
                        .iter()
                        .copied()
                        .find(|operand| {
                            argument.span.contains(*operand) && operand.end == argument.span.end
                        })
                        .unwrap_or(argument.span)
                } else {
                    argument.value_span.unwrap_or(argument.span)
                };
                let named = if argument.spread {
                    resolver.spread(file, value, 0)
                } else {
                    // `then(undefined, onError)`: a nullish or primitive literal is no function.
                    matches!(
                        argument.runtime_value_kind,
                        RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
                    ) || resolver.value(file, value, 0)
                };
                if !named {
                    unresolved.push((file.path.as_str(), file.ast.peel_ts_sugar_span(value)));
                }
            }
        }
    }
    unresolved.sort_unstable_by_key(|(path, span)| (*path, span.start, span.end));
    unresolved.dedup();
    Found {
        functions: resolver.functions,
        unresolved,
    }
}

/// The hops one handler argument is followed through.
const MAX_DEPTH: usize = 8;

/// ADR 1003: the functions a handler argument may be.
struct Resolver<'c, 'a> {
    ctx: &'c AnalysisContext<'a>,
    functions: Handlers<'a>,
    /// Whether every function of the value at a span was named; `None` while the span is being walked (a cycle proves nothing).
    answered: HashMap<(&'a str, Span), Option<bool>>,
    /// A reference's declaration, by file.
    declarations: HashMap<&'a str, HashMap<Span, Span>>,
    /// The calls the program names of each project function, built at the first parameter that asks.
    callers: Option<CallerIndex<'a>>,
}

/// The calls of a project function (by file path and span) that name it: the file of each call and its index among the file's calls.
type CallerIndex<'a> = HashMap<(&'a str, Span), Vec<(&'a FileFacts, usize)>>;

impl<'c, 'a> Resolver<'c, 'a> {
    fn new(ctx: &'c AnalysisContext<'a>) -> Self {
        Self {
            ctx,
            functions: HashMap::default(),
            answered: HashMap::default(),
            declarations: HashMap::default(),
            callers: None,
        }
    }

    fn record(&mut self, file: &'a FileFacts, function: Span) {
        self.functions
            .entry(file.path.as_str())
            .or_default()
            .insert(function);
    }

    /// Records the functions the value at `span` may be; whether all of them were named.
    fn value(&mut self, file: &'a FileFacts, span: Span, depth: usize) -> bool {
        let span = file.ast.peel_ts_sugar_span(span);
        let key = (file.path.as_str(), span);
        // An answer already given. A span the walk is on (it came back to it) is not answered yet: nothing is proved of it, so the argument stays a site.
        if let Some(answer) = self.answered.get(&key) {
            return answer.unwrap_or(false);
        }
        if depth > MAX_DEPTH {
            return false;
        }
        self.answered.insert(key, None);
        let answer = self.walk(file, span, depth);
        self.answered.insert(key, Some(answer));
        answer
    }

    fn walk(&mut self, file: &'a FileFacts, span: Span, depth: usize) -> bool {
        let ast = &file.ast;
        let lookup = self.ctx.semantic_lookup;
        if let Some(function) = ast.function_at(span) {
            self.record(file, function.span);
            return true;
        }
        if let Some(conditional) = ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == span)
        {
            let consequent = self.value(file, conditional.consequent, depth + 1);
            let alternate = self.value(file, conditional.alternate, depth + 1);
            return consequent && alternate;
        }
        if let Some(logical) = ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == span)
        {
            let left = self.value(file, logical.left, depth + 1);
            let right = self.value(file, logical.right, depth + 1);
            return left && right;
        }
        if let Some(sequence) = ast.sequences.iter().find(|sequence| sequence.span == span) {
            return self.value(file, sequence.tail, depth + 1);
        }
        if let Some(call) = ast.call_at(span) {
            // `f.bind(receiver, …)`, where the compiler resolves the call to the standard `bind`, runs `f`; any other call's result is a function nothing names.
            let callee = ast.peel_ts_sugar_span(call.callee);
            let binds = !call.construct
                && matches!(
                    lookup.resolved_standard_call_name(file, callee),
                    Some("Function.bind" | "CallableFunction.bind" | "NewableFunction.bind")
                );
            return match ast.member_at(callee) {
                Some(member) if binds => self.value(file, member.object, depth + 1),
                Some(_) | None => false,
            };
        }
        if let Some(member) = ast.member_at(span) {
            // `handlers.ok`: the function the object literal a `const` holds writes under that key is one the member may be. Nothing here proves the object kept
            // it (a write through an alias, `Object.assign`), so the member is never all the walk names: the argument stays a site.
            if ast.computed_members.binary_search(&span).is_err()
                && let Some(key) = file.source_text(member.property)
                && let Some((literal_file, literal)) =
                    lookup.exact_initializer_at_reference(file, member.object)
            {
                let literal = literal_file.ast.peel_ts_sugar_span(literal);
                let values = literal_file
                    .ast
                    .object_properties
                    .iter()
                    .filter(|property| {
                        literal.contains(property.span)
                            && !property.computed
                            && literal_file.source_text(property.key) == Some(key)
                            // A property of a literal nested in this one is another literal's.
                            && !literal_file.ast.object_properties.iter().any(|outer| {
                                outer.span != property.span
                                    && literal.contains(outer.span)
                                    && outer.span.contains(property.span)
                            })
                    })
                    .map(|property| property.value)
                    .collect::<Vec<_>>();
                for value in values {
                    self.value(literal_file, value, depth + 1);
                }
            }
            return false;
        }
        if let Some((owner, function)) = lookup.function_called_at(file.path.as_str(), span) {
            self.record(owner, function.span);
            return true;
        }
        // A function declaration of this file the binder resolves the name to (no type fact is produced for an operand of a conditional), where nothing assigns the name.
        if let Some(function) = self.declared_function(file, span) {
            self.record(file, function);
            return true;
        }
        if let Some((initializer_file, initializer)) =
            lookup.exact_initializer_at_reference(file, span)
        {
            return self.value(initializer_file, initializer, depth + 1);
        }
        if let Some((function, index)) = self.parameter(file, span) {
            // The executor of a standard `new Promise(…)`: its parameters are the library's `resolve` and `reject`, which run no project function.
            if standard_promise_executor(self.ctx, file, function) {
                return true;
            }
            for (caller, call) in self.calls_of(file, function) {
                let call = &caller.ast.calls[call];
                if call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
                {
                    continue;
                }
                if let Some(argument) = call.arguments.get(index) {
                    self.value(
                        caller,
                        argument.value_span.unwrap_or(argument.span),
                        depth + 1,
                    );
                }
            }
            // A caller the program does not name (the function handed on as a value, an export) may hand any function.
            return false;
        }
        false
    }

    /// The functions of an argument written as a spread: the elements of an array literal, through a `const` that holds one.
    fn spread(&mut self, file: &'a FileFacts, span: Span, depth: usize) -> bool {
        let span = file.ast.peel_ts_sugar_span(span);
        if depth > MAX_DEPTH {
            return false;
        }
        if let Some(array) = file
            .ast
            .array_literal_elements
            .iter()
            .find(|array| array.span == span)
        {
            let mut named = true;
            for element in &array.elements {
                named &= element.kind == solid_facts::ast::ArrayElementKind::Value
                    && self.value(file, element.span, depth + 1);
            }
            return named;
        }
        match self
            .ctx
            .semantic_lookup
            .exact_initializer_at_reference(file, span)
        {
            Some((initializer_file, initializer)) => {
                self.spread(initializer_file, initializer, depth + 1)
            }
            None => false,
        }
    }

    /// The function declaration of `file` the reference at `span` names, where no assignment writes that name.
    fn declared_function(&mut self, file: &'a FileFacts, span: Span) -> Option<Span> {
        let declarations = self
            .declarations
            .entry(file.path.as_str())
            .or_insert_with(|| file.ast.reference_declarations.iter().copied().collect());
        let declaration = *declarations.get(&span)?;
        let function = file.ast.functions.iter().find(|function| {
            function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        })?;
        let written = file
            .ast
            .write_targets
            .iter()
            .map(|target| file.ast.peel_ts_sugar_span(*target))
            .any(|target| declarations.get(&target) == Some(&declaration));
        (!written).then_some(function.span)
    }

    /// The function `span` is a plain parameter of, and the parameter's index.
    fn parameter(&mut self, file: &'a FileFacts, span: Span) -> Option<(Span, usize)> {
        let declarations = self
            .declarations
            .entry(file.path.as_str())
            .or_insert_with(|| file.ast.reference_declarations.iter().copied().collect());
        let declaration = *declarations.get(&span)?;
        file.ast.functions.iter().find_map(|function| {
            function
                .parameters
                .iter()
                .position(|parameter| {
                    parameter.shape == solid_facts::ast::BindingShape::Identifier
                        && parameter.names.iter().any(|name| name.span == declaration)
                })
                .map(|index| (function.span, index))
        })
    }

    /// The calls the program names of the function at `function` of `file`.
    fn calls_of(&mut self, file: &'a FileFacts, function: Span) -> Vec<(&'a FileFacts, usize)> {
        let lookup = self.ctx.semantic_lookup;
        self.callers
            .get_or_insert_with(|| {
                let mut callers = CallerIndex::default();
                for caller in lookup.files() {
                    for (index, call) in caller.ast.calls.iter().enumerate() {
                        if call.construct {
                            continue;
                        }
                        if let Some((owner, called)) =
                            lookup.function_called_at(caller.path.as_str(), call.callee)
                        {
                            callers
                                .entry((owner.path.as_str(), called.span))
                                .or_default()
                                .push((caller, index));
                        }
                    }
                }
                callers
            })
            .get(&(file.path.as_str(), function))
            .cloned()
            .unwrap_or_default()
    }
}

/// Whether the function at `function` is written as the executor of a `new Promise(…)` the compiler resolves to the standard constructor.
fn standard_promise_executor(ctx: &AnalysisContext<'_>, file: &FileFacts, function: Span) -> bool {
    file.ast
        .arguments_containing(function)
        .any(|(call, index)| {
            index == 0
                && call.construct
                && !call.arguments[0].spread
                && file.ast.peel_ts_sugar_span(call.arguments[0].span) == function
                && ctx
                    .semantic_lookup
                    .resolved_standard_call_name(file, file.ast.peel_ts_sugar_span(call.callee))
                    == Some("PromiseConstructor.construct")
        })
}

/// Whether `call` is a promise handler call: resolved to a standard promise handler, or a member spelled like one where no standard declaration resolves.
fn is_handler_call(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if let Some(name) = ctx
        .semantic_lookup
        .resolved_standard_call_name(file, callee)
    {
        return crate::runtime_semantics::is_promise_handler_name(name);
    }
    file.ast
        .member_at(callee)
        .is_some_and(|member| spells_handler(file, member))
}

/// Whether the member's property is spelled like a promise handler (a thenable of the project's, or a receiver whose type no fact proves).
fn spells_handler(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> bool {
    matches!(
        file.source_text(member.property),
        Some("then" | "catch" | "finally")
    )
}

/// Whether some member of the file is spelled like a promise handler: the text arm of [`is_handler_call`] answers only for such a member.
fn spells_handler_member(file: &FileFacts) -> bool {
    file.ast
        .members
        .iter()
        .any(|member| spells_handler(file, member))
}

/// Whether the function whose own body holds `span` is a promise handler of the program.
pub(super) fn in_handler(handlers: &Handlers<'_>, file: &FileFacts, span: Span) -> bool {
    handlers.get(file.path.as_str()).is_some_and(|functions| {
        crate::owners::containing_ast_function(&file.ast, span)
            .is_some_and(|function| functions.contains(&function.span))
    })
}
