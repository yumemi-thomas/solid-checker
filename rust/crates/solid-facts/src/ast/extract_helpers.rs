//! Free helpers for extraction: expression peeling, binding and key helpers, export names, and chain parts.

use super::*;

pub(super) fn contains_runtime_type_escape(expression: &Expression<'_>) -> bool {
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

pub(super) fn transparent_inner_expression<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::ParenthesizedExpression(expression) => Some(&expression.expression),
        Expression::TSAsExpression(expression) => Some(&expression.expression),
        Expression::TSTypeAssertion(expression) => Some(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => Some(&expression.expression),
        Expression::TSNonNullExpression(expression) => Some(&expression.expression),
        _ => None,
    }
}

/// ADR 0468: the identifier a call's callee invokes when only the inert leading
/// operands of a comma expression (and parentheses, which the parser drops)
/// stand in front of it: `(void 0, f)()`, `(0, f)()`. Each calls the identifier's
/// value with an undefined receiver, as `f()` does: a comma applies GetValue,
/// but an identifier's Reference has no base object to lose, and `0` or
/// `void 0` evaluates to a constant with no effect. Not peeled, so the callee
/// stays the spelling as written:
///
/// - a member after a comma, which loses its receiver (`(0, obj.m)()` is not
///   `obj.m()`), and any other expression;
/// - a leading operand that can run code (`(change(), f)()`);
/// - `eval` after a comma: an indirect eval, not the direct one;
/// - a TypeScript wrapper: the facts ask for the type at the callee, and the
///   wrapper's type is not the identifier's.
///
/// ADR 0515: a plain assignment to a binding identifier is peeled to its right
/// operand, `(slot = f)()`: the expression's value is the right operand's
/// value (`f`, read once), the assignment to a plain binding runs no code, and
/// the call has an undefined receiver, as `f()` does. A compound or logical
/// assignment (`(slot ||= f)()`) yields `slot`'s value or `f`'s, and is not
/// peeled; nor is an assignment to a member or a pattern. `assignments` is
/// false in an `async` function: the after-await owner placement there reads
/// the assignment's operands, not this peel, and a peeled call was silent
/// where the plain call is a violation (fuzzer recall grid, 2026-10-31).
pub(super) fn invoked_identifier<'b, 'a>(
    assignments: bool,
    callee: &'b Expression<'a>,
) -> Option<&'b oxc_ast::ast::IdentifierReference<'a>> {
    fn inert(operand: &Expression<'_>) -> bool {
        match operand {
            Expression::NumericLiteral(_) => true,
            Expression::UnaryExpression(unary) => {
                unary.operator == oxc_ast::ast::UnaryOperator::Void
                    && matches!(unary.argument, Expression::NumericLiteral(_))
            }
            _ => false,
        }
    }
    let mut expression = callee;
    let mut through_comma = false;
    loop {
        match expression {
            Expression::ParenthesizedExpression(inner) => expression = &inner.expression,
            Expression::SequenceExpression(sequence) => {
                let (tail, prefix) = sequence.expressions.split_last()?;
                if !prefix.iter().all(inert) {
                    return None;
                }
                through_comma = true;
                expression = tail;
            }
            Expression::Identifier(identifier) => {
                return (!(through_comma && identifier.name == "eval")).then_some(identifier);
            }
            Expression::AssignmentExpression(assignment)
                if assignments
                    && assignment.operator == oxc_ast::ast::AssignmentOperator::Assign
                    && matches!(
                        assignment.left,
                        oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(_)
                    ) =>
            {
                // Like a comma, the value is the operand, read with no receiver.
                through_comma = true;
                expression = &assignment.right;
            }
            _ => return None,
        }
    }
}

/// What a binding-pattern element binds, and its default initializer.
pub(super) fn binding_target_and_default(pattern: &BindingPattern<'_>) -> (Span, Option<Span>) {
    match pattern {
        BindingPattern::AssignmentPattern(assignment) => (
            span(assignment.left.span()),
            Some(span(assignment.right.span())),
        ),
        other => (span(other.span()), None),
    }
}

