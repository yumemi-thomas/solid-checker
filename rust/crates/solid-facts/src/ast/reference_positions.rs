//! ADR 1114, families 2-4: the syntactic position of every identifier reference, from its parent node.
//!
//! The site universe (`solid-reactive-ir/src/proof_universe.rs`) listed only the positions it knew (a call's callee, a member chain's object);
//! a reference anywhere else was left to the value-use census, and a position neither named was a gap no gate saw. This fact gives every
//! `IdentifierReference` one [`ReferencePosition`], found by an exhaustive match on its parent's [`AstKind`] with no wildcard arm: an Oxc
//! release that adds a node kind fails this build, and a parent that cannot hold a reference in valid code is named [`ReferencePosition::Other`]
//! arm by arm. Transparent TypeScript wrappers (`as`, `satisfies`, `!`, `<T>x`, `f<T>`) and parentheses are climbed, so the position is the
//! one the value lands in, and `at` is the outermost wrapper's span.

use oxc_ast::AstKind;
use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, BindingPattern, JSXElementName, UnaryOperator,
};
use oxc_semantic::{AstNodes, NodeId};
use oxc_span::GetSpan;

use super::extract_helpers::span as to_span;
use super::types::{ReferencePosition, ReferencePositionFact};

/// Every identifier reference's position, sorted by the reference's span.
pub(super) fn reference_positions(nodes: &AstNodes<'_>) -> Vec<ReferencePositionFact> {
    let mut out = Vec::new();
    for node in nodes.iter() {
        let AstKind::IdentifierReference(identifier) = node.kind() else {
            continue;
        };
        let span = to_span(identifier.span);
        let (position, at) = position_of(nodes, node.id(), identifier.span);
        out.push(ReferencePositionFact {
            span,
            at: to_span(at),
            position,
        });
    }
    out.sort_by_key(|fact| (fact.span.start, fact.span.end));
    out
}

/// The position of the node `id`, whose span is `child`, climbing transparent wrappers; and the outermost wrapper's span.
fn position_of(
    nodes: &AstNodes<'_>,
    id: NodeId,
    child: oxc_span::Span,
) -> (ReferencePosition, oxc_span::Span) {
    let mut id = id;
    let mut child = child;
    loop {
        let parent_id = nodes.parent_id(id);
        if parent_id == id {
            return (ReferencePosition::Other, child);
        }
        let parent = nodes.kind(parent_id);
        if let Some(position) = slot(nodes, parent_id, parent, child) {
            return (position, child);
        }
        // A transparent wrapper: the value is the wrapper's.
        child = parent.span();
        id = parent_id;
    }
}

const fn index(position: usize) -> u32 {
    if position > u32::MAX as usize {
        u32::MAX
    } else {
        position as u32
    }
}

