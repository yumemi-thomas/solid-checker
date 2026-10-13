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
//
// **One escape shape is reported rather than refused** (ADR 0112). A container
// handed to `Container.member.bind(Container)` is the `thisArg` of a bound
// function whose target is a member of that same container, and
// `Function.prototype.bind` neither mutates its argument nor invokes anything.
// Whether the *bound target* can mutate it is the consumer's reviewed question,
// exactly as which members a domain may close on already is, so the qualified
// member is returned for the consumer to decide on and this walk states no
// opinion about it. Every other escape still refuses here.
//
// `stable` false means "an escape this walk cannot describe". `stable` true
// with a non-empty `escapes` means "no escape but the reviewable ones, and here
// they are"; a caller that has not reviewed them must treat that as unstable.
func (p *project) immutableAliasLibrarySourceIsStable(file *ast.SourceFile, target, receiver *ast.Symbol) bool {
	stable, escapes := p.immutableAliasLibrarySourceStability(file, target, receiver)
	return stable && len(escapes) == 0
}

// immutableAliasLibrarySourceStability is
// [project.immutableAliasLibrarySourceIsStable] with ADR 0112's reviewable
// escapes reported instead of folded into the verdict.
//
// The escapes are qualified member names (`Object.is`), deduplicated, in source
// order of first occurrence, so a receipt records the same list a reader sees.
func (p *project) immutableAliasLibrarySourceStability(
	file *ast.SourceFile,
	target, receiver *ast.Symbol,
) (bool, []string) {
	stable := true
	var escapes []string
	seen := map[string]struct{}{}
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
						if qualified := p.reviewableContainerEscapeLocked(node, receiver); qualified != "" {
							if _, held := seen[qualified]; !held {
								seen[qualified] = struct{}{}
								escapes = append(escapes, qualified)
							}
							return
						}
						stable = false
						return
					}
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool { visit(child); return false })
	}
	visit(file.AsNode())
	if !stable {
		return false, nil
	}
	return true, escapes
}

// reviewableContainerEscapeLocked answers `Container.member` when `node` is the
// container in exactly `Container.member.bind(Container)`, and "" for every
// other position (ADR 0112).
//
// Four premises, all required, because each is a way the shape could fail to be
// the one reviewed:
//
//   - `node` is argument **0** of a call — the `thisArg` slot. A later argument
//     is a value the bound target receives, which is a different claim;
//   - the callee is a property access named `bind`, resolving to a
//     default-library member, so it is `Function.prototype.bind` and not a
//     `bind` this file or its dependencies installed;
//   - the object that `bind` is read from is itself a property access whose
//     object resolves to the **same** container symbol, so the bound target is
//     a member of the thing being handed over rather than of anything else;
//   - the member resolves to a default-library member of that container, which
//     is what makes the returned name meaningful to the consumer's table.
func (p *project) reviewableContainerEscapeLocked(node *ast.Node, receiver *ast.Symbol) string {
	call := node.Parent
	if call == nil || !ast.IsCallExpression(call) || call.QuestionDotToken() != nil {
		return ""
	}
	arguments := call.Arguments()
	if len(arguments) == 0 || arguments[0] != node {
		return ""
	}
	bindAccess := identityPreservingUnwrap(call.Expression())
	if bindAccess == nil || !ast.IsPropertyAccessExpression(bindAccess) ||
		bindAccess.Name() == nil || bindAccess.Name().Text() != "bind" {
		return ""
	}
	bindSymbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(bindAccess))
	if bindSymbol == nil || !p.isDefaultLibraryMemberLocked(bindSymbol, "bind", nil) {
		return ""
	}
	memberAccess := identityPreservingUnwrap(bindAccess.Expression())
	if memberAccess == nil || !ast.IsPropertyAccessExpression(memberAccess) || memberAccess.Name() == nil {
		return ""
	}
	container := identityPreservingUnwrap(memberAccess.Expression())
	if container == nil || !ast.IsIdentifier(container) {
		return ""
	}
	if p.canonicalSymbol(p.checker.GetSymbolAtLocation(container)) != receiver {
		return ""
	}
	member := p.canonicalSymbol(p.checker.GetSymbolAtLocation(memberAccess))
	if member == nil {
		return ""
	}
	containers := map[string]struct{}{containerInterfaceName(receiver.Name): {}}
	if !p.isDefaultLibraryMemberLocked(member, memberAccess.Name().Text(), containers) {
		return ""
	}
	return receiver.Name + "." + memberAccess.Name().Text()
}