/// What an assignment-pattern element assigns, and its default initializer.
pub(super) fn assignment_target_and_default(
    element: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
) -> (Span, Option<Span>) {
    match element {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
            (span(target.binding.span()), Some(span(target.init.span())))
        }
        other => (span(other.span()), None),
    }
}

/// The slot an object-pattern key projects: a key the syntax states (an
/// identifier, a string or a numeric literal, computed or not), or a key
/// computed at run time.
pub(super) fn pattern_key_slot(key: &PropertyKey<'_>, computed: bool) -> PatternSlot {
    let stated = match key {
        PropertyKey::StaticIdentifier(identifier) if !computed => {
            Some(CompactString::from(identifier.name.as_str()))
        }
        PropertyKey::StringLiteral(literal) => Some(CompactString::from(literal.value.as_str())),
        PropertyKey::NumericLiteral(literal) => literal
            .raw
            .as_ref()
            // Only a canonical integer spells its own runtime key.
            .filter(|raw| {
                raw.bytes().all(|byte| byte.is_ascii_digit())
                    && (raw.as_str() == "0" || !raw.starts_with('0'))
            })
            .map(|raw| CompactString::from(raw.as_str())),
        _ => None,
    };
    stated.map_or(PatternSlot::ComputedKey, PatternSlot::Key)
}

pub(super) fn static_property_name(key: &PropertyKey<'_>) -> Option<NamedSpan> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(NamedSpan {
            span: span(identifier.span),
        }),
        _ => None,
    }
}

pub(super) const fn span(value: OxcSpan) -> Span {
    Span::new(value.start, value.end)
}

pub(super) fn module_export_name(name: &ModuleExportName<'_>) -> CompactString {
    match name {
        ModuleExportName::IdentifierName(name) => name.name.as_str().into(),
        ModuleExportName::IdentifierReference(name) => name.name.as_str().into(),
        ModuleExportName::StringLiteral(name) => name.value.as_str().into(),
    }
}

