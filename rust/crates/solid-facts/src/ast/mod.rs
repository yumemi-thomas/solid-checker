//! Oxc-derived structural facts.
//!
//! This crate is intentionally checker-independent and regex-free. It parses
//! original source once and exports finite, deterministic tables. Consumers
//! join these spans with tsrs semantic facts; Oxc nodes never escape.
//!
//! Module map (files under `src/ast/`):
//!
//! | Module | Responsibility |
//! | --- | --- |
//! | `types` | The fact schema: every fact struct and enum, `AstFacts` and its accessors, `AstFactsError`. |
//! | `extract` | `extract` and the `Collector` state that builds `AstFacts` from one parse. |
//! | `visitor` | The oxc `Visit` implementation that records facts as the walk proceeds. |
//! | `extract_helpers` | Free helpers: expression peeling, binding and key helpers, export names, chain parts. |
//! | `span_index` | `AstSpanIndex` and `LazySpanIndex`, span-keyed lookups over the fact tables. |
//! | `emission` | Whether a module's exact bytes emit any JavaScript (`module_emission`). |
//! | `host_constants`, `host_execution`, `host_call_completion` | Host constant folding, host-execution facts, bounded synchronous host call completion. |
//! | `argument_completion`, `completion_call_cover`, `primitive_completion` | Completion standings for arguments, calls and primitives. |
//! | `component_value_flow`, `proxy_get_forward`, `literal_array` | Component value flow, proxy Get forwarding, literal array elements. |
//! | `unconditional_calls`, `module_requests`, `inert_javascript` | Synchronously unconditional calls, static module requests, inert JavaScript modules. |
//!
//! `mod.rs` keeps the submodule list and re-exports; fact types live in
//! `types.rs`, and unit tests in `tests.rs` and per-module `tests.rs` files.

use crate::core::{SourceIdentity, Span};
use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionExpression, AssignmentExpression,
    AssignmentTarget, AwaitExpression, BinaryExpression, BindingPattern, CallExpression,
    ComputedMemberExpression, ConditionalExpression, Declaration, ExportAllDeclaration,
    ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportNamedDeclaration, Expression,
    FormalParameter, Function, FunctionType, IdentifierReference, IfStatement, ImportDeclaration,
    ImportDeclarationSpecifier, ImportExpression, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXElement, JSXElementName, JSXExpression, LogicalExpression,
    LogicalOperator, ModuleExportName, NewExpression, ObjectProperty, ObjectPropertyKind,
    PropertyKey, PropertyKind, ReturnStatement, SpreadElement, StaticMemberExpression,
    TSGlobalDeclaration, TSImportEqualsDeclaration, TSImportType, TSImportTypeQualifier,
    TSModuleBlock, TSModuleDeclaration, TSModuleDeclarationName, TSModuleReference, TSTypeQuery,
    UnaryExpression, UpdateExpression, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{ScopeId, Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span as OxcSpan};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const AST_FACTS_SCHEMA: u32 = 74;

mod argument_completion;
mod completion_call_cover;
mod component_value_flow;
mod emission;
mod host_call_completion;
mod host_constants;
mod host_execution;
mod inert_javascript;
mod literal_array;
mod module_requests;
mod primitive_completion;
mod proxy_get_forward;
mod span_index;
mod unconditional_calls;

mod types;
pub use types::*;
mod extract;
pub use extract::*;
mod extract_helpers;
pub use extract_helpers::*;
mod visitor;

pub use argument_completion::{
    ParameterValue, SiteStanding, argument_site_standings, inert_parameter_references,
    parameter_value_uses,
};
pub use completion_call_cover::{
    completion_call_cover, completion_call_cover_including_throws, completion_return_cover,
};
pub use component_value_flow::{ComponentPropSite, ComponentValueFlow, component_value_flows};
pub use host_constants::{
    HostConstantFold, HostConstantImport, HostConstantScope, WindowRealm,
    exported_boolean_constant, fold_host_constant_branches,
    fold_host_constant_branches_in_window_realm, module_load_specifiers, named_value_imports,
};
pub use host_execution::{
    HostCallSequence, HostConstantIdentity, HostDefaultInitializer, HostExecutionFact,
    HostExecutionPredicate, HostExecutionSiteKind,
};
pub use inert_javascript::{
    InertJavaScriptModule, InertJavaScriptRefusal, inert_javascript_module,
    inert_javascript_module_with_export_all,
};
pub use literal_array::{literal_array_element, this_expression_spans};
pub use module_requests::static_module_requests;
pub use primitive_completion::primitive_completion_by_syntax;
pub use proxy_get_forward::proxy_get_forward_target;
pub use unconditional_calls::{synchronously_unconditional_calls, unconditional_calls};

pub use emission::{
    EmittingStatement, ModuleEmission, ModuleEmissionError, ModuleFlavor, has_export_assignment,
    module_emission,
};
pub use span_index::{AstSpanIndex, LazySpanIndex};

