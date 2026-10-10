package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// A limit withholds the entire tree. Neither an empty list nor an enumerated
// prefix can stand in for a closed member census.
const maxReturnStructureNodes = 128
const maxReturnStructureDepth = 8

func (p *project) returnStructureLocked(implementation, expression *ast.Node) *typefacts.ReturnStructure {
	root := identityPreservingUnwrap(expression)
	if root == nil || !(ast.IsArrayLiteralExpression(root) || ast.IsObjectLiteralExpression(root)) {
		return nil
	}
	remaining := maxReturnStructureNodes
	var visit func(*ast.Node, int) *typefacts.ReturnStructure
	visit = func(node *ast.Node, depth int) *typefacts.ReturnStructure {
		node = identityPreservingUnwrap(node)
		if node == nil || depth > maxReturnStructureDepth || remaining == 0 {
			return nil
		}
		remaining--
		result := &typefacts.ReturnStructure{Location: nodeLocation(node), Kind: "leaf"}
		if ast.IsArrayLiteralExpression(node) {
			result.Kind = "tuple"
			for _, item := range node.AsArrayLiteralExpression().Elements.Nodes {
				if item == nil || ast.IsSpreadElement(item) || nodeKindName(item) == "OmittedExpression" {
					return nil
				}
				child := visit(item, depth+1)
				if child == nil {
					return nil
				}
				result.Items = append(result.Items, *child)
			}
			result.Complete = true
			return result
		}
		if ast.IsObjectLiteralExpression(node) {
			result.Kind = "object"
			names := make(map[string]bool)
			for _, property := range node.AsObjectLiteralExpression().Properties.Nodes {
				// Ordinary assignments, and (ADR 0181) a shorthand `{ value }`,
				// which is `{ value: value }`: its key and its value are the one
				// identifier, whose value symbol the leaf resolves
				// (referenceSymbolLocked). A shorthand default (`{ value = 1 }`)
				// is only a destructuring target and stays refused, as do
				// numeric and computed keys.
				if property == nil {
					return nil
				}
				shorthand := ast.IsShorthandPropertyAssignment(property)
				if shorthand {
					if property.AsShorthandPropertyAssignment().ObjectAssignmentInitializer != nil {
						return nil
					}
				} else if nodeKindName(property) != "PropertyAssignment" {
					return nil
				}
				key := property.Name()
				if key == nil || !(ast.IsIdentifier(key) || (!shorthand && ast.IsStringLiteral(key))) {
					return nil
				}
				name := key.Text()
				if name == "__proto__" || names[name] {
					return nil
				}
				names[name] = true
				value := key
				if !shorthand {
					value = property.Initializer()
				}
				child := visit(value, depth+1)
				if child == nil {
					return nil
				}
				result.Properties = append(result.Properties, typefacts.ReturnStructureProperty{Name: name, Key: nodeLocation(key), Value: *child})
			}
			result.Complete = true
			return result
		}
		result.PrimitiveSyntax = p.primitiveBySyntaxLocked(node, 0)
		result.Parameter = p.unwrittenParameterIdentityLocked(implementation, node)
		result.Sources = p.returnValueSourcesLocked(node)
		result.DefaultLibraryCall = p.defaultLibraryCallLocked(node)
		return result
	}
	return visit(root, 0)
}