/// The position `child` holds in `parent`; `None` for a transparent wrapper, which the caller climbs.
#[allow(clippy::too_many_lines)]
fn slot(
    nodes: &AstNodes<'_>,
    parent_id: NodeId,
    parent: AstKind<'_>,
    child: oxc_span::Span,
) -> Option<ReferencePosition> {
    use ReferencePosition as P;
    let position = match parent {
        // Transparent: the value passes through unchanged.
        AstKind::ParenthesizedExpression(_)
        | AstKind::TSAsExpression(_)
        | AstKind::TSSatisfiesExpression(_)
        | AstKind::TSNonNullExpression(_)
        | AstKind::TSTypeAssertion(_)
        | AstKind::TSInstantiationExpression(_) => return None,
        AstKind::CallExpression(call) => {
            if call.callee.span() == child {
                P::Callee
            } else {
                P::Argument {
                    index: index(
                        call.arguments
                            .iter()
                            .position(|argument| argument.span() == child)
                            .unwrap_or(usize::MAX),
                    ),
                }
            }
        }
        AstKind::NewExpression(call) => {
            if call.callee.span() == child {
                P::NewCallee
            } else {
                P::NewArgument {
                    index: index(
                        call.arguments
                            .iter()
                            .position(|argument| argument.span() == child)
                            .unwrap_or(usize::MAX),
                    ),
                }
            }
        }
        // `%Name(x)` (V8 natives syntax, parsed only when enabled): an argument of a call into the engine.
        AstKind::V8IntrinsicExpression(call) => P::Argument {
            index: index(
                call.arguments
                    .iter()
                    .position(|argument| argument.span() == child)
                    .unwrap_or(usize::MAX),
            ),
        },
        AstKind::TaggedTemplateExpression(_) => P::TaggedTemplateTag,
        AstKind::TemplateLiteral(_) => P::TemplateExpression,
        AstKind::StaticMemberExpression(_) | AstKind::PrivateFieldExpression(_) => P::MemberObject,
        AstKind::ComputedMemberExpression(member) => {
            if member.object.span() == child {
                P::MemberObject
            } else {
                P::ComputedKey
            }
        }
        AstKind::AssignmentExpression(assignment) => {
            if assignment.left.span() == child {
                P::AssignmentTarget {
                    compound: assignment.operator != AssignmentOperator::Assign,
                }
            } else {
                P::AssignmentValue {
                    pattern: matches!(
                        assignment.left,
                        AssignmentTarget::ObjectAssignmentTarget(_)
                            | AssignmentTarget::ArrayAssignmentTarget(_)
                    ),
                }
            }
        }
        AstKind::UpdateExpression(_) => P::UpdateOperand,
        AstKind::ArrayAssignmentTarget(_) | AstKind::AssignmentTargetRest(_) => P::PatternTarget,
        AstKind::AssignmentTargetWithDefault(target) => {
            if target.init.span() == child {
                P::PatternDefault
            } else {
                P::PatternTarget
            }
        }
        AstKind::AssignmentTargetPropertyIdentifier(target) => {
            if target.binding.span == child {
                P::PatternTarget
            } else {
                P::PatternDefault
            }
        }
        AstKind::AssignmentTargetPropertyProperty(target) => {
            if target.computed && target.name.span() == child {
                P::PropertyKey
            } else {
                P::PatternTarget
            }
        }
        AstKind::VariableDeclarator(declarator) => P::Initializer {
            pattern: !matches!(declarator.id, BindingPattern::BindingIdentifier(_)),
        },
        AstKind::AssignmentPattern(_) | AstKind::FormalParameter(_) => P::PatternDefault,
        AstKind::BindingProperty(_) => P::PropertyKey,
        AstKind::ObjectProperty(property) => {
            if property.computed && property.key.span() == child {
                P::PropertyKey
            } else {
                P::PropertyValue {
                    shorthand: property.shorthand,
                }
            }
        }
        AstKind::PropertyDefinition(property) => {
            if property.computed && property.key.span() == child {
                P::PropertyKey
            } else {
                P::ClassFieldValue
            }
        }
        AstKind::AccessorProperty(property) => {
            if property.computed && property.key.span() == child {
                P::PropertyKey
            } else {
                P::ClassFieldValue
            }
        }
        AstKind::MethodDefinition(_) => P::PropertyKey,
        AstKind::SpreadElement(_) => P::Spread,
        AstKind::ArrayExpression(_) => P::ArrayElement,
        AstKind::ReturnStatement(_) => P::Return,
        AstKind::ExpressionStatement(_) => {
            // A concise arrow's body is an expression statement in a function body the arrow marks `expression`.
            let body = nodes.parent_id(parent_id);
            let arrow = nodes.parent_kind(body);
            if matches!(nodes.kind(body), AstKind::FunctionBody(_))
                && matches!(arrow, AstKind::ArrowFunctionExpression(function) if function.expression)
            {
                P::ArrowBody
            } else {
                P::Statement
            }
        }
        AstKind::YieldExpression(_) => P::Yield,
        AstKind::AwaitExpression(_) => P::Await,
        AstKind::ThrowStatement(_) => P::Throw,
        AstKind::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::Typeof => P::Typeof,
            UnaryOperator::Void => P::Void,
            UnaryOperator::Delete => P::Delete,
            UnaryOperator::UnaryNegation
            | UnaryOperator::UnaryPlus
            | UnaryOperator::LogicalNot
            | UnaryOperator::BitwiseNot => P::UnaryOperand,
        },
        AstKind::BinaryExpression(_) => P::BinaryOperand,
        AstKind::PrivateInExpression(_) => P::PrivateIn,
        AstKind::LogicalExpression(_) => P::LogicalOperand,
        AstKind::ConditionalExpression(conditional) => {
            if conditional.test.span() == child {
                P::ConditionalTest
            } else {
                P::ConditionalBranch
            }
        }
        AstKind::SequenceExpression(sequence) => P::Sequence {
            last: sequence
                .expressions
                .last()
                .is_some_and(|last| last.span() == child),
        },
        AstKind::IfStatement(_) | AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) => {
            P::Test
        }
        AstKind::ForStatement(statement) => {
            if statement
                .test
                .as_ref()
                .is_some_and(|test| test.span() == child)
            {
                P::Test
            } else if statement
                .update
                .as_ref()
                .is_some_and(|update| update.span() == child)
            {
                P::ForUpdate
            } else {
                P::ForInit
            }
        }
        AstKind::ForInStatement(statement) => {
            if statement.right.span() == child {
                P::ForInOfSource
            } else {
                P::ForInOfTarget
            }
        }
        AstKind::ForOfStatement(statement) => {
            if statement.right.span() == child {
                P::ForInOfSource
            } else {
                P::ForInOfTarget
            }
        }
        AstKind::SwitchStatement(_) => P::SwitchDiscriminant,
        AstKind::SwitchCase(_) => P::CaseTest,
        AstKind::WithStatement(_) => P::WithObject,
        AstKind::JSXExpressionContainer(_) => P::JsxExpression,
        AstKind::JSXSpreadAttribute(_) | AstKind::JSXSpreadChild(_) => P::JsxSpread,
        AstKind::JSXOpeningElement(element) => {
            if matches!(element.name, JSXElementName::IdentifierReference(_)) {
                P::JsxTag
            } else {
                P::Other
            }
        }
        AstKind::JSXClosingElement(_) => P::JsxTag,
        AstKind::JSXMemberExpression(_) => P::JsxMemberObject,
        AstKind::ExportSpecifier(_) => P::ExportSpecifier,
        AstKind::ExportDefaultDeclaration(_) => P::ExportDefault,
        AstKind::Class(_) => P::ClassExtends,
        AstKind::Decorator(_) => P::Decorator,
        AstKind::ImportExpression(_) => P::ImportSource,
        AstKind::TSExportAssignment(_) => P::TsExport,
        AstKind::TSEnumMember(_) => P::EnumInitializer,
        AstKind::TSImportEqualsDeclaration(_) => P::TsImportAlias,
        // TypeScript type positions: erased, no runtime read.
        AstKind::TSTypeQuery(_)
        | AstKind::TSQualifiedName(_)
        | AstKind::TSTypeReference(_)
        | AstKind::TSClassImplements(_)
        | AstKind::TSInterfaceHeritage(_)
        | AstKind::TSImportTypeQualifiedName(_)
        | AstKind::TSTypePredicate(_)
        | AstKind::TSImportType(_)
        | AstKind::TSExternalModuleReference(_) => P::TypeOnly,
        // A parent that holds no IdentifierReference as a direct child in valid code: named, so a new kind fails the build here.
        AstKind::Program(_)
        | AstKind::IdentifierName(_)
        | AstKind::IdentifierReference(_)
        | AstKind::BindingIdentifier(_)
        | AstKind::LabelIdentifier(_)
        | AstKind::ThisExpression(_)
        | AstKind::Elision(_)
        | AstKind::ObjectExpression(_)
        | AstKind::TemplateElement(_)
        | AstKind::MetaProperty(_)
        | AstKind::ObjectAssignmentTarget(_)
        | AstKind::Super(_)
        | AstKind::ChainExpression(_)
        | AstKind::Directive(_)
        | AstKind::Hashbang(_)
        | AstKind::BlockStatement(_)
        | AstKind::VariableDeclaration(_)
        | AstKind::EmptyStatement(_)
        | AstKind::ContinueStatement(_)
        | AstKind::BreakStatement(_)
        | AstKind::LabeledStatement(_)
        | AstKind::TryStatement(_)
        | AstKind::CatchClause(_)
        | AstKind::CatchParameter(_)
        | AstKind::DebuggerStatement(_)
        | AstKind::ObjectPattern(_)
        | AstKind::ArrayPattern(_)
        | AstKind::BindingRestElement(_)
        | AstKind::Function(_)
        | AstKind::FormalParameters(_)
        | AstKind::FormalParameterRest(_)
        | AstKind::FunctionBody(_)
        | AstKind::ArrowFunctionExpression(_)
        | AstKind::ClassBody(_)
        | AstKind::PrivateIdentifier(_)
        | AstKind::StaticBlock(_)
        | AstKind::ImportDeclaration(_)
        | AstKind::ImportSpecifier(_)
        | AstKind::ImportDefaultSpecifier(_)
        | AstKind::ImportNamespaceSpecifier(_)
        | AstKind::WithClause(_)
        | AstKind::ImportAttribute(_)
        | AstKind::ExportNamedDeclaration(_)
        | AstKind::ExportAllDeclaration(_)
        | AstKind::BooleanLiteral(_)
        | AstKind::NullLiteral(_)
        | AstKind::NumericLiteral(_)
        | AstKind::StringLiteral(_)
        | AstKind::BigIntLiteral(_)
        | AstKind::RegExpLiteral(_)
        | AstKind::JSXElement(_)
        | AstKind::JSXFragment(_)
        | AstKind::JSXOpeningFragment(_)
        | AstKind::JSXClosingFragment(_)
        | AstKind::JSXNamespacedName(_)
        | AstKind::JSXEmptyExpression(_)
        | AstKind::JSXAttribute(_)
        | AstKind::JSXIdentifier(_)
        | AstKind::JSXText(_)
        | AstKind::TSThisParameter(_)
        | AstKind::TSEnumDeclaration(_)
        | AstKind::TSEnumBody(_)
        | AstKind::TSTypeAnnotation(_)
        | AstKind::TSLiteralType(_)
        | AstKind::TSConditionalType(_)
        | AstKind::TSUnionType(_)
        | AstKind::TSIntersectionType(_)
        | AstKind::TSParenthesizedType(_)
        | AstKind::TSTypeOperator(_)
        | AstKind::TSArrayType(_)
        | AstKind::TSIndexedAccessType(_)
        | AstKind::TSTupleType(_)
        | AstKind::TSNamedTupleMember(_)
        | AstKind::TSOptionalType(_)
        | AstKind::TSRestType(_)
        | AstKind::TSAnyKeyword(_)
        | AstKind::TSStringKeyword(_)
        | AstKind::TSBooleanKeyword(_)
        | AstKind::TSNumberKeyword(_)
        | AstKind::TSNeverKeyword(_)
        | AstKind::TSIntrinsicKeyword(_)
        | AstKind::TSUnknownKeyword(_)
        | AstKind::TSNullKeyword(_)
        | AstKind::TSUndefinedKeyword(_)
        | AstKind::TSVoidKeyword(_)
        | AstKind::TSSymbolKeyword(_)
        | AstKind::TSThisType(_)
        | AstKind::TSObjectKeyword(_)
        | AstKind::TSBigIntKeyword(_)
        | AstKind::TSTypeParameterInstantiation(_)
        | AstKind::TSTypeParameter(_)
        | AstKind::TSTypeParameterDeclaration(_)
        | AstKind::TSTypeAliasDeclaration(_)
        | AstKind::TSInterfaceDeclaration(_)
        | AstKind::TSInterfaceBody(_)
        | AstKind::TSPropertySignature(_)
        | AstKind::TSIndexSignature(_)
        | AstKind::TSCallSignatureDeclaration(_)
        | AstKind::TSMethodSignature(_)
        | AstKind::TSConstructSignatureDeclaration(_)
        | AstKind::TSIndexSignatureName(_)
        | AstKind::TSModuleDeclaration(_)
        | AstKind::TSGlobalDeclaration(_)
        | AstKind::TSModuleBlock(_)
        | AstKind::TSTypeLiteral(_)
        | AstKind::TSInferType(_)
        | AstKind::TSFunctionType(_)
        | AstKind::TSConstructorType(_)
        | AstKind::TSMappedType(_)
        | AstKind::TSTemplateLiteralType(_)
        | AstKind::TSNamespaceExportDeclaration(_)
        | AstKind::JSDocNullableType(_)
        | AstKind::JSDocNonNullableType(_)
        | AstKind::JSDocUnknownType(_) => P::Other,
    };
    Some(position)
}
