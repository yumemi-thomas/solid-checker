//! The generator's valueless-completion walk: whether an export's own
//! implementation may *propose* `returns: []` (ADR 0035).
//!
//! A **proposal** input, never a proof. `returns: [] closed` denies that one
//! invocation yields a value to its caller, and the semantic model fixes that a
//! valueless completion — a bare `return;`, a body that falls off its end — is
//! not a `return` operation (`docs/package-contract-v2/semantic-model.md`
//! § returns). Only the certifier's implementation census against
//! authenticated bytes may close the domain; what this walk decides is the
//! weaker, earlier question: has the generator's own syntax seen anything a
//! `returns: []` proposal would contradict?
//!
//! Everything here fails **closed**, and silence is "do not propose":
//!
//! * an `async` function hands its caller a promise on every completion, a
//!   generator an iterator — whatever the body does — so both decline;
//! * an expression-bodied arrow completes with its expression, so it declines;
//! * a `return` statement carrying an expression, anywhere in the function's
//!   *own* body, declines. A return inside a nested function declaration,
//!   function expression or arrow is that callable's completion, not this
//!   one's, and is skipped — its value reaches the caller only if this
//!   function returns it, which is then a return of its own with an
//!   expression. A return inside a construct these facts do not list as a
//!   function (a class or object-literal method) is attributed to this
//!   function, which can only over-decline.
//!
//! The census re-asks every one of these against the producer's control-flow
//! census, which also decides reachability — a value-carrying return the
//! producer proves unreachable is admitted there and declined here, because
//! this walk has no reachability and does not guess at one.

use solid_facts::FileFacts;
use solid_facts::ast::{AstFacts, FunctionFact, ReturnFact, ReturnValueKind};

/// Why the valueless-completion walk declined to propose `returns: []`.
///
/// Measurement only: nothing in a contract document is decided from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReturnsDecline {
    /// The implementation is an `async` function.
    Async,
    /// The implementation is a generator.
    Generator,
    /// The implementation is an expression-bodied arrow.
    ExpressionBody,
    /// A `return` statement in the function's own body carries an expression,
    /// at this byte range.
    ValueReturn { start: u32, end: u32 },
}

impl ReturnsDecline {
    /// The stable spelling of this decline, for reports.
    #[must_use]
    pub fn spelling(&self) -> &'static str {
        match self {
            Self::Async => "async",
            Self::Generator => "generator",
            Self::ExpressionBody => "expression-body",
            Self::ValueReturn { .. } => "value-return",
        }
    }
}

/// Whether `function` in `file` completes without yielding a value on every
/// path the generator's syntax facts can see. `Ok(())` is the positive answer
/// the `returns: []` proposal needs; `Err` names the first blocker.
pub fn valueless_completion(
    file: &FileFacts,
    function: &FunctionFact,
) -> Result<(), ReturnsDecline> {
    valueless_completion_in(&file.ast, function)
}

fn valueless_completion_in(ast: &AstFacts, function: &FunctionFact) -> Result<(), ReturnsDecline> {
    if function.r#async {
        return Err(ReturnsDecline::Async);
    }
    if function.generator {
        return Err(ReturnsDecline::Generator);
    }
    if function.expression_body {
        return Err(ReturnsDecline::ExpressionBody);
    }
    if let Some(returned) = own_returns(ast, function).find(|returned| returned.argument.is_some())
    {
        return Err(ReturnsDecline::ValueReturn {
            start: returned.span.start,
            end: returned.span.end,
        });
    }
    Ok(())
}

/// The generator's **value completion** walk (ADR 0113): whether a plain
/// function may propose `returns` closed over one `plain` return.
///
/// A proposal input, never a proof, exactly as [`valueless_completion`] is. The
/// certifier's census decides whether every completion *is* a primitive, from
/// the producer's types; what this decides is the earlier question, whether
/// there is anything for that census to decide. The positive answer needs the
/// valueless walk to have declined on a value -- a `return` carrying an
/// expression, or an expression body -- and no completion of the function's own
/// whose syntax already rules a primitive out: a function or arrow literal, or
/// an array or object literal the facts record an element or property of. Each
/// of those is an object on every run, so proposing over it would only spend a
/// census refusal. Everything else -- a call, an identifier, a member, an
/// operator, a literal the facts do not break down (`{}`, `[]`, a class
/// expression, `new`) -- is left to the census.
///
/// Silence is "do not propose": an `async` function or a generator hands its
/// caller a promise or an iterator whatever the body returns, and a body the
/// valueless walk cleared is ADR 0035's `returns: []`, never a plain return.
pub fn value_completion(file: &FileFacts, function: &FunctionFact) -> bool {
    value_completion_in(&file.ast, function)
}

