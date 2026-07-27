package tsgo

import (
	"context"
	"path/filepath"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

// SemanticEntities resolves the experimental demand payload under one checker
// lock. Missing fields are represented by their zero value, matching the
// tolerant behavior of the individual Project queries.
func (p *project) SemanticEntities(ctx context.Context, demands []typefacts.EntityDemand) ([]typefacts.EntityFact, error) {
	entities, _, err := p.SemanticEntitiesScoped(ctx, demands, nil, nil)
	return entities, err
}

// SemanticEntitiesScoped resolves a demand batch whose output must match a
// larger batch's semantics: suppression carries the structural-accessor
// symbols of demands outside this batch, descriptorSeed carries type
// descriptors those outside demands already computed (the batch-wide
// first-wins descriptor dedup), and the returned slice reports this batch's
// structural-accessor symbol per demand (empty where not applicable) so the
// caller can maintain the union. Nil arguments restrict both to this batch
// alone, which is exactly SemanticEntities.
func (p *project) SemanticEntitiesScoped(ctx context.Context, demands []typefacts.EntityDemand, suppression map[typefacts.SymbolID]struct{}, descriptorSeed map[typefacts.SymbolID]*typefacts.TypeDescriptor) ([]typefacts.EntityFact, []typefacts.SymbolID, error) {
	if err := ctx.Err(); err != nil {
		return nil, nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, nil, ErrClosed
	}
	// suppression and descriptorSeed are read for the duration of this call
	// only, under the project lock, while the caller blocks — so they are
	// borrowed rather than copied. suppression is the project-wide
	// structural-accessor union and descriptorSeed every retained descriptor;
	// duplicating both per batch cost more than most batches themselves. This
	// batch's own additions layer on top in small local maps.
	prefetchedSymbols := make(map[int]typefacts.SymbolID)
	batchStructural := make(map[typefacts.SymbolID]struct{})
	structuralSuppressed := func(symbol typefacts.SymbolID) bool {
		if _, hit := batchStructural[symbol]; hit {
			return true
		}
		_, hit := suppression[symbol]
		return hit
	}
	structural := make([]typefacts.SymbolID, len(demands))
	var markerPath, markerCleanPath string
	var markerSourceFile *ast.SourceFile
	var markerSourceError error
	for index, demand := range demands {
		if !demand.StructuralAccessor {
			continue
		}
		if demand.Location.Path != markerPath {
			markerPath = demand.Location.Path
			markerCleanPath = filepath.Clean(markerPath)
			markerSourceFile, markerSourceError = p.sourceFileFor(typefacts.Location{Path: markerCleanPath})
		}
		if markerSourceError != nil || markerSourceFile == nil {
			continue
		}
		node := deepestNodeAt(ast.GetNodeAtPosition(markerSourceFile, demand.Location.StartByte, false), demand.Location.StartByte)
		if node == nil {
			continue
		}
		if symbol := p.checker.GetSymbolAtLocation(node); symbol != nil {
			id := p.idFor(symbol)
			prefetchedSymbols[index] = id
			batchStructural[id] = struct{}{}
			structural[index] = id
		}
	}
	result := make([]typefacts.EntityFact, 0, len(demands))
	var currentDemandPath, currentCleanPath string
	var currentSourceFile *ast.SourceFile
	var currentSourceError error
	var currentSemanticDiagnostics []*ast.Diagnostic
	batchDescriptors := make(map[typefacts.SymbolID]*typefacts.TypeDescriptor)
	cachedDescriptor := func(symbol typefacts.SymbolID) *typefacts.TypeDescriptor {
		if descriptor := batchDescriptors[symbol]; descriptor != nil {
			return descriptor
		}
		return descriptorSeed[symbol]
	}
	for demandIndex, demand := range demands {
		if err := ctx.Err(); err != nil {
			return nil, nil, err
		}
		location := demand.Location
		entity := typefacts.EntityFact{Location: location}
		if symbol := prefetchedSymbols[demandIndex]; symbol != "" {
			entity.Symbol = symbol
		}
		if location.Path != currentDemandPath {
			currentDemandPath = location.Path
			currentCleanPath = filepath.Clean(location.Path)
			currentSourceFile, currentSourceError = p.sourceFileFor(typefacts.Location{Path: currentCleanPath})
			currentSemanticDiagnostics = nil
		}
		location.Path = currentCleanPath
		entity.Location.Path = currentCleanPath
		if currentSourceError != nil || currentSourceFile == nil {
			result = append(result, entity)
			continue
		}
		if entity.Symbol != "" && !demand.TypeDescriptor && !demand.ResolvedCall && !demand.Callability && !demand.ReferenceSpace && !demand.RuntimeIdentity {
			result = append(result, entity)
			continue
		}
		resultNode := deepestNodeAt(ast.GetNodeAtPosition(currentSourceFile, location.StartByte, false), location.StartByte)
		var resultSymbol *ast.Symbol
		if resultNode != nil && (demand.Symbol || demand.ReferenceSpace || demand.RuntimeIdentity) {
			resultSymbol = p.checker.GetSymbolAtLocation(resultNode)
			if demand.Symbol && entity.Symbol == "" && resultSymbol != nil {
				entity.Symbol = p.idFor(resultSymbol)
			}
		}
		if demand.ReferenceSpace {
			entity.ReferenceSpace = typefacts.ReferenceSpaceNeither
			if resultSymbol != nil {
				entity.ReferenceSpace = p.referenceIndex.spaceFor(p, p.idFor(resultSymbol))
			}
		}
		if demand.RuntimeIdentity && resultSymbol != nil {
			runtimeSymbol := p.canonicalSymbol(resultSymbol)
			if runtimeSymbol.Flags&ast.SymbolFlagsValue != 0 {
				if ref, ok := durableRuntimeRefFor(runtimeSymbol); ok {
					entity.RuntimeIdentity = ref.runtimeID()
				}
			}
		}
		query := location
		if demand.QueryLocation != nil {
			query = *demand.QueryLocation
		}
		queryNode := resultNode
		if query.StartByte != location.StartByte || query.EndByte != location.EndByte {
			queryNode = deepestNodeAt(ast.GetNodeAtPosition(currentSourceFile, query.StartByte, false), query.StartByte)
		}
		if demand.TypeDescriptor && queryNode != nil {
			if !structuralSuppressed(entity.Symbol) {
				if cached := cachedDescriptor(entity.Symbol); cached != nil {
					entity.TypeDescriptor = cached
				} else if value := p.checker.GetTypeAtLocation(queryNode); value != nil {
					descriptor := p.typeDescriptorFor(value)
					if entity.Symbol != "" {
						batchDescriptors[entity.Symbol] = descriptor
					}
					entity.TypeDescriptor = descriptor
				}
			}
		}
		if demand.Callability && queryNode != nil {
			entity.Callability = callabilityOfType(p.checker, p.checker.GetTypeAtLocation(queryNode))
		}
		if demand.ResolvedCall && queryNode != nil {
			entity.ResolvedCall = &typefacts.Call{
				Validity: typefacts.ResolvedCallUnresolved,
				Kind:     typefacts.CallKindUnknown,
			}
			node := queryNode
			for node != nil && !isCallLikeExpression(node) {
				node = node.Parent
			}
			if node != nil {
				if ast.IsNewExpression(node) {
					entity.ResolvedCall.Kind = typefacts.CallKindConstruct
				} else {
					entity.ResolvedCall.Kind = typefacts.CallKindCall
				}
				callee := node.Expression()
				target := p.checker.GetSymbolAtLocation(callee)
				signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
				candidates := []*checker.Signature{}
				_ = checker.Checker_getResolvedSignature(p.checker, node, &candidates, checker.CheckModeNormal)
				if target != nil {
					target = p.canonicalSymbol(target)
					entity.ResolvedCall.Target = p.idFor(target)
					if current, ok := p.symbolFor(entity.ResolvedCall.Target); ok {
						target = current
					}
				}
				if signature != nil {
					entity.ResolvedCall.Validity = typefacts.ResolvedCallValid
					if len(candidates) == 0 || signature.Flags()&checker.SignatureFlagsIsSignatureCandidateForOverloadFailure != 0 {
						entity.ResolvedCall.Validity = typefacts.ResolvedCallRecovery
					} else {
						if currentSemanticDiagnostics == nil {
							currentSemanticDiagnostics = p.program.GetSemanticDiagnostics(ctx, currentSourceFile)
						}
						if hasCallResolutionDiagnostic(node, currentSourceFile, currentSemanticDiagnostics) {
							entity.ResolvedCall.Validity = typefacts.ResolvedCallRecovery
						}
					}
					returnType := checker.Checker_getReturnTypeOfSignature(p.checker, signature)
					if returnType != nil {
						entity.ResolvedCall.ReturnTypeText = p.typeDescriptorFor(returnType).Text
					}
					if entity.ResolvedCall.Validity == typefacts.ResolvedCallValid {
						calleeType := p.checker.GetTypeAtLocation(callee)
						if calleeType != nil && calleeType.Flags()&checker.TypeFlagsUnion != 0 {
							entity.ResolvedCall.Arguments = unresolvedArgumentMappings(
								node.Arguments(),
								typefacts.ArgumentMappingCompositeSignature,
							)
						} else {
							declaration := p.currentSignatureDeclaration(signature, target)
							entity.ResolvedCall.Declaration = p.resolvedDeclaration(signature, declaration, target)
							entity.ResolvedCall.Arguments = p.argumentMappings(node, signature, declaration)
						}
					} else {
						entity.ResolvedCall.Arguments = unresolvedArgumentMappings(
							node.Arguments(),
							typefacts.ArgumentMappingRecoverySignature,
						)
					}
				} else {
					entity.ResolvedCall.Arguments = unresolvedArgumentMappings(
						node.Arguments(),
						typefacts.ArgumentMappingCallUnresolved,
					)
				}
			}
		} else if demand.ResolvedCall {
			entity.ResolvedCall = &typefacts.Call{
				Validity: typefacts.ResolvedCallUnresolved,
				Kind:     typefacts.CallKindUnknown,
			}
		}
		result = append(result, entity)
	}
	return result, structural, nil
}

func isCallLikeExpression(node *ast.Node) bool {
	return ast.IsCallExpression(node) || ast.IsNewExpression(node)
}

type resolvedDeclarationCacheKey struct {
	signature *checker.Signature
	fallback  *ast.Symbol
}

type resolvedParameterCacheKey struct {
	signature     *checker.Signature
	argumentIndex int
}

func (p *project) currentSignatureDeclaration(signature *checker.Signature, target *ast.Symbol) *ast.Node {
	declaration := signature.Declaration()
	if declaration == nil {
		return nil
	}
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return nil
	}
	if p.isCurrentSourceFile(sourceFile) {
		return declaration
	}
	target = p.canonicalSymbol(target)
	if target == nil {
		return nil
	}

	ordinal := 0
	if declarationSymbol := declaration.Symbol(); declarationSymbol != nil {
		for _, candidate := range declarationSymbol.Declarations {
			if candidate.Kind != declaration.Kind {
				continue
			}
			if candidate == declaration {
				break
			}
			ordinal++
		}
	}
	matches := make([]*ast.Node, 0, ordinal+1)
	for _, root := range target.Declarations {
		if root.Kind == declaration.Kind {
			matches = append(matches, root)
			continue
		}
		var visit func(*ast.Node) bool
		visit = func(node *ast.Node) bool {
			if node.Kind == declaration.Kind {
				matches = append(matches, node)
				return false
			}
			node.ForEachChild(visit)
			return false
		}
		root.ForEachChild(visit)
	}
	if ordinal < len(matches) {
		return matches[ordinal]
	}
	return nil
}

