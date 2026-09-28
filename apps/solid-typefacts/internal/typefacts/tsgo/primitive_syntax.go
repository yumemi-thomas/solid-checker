package tsgo

import (
	"path/filepath"
	"strings"

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

// primitiveBySyntaxLocked answers whether an expression's value is a primitive
// by its grammar alone, whatever any binding it reads holds at run time (ADR
// 0145, handshake protocol 66): a non-object literal, an untagged template,
// a prefix or postfix unary operator, `typeof`, `void`, `delete`, a binary
// operator from primitiveBySyntaxOperators, and a conditional, `&&`, `||`,
// `??` or plain `=` whose every value it can hand back is one, a comma
// sequence by its last operand. Identity-preserving wrappers are looked
// through.
//
// Two identifiers are one too (the 2026-09-28 amendment to ADR 0113,
// handshake protocol 67), because neither can hold anything but the value its
// grammar fixes:
//
//   - a `const` binding with a plain name, declared exactly once in the
//     reading file and never written, whose initializer is one by this same
//     rule: a `const` holds its initializer's value for its whole life;
//   - the intrinsic `undefined` -- the checker's own symbol, which has no
//     declaration, so a binding that shadows the name is not it.
//
// It exists because a checker type is not that proof in a JavaScript file:
// `let n = 0; n = {}; return n;` is typed `number` there, since an unchecked
// assignment does not widen a declaration's type, and a closure's read of a
// captured binding is typed by its declaration whatever the code between
// wrote. Every other identifier, and every member, call, `new` and literal
// object, is false here, which only refuses more.
func (p *project) primitiveBySyntaxLocked(node *ast.Node, depth int) bool {
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
	case ast.IsIdentifier(node):
		return p.primitiveIdentifierLocked(node, depth)
	case ast.IsConditionalExpression(node):
		conditional := node.AsConditionalExpression()
		return p.primitiveBySyntaxLocked(conditional.WhenTrue, depth+1) &&
			p.primitiveBySyntaxLocked(conditional.WhenFalse, depth+1)
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
			return p.primitiveBySyntaxLocked(binary.Left, depth+1) &&
				p.primitiveBySyntaxLocked(binary.Right, depth+1)
		case operator == "CommaToken", operator == "EqualsToken":
			return p.primitiveBySyntaxLocked(binary.Right, depth+1)
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

// primitiveIdentifierLocked is primitiveBySyntaxLocked's identifier arm: the
// intrinsic `undefined`, or a `const` whose initializer is a primitive by
// grammar. A `let`, a `var`, a parameter, a destructured or imported binding,
// a function or class, and a binding declared twice are all false: each can
// hold a value the grammar of its declaration does not fix.
func (p *project) primitiveIdentifierLocked(node *ast.Node, depth int) bool {
	if p.checker == nil {
		return false
	}
	symbol := p.checker.GetSymbolAtLocation(node)
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias != 0 {
		return false
	}
	if node.Text() == "undefined" {
		return symbol.Name == "undefined" && len(symbol.Declarations) == 0
	}
	if len(symbol.Declarations) != 1 {
		return false
	}
	declaration := symbol.Declarations[0]
	file := ast.GetSourceFileOfNode(node)
	if declaration == nil || file == nil || ast.GetSourceFileOfNode(declaration) != file ||
		!ast.IsVariableDeclaration(declaration) || !ast.IsVarConst(declaration) ||
		declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) ||
		p.symbolIsAssignedLocked(symbol, declaration) {
		return false
	}
	initializer := declaration.Initializer()
	if initializer == nil {
		return false
	}
	return p.primitiveBySyntaxLocked(initializer, depth+1)
}

// defaultLibraryCallLocked names the default-library member a returned call
// invokes, as `Receiver.member` (the 2026-09-28 amendment to ADR 0113,
// handshake protocol 67), when the returned expression is, after
// identity-preserving wrappers, a plain call -- not optional, not `new`, not
// tagged -- whose callee is a non-computed, non-optional member read of an
// identifier, where both the receiver and the member resolve to
// default-library declarations alone and the file neither writes, deletes nor
// lets escape either of them (immutableAliasLibrarySourceIsStable). Empty
// otherwise.
//
// The name is a fact about which built-in runs, by identity; what that
// built-in hands back is the consumer's reviewed question, never this one's.
// A member read of anything else -- a binding holding `Math`, an import, a
// parameter -- names nothing, since its value is only as good as the binding.
func (p *project) defaultLibraryCallLocked(expression *ast.Node) string {
	node := identityPreservingUnwrap(expression)
	if node == nil || !ast.IsCallExpression(node) || p.checker == nil ||
		node.QuestionDotToken() != nil {
		return ""
	}
	callee := identityPreservingUnwrap(node.Expression())
	if callee == nil || !ast.IsPropertyAccessExpression(callee) ||
		callee.QuestionDotToken() != nil {
		return ""
	}
	object := identityPreservingUnwrap(callee.Expression())
	if object == nil || !ast.IsIdentifier(object) {
		return ""
	}
	member := p.canonicalSymbol(p.checker.GetSymbolAtLocation(callee))
	receiver := p.canonicalSymbol(p.checker.GetSymbolAtLocation(object))
	if member == nil || receiver == nil ||
		!p.isDefaultLibraryMemberLocked(member, member.Name, nil) ||
		!p.isDefaultLibraryMemberLocked(receiver, receiver.Name, nil) {
		return ""
	}
	file := ast.GetSourceFileOfNode(node)
	if file == nil || !p.immutableAliasLibrarySourceIsStable(file, member, receiver) {
		return ""
	}
	return receiver.Name + "." + member.Name
}

// isTypeScriptSourceFile answers whether a file is TypeScript source -- not a
// declaration file and not JavaScript -- the one kind of file in which the
// checker holds every write to a binding to that binding's declared type (the
// 2026-09-28 amendment to ADR 0113, handshake protocol 67). The extension test
// mirrors isJavaScriptSourceFile.
func isTypeScriptSourceFile(sourceFile *ast.SourceFile) bool {
	if sourceFile == nil || sourceFile.IsDeclarationFile {
		return false
	}
	switch strings.ToLower(filepath.Ext(sourceFile.FileName())) {
	case ".ts", ".tsx", ".mts", ".cts":
		return true
	}
	return false
}

// argumentsPrimitiveSyntaxLocked answers primitiveBySyntaxLocked for each written argument
// of a call (ADR 0146), in order; a spread is never one. An empty call answers
// an empty, non-nil list, so "no arguments" and "not stated" stay apart on the
// wire only by the source's presence.
func (p *project) argumentsPrimitiveSyntaxLocked(call *ast.Node) []bool {
	if call == nil || !ast.IsCallExpression(call) {
		return nil
	}
	arguments := call.Arguments()
	answers := make([]bool, 0, len(arguments))
	for _, argument := range arguments {
		answers = append(answers, p.primitiveBySyntaxLocked(argument, 0))
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