fn value_completion_in(ast: &AstFacts, function: &FunctionFact) -> bool {
    match valueless_completion_in(ast, function) {
        Ok(()) | Err(ReturnsDecline::Async | ReturnsDecline::Generator) => false,
        Err(ReturnsDecline::ExpressionBody) => function
            .expression_return
            .as_ref()
            .is_some_and(|returned| !never_primitive(returned)),
        Err(ReturnsDecline::ValueReturn { .. }) => {
            own_returns(ast, function).all(|returned| !never_primitive(returned))
        }
    }
}

/// Whether a return's own syntax hands back an object on every run.
fn never_primitive(returned: &ReturnFact) -> bool {
    returned.value == ReturnValueKind::Function || returned.structure.is_some()
}

/// The return facts `function`'s *own* body writes, in source order.
fn own_returns<'a>(
    ast: &'a AstFacts,
    function: &'a FunctionFact,
) -> impl Iterator<Item = &'a ReturnFact> + 'a {
    let body = function.body;
    ast.returns.iter().filter(move |returned| {
        if returned.span.start < body.start || returned.span.end > body.end {
            return false;
        }
        // Owned by a nested callable: some *other* function whose span lies
        // inside this body and *strictly* contains the return fact. Strict,
        // because a return fact's span is its argument's span when it has one:
        // `return () => …` yields a fact whose span *is* the arrow's, and that
        // arrow does not own the return — it is the value returned.
        !ast.functions.iter().any(|other| {
            other.span != function.span
                && other.span.start >= body.start
                && other.span.end <= body.end
                && (other.span.start < returned.span.start || returned.span.end < other.span.end)
                && other.span.start <= returned.span.start
                && returned.span.end <= other.span.end
        })
    })
}

/// One value an export's return hands back that is its caller's own argument,
/// or a fresh array of its caller's arguments (ADR 0115).
///
/// The certifier reads the same two answers off the producer's arms of each
/// return; this is the vocabulary both sides and the contract share.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArgumentContainer {
    /// The caller's argument at this index, itself.
    Parameter(u16),
    /// A fresh array whose elements, in order, are the caller's arguments at
    /// these indices; empty for `[]`.
    Array(Vec<u16>),
    /// What an invocation of the caller's argument at this index returned
    /// (ADR 0116).
    Invocation(u16),
    /// The value the caller's argument at this index holds at this one
    /// property key when the return reads it (item B round 2 of
    /// ways-to-improve § 3.3): `event.defaultPrevented` is
    /// `Member(0, "defaultPrevented")`, and `handler[0]` is key `"0"`, the key
    /// ToPropertyKey gives the access. Read at return time, so whatever the
    /// call ran before it -- its own body, or a callback it invoked -- may have
    /// put it there.
    Member(u16, String),
    /// Exactly `undefined`: an optional chain's other value (`p?.key` when
    /// `p` is nullish). Not an argument container; it is enumerated beside
    /// them because the same census decides it from the same arms.
    Undefined,
}

impl ArgumentContainer {
    /// The container a return's output names, if it names one.
    #[must_use]
    pub fn of(output: &crate::contract_semantics::ValueShape) -> Option<Self> {
        match output {
            crate::contract_semantics::ValueShape::Parameter { index, path } if path.is_empty() => {
                Some(Self::Parameter(*index))
            }
            crate::contract_semantics::ValueShape::ArgumentArray { items } => {
                Some(Self::Array(items.clone()))
            }
            crate::contract_semantics::ValueShape::InvocationResult { parameter } => {
                Some(Self::Invocation(*parameter))
            }
            crate::contract_semantics::ValueShape::Parameter { index, path } => {
                match path.as_slice() {
                    [key] => Some(Self::Member(*index, key.clone())),
                    _ => None,
                }
            }
            crate::contract_semantics::ValueShape::Undefined => Some(Self::Undefined),
            _ => None,
        }
    }