func (p *project) isCurrentSourceFile(sourceFile *ast.SourceFile) bool {
	if p.currentSourceFiles == nil {
		p.currentSourceFiles = make(map[*ast.SourceFile]struct{}, len(p.program.SourceFiles()))
		for _, current := range p.program.SourceFiles() {
			p.currentSourceFiles[current] = struct{}{}
		}
	}
	_, current := p.currentSourceFiles[sourceFile]
	return current
}

func (p *project) resolvedDeclaration(signature *checker.Signature, node *ast.Node, fallbackSymbol *ast.Symbol) *typefacts.ResolvedDeclaration {
	key := resolvedDeclarationCacheKey{signature: signature, fallback: fallbackSymbol}
	if cached := p.resolvedDeclarations[key]; cached != nil {
		return cached
	}
	if node == nil {
		return nil
	}
	sourceFile := ast.GetSourceFileOfNode(node)
	if sourceFile == nil {
		return nil
	}
	nameNode := node.Name()
	if nameNode == nil {
		nameNode = node
	}
	symbol := node.Symbol()
	if (ast.IsArrowFunction(node) || ast.IsFunctionExpression(node)) && fallbackSymbol != nil {
		// Anonymous function expressions often carry an internal signature
		// symbol. The callable declaration exposed to consumers is the
		// compiler-resolved callee symbol that owns that expression.
		symbol = fallbackSymbol
	}
	if symbol == nil && node.Name() != nil {
		symbol = p.checker.GetSymbolAtLocation(node.Name())
	}
	if symbol == nil {
		symbol = fallbackSymbol
	}
	kind := strings.TrimPrefix(node.KindString(), "Kind")
	result := &typefacts.ResolvedDeclaration{
		Name:       "",
		Kind:       kind,
		Location:   typefacts.Location{Path: filepath.Clean(sourceFile.FileName()), StartByte: scanner.SkipTrivia(sourceFile.Text(), nameNode.Pos()), EndByte: nameNode.End()},
		SourceFile: filepath.Clean(sourceFile.FileName()),
	}
	if symbol != nil {
		symbol = p.canonicalSymbol(symbol)
		result.Symbol = p.idFor(symbol)
		result.OriginModule = declarationModule(symbol)
		if !strings.HasPrefix(symbol.Name, ast.InternalSymbolNamePrefix) {
			result.Name = symbol.Name
		}
	}
	switch kind {
	case "Constructor":
		result.Name = "constructor"
	case "ArrowFunction", "FunctionExpression":
		if result.Name == "" {
			result.Name = "call"
		}
	case "CallSignature", "FunctionType":
		result.Name = "call"
	case "ConstructSignature", "ConstructorType":
		result.Name = "construct"
	}
	for owner := node.Parent; owner != nil && owner.Parent != nil; owner = owner.Parent {
		ownerSymbol := owner.Symbol()
		if ownerSymbol == nil && owner.Name() != nil {
			ownerSymbol = p.checker.GetSymbolAtLocation(owner.Name())
		}
		if ownerSymbol == nil {
			continue
		}
		ownerSource := ast.GetSourceFileOfNode(owner)
		if ownerSource == nil {
			continue
		}
		ownerSymbol = p.canonicalSymbol(ownerSymbol)
		if ownerSymbol == symbol {
			continue
		}
		name := owner.Name()
		if name == nil {
			name = owner
		}
		result.Owners = append(result.Owners, typefacts.DeclarationOwner{
			Symbol: p.idFor(ownerSymbol),
			Name:   ownerSymbol.Name,
			Kind:   strings.TrimPrefix(owner.KindString(), "Kind"),
			Location: typefacts.Location{
				Path:      filepath.Clean(ownerSource.FileName()),
				StartByte: scanner.SkipTrivia(ownerSource.Text(), name.Pos()),
				EndByte:   name.End(),
			},
		})
	}
	for left, right := 0, len(result.Owners)-1; left < right; left, right = left+1, right-1 {
		result.Owners[left], result.Owners[right] = result.Owners[right], result.Owners[left]
	}
	qualified := make([]string, 0, len(result.Owners)+1)
	for _, owner := range result.Owners {
		if owner.Name != "" && !strings.HasPrefix(owner.Name, ast.InternalSymbolNamePrefix) {
			qualified = append(qualified, owner.Name)
		}
	}
	if result.Name != "" {
		qualified = append(qualified, result.Name)
	}
	result.QualifiedName = strings.Join(qualified, ".")
	result.StandardLibrary = p.program.IsSourceFileDefaultLibrary(sourceFile.Path())
	if p.resolvedDeclarations == nil {
		p.resolvedDeclarations = make(map[resolvedDeclarationCacheKey]*typefacts.ResolvedDeclaration)
	}
	p.resolvedDeclarations[key] = result
	return result
}

