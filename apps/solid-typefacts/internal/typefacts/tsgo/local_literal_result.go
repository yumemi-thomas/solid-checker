package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// localLiteralResultLocked follows only value-preserving local names to one
// call. A property of the result is an arbitrary value, so member chains do
// not inherit this premise. The call's execution remains a separate demand.
func (p *project) localLiteralResultLocked(subject *ast.Node) *typefacts.LocalLiteralResultPremise {
	node := identityPreservingUnwrap(subject)
	seen := make(map[*ast.Node]bool)
	for depth := 0; node != nil && depth < 8; depth++ {
		if ast.IsCallExpression(node) {
			return p.localLiteralCallResultLocked(node)
		}
		if !ast.IsIdentifier(node) {
			return nil
		}
		declaration := p.singleUnwrittenLocalInitializerLocked(node)
		if declaration == nil || seen[declaration] {
			return nil
		}
		seen[declaration] = true
		node = identityPreservingUnwrap(declaration.Initializer())
	}
	return nil
}

func literalResultBoundary(node *ast.Node) bool {
	switch nodeKindName(node) {
	case "FunctionDeclaration", "FunctionExpression", "ArrowFunction", "MethodDeclaration",
		"GetAccessor", "SetAccessor", "Constructor", "ClassDeclaration", "ClassExpression":
		return true
	}
	return false
}

// Every return must identify the same allocation in this function, and a
// terminal return must explicitly cover fallthrough. Async/generator wrapping
// changes the completion value and is deliberately not admitted. Mutation of
// data properties has the existing own-literal premise; replacing the binding
// does not. No missing return or missing type is read as positive evidence.
func (p *project) localLiteralCallResultLocked(call *ast.Node) *typefacts.LocalLiteralResultPremise {
	callee := p.formRuntimeCalleeDeclarationLocked(call)
	if callee == nil || ast.HasSyntacticModifier(callee, ast.ModifierFlagsAsync) || mentionsArgumentsOrEval(callee) {
		return nil
	}
	if ast.IsFunctionDeclaration(callee) && callee.AsFunctionDeclaration().AsteriskToken != nil {
		return nil
	}
	if ast.IsFunctionExpression(callee) && callee.AsFunctionExpression().AsteriskToken != nil {
		return nil
	}
	body := callee.Body()
	if body == nil || !ast.IsBlock(body) || body.AsBlock().Statements == nil {
		return nil
	}
	statements := body.AsBlock().Statements.Nodes
	if len(statements) == 0 || !ast.IsReturnStatement(statements[len(statements)-1]) {
		return nil
	}
	var allocation *ast.Node
	var returns []typefacts.Location
	valid := true
	var walk func(*ast.Node)
	walk = func(node *ast.Node) {
		if !valid || node == nil || node != body && literalResultBoundary(node) {
			return
		}
		if ast.IsReturnStatement(node) {
			expression := identityPreservingUnwrap(node.Expression())
			if expression == nil || !ast.IsIdentifier(expression) {
				valid = false
				return
			}
			symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(expression))
			declaration := p.ownLiteralDeclarationLocked(symbol)
			if declaration == nil || !ast.IsVariableDeclaration(declaration) {
				valid = false
				return
			}
			owner := declaration.Parent
			for owner != nil && !literalResultBoundary(owner) {
				owner = owner.Parent
			}
			if owner != callee || allocation != nil && allocation != declaration {
				valid = false
				return
			}
			allocation = declaration
			returns = append(returns, nodeLocation(node))
			return
		}
		node.ForEachChild(func(child *ast.Node) bool { walk(child); return false })
	}
	walk(body)
	if !valid || allocation == nil || len(returns) == 0 {
		return nil
	}
	return &typefacts.LocalLiteralResultPremise{
		Call: nodeLocation(call), Callee: nodeLocation(callee),
		Allocation: nodeLocation(allocation), Returns: returns,
	}
}
