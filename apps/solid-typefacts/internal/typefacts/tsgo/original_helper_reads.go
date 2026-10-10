package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// One local hop only. No callback, cross-file, recursion, alias, spread, or
// defaulted-input proof is inferred. The limits bound extra helper censuses.
func (p *project) originalHelperReadsLocked(fn *ast.Node, signature *typefacts.SelectedSignature, calls []typefacts.ImplementationCall, uses []typefacts.ParameterUse) []typefacts.OriginalHelperRead {
	if signature == nil || len(calls) > 128 || implementationCompletionForm(fn) != typefacts.CompletionPlain || mentionsArgumentsOrEval(ast.GetSourceFileOfNode(fn).AsNode()) {
		return nil
	}
	nested := false
	var scan func(*ast.Node)
	scan = func(n *ast.Node) {
		if n == nil {
			return
		}
		if isCallableDeclaration(n) {
			nested = true
			return
		}
		n.ForEachChild(func(c *ast.Node) bool { scan(c); return nested })
	}
	scan(fn.Body())
	symbols := map[int]*ast.Symbol{}
	seen := map[*ast.Symbol]bool{}
	for i, param := range fn.Parameters() {
		if param.Name() == nil || !ast.IsIdentifier(param.Name()) || param.AsParameterDeclaration().DotDotDotToken != nil || i >= len(signature.Parameters) {
			return nil
		}
		scan(param.Initializer())
		symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(param.Name()))
		if symbol == nil || len(symbol.Declarations) != 1 || seen[symbol] {
			return nil
		}
		seen[symbol] = true
		symbols[i] = symbol
	}
	if nested {
		return nil
	}
	cutoffs := p.positionalOriginalInputCutoffsLocked(fn.Body(), symbols, p.formChecker())
	if cutoffs == nil {
		return nil
	}
	for _, param := range fn.Parameters() {
		if param.Initializer() == nil {
			continue
		}
		for index, cutoff := range p.positionalOriginalInputCutoffsLocked(param.Initializer(), symbols, p.formChecker()) {
			if old, ok := cutoffs[index]; !ok || cutoff < old {
				cutoffs[index] = cutoff
			}
		}
	}
	admissible := map[typefacts.Location]typefacts.ImplementationCall{}
	for _, call := range calls {
		if !call.Captured && call.Reach != typefacts.Unreachable && call.Declaration != nil {
			admissible[call.Location] = call
		}
	}
	var found []typefacts.OriginalHelperRead
	helperCount := 0
	var walk func(*ast.Node)
	walk = func(n *ast.Node) {
		if n == nil || helperCount > 32 || len(found) > 256 {
			return
		}
		if ast.IsCallExpression(n) {
			call, ok := admissible[nodeLocation(n)]
			callee := p.formRuntimeCalleeDeclarationLocked(n)
			name := identityPreservingUnwrap(n.Expression())
			if ok && callee != nil && callee != fn && name != nil && ast.IsIdentifier(name) && ast.GetSourceFileOfNode(callee) == ast.GetSourceFileOfNode(fn) && implementationCompletionForm(callee) == typefacts.CompletionPlain {
				symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(name))
				if symbol != nil && len(symbol.Declarations) == 1 && !p.symbolIsAssignedLocked(symbol, symbol.Declarations[0]) {
					helperCount++
					helperCalls := p.implementationCallCensusLocked(callee)
					for slot, arg := range n.Arguments() {
						if ast.IsSpreadElement(arg) {
							break
						}
						arg = identityPreservingUnwrap(arg)
						if arg == nil || !ast.IsIdentifier(arg) || slot >= len(callee.Parameters()) {
							continue
						}
						actual := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(arg))
						for index, wanted := range symbols {
							parameter := signature.Parameters[index]
							if actual != wanted || fn.Parameters()[index].Initializer() != nil || parameter.Index != index || parameter.Rest || parameter.Defaulted || parameter.Declaration == nil || parameter.Declaration.Location != nodeLocation(fn.Parameters()[index].Name()) {
								continue
							}
							// Source discovery alone cannot bind a row to an aliased or
							// otherwise unsupported census argument. Publish only the
							// exact coordinates that the client independently joins.
							if call.Target == "" || slot >= len(call.ArgumentParameters) || call.ArgumentParameters[slot] == nil || call.ArgumentParameters[slot].ParameterIndex != index || len(call.ArgumentParameters[slot].Path) != 0 {
								continue
							}
							matchingUses := 0
							for _, use := range uses {
								if use.ParameterIndex == index && use.Location == nodeLocation(arg) && len(use.BindingPath) == 0 && !use.Alias && !use.Captured && use.Reach != typefacts.Unreachable && use.Kind == typefacts.ParameterUseArgumentKnown {
									matchingUses++
								}
							}
							if matchingUses != 1 || !locationEncloses(call.Location, nodeLocation(arg)) {
								continue
							}
							if cutoff, written := cutoffs[index]; written && nodeLocation(arg).EndByte > cutoff {
								continue
							}
							helperParam := callee.Parameters()[slot]
							origin := p.returnedParameterIdentityLocked(callee, helperParam.Name())
							if origin == nil || origin.ParameterIndex != slot || len(origin.Path) != 0 {
								continue
							}
							if !locationEncloses(nodeLocation(callee), call.Declaration.Location) || !locationEncloses(nodeLocation(callee), nodeLocation(helperParam.Name())) {
								continue
							}
							for _, read := range helperCalls {
								root := read.CalleeParameter
								if read.Kind != typefacts.CallKindCall || read.Captured || read.Reach == typefacts.Unreachable || root == nil || root.ParameterIndex != slot || len(root.Path) != 1 || root.Path[0].Property == "" {
									continue
								}
								if !locationEncloses(nodeLocation(callee), read.Location) {
									continue
								}
								found = append(found, typefacts.OriginalHelperRead{ParameterIndex: index, Declaration: parameter.Declaration.Location, Call: call.Location, ArgumentIndex: slot, Argument: nodeLocation(arg), Helper: call.Declaration.Location, HelperImplementation: nodeLocation(callee), HelperParameter: nodeLocation(helperParam.Name()), Read: read.Location, Property: root.Path[0].Property})
							}
							break
						}
					}
				}
			}
		}
		n.ForEachChild(func(c *ast.Node) bool { walk(c); return false })
	}
	walk(fn.Body())
	if helperCount > 32 || len(found) > 256 {
		return nil
	}
	return found
}
