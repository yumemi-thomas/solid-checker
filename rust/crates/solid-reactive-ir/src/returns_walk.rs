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
