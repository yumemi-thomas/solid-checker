package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

const maxImmutableCalleeAliasDepth = 8

// Only identifier aliases are followed. Calls, computed members, caller
// parameters, imported bindings, and values produced by factories state no
// fact. Each copied binding has one const declaration in this same runtime
// file, no write, and an initializer before its use in the chain.
func (p *project) immutableCalleeAliasLocked(call *ast.Node) *typefacts.ImmutableCalleeAlias {
	if !ast.IsCallExpression(call) {
		return nil
	}
	expression := identityPreservingUnwrap(call.Expression())
	file := ast.GetSourceFileOfNode(call)
	if file == nil || expression == nil || !ast.IsIdentifier(expression) {
		return nil
	}
	result := &typefacts.ImmutableCalleeAlias{}
	seen := make(map[*ast.Symbol]bool)
	for range maxImmutableCalleeAliasDepth {
		symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(expression))
		if symbol == nil || seen[symbol] {
			return nil
		}
		seen[symbol] = true
		_, _, _, resolved := p.implementationCallTargetLocked(expression)
		if resolved == nil {
			return nil
		}
		if resolved.StandardLibrary {
			if len(result.Bindings) == 0 ||
				!p.isDefaultLibraryMemberLocked(symbol, symbol.Name, nil) {
				return nil
			}
			var receiver *ast.Symbol
			if ast.IsPropertyAccessExpression(expression) {
				object := identityPreservingUnwrap(expression.Expression())
				if object == nil || !ast.IsIdentifier(object) {
					return nil
				}
				receiver = p.canonicalSymbol(p.checker.GetSymbolAtLocation(object))
				if receiver == nil || !p.isDefaultLibraryMemberLocked(receiver, receiver.Name, nil) {
					return nil
				}
				_, _, _, result.Receiver = p.implementationCallTargetLocked(object)
				if result.Receiver == nil || !result.Receiver.StandardLibrary {
					return nil
				}
			} else if !ast.IsIdentifier(expression) {
				return nil
			}
			if !p.immutableAliasLibrarySourceIsStable(file, symbol, receiver) {
				return nil
			}
			result.Expression = nodeLocation(expression)
			result.Declaration = *resolved
			result.DefaultLibraryInvoker, result.InvokedArguments =
				p.defaultLibraryInvokerForCalleeLocked(expression, false)
			return result
		}
		if !ast.IsIdentifier(expression) || len(symbol.Declarations) != 1 {
			return nil
		}
		declaration := symbol.Declarations[0]
		if ast.GetSourceFileOfNode(declaration) != file || p.symbolIsAssignedLocked(symbol, declaration) {
			return nil
		}
		if len(result.Bindings) != 0 && callableBodyDeclaration(symbol) != nil {
			result.Expression = nodeLocation(expression)
			result.Declaration = *resolved
			return result
		}
		if !ast.IsVariableDeclaration(declaration) || !ast.IsVarConst(declaration) ||
			declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) ||
			declaration.End() > nodeLocation(expression).StartByte {
			return nil
		}
		initializer := identityPreservingUnwrap(declaration.Initializer())
		if initializer == nil || !(ast.IsIdentifier(initializer) || ast.IsPropertyAccessExpression(initializer)) {
			return nil
		}
		result.Bindings = append(result.Bindings, typefacts.ImmutableCalleeAliasBinding{
			Declaration: *resolved, Initializer: nodeLocation(initializer),
		})
		expression = initializer
	}
	return nil
}

// Library identity alone does not prove that this file copied the library's
// original value. Refuse writes to the terminal or receiver, and any escape of
// the receiver object (including passing it to defineProperty/assign or making
// another alias). A direct member may only be read or called, never used as a
// write target. This is deliberately conservative across the whole file.
func (p *project) immutableAliasLibrarySourceIsStable(file *ast.SourceFile, target, receiver *ast.Symbol) bool {
	stable := true
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || !stable || ast.IsPartOfTypeNode(node) {
			return
		}
		if ast.IsIdentifier(node) || ast.IsPropertyAccessExpression(node) || nodeKindName(node) == "ElementAccessExpression" {
			symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(node))
			if symbol == target || (receiver != nil && symbol == receiver) {
				if ast.GetAssignmentTarget(node) != nil ||
					(node.Parent != nil && node.Parent.KindString() == "KindDeleteExpression") {
					stable = false
					return
				}
				if receiver != nil && symbol == receiver {
					parent := node.Parent
					if parent == nil || !ast.IsPropertyAccessExpression(parent) || parent.Expression() != node ||
						ast.GetAssignmentTarget(parent) != nil ||
						(parent.Parent != nil && parent.Parent.KindString() == "KindDeleteExpression") {
						stable = false
						return
					}
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool { visit(child); return false })
	}
	visit(file.AsNode())
	return stable
}
