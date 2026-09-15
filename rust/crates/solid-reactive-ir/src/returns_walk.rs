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
use solid_facts::ast::FunctionFact;

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
    if function.r#async {
        return Err(ReturnsDecline::Async);
    }
    if function.generator {
        return Err(ReturnsDecline::Generator);
    }
    if function.expression_body {
        return Err(ReturnsDecline::ExpressionBody);
    }
    let body = function.body;
    for returned in &file.ast.returns {
        if returned.span.start < body.start || returned.span.end > body.end {
            continue;
        }
        // Owned by a nested callable: some *other* function whose span lies
        // inside this body and *strictly* contains the return fact. Strict,
        // because a return fact's span is its argument's span when it has one:
        // `return () => …` yields a fact whose span *is* the arrow's, and that
        // arrow does not own the return — it is the value returned.
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
        if returned.argument.is_some() {
            return Err(ReturnsDecline::ValueReturn {
                start: returned.span.start,
                end: returned.span.end,
            });
        }
    }
    Ok(())
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
