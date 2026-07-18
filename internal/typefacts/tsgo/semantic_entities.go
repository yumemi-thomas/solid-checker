package tsgo

import (
	"context"
	"path/filepath"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-check/internal/typefacts"
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
	prefetchedSymbols := make(map[int]typefacts.SymbolID)
	structuralAccessorSymbols := make(map[typefacts.SymbolID]struct{}, len(suppression))
	for symbol := range suppression {
		structuralAccessorSymbols[symbol] = struct{}{}
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
			structuralAccessorSymbols[id] = struct{}{}
			structural[index] = id
		}
	}
	result := make([]typefacts.EntityFact, 0, len(demands))
	var currentDemandPath, currentCleanPath string
	var currentSourceFile *ast.SourceFile
	var currentSourceError error
	descriptorCache := make(map[typefacts.SymbolID]*typefacts.TypeDescriptor, len(descriptorSeed))
	for symbol, descriptor := range descriptorSeed {
		descriptorCache[symbol] = descriptor
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
		}
		location.Path = currentCleanPath
		entity.Location.Path = currentCleanPath
		if currentSourceError != nil || currentSourceFile == nil {
			result = append(result, entity)
			continue
		}
		if entity.Symbol != "" && !demand.TypeDescriptor && !demand.ResolvedCall {
			result = append(result, entity)
			continue
		}
		resultNode := deepestNodeAt(ast.GetNodeAtPosition(currentSourceFile, location.StartByte, false), location.StartByte)
		if demand.Symbol && entity.Symbol == "" && resultNode != nil {
			if symbol := p.checker.GetSymbolAtLocation(resultNode); symbol != nil {
				entity.Symbol = p.idFor(symbol)
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
			if _, structural := structuralAccessorSymbols[entity.Symbol]; !structural {
				if cached := descriptorCache[entity.Symbol]; cached != nil {
					entity.TypeDescriptor = cached
				} else if value := p.checker.GetTypeAtLocation(queryNode); value != nil {
					descriptor := typefacts.TypeDescriptor{Text: p.checker.TypeToString(value)}
					if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
						descriptor.AliasDeclarations = declarationsForSymbol(alias.Symbol())
						descriptor.OriginModule = declarationModule(alias.Symbol())
					}
					if entity.Symbol != "" {
						descriptorCache[entity.Symbol] = &descriptor
					}
					entity.TypeDescriptor = &descriptor
				}
			}
		}
		if demand.ResolvedCall && queryNode != nil {
			node := queryNode
			for node != nil && !ast.IsCallExpression(node) {
				node = node.Parent
			}
			if node != nil {
				callee := node.AsCallExpression().Expression
				target := p.checker.GetSymbolAtLocation(callee)
				signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
				if target != nil && signature != nil {
					returnType := checker.Checker_getReturnTypeOfSignature(p.checker, signature)
					if returnType != nil {
						entity.ResolvedCall = &typefacts.Call{
							Target:         p.idFor(p.canonicalSymbol(target)),
							ReturnType:     p.idForType(returnType),
							ReturnTypeText: p.checker.TypeToString(returnType),
						}
					}
				}
			}
		}
		result = append(result, entity)
	}
	return result, structural, nil
}