    /// The output shape a return handing back this container states.
    #[must_use]
    pub fn value_shape(&self) -> crate::contract_semantics::ValueShape {
        match self {
            Self::Parameter(index) => crate::contract_semantics::ValueShape::Parameter {
                index: *index,
                path: Vec::new(),
            },
            Self::Array(items) => crate::contract_semantics::ValueShape::ArgumentArray {
                items: items.clone(),
            },
            Self::Invocation(parameter) => {
                crate::contract_semantics::ValueShape::InvocationResult {
                    parameter: *parameter,
                }
            }
            Self::Member(index, key) => crate::contract_semantics::ValueShape::Parameter {
                index: *index,
                path: vec![key.clone()],
            },
            Self::Undefined => crate::contract_semantics::ValueShape::Undefined,
        }
    }

    /// A stable spelling, for census sites and refusal text.
    #[must_use]
    pub fn spelling(&self) -> String {
        match self {
            Self::Parameter(index) => format!("parameter-{index}"),
            Self::Array(items) => format!(
                "array[{}]",
                items
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::Invocation(index) => format!("invocation-{index}"),
            Self::Member(index, key) => format!("parameter-{index}[{key:?}]"),
            Self::Undefined => "undefined".into(),
        }
    }
}

/// The generator's **argument container** walk (ADR 0115): which of the
/// caller's own arguments, and fresh arrays of them, this function's
/// completions hand back.
///
/// A proposal input, never a proof, exactly as [`valueless_completion`] is:
/// the certifier re-asks it against the producer's own arms of each return,
/// which also decide reachability and whether a parameter is ever written.
/// What this decides is the earlier question -- is the function's own syntax
/// nothing but conditionals whose branches are its whole parameters and array
/// literals of them?
///
/// A completion, or a branch, may also be a call of one of those parameters,
/// whose value is what the invocation returned (ADR 0116), or a non-call read
/// of one literal member of one (item B round 2 of ways-to-improve § 3.3):
/// `p.key`, `p[0]` and `p["key"]` are that member, and `p?.key` that member
/// or `undefined`. A longer path, a computed key the facts do not name, and a
/// member of anything but a whole parameter are not read.
///
/// Silence is "do not propose", and every path out is `None`: an `async`
/// function or a generator; a completion, or a branch, that is anything else,
/// including a spread or a hole in an array and a `new`; a conditional deeper
/// than the producer decomposes; and one whole parameter alone, which is
/// ADR 0075's claim. A bare `return;` hands back `undefined`, which is no
/// `return` operation, and contributes nothing.
#[must_use]
pub(crate) fn argument_container_return(
    file: &FileFacts,
    function: &FunctionFact,
    entities: &crate::EntitySymbols,
) -> Option<Vec<ArgumentContainer>> {
    if function.r#async || function.generator {
        return None;
    }
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.shape == solid_facts::ast::BindingShape::Identifier)
        .filter_map(|(index, parameter)| {
            let name = parameter.names.first()?;
            let symbol = entities.get(&crate::location(file.path.shared(), name.span))?;
            Some((symbol.clone(), u16::try_from(index).ok()?))
        })
        .collect::<std::collections::HashMap<_, _>>();
    if parameters.is_empty() {
        return None;
    }
    let parameter_at = |span: solid_facts::core::Span| {
        entities
            .get(&crate::location(file.path.shared(), span))
            .and_then(|symbol| parameters.get(symbol))
            .copied()
    };
    let mut containers = std::collections::BTreeSet::new();
    let mut pending = Vec::new();
    // A returned array literal is read off its own return fact, whose elements
    // the facts record when there is at least one; `return []` records none,
    // and is not proposed.
    let mut completion = |returned: &ReturnFact, span: solid_facts::core::Span| {
        if !returned.elements().is_empty() && returned.properties().is_empty() {
            let items = returned
                .elements()
                .iter()
                .map(|element| element.and_then(parameter_at))
                .collect::<Option<Vec<_>>>()?;
            containers.insert(ArgumentContainer::Array(items));
        } else {
            pending.push((span, 0usize));
        }
        Some(())
    };
    let mut any = false;
    if let Some(expression) = function.expression_return.as_ref() {
        completion(expression, expression.span)?;
        any = true;
    }
    for returned in own_returns(&file.ast, function) {
        if let Some(argument) = returned.argument {
            completion(returned, argument)?;
            any = true;
        }
    }
    if !any {
        return None;
    }
    while let Some((span, depth)) = pending.pop() {
        // The producer's own bound: past it the producer states no arms, and
        // the census could only refuse.
        if depth > 8 || containers.len() > 16 {
            return None;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == span)
        {
            for (branch, array) in [
                (
                    conditional.consequent,
                    conditional.consequent_array.as_deref(),
                ),
                (
                    conditional.alternate,
                    conditional.alternate_array.as_deref(),
                ),
            ] {
                match array {
                    Some(elements) => {
                        let items = elements
                            .iter()
                            .map(|element| element.and_then(parameter_at))
                            .collect::<Option<Vec<_>>>()?;
                        containers.insert(ArgumentContainer::Array(items));
                    }
                    None => pending.push((branch, depth + 1)),
                }
            }
            continue;
        }
        // ADR 0116: a call of the caller's own argument hands back whatever
        // that invocation returned. `new` starts before its callee, and an
        // optional call reaches the census, which refuses it by name.
        if let Some(call) = file.ast.calls.iter().find(|call| call.span == span) {
            if !call.direct_callee || call.callee.start != call.span.start {
                return None;
            }
            containers.insert(ArgumentContainer::Invocation(parameter_at(call.callee)?));
            continue;
        }
        // Item B round 2: a read of one literal member of the caller's own
        // argument. The key is the property's own name, or the literal key
        // the facts name for a computed access (the one ToPropertyKey gives
        // it, as the producer roots it); an optional link adds `undefined`.
        if let Some(member) = file.ast.members.iter().find(|member| member.span == span) {
            // The receiver is the parameter's own identifier, nothing longer:
            // `options.inner.key`'s receiver is a member too, and an entity at
            // its span is not the parameter's binding.
            if !file.ast.identifiers.iter().any(|identifier| {
                identifier.span == member.object
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            }) {
                return None;
            }
            let index = parameter_at(member.object)?;
            let key = if file
                .ast
                .computed_members
                .binary_search(&member.span)
                .is_ok()
            {
                let position = file
                    .ast
                    .literal_computed_members
                    .binary_search_by_key(&member.span, |fact| fact.span)
                    .ok()?;
                file.ast.literal_computed_members[position].key.to_string()
            } else {
                file.source_text(member.property)?.to_owned()
            };
            containers.insert(ArgumentContainer::Member(index, key));
            if file
                .ast
                .optional_members
                .binary_search(&member.span)
                .is_ok()
            {
                containers.insert(ArgumentContainer::Undefined);
            }
            continue;
        }
        containers.insert(ArgumentContainer::Parameter(parameter_at(span)?));
    }
    // One whole parameter alone is ADR 0075's claim; one array, or one
    // invocation, is this one's.
    match containers.len() {
        0 => None,
        1 if matches!(containers.first(), Some(ArgumentContainer::Parameter(_))) => None,
        _ => Some(containers.into_iter().collect()),
    }
}