pub(super) fn export_declaration_names(declaration: &Declaration<'_>) -> Vec<ExportSpecifierFact> {
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

pub(super) fn export_declaration_surface_names(
    declaration: &Declaration<'_>,
) -> Vec<ExportSpecifierFact> {
    let Declaration::TSModuleDeclaration(declaration) = declaration else {
        return vec![];
    };
    let TSModuleDeclarationName::Identifier(name) = &declaration.id else {
        return vec![];
    };
    vec![ExportSpecifierFact {
        local: NamedSpan {
            span: span(name.span),
        },
        exported: name.name.as_str().into(),
        type_only: false,
    }]
}

/// The one property key a literal names after transparent TypeScript wrappers:
/// a string or substitution-free template's cooked value (lone surrogates
/// refused), or a numeric literal whose value is a non-negative
/// integer no larger than `i32::MAX`, in the canonical decimal spelling
/// ToPropertyKey gives it. Everything else names no key statically.
pub(super) fn literal_property_key(expression: &Expression<'_>) -> Option<CompactString> {
    match peel_ts_sugar(expression) {
        Expression::StringLiteral(literal) if !literal.lone_surrogates => {
            Some(literal.value.as_str().into())
        }
        Expression::NumericLiteral(literal) => canonical_index_key(literal.value),
        Expression::TemplateLiteral(literal) if literal.expressions.is_empty() => literal
            .quasis
            .first()
            .filter(|quasi| !quasi.lone_surrogates)
            .and_then(|quasi| quasi.value.cooked.as_ref())
            .map(|key| key.as_str().into()),
        _ => None,
    }
}

pub(super) fn canonical_index_key(value: f64) -> Option<CompactString> {
    (value.fract() == 0.0 && (0.0..=f64::from(i32::MAX)).contains(&value)).then(|| {
        // Exact: an integral value in range converts without loss.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = value as u32;
        index.to_string().into()
    })
}

/// Record literal facts from the exact argument expression, without following
/// bindings or inferring properties from their declared types.
pub(super) fn argument_literal_fact(expression: &Expression<'_>, depth: u8) -> ArgumentLiteralFact {
    if depth >= 32 {
        return ArgumentLiteralFact::Unknown;
    }
    match peel_ts_sugar(expression) {
        Expression::NullLiteral(_) => ArgumentLiteralFact::Null,
        Expression::BooleanLiteral(value) => ArgumentLiteralFact::Boolean(value.value),
        Expression::NumericLiteral(value) => canonical_index_key(value.value)
            .and_then(|key| key.parse().ok())
            .map_or(ArgumentLiteralFact::Unknown, ArgumentLiteralFact::Integer),
        Expression::StringLiteral(value) if !value.lone_surrogates => {
            ArgumentLiteralFact::String(value.value.as_str().into())
        }
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
            ArgumentLiteralFact::Function
        }
        Expression::ArrayExpression(array)
            if !array
                .elements
                .iter()
                .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_))) =>
        {
            u32::try_from(array.elements.len()).map_or(
                ArgumentLiteralFact::Unknown,
                ArgumentLiteralFact::ArrayLength,
            )
        }
        Expression::ObjectExpression(object) => {
            let mut properties = Vec::<ArgumentLiteralPropertyFact>::new();
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return ArgumentLiteralFact::Unknown;
                };
                if property.kind != PropertyKind::Init || property.computed {
                    return ArgumentLiteralFact::Unknown;
                }
                let name: CompactString = match &property.key {
                    PropertyKey::StaticIdentifier(key) => key.name.as_str().into(),
                    PropertyKey::StringLiteral(key) if !key.lone_surrogates => {
                        key.value.as_str().into()
                    }
                    PropertyKey::NumericLiteral(key) => {
                        let Some(key) = canonical_index_key(key.value) else {
                            return ArgumentLiteralFact::Unknown;
                        };
                        key
                    }
                    _ => return ArgumentLiteralFact::Unknown,
                };
                if name == "__proto__" {
                    return ArgumentLiteralFact::Unknown;
                }
                properties.retain(|property| property.name != name);
                properties.push(ArgumentLiteralPropertyFact {
                    name,
                    value: argument_literal_fact(&property.value, depth + 1),
                });
            }
            ArgumentLiteralFact::Object(properties)
        }
        _ => ArgumentLiteralFact::Unknown,
    }
}

/// The statically exact members of an array or object literal: see
/// [`ArgumentFact::literal_members`].
pub(super) fn literal_members_of(expression: &Expression<'_>) -> Vec<ArgumentMemberFact> {
    match expression {
        Expression::ArrayExpression(array) => {
            let mut members = Vec::new();
            for (index, element) in array.elements.iter().enumerate() {
                match element {
                    ArrayExpressionElement::SpreadElement(_) => break,
                    ArrayExpressionElement::Elision(_) => {}
                    element => {
                        let Ok(index) = u32::try_from(index) else {
                            break;
                        };
                        members.push(ArgumentMemberFact {
                            key: index.to_string().into(),
                            value: span(element.span()),
                        });
                    }
                }
            }
            members
        }
        Expression::ObjectExpression(object) => {
            let mut members = Vec::<ArgumentMemberFact>::new();
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return Vec::new();
                };
                if property.kind != PropertyKind::Init || property.computed {
                    return Vec::new();
                }
                let key: CompactString = match &property.key {
                    PropertyKey::StaticIdentifier(key) => key.name.as_str().into(),
                    PropertyKey::StringLiteral(key) if !key.lone_surrogates => {
                        key.value.as_str().into()
                    }
                    PropertyKey::NumericLiteral(key) => {
                        let Some(key) = canonical_index_key(key.value) else {
                            return Vec::new();
                        };
                        key
                    }
                    _ => return Vec::new(),
                };
                // A non-shorthand `__proto__: value` sets the prototype rather
                // than a property, and a shorthand one is an ordinary property:
                // neither is worth the distinction here.
                if key == "__proto__" {
                    return Vec::new();
                }
                members.retain(|member| member.key != key);
                members.push(ArgumentMemberFact {
                    key,
                    value: span(property.value.span()),
                });
            }
            members
        }
        _ => Vec::new(),
    }
}

