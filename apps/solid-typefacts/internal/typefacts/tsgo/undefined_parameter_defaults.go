package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// Called only after the initial-read census excludes parameter initializers,
// rest/destructuring, nested callables, arguments and eval. Exactly one store
// may target this parameter, under a direct body-level `if (p === void 0)`.
// A defined caller value therefore survives every execution of the body. An
// undefined caller value instead gets a local empty array: that branch is not
// a read of the caller's value and must never justify a guaranteed read.
type undefinedParameterDefaultRead struct {
	fact *typefacts.UndefinedParameterDefault
	use  typefacts.Location
}

func (p *project) undefinedParameterDefaultsLocked(body *ast.Node, symbols map[int]*ast.Symbol, fileChecker *checker.Checker) map[int]undefinedParameterDefaultRead {
	tracked := make(map[*ast.Symbol]int)
	for index, symbol := range symbols {
		if symbol == nil {
			return nil
		}
		if _, duplicate := tracked[symbol]; duplicate {
			return nil
		}
		tracked[symbol] = index
	}
	writes := make(map[int][]*ast.Node)
	references := make(map[int][]*ast.Node)
	invalid := make(map[int]bool)
	unsupported := false
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if nodeKindName(node) == "WithStatement" {
			unsupported = true
			return
		}
		if ast.IsIdentifier(node) && !ast.IsPartOfTypeNode(node) {
			if index, ok := tracked[p.canonicalSymbol(fileChecker.GetSymbolAtLocation(node))]; ok {
				references[index] = append(references[index], node)
			}
			if assignment := ast.GetAssignmentTarget(node); assignment != nil {
				if index, ok := tracked[p.assignedBindingSymbol(fileChecker, node)]; ok {
					writes[index] = append(writes[index], assignment)
				}
			} else if ast.IsDeclarationNameOrImportPropertyName(node) {
				if index, ok := tracked[p.canonicalSymbol(fileChecker.GetSymbolAtLocation(node))]; ok {
					// Includes `var p = ...`, whose initializer is not an
					// AssignmentExpression but can replace the same binding.
					invalid[index] = true
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool { visit(child); return false })
	}
	visit(body)
	if unsupported {
		return nil
	}
	defaults := make(map[int]undefinedParameterDefaultRead)
	for _, statement := range body.AsBlock().Statements.Nodes {
		if nodeKindName(statement) != "IfStatement" {
			continue
		}
		branch := statement.AsIfStatement()
		condition := identityPreservingUnwrap(branch.Expression)
		if branch.ElseStatement != nil || condition == nil || !ast.IsBinaryExpression(condition) {
			continue
		}
		comparison := condition.AsBinaryExpression()
		left, right := identityPreservingUnwrap(comparison.Left), identityPreservingUnwrap(comparison.Right)
		if nodeKindName(comparison.OperatorToken) != "EqualsEqualsEqualsToken" || left == nil || !ast.IsIdentifier(left) || right == nil || nodeKindName(right) != "VoidExpression" {
			continue
		}
		zero := right.Expression()
		if zero == nil || !ast.IsNumericLiteral(zero) || zero.Text() != "0" {
			continue
		}
		index, ok := tracked[p.canonicalSymbol(fileChecker.GetSymbolAtLocation(left))]
		if !ok || invalid[index] || len(writes[index]) != 1 || !ast.IsBlock(branch.ThenStatement) {
			continue
		}
		statements := branch.ThenStatement.AsBlock().Statements.Nodes
		if len(statements) != 1 || nodeKindName(statements[0]) != "ExpressionStatement" {
			continue
		}
		store := identityPreservingUnwrap(statements[0].Expression())
		if store == nil || !ast.IsBinaryExpression(store) || writes[index][0] != store {
			continue
		}
		assignment := store.AsBinaryExpression()
		if assignment.OperatorToken.Kind != ast.KindEqualsToken || !ast.IsIdentifier(assignment.Left) ||
			p.canonicalSymbol(fileChecker.GetSymbolAtLocation(assignment.Left)) != symbols[index] ||
			!ast.IsArrayLiteralExpression(assignment.Right) || len(assignment.Right.AsArrayLiteralExpression().Elements.Nodes) != 0 {
			continue
		}
		// Do not infer possibility from a conditional identity alone. An
		// earlier reference could save `p === void 0` and gate the later read
		// to fallback-only executions. Bind just the first other reference,
		// and require it to occur after this guard. Thus nothing before this
		// read can distinguish the caller argument through this binding.
		var first *ast.Node
		for _, reference := range references[index] {
			if reference == left || reference == assignment.Left {
				continue
			}
			if first == nil || nodeLocation(reference).StartByte < nodeLocation(first).StartByte {
				first = reference
			}
		}
		if first == nil || nodeLocation(first).StartByte < nodeLocation(statement).EndByte {
			continue
		}
		defaults[index] = undefinedParameterDefaultRead{
			fact: &typefacts.UndefinedParameterDefault{Guard: nodeLocation(statement), Assignment: nodeLocation(store)},
			use:  nodeLocation(first),
		}
	}
	return defaults
}
