//! Oxc-derived structural facts.
//!
//! This crate is intentionally checker-independent and regex-free. It parses
//! original source once and exports finite, deterministic tables. Consumers
//! join these spans with TypeScript-Go semantic facts; Oxc nodes never escape.

use crate::core::{SourceIdentity, Span};
use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, AssignmentExpression, AwaitExpression, BinaryExpression,
    BindingPattern, CallExpression, ComputedMemberExpression, ConditionalExpression, Declaration,
    ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
    ExportNamedDeclaration, Expression, FormalParameter, Function, FunctionType,
    IdentifierReference, IfStatement, ImportDeclaration, ImportDeclarationSpecifier,
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXElement, JSXElementName,
    JSXExpression, LogicalExpression, LogicalOperator, ModuleExportName, NewExpression,
    ObjectProperty, ObjectPropertyKind, PropertyKey, ReturnStatement, SpreadElement,
    StaticMemberExpression, TSModuleDeclarationName, UnaryExpression, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{ScopeId, Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span as OxcSpan};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const AST_FACTS_SCHEMA: u32 = 24;

mod span_index;

pub use span_index::{AstSpanIndex, LazySpanIndex};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AstFacts {
    pub schema: u32,
    pub source: SourceIdentity,
    pub calls: Vec<CallFact>,
    pub bindings: Vec<BindingFact>,
    pub functions: Vec<FunctionFact>,
    pub imports: Vec<ImportFact>,
    pub exports: Vec<ExportFact>,
    pub identifiers: Vec<IdentifierFact>,
    pub awaits: Vec<Span>,
    pub returns: Vec<ReturnFact>,
    pub jsx_elements: Vec<JsxElementFact>,
    /// JSX fragment spans (`<>…</>`). A fragment's children are as tracked
    /// as an element's, but it has no name and no attributes, so it gets a
    /// bare span rather than a [`JsxElementFact`]; consumers asking "is this
    /// span inside JSX" must consult both tables.
    #[serde(default)]
    pub jsx_fragments: Vec<Span>,
    pub members: Vec<MemberFact>,
    #[serde(default)]
    pub computed_members: Vec<Span>,
    #[serde(default)]
    pub parameter_properties: Vec<Span>,
    pub spreads: Vec<SpreadFact>,
    pub conditional_tests: Vec<Span>,
    #[serde(default)]
    pub conditional_expressions: Vec<ConditionalExpressionFact>,
    #[serde(default)]
    pub logical_expressions: Vec<LogicalExpressionFact>,
    #[serde(default)]
    pub object_properties: Vec<ObjectPropertyFact>,
    #[serde(default)]
    pub template_literals: Vec<TemplateLiteralFact>,
    /// Operands whose operator coerces a value. A function object is never a
    /// useful substitute for calling a reactive accessor in one of these
    /// slots. Equality, `in`, `instanceof`, `typeof`, `void`, and `delete`
    /// are deliberately absent because they can inspect a function itself.
    #[serde(default)]
    pub coercive_operands: Vec<CoerciveOperandFact>,
    #[serde(default)]
    pub assignments: Vec<AssignmentFact>,
    #[serde(default)]
    pub if_regions: Vec<IfRegionFact>,
    #[serde(skip, default)]
    pub span_index: LazySpanIndex,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFact {
    pub span: Span,
    pub callee: Span,
    pub direct_callee: bool,
    pub type_arguments: bool,
    pub arguments: Vec<ArgumentFact>,
    pub static_callee: bool,
    pub owned_write_option: bool,
}

impl CallFact {
    /// Returns the callee text only when extraction certified it as a static
    /// identifier or dotted identifier path.
    #[must_use]
    pub fn static_callee<'s>(&self, source: &'s str) -> Option<&'s str> {
        self.static_callee
            .then_some(self.callee)
            .and_then(|span| source.get(span.start as usize..span.end as usize))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentFact {
    pub span: Span,
    pub spread: bool,
    pub value: ArgumentValueKind,
    pub boolean_properties: Vec<BooleanPropertyFact>,
    pub identifier_properties: Vec<NamedSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgumentValueKind {
    Undefined,
    Identifier,
    Function,
    AsyncFunction,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BooleanPropertyFact {
    pub name: Span,
    pub value: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingShape {
    Identifier,
    Array,
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingFact {
    pub declaration: Span,
    pub pattern: Span,
    pub shape: BindingShape,
    pub names: Vec<NamedSpan>,
    pub array_slots: Vec<Option<NamedSpan>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_initializer: Option<Span>,
    pub initializer_function: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer_identifier: Option<NamedSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionKind {
    Declaration,
    Expression,
    Arrow,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionFact {
    pub span: Span,
    pub body: Span,
    pub kind: FunctionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<NamedSpan>,
    pub parameters: Vec<BindingFact>,
    pub r#async: bool,
    pub generator: bool,
    pub expression_body: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression_return: Option<ReturnFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSpan {
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportKind {
    SideEffect,
    Named,
    Default,
    Namespace,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBindingFact {
    pub kind: ImportKind,
    pub local: NamedSpan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported: Option<CompactString>,
    pub type_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFact {
    pub span: Span,
    pub module: CompactString,
    pub type_only: bool,
    pub bindings: Vec<ImportBindingFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportKind {
    Named,
    Default,
    All,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFact {
    pub span: Span,
    pub kind: ExportKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<CompactString>,
    pub type_only: bool,
    pub specifiers: Vec<ExportSpecifierFact>,
    pub declarations: Vec<ExportSpecifierFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSpecifierFact {
    pub local: NamedSpan,
    pub exported: CompactString,
    pub type_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierRole {
    Binding,
    Reference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifierFact {
    pub span: Span,
    pub role: IdentifierRole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnFact {
    pub span: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument: Option<Span>,
    pub control_tests: Vec<Span>,
    pub value: ReturnValueKind,
    pub conditional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callee: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsxElementFact {
    pub span: Span,
    #[serde(default)]
    pub opening: Span,
    pub name: NamedSpan,
    /// The object and final property of a dotted JSX name. Keeping these
    /// spans separate lets semantic consumers prove `<Context.Provider>`
    /// from the `Context` binding without parsing the name text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_object: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_property: Option<Span>,
    pub properties: Vec<Span>,
    pub boolean_properties: Vec<BooleanPropertyFact>,
    #[serde(default)]
    pub attributes: Vec<JsxAttributeFact>,
    #[serde(default)]
    pub spreads: Vec<JsxSpreadAttributeFact>,
    #[serde(default)]
    pub self_closing: bool,
    #[serde(default)]
    pub children: Vec<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JsxAttributeValueKind {
    Boolean,
    String,
    Expression,
    Element,
    Fragment,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsxAttributeFact {
    pub span: Span,
    pub name: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<Span>,
    pub local_name: Span,
    /// Declaration selected by ECMAScript/TypeScript lexical scope lookup for
    /// the local name of a `use:name` directive. `None` means that the binder
    /// found no declaration in this file's scope tree. Other JSX namespaces
    /// do not interpret their local name as a value binding and leave this
    /// field empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directive_binding: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Span>,
    pub value_kind: JsxAttributeValueKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsxSpreadAttributeFact {
    pub span: Span,
    pub argument: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalExpressionFact {
    pub span: Span,
    pub test: Span,
    pub consequent: Span,
    pub alternate: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogicalOperatorKind {
    And,
    Or,
    Coalesce,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalExpressionFact {
    pub span: Span,
    pub left: Span,
    pub right: Span,
    pub operator: LogicalOperatorKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPropertyFact {
    pub span: Span,
    pub key: Span,
    pub value: Span,
    pub computed: bool,
}

/// An untagged template literal and the expressions interpolated into it.
///
/// Tagged templates are deliberately excluded because the two coerce
/// differently: a tag receives the interpolations as values and may do
/// anything with them, while an untagged literal stringifies each one. That
/// makes this the fact that proves an interpolated accessor renders its own
/// source text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateLiteralFact {
    pub span: Span,
    pub expressions: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberFact {
    pub span: Span,
    pub object: Span,
    pub property: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpreadFact {
    pub span: Span,
    pub argument: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoerciveOperandKind {
    Binary,
    Unary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoerciveOperandFact {
    pub span: Span,
    pub kind: CoerciveOperandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentFact {
    pub target: Span,
    pub value_span: Span,
    pub value: AssignmentValueKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssignmentValueKind {
    Array,
    Function,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IfRegionFact {
    pub test: Span,
    pub consequent: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReturnValueKind {
    Undefined,
    Function,
    Identifier,
    Call,
    Member,
    Other,
}

impl AstFacts {
    #[must_use]
    pub fn structural_seed_spans(&self) -> Vec<Span> {
        let mut spans = self
            .identifiers
            .iter()
            .map(|identifier| identifier.span)
            .chain(
                self.imports
                    .iter()
                    .flat_map(|import| import.bindings.iter())
                    .filter(|binding| binding.kind != ImportKind::SideEffect)
                    .map(|binding| binding.local.span),
            )
            .chain(
                self.bindings
                    .iter()
                    .flat_map(|binding| binding.names.iter())
                    .map(|name| name.span),
            )
            .chain(self.jsx_elements.iter().map(|element| element.name.span))
            .collect::<Vec<_>>();
        spans.sort_unstable();
        spans.dedup();
        spans
    }
}

#[derive(Debug, Error)]
pub enum AstFactsError {
    #[error(transparent)]
    Identity(#[from] crate::core::FactIdentityError),
    #[error("unsupported source path {path}: {message}")]
    SourceType { path: String, message: String },
    #[error("Oxc parse failed: {0}")]
    Parse(String),
}

pub fn extract(path: impl Into<String>, source: &str) -> Result<AstFacts, AstFactsError> {
    let path = path.into();
    let identity = SourceIdentity::new(path.clone(), source)?;
    let source_type = SourceType::from_path(&path).map_err(|error| AstFactsError::SourceType {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type)
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if let Some(error) = parsed.errors.into_iter().next() {
        return Err(AstFactsError::Parse(error.to_string()));
    }

    // JSX namespace names are not IdentifierReference nodes, so TypeScript's
    // GetSymbolAtLocation deliberately returns no symbol for `use:name`.
    // Build Oxc's semantic scope tree once and retain the exact declaration
    // chosen by its binder instead of approximating scope with source text.
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut collector = Collector::new(source, semantic.scoping());
    collector.visit_program(&parsed.program);
    Ok(collector.finish(identity))
}

struct Collector<'s, 'semantic> {
    source: &'s str,
    scoping: &'semantic Scoping,
    scope_stack: Vec<ScopeId>,
    calls: Vec<CallFact>,
    bindings: Vec<BindingFact>,
    functions: Vec<FunctionFact>,
    imports: Vec<ImportFact>,
    exports: Vec<ExportFact>,
    identifiers: Vec<IdentifierFact>,
    awaits: Vec<Span>,
    returns: Vec<ReturnFact>,
    jsx_elements: Vec<JsxElementFact>,
    jsx_fragments: Vec<Span>,
    members: Vec<MemberFact>,
    computed_members: Vec<Span>,
    parameter_properties: Vec<Span>,
    spreads: Vec<SpreadFact>,
    conditional_tests: Vec<Span>,
    conditional_expressions: Vec<ConditionalExpressionFact>,
    logical_expressions: Vec<LogicalExpressionFact>,
    object_properties: Vec<ObjectPropertyFact>,
    template_literals: Vec<TemplateLiteralFact>,
    coercive_operands: Vec<CoerciveOperandFact>,
    assignments: Vec<AssignmentFact>,
    if_regions: Vec<IfRegionFact>,
    conditional_control_stack: Vec<Span>,
}

impl<'s, 'semantic> Collector<'s, 'semantic> {
    fn new(source: &'s str, scoping: &'semantic Scoping) -> Self {
        Self {
            source,
            scoping,
            scope_stack: Vec::new(),
            calls: Vec::new(),
            bindings: Vec::new(),
            functions: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            identifiers: Vec::new(),
            awaits: Vec::new(),
            returns: Vec::new(),
            jsx_elements: Vec::new(),
            jsx_fragments: Vec::new(),
            members: Vec::new(),
            computed_members: Vec::new(),
            parameter_properties: Vec::new(),
            spreads: Vec::new(),
            conditional_tests: Vec::new(),
            conditional_expressions: Vec::new(),
            logical_expressions: Vec::new(),
            object_properties: Vec::new(),
            template_literals: Vec::new(),
            coercive_operands: Vec::new(),
            assignments: Vec::new(),
            if_regions: Vec::new(),
            conditional_control_stack: Vec::new(),
        }
    }

    fn finish(mut self, source: SourceIdentity) -> AstFacts {
        self.calls.sort_by_key(|fact| fact.span);
        self.bindings.sort_by_key(|fact| fact.declaration);
        self.functions.sort_by_key(|fact| fact.span);
        self.imports.sort_by_key(|fact| fact.span);
        self.exports.sort_by_key(|fact| fact.span);
        self.identifiers.sort_by_key(|identifier| identifier.span);
        self.awaits.sort_unstable();
        self.returns.sort_by_key(|fact| fact.span);
        self.jsx_elements.sort_by_key(|fact| fact.span);
        self.jsx_fragments.sort();
        self.members.sort_by_key(|fact| fact.span);
        self.computed_members.sort_unstable();
        self.parameter_properties.sort_unstable();
        self.spreads.sort_by_key(|fact| fact.span);
        self.conditional_tests.sort_unstable();
        self.conditional_expressions.sort_by_key(|fact| fact.span);
        self.logical_expressions.sort_by_key(|fact| fact.span);
        self.object_properties.sort_by_key(|fact| fact.span);
        self.template_literals.sort_by_key(|fact| fact.span);
        self.coercive_operands.sort_by_key(|fact| fact.span);
        self.assignments.sort_by_key(|fact| fact.target);
        self.if_regions.sort_by_key(|fact| fact.consequent);
        AstFacts {
            schema: AST_FACTS_SCHEMA,
            source,
            span_index: LazySpanIndex::default(),
            calls: self.calls,
            bindings: self.bindings,
            functions: self.functions,
            imports: self.imports,
            exports: self.exports,
            identifiers: self.identifiers,
            awaits: self.awaits,
            returns: self.returns,
            jsx_elements: self.jsx_elements,
            jsx_fragments: self.jsx_fragments,
            members: self.members,
            computed_members: self.computed_members,
            parameter_properties: self.parameter_properties,
            spreads: self.spreads,
            conditional_tests: self.conditional_tests,
            conditional_expressions: self.conditional_expressions,
            logical_expressions: self.logical_expressions,
            object_properties: self.object_properties,
            template_literals: self.template_literals,
            coercive_operands: self.coercive_operands,
            assignments: self.assignments,
            if_regions: self.if_regions,
        }
    }

    fn binding_fact(
        &self,
        declaration: OxcSpan,
        pattern: &BindingPattern<'_>,
        initializer: Option<OxcSpan>,
        call_initializer: Option<OxcSpan>,
        initializer_function: bool,
        initializer_identifier: Option<NamedSpan>,
    ) -> BindingFact {
        let shape = match pattern {
            BindingPattern::BindingIdentifier(_) | BindingPattern::AssignmentPattern(_) => {
                BindingShape::Identifier
            }
            BindingPattern::ArrayPattern(_) => BindingShape::Array,
            BindingPattern::ObjectPattern(_) => BindingShape::Object,
        };
        BindingFact {
            declaration: span(declaration),
            pattern: span(pattern.span()),
            shape,
            names: pattern
                .get_binding_identifiers()
                .into_iter()
                .map(|identifier| NamedSpan {
                    span: span(identifier.span),
                })
                .collect(),
            array_slots: match pattern {
                BindingPattern::ArrayPattern(array) => {
                    array
                        .elements
                        .iter()
                        .map(|element| {
                            element.as_ref().and_then(|pattern| {
                                pattern.get_binding_identifiers().into_iter().next().map(
                                    |identifier| NamedSpan {
                                        span: span(identifier.span),
                                    },
                                )
                            })
                        })
                        .collect()
                }
                _ => vec![],
            },
            initializer: initializer.map(span),
            call_initializer: call_initializer.map(span),
            initializer_function,
            initializer_identifier,
        }
    }

    fn return_fact(&self, expression: Option<&Expression<'_>>, fallback: OxcSpan) -> ReturnFact {
        let Some(expression) = expression else {
            return ReturnFact {
                span: span(fallback),
                argument: None,
                control_tests: self.conditional_control_stack.clone(),
                value: ReturnValueKind::Undefined,
                conditional: false,
                callee: None,
            };
        };
        let argument_span = span(expression.span());
        let expression = expression.get_inner_expression();
        let conditional = matches!(expression, Expression::ConditionalExpression(_));
        let (value, callee) = match expression {
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                (ReturnValueKind::Function, None)
            }
            Expression::Identifier(identifier) if identifier.name == "undefined" => {
                (ReturnValueKind::Undefined, None)
            }
            Expression::Identifier(_) => (ReturnValueKind::Identifier, None),
            Expression::CallExpression(call) => {
                (ReturnValueKind::Call, Some(span(call.callee.span())))
            }
            Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::PrivateFieldExpression(_) => (ReturnValueKind::Member, None),
            Expression::UnaryExpression(unary)
                if unary.operator == oxc_syntax::operator::UnaryOperator::Void =>
            {
                (ReturnValueKind::Undefined, None)
            }
            _ => (ReturnValueKind::Other, None),
        };
        ReturnFact {
            span: span(expression.span()),
            argument: Some(argument_span),
            control_tests: self.conditional_control_stack.clone(),
            value,
            conditional,
            callee,
        }
    }

    fn is_static_callee(&self, callee: OxcSpan) -> bool {
        self.source
            .get(callee.start as usize..callee.end as usize)
            .is_some_and(|text| {
                text.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.'))
            })
    }

    fn argument_fact(argument: &Argument<'_>) -> ArgumentFact {
        let value = match argument {
            Argument::Identifier(identifier) if identifier.name == "undefined" => {
                ArgumentValueKind::Undefined
            }
            Argument::Identifier(_) => ArgumentValueKind::Identifier,
            Argument::ArrowFunctionExpression(function) if function.r#async => {
                ArgumentValueKind::AsyncFunction
            }
            Argument::FunctionExpression(function) if function.r#async => {
                ArgumentValueKind::AsyncFunction
            }
            Argument::ArrowFunctionExpression(_) | Argument::FunctionExpression(_) => {
                ArgumentValueKind::Function
            }
            _ => ArgumentValueKind::Other,
        };
        let boolean_properties = match argument {
            Argument::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    let Expression::BooleanLiteral(value) = &property.value else {
                        return None;
                    };
                    Some(BooleanPropertyFact {
                        name: span(key.span),
                        value: value.value,
                    })
                })
                .collect(),
            _ => vec![],
        };
        let identifier_properties = match argument {
            Argument::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    if !matches!(key.name.as_str(), "effect" | "error") {
                        return None;
                    }
                    let Expression::Identifier(value) = &property.value else {
                        return None;
                    };
                    Some(NamedSpan {
                        span: span(value.span),
                    })
                })
                .collect(),
            _ => vec![],
        };
        ArgumentFact {
            span: span(argument.span()),
            spread: argument.is_spread(),
            value,
            boolean_properties,
            identifier_properties,
        }
    }
}

impl<'a> Visit<'a> for Collector<'_, '_> {
    fn enter_scope(&mut self, _flags: ScopeFlags, scope_id: &std::cell::Cell<Option<ScopeId>>) {
        // SemanticBuilder populated every scope cell in this same AST before
        // fact collection. Falling back to the root keeps extraction total if
        // Oxc ever introduces an unpopulated synthetic scope.
        self.scope_stack.push(
            scope_id
                .get()
                .unwrap_or_else(|| self.scoping.root_scope_id()),
        );
    }

    fn leave_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let callee_span = call.callee.span();
        self.calls.push(CallFact {
            span: span(call.span),
            callee: span(callee_span),
            direct_callee: matches!(call.callee, Expression::Identifier(_)),
            type_arguments: call.type_arguments.is_some(),
            arguments: call.arguments.iter().map(Self::argument_fact).collect(),
            static_callee: self.is_static_callee(callee_span),
            owned_write_option: call.arguments.get(1).is_some_and(|argument| {
                let Argument::ObjectExpression(options) = argument else {
                    return false;
                };
                options.properties.iter().any(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return false;
                    };
                    key.name == "ownedWrite"
                        && matches!(
                            &property.value,
                            oxc_ast::ast::Expression::BooleanLiteral(value) if value.value
                        )
                })
            }),
        });
        walk::walk_call_expression(self, call);
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        let callee_span = expression.callee.span();
        self.calls.push(CallFact {
            span: span(expression.span),
            callee: span(callee_span),
            direct_callee: matches!(expression.callee, Expression::Identifier(_)),
            type_arguments: expression.type_arguments.is_some(),
            arguments: expression
                .arguments
                .iter()
                .map(Self::argument_fact)
                .collect(),
            static_callee: self.is_static_callee(callee_span),
            owned_write_option: false,
        });
        walk::walk_new_expression(self, expression);
    }

    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
        let initializer = declaration.init.as_ref().map(GetSpan::span);
        let initializer_function = declaration.init.as_ref().is_some_and(|expression| {
            matches!(
                expression.get_inner_expression(),
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
        });
        let initializer_identifier = declaration.init.as_ref().and_then(|expression| {
            let Expression::Identifier(identifier) = expression.get_inner_expression() else {
                return None;
            };
            Some(NamedSpan {
                span: span(identifier.span),
            })
        });
        let call_initializer = declaration.init.as_ref().and_then(|expression| {
            match expression.get_inner_expression() {
                oxc_ast::ast::Expression::CallExpression(call) => Some(call.span),
                _ => None,
            }
        });
        self.bindings.push(self.binding_fact(
            declaration.span,
            &declaration.id,
            initializer,
            call_initializer,
            initializer_function,
            initializer_identifier,
        ));
        walk::walk_variable_declarator(self, declaration);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.functions.push(FunctionFact {
                span: span(function.span),
                body: span(body.span),
                kind: match function.r#type {
                    FunctionType::FunctionExpression
                    | FunctionType::TSEmptyBodyFunctionExpression => FunctionKind::Expression,
                    FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction => {
                        FunctionKind::Declaration
                    }
                },
                name: function.id.as_ref().map(|identifier| NamedSpan {
                    span: span(identifier.span),
                }),
                parameters: function
                    .params
                    .items
                    .iter()
                    .map(|parameter| {
                        self.binding_fact(
                            parameter.span,
                            &parameter.pattern,
                            None,
                            None,
                            false,
                            None,
                        )
                    })
                    .collect(),
                r#async: function.r#async,
                generator: function.generator,
                expression_body: false,
                expression_return: None,
            });
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        let expression_return = function
            .get_expression()
            .map(|expression| self.return_fact(Some(expression), expression.span()));
        self.functions.push(FunctionFact {
            span: span(function.span),
            body: span(function.body.span),
            kind: FunctionKind::Arrow,
            name: None,
            parameters: function
                .params
                .items
                .iter()
                .map(|parameter| {
                    self.binding_fact(parameter.span, &parameter.pattern, None, None, false, None)
                })
                .collect(),
            r#async: function.r#async,
            generator: false,
            expression_body: function.expression,
            expression_return,
        });
        walk::walk_arrow_function_expression(self, function);
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        let mut bindings = Vec::new();
        for specifier in declaration.specifiers.iter().flatten() {
            let (kind, local) = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    (ImportKind::Named, &specifier.local)
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    (ImportKind::Default, &specifier.local)
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    (ImportKind::Namespace, &specifier.local)
                }
            };
            bindings.push(ImportBindingFact {
                kind,
                local: NamedSpan {
                    span: span(local.span),
                },
                imported: match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        Some(match &specifier.imported {
                            ModuleExportName::IdentifierName(name) => name.name.as_str().into(),
                            ModuleExportName::IdentifierReference(name) => {
                                name.name.as_str().into()
                            }
                            ModuleExportName::StringLiteral(name) => name.value.as_str().into(),
                        })
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => Some("default".into()),
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => None,
                },
                type_only: declaration.import_kind.is_type()
                    || matches!(
                        specifier,
                        ImportDeclarationSpecifier::ImportSpecifier(specifier)
                            if specifier.import_kind.is_type()
                    ),
            });
        }
        if bindings.is_empty() {
            bindings.push(ImportBindingFact {
                kind: ImportKind::SideEffect,
                local: NamedSpan {
                    span: span(declaration.source.span),
                },
                imported: None,
                type_only: false,
            });
        }
        self.imports.push(ImportFact {
            span: span(declaration.span),
            module: declaration.source.value.as_str().into(),
            type_only: declaration.import_kind.is_type(),
            bindings,
        });
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        self.exports.push(ExportFact {
            span: span(declaration.span),
            kind: ExportKind::Named,
            module: declaration
                .source
                .as_ref()
                .map(|source| source.value.as_str().into()),
            type_only: declaration.export_kind.is_type(),
            specifiers: declaration
                .specifiers
                .iter()
                .map(|specifier| ExportSpecifierFact {
                    local: NamedSpan {
                        span: span(specifier.local.span()),
                    },
                    exported: module_export_name(&specifier.exported),
                    type_only: specifier.export_kind.is_type(),
                })
                .collect(),
            declarations: declaration
                .declaration
                .as_ref()
                .map_or_else(Vec::new, export_declaration_names),
        });
        walk::walk_export_named_declaration(self, declaration);
    }

    fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'a>) {
        let local = match &declaration.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                function.id.as_ref().map_or(function.span, |id| id.span)
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                class.id.as_ref().map_or(class.span, |id| id.span)
            }
            declaration => declaration.span(),
        };
        self.exports.push(ExportFact {
            span: span(declaration.span),
            kind: ExportKind::Default,
            module: None,
            type_only: false,
            specifiers: vec![],
            declarations: vec![ExportSpecifierFact {
                local: NamedSpan { span: span(local) },
                exported: "default".into(),
                type_only: false,
            }],
        });
        walk::walk_export_default_declaration(self, declaration);
    }

    fn visit_export_all_declaration(&mut self, declaration: &ExportAllDeclaration<'a>) {
        self.exports.push(ExportFact {
            span: span(declaration.span),
            kind: ExportKind::All,
            module: Some(declaration.source.value.as_str().into()),
            type_only: declaration.export_kind.is_type(),
            specifiers: vec![],
            declarations: vec![],
        });
        walk::walk_export_all_declaration(self, declaration);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.identifiers.push(IdentifierFact {
            span: span(identifier.span),
            role: IdentifierRole::Reference,
        });
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.identifiers.push(IdentifierFact {
            span: span(identifier.span),
            role: IdentifierRole::Binding,
        });
        walk::walk_binding_identifier(self, identifier);
    }

    fn visit_await_expression(&mut self, expression: &AwaitExpression<'a>) {
        self.awaits.push(span(expression.span));
        walk::walk_await_expression(self, expression);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        let returned = self.return_fact(statement.argument.as_ref(), statement.span);
        self.returns.push(returned);
        walk::walk_return_statement(self, statement);
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        let test = span(statement.test.span());
        self.conditional_tests.push(test);
        self.if_regions.push(IfRegionFact {
            test,
            consequent: span(statement.consequent.span()),
        });
        self.visit_expression(&statement.test);
        self.conditional_control_stack.push(test);
        self.visit_statement(&statement.consequent);
        if let Some(alternate) = &statement.alternate {
            self.visit_statement(alternate);
        }
        self.conditional_control_stack.pop();
    }

    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'a>) {
        if expression.operator == AssignmentOperator::Assign {
            self.assignments.push(AssignmentFact {
                target: span(expression.left.span()),
                value_span: span(expression.right.span()),
                value: match expression.right {
                    Expression::ArrayExpression(_) => AssignmentValueKind::Array,
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                        AssignmentValueKind::Function
                    }
                    _ => AssignmentValueKind::Other,
                },
            });
        }
        walk::walk_assignment_expression(self, expression);
    }

    fn visit_formal_parameter(&mut self, parameter: &FormalParameter<'a>) {
        if parameter.accessibility.is_some() || parameter.readonly {
            self.parameter_properties.extend(
                parameter
                    .pattern
                    .get_binding_identifiers()
                    .into_iter()
                    .map(|identifier| span(identifier.span)),
            );
        }
        walk::walk_formal_parameter(self, parameter);
    }

    fn visit_conditional_expression(&mut self, expression: &ConditionalExpression<'a>) {
        self.conditional_tests.push(span(expression.test.span()));
        self.conditional_expressions
            .push(ConditionalExpressionFact {
                span: span(expression.span),
                test: span(expression.test.span()),
                consequent: span(expression.consequent.span()),
                alternate: span(expression.alternate.span()),
            });
        walk::walk_conditional_expression(self, expression);
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        self.logical_expressions.push(LogicalExpressionFact {
            span: span(expression.span),
            left: span(expression.left.span()),
            right: span(expression.right.span()),
            operator: match expression.operator {
                LogicalOperator::And => LogicalOperatorKind::And,
                LogicalOperator::Or => LogicalOperatorKind::Or,
                LogicalOperator::Coalesce => LogicalOperatorKind::Coalesce,
            },
        });
        walk::walk_logical_expression(self, expression);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        self.object_properties.push(ObjectPropertyFact {
            span: span(property.span),
            key: span(property.key.span()),
            value: span(property.value.span()),
            computed: property.computed,
        });
        walk::walk_object_property(self, property);
    }

    fn visit_tagged_template_expression(
        &mut self,
        expression: &oxc_ast::ast::TaggedTemplateExpression<'a>,
    ) {
        // Not the default walk: that would visit the quasi through
        // `visit_template_literal` and record every tagged template's quasi
        // in `template_literals`, violating that table's untagged-only
        // contract. The tag and the interpolated expressions are visited
        // directly so their own nested nodes — including genuinely untagged
        // templates inside an interpolation — are still collected.
        self.visit_expression(&expression.tag);
        for interpolated in &expression.quasi.expressions {
            self.visit_expression(interpolated);
        }
    }

    fn visit_template_literal(&mut self, literal: &oxc_ast::ast::TemplateLiteral<'a>) {
        self.template_literals.push(TemplateLiteralFact {
            span: span(literal.span),
            expressions: literal
                .expressions
                .iter()
                .map(|interpolated| span(interpolated.span()))
                .collect(),
        });
        walk::walk_template_literal(self, literal);
    }

    fn visit_jsx_fragment(&mut self, fragment: &oxc_ast::ast::JSXFragment<'a>) {
        self.jsx_fragments.push(span(fragment.span));
        walk::walk_jsx_fragment(self, fragment);
    }

    fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
        let current_scope = self
            .scope_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.scoping.root_scope_id());
        let name_span = element.opening_element.name.span();
        let (member_object, member_property) =
            if let JSXElementName::MemberExpression(member) = &element.opening_element.name {
                (
                    Some(span(member.object.span())),
                    Some(span(member.property.span)),
                )
            } else {
                (None, None)
            };
        self.jsx_elements.push(JsxElementFact {
            span: span(element.span),
            opening: span(element.opening_element.span),
            name: NamedSpan {
                span: span(name_span),
            },
            member_object,
            member_property,
            properties: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return None;
                    };
                    let JSXAttributeName::Identifier(name) = &attribute.name else {
                        return None;
                    };
                    Some(span(name.span))
                })
                .collect(),
            boolean_properties: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return None;
                    };
                    let JSXAttributeName::Identifier(name) = &attribute.name else {
                        return None;
                    };
                    let value = match attribute.value.as_ref() {
                        None => true,
                        Some(JSXAttributeValue::ExpressionContainer(container)) => {
                            let JSXExpression::BooleanLiteral(value) = &container.expression else {
                                return None;
                            };
                            value.value
                        }
                        _ => return None,
                    };
                    Some(BooleanPropertyFact {
                        name: span(name.span),
                        value,
                    })
                })
                .collect(),
            attributes: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return None;
                    };
                    let (name, namespace, local_name, directive_binding) = match &attribute.name {
                        JSXAttributeName::Identifier(name) => (name.span, None, name.span, None),
                        JSXAttributeName::NamespacedName(name) => {
                            let directive_binding = (name.namespace.name == "use")
                                .then(|| {
                                    self.scoping
                                        .find_binding(current_scope, name.name.name.as_str().into())
                                        .filter(|&symbol| {
                                            self.scoping
                                                .symbol_flags(symbol)
                                                .can_be_referenced_by_value()
                                        })
                                        .map(|symbol| span(self.scoping.symbol_span(symbol)))
                                })
                                .flatten();
                            (
                                name.span,
                                Some(name.namespace.span),
                                name.name.span,
                                directive_binding,
                            )
                        }
                    };
                    let (value, expression, value_kind) = match attribute.value.as_ref() {
                        None => (None, None, JsxAttributeValueKind::Boolean),
                        Some(JSXAttributeValue::StringLiteral(value)) => {
                            (Some(span(value.span)), None, JsxAttributeValueKind::String)
                        }
                        Some(JSXAttributeValue::ExpressionContainer(container)) => (
                            Some(span(container.span)),
                            Some(span(container.expression.span())),
                            JsxAttributeValueKind::Expression,
                        ),
                        Some(JSXAttributeValue::Element(value)) => {
                            (Some(span(value.span)), None, JsxAttributeValueKind::Element)
                        }
                        Some(JSXAttributeValue::Fragment(value)) => (
                            Some(span(value.span)),
                            None,
                            JsxAttributeValueKind::Fragment,
                        ),
                    };
                    Some(JsxAttributeFact {
                        span: span(attribute.span),
                        name: span(name),
                        namespace: namespace.map(span),
                        local_name: span(local_name),
                        directive_binding,
                        value,
                        expression,
                        value_kind,
                    })
                })
                .collect(),
            spreads: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::SpreadAttribute(attribute) = item else {
                        return None;
                    };
                    Some(JsxSpreadAttributeFact {
                        span: span(attribute.span),
                        argument: span(attribute.argument.span()),
                    })
                })
                .collect(),
            self_closing: element.closing_element.is_none(),
            children: element
                .children
                .iter()
                .map(|child| span(child.span()))
                .collect(),
        });
        walk::walk_jsx_element(self, element);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.members.push(MemberFact {
            span: span(member.span),
            object: span(member.object.span()),
            property: span(member.property.span),
        });
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        let property = member.expression.span();
        let member_span = span(member.span);
        self.members.push(MemberFact {
            span: member_span,
            object: span(member.object.span()),
            property: span(property),
        });
        self.computed_members.push(member_span);
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_spread_element(&mut self, spread: &SpreadElement<'a>) {
        self.spreads.push(SpreadFact {
            span: span(spread.span),
            argument: span(spread.argument.span()),
        });
        walk::walk_spread_element(self, spread);
    }

    fn visit_binary_expression(&mut self, expression: &BinaryExpression<'a>) {
        use oxc_syntax::operator::BinaryOperator;

        if !matches!(
            expression.operator,
            BinaryOperator::Equality
                | BinaryOperator::Inequality
                | BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality
                | BinaryOperator::In
                | BinaryOperator::Instanceof
        ) {
            self.coercive_operands.extend([
                CoerciveOperandFact {
                    span: span(expression.left.span()),
                    kind: CoerciveOperandKind::Binary,
                },
                CoerciveOperandFact {
                    span: span(expression.right.span()),
                    kind: CoerciveOperandKind::Binary,
                },
            ]);
        }
        walk::walk_binary_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        use oxc_syntax::operator::UnaryOperator;

        if matches!(
            expression.operator,
            UnaryOperator::UnaryPlus
                | UnaryOperator::UnaryNegation
                | UnaryOperator::LogicalNot
                | UnaryOperator::BitwiseNot
        ) {
            self.coercive_operands.push(CoerciveOperandFact {
                span: span(expression.argument.span()),
                kind: CoerciveOperandKind::Unary,
            });
        }
        walk::walk_unary_expression(self, expression);
    }
}

const fn span(value: OxcSpan) -> Span {
    Span::new(value.start, value.end)
}

fn module_export_name(name: &ModuleExportName<'_>) -> CompactString {
    match name {
        ModuleExportName::IdentifierName(name) => name.name.as_str().into(),
        ModuleExportName::IdentifierReference(name) => name.name.as_str().into(),
        ModuleExportName::StringLiteral(name) => name.value.as_str().into(),
    }
}

fn export_declaration_names(declaration: &Declaration<'_>) -> Vec<ExportSpecifierFact> {
    let named = |name: &oxc_ast::ast::BindingIdentifier<'_>, type_only| ExportSpecifierFact {
        local: NamedSpan {
            span: span(name.span),
        },
        exported: name.name.as_str().into(),
        type_only,
    };
    match declaration {
        Declaration::VariableDeclaration(declaration) => declaration
            .declarations
            .iter()
            .flat_map(|declarator| declarator.id.get_binding_identifiers())
            .map(|name| named(name, false))
            .collect(),
        Declaration::FunctionDeclaration(declaration) => declaration
            .id
            .as_ref()
            .map(|name| vec![named(name, false)])
            .unwrap_or_default(),
        Declaration::ClassDeclaration(declaration) => declaration
            .id
            .as_ref()
            .map(|name| vec![named(name, false)])
            .unwrap_or_default(),
        Declaration::TSTypeAliasDeclaration(declaration) => vec![named(&declaration.id, true)],
        Declaration::TSInterfaceDeclaration(declaration) => vec![named(&declaration.id, true)],
        Declaration::TSEnumDeclaration(declaration) => vec![named(&declaration.id, false)],
        Declaration::TSModuleDeclaration(declaration) => match &declaration.id {
            TSModuleDeclarationName::Identifier(name) if !declaration.declare => {
                vec![named(name, false)]
            }
            _ => vec![],
        },
        Declaration::TSImportEqualsDeclaration(declaration) => {
            vec![named(&declaration.id, declaration.import_kind.is_type())]
        }
        Declaration::TSGlobalDeclaration(_) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tsx_structure_without_text_patterns() {
        let source = r#"
import { createSignal as signal, createEffect } from "solid-js";
export const [count, setCount] = signal(0);
export async function App(props: { title: string }) {
  await ready();
  createEffect(() => count(), value => console.log(value));
  return <button onClick={() => setCount(count() + 1)}>{props.title}</button>;
}
"#;
        let facts = extract("/project/App.tsx", source).unwrap();
        assert_eq!(facts.schema, AST_FACTS_SCHEMA);
        assert_eq!(facts.imports[0].module, "solid-js");
        assert!(
            facts
                .calls
                .iter()
                .any(|call| call.static_callee(source) == Some("signal"))
        );
        assert!(
            facts
                .calls
                .iter()
                .any(|call| call.static_callee(source) == Some("createEffect"))
        );
        assert!(facts.bindings.iter().any(|binding| {
            binding.shape == BindingShape::Array
                && binding
                    .names
                    .iter()
                    .filter_map(|name| source.get(name.span.start as usize..name.span.end as usize))
                    .collect::<Vec<_>>()
                    == ["count", "setCount"]
        }));
        assert!(facts.functions.iter().any(|function| {
            function.name.as_ref().is_some_and(|name| {
                source.get(name.span.start as usize..name.span.end as usize) == Some("App")
            }) && function.r#async
        }));
        assert_eq!(facts.awaits.len(), 1);
        assert_eq!(facts.returns.len(), 1);
        assert_eq!(facts.jsx_elements.len(), 1);
        assert_eq!(
            source.get(
                facts.jsx_elements[0].name.span.start as usize
                    ..facts.jsx_elements[0].name.span.end as usize
            ),
            Some("button")
        );
        assert!(facts.members.iter().any(|member| {
            source.get(member.property.start as usize..member.property.end as usize)
                == Some("title")
        }));
    }

    #[test]
    fn unwraps_typescript_assertions_around_call_initializers() {
        let source =
            "const [state, setState] = createSignal(0) as unknown as [() => number, Function];";
        let facts = extract("state.ts", source).unwrap();
        assert_eq!(facts.bindings.len(), 1);
        assert!(facts.bindings[0].call_initializer.is_some());
    }

    #[test]
    fn certifies_static_callee_spans_without_retaining_their_text() {
        let source = "solid.createEffect(); (factory())();";
        let facts = extract("calls.ts", source).unwrap();
        let callees = facts
            .calls
            .iter()
            .map(|call| call.static_callee(source))
            .collect::<Vec<_>>();

        assert!(callees.contains(&Some("solid.createEffect")));
        assert!(callees.contains(&Some("factory")));
        assert!(callees.contains(&None));
    }

    #[test]
    fn classifies_cleanup_return_shapes_from_ast_nodes() {
        let source = r#"
const cleanup = () => {};
const valid = () => {
  if (ready) return undefined;
  return cleanup;
};
const invalid = async () => 42;
const mixed = () => {
  if (ready) return () => {};
  return { invalid: true };
};
"#;
        let facts = extract("cleanup.ts", source).unwrap();
        let cleanup = facts
            .bindings
            .iter()
            .find(|binding| {
                source.get(binding.names[0].span.start as usize..binding.names[0].span.end as usize)
                    == Some("cleanup")
            })
            .unwrap();
        assert!(cleanup.initializer_function);
        let values = facts
            .returns
            .iter()
            .map(|returned| returned.value)
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            [
                ReturnValueKind::Undefined,
                ReturnValueKind::Identifier,
                ReturnValueKind::Function,
                ReturnValueKind::Other,
            ]
        );
        let returned_identifier = facts
            .returns
            .iter()
            .find(|returned| returned.value == ReturnValueKind::Identifier)
            .unwrap();
        assert_eq!(
            source.get(
                returned_identifier.span.start as usize..returned_identifier.span.end as usize
            ),
            Some("cleanup")
        );
        assert!(facts.functions.iter().any(|function| {
            function.r#async
                && function
                    .expression_return
                    .as_ref()
                    .is_some_and(|returned| returned.value == ReturnValueKind::Other)
        }));
    }

    #[test]
    fn classifies_argument_shapes_and_boolean_options() {
        let source = "createMemo(async () => 1, { sync: true, ownedWrite: false });";
        let facts = extract("options.ts", source).unwrap();
        let call = &facts.calls[0];
        assert_eq!(call.arguments[0].value, ArgumentValueKind::AsyncFunction);
        assert_eq!(
            call.arguments[1]
                .boolean_properties
                .iter()
                .map(|property| (
                    source.get(property.name.start as usize..property.name.end as usize),
                    property.value,
                ))
                .collect::<Vec<_>>(),
            [(Some("sync"), true), (Some("ownedWrite"), false),]
        );
    }

    #[test]
    fn retains_named_callbacks_in_object_arguments() {
        let facts = extract(
            "effect.ts",
            "createEffect(compute, { effect: apply, error: handle });",
        )
        .unwrap();
        assert_eq!(
            facts.calls[0].arguments[1].identifier_properties,
            [
                NamedSpan {
                    span: Span::new(32, 37),
                },
                NamedSpan {
                    span: Span::new(46, 52),
                },
            ]
        );
    }

    #[test]
    fn retains_conditional_returns_and_jsx_boolean_properties() {
        let source =
            "const View = (props) => props.ready ? <For keyed={false} /> : <Show keyed />;";
        let facts = extract("control.tsx", source).unwrap();
        assert!(
            facts.functions[0]
                .expression_return
                .as_ref()
                .is_some_and(|returned| returned.conditional)
        );
        assert_eq!(
            facts
                .jsx_elements
                .iter()
                .map(|element| {
                    element
                        .boolean_properties
                        .iter()
                        .map(|property| {
                            (
                                source
                                    .get(property.name.start as usize..property.name.end as usize),
                                property.value,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            [vec![(Some("keyed"), false)], vec![(Some("keyed"), true)],]
        );
        assert!(facts.jsx_elements.iter().all(|element| {
            element.properties.iter().all(|property| {
                source.get(property.start as usize..property.end as usize) == Some("keyed")
            })
        }));
    }

    #[test]
    fn relates_returns_to_the_if_tests_that_control_them() {
        let source = "function View(props) { if (props.debug) return null; const label = props.ready ? 'yes' : 'no'; return <div>{label}</div>; }";
        let facts = extract("control.tsx", source).unwrap();
        let debug_start = u32::try_from(source.find("props.debug").unwrap()).unwrap();
        let ready_start = u32::try_from(source.find("props.ready").unwrap()).unwrap();
        let guarded = facts
            .returns
            .iter()
            .find(|returned| !returned.control_tests.is_empty())
            .expect("guarded return");
        assert_eq!(guarded.control_tests[0].start, debug_start);
        assert!(
            guarded
                .control_tests
                .iter()
                .all(|test| test.start != ready_start),
            "an unrelated conditional must not control the early return"
        );
        assert!(
            facts
                .returns
                .iter()
                .any(|returned| returned.control_tests.is_empty()),
            "the final return is not controlled by the earlier if"
        );
    }

    #[test]
    fn retains_type_only_import_specifiers() {
        let facts = extract(
            "types.ts",
            r#"import value, { type Shape, runtime as renamed } from "./dependency";"#,
        )
        .unwrap();

        let bindings = &facts.imports[0].bindings;
        assert!(
            bindings
                .iter()
                .find(|binding| binding.imported.as_deref() == Some("Shape"))
                .is_some_and(|binding| binding.type_only)
        );
        assert!(
            bindings
                .iter()
                .filter(|binding| binding.imported.as_deref() != Some("Shape"))
                .all(|binding| !binding.type_only)
        );
    }

    #[test]
    fn represents_side_effect_imports_without_an_empty_name_sentinel() {
        let source = r#"import "./setup";"#;
        let facts = extract("setup.ts", source).unwrap();
        let binding = &facts.imports[0].bindings[0];

        assert_eq!(binding.kind, ImportKind::SideEffect);
        assert_eq!(
            source.get(binding.local.span.start as usize..binding.local.span.end as usize),
            Some(r#""./setup""#)
        );
        assert!(facts.structural_seed_spans().is_empty());
    }

    #[test]
    fn retains_runtime_and_type_only_export_declarations() {
        let facts = extract(
            "exports.ts",
            "export class RuntimeClass {} export interface Shape {} export const value = 1; export default function Main() {}",
        )
        .unwrap();
        let declarations = facts
            .exports
            .iter()
            .flat_map(|export| &export.declarations)
            .map(|declaration| (declaration.exported.as_str(), declaration.type_only))
            .collect::<Vec<_>>();

        assert_eq!(
            declarations,
            [
                ("RuntimeClass", false),
                ("Shape", true),
                ("value", false),
                ("default", false)
            ]
        );
    }

    #[test]
    fn rejects_malformed_source() {
        assert!(matches!(
            extract("broken.tsx", "const = ;"),
            Err(AstFactsError::Parse(_))
        ));
    }

    #[test]
    fn retains_computed_members_and_spreads_as_reactive_read_shapes() {
        let source = "const key = 'name'; const value = props[key]; const copy = { ...props };";
        let facts = extract("reads.ts", source).unwrap();

        assert_eq!(facts.members.len(), 1);
        assert_eq!(
            source.get(
                facts.members[0].property.start as usize..facts.members[0].property.end as usize
            ),
            Some("key")
        );
        assert_eq!(
            &source[usize::try_from(facts.members[0].object.start).unwrap()
                ..usize::try_from(facts.members[0].object.end).unwrap()],
            "props"
        );
        assert_eq!(facts.spreads.len(), 1);
        assert_eq!(
            &source[usize::try_from(facts.spreads[0].argument.start).unwrap()
                ..usize::try_from(facts.spreads[0].argument.end).unwrap()],
            "props"
        );
    }

    #[test]
    fn retains_only_operators_that_coerce_accessor_values() {
        let source = "const a = signal + 1; const b = -signal; const c = !signal; const d = signal === other; const e = typeof signal;";
        let facts = extract("operators.ts", source).unwrap();
        let operands = facts
            .coercive_operands
            .iter()
            .filter_map(|operand| {
                source.get(operand.span.start as usize..operand.span.end as usize)
            })
            .collect::<Vec<_>>();

        assert_eq!(operands, ["signal", "1", "signal", "signal"]);
    }

    #[test]
    fn retains_array_assignments_and_if_regions_for_runtime_proofs() {
        let source = r#"
let callbacks = null;
if (!callbacks) callbacks = [];
if (Array.isArray(callbacks)) callbacks.push(fn);
"#;
        let facts = extract("/project/runtime.js", source).unwrap();
        assert!(facts.assignments.iter().any(|assignment| {
            assignment.value == AssignmentValueKind::Array
                && source.get(assignment.target.start as usize..assignment.target.end as usize)
                    == Some("callbacks")
        }));
        assert!(facts.if_regions.iter().any(|region| {
            source.get(region.test.start as usize..region.test.end as usize)
                == Some("Array.isArray(callbacks)")
                && source
                    .get(region.consequent.start as usize..region.consequent.end as usize)
                    .is_some_and(|consequent| consequent.contains("callbacks.push(fn)"))
        }));
    }

    #[test]
    fn retains_complete_jsx_rule_structure() {
        let source = r#"<Button use:focus style={{ fontSize: 12 }} {...props}>
  {ready && <Content />}
  {ready ? <Yes /> : <No />}
</Button>"#;
        let facts = extract("rules.tsx", source).unwrap();
        let button = &facts.jsx_elements[0];

        assert!(!button.self_closing);
        assert_eq!(button.attributes.len(), 2);
        assert_eq!(button.spreads.len(), 1);
        assert_eq!(button.children.len(), 5);
        let directive = &button.attributes[0];
        assert_eq!(
            directive
                .namespace
                .and_then(|span| source.get(span.start as usize..span.end as usize)),
            Some("use")
        );
        assert_eq!(
            source.get(directive.local_name.start as usize..directive.local_name.end as usize),
            Some("focus")
        );
        assert_eq!(facts.object_properties.len(), 1);
        assert_eq!(facts.logical_expressions.len(), 1);
        assert_eq!(facts.conditional_expressions.len(), 1);
    }

    #[test]
    fn resolves_directive_names_through_value_scope_only() {
        let source = r#"
import { importedDirective } from "./directives";
import type { typeOnlyDirective } from "./types";
function hoistedDirective() {}
const moduleDirective = () => {};
const shadowedDirective = () => {};
interface interfaceDirective {}

function View(shadowedDirective: (element: HTMLDivElement) => void) {
  const parameterUse = <div use:shadowedDirective />;
  const visible = <div use:importedDirective use:hoistedDirective use:moduleDirective />;
  {
    const blockDirective = () => {};
    const blockUse = <div use:blockDirective />;
  }
  const outsideBlock = <div use:blockDirective />;
  const typeOnly = <div use:typeOnlyDirective use:interfaceDirective />;
  return <div class:moduleDirective use:missingDirective />;
}
"#;
        let facts = extract("directives.tsx", source).unwrap();
        let attributes = facts
            .jsx_elements
            .iter()
            .flat_map(|element| &element.attributes)
            .map(|attribute| {
                let namespace = attribute
                    .namespace
                    .and_then(|span| source.get(span.start as usize..span.end as usize));
                let name = source
                    .get(attribute.local_name.start as usize..attribute.local_name.end as usize)
                    .unwrap();
                let binding = attribute.directive_binding.and_then(|span| {
                    source
                        .get(span.start as usize..span.end as usize)
                        .map(|text| (text, span.start))
                });
                (namespace, name, binding)
            })
            .collect::<Vec<_>>();

        for name in [
            "importedDirective",
            "hoistedDirective",
            "moduleDirective",
            "blockDirective",
        ] {
            assert!(
                attributes.iter().any(|&(namespace, local, binding)| {
                    namespace == Some("use")
                        && local == name
                        && binding.is_some_and(|(declaration, _)| declaration == name)
                }),
                "missing lexical value resolution for {name}: {attributes:#?}"
            );
        }

        let shadowed = attributes
            .iter()
            .find(|&&(namespace, name, _)| namespace == Some("use") && name == "shadowedDirective")
            .and_then(|&(_, _, binding)| binding)
            .expect("shadowed parameter binding");
        assert_eq!(
            shadowed.1 as usize,
            source
                .find("shadowedDirective: (element")
                .expect("parameter declaration")
        );

        let unresolved = attributes
            .iter()
            .filter(|&&(namespace, _, binding)| namespace == Some("use") && binding.is_none())
            .map(|&(_, name, _)| name)
            .collect::<Vec<_>>();
        assert_eq!(
            unresolved,
            [
                "blockDirective",
                "typeOnlyDirective",
                "interfaceDirective",
                "missingDirective"
            ]
        );
        assert!(attributes.iter().any(|&(namespace, name, binding)| {
            namespace == Some("class") && name == "moduleDirective" && binding.is_none()
        }));
    }

    #[test]
    fn retains_untagged_template_interpolations() {
        let facts = extract(
            "App.tsx",
            "const label = `count is ${count} of ${total()}`;\n",
        )
        .unwrap();
        assert_eq!(facts.template_literals.len(), 1);
        let literal = &facts.template_literals[0];
        assert_eq!(literal.expressions.len(), 2);
        assert_eq!(
            literal
                .expressions
                .iter()
                .map(|slot| &"const label = `count is ${count} of ${total()}`;\n"
                    [slot.start as usize..slot.end as usize])
                .collect::<Vec<_>>(),
            ["count", "total()"]
        );
    }

    /// A tagged template owns a template literal, but a tag receives the
    /// values while an untagged literal stringifies them -- so a consumer
    /// asking "is this interpolated into a string" must not be answered by a
    /// tagged one.
    #[test]
    fn a_tagged_template_is_not_also_an_untagged_one() {
        let facts = extract("App.tsx", "const styled = css`color: ${theme}`;\n").unwrap();
        assert_eq!(
            facts.template_literals.len(),
            0,
            "a tag's quasi is filtered at collection; the table's untagged-only contract holds"
        );
        // An untagged template nested inside a tagged interpolation is still
        // an untagged template — the manual visit must reach it.
        let facts = extract("App.tsx", "const styled = css`color: ${`x${theme}`}`;\n").unwrap();
        assert_eq!(facts.template_literals.len(), 1);
    }

    /// A fragment has no element fact — no name, no attributes — so it gets
    /// its own span table, and a consumer asking "is this span inside JSX"
    /// must consult both. Only the element table answered that question
    /// before, which made fragment children invisible.
    #[test]
    fn fragments_are_recorded_beside_elements() {
        let facts = extract(
            "App.tsx",
            "const view = <>{outer()}<div>{inner()}</div></>;\n",
        )
        .unwrap();
        assert_eq!(facts.jsx_fragments.len(), 1);
        assert_eq!(facts.jsx_elements.len(), 1);
        let fragment = facts.jsx_fragments[0];
        let element = facts.jsx_elements[0].span;
        assert!(
            fragment.contains(element),
            "the fragment wraps the element: {fragment:?} vs {element:?}"
        );
    }
}