#[cfg(test)]
mod tests;

/// Characterization pins for `own_returns`, fixed at the baseline before the
/// query moved here from the generator. Every expectation is a literal source slice: the
/// function is selected by the exact text of its span, and each expected return
/// is the text of its `ReturnFact` span, in the order the table holds them.
#[cfg(test)]
mod own_return_pins {
    use crate::ast::{AstFacts, FunctionFact, extract};

    fn function_at<'a>(facts: &'a AstFacts, source: &str, span_text: &str) -> &'a FunctionFact {
        let mut found = facts.functions.iter().filter(|function| {
            &source[function.span.start as usize..function.span.end as usize] == span_text
        });
        let function = found
            .next()
            .unwrap_or_else(|| panic!("no function spans {span_text:?}"));
        assert!(found.next().is_none(), "two functions span {span_text:?}");
        function
    }

    fn own<'s>(source: &'s str, function_text: &str) -> Vec<&'s str> {
        let facts = extract("pins.ts", source).unwrap();
        let function = function_at(&facts, source, function_text);
        facts
            .own_returns(function)
            .map(|returned| &source[returned.span.start as usize..returned.span.end as usize])
            .collect()
    }

    // P1: a value return is its argument's span, a bare return its statement's;
    // a nested declaration's return is skipped even between two outer returns.
    #[test]
    fn flat_returns_keep_source_order_and_skip_a_nested_declaration() {
        let source =
            "function f(a){ if (a) return 1; return 2; function g(){ return 3; } return; }";
        assert_eq!(own(source, source), ["1", "2", "return;"]);
        assert_eq!(own(source, "function g(){ return 3; }"), ["3"]);
    }

    // P2: nested declaration, function expression and block-bodied arrow.
    #[test]
    fn nested_declarations_expressions_and_block_arrows_own_their_returns() {
        let source = "function outer(){ function d(){ return 1; } const e = function(){ return 2; }; const a = () => { return 3; }; return 4; }";
        assert_eq!(own(source, source), ["4"]);
        assert_eq!(own(source, "function d(){ return 1; }"), ["1"]);
        assert_eq!(own(source, "function(){ return 2; }"), ["2"]);
        assert_eq!(own(source, "() => { return 3; }"), ["3"]);
    }

    // P3: a returned callable is the outer function's value although its span
    // equals the fact's span (equal is not strictly nested); the callable's own
    // body return stays its own, and the callable owns neither.
    #[test]
    fn a_returned_callable_belongs_to_the_outer_function_not_to_itself() {
        let arrow = "function outer(){ return () => 4; }";
        assert_eq!(own(arrow, arrow), ["() => 4"]);
        assert!(own(arrow, "() => 4").is_empty());
        let expression = "function outer2(){ return function(){ return 1; }; }";
        assert_eq!(own(expression, expression), ["function(){ return 1; }"]);
        assert_eq!(own(expression, "function(){ return 1; }"), ["1"]);
    }

    // P4: an expression-bodied arrow completes with its expression, which is not
    // in `ast.returns`; a function with no `return` owns none.
    #[test]
    fn expression_completions_and_returnless_functions_own_nothing() {
        let source = "const x = () => y; function n(){ const z = 1; }";
        assert!(own(source, "() => y").is_empty());
        assert!(own(source, "function n(){ const z = 1; }").is_empty());
    }

    // P5: constructors are executable functions.
    #[test]
    fn a_constructor_owns_its_returned_object() {
        let with = "class C { constructor(){ return {}; } }";
        assert_eq!(own(with, "(){ return {}; }"), ["{}"]);
        let without = "class D { constructor(){ this.a = 1; } }";
        assert!(own(without, "(){ this.a = 1; }").is_empty());
    }

    // P6: object and class methods are functions of their own, so the enclosing
    // function owns none of their returns.
    #[test]
    fn object_and_class_methods_own_their_returns() {
        let source =
            "function outer(){const o={m(){return 1;}}; class C{m(){return 2;}}; return 9; }";
        assert_eq!(own(source, source), ["9"]);
        assert_eq!(own(source, "(){return 1;}"), ["1"]);
        assert_eq!(own(source, "(){return 2;}"), ["2"]);
    }

    // P7: order across several nesting levels, and a returned block arrow whose
    // own return is nested one level further.
    #[test]
    fn mixed_nesting_keeps_the_table_order() {
        let source = "function a(){ if (x) { return 1; } const b = () => { return 2; }; function c(){ return () => { return 3; }; } return 4; }";
        assert_eq!(own(source, source), ["1", "4"]);
        assert_eq!(own(source, "() => { return 2; }"), ["2"]);
        assert_eq!(
            own(source, "function c(){ return () => { return 3; }; }"),
            ["() => { return 3; }"]
        );
        assert_eq!(own(source, "() => { return 3; }"), ["3"]);
    }
}
