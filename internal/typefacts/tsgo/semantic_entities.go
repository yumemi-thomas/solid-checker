package tsgo

import (
	"context"
	"path/filepath"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
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
					descriptor := typefacts.TypeDescriptor{Text: p.checker.TypeToString(value)}
					if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
						descriptor.AliasDeclarations = declarationsForSymbol(alias.Symbol())
						descriptor.OriginModule = declarationModule(alias.Symbol())
					}
					if entity.Symbol != "" {
						batchDescriptors[entity.Symbol] = &descriptor
					}
					entity.TypeDescriptor = &descriptor
				}
			}
		}
		if demand.Callability && queryNode != nil {
			entity.Callability = callabilityOfType(p.checker, p.checker.GetTypeAtLocation(queryNode))
		}
		if demand.ResolvedCall && queryNode != nil {
			entity.ResolvedCall = &typefacts.Call{Validity: typefacts.ResolvedCallUnresolved}
			node := queryNode
			for node != nil && !ast.IsCallExpression(node) {
				node = node.Parent
			}
			if node != nil {
				callee := node.AsCallExpression().Expression
				target := p.checker.GetSymbolAtLocation(callee)
				signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
				candidates := []*checker.Signature{}
				_ = checker.Checker_getResolvedSignature(p.checker, node, &candidates, checker.CheckModeNormal)
				if target != nil {
					entity.ResolvedCall.Target = p.idFor(p.canonicalSymbol(target))
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
						entity.ResolvedCall.ReturnTypeText = p.checker.TypeToString(returnType)
					}
				}
			}
		} else if demand.ResolvedCall {
			entity.ResolvedCall = &typefacts.Call{Validity: typefacts.ResolvedCallUnresolved}
		}
		result = append(result, entity)
	}
	return result, structural, nil
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
			for node != nil && !ast.IsCallExpression(node) {
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