/// Every function in the project whose completions [`argument_container_return`]
/// cleared, by file and span (ADR 0115). Built with the analysis in hand, as
/// [`MergedPropsReturns`] is, because the walk resolves parameters by symbol.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArgumentContainerReturns {
    by_file: std::collections::BTreeMap<String, Vec<ArgumentContainerReturnRow>>,
}

type ArgumentContainerReturnRow = ((u32, u32), Vec<ArgumentContainer>);

impl ArgumentContainerReturns {
    /// The containers the function at `span` hands back, when the walk cleared
    /// it.
    #[must_use]
    pub fn containers_for(&self, path: &str, span: (u64, u64)) -> Option<&[ArgumentContainer]> {
        let span = (u32::try_from(span.0).ok()?, u32::try_from(span.1).ok()?);
        self.by_file
            .get(path)?
            .iter()
            .find(|(function, _)| *function == span)
            .map(|(_, containers)| containers.as_slice())
    }
}

pub(crate) fn collect_argument_container_returns(
    ctx: &crate::pipeline::AnalysisContext<'_>,
) -> ArgumentContainerReturns {
    let mut by_file = std::collections::BTreeMap::<String, Vec<_>>::new();
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            if let Some(containers) = argument_container_return(file, function, ctx.entities) {
                by_file
                    .entry(file.path.to_string())
                    .or_default()
                    .push(((function.span.start, function.span.end), containers));
            }
        }
    }
    ArgumentContainerReturns { by_file }
}

