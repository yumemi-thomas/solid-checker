//! Oxc-derived structural facts.
//!
//! This crate is intentionally checker-independent and regex-free. It parses
//! original source once and exports finite, deterministic tables. Consumers
//! join these spans with TypeScript-Go semantic facts; Oxc nodes never escape.

use crate::core::{SourceIdentity, Span};
use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionExpression, AssignmentExpression,
    AssignmentTarget, AwaitExpression, BinaryExpression, BindingPattern, CallExpression,
    ComputedMemberExpression, ConditionalExpression, Declaration, ExportAllDeclaration,
    ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportNamedDeclaration, Expression,
    FormalParameter, Function, FunctionType, IdentifierReference, IfStatement, ImportDeclaration,
    ImportDeclarationSpecifier, JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXElement,
    JSXElementName, JSXExpression, LogicalExpression, LogicalOperator, ModuleExportName,
    NewExpression, ObjectProperty, ObjectPropertyKind, PropertyKey, PropertyKind, ReturnStatement,
    SpreadElement, StaticMemberExpression, TSModuleDeclarationName, UnaryExpression,
    UpdateExpression, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{ScopeId, Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span as OxcSpan};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const AST_FACTS_SCHEMA: u32 = 34;

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
    /// The subset of [`AstFacts::awaits`] proven to execute on every run of
    /// their innermost enclosing function (or of module evaluation): awaits
    /// with no conditional, logical, loop, switch, or try construct between
    /// the function entry and the expression. Code positioned after one of
    /// these is await-dominated in the straight-line sense — the conservative
    /// core the after-await member-read check builds on.
    #[serde(default)]
    pub unconditional_awaits: Vec<Span>,
    pub returns: Vec<ReturnFact>,
    pub jsx_elements: Vec<JsxElementFact>,
    /// JSX fragment spans (`<>…</>`). A fragment's children are as tracked
    /// as an element's, but it has no name and no attributes, so it gets a
    /// bare span rather than a [`JsxElementFact`]; consumers asking "is this
    /// span inside JSX" must consult both tables.
    #[serde(default)]
    pub jsx_fragments: Vec<Span>,
    /// Transparent TypeScript expression wrappers and the expression they
    /// contain. Consumers use this table instead of reproducing parser-node
    /// knowledge at each span-equality gate.
    #[serde(default)]
    pub transparent_wrappers: Vec<TransparentWrapperFact>,
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
    /// Operands in TypeScript-valid coercions where a function object is
    /// provably observed as a value: string-literal concatenation, logical
    /// not, and the unary numeric coercions. *Binary* arithmetic and bitwise
    /// positions are deliberately absent because TypeScript rejects a function
    /// operand there (`f + 1` is TS2365, `f - 1`/`f * 2`/`f | 0` are TS2362,
    /// probed against solid-js@1.9.14 in both passes). The unary forms are
    /// not: `-f`, `+f`, and `~f` are all accepted, so a coerced accessor there
    /// is this checker's to report.
    #[serde(default)]
    pub coercive_operands: Vec<CoerciveOperandFact>,
    #[serde(default)]
    pub assignments: Vec<AssignmentFact>,
    #[serde(default)]
    pub if_regions: Vec<IfRegionFact>,
    /// Module-level string directives (`"use server"`, `"use strict"`, …):
    /// the statements the parser classifies as the module's directive
    /// prologue, in source order, carrying the cooked directive text. A
    /// string literal after the first non-directive statement is an ordinary
    /// expression statement and is deliberately absent.
    #[serde(default)]
    pub module_directives: Vec<DirectiveFact>,
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
    /// Statically-named properties of an object-literal argument whose value
    /// is a string literal, carrying the cooked string value.
    #[serde(default)]
    pub string_properties: Vec<StringPropertyFact>,
    /// The key spans of every statically-named data property when the
    /// argument is an object literal, regardless of value shape. Presence in
    /// this list proves `"name" in options` at runtime; absence proves
    /// nothing unless [`ArgumentFact::exact_object_literal`] also holds.
    #[serde(default)]
    pub property_names: Vec<Span>,
    /// True when the argument is an object literal whose complete property
    /// set is statically known: every property is a plain data property with
    /// a static identifier key — no spreads, no computed keys, no accessors.
    /// Only such a literal proves the *absence* or the *final value* of an
    /// option.
    #[serde(default)]
    pub exact_object_literal: bool,
    /// True when the argument is an object literal with a closed, statically
    /// named property set. Unlike `exact_object_literal`, accessors are
    /// allowed because they still cannot add a hidden spread/computed key;
    /// consumers may prove key absence but must not infer final property
    /// values from this flag.
    #[serde(default)]
    pub closed_object_literal: bool,
    /// Normalized runtime shape underneath transparent TypeScript wrappers.
    /// This is shared by arguments and object-property values so consumers do
    /// not rebuild a partial literal taxonomy from source text.
    #[serde(default)]
    pub runtime_value_kind: RuntimeValueKind,
    /// Declaration this argument's identifier refers to, as resolved by the
    /// binder for that exact reference.
    ///
    /// The same contract as [`ObjectPropertyFact::shorthand_binding`]: Oxc's
    /// scope tree resolved the reference, so the declaration it chose is
    /// recorded rather than left to be re-derived from the spelling. It exists
    /// so a *demand* can follow `save(payload)` to the literal `payload` was
    /// built from without either resolving names by text or sweeping every
    /// binding in the file. A consumer proving something still resolves the
    /// symbol semantically; this only decides what to ask the compiler about.
    ///
    /// `None` for a non-identifier argument, and for an identifier the binder
    /// resolves to no declaration in this file's scope tree — an import or a
    /// global. Consumers fail closed on `None`; it is the absence of a fact,
    /// never proof of a missing binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_declaration: Option<Span>,
    /// The span of the runtime value expression when TypeScript sugar
    /// (parentheses, `as`, `satisfies`, `!`) wraps it; `None` when the
    /// argument span already is the runtime value.
    #[serde(default)]
    pub value_span: Option<Span>,
    /// True when a TypeScript assertion or non-null assertion can make the
    /// static call valid without changing the runtime value. Parentheses and
    /// `satisfies` are transparent too, but cannot launder an invalid value.
    #[serde(default)]
    pub runtime_type_escape: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgumentValueKind {
    Undefined,
    Null,
    Identifier,
    Function,
    AsyncFunction,
    Other,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeValueKind {
    #[default]
    Unknown,
    Nullish,
    Primitive,
    Function,
    Object,
    Array,
}

impl RuntimeValueKind {
    #[must_use]
    pub const fn is_proven_noncallable(self) -> bool {
        matches!(
            self,
            Self::Nullish | Self::Primitive | Self::Object | Self::Array
        )
    }

    /// A literal carrying data rather than behavior: a primitive, object, or
    /// array literal. This is [`Self::is_proven_noncallable`] without the
    /// nullish arm, which callers keep separate because `null`/`undefined`
    /// answer a different question — absence, rather than a wrong value.
    #[must_use]
    pub const fn is_data_literal(self) -> bool {
        matches!(self, Self::Primitive | Self::Object | Self::Array)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BooleanPropertyFact {
    pub name: Span,
    pub value: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringPropertyFact {
    pub name: Span,
    pub value: CompactString,
}

/// One string directive from a directive prologue (module or function body),
/// carrying its span and cooked text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectiveFact {
    pub span: Span,
    pub value: CompactString,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_slots: Vec<ObjectBindingFact>,
    /// Whether this binding is a `const` variable declarator. Parameters and
    /// mutable declarations are false.
    #[serde(default)]
    pub immutable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_initializer: Option<Span>,
    pub initializer_function: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer_identifier: Option<NamedSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectBindingFact {
    pub property: CompactString,
    pub local: NamedSpan,
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
    /// The property name for an object-literal or class method. This is kept
    /// separate from `name`: a method is not a lexical binding, but its
    /// canonical property symbol is still the exact callee target for
    /// interprocedural summaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method_name: Option<NamedSpan>,
    pub parameters: Vec<BindingFact>,
    /// Whether the declaration ends in a rest parameter (`...rest`). The rest
    /// binding is deliberately *not* one of `parameters`: it has no single
    /// argument index. It absorbs every argument from index
    /// `parameters.len()` onward, so a consumer reasoning about argument slots
    /// must treat that tail as observable rather than as unnamed.
    #[serde(default)]
    pub rest_parameter: bool,
    pub r#async: bool,
    pub generator: bool,
    pub expression_body: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression_return: Option<ReturnFact>,
    /// The string directives of this function body's directive prologue
    /// (`"use server"`, …), in source order. Always empty for
    /// expression-bodied arrows, which have no statement prologue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<CompactString>,
}

impl FunctionFact {
    /// Whether this function's own directive prologue contains `directive`.
    #[must_use]
    pub fn has_directive(&self, directive: &str) -> bool {
        self.directives.iter().any(|value| value == directive)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSpan {
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparentWrapperFact {
    pub span: Span,
    pub inner: Span,
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
    /// Whether Oxc resolved at least one runtime-value reference to this
    /// binding. A syntactic value import used only in type positions is
    /// normally erased by TypeScript and therefore does not load its module.
    #[serde(default)]
    pub runtime_referenced: bool,
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
    pub control_tests: Box<[Span]>,
    pub value: ReturnValueKind,
    pub conditional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callee: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Box<ReturnStructureFact>>,
}

impl ReturnFact {
    #[must_use]
    pub fn elements(&self) -> &[Option<Span>] {
        self.structure
            .as_deref()
            .map_or(&[], |structure| structure.elements.as_slice())
    }

    #[must_use]
    pub fn properties(&self) -> &[ReturnPropertyFact] {
        self.structure
            .as_deref()
            .map_or(&[], |structure| structure.properties.as_slice())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnStructureFact {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Option<Span>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<ReturnPropertyFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnPropertyFact {
    pub name: CompactString,
    pub value: Span,
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
    /// True when an assertion or non-null assertion can make the attribute
    /// type-check without changing its runtime value. `satisfies` and
    /// parentheses are transparent but do not launder an invalid value.
    #[serde(default)]
    pub runtime_type_escape: bool,
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
    /// Runtime shape of the property value after transparent TypeScript
    /// wrappers are removed.
    #[serde(default)]
    pub value_kind: RuntimeValueKind,
    /// Whether a type or non-null assertion can make this property value
    /// statically acceptable without changing what reaches the runtime.
    #[serde(default)]
    pub runtime_type_escape: bool,
    /// Whether this is a plain **data** property: `kind: Init` and not a
    /// method. A getter, setter, or method makes `value` a function whose body
    /// runs on access, so the property's runtime value is not the thing
    /// written here.
    ///
    /// This is the fact that lets a consumer prove a literal's property set is
    /// *closed against accessors*, and so conclude something about every value
    /// in it. Without it `{ get when() { return new Date(); } }` is
    /// indistinguishable from `{ when: "2026-01-01" }` — the first would read
    /// as JSON-safe when it is not. `ArgumentFact::exact_object_literal`
    /// carries the same guarantee for a literal written directly as an
    /// argument; this carries it per property, so a literal in any position
    /// can be judged.
    #[serde(default)]
    pub data: bool,
    /// Declaration this shorthand property's value binding refers to, as
    /// resolved by the binder for that exact reference.
    ///
    /// A shorthand (`{ pathname }`) writes one identifier where a key and a
    /// value both stand, and TypeScript answers a symbol query at that span
    /// with the *property's* symbol, never the value binding's. Oxc's scope
    /// tree does resolve the value reference, so the declaration it chose is
    /// recorded here rather than left to be re-derived from the spelling.
    ///
    /// `None` for a non-shorthand property, and for a shorthand whose value
    /// the binder resolves to no declaration in this file's scope tree -- an
    /// import namespace member or a global. Consumers fail closed on `None`;
    /// it is the absence of a fact, never proof of a missing binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shorthand_binding: Option<Span>,
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
    StringConcatenation,
    LogicalNot,
    /// Unary `+`, `-`, or `~`: the operand is read through `ToNumber`, and
    /// TypeScript accepts a function there.
    NumericCoercion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoerciveOperandFact {
    pub span: Span,
    pub kind: CoerciveOperandKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentFact {
    pub target: Span,
    pub value_span: Span,
    pub value: AssignmentValueKind,
    /// Whether evaluating the assignment reads the previous target value.
    /// Plain `=` does not; compound assignments and update expressions do.
    #[serde(default)]
    pub reads_target: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_initializer: Option<Span>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub array_slots: Vec<Option<Span>>,
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
    /// Whether `span` is *exactly* the target an assignment overwrites without
    /// reading the previous value — plain `=` and nothing else.
    ///
    /// Span identity is the whole point: a span merely *contained* in the
    /// target is a genuine read. `state.rows[props.index].done = true` writes
    /// one member and reads `props.index` to address it, and
    /// `({ a: local = profile.fallback } = obj)` reads `profile.fallback` to
    /// build the default. Compound assignments and update expressions are
    /// excluded outright: they evaluate the old value before storing the new
    /// one, so their target is a read as well as a write.
    #[must_use]
    pub fn is_plain_assignment_target(&self, span: Span) -> bool {
        self.assignments
            .iter()
            .any(|assignment| !assignment.reads_target && assignment.target == span)
    }

    /// Peel only syntax that preserves the runtime value of an expression.
    ///
    /// The parser already exposes the equivalent node-level operation as
    /// [`Expression::get_inner_expression`]. Structural facts are serialized
    /// as spans, so this small shared adapter preserves that operation for
    /// consumers that no longer have Oxc nodes. It intentionally does not
    /// peel arbitrary conversions, calls, or member expressions.
    #[must_use]
    pub fn peel_ts_sugar_span(&self, mut span: Span) -> Span {
        // `transparent_wrappers` is sorted by span, and this runs at every
        // callee and value resolution, so the lookup is a binary search
        // rather than a scan of every wrapper in the file.
        while let Ok(index) = self
            .transparent_wrappers
            .binary_search_by_key(&span, |wrapper| wrapper.span)
        {
            let wrapper = &self.transparent_wrappers[index];
            if wrapper.inner == span {
                break;
            }
            span = wrapper.inner;
        }
        span
    }

    /// The non-array bindings an export statement itself declares: inside the
    /// export's span, but not inside the body of any function the same export
    /// contains (those belong to the function, not the export surface).
    pub fn exported_bindings<'a>(
        &'a self,
        export: &'a ExportFact,
    ) -> impl Iterator<Item = &'a BindingFact> {
        self.bindings.iter().filter(move |binding| {
            binding.shape != BindingShape::Array
                && export.span.contains(binding.declaration)
                && !self.functions.iter().any(|function| {
                    export.span.contains(function.span)
                        && function.body.contains(binding.declaration)
                })
        })
    }

    /// An all-empty fact table for a source that carries no JavaScript
    /// syntax at all, such as a `.json` module.
    ///
    /// This is a proof, not an approximation: a JSON module's namespace is
    /// data, so it certifiably contributes no call, no binding, no function,
    /// no JSX, no read, no write -- every table is correctly empty because
    /// there is nothing in the source for any of them to describe. Consumers
    /// resolving a member or call against this file's exports terminate on a
    /// proven "not a function" rather than falling back to the conservative
    /// "unresolved import" path.
    #[must_use]
    pub fn empty(source: SourceIdentity) -> Self {
        Self {
            schema: AST_FACTS_SCHEMA,
            source,
            span_index: LazySpanIndex::default(),
            calls: Vec::new(),
            bindings: Vec::new(),
            functions: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            identifiers: Vec::new(),
            awaits: Vec::new(),
            unconditional_awaits: Vec::new(),
            returns: Vec::new(),
            jsx_elements: Vec::new(),
            jsx_fragments: Vec::new(),
            transparent_wrappers: Vec::new(),
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
            module_directives: Vec::new(),
        }
    }

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

/// Whether `path`'s extension marks it as a JSON module rather than
/// JavaScript or TypeScript.
///
/// A `.json` specifier is legitimate ESM (`import pkg from "./package.json"`,
/// with or without a bundler-inferred `type: "json"` assertion): the module
/// system resolves it to a real file, but that file has no JS grammar to
/// speak of. This is a fact about the module *kind*, never about one
/// filename -- `package.json`, `data.json`, and any other `.json` specifier
/// all take the same inert path through [`extract`].
#[must_use]
pub fn is_json_module_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

pub fn extract(path: impl Into<String>, source: &str) -> Result<AstFacts, AstFactsError> {
    let path = path.into();
    let identity = SourceIdentity::new(path.clone(), source)?;
    if is_json_module_path(&path) {
        // Enroll the JSON module as an analyzed file with a proven-empty
        // fact table instead of asking Oxc's JS/TS parser to make sense of
        // non-JS content, or failing the whole build closed on a source the
        // module graph legitimately reaches. See [`AstFacts::empty`] for why
        // "empty" here is a proof of inertness, not an approximation.
        return Ok(AstFacts::empty(identity));
    }
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
    unconditional_awaits: Vec<Span>,
    returns: Vec<ReturnFact>,
    jsx_elements: Vec<JsxElementFact>,
    jsx_fragments: Vec<Span>,
    transparent_wrappers: Vec<TransparentWrapperFact>,
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
    module_directives: Vec<DirectiveFact>,
    conditional_control_stack: Vec<Span>,
    method_names: Vec<Option<NamedSpan>>,
    /// How many conditional/repeated/aborting constructs (if, ternary,
    /// logical right operand, loop, switch case, try) enclose the current
    /// node. Compared against the depth recorded at the innermost function
    /// entry to decide whether an await is in that function's straight-line
    /// flow.
    conditional_flow_depth: usize,
    /// The [`Collector::conditional_flow_depth`] at each enclosing function's
    /// entry, innermost last.
    function_flow_depths: Vec<usize>,
}

#[derive(Default)]
struct BindingMetadata {
    initializer: Option<OxcSpan>,
    call_initializer: Option<OxcSpan>,
    initializer_function: bool,
    initializer_identifier: Option<NamedSpan>,
    immutable: bool,
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
            unconditional_awaits: Vec::new(),
            returns: Vec::new(),
            jsx_elements: Vec::new(),
            jsx_fragments: Vec::new(),
            transparent_wrappers: Vec::new(),
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
            module_directives: Vec::new(),
            conditional_control_stack: Vec::new(),
            method_names: Vec::new(),
            conditional_flow_depth: 0,
            function_flow_depths: Vec::new(),
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
        self.unconditional_awaits.sort_unstable();
        self.returns.sort_by_key(|fact| fact.span);
        self.jsx_elements.sort_by_key(|fact| fact.span);
        self.jsx_fragments.sort();
        self.transparent_wrappers.sort_by_key(|fact| fact.span);
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
        self.module_directives.sort_by_key(|fact| fact.span);
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
            unconditional_awaits: self.unconditional_awaits,
            returns: self.returns,
            jsx_elements: self.jsx_elements,
            jsx_fragments: self.jsx_fragments,
            transparent_wrappers: self.transparent_wrappers,
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
            module_directives: self.module_directives,
        }
    }

    fn binding_fact(
        &self,
        declaration: OxcSpan,
        pattern: &BindingPattern<'_>,
        metadata: BindingMetadata,
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
            object_slots: match pattern {
                BindingPattern::ObjectPattern(object) => object
                    .properties
                    .iter()
                    .filter_map(|property| {
                        let local = property
                            .value
                            .get_binding_identifiers()
                            .into_iter()
                            .next()?;
                        let property_name = self
                            .source
                            .get(
                                property.key.span().start as usize
                                    ..property.key.span().end as usize,
                            )?
                            .trim_matches(['\'', '"']);
                        Some(ObjectBindingFact {
                            property: property_name.into(),
                            local: NamedSpan {
                                span: span(local.span),
                            },
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            },
            immutable: metadata.immutable,
            initializer: metadata.initializer.map(span),
            call_initializer: metadata.call_initializer.map(span),
            initializer_function: metadata.initializer_function,
            initializer_identifier: metadata.initializer_identifier,
        }
    }

    fn return_fact(&self, expression: Option<&Expression<'_>>, fallback: OxcSpan) -> ReturnFact {
        let Some(expression) = expression else {
            return ReturnFact {
                span: span(fallback),
                argument: None,
                control_tests: self.conditional_control_stack.clone().into_boxed_slice(),
                value: ReturnValueKind::Undefined,
                conditional: false,
                callee: None,
                structure: None,
            };
        };
        let argument_span = span(expression.span());
        let expression = expression.get_inner_expression();
        let elements = match expression {
            Expression::ArrayExpression(array) => array
                .elements
                .iter()
                .map(|element| {
                    (!matches!(
                        element,
                        ArrayExpressionElement::Elision(_)
                            | ArrayExpressionElement::SpreadElement(_)
                    ))
                    .then(|| span(element.span()))
                })
                .collect(),
            _ => Vec::new(),
        };
        let properties = match expression {
            Expression::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let key = self
                        .source
                        .get(property.key.span().start as usize..property.key.span().end as usize)?
                        .trim_matches(['\'', '"']);
                    (!key.is_empty()).then(|| ReturnPropertyFact {
                        name: key.into(),
                        value: span(property.value.span()),
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        let conditional = matches!(expression, Expression::ConditionalExpression(_));
        let (value, callee) = match expression {
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                (ReturnValueKind::Function, None)
            }
            Expression::Identifier(identifier) if self.is_global_undefined(identifier) => {
                (ReturnValueKind::Undefined, None)
            }
            Expression::Identifier(_) => (ReturnValueKind::Identifier, None),
            Expression::CallExpression(call) => {
                (ReturnValueKind::Call, Some(span(call.callee.span())))
            }
            Expression::NewExpression(expression) => {
                (ReturnValueKind::Call, Some(span(expression.callee.span())))
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
            control_tests: self.conditional_control_stack.clone().into_boxed_slice(),
            value,
            conditional,
            callee,
            structure: (!elements.is_empty() || !properties.is_empty()).then(|| {
                Box::new(ReturnStructureFact {
                    elements,
                    properties,
                })
            }),
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

    /// The declaration a shorthand property's value binding refers to.
    ///
    /// The binder already resolved this exact reference, so its answer is
    /// used rather than a scope-chain lookup by spelling: it distinguishes
    /// sibling block scopes, and an unresolved reference stays `None` instead
    /// of silently matching a same-named binding somewhere else in the file.
    fn shorthand_binding(&self, property: &ObjectProperty<'_>) -> Option<Span> {
        if !property.shorthand {
            return None;
        }
        let Expression::Identifier(value) = &property.value else {
            return None;
        };
        let symbol = self
            .scoping
            .get_reference(value.reference_id.get()?)
            .symbol_id()?;
        Some(span(self.scoping.symbol_span(symbol)))
    }

    fn argument_fact(&self, argument: &Argument<'_>) -> ArgumentFact {
        let value = match argument {
            Argument::Identifier(identifier) if self.is_global_undefined(identifier) => {
                ArgumentValueKind::Undefined
            }
            Argument::Identifier(_) => ArgumentValueKind::Identifier,
            Argument::NullLiteral(_) => ArgumentValueKind::Null,
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
        let string_properties = match argument {
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
                    let Expression::StringLiteral(value) = &property.value else {
                        return None;
                    };
                    Some(StringPropertyFact {
                        name: span(key.span),
                        value: value.value.as_str().into(),
                    })
                })
                .collect(),
            _ => vec![],
        };
        // Property completeness belongs to the runtime object, not to the
        // transparent TypeScript wrapper around it. This keeps
        // `({ effect }) as EffectBundle` equivalent to `{ effect }` and lets
        // consumers prove absence on `{} as SomeObject` without parsing
        // source text.
        let expression = argument.as_expression().map(peel_ts_sugar);
        let property_names = match expression {
            Some(Expression::ObjectExpression(object)) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    Some(span(key.span))
                })
                .collect(),
            _ => vec![],
        };
        // Getter/setter pairs are `ObjectProperty` nodes with a non-`Init`
        // kind; the presence of a `get loadingValue()` still puts the key in
        // `property_names` above (an `in` check sees it), but a literal that
        // contains one no longer proves final option *values*, so it is not
        // exact.
        let exact_object_literal = match expression {
            Some(Expression::ObjectExpression(object)) => {
                object.properties.iter().all(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    property.kind == PropertyKind::Init
                        && matches!(&property.key, PropertyKey::StaticIdentifier(_))
                })
            }
            _ => false,
        };
        let closed_object_literal = match expression {
            Some(Expression::ObjectExpression(object)) => {
                object.properties.iter().all(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    matches!(&property.key, PropertyKey::StaticIdentifier(_))
                })
            }
            _ => false,
        };
        // Classify the runtime value behind TypeScript sugar so downstream
        // rules can reason about what actually reaches the call: a
        // `value as const` still passes the literal, and `target!` still
        // passes the member chain.
        let runtime_value_kind = expression.map_or(RuntimeValueKind::Unknown, |expression| {
            self.runtime_value_kind(expression)
        });
        let value_span = expression
            .map(|expression| span(expression.span()))
            .filter(|inner| *inner != span(argument.span()));
        let runtime_type_escape = argument
            .as_expression()
            .is_some_and(contains_runtime_type_escape);
        let binding_declaration = match argument {
            Argument::Identifier(identifier) => identifier
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                .map(|symbol| span(self.scoping.symbol_span(symbol))),
            _ => None,
        };
        ArgumentFact {
            span: span(argument.span()),
            binding_declaration,
            spread: argument.is_spread(),
            value,
            boolean_properties,
            identifier_properties,
            string_properties,
            property_names,
            exact_object_literal,
            closed_object_literal,
            runtime_value_kind,
            value_span,
            runtime_type_escape,
        }
    }

    fn is_global_undefined(&self, identifier: &IdentifierReference<'_>) -> bool {
        identifier.name == "undefined"
            && identifier.reference_id.get().is_some_and(|reference| {
                self.scoping.get_reference(reference).symbol_id().is_none()
            })
    }

    fn runtime_value_kind(&self, expression: &Expression<'_>) -> RuntimeValueKind {
        match peel_ts_sugar(expression) {
            Expression::NullLiteral(_) => RuntimeValueKind::Nullish,
            Expression::Identifier(identifier) if self.is_global_undefined(identifier) => {
                RuntimeValueKind::Nullish
            }
            Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::TemplateLiteral(_)
            | Expression::RegExpLiteral(_) => RuntimeValueKind::Primitive,
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                RuntimeValueKind::Function
            }
            Expression::ObjectExpression(_) => RuntimeValueKind::Object,
            Expression::ArrayExpression(_) => RuntimeValueKind::Array,
            _ => RuntimeValueKind::Unknown,
        }
    }
}

fn contains_runtime_type_escape(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::TSAsExpression(_)
        | Expression::TSTypeAssertion(_)
        | Expression::TSNonNullExpression(_) => true,
        Expression::ParenthesizedExpression(expression) => {
            contains_runtime_type_escape(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            contains_runtime_type_escape(&expression.expression)
        }
        _ => false,
    }
}

/// Return the runtime-value expression underneath one or more transparent
/// TypeScript wrappers. This is the node-level half of
/// [`AstFacts::peel_ts_sugar_span`].
#[must_use]
pub fn peel_ts_sugar<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    let mut expression = expression;
    loop {
        let Some(inner) = transparent_inner_expression(expression) else {
            return expression;
        };
        expression = inner;
    }
}

fn transparent_inner_expression<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::ParenthesizedExpression(expression) => Some(&expression.expression),
        Expression::TSAsExpression(expression) => Some(&expression.expression),
        Expression::TSTypeAssertion(expression) => Some(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => Some(&expression.expression),
        Expression::TSNonNullExpression(expression) => Some(&expression.expression),
        _ => None,
    }
}

fn static_property_name(key: &PropertyKey<'_>) -> Option<NamedSpan> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(NamedSpan {
            span: span(identifier.span),
        }),
        _ => None,
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

    fn visit_program(&mut self, program: &oxc_ast::ast::Program<'a>) {
        // The parser has already classified the directive prologue: only the
        // leading string-literal statements land in `directives`, so this is
        // a transcription, not a re-derivation.
        self.module_directives
            .extend(program.directives.iter().map(|directive| DirectiveFact {
                span: span(directive.span),
                value: directive.expression.value.as_str().into(),
            }));
        walk::walk_program(self, program);
    }

    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if let Some(inner) = transparent_inner_expression(expression) {
            self.transparent_wrappers.push(TransparentWrapperFact {
                span: span(expression.span()),
                inner: span(inner.span()),
            });
        }
        walk::walk_expression(self, expression);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let callee_span = call.callee.span();
        self.calls.push(CallFact {
            span: span(call.span),
            callee: span(callee_span),
            direct_callee: matches!(call.callee, Expression::Identifier(_)),
            type_arguments: call.type_arguments.is_some(),
            arguments: call
                .arguments
                .iter()
                .map(|argument| self.argument_fact(argument))
                .collect(),
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
                .map(|argument| self.argument_fact(argument))
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
            BindingMetadata {
                initializer,
                call_initializer,
                initializer_function,
                initializer_identifier,
                immutable: declaration.kind == oxc_ast::ast::VariableDeclarationKind::Const,
            },
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
                method_name: self.method_names.last().cloned().flatten(),
                parameters: function
                    .params
                    .items
                    .iter()
                    .map(|parameter| {
                        self.binding_fact(
                            parameter.span,
                            &parameter.pattern,
                            BindingMetadata::default(),
                        )
                    })
                    .collect(),
                rest_parameter: function.params.rest.is_some(),
                r#async: function.r#async,
                generator: function.generator,
                expression_body: false,
                expression_return: None,
                directives: body
                    .directives
                    .iter()
                    .map(|directive| directive.expression.value.as_str().into())
                    .collect(),
            });
        }
        self.function_flow_depths.push(self.conditional_flow_depth);
        walk::walk_function(self, function, flags);
        self.function_flow_depths.pop();
    }

    fn visit_method_definition(&mut self, method: &oxc_ast::ast::MethodDefinition<'a>) {
        self.method_names.push(static_property_name(&method.key));
        walk::walk_method_definition(self, method);
        self.method_names.pop();
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
            method_name: None,
            parameters: function
                .params
                .items
                .iter()
                .map(|parameter| {
                    self.binding_fact(
                        parameter.span,
                        &parameter.pattern,
                        BindingMetadata::default(),
                    )
                })
                .collect(),
            rest_parameter: function.params.rest.is_some(),
            r#async: function.r#async,
            generator: false,
            expression_body: function.expression,
            expression_return,
            // An expression-bodied arrow has no statement prologue; a
            // block-bodied one carries its directives like any function.
            directives: if function.expression {
                Vec::new()
            } else {
                function
                    .body
                    .directives
                    .iter()
                    .map(|directive| directive.expression.value.as_str().into())
                    .collect()
            },
        });
        self.function_flow_depths.push(self.conditional_flow_depth);
        walk::walk_arrow_function_expression(self, function);
        self.function_flow_depths.pop();
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
                runtime_referenced: local.symbol_id.get().is_some_and(|symbol| {
                    self.scoping
                        .get_resolved_references(symbol)
                        .any(|reference| {
                            reference.is_value() && !reference.flags().is_value_as_type()
                        })
                }),
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
                runtime_referenced: true,
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
        if self.conditional_flow_depth
            == self
                .function_flow_depths
                .last()
                .copied()
                .unwrap_or_default()
        {
            self.unconditional_awaits.push(span(expression.span));
        }
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
        self.conditional_flow_depth += 1;
        self.visit_statement(&statement.consequent);
        if let Some(alternate) = &statement.alternate {
            self.visit_statement(alternate);
        }
        self.conditional_flow_depth -= 1;
        self.conditional_control_stack.pop();
    }

    /// A switch discriminant selects among the statement's clauses exactly as
    /// an if test selects a branch: it joins the conditional-test table, and
    /// returns inside the clauses carry it as a control test.
    fn visit_switch_statement(&mut self, statement: &oxc_ast::ast::SwitchStatement<'a>) {
        let discriminant = span(statement.discriminant.span());
        self.conditional_tests.push(discriminant);
        self.conditional_control_stack.push(discriminant);
        self.conditional_flow_depth += 1;
        walk::walk_switch_statement(self, statement);
        self.conditional_flow_depth -= 1;
        self.conditional_control_stack.pop();
    }

    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'a>) {
        let plain = expression.operator == AssignmentOperator::Assign;
        let inner = expression.right.get_inner_expression();
        self.assignments.push(AssignmentFact {
            target: span(expression.left.span()),
            value_span: span(expression.right.span()),
            value: if plain {
                match expression.right {
                    Expression::ArrayExpression(_) => AssignmentValueKind::Array,
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                        AssignmentValueKind::Function
                    }
                    _ => AssignmentValueKind::Other,
                }
            } else {
                AssignmentValueKind::Other
            },
            reads_target: !plain,
            call_initializer: if plain {
                match inner {
                    Expression::CallExpression(call) => Some(span(call.span)),
                    _ => None,
                }
            } else {
                None
            },
            array_slots: if plain {
                match &expression.left {
                    AssignmentTarget::ArrayAssignmentTarget(array) => array
                        .elements
                        .iter()
                        .map(|element| element.as_ref().map(|element| span(element.span())))
                        .collect(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            },
        });
        walk::walk_assignment_expression(self, expression);
    }

    fn visit_update_expression(&mut self, expression: &UpdateExpression<'a>) {
        self.assignments.push(AssignmentFact {
            target: span(expression.argument.span()),
            value_span: span(expression.span()),
            value: AssignmentValueKind::Other,
            reads_target: true,
            call_initializer: None,
            array_slots: Vec::new(),
        });
        walk::walk_update_expression(self, expression);
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
        self.visit_expression(&expression.test);
        self.conditional_flow_depth += 1;
        self.visit_expression(&expression.consequent);
        self.visit_expression(&expression.alternate);
        self.conditional_flow_depth -= 1;
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
        self.visit_expression(&expression.left);
        self.conditional_flow_depth += 1;
        self.visit_expression(&expression.right);
        self.conditional_flow_depth -= 1;
    }

    fn visit_for_statement(&mut self, statement: &oxc_ast::ast::ForStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_for_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_for_in_statement(&mut self, statement: &oxc_ast::ast::ForInStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_for_in_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_for_of_statement(&mut self, statement: &oxc_ast::ast::ForOfStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_for_of_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_while_statement(&mut self, statement: &oxc_ast::ast::WhileStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_while_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_do_while_statement(&mut self, statement: &oxc_ast::ast::DoWhileStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_do_while_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_try_statement(&mut self, statement: &oxc_ast::ast::TryStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_try_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        let method_name = property
            .method
            .then(|| static_property_name(&property.key))
            .flatten();
        self.method_names.push(method_name);
        self.object_properties.push(ObjectPropertyFact {
            span: span(property.span),
            key: span(property.key.span()),
            value: span(property.value.span()),
            computed: property.computed,
            value_kind: self.runtime_value_kind(&property.value),
            runtime_type_escape: contains_runtime_type_escape(&property.value),
            shorthand_binding: self.shorthand_binding(property),
            data: property.kind == PropertyKind::Init && !property.method,
        });
        walk::walk_object_property(self, property);
        self.method_names.pop();
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
                    let (value, expression, runtime_type_escape, value_kind) =
                        match attribute.value.as_ref() {
                            None => (None, None, false, JsxAttributeValueKind::Boolean),
                            Some(JSXAttributeValue::StringLiteral(value)) => (
                                Some(span(value.span)),
                                None,
                                false,
                                JsxAttributeValueKind::String,
                            ),
                            Some(JSXAttributeValue::ExpressionContainer(container)) => (
                                Some(span(container.span)),
                                Some(span(container.expression.span())),
                                container
                                    .expression
                                    .as_expression()
                                    .is_some_and(contains_runtime_type_escape),
                                JsxAttributeValueKind::Expression,
                            ),
                            Some(JSXAttributeValue::Element(value)) => (
                                Some(span(value.span)),
                                None,
                                false,
                                JsxAttributeValueKind::Element,
                            ),
                            Some(JSXAttributeValue::Fragment(value)) => (
                                Some(span(value.span)),
                                None,
                                false,
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
                        runtime_type_escape,
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

        // Most binary operators reject function operands themselves, so a
        // checker finding there would duplicate TypeScript. Keep only the
        // syntactically proven string-concatenation subset: `+` with a string
        // literal on one side. Broader concatenation requires the operator's
        // resolved signature, which this structural fact deliberately does
        // not guess.
        if expression.operator == BinaryOperator::Addition
            && (matches!(&expression.left, Expression::StringLiteral(_))
                || matches!(&expression.right, Expression::StringLiteral(_)))
        {
            self.coercive_operands.extend([
                CoerciveOperandFact {
                    span: span(expression.left.span()),
                    kind: CoerciveOperandKind::StringConcatenation,
                },
                CoerciveOperandFact {
                    span: span(expression.right.span()),
                    kind: CoerciveOperandKind::StringConcatenation,
                },
            ]);
        }
        walk::walk_binary_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        use oxc_syntax::operator::UnaryOperator;

        // Every unary operator here accepts a function operand in TypeScript
        // (probed against the published typings: `-f`, `+f`, `~f`, and `!f`
        // are all clean in the strict and loose passes), so a coerced accessor
        // in one of these slots is this checker's to report rather than a
        // duplicate of a type error. `typeof`, `void`, and `delete` stay out:
        // they inspect the function itself.
        let kind = match expression.operator {
            UnaryOperator::LogicalNot => Some(CoerciveOperandKind::LogicalNot),
            UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
                Some(CoerciveOperandKind::NumericCoercion)
            }
            _ => None,
        };
        if let Some(kind) = kind {
            self.coercive_operands.push(CoerciveOperandFact {
                span: span(expression.argument.span()),
                kind,
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

    /// String directives: the module prologue lands in `module_directives`,
    /// each function's prologue lands on its `FunctionFact`, and a string
    /// literal after the first real statement is not a directive.
    #[test]
    fn extracts_module_and_function_directive_prologues() {
        let source = r#""use server";
"use strict";
export async function addTodo(title: string) {
  "use server";
  return title;
}
export const wrapped = async () => {
  "use server";
  return 1;
};
const notAPrologue = 1;
"not a directive";
export const short = async () => 2;
"#;
        let facts = extract("/project/api.ts", source).unwrap();
        assert_eq!(
            facts
                .module_directives
                .iter()
                .map(|directive| directive.value.as_str())
                .collect::<Vec<_>>(),
            ["use server", "use strict"]
        );
        let declaration = facts
            .functions
            .iter()
            .find(|function| function.kind == FunctionKind::Declaration)
            .unwrap();
        assert!(declaration.has_directive("use server"));
        assert!(!declaration.has_directive("use strict"));
        let arrows: Vec<_> = facts
            .functions
            .iter()
            .filter(|function| function.kind == FunctionKind::Arrow)
            .collect();
        assert_eq!(arrows.len(), 2);
        assert!(arrows.iter().any(|arrow| arrow.has_directive("use server")));
        // The expression-bodied arrow has no prologue at all.
        assert!(
            arrows
                .iter()
                .any(|arrow| arrow.expression_body && arrow.directives.is_empty())
        );
        // The trailing string literal sits after a statement and is not a
        // module directive.
        assert_eq!(facts.module_directives.len(), 2);
    }

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
    fn peels_only_transparent_typescript_wrappers_and_names_methods() {
        let source = r#"
declare const fn: () => number;
const wrapped = (((fn) as (() => number)) satisfies (() => number))!;
const object = { helper() { return fn(); } };
class Box { helper() { return fn(); } }
"#;
        let facts = extract("wrappers.ts", source).unwrap();
        let initializer = facts.bindings.iter().find_map(|binding| {
            (source.get(binding.pattern.start as usize..binding.pattern.end as usize)
                == Some("wrapped"))
            .then_some(binding.initializer)
            .flatten()
        });
        let initializer = initializer.expect("wrapped initializer");
        let peeled = facts.peel_ts_sugar_span(initializer);
        assert_eq!(&source[peeled.start as usize..peeled.end as usize], "fn");
        assert!(facts.transparent_wrappers.len() >= 3);
        let methods = facts
            .functions
            .iter()
            .filter_map(|function| function.method_name.as_ref())
            .map(|name| &source[name.span.start as usize..name.span.end as usize])
            .collect::<Vec<_>>();
        assert_eq!(methods, ["helper", "helper"]);
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
        let source =
            "createMemo(async () => 1, { sync: true, ownedWrite: false }); runWithOwner(null, f);";
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
        assert_eq!(facts.calls[1].arguments[0].value, ArgumentValueKind::Null);
    }

    #[test]
    fn object_completeness_survives_transparent_typescript_wrappers() {
        let source = "effect(compute, {} as unknown as Apply); effect(compute, ({ effect: apply }) satisfies Bundle); effect(compute, [] as unknown as Apply); effect(compute, null!); effect(compute, <Apply><unknown>5);";
        let facts = extract("effect.ts", source).unwrap();
        let empty = &facts.calls[0].arguments[1];
        assert_eq!(empty.runtime_value_kind, RuntimeValueKind::Object);
        assert!(empty.runtime_value_kind.is_data_literal());
        assert!(empty.exact_object_literal);
        assert!(empty.closed_object_literal);
        assert!(empty.property_names.is_empty());
        assert!(empty.runtime_type_escape);

        let bundle = &facts.calls[1].arguments[1];
        assert_eq!(bundle.runtime_value_kind, RuntimeValueKind::Object);
        assert!(bundle.exact_object_literal);
        assert!(bundle.closed_object_literal);
        assert!(!bundle.runtime_type_escape);
        assert_eq!(
            bundle
                .property_names
                .iter()
                .filter_map(|span| source.get(span.start as usize..span.end as usize))
                .collect::<Vec<_>>(),
            ["effect"]
        );

        let array = &facts.calls[2].arguments[1];
        assert_eq!(array.runtime_value_kind, RuntimeValueKind::Array);
        assert!(array.runtime_value_kind.is_data_literal());
        assert!(array.runtime_type_escape);

        let nullish = &facts.calls[3].arguments[1];
        assert_eq!(nullish.runtime_value_kind, RuntimeValueKind::Nullish);
        assert!(nullish.runtime_type_escape);

        let angle_asserted = &facts.calls[4].arguments[1];
        assert_eq!(
            angle_asserted.runtime_value_kind,
            RuntimeValueKind::Primitive
        );
        assert!(angle_asserted.runtime_type_escape);
    }

    #[test]
    fn closed_object_literals_allow_accessors_but_reject_hidden_keys() {
        let facts = extract(
            "objects.ts",
            "merge({ get value() { return 1; } }); merge({ ...other }); merge({ [key]: 1 });",
        )
        .unwrap();
        let getter = &facts.calls[0].arguments[0];
        assert!(getter.closed_object_literal);
        assert!(!getter.exact_object_literal);
        assert!(!facts.calls[1].arguments[0].closed_object_literal);
        assert!(!facts.calls[2].arguments[0].closed_object_literal);
    }

    #[test]
    fn global_undefined_is_distinct_from_a_shadowed_binding() {
        let facts = extract(
            "effect.ts",
            r#"effect(undefined!);
function run(undefined: () => void) {
    effect(undefined!);
    return undefined;
}"#,
        )
        .unwrap();

        assert_eq!(
            facts.calls[0].arguments[0].runtime_value_kind,
            RuntimeValueKind::Nullish
        );
        assert_eq!(
            facts.calls[1].arguments[0].runtime_value_kind,
            RuntimeValueKind::Unknown
        );
        assert_eq!(facts.returns[0].value, ReturnValueKind::Identifier);
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
            r#"import value, { type Shape, runtime as renamed, OnlyType } from "./dependency";
type Alias = OnlyType;
void value;
renamed();"#,
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
        assert!(
            !bindings
                .iter()
                .find(|binding| binding.imported.as_deref() == Some("Shape"))
                .unwrap()
                .runtime_referenced
        );
        assert!(
            !bindings
                .iter()
                .find(|binding| binding.imported.as_deref() == Some("OnlyType"))
                .unwrap()
                .runtime_referenced
        );
        assert!(
            bindings
                .iter()
                .find(|binding| binding.imported.as_deref() == Some("runtime"))
                .unwrap()
                .runtime_referenced
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
    fn a_json_import_target_is_certified_inert_rather_than_a_fatal_error() {
        // A real dependency's ESM entrypoint reaching its own `package.json`
        // (e.g. `@solidjs/start@2.0.3`'s `dist/shared/dev-toolbar/index.jsx`)
        // used to make the whole build fail with an Oxc "unsupported source
        // path" error, because Oxc's `SourceType` has no JSON source kind.
        // `extract` must instead certify the JSON module as inert: every
        // fact table empty, proven rather than assumed, and no error.
        let facts = extract("package.json", r#"{"name": "demo", "version": "1.0.0"}"#).unwrap();
        assert_eq!(facts.calls, Vec::new());
        assert_eq!(facts.bindings, Vec::new());
        assert_eq!(facts.functions, Vec::new());
        assert_eq!(facts.imports, Vec::new());
        assert_eq!(facts.exports, Vec::new());
        assert_eq!(facts.identifiers, Vec::new());
        assert_eq!(facts.jsx_elements, Vec::new());
        assert_eq!(facts.members, Vec::new());
    }

    #[test]
    fn json_module_detection_is_by_extension_not_by_filename() {
        // The rule is the `.json` module kind, not the specific filename
        // `package.json`: any `.json` specifier takes the inert path, and a
        // non-JSON extension does not, even when it is not `package.json`.
        assert!(is_json_module_path("package.json"));
        assert!(is_json_module_path("/some/pkg/data.json"));
        assert!(is_json_module_path("/some/pkg/DATA.JSON"));
        assert!(!is_json_module_path("package.jsonc"));
        assert!(!is_json_module_path("package.js"));
    }

    #[test]
    fn a_genuinely_unsupported_extension_still_fails_closed() {
        // The JSON fix must not become a blanket "skip anything we cannot
        // parse": a source reached through some other unsupported extension
        // is still a fatal AST facts error, exactly as before.
        assert!(matches!(
            extract("logo.svg", "<svg></svg>"),
            Err(AstFactsError::SourceType { .. })
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
    fn retains_only_typescript_valid_operators_that_coerce_accessor_values() {
        let source = "const a = signal + 1; const b = -signal; const c = !signal; const d = signal === other; const e = typeof signal; const f = \"value: \" + signal;";
        let facts = extract("operators.ts", source).unwrap();
        let operands = facts
            .coercive_operands
            .iter()
            .filter_map(|operand| {
                source.get(operand.span.start as usize..operand.span.end as usize)
            })
            .collect::<Vec<_>>();

        // `signal + 1` is TS2365 and stays out; `-signal`, `!signal`, and the
        // string-literal concatenation are all TypeScript-clean and stay in.
        assert_eq!(operands, ["signal", "signal", "\"value: \"", "signal"]);
    }

    #[test]
    fn retains_array_assignments_and_if_regions_for_runtime_proofs() {
        let source = r#"
let callbacks = null;
let resource;
if (!callbacks) callbacks = [];
if (Array.isArray(callbacks)) callbacks.push(fn);
[resource] = createResource(source, fetcher);
"#;
        let facts = extract("/project/runtime.js", source).unwrap();
        assert!(facts.assignments.iter().any(|assignment| {
            assignment.value == AssignmentValueKind::Array
                && source.get(assignment.target.start as usize..assignment.target.end as usize)
                    == Some("callbacks")
        }));
        let resource = facts
            .assignments
            .iter()
            .find(|assignment| !assignment.array_slots.is_empty())
            .unwrap();
        assert_eq!(
            resource
                .array_slots
                .first()
                .and_then(|slot| *slot)
                .and_then(|span| source.get(span.start as usize..span.end as usize)),
            Some("resource")
        );
        assert_eq!(
            resource
                .call_initializer
                .and_then(|span| source.get(span.start as usize..span.end as usize)),
            Some("createResource(source, fetcher)")
        );
        assert!(facts.if_regions.iter().any(|region| {
            source.get(region.test.start as usize..region.test.end as usize)
                == Some("Array.isArray(callbacks)")
                && source
                    .get(region.consequent.start as usize..region.consequent.end as usize)
                    .is_some_and(|consequent| consequent.contains("callbacks.push(fn)"))
        }));
    }

    #[test]
    fn assignment_facts_distinguish_plain_compound_and_update_reads() {
        let source = "let value = 0; value = 1; value += 2; value++;";
        let facts = extract("assignments.ts", source).unwrap();
        let reads = facts
            .assignments
            .iter()
            .map(|assignment| {
                (
                    source
                        .get(assignment.target.start as usize..assignment.target.end as usize)
                        .unwrap(),
                    assignment.reads_target,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            reads,
            vec![("value", false), ("value", true), ("value", true)]
        );
    }

    #[test]
    fn only_the_written_target_itself_is_a_plain_assignment_target() {
        let source = "state.rows[props.index].done = true; state.count += 1; ({ a: local = fallback.name } = incoming);";
        let facts = extract("targets.ts", source).unwrap();
        let plain = |text: &str| {
            let start = u32::try_from(source.find(text).unwrap()).unwrap();
            facts.is_plain_assignment_target(Span::new(
                start,
                start + u32::try_from(text.len()).unwrap(),
            ))
        };
        // The written member is a write; the computed key that addresses it,
        // the compound target that evaluates its old value, and the default
        // that builds a destructured value are all genuine reads.
        assert!(plain("state.rows[props.index].done"));
        assert!(!plain("props.index"));
        assert!(!plain("state.count"));
        assert!(!plain("fallback.name"));
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
    fn jsx_attributes_distinguish_runtime_type_escapes_from_checked_wrappers() {
        let source = r#"<div
  onClick={handler as unknown as () => void}
  onInput={handler satisfies () => void}
  onFocus={(handler)}
  onBlur={handler!}
/>"#;
        let facts = extract("attributes.tsx", source).unwrap();
        let element = &facts.jsx_elements[0];
        let escaped = element
            .attributes
            .iter()
            .filter(|attribute| attribute.runtime_type_escape)
            .filter_map(|attribute| {
                source.get(attribute.name.start as usize..attribute.name.end as usize)
            })
            .collect::<Vec<_>>();
        assert_eq!(escaped, ["onClick", "onBlur"]);
        for attribute in &element.attributes {
            let expression = attribute.expression.expect("expression attribute");
            let runtime = facts.peel_ts_sugar_span(expression);
            assert_eq!(
                source.get(runtime.start as usize..runtime.end as usize),
                Some("handler")
            );
        }
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
    fn resolves_shorthand_property_values_through_block_scope() {
        let source = r#"
import { importedValue } from "./values";

const moduleValue = () => "module";

function make() {
  {
    const scoped = () => "block";
    const fromBlock = { scoped };
  }
  const scoped = () => "function";
  const fromFunction = { scoped };
  const fromModule = { moduleValue };
  const fromImport = { importedValue };
  const fromGlobal = { structuredClone };
  const written = { scoped: moduleValue };
  const computed = { [moduleValue]: 1 };
}
"#;
        let facts = extract("shorthand.ts", source).unwrap();
        let text = |span: Span| source.get(span.start as usize..span.end as usize).unwrap();
        let resolved = facts
            .object_properties
            .iter()
            .map(|property| {
                (
                    text(property.span),
                    property
                        .shorthand_binding
                        .map(|binding| (text(binding), binding.start as usize)),
                )
            })
            .collect::<Vec<_>>();

        // Two `scoped` declarations in sibling scopes. Each shorthand takes
        // the one its own scope chain reaches -- the distinction the spelling
        // alone cannot make. Everything below is keyed off the declaration
        // offset, so a same-spelled sibling cannot satisfy the assertion.
        assert_eq!(
            resolved,
            [
                (
                    "scoped",
                    Some(("scoped", source.find("scoped = () => \"block\"").unwrap()))
                ),
                (
                    "scoped",
                    Some((
                        "scoped",
                        source.find("scoped = () => \"function\"").unwrap()
                    ))
                ),
                (
                    "moduleValue",
                    Some((
                        "moduleValue",
                        source.find("moduleValue = () => \"module\"").unwrap()
                    ))
                ),
                (
                    "importedValue",
                    Some((
                        "importedValue",
                        source.find("importedValue } from").unwrap()
                    ))
                ),
                // A global resolves to no declaration in this file's scope
                // tree, and a written or computed key is not a shorthand at
                // all. All three are the absence of a fact, never proof that
                // no binding exists.
                ("structuredClone", None),
                ("scoped: moduleValue", None),
                ("[moduleValue]: 1", None),
            ]
        );
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

    #[test]
    fn structured_returns_retain_tuple_slots_and_object_properties() {
        let source = r#"
function tuple() { return [store, accessor] as const; }
function object() { return { active: () => state(), pending: createMemo(() => state()) }; }
"#;
        let facts = extract("App.ts", source).unwrap();
        let tuple = facts
            .returns
            .iter()
            .find(|returned| !returned.elements().is_empty())
            .unwrap();
        assert_eq!(
            tuple
                .elements()
                .iter()
                .flatten()
                .map(|span| &source[span.start as usize..span.end as usize])
                .collect::<Vec<_>>(),
            ["store", "accessor"]
        );
        let object = facts
            .returns
            .iter()
            .find(|returned| !returned.properties().is_empty())
            .unwrap();
        assert_eq!(
            object
                .properties()
                .iter()
                .map(|property| property.name.as_str())
                .collect::<Vec<_>>(),
            ["active", "pending"]
        );
        let pending = object
            .properties()
            .iter()
            .find(|property| property.name == "pending")
            .unwrap();
        assert_eq!(
            source.get(pending.value.start as usize..pending.value.end as usize),
            Some("createMemo(() => state())")
        );
        assert!(facts.calls.iter().any(|call| call.span == pending.value));
    }
}
