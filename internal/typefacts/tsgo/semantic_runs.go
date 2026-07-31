package tsgo

import (
	"context"
	"fmt"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

type semanticNodeCursor struct {
	sourceFile *ast.SourceFile
	position   int
	node       *ast.Node
	valid      bool
}

func (c *semanticNodeCursor) at(position int) *ast.Node {
	if c.valid && c.position == position {
		return c.node
	}
	c.position = position
	c.valid = true
	c.node = deepestNodeAt(
		ast.GetNodeAtPosition(c.sourceFile, position, false),
		position,
	)
	return c.node
}

// SemanticDemandRuns resolves canonically ordered per-file runs under one
// checker lock. Results are index-aligned with runs and with each run's
// demands; no caller has to flatten or repartition file ownership.
func (p *project) SemanticDemandRuns(
	ctx context.Context,
	runs []typefacts.SemanticDemandRun,
	scope typefacts.SemanticScope,
) ([]typefacts.SemanticDemandRunResult, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, err
	}
	totalDemands := 0
	for index := range runs {
		totalDemands += len(runs[index].Demands)
	}
	results := make([]typefacts.SemanticDemandRunResult, len(runs))
	entityArena := make([]typefacts.EntityFact, totalDemands)
	structuralArena := make([]typefacts.SymbolID, totalDemands)
	nodeArena := make([]*ast.Node, totalDemands)
	symbolArena := make([]*ast.Symbol, totalDemands)
	sourceFiles := make([]*ast.SourceFile, len(runs))
	sourceErrors := make([]error, len(runs))
	evidence := make([]semanticEvidence, len(runs))
	batchStructural := make(map[typefacts.SymbolID]struct{})

	demandOffset := 0
	for runIndex := range runs {
		run := &runs[runIndex]
		path := run.Path
		evidence[runIndex] = newSemanticEvidence(path)
		nextOffset := demandOffset + len(run.Demands)
		results[runIndex].Entities = entityArena[demandOffset:nextOffset:nextOffset]
		results[runIndex].Structural = structuralArena[demandOffset:nextOffset:nextOffset]
		resultNodes := nodeArena[demandOffset:nextOffset:nextOffset]
		resultSymbols := symbolArena[demandOffset:nextOffset:nextOffset]
		demandOffset = nextOffset
		sourceFiles[runIndex], sourceErrors[runIndex] = p.sourceFileFor(typefacts.Location{Path: path})
		cursor := semanticNodeCursor{sourceFile: sourceFiles[runIndex]}
		for demandIndex := range run.Demands {
			demand := &run.Demands[demandIndex]
			if demand.Location.Path != path {
				return nil, fmt.Errorf("semantic demand run %q contains location from %q", path, demand.Location.Path)
			}
			if demand.QueryLocation != nil && demand.QueryLocation.Path != path {
				return nil, fmt.Errorf("semantic demand run %q contains query location from %q", path, demand.QueryLocation.Path)
			}
			location := demand.Location
			location.Path = path
			results[runIndex].Entities[demandIndex].Location = location
			if sourceErrors[runIndex] != nil || sourceFiles[runIndex] == nil {
				continue
			}
			node := cursor.at(location.StartByte)
			resultNodes[demandIndex] = node
			if !demand.StructuralAccessor {
				continue
			}
			if node == nil {
				continue
			}
			symbol := p.checker.GetSymbolAtLocation(node)
			if symbol != nil {
				id := p.idFor(symbol)
				results[runIndex].Structural[demandIndex] = id
				results[runIndex].Entities[demandIndex].Symbol = id
				if jsxTagNameAt(node, location) == node {
					resultSymbols[demandIndex] = symbol
				}
				batchStructural[id] = struct{}{}
				evidence[runIndex].symbol(id)
			}
		}
	}

	structuralSuppressed := func(symbol typefacts.SymbolID) bool {
		if _, hit := batchStructural[symbol]; hit {
			return true
		}
		_, hit := scope.Suppression[symbol]
		return hit
	}
	batchDescriptors := make(map[typefacts.SymbolID]*typefacts.TypeDescriptor)
	cachedDescriptor := func(symbol typefacts.SymbolID) *typefacts.TypeDescriptor {
		if descriptor := batchDescriptors[symbol]; descriptor != nil {
			return descriptor
		}
		return scope.DescriptorSeed[symbol]
	}

	demandOffset = 0
	for runIndex := range runs {
		run := &runs[runIndex]
		result := &results[runIndex]
		path := run.Path
		sourceFile := sourceFiles[runIndex]
		sourceError := sourceErrors[runIndex]
		resultNodes := nodeArena[demandOffset : demandOffset+len(run.Demands)]
		resultSymbols := symbolArena[demandOffset : demandOffset+len(run.Demands)]
		demandOffset += len(run.Demands)
		callDemands := p.callDemandScratch[:0]
		queryCursor := semanticNodeCursor{sourceFile: sourceFile}

		for demandIndex := range run.Demands {
			if err := ctx.Err(); err != nil {
				p.releaseCallDemandScratch(callDemands)
				return nil, err
			}
			demand := &run.Demands[demandIndex]
			entity := &result.Entities[demandIndex]
			if sourceError != nil || sourceFile == nil {
				continue
			}
			if entity.Symbol != "" &&
				!demand.TypeDescriptor &&
				!demand.ResolvedCall &&
				!demand.Callability &&
				!demand.RuntimeValueDomain &&
				!demand.ReferenceSpace &&
				!demand.RuntimeIdentity {
				continue
			}

			location := entity.Location
			resultNode := resultNodes[demandIndex]
			var resultSymbol *ast.Symbol
			if resultNode != nil && (demand.Symbol || demand.ReferenceSpace || demand.RuntimeIdentity) {
				resultSymbol = resultSymbols[demandIndex]
				if resultSymbol == nil {
					resultSymbol = p.checker.GetSymbolAtLocation(jsxTagNameAt(resultNode, location))
				}
				if demand.Symbol && entity.Symbol == "" && resultSymbol != nil {
					entity.Symbol = p.idFor(resultSymbol)
				}
			}
			evidence[runIndex].symbol(entity.Symbol)
			if demand.ReferenceSpace {
				entity.ReferenceSpace = typefacts.ReferenceSpaceNeither
				if resultSymbol != nil {
					entity.ReferenceSpace = p.referenceIndex.spaceFor(p, p.idFor(resultSymbol))
				}
			}
			if demand.RuntimeIdentity && resultSymbol != nil {
				if runtimeSymbol := p.runtimeIdentitySymbol(resultSymbol); runtimeSymbol != nil {
					if ref, ok := durableRuntimeRefFor(runtimeSymbol); ok {
						entity.RuntimeIdentity = ref.runtimeID(p.runtimePath(ref.path))
					}
				}
			}

			query := location
			if demand.QueryLocation != nil {
				query = *demand.QueryLocation
				query.Path = path
			}
			queryNode := resultNode
			if query.StartByte != location.StartByte || query.EndByte != location.EndByte {
				queryNode = queryCursor.at(query.StartByte)
			}
			var queryType *checker.Type
			queryTypeLoaded := false
			if demand.TypeDescriptor && queryNode != nil && !structuralSuppressed(entity.Symbol) {
				if descriptor := cachedDescriptor(entity.Symbol); descriptor != nil {
					entity.TypeDescriptor = descriptor
				} else {
					queryType = p.checker.GetTypeAtLocation(queryNode)
					queryTypeLoaded = true
					if queryType != nil {
						entity.TypeDescriptor = p.typeDescriptorFor(queryType)
						if entity.Symbol != "" {
							batchDescriptors[entity.Symbol] = entity.TypeDescriptor
						}
					}
				}
				evidence[runIndex].descriptor(entity.TypeDescriptor)
			}
			if (demand.Callability || demand.RuntimeValueDomain) && queryNode != nil {
				if !queryTypeLoaded {
					queryType = p.checker.GetTypeAtLocation(queryNode)
				}
				if demand.Callability {
					entity.Callability = callabilityOfType(p.checker, queryType)
				}
				if demand.RuntimeValueDomain {
					domain := runtimeValueDomainOfType(p.checker, queryType)
					entity.RuntimeValueDomain = &domain
				}
			}
			if demand.ResolvedCall {
				node := queryNode
				for node != nil && !isCallLikeExpression(node) {
					node = node.Parent
				}
				argumentCount := 0
				if node != nil {
					argumentCount = len(node.Arguments())
				}
				callDemands = append(callDemands, resolvedCallDemand{
					entityIndex:   demandIndex,
					node:          node,
					argumentCount: argumentCount,
				})
			}
		}
		if len(callDemands) != 0 {
			if err := p.resolveCallRunLocked(ctx, sourceFile, callDemands, result.Entities, &evidence[runIndex]); err != nil {
				p.releaseCallDemandScratch(callDemands)
				return nil, err
			}
		}
		p.releaseCallDemandScratch(callDemands)
		result.Dependencies, result.Durable = evidence[runIndex].finish()
	}
	return results, nil
}

func (p *project) releaseCallDemandScratch(demands []resolvedCallDemand) {
	clear(demands)
	p.callDemandScratch = demands[:0]
}
