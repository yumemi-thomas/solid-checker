package tsgo

import (
	"context"
	"fmt"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
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

// covering returns the smallest syntax node that contains the complete
// demanded range. Type/value queries describe expressions, not merely the
// deepest token at their first byte: `factory()` and `factory` start at the
// same offset but have different runtime domains and callability.
func (c *semanticNodeCursor) covering(start int, end int) *ast.Node {
	if end <= start {
		return c.at(start)
	}
	return deepestNodeCovering(c.sourceFile.AsNode(), start, end)
}

func deepestNodeCovering(node *ast.Node, start int, end int) *ast.Node {
	if node == nil || node.Pos() > start || node.End() < end {
		return nil
	}
	best := node
	node.ForEachChild(func(child *ast.Node) bool {
		if child.Pos() <= start && end <= child.End() {
			if candidate := deepestNodeCovering(child, start, end); candidate != nil {
				best = candidate
			}
			return true
		}
		return false
	})
	return best
}

// exactCallLikeAt selects the call-like expression reached from the demanded
// start byte only when its emitted source span is exactly the demanded span.
// Pos includes leading trivia, while all producer locations use SkipTrivia.
// Walking parents preserves resolvedCall's call-like lookup, but the end check
// prevents a callee or enclosing expression from answering this field.
func (c *semanticNodeCursor) exactCallLikeAt(start int, end int) *ast.Node {
	if c.sourceFile == nil {
		return nil
	}
	for node := c.at(start); node != nil; node = node.Parent {
		if !isCallLikeExpression(node) || node.End() != end {
			continue
		}
		if scanner.SkipTrivia(c.sourceFile.Text(), node.Pos()) == start {
			return node
		}
	}
	return nil
}

// exactExpressionAt selects only an expression whose trivia-normalized source
// span equals the demand. Unlike covering, it can never answer from a child of
// the requested expression.
func (c *semanticNodeCursor) exactExpressionAt(start int, end int) *ast.Node {
	if c.sourceFile == nil || end <= start {
		return nil
	}
	for node := c.at(start); node != nil; node = node.Parent {
		if !ast.IsExpression(node) || node.End() != end {
			continue
		}
		if scanner.SkipTrivia(c.sourceFile.Text(), node.Pos()) == start {
			return node
		}
	}
	return nil
}

// resolvedCallLikeAt keeps the historical start-byte anchor for resolvedCall,
// but chooses the outermost call-like ancestor contained by the demanded span.
// Chained calls share a start byte, so stopping at the first ancestor would
// describe `factory()` instead of `factory()(value)`.
func (c *semanticNodeCursor) resolvedCallLikeAt(start int, end int) *ast.Node {
	var first *ast.Node
	var best *ast.Node
	for node := c.at(start); node != nil; node = node.Parent {
		if !isCallLikeExpression(node) {
			continue
		}
		if first == nil {
			first = node
		}
		if node.End() <= end && (best == nil || node.End() > best.End()) {
			best = node
		}
	}
	if best != nil {
		return best
	}
	return first
}

// callResultDomainAtLocked applies the same call-resolution recovery guard as
// resolvedCall before classifying the call expression's result type. A
// recovery signature can still expose the declared return type, which is not
// safe evidence for a result-domain query.
func (p *project) callResultDomainAtLocked(
	ctx context.Context,
	sourceFile *ast.SourceFile,
	node *ast.Node,
) (typefacts.RuntimeValueDomain, error) {
	signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
	if signature == nil {
		return unknownRuntimeValueDomain(), nil
	}
	target := p.checker.GetSymbolAtLocation(node.Expression())
	if target != nil {
		target = p.canonicalSymbol(target)
	}
	validity, _, err := p.resolvedCallValidityAndCalleeTypeLocked(ctx, sourceFile, node, signature, target)
	if err != nil {
		return typefacts.RuntimeValueDomain{}, err
	}
	if validity != typefacts.ResolvedCallValid {
		return unknownRuntimeValueDomain(), nil
	}
	return runtimeValueDomainOfType(p.checker, p.checker.GetTypeAtLocation(node)), nil
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
				!demand.Constructability &&
				!demand.RuntimeValueDomain &&
				!demand.PrimitiveValueDomain &&
				!demand.PrimitiveLiteralCandidates &&
				!demand.ParameterObjectShape &&
				!demand.CallResultDomain &&
				!demand.ConstantValue &&
				!demand.ArrayShape &&
				!demand.TupleShape &&
				!demand.LibraryTypes &&
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
				if demand.Symbol && entity.Symbol == "" && resultSymbol == nil {
					entity.SymbolUnresolved = true
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
			queryNode := queryCursor.covering(query.StartByte, query.EndByte)
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
			if (demand.Callability || demand.Constructability || demand.RuntimeValueDomain) && queryNode != nil {
				if !queryTypeLoaded {
					queryType = p.checker.GetTypeAtLocation(queryNode)
					queryTypeLoaded = true
				}
				if demand.Callability {
					entity.Callability = callabilityOfType(p.checker, queryType)
				}
				if demand.Constructability {
					entity.Constructability = constructabilityOfType(p.checker, queryType)
				}
				if demand.RuntimeValueDomain {
					domain := runtimeValueDomainOfType(p.checker, queryType)
					entity.RuntimeValueDomain = &domain
				}
			}
			if demand.PrimitiveValueDomain || demand.PrimitiveLiteralCandidates {
				if primitiveNode := queryCursor.exactExpressionAt(query.StartByte, query.EndByte); primitiveNode != nil {
					primitiveType := queryType
					if !queryTypeLoaded || primitiveNode != queryNode {
						primitiveType = p.checker.GetTypeAtLocation(primitiveNode)
					}
					if demand.PrimitiveValueDomain {
						entity.PrimitiveValueDomain = primitiveValueDomainOfType(p.checker, primitiveType)
					}
					if demand.PrimitiveLiteralCandidates {
						entity.PrimitiveLiteralCandidates = primitiveLiteralCandidatesOfType(p.checker, primitiveType)
					}
				}
			}
			if demand.CallResultDomain {
				if callNode := queryCursor.exactCallLikeAt(query.StartByte, query.EndByte); callNode != nil {
					domain, err := p.callResultDomainAtLocked(ctx, sourceFile, callNode)
					if err != nil {
						return nil, err
					}
					entity.CallResultDomain = &domain
				}
			}
			if demand.ConstantValue {
				if constantNode := queryCursor.exactExpressionAt(query.StartByte, query.EndByte); constantNode != nil {
					entity.ConstantValue = p.constantValueAtLocked(constantNode, &evidence[runIndex])
				}
			}
			if demand.ArrayShape {
				if shapeNode := queryCursor.exactExpressionAt(query.StartByte, query.EndByte); shapeNode != nil {
					entity.ArrayShape = p.arrayShapeAtLocked(shapeNode, &evidence[runIndex])
				}
			}
			if demand.TupleShape {
				if shapeNode := queryCursor.exactExpressionAt(query.StartByte, query.EndByte); shapeNode != nil {
					entity.TupleShape = p.tupleShapeAtLocked(shapeNode, &evidence[runIndex])
				}
			}
			if demand.LibraryTypes {
				if typeNode := queryCursor.exactExpressionAt(query.StartByte, query.EndByte); typeNode != nil {
					entity.LibraryTypes = p.libraryTypesAtLocked(typeNode, &evidence[runIndex])
				}
			}
			if demand.ResolvedCall {
				// Call lookup is anchored at the first token and walks outward;
				// callers may demand a statement-sized range including a trailing
				// semicolon. Type/value domains above instead classify the complete
				// demanded expression range.
				node := queryCursor.resolvedCallLikeAt(query.StartByte, query.EndByte)
				argumentCount := 0
				if node != nil {
					argumentCount = len(node.Arguments())
				}
				callDemands = append(callDemands, resolvedCallDemand{
					entityIndex:   demandIndex,
					node:          node,
					argumentCount: argumentCount,
					objectShape:   demand.ParameterObjectShape,
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