/// The generator's **merged props return** walk (ADR 0109): which of this
/// export's own parameters a props merge it returns carries the reactivity of.
///
/// A proposal input, never a proof, exactly as [`valueless_completion`] is. The
/// certifier re-asks every question here against the producer's control-flow
/// census and its resolved call census, where reachability and exact callee
/// identity live; what this decides is the earlier one — has the generator's
/// own syntax seen a body that is nothing but a props merge over one of its
/// parameters?
///
/// Silence is "do not propose", and every path out is `None`:
///
/// * an `async` function or a generator hands its caller a promise or an
///   iterator, not a props object;
/// * a completion that is not a call of a primitive the dialect's
///   [`solid_dialect::Dialect::merges_props_reactivity`] row names — including
///   a bare `return;` and a body that can fall off its end;
/// * a merge with no whole-parameter argument, or with more than one: this
///   shape names a single argument and choosing between two is not the
///   generator's call;
/// * two completions that name different parameters.
///
/// A body with *no* completion at all yields `None` too: that function returns
/// `undefined`, which ADR 0035's empty closure describes and this shape does
/// not.
#[must_use]
pub(crate) fn merged_props_return(
    file: &FileFacts,
    function: &FunctionFact,
    entities: &crate::EntitySymbols,
    symbol_names: &std::collections::HashMap<crate::SymbolId, crate::SymbolName>,
    dialect: &dyn solid_dialect::Dialect,
) -> Option<usize> {
    if function.r#async || function.generator {
        return None;
    }
    // The parameter bindings this export declares, by symbol. A destructured or
    // rest parameter has no single whole-parameter identity, so it contributes
    // none and an argument rooted at it can never match.
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.shape == solid_facts::ast::BindingShape::Identifier)
        .filter_map(|(index, parameter)| {
            let name = parameter.names.first()?;
            let symbol = entities.get(&crate::location(file.path.shared(), name.span))?;
            Some((symbol.clone(), index))
        })
        .collect::<std::collections::HashMap<_, _>>();
    if parameters.is_empty() {
        return None;
    }

    let body = function.body;
    let mut completions = Vec::new();
    if let Some(expression) = function.expression_return.as_ref() {
        completions.push(expression.span);
    }
    for returned in &file.ast.returns {
        if returned.span.start < body.start || returned.span.end > body.end {
            continue;
        }
        // The same nesting rule `valueless_completion` applies, and for the
        // same reason: a return inside a nested callable is that callable's
        // completion, not this one's.
        let nested = file.ast.functions.iter().any(|other| {
            other.span != function.span
                && other.span.start >= body.start
                && other.span.end <= body.end
                && (other.span.start < returned.span.start || returned.span.end < other.span.end)
                && other.span.start <= returned.span.start
                && returned.span.end <= other.span.end
        });
        if nested {
            continue;
        }
        // A bare completion yields `undefined`, which contradicts the claim.
        completions.push(returned.argument?);
    }
    if completions.is_empty() {
        return None;
    }

    let mut agreed = None::<usize>;
    for completion in completions {
        let call = file.ast.calls.iter().find(|call| call.span == completion)?;
        let primitive = crate::known_primitive(&crate::primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        ))?;
        if !dialect.merges_props_reactivity(primitive) {
            return None;
        }
        let mut rooted = call
            .arguments
            .iter()
            .filter(|argument| {
                !argument.spread
                    && argument.value == solid_facts::ast::ArgumentValueKind::Identifier
            })
            .filter_map(|argument| {
                entities
                    .get(&crate::location(file.path.shared(), argument.span))
                    .and_then(|symbol| parameters.get(symbol))
                    .copied()
            });
        let index = rooted.next()?;
        if rooted.next().is_some() {
            return None;
        }
        match agreed {
            Some(agreed) if agreed != index => return None,
            _ => agreed = Some(index),
        }
    }
    agreed
}

/// Every function in the project whose body is nothing but a props merge over
/// one of its own parameters, by file and span (ADR 0109).
///
/// Built once with the analysis in hand, because the walk has to resolve a
/// callee to a dialect primitive and that needs the entity and symbol tables.
/// Read at the emit boundary by the same two identities `creates_walk_clean`
/// and `returns_walk_clean` are read by.
///
/// **A proposal input.** An absent entry is "do not propose", which is also
/// what an unanalysed function has.
/// One cleared function: its span in its file, and the parameter its merge
/// carries the reactivity of.
type MergedPropsReturnRow = ((u32, u32), usize);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MergedPropsReturns {
    by_file: std::collections::BTreeMap<String, Vec<MergedPropsReturnRow>>,
}