/// The elements of an array literal, after parentheses and type wrappers, in
/// order: `None` for a spread or a hole. `None` when the expression is not an
/// array literal at all (ADR 0115).
pub(super) fn array_literal_elements(expression: &Expression<'_>) -> Option<Box<[Option<Span>]>> {
    let Expression::ArrayExpression(array) = expression.get_inner_expression() else {
        return None;
    };
    Some(
        array
            .elements
            .iter()
            .map(|element| {
                (!matches!(
                    element,
                    ArrayExpressionElement::Elision(_) | ArrayExpressionElement::SpreadElement(_)
                ))
                .then(|| span(element.span()))
            })
            .collect(),
    )
}

/// One link of an optional chain's spine: whether it is a `?.` link, its own
/// span, the ranges only it evaluates (arguments, a computed key), and the
/// object or callee below it.
pub(super) fn chain_link<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<(
    bool,
    oxc_span::Span,
    Vec<oxc_span::Span>,
    &'e Expression<'a>,
)> {
    Some(match expression {
        Expression::CallExpression(call) => (
            call.optional,
            call.span,
            call.arguments.iter().map(GetSpan::span).collect(),
            &call.callee,
        ),
        Expression::StaticMemberExpression(member) => {
            (member.optional, member.span, Vec::new(), &member.object)
        }
        Expression::ComputedMemberExpression(member) => (
            member.optional,
            member.span,
            vec![member.expression.span()],
            &member.object,
        ),
        Expression::PrivateFieldExpression(member) => {
            (member.optional, member.span, Vec::new(), &member.object)
        }
        Expression::TSNonNullExpression(expression) => {
            (false, expression.span, Vec::new(), &expression.expression)
        }
        _ => return None,
    })
}

/// ADR 0276: the parts of an optional chain its `?.` links may skip. A link
/// is skipped, with what only it evaluates, when it or a link below it on the
/// spine is optional; the spine below the lowest `?.` always runs.
pub(crate) fn skippable_chain_parts(
    element: &oxc_ast::ast::ChainElement<'_>,
    sites: &mut Vec<Span>,
    ranges: &mut Vec<Span>,
) {
    use oxc_ast::ast::ChainElement;
    let root = match element {
        ChainElement::CallExpression(call) => (
            call.optional,
            call.span,
            call.arguments.iter().map(GetSpan::span).collect(),
            &call.callee,
        ),
        ChainElement::StaticMemberExpression(member) => {
            (member.optional, member.span, Vec::new(), &member.object)
        }
        ChainElement::ComputedMemberExpression(member) => (
            member.optional,
            member.span,
            vec![member.expression.span()],
            &member.object,
        ),
        ChainElement::PrivateFieldExpression(member) => {
            (member.optional, member.span, Vec::new(), &member.object)
        }
        ChainElement::TSNonNullExpression(expression) => {
            (false, expression.span, Vec::new(), &expression.expression)
        }
    };
    let mut spine = vec![(root.0, root.1, root.2)];
    let mut below = root.3;
    while let Some((optional, own, parts, next)) = chain_link(below) {
        spine.push((optional, own, parts));
        below = next;
    }
    let mut skipped = false;
    for (optional, own, parts) in spine.into_iter().rev() {
        skipped |= optional;
        if skipped {
            sites.push(span(own));
            ranges.extend(parts.into_iter().map(span));
        }
    }
}
