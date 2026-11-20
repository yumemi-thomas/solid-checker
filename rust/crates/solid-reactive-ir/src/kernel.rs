//! ADR 1120: the inertness kernel, IR half. Proofs ask these instead of judging inertness or keys themselves (proof-lint
//! `private-judgment`). The syntax half is [`solid_facts::kernel`]; this half discharges its operand obligations from the file's
//! binder facts, and from nothing else.
//!
//! - **K1** [`inert_evaluation`] / [`evaluates_no_user_code`]: evaluating the expression runs no project code. An identifier must
//!   name a binding the binder resolved in the file (an import counts: reading a live binding runs nothing), or be one of the
//!   global object's non-configurable values (`undefined`, `NaN`, `Infinity`), which no program can turn into accessors. An
//!   operand a coercing operator converts must be proved primitive: a `const` identifier binding whose initializer is itself an
//!   inert primitive evaluation, to a fixed depth. Types are not used: a TypeScript type is no evidence of a runtime primitive.
//! - **K3** [`property_key`]: the exact key a key's or member name's source denotes.

use solid_facts::FileFacts;
use solid_facts::core::Span;
use solid_facts::kernel::{OperandNeed, ValueKind};

/// How many `const` initializers a primitive obligation may follow.
const PRIMITIVE_DEPTH: usize = 4;

/// The global object's own properties that are non-configurable and non-writable data properties: a read of one runs nothing.
const IMMUTABLE_GLOBALS: [&str; 3] = ["undefined", "NaN", "Infinity"];

/// K1: the kind of value the expression at `span` yields, when evaluating it runs no project code; `None` otherwise.
#[must_use]
pub fn inert_evaluation(file: &FileFacts, span: Span) -> Option<ValueKind> {
    evaluate(file, span, PRIMITIVE_DEPTH)
}

/// K1: whether evaluating the expression at `span` runs no project code.
#[must_use]
pub fn evaluates_no_user_code(file: &FileFacts, span: Span) -> bool {
    inert_evaluation(file, span).is_some()
}

/// K3: the exact property key that the key or member name at `span` denotes (cooked, never trimmed text).
#[must_use]
pub fn property_key(file: &FileFacts, span: Span) -> Option<String> {
    solid_facts::kernel::cooked_key(file.source_text(span)?)
}

/// K5: whether the markup `text` (a cooked string) holds no script, no event attribute and no `javascript:` URL ([`solid_facts::kernel::markup_is_inert`]).
#[must_use]
pub fn markup_is_inert(text: &str) -> bool {
    solid_facts::kernel::markup_is_inert(text)
}

fn evaluate(file: &FileFacts, span: Span, depth: usize) -> Option<ValueKind> {
    let evaluation = solid_facts::kernel::evaluation(file.path.as_str(), &file.source, span)?;
    for obligation in &evaluation.obligations {
        let declaration = file.ast.reference_declaration(obligation.span);
        match (declaration, obligation.need) {
            (None, OperandNeed::Bound) if IMMUTABLE_GLOBALS.contains(&obligation.name.as_str()) => {
            }
            (None, _) => return None,
            (Some(_), OperandNeed::Bound) => {}
            (Some(declaration), OperandNeed::Primitive) => {
                if !primitive_constant(file, declaration, depth) {
                    return None;
                }
            }
        }
    }
    Some(evaluation.kind)
}

/// Whether the binding declared at `declaration` is a `const` identifier whose initializer evaluates inertly to a primitive.
fn primitive_constant(file: &FileFacts, declaration: Span, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    file.ast.bindings.iter().any(|binding| {
        binding.immutable
            && !binding.in_switch_case
            && matches!(binding.shape, solid_facts::ast::BindingShape::Identifier)
            && binding.names.iter().any(|name| name.span == declaration)
            && binding.initializer.is_some_and(|initializer| {
                evaluate(file, initializer, depth - 1) == Some(ValueKind::Primitive)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(source: &str) -> FileFacts {
        let ast = solid_facts::ast::extract("case.tsx", source).unwrap();
        let compiler = solid_facts::compiler::ExecutionMap {
            compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
            source_hash: ast.source.hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        FileFacts::new(
            solid_facts::core::Generation::new(1).unwrap(),
            source,
            ast,
            compiler,
        )
        .unwrap()
    }

    /// The span of the expression written after the marker `/*@*/` up to `/*$*/`.
    fn marked(source: &str) -> Span {
        let start = source.find("/*@*/").unwrap() + 5;
        let end = source.find("/*$*/").unwrap();
        Span::new(u32::try_from(start).unwrap(), u32::try_from(end).unwrap())
    }

    fn inert(source: &str) -> Option<ValueKind> {
        inert_evaluation(&file(source), marked(source))
    }

    #[test]
    fn a_coerced_operand_must_be_a_const_of_an_inert_primitive() {
        assert_eq!(
            inert("const n = 1; const m = n + 1; f(/*@*/`${m}`/*$*/);"),
            Some(ValueKind::Primitive)
        );
        assert_eq!(
            inert("const h = { toString() { return 'x'; } }; f(/*@*/`${h}`/*$*/);"),
            None
        );
        assert_eq!(
            inert("let n = 1; f(/*@*/n + 1/*$*/);"),
            None,
            "a let may hold an object later"
        );
        assert_eq!(
            inert("const n = g(); f(/*@*/n + 1/*$*/);"),
            None,
            "a call may return an object"
        );
        assert_eq!(
            inert("function k(n: number) { f(/*@*/n + 1/*$*/); }"),
            None,
            "a type is no evidence"
        );
        assert_eq!(inert("const [n] = [1]; f(/*@*/n + 1/*$*/);"), None);
    }

    #[test]
    fn a_read_must_name_a_binding_or_an_immutable_global() {
        assert_eq!(
            inert("const h = { toString() { return 'x'; } }; f(/*@*/h === 1/*$*/);"),
            Some(ValueKind::Primitive)
        );
        assert_eq!(
            inert("f(/*@*/x === undefined/*$*/);"),
            None,
            "x is a global: the global object may hold an accessor"
        );
        assert_eq!(
            inert("const x = 1; f(/*@*/x === undefined/*$*/);"),
            Some(ValueKind::Primitive)
        );
        assert_eq!(
            inert("import { y } from './y'; f(/*@*/!y/*$*/);"),
            Some(ValueKind::Primitive)
        );
    }

    #[test]
    fn keys_come_from_the_cooked_value() {
        let source = "o['_\\x24\\x24click']; o.f\\u006fo;";
        let facts = file(source);
        let at = |needle: &str| {
            let start = source.find(needle).unwrap();
            Span::new(
                u32::try_from(start).unwrap(),
                u32::try_from(start + needle.len()).unwrap(),
            )
        };
        assert_eq!(
            property_key(&facts, at("'_\\x24\\x24click'")).as_deref(),
            Some("_$$click")
        );
        assert_eq!(
            property_key(&facts, at("f\\u006fo")).as_deref(),
            Some("foo")
        );
    }
}
