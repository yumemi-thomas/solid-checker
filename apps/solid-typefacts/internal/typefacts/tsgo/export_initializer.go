package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// An internal source derivation, not an export-value verdict. Its eventual
// consumer must authenticate the imported factory's exact return contract.
// Nodes stay inside the producer; no compiler node crosses a fact boundary.
type exportInitializerDerivation struct {
	bindings        []*ast.Node
	call            *ast.Node
	callee          *ast.Symbol
	specifier       string
	exportName      string
	objectArguments map[int]*ast.Node
}

func (p *project) exportInitializerLocked(query *ast.Node) *exportInitializerDerivation {
	if query == nil || !ast.IsIdentifier(query) {
		return nil
	}
	target := p.canonicalSymbol(p.checker.GetSymbolAtLocation(query))
	if target == nil || target.ValueDeclaration == nil {
		return nil
	}
	file := ast.GetSourceFileOfNode(target.ValueDeclaration)
	if file == nil || file.IsDeclarationFile || !p.isCurrentSourceFile(file) || mentionsArgumentsOrEval(file.AsNode()) {
		return nil
	}
	seen := make(map[*ast.Symbol]bool)
	result := &exportInitializerDerivation{objectArguments: make(map[int]*ast.Node)}
	for range 16 {
		if target == nil || seen[target] || len(target.Declarations) != 1 {
			return nil
		}
		seen[target] = true
		binding := target.ValueDeclaration
		if binding == nil || !ast.IsVariableDeclaration(binding) || ast.GetSourceFileOfNode(binding) != file || !ast.IsIdentifier(binding.Name()) || p.symbolIsAssignedLocked(target, binding) {
			return nil
		}
		list := binding.Parent
		if list == nil || list.Parent == nil || !ast.IsVariableStatement(list.Parent) || list.Parent.Parent != file.AsNode() {
			return nil
		}
		result.bindings = append(result.bindings, binding)
		initializer := identityPreservingUnwrap(binding.Initializer())
		if initializer == nil {
			return nil
		}
		if ast.IsIdentifier(initializer) {
			target = p.canonicalSymbol(p.checker.GetSymbolAtLocation(initializer))
			if target == nil || target.ValueDeclaration == nil || initializer.Pos() < target.ValueDeclaration.End() {
				return nil
			}
			continue
		}
		if !ast.IsCallExpression(initializer) || initializer.QuestionDotToken() != nil {
			return nil
		}
		callee := identityPreservingUnwrap(initializer.Expression())
		if callee == nil || !ast.IsIdentifier(callee) {
			return nil
		}
		alias := p.checker.GetSymbolAtLocation(callee)
		specifier, exportName := importedAliasIdentity(alias)
		canonical := p.canonicalSymbol(alias)
		if specifier == "" || exportName == "" || canonical == nil || canonical.ValueDeclaration == nil {
			return nil
		}
		arguments := initializer.AsCallExpression().Arguments
		if arguments == nil {
			return nil
		}
		for index, argument := range arguments.Nodes {
			if nodeKindName(argument) == "SpreadElement" {
				return nil
			}
			value := identityPreservingUnwrap(argument)
			if value != nil && ast.IsObjectLiteralExpression(value) {
				result.objectArguments[index] = value
			}
		}
		if len(result.objectArguments) == 0 {
			return nil
		}
		result.call, result.callee = initializer, canonical
		result.specifier, result.exportName = specifier, exportName
		return result
	}
	return nil
}

func (p *project) exportInitializerTranscriptLocked(location typefacts.Location) *typefacts.ExportInitializerTranscript {
	file, err := p.sourceFileFor(location)
	if err != nil {
		return nil
	}
	cursor := semanticNodeCursor{sourceFile: file}
	query := cursor.exactExpressionAt(location.StartByte, location.EndByte)
	derivation := p.exportInitializerLocked(query)
	if derivation == nil {
		return nil
	}
	declaration := p.resolvedDeclaration(nil, derivation.callee.ValueDeclaration, derivation.callee)
	if declaration == nil {
		return nil
	}
	result := &typefacts.ExportInitializerTranscript{
		Location: location, Target: p.idFor(p.canonicalSymbol(p.checker.GetSymbolAtLocation(query))),
		Call: nodeLocation(derivation.call), Callee: nodeLocation(identityPreservingUnwrap(derivation.call.Expression())),
		Declaration: *declaration, Specifier: derivation.specifier, ExportName: derivation.exportName,
	}
	for _, binding := range derivation.bindings {
		resolved := p.resolvedDeclaration(nil, binding, p.checker.GetSymbolAtLocation(binding.Name()))
		if resolved == nil {
			return nil
		}
		result.Bindings = append(result.Bindings, typefacts.ExportInitializerBinding{
			Declaration: *resolved, Location: nodeLocation(binding), Initializer: nodeLocation(identityPreservingUnwrap(binding.Initializer())),
		})
	}
	// Source argument order is deterministic; iterating the internal map is not.
	for index := range derivation.call.AsCallExpression().Arguments.Nodes {
		if object := derivation.objectArguments[index]; object != nil {
			result.ObjectArguments = append(result.ObjectArguments, typefacts.ExportInitializerObjectArgument{Index: index, Location: nodeLocation(object)})
		}
	}
	return result
}