impl MergedPropsReturns {
    /// The parameter a props merge returned by the function at `span` carries
    /// the reactivity of, when this walk cleared it.
    #[must_use]
    pub fn parameter_for(&self, path: &str, span: (u64, u64)) -> Option<usize> {
        let span = (u32::try_from(span.0).ok()?, u32::try_from(span.1).ok()?);
        self.by_file
            .get(path)?
            .iter()
            .find(|(function, _)| *function == span)
            .map(|(_, parameter)| *parameter)
    }
}

pub(crate) fn collect_merged_props_returns(
    ctx: &crate::pipeline::AnalysisContext<'_>,
) -> MergedPropsReturns {
    let mut by_file = std::collections::BTreeMap::<String, Vec<MergedPropsReturnRow>>::new();
    for file in &ctx.facts.files {
        for function in &file.ast.functions {
            if let Some(parameter) =
                merged_props_return(file, function, ctx.entities, ctx.symbol_names, ctx.dialect)
            {
                by_file
                    .entry(file.path.to_string())
                    .or_default()
                    .push(((function.span.start, function.span.end), parameter));
            }
        }
    }
    MergedPropsReturns { by_file }
}

#[cfg(test)]
mod tests {
    use super::{ReturnsDecline, value_completion_in, valueless_completion_in};
    use solid_facts::ast;

    /// Both walks' answers for the outermost function `source` declares.
    fn answers(source: &str) -> (Result<(), ReturnsDecline>, bool) {
        let facts = ast::extract("test.js", source).unwrap();
        let function = facts
            .functions
            .iter()
            .min_by_key(|function| (function.span.start, std::cmp::Reverse(function.span.end)))
            .expect("the source declares a function");
        (
            valueless_completion_in(&facts, function),
            value_completion_in(&facts, function),
        )
    }

    /// ADR 0113: the value completion walk proposes exactly where the
    /// valueless walk declined on a value and no own completion is a literal
    /// that is an object on every run. Everything it cannot rule out syntactically
    /// is the census's to decide.
    #[test]
    fn a_value_completion_is_proposed_unless_its_syntax_rules_a_primitive_out() {
        for source in [
            "function f() { return 1; }",
            "const f = () => true;",
            "const f = () => void 0;",
            "function f(value, min, max) { return Math.min(Math.max(value, min), max); }",
            "function f(value) { return value !== null && typeof value === 'object'; }",
            "function f(flag) { if (flag) { return; } return flag ? 1 : -1; }",
            // A nested callable's completion is not this function's.
            "function f() { function inner() { return {}; } return inner.length; }",
            // Not ruled out by syntax, so the census decides -- and refuses.
            "function f(value) { return value; }",
            "function f() { return {}; }",
            "function f() { return new Map(); }",
        ] {
            let (valueless, value) = answers(source);
            assert!(valueless.is_err(), "{source}");
            assert!(value, "{source}: the census decides");
        }
        for source in [
            "function f() { return () => 1; }",
            "function f() { return function () {}; }",
            "const f = () => () => 1;",
            "function f(value) { return { value }; }",
            "function f(value) { return [value]; }",
            "const f = (value) => ({ value });",
            // One completion that is certainly an object is one too many.
            "function f(flag) { if (flag) { return 1; } return () => 1; }",
        ] {
            let (valueless, value) = answers(source);
            assert!(valueless.is_err(), "{source}");
            assert!(!value, "{source}: syntax already rules a primitive out");
        }
        // A valueless body is ADR 0035's `returns: []`, and an `async` function
        // or a generator hands back a promise or an iterator: none proposes.
        assert_eq!(answers("function f() {}"), (Ok(()), false));
        assert_eq!(
            answers("function f(flag) { if (flag) { return; } }"),
            (Ok(()), false)
        );
        assert_eq!(
            answers("async function f() { return 1; }"),
            (Err(ReturnsDecline::Async), false)
        );
        assert_eq!(
            answers("function* f() { return 1; }"),
            (Err(ReturnsDecline::Generator), false)
        );
    }
}