func (p *project) argumentMappings(call *ast.Node, signature *checker.Signature, signatureDeclaration *ast.Node) []typefacts.ArgumentMapping {
	arguments := call.Arguments()
	result := make([]typefacts.ArgumentMapping, 0, len(arguments))
	parameters := signature.Parameters()
	for argumentIndex, argument := range arguments {
		if ast.IsSpreadElement(argument) {
			result = append(result, typefacts.ArgumentMapping{
				ArgumentIndex: argumentIndex,
				Status:        typefacts.ArgumentMappingUnresolved,
				Unresolved:    typefacts.ArgumentMappingSpreadArgument,
			})
			continue
		}
		parameterKey := resolvedParameterCacheKey{signature: signature, argumentIndex: argumentIndex}
		if cached := p.resolvedParameters[parameterKey]; cached != nil {
			result = append(result, typefacts.ArgumentMapping{
				ArgumentIndex: argumentIndex,
				Status:        typefacts.ArgumentMappingResolved,
				Parameter:     cached,
			})
			continue
		}
		formalIndex := argumentIndex
		if formalIndex >= len(parameters) {
			if !signature.HasRestParameter() || len(parameters) == 0 {
				result = append(result, typefacts.ArgumentMapping{
					ArgumentIndex: argumentIndex,
					Status:        typefacts.ArgumentMappingUnresolved,
					Unresolved:    typefacts.ArgumentMappingParameterUnavailable,
				})
				continue
			}
			formalIndex = len(parameters) - 1
		}
		parameter := parameters[formalIndex]
		if signatureDeclaration != nil && formalIndex < len(signatureDeclaration.Parameters()) {
			currentParameter := signatureDeclaration.Parameters()[formalIndex]
			if currentParameter.Symbol() != nil {
				parameter = currentParameter.Symbol()
			} else if currentParameter.Name() != nil {
				if symbol := p.checker.GetSymbolAtLocation(currentParameter.Name()); symbol != nil {
					parameter = symbol
				}
			}
		}
		parameterType := checker.Checker_getTypeAtPosition(p.checker, signature, argumentIndex)
		rest := signature.HasRestParameter() && formalIndex == len(parameters)-1
		fact := &typefacts.ParameterFact{
			Index:       formalIndex,
			Rest:        rest,
			Optional:    !rest && formalIndex >= signature.MinArgumentCount(),
			Callability: callabilityOfType(p.checker, parameterType),
		}
		if parameter != nil {
			fact.Optional = fact.Optional || !rest && parameter.Flags&ast.SymbolFlagsOptional != 0
			fact.Symbol = p.idFor(parameter)
			if declarations := declarationsForSymbol(parameter); len(declarations) != 0 {
				fact.Declaration = &declarations[0]
			}
		}
		if parameterType != nil {
			fact.TypeDescriptor = p.typeDescriptorFor(parameterType)
		}
		if p.resolvedParameters == nil {
			p.resolvedParameters = make(map[resolvedParameterCacheKey]*typefacts.ParameterFact)
		}
		p.resolvedParameters[parameterKey] = fact
		result = append(result, typefacts.ArgumentMapping{
			ArgumentIndex: argumentIndex,
			Status:        typefacts.ArgumentMappingResolved,
			Parameter:     fact,
		})
	}
	return result
}

