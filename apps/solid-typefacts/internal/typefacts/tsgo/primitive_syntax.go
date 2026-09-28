package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// maxPrimitiveSyntaxDepth bounds primitiveBySyntax's descent through
// conditionals, logical operators and comma sequences. Past it the answer is
// false, which only refuses more.
const maxPrimitiveSyntaxDepth = 16

// primitiveBySyntaxOperators are the binary operators whose result is a
// primitive whatever their operands hold at run time: arithmetic, bitwise,
// shift, relational, equality, `in`, `instanceof`, and the compound
// assignments built on them. Each either completes with a number, a bigint, a
// string or a boolean, or throws.
var primitiveBySyntaxOperators = map[string]bool{
	"PlusToken":                                    true,
	"MinusToken":                                   true,
	"AsteriskToken":                                true,
	"AsteriskAsteriskToken":                        true,
	"SlashToken":                                   true,
	"PercentToken":                                 true,
	"LessThanLessThanToken":                        true,
	"GreaterThanGreaterThanToken":                  true,
	"GreaterThanGreaterThanGreaterThanToken":       true,
	"AmpersandToken":                               true,
	"BarToken":                                     true,
	"CaretToken":                                   true,
	"LessThanToken":                                true,
	"GreaterThanToken":                             true,
	"LessThanEqualsToken":                          true,
	"GreaterThanEqualsToken":                       true,
	"EqualsEqualsToken":                            true,
	"ExclamationEqualsToken":                       true,
	"EqualsEqualsEqualsToken":                      true,
	"ExclamationEqualsEqualsToken":                 true,
	"InKeyword":                                    true,
	"InstanceOfKeyword":                            true,
	"PlusEqualsToken":                              true,
	"MinusEqualsToken":                             true,
	"AsteriskEqualsToken":                          true,
	"AsteriskAsteriskEqualsToken":                  true,
	"SlashEqualsToken":                             true,
	"PercentEqualsToken":                           true,
	"LessThanLessThanEqualsToken":                  true,
	"GreaterThanGreaterThanEqualsToken":            true,
	"GreaterThanGreaterThanGreaterThanEqualsToken": true,
	"AmpersandEqualsToken":                         true,
	"BarEqualsToken":                               true,
	"CaretEqualsToken":                             true,
}

// primitiveBySyntax answers whether an expression's value is a primitive by
// its grammar alone, whatever any binding it reads holds at run time (ADR
// 0145, handshake protocol 66): a non-object literal, an untagged template,
// a prefix or postfix unary operator, `typeof`, `void`, `delete`, a binary
// operator from primitiveBySyntaxOperators, and a conditional, `&&`, `||`,
// `??` or plain `=` whose every value it can hand back is one, a comma
// sequence by its last operand. Identity-preserving wrappers are looked
// through.
//
// It exists because a checker type is not that proof in a JavaScript file:
// `let n = 0; n = {}; return n;` is typed `number` there, since an unchecked
// assignment does not widen a declaration's type, and a closure's read of a
// captured binding is typed by its declaration whatever the code between
// wrote. Every identifier, member, call, `new` and literal object is false
// here, which only refuses more.
func primitiveBySyntax(node *ast.Node, depth int) bool {
	node = identityPreservingUnwrap(node)
	if node == nil || depth > maxPrimitiveSyntaxDepth {
		return false
	}
	switch {
	case node.Kind == ast.KindTrueKeyword, node.Kind == ast.KindFalseKeyword,
		node.Kind == ast.KindNullKeyword:
		return true
	case ast.IsNumericLiteral(node), ast.IsStringLiteral(node),
		ast.IsNoSubstitutionTemplateLiteral(node):
		return true
	case ast.IsPrefixUnaryExpression(node):
		return true
	case ast.IsConditionalExpression(node):
		conditional := node.AsConditionalExpression()
		return primitiveBySyntax(conditional.WhenTrue, depth+1) &&
			primitiveBySyntax(conditional.WhenFalse, depth+1)
	case ast.IsBinaryExpression(node):
		binary := node.AsBinaryExpression()
		if binary == nil || binary.OperatorToken == nil {
			return false
		}
		operator := nodeKindName(binary.OperatorToken)
		switch {
		case primitiveBySyntaxOperators[operator]:
			return true
		case operator == "AmpersandAmpersandToken", operator == "BarBarToken",
			operator == "QuestionQuestionToken":
			return primitiveBySyntax(binary.Left, depth+1) && primitiveBySyntax(binary.Right, depth+1)
		case operator == "CommaToken", operator == "EqualsToken":
			return primitiveBySyntax(binary.Right, depth+1)
		}
		return false
	}
	switch nodeKindName(node) {
	case "BigIntLiteral", "TemplateExpression", "PostfixUnaryExpression",
		"TypeOfExpression", "VoidExpression", "DeleteExpression":
		return true
	}
	return false
}

// argumentsPrimitiveSyntax answers primitiveBySyntax for each written argument
// of a call (ADR 0146), in order; a spread is never one. An empty call answers
// an empty, non-nil list, so "no arguments" and "not stated" stay apart on the
// wire only by the source's presence.
func argumentsPrimitiveSyntax(call *ast.Node) []bool {
	if call == nil || !ast.IsCallExpression(call) {
		return nil
	}
	arguments := call.Arguments()
	answers := make([]bool, 0, len(arguments))
	for _, argument := range arguments {
		answers = append(answers, primitiveBySyntax(argument, 0))
	}
	return answers
}

// returnedCallExpression answers the exact location of the call expression a
// returned expression is, after identity-preserving wrappers (ADR 0146), and
// nil for every other expression: the site hands back what that call returned.
func returnedCallExpression(expression *ast.Node) *typefacts.Location {
	node := identityPreservingUnwrap(expression)
	if node == nil || !ast.IsCallExpression(node) {
		return nil
	}
	location := nodeLocation(node)
	return &location
}