func unresolvedArgumentMappings(arguments []*ast.Node, reason typefacts.ArgumentMappingReason) []typefacts.ArgumentMapping {
	result := make([]typefacts.ArgumentMapping, 0, len(arguments))
	for index := range arguments {
		result = append(result, typefacts.ArgumentMapping{
			ArgumentIndex: index,
			Status:        typefacts.ArgumentMappingUnresolved,
			Unresolved:    reason,
		})
	}
	return result
}

func (p *project) typeDescriptorFor(value *checker.Type) *typefacts.TypeDescriptor {
	if descriptor := p.typeDescriptors[value]; descriptor != nil {
		return descriptor
	}
	descriptor := &typefacts.TypeDescriptor{Text: p.checker.TypeToString(value)}
	if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
		descriptor.AliasDeclarations = declarationsForSymbol(alias.Symbol())
		descriptor.OriginModule = declarationModule(alias.Symbol())
	}
	if p.typeDescriptors == nil {
		p.typeDescriptors = make(map[*checker.Type]*typefacts.TypeDescriptor)
	}
	p.typeDescriptors[value] = descriptor
	return descriptor
}

func hasCallResolutionDiagnostic(call *ast.Node, sourceFile *ast.SourceFile, diagnostics []*ast.Diagnostic) bool {
	for _, diagnostic := range diagnostics {
		if diagnostic.Pos() < call.Pos() || diagnostic.End() > call.End() {
			continue
		}
		switch diagnostic.Code() {
		case 2344, 2345, 2348, 2349, 2350, 2379, 2554, 2555, 2558, 2635,
			2677, 2721, 2722, 2723, 2757, 2769, 2794, 2810, 6234:
			node := deepestNodeAt(ast.GetNodeAtPosition(sourceFile, diagnostic.Pos(), false), diagnostic.Pos())
			for node != nil && !isCallLikeExpression(node) {
				node = node.Parent
			}
			if node == call {
				return true
			}
		}
	}
	return false
}

func callabilityOfType(typeChecker *checker.Checker, value *checker.Type) typefacts.Callability {
	if value == nil || value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
		return typefacts.CallabilityUnknown
	}
	constituents := value.Distributed()
	if len(constituents) == 0 {
		return typefacts.CallabilityUnknown
	}
	callable, nonCallable := false, false
	for _, constituent := range constituents {
		if constituent == nil || constituent.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever|checker.TypeFlagsIncludesError) != 0 {
			return typefacts.CallabilityUnknown
		}
		if len(typeChecker.GetSignaturesOfType(constituent, checker.SignatureKindCall)) != 0 {
			callable = true
		} else {
			nonCallable = true
		}
	}
	switch {
	case callable && nonCallable:
		return typefacts.CallabilityMixed
	case callable:
		return typefacts.CallabilityCallable
	default:
		return typefacts.CallabilityNonCallable
	}
}
